use super::context::HirCtx;
use super::pass_5::Expectation;
use crate::ast;
use crate::error::{Error, ErrorSink};
use crate::errors as err;
use crate::hir;
use crate::intern::LitID;
use crate::text::TextRange;
use std::collections::HashSet;
use std::fmt::Write;

pub fn match_kind<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    ty: hir::Type<'hir>,
    range: TextRange,
) -> Option<hir::MatchKind<'hir>> {
    let kind = match ty {
        hir::Type::Error => return None,
        hir::Type::Int(int_ty) => Some(hir::MatchKind::Int { int_ty }),
        hir::Type::Bool(bool_ty) => Some(hir::MatchKind::Bool { bool_ty }),
        hir::Type::Char => Some(hir::MatchKind::Char),
        hir::Type::String(hir::StringType::String) => Some(hir::MatchKind::String),
        hir::Type::Enum(enum_id, poly_types) => {
            let poly_types = (!poly_types.is_empty()).then(|| ctx.arena.alloc(poly_types));
            Some(hir::MatchKind::Enum { enum_id, ref_mut: None, poly_types })
        }
        hir::Type::Reference(mutt, ref_ty) => match *ref_ty {
            hir::Type::Error => return None,
            hir::Type::Enum(enum_id, poly_types) => {
                let poly_types = (!poly_types.is_empty()).then(|| ctx.arena.alloc(poly_types));
                Some(hir::MatchKind::Enum { enum_id, ref_mut: Some(mutt), poly_types })
            }
            _ => None,
        },
        _ => None,
    };
    if kind.is_none() {
        let src = ctx.src(range);
        let ty = super::pass_5::type_format(ctx, ty);
        err::tycheck_cannot_match_on_ty(&mut ctx.emit, src, ty.as_str());
    }
    kind
}

pub fn match_pat_expect<'hir>(
    ctx: &HirCtx,
    range: TextRange,
    kind: Option<hir::MatchKind<'hir>>,
) -> (Expectation<'hir>, Option<ast::Mut>) {
    let (expect_ty, ref_mut) = match kind {
        Some(hir::MatchKind::Int { int_ty }) => (hir::Type::Int(int_ty), None),
        Some(hir::MatchKind::Bool { bool_ty }) => (hir::Type::Bool(bool_ty), None),
        Some(hir::MatchKind::Char) => (hir::Type::Char, None),
        Some(hir::MatchKind::String) => (hir::Type::String(hir::StringType::String), None),
        Some(hir::MatchKind::Enum { enum_id, ref_mut, poly_types }) => {
            (hir::Type::Enum(enum_id, poly_types.map_or(&[], |p| *p)), ref_mut)
        }
        None => (hir::Type::Error, None),
    };
    (Expectation::HasType(expect_ty, Some(ctx.src(range))), ref_mut)
}

pub fn match_cov(ctx: &mut HirCtx, kind: hir::MatchKind, check: &CheckContext, error_count: usize) {
    if ctx.emit.did_error(error_count) {
        return;
    }
    for arm in check.arms {
        match arm.pat {
            hir::Pat::Error => return,
            hir::Pat::Or(pats) => {
                if pats.iter().any(|p| matches!(p, hir::Pat::Error)) {
                    return;
                }
            }
            _ => {}
        }
    }
    match kind {
        hir::MatchKind::Int { int_ty } => {
            ctx.pat.cov_int.reset();
            match_cov_int(ctx, check, int_ty);
        }
        hir::MatchKind::Bool { .. } => {
            ctx.pat.cov_bool.reset();
            match_cov_bool(ctx, check);
        }
        hir::MatchKind::Char => {
            ctx.pat.cov_char.reset();
            match_cov_char(ctx, check);
        }
        hir::MatchKind::String => {
            ctx.pat.cov_string.reset();
            match_cov_string(ctx, check);
        }
        hir::MatchKind::Enum { enum_id, .. } => {
            ctx.pat.cov_enum.reset();
            match_cov_enum(ctx, check, enum_id);
        }
    }
}

