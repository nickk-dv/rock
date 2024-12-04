use super::context::HirCtx;
use crate::ast::{self, BasicType};
use crate::error::{Error, ErrorSink};
use crate::hir;
use crate::intern::LitID;
use crate::text::TextRange;
use std::collections::HashSet;

//@slice elem_ty, ref_ty not handled for Error, trying to avoid them for now
pub fn match_kind<'hir>(ty: hir::Type<'hir>) -> Result<hir::MatchKind, bool> {
    match ty {
        hir::Type::Error => Err(false),
        hir::Type::InferDef(_, _) => Err(true),
        hir::Type::Basic(basic) => {
            if let Some(int_ty) = hir::BasicInt::from_basic(basic) {
                Ok(hir::MatchKind::Int { int_ty })
            } else if basic == BasicType::Bool {
                Ok(hir::MatchKind::Bool)
            } else if basic == BasicType::Char {
                Ok(hir::MatchKind::Char)
            } else {
                Err(true)
            }
        }
        //@gen types not handled
        hir::Type::Enum(enum_id, _) => Ok(hir::MatchKind::Enum {
            enum_id,
            ref_mut: None,
        }),
        //@gen types not handled
        hir::Type::Struct(_, _) => Err(true),
        hir::Type::Reference(mutt, ref_ty) => match *ref_ty {
            //@gen types not handled
            hir::Type::Enum(enum_id, _) => Ok(hir::MatchKind::Enum {
                enum_id,
                ref_mut: Some(mutt),
            }),
            _ => Err(true),
        },
        hir::Type::MultiReference(_, _) => Err(true),
        hir::Type::Procedure(_) => Err(true),
        hir::Type::ArraySlice(slice) => {
            if matches!(slice.elem_ty, hir::Type::Basic(BasicType::U8)) {
                Ok(hir::MatchKind::String)
            } else {
                Err(true)
            }
        }
        hir::Type::ArrayStatic(_) => Err(true),
    }
}

pub fn match_cov<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    kind: hir::MatchKind,
    arms: &mut [hir::MatchArm<'hir>],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
) {
    let mut cov = PatCov::new(); //@cache? can it be re-used without collision?

    match kind {
        hir::MatchKind::Int { int_ty } => {
            cov.cov_int.reset();
            match_cov_int(ctx, &mut cov.cov_int, arms, arms_ast, match_kw, int_ty)
        }
        hir::MatchKind::Bool => {
            cov.cov_bool.reset();
            match_cov_bool(ctx, &mut cov.cov_bool, arms, arms_ast, match_kw);
        }
        hir::MatchKind::Char => {
            cov.cov_char.reset();
            match_cov_char(ctx, &mut cov.cov_char, arms, arms_ast, match_kw);
        }
        hir::MatchKind::String => {
            cov.cov_string.reset();
            match_cov_string(ctx, &mut cov.cov_string, arms, arms_ast, match_kw);
        }
        hir::MatchKind::Enum { enum_id, .. } => {
            cov.cov_enum.reset();
            match_cov_enum(ctx, &mut cov.cov_enum, arms, arms_ast, match_kw, enum_id);
        }
    }
}