fn match_cov_int(ctx: &mut HirCtx, check: &CheckContext, int_ty: hir::IntType) {
    pat_cov(ctx, check, int_ty, pat_cov_int);

    if let Some(match_kw) = check.match_kw {
        let ptr_width = ctx.session.config.target_ptr_width;
        let not_covered =
            ctx.pat.cov_int.not_covered(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width));

        if !not_covered.is_empty() {
            let mut msg = String::from("patterns not covered:\n");
            for value in not_covered {
                match value.display() {
                    RangeIncDisplay::Collapsed(value) => {
                        let _ = write!(&mut msg, "- `{}`\n", value);
                    }
                    RangeIncDisplay::Range(range) => {
                        let _ = write!(&mut msg, "- `{}..={}`\n", range.start, range.end);
                    }
                }
            }
            let src = ctx.src(match_kw);
            ctx.emit.error(Error::new(msg, src, None));
        }
    }
}

fn match_cov_bool(ctx: &mut HirCtx, check: &CheckContext) {
    pat_cov(ctx, check, (), pat_cov_bool);

    if let Some(match_kw) = check.match_kw {
        let not_covered = ctx.pat.cov_bool.not_covered();
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
}

fn match_cov_char(ctx: &mut HirCtx, check: &CheckContext) {
    pat_cov(ctx, check, (), pat_cov_char);

    if let Some(match_kw) = check.match_kw {
        if ctx.pat.cov_char.not_covered() {
            let msg = "patterns not covered:\n- `_`";
            let src = ctx.src(match_kw);
            ctx.emit.error(Error::new(msg, src, None));
        }
    }
}

fn match_cov_string(ctx: &mut HirCtx, check: &CheckContext) {
    pat_cov(ctx, check, (), pat_cov_string);

    if let Some(match_kw) = check.match_kw {
        if ctx.pat.cov_string.not_covered() {
            let msg = "patterns not covered:\n- `_`";
            let src = ctx.src(match_kw);
            ctx.emit.error(Error::new(msg, src, None));
        }
    }
}

fn match_cov_enum(ctx: &mut HirCtx, check: &CheckContext, enum_id: hir::EnumID) {
    pat_cov(ctx, check, enum_id, pat_cov_enum);

    if let Some(match_kw) = check.match_kw {
        let data = ctx.registry.enum_data(enum_id);
        let variant_count = data.variants.len();
        let not_covered = ctx.pat.cov_enum.not_covered(variant_count);

        if !not_covered.is_empty() {
            let mut msg = String::from("patterns not covered:\n");

            for variant_id in not_covered {
                let variant = data.variant(*variant_id);
                let name = ctx.session.intern_name.get(variant.name.id);
                msg.push_str(&format!("- `.{name}`\n"));
            }
            let src = ctx.src(match_kw);
            ctx.emit.error(Error::new(msg, src, None));
        }
    }
}

fn pat_cov<F, D>(ctx: &mut HirCtx, check: &CheckContext, data: D, cov_fn: F)
where
    D: Copy + Clone,
    F: Fn(&mut HirCtx, hir::Pat, TextRange, D),
{
    for (arm_idx, arm) in check.arms.iter().enumerate() {
        let pat_ast = &check.arms_ast[arm_idx].pat;
        match arm.pat {
            hir::Pat::Or(pats) => {
                for (pat_idx, pat) in pats.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { pats } => pats[pat_idx].range,
                        _ => unreachable!(),
                    };
                    cov_fn(ctx, *pat, range, data)
                }
            }
            _ => cov_fn(ctx, arm.pat, pat_ast.range, data),
        }
    }
}

fn pat_cov_int(ctx: &mut HirCtx, pat: hir::Pat, pat_range: TextRange, int_ty: hir::IntType) {
    let ptr_width = ctx.session.config.target_ptr_width;

    let result = match pat {
        hir::Pat::Wild => {
            ctx.pat.cov_int.cover_wild(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width))
        }
        hir::Pat::Lit(value) => {
            let value = value.into_int();
            let range = RangeInc::new(value, value);
            ctx.pat.cov_int.cover(range)
        }
        _ => unreachable!(),
    };
    check_pat_cov_result(ctx, pat_range, result);
}

fn pat_cov_bool(ctx: &mut HirCtx, pat: hir::Pat, pat_range: TextRange, _: ()) {
    let result = match pat {
        hir::Pat::Wild => ctx.pat.cov_bool.cover_wild(),
        hir::Pat::Lit(hir::ConstValue::Bool { val, .. }) => ctx.pat.cov_bool.cover(val),
        _ => unreachable!(),
    };
    check_pat_cov_result(ctx, pat_range, result);
}

fn pat_cov_char(ctx: &mut HirCtx, pat: hir::Pat, pat_range: TextRange, _: ()) {
    let result = match pat {
        hir::Pat::Wild => ctx.pat.cov_char.cover_wild(),
        hir::Pat::Lit(hir::ConstValue::Char { val, .. }) => {
            ctx.pat.cov_char.cover(RangeInc::new(val as u32, val as u32))
        }
        _ => unreachable!(),
    };
    check_pat_cov_result(ctx, pat_range, result);
}

fn pat_cov_string(ctx: &mut HirCtx, pat: hir::Pat, pat_range: TextRange, _: ()) {
    let result = match pat {
        hir::Pat::Wild => ctx.pat.cov_string.cover_wild(),
        hir::Pat::Lit(hir::ConstValue::String { val, .. }) => ctx.pat.cov_string.cover(val),
        _ => unreachable!(),
    };
    check_pat_cov_result(ctx, pat_range, result);
}

fn pat_cov_enum(ctx: &mut HirCtx, pat: hir::Pat, pat_range: TextRange, enum_id: hir::EnumID) {
    let data = ctx.registry.enum_data(enum_id);
    let variant_count = data.variants.len();

    let result = match pat {
        hir::Pat::Wild => ctx.pat.cov_enum.cover_wild(variant_count),
        hir::Pat::Variant(_, variant_id, _) => ctx.pat.cov_enum.cover(variant_id, variant_count),
        _ => unreachable!(),
    };
    check_pat_cov_result(ctx, pat_range, result);
}

fn check_pat_cov_result(ctx: &mut HirCtx, pat_range: TextRange, result: Result<(), PatCovError>) {
    if let Err(error) = result {
        let msg = match error {
            PatCovError::CoverFull => "pattern already covered",
            PatCovError::CoverPartial => "pattern partially covered",
        };
        let src = ctx.src(pat_range);
        ctx.emit.error(Error::new(msg, src, None));
    }
}

pub struct CheckContext<'a> {
    match_kw: Option<TextRange>,
    arms: &'a [hir::MatchArm<'a>],
    arms_ast: &'a [ast::MatchArm<'a>],
}

impl<'a> CheckContext<'a> {
    pub fn new(
        match_kw: Option<TextRange>,
        arms: &'a [hir::MatchArm<'a>],
        arms_ast: &'a [ast::MatchArm<'a>],
    ) -> CheckContext<'a> {
        CheckContext { match_kw, arms, arms_ast }
    }
}

pub struct PatCov {
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

impl PatCov {
    pub fn new() -> PatCov {
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
        PatCovBool { cov_true: false, cov_false: false, not_covered: Vec::with_capacity(2) }
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
        PatCovChar { wild_covered: false, covered: PatCovInt::new() }
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
        PatCovString { wild_covered: false, covered: Vec::with_capacity(32) }
    }

    fn reset(&mut self) {
        self.wild_covered = false;
        self.covered.clear();
    }

    fn cover(&mut self, new_id: LitID) -> Result<(), PatCovError> {
        if self.wild_covered || self.covered.iter().copied().any(|cov_id| cov_id == new_id) {
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
        if self.all_covered(variant_count) || self.covered.contains(&variant_id) {
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
            let variant_iter = each_idx.map(hir::VariantID::new);
            for variant_id in variant_iter {
                if !self.covered.contains(&variant_id) {
                    self.not_covered.push(variant_id);
                }
            }
            &self.not_covered
        }
    }
}

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
        RangeInc { start, end }
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
        PatCovInt { ranges: Vec::with_capacity(64), not_covered: Vec::with_capacity(64) }
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
                let next_range = self.ranges[next_idx];

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