fn match_cov_int(
    ctx: &mut HirCtx,
    cov: &mut PatCovInt<i128>,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
    int_ty: hir::BasicInt,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_int(ctx, cov, *pat, range, int_ty)
                }
            }
            _ => pat_cov_int(ctx, cov, arm.pat, pat_ast.range, int_ty),
        }
    }

    let ptr_width = ctx.session.config.target_ptr_width;
    let not_covered = cov.not_covered(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width));

    if !not_covered.is_empty() {
        let mut msg = String::from("patterns not covered:\n");
        let src = ctx.src(match_kw);

        for value in not_covered {
            match value.display() {
                RangeIncDisplay::Collapsed(value) => msg.push_str(&format!("- `{}`\n", value)),
                RangeIncDisplay::Range(range) => {
                    msg.push_str(&format!("- `{}..={}`\n", range.start, range.end))
                }
            }
        }
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn pat_cov_int(
    ctx: &mut HirCtx,
    cov: &mut PatCovInt<i128>,
    pat: hir::Pat,
    pat_range: TextRange,
    int_ty: hir::BasicInt,
) {
    let ptr_width = ctx.session.config.target_ptr_width;

    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width)),
        hir::Pat::Lit(value) => {
            let value = value.into_int();
            let range = RangeInc::new(value, value);
            cov.cover(range)
        }
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.resolved_unwrap();
            let value = ctx.const_intern.get(value_id);

            let value = value.into_int();
            let range = RangeInc::new(value, value);
            cov.cover(range)
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn match_cov_bool(
    ctx: &mut HirCtx,
    cov: &mut PatCovBool,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_bool(ctx, cov, *pat, range)
                }
            }
            _ => pat_cov_bool(ctx, cov, arm.pat, pat_ast.range),
        }
    }

    let not_covered = cov.not_covered();
    if !not_covered.is_empty() {
        let mut msg = String::from("patterns not covered:\n");
        for &value in not_covered {
            if value {
                msg.push_str("- `true`\n");
            } else {
                msg.push_str("- `false`\n");
            }
        }
        msg.pop();
        let src = ctx.src(match_kw);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn pat_cov_bool(ctx: &mut HirCtx, cov: &mut PatCovBool, pat: hir::Pat, pat_range: TextRange) {
    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(),
        hir::Pat::Lit(value) => match value {
            hir::ConstValue::Bool { val } => cov.cover(val),
            _ => unreachable!(),
        },
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.resolved_unwrap();
            let value = ctx.const_intern.get(value_id);

            match value {
                hir::ConstValue::Bool { val } => cov.cover(val),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn match_cov_char(
    ctx: &mut HirCtx,
    cov: &mut PatCovChar,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_char(ctx, cov, *pat, range)
                }
            }
            _ => pat_cov_char(ctx, cov, arm.pat, pat_ast.range),
        }
    }

    if cov.not_covered() {
        let msg = "patterns not covered:\n- `_`";
        let src = ctx.src(match_kw);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn pat_cov_char(ctx: &mut HirCtx, cov: &mut PatCovChar, pat: hir::Pat, pat_range: TextRange) {
    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(),
        hir::Pat::Lit(value) => match value {
            hir::ConstValue::Char { val } => cov.cover(RangeInc::new(val as u32, val as u32)),
            _ => unreachable!(),
        },
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.resolved_unwrap();
            let value = ctx.const_intern.get(value_id);

            match value {
                hir::ConstValue::Char { val } => cov.cover(RangeInc::new(val as u32, val as u32)),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn match_cov_string(
    ctx: &mut HirCtx,
    cov: &mut PatCovString,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_string(ctx, cov, *pat, range)
                }
            }
            _ => pat_cov_string(ctx, cov, arm.pat, pat_ast.range),
        }
    }

    if cov.not_covered() {
        let msg = "patterns not covered:\n- `_`";
        let src = ctx.src(match_kw);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn pat_cov_string(ctx: &mut HirCtx, cov: &mut PatCovString, pat: hir::Pat, pat_range: TextRange) {
    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(),
        hir::Pat::Lit(value) => match value {
            hir::ConstValue::String { string_lit } => cov.cover(string_lit.id),
            _ => unreachable!(),
        },
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.resolved_unwrap();
            let value = ctx.const_intern.get(value_id);

            match value {
                hir::ConstValue::String { string_lit } => cov.cover(string_lit.id),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn match_cov_enum<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    cov: &mut PatCovEnum,
    arms: &mut [hir::MatchArm<'hir>],
    arms_ast: &[ast::MatchArm],
    match_kw: TextRange,
    enum_id: hir::EnumID,
) {
    let data = ctx.registry.enum_data(enum_id);
    let variant_count = data.variants.len();

    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_enum(ctx, cov, *pat, range, enum_id)
                }
            }
            _ => pat_cov_enum(ctx, cov, arm.pat, pat_ast.range, enum_id),
        }
    }

    let data = ctx.registry.enum_data(enum_id);
    let not_covered = cov.not_covered(variant_count);

    if !not_covered.is_empty() {
        let mut msg = String::from("patterns not covered:\n");
        let src = ctx.src(match_kw);

        for variant_id in not_covered {
            let variant = data.variant(*variant_id);
            let name = ctx.name(variant.name.id);
            msg.push_str(&format!("- `.{name}`\n"));
        }
        ctx.emit.error(Error::new(msg, src, None));
    }
}

fn pat_cov_enum<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    cov: &mut PatCovEnum,
    pat: hir::Pat<'hir>,
    pat_range: TextRange,
    enum_id: hir::EnumID,
) {
    let data = ctx.registry.enum_data(enum_id);
    let variant_count = data.variants.len();

    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(variant_count),
        hir::Pat::Variant(_, variant_id, _) => cov.cover(variant_id, variant_count),
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.resolved_unwrap();
            let value = ctx.const_intern.get(value_id);

            match value {
                hir::ConstValue::Variant { variant } => {
                    cov.cover(variant.variant_id, variant_count)
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

///==================== PATTERN COV STATE ====================

struct PatCov {
    cov_int: PatCovInt<i128>,
    cov_bool: PatCovBool,
    cov_char: PatCovChar,
    cov_string: PatCovString,
    cov_enum: PatCovEnum,
}

struct PatCovBool {
    cov_true: bool,
    cov_false: bool,
    not_covered: Vec<bool>,
}

struct PatCovChar {
    wild_covered: bool,
    covered: PatCovInt<u32>,
}

struct PatCovString {
    wild_covered: bool,
    covered: Vec<LitID>,
}

struct PatCovEnum {
    wild_covered: bool,
    covered: HashSet<hir::VariantID>,
    not_covered: Vec<hir::VariantID>,
}

enum PatCovError {
    CoverFull,
    CoverPartial,
}

///==================== PATTERN COV IMPL ====================

impl PatCov {
    fn new() -> PatCov {
        PatCov {
            cov_int: PatCovInt::new(),
            cov_bool: PatCovBool::new(),
            cov_char: PatCovChar::new(),
            cov_string: PatCovString::new(),
            cov_enum: PatCovEnum::new(),
        }
    }
}

impl PatCovBool {
    fn new() -> PatCovBool {
        PatCovBool {
            cov_true: false,
            cov_false: false,
            not_covered: Vec::with_capacity(2),
        }
    }

    fn reset(&mut self) {
        self.cov_true = false;
        self.cov_false = false;
        self.not_covered.clear();
    }

    fn cover(&mut self, new_value: bool) -> Result<(), PatCovError> {
        if new_value {
            if self.cov_true {
                return Err(PatCovError::CoverFull);
            }
            self.cov_true = true;
        } else {
            if self.cov_false {
                return Err(PatCovError::CoverFull);
            }
            self.cov_false = true;
        }
        Ok(())
    }

    fn cover_wild(&mut self) -> Result<(), PatCovError> {
        if self.cov_true && self.cov_false {
            Err(PatCovError::CoverFull)
        } else {
            self.cov_true = true;
            self.cov_false = true;
            Ok(())
        }
    }

    fn not_covered(&mut self) -> &[bool] {
        if !self.cov_true {
            self.not_covered.push(true);
        }
        if !self.cov_false {
            self.not_covered.push(false);
        }
        &self.not_covered
    }
}

impl PatCovChar {
    fn new() -> PatCovChar {
        PatCovChar {
            wild_covered: false,
            covered: PatCovInt::new(),
        }
    }

    fn reset(&mut self) {
        self.wild_covered = false;
        self.covered.reset();
    }

    fn cover(&mut self, new_range: RangeInc<u32>) -> Result<(), PatCovError> {
        if self.wild_covered {
            Err(PatCovError::CoverFull)
        } else {
            self.covered.cover(new_range)
        }
    }

    fn cover_wild(&mut self) -> Result<(), PatCovError> {
        if self.wild_covered {
            Err(PatCovError::CoverFull)
        } else {
            self.wild_covered = true;
            Ok(())
        }
    }

    fn not_covered(&self) -> bool {
        !self.wild_covered
    }
}

impl PatCovString {
    fn new() -> PatCovString {
        PatCovString {
            wild_covered: false,
            covered: Vec::with_capacity(32),
        }
    }

    fn reset(&mut self) {
        self.wild_covered = false;
        self.covered.clear();
    }

    fn cover(&mut self, new_id: LitID) -> Result<(), PatCovError> {
        if self.wild_covered {
            Err(PatCovError::CoverFull)
        } else if let Some(_) = self
            .covered
            .iter()
            .copied()
            .find(|&cov_id| cov_id == new_id)
        {
            Err(PatCovError::CoverFull)
        } else {
            self.covered.push(new_id);
            Ok(())
        }
    }

    fn cover_wild(&mut self) -> Result<(), PatCovError> {
        if self.wild_covered {
            Err(PatCovError::CoverFull)
        } else {
            self.wild_covered = true;
            Ok(())
        }
    }

    fn not_covered(&self) -> bool {
        !self.wild_covered
    }
}

impl PatCovEnum {
    fn new() -> PatCovEnum {
        PatCovEnum {
            wild_covered: false,
            covered: HashSet::with_capacity(32),
            not_covered: Vec::with_capacity(32),
        }
    }

    fn reset(&mut self) {
        self.wild_covered = false;
        self.covered.clear();
        self.not_covered.clear();
    }

    fn all_covered(&self, variant_count: usize) -> bool {
        self.wild_covered || variant_count == self.covered.len()
    }

    fn cover(
        &mut self,
        variant_id: hir::VariantID,
        variant_count: usize,
    ) -> Result<(), PatCovError> {
        if self.all_covered(variant_count) {
            Err(PatCovError::CoverFull)
        } else if self.covered.contains(&variant_id) {
            Err(PatCovError::CoverFull)
        } else {
            self.covered.insert(variant_id);
            Ok(())
        }
    }

    fn cover_wild(&mut self, variant_count: usize) -> Result<(), PatCovError> {
        if self.all_covered(variant_count) {
            Err(PatCovError::CoverFull)
        } else {
            self.wild_covered = true;
            Ok(())
        }
    }

    fn not_covered(&mut self, variant_count: usize) -> &[hir::VariantID] {
        if self.all_covered(variant_count) {
            &[]
        } else {
            let each_idx = 0..variant_count;
            let variant_iter = each_idx.map(|idx| hir::VariantID::new(idx));
            for variant_id in variant_iter {
                if !self.covered.contains(&variant_id) {
                    self.not_covered.push(variant_id);
                }
            }
            &self.not_covered
        }
    }
}

//@design: are ranges where start > end
// considered empty or just reversed?
// currently flipping them to have accending ordering
#[derive(Copy, Clone)]
struct RangeInc<T> {
    start: T,
    end: T,
}

enum RangeIncDisplay<T> {
    Collapsed(T),
    Range(RangeInc<T>),
}

impl<T> RangeInc<T>
where
    T: Copy + Clone + PartialEq + Ord,
{
    fn new(start: T, end: T) -> RangeInc<T> {
        if start < end {
            RangeInc { start, end }
        } else {
            RangeInc {
                start: end,
                end: start,
            }
        }
    }
    fn display(self) -> RangeIncDisplay<T> {
        if self.start == self.end {
            RangeIncDisplay::Collapsed(self.start)
        } else {
            RangeIncDisplay::Range(self)
        }
    }
}

trait PatCovIncrement<T> {
    fn inc(self) -> T;
    fn dec(self) -> T;
}

impl PatCovIncrement<u32> for u32 {
    fn inc(self) -> u32 {
        self + 1
    }
    fn dec(self) -> u32 {
        self - 1
    }
}

impl PatCovIncrement<i128> for i128 {
    fn inc(self) -> i128 {
        self + 1
    }
    fn dec(self) -> i128 {
        self - 1
    }
}

struct PatCovInt<T>
where
    T: Copy + Clone + PartialEq + Ord,
{
    ranges: Vec<RangeInc<T>>,
    not_covered: Vec<RangeInc<T>>,
}

impl<T> PatCovInt<T>
where
    T: Copy + Clone + PartialEq + Ord + std::fmt::Display + PatCovIncrement<T>,
{
    fn new() -> PatCovInt<T> {
        PatCovInt {
            ranges: Vec::with_capacity(64),
            not_covered: Vec::with_capacity(64),
        }
    }

    fn reset(&mut self) {
        self.ranges.clear();
        self.not_covered.clear();
    }

    fn cover(&mut self, new_range: RangeInc<T>) -> Result<(), PatCovError> {
        let mut idx = 0;

        while idx < self.ranges.len() {
            let range = self.ranges[idx];

            // fully before
            if new_range.end.inc() < range.start {
                break;
            }
            // fully after
            if new_range.start > range.end.inc() {
                idx += 1;
                continue;
            }

            // overlap or touching
            let new_start = new_range.start.min(range.start);
            let mut new_end = new_range.end.max(range.end);
            let mut next_idx = idx + 1;
            let mut remove_range = next_idx..next_idx;

            // merge with existing ranges
            while next_idx < self.ranges.len() {
                let next_range = self.ranges[next_idx].clone();

                if new_end.inc() >= next_range.start {
                    new_end = new_end.max(next_range.end);
                    next_idx += 1;
                    remove_range.end += 1;
                } else {
                    break;
                }
            }

            self.ranges.drain(remove_range);
            self.ranges[idx] = RangeInc::new(new_start, new_end);

            // coverage result
            return if new_range.start >= range.start && new_range.end <= range.end {
                Err(PatCovError::CoverFull)
            } else if new_range.start <= range.end && new_range.end >= range.start {
                Err(PatCovError::CoverPartial)
            } else {
                Ok(())
            };
        }

        self.ranges.insert(idx, new_range);
        Ok(())
    }

    fn cover_wild(&mut self, min: T, max: T) -> Result<(), PatCovError> {
        if self.ranges.len() == 1 {
            let first = self.ranges[0];
            if first.start == min && first.end == max {
                return Err(PatCovError::CoverFull);
            }
        }

        self.ranges.clear();
        self.ranges.push(RangeInc::new(min, max));
        Ok(())
    }

    fn not_covered(&mut self, min: T, max: T) -> &[RangeInc<T>] {
        if let Some(first) = self.ranges.first() {
            if min < first.start {
                let first_gap = RangeInc::new(min, first.start.dec());
                self.not_covered.push(first_gap);
            }
        }

        if self.ranges.is_empty() {
            self.not_covered.push(RangeInc::new(min, max));
        } else {
            for i in 0..self.ranges.len() - 1 {
                let curr_range = &self.ranges[i];
                let next_range = &self.ranges[i + 1];
                if curr_range.end.inc() < next_range.start {
                    let range = RangeInc::new(curr_range.end.inc(), next_range.start.dec());
                    self.not_covered.push(range);
                }
            }
        }

        if let Some(last) = self.ranges.last() {
            if last.end < max {
                let last_gap = RangeInc::new(last.end.inc(), max);
                self.not_covered.push(last_gap);
            }
        }

        &self.not_covered
    }
}
