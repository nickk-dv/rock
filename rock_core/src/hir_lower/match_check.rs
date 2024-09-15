use super::context::HirCtx;
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, ErrorSink, SourceRange};
use crate::hir;
use crate::text::TextRange;

pub fn match_cov(
    ctx: &mut HirCtx,
    on_ty: hir::Type,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_range: TextRange,
) {
    let mut cov = PatCov::new(); //@cache? can it be re-used without collision?
    let kind = MatchKind::new(on_ty);

    match kind {
        MatchKind::Int(int_ty) => {
            cov.cov_int.reset();
            match_cov_int(ctx, &mut cov.cov_int, arms, arms_ast, match_range, int_ty)
        }
        MatchKind::Bool => {
            cov.cov_bool.reset();
            match_cov_bool(ctx, &mut cov.cov_bool, arms, arms_ast, match_range);
        }
        MatchKind::Char => {}
        MatchKind::String => {}
        MatchKind::Enum(_) => {}
    }
}

fn match_cov_int(
    ctx: &mut HirCtx,
    cov: &mut PatCovIntRange<i128>,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_range: TextRange,
    int_ty: hir::BasicInt,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(patterns) => {
                for (pat_idx, pat) in patterns.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { patterns } => patterns[pat_idx].range,
                        _ => unreachable!(),
                    };
                    pat_cov_int(ctx, cov, *pat, range, int_ty)
                }
            }
            _ => pat_cov_int(ctx, cov, arm.pat, pat_ast.range, int_ty),
        }
    }

    let ptr_width = ctx.target.arch().ptr_width();
    let not_covered = cov.not_covered(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width));

    if !not_covered.is_empty() {
        let mut msg = String::from("not covered patterns:\n");
        let src = SourceRange::new(ctx.proc.origin(), match_range);

        for value in not_covered {
            match value.display() {
                RangeIncDisplay::Collapsed(value) => msg.push_str(&format!("- `{}`\n", value)),
                RangeIncDisplay::Range(range) => {
                    msg.push_str(&format!("- `{}..={}`\n", range.start, range.end))
                }
            }
        }
        ctx.emit.error(ErrorComp::new(msg, src, None));
    }
}

fn pat_cov_int(
    ctx: &mut HirCtx,
    cov: &mut PatCovIntRange<i128>,
    pat: hir::Pat,
    pat_range: TextRange,
    int_ty: hir::BasicInt,
) {
    let ptr_width = ctx.target.arch().ptr_width();

    let result = match pat {
        hir::Pat::Wild => cov.cover_wild(int_ty.min_128(ptr_width), int_ty.max_128(ptr_width)),
        hir::Pat::Lit(value) => {
            let value = value.into_int();
            let range = RangeInc::new(value, value);
            cov.cover(range);
            Ok(())
        }
        hir::Pat::Const(const_id) => {
            let data = ctx.registry.const_data(const_id);
            let (eval, _) = ctx.registry.const_eval(data.value);
            let value_id = eval.get_resolved().unwrap();
            let value = ctx.const_intern.get(value_id);

            let value = value.into_int();
            let range = RangeInc::new(value, value);
            cov.cover(range);
            Ok(())
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        match error {
            PatCovError::AlreadyCovered => {
                let msg = "unreachable pattern";
                let src = SourceRange::new(ctx.proc.origin(), pat_range);
                ctx.emit.error(ErrorComp::new(msg, src, None));
            }
        }
    }
}

fn match_cov_bool(
    ctx: &mut HirCtx,
    cov: &mut PatCovBool,
    arms: &mut [hir::MatchArm],
    arms_ast: &[ast::MatchArm],
    match_range: TextRange,
) {
    for (arm_idx, arm) in arms.iter().enumerate() {
        let pat_ast = &arms_ast[arm_idx].pat;

        match arm.pat {
            hir::Pat::Or(patterns) => {
                for (pat_idx, pat) in patterns.iter().enumerate() {
                    let range = match pat_ast.kind {
                        ast::PatKind::Or { patterns } => patterns[pat_idx].range,
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
        let mut msg = String::from("not covered patterns:\n");
        let src = SourceRange::new(ctx.proc.origin(), match_range);

        for &value in not_covered {
            if value {
                msg.push_str("- `true`\n");
            } else {
                msg.push_str("- `false`\n");
            }
        }
        ctx.emit.error(ErrorComp::new(msg, src, None));
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
            let value_id = eval.get_resolved().unwrap();
            let value = ctx.const_intern.get(value_id);

            match value {
                hir::ConstValue::Bool { val } => cov.cover(val),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };

    if let Err(error) = result {
        match error {
            PatCovError::AlreadyCovered => {
                let msg = "unreachable pattern";
                let src = SourceRange::new(ctx.proc.origin(), pat_range);
                ctx.emit.error(ErrorComp::new(msg, src, None));
            }
        }
    }
}

enum MatchKind<'hir> {
    Int(hir::BasicInt),
    Bool,
    Char,
    String,
    Enum(hir::EnumID<'hir>),
}

enum PatCovError {
    AlreadyCovered,
}

//@refactor to not use this trait
// each match has slightly different fn signatures
trait PatCoverage<T> {
    fn reset(&mut self);
    fn cover(&mut self, new_value: T) -> Result<(), PatCovError>;
    fn cover_wild(&mut self) -> Result<(), PatCovError>;
    fn not_covered(&mut self) -> &[T];
}

struct PatCov {
    cov_int: PatCovIntRange<i128>,
    cov_bool: PatCovBool,
}

struct PatCovBool {
    cov_true: bool,
    cov_false: bool,
    not_covered: Vec<bool>,
}

impl<'hir> MatchKind<'hir> {
    fn new(ty: hir::Type<'hir>) -> MatchKind<'hir> {
        match ty {
            hir::Type::Error => unreachable!(),
            hir::Type::Basic(basic) => {
                if let Some(int_ty) = hir::BasicInt::from_basic(basic) {
                    MatchKind::Int(int_ty)
                } else {
                    match basic {
                        BasicType::Bool => MatchKind::Bool,
                        BasicType::Char => MatchKind::Char,
                        _ => unreachable!(),
                    }
                }
            }
            hir::Type::Enum(enum_id) => MatchKind::Enum(enum_id),
            hir::Type::Reference(ref_ty, _) => {
                if matches!(ref_ty, hir::Type::Basic(BasicType::U8)) {
                    MatchKind::String
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        }
    }
}

impl PatCov {
    fn new() -> PatCov {
        PatCov {
            cov_int: PatCovIntRange::new(),
            cov_bool: PatCovBool::new(),
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
}

impl PatCoverage<bool> for PatCovBool {
    fn reset(&mut self) {
        self.cov_true = false;
        self.cov_false = false;
        self.not_covered.clear();
    }

    fn cover(&mut self, new_value: bool) -> Result<(), PatCovError> {
        if new_value {
            if self.cov_true {
                return Err(PatCovError::AlreadyCovered);
            }
            self.cov_true = true;
        } else {
            if self.cov_false {
                return Err(PatCovError::AlreadyCovered);
            }
            self.cov_false = true;
        }
        Ok(())
    }

    fn cover_wild(&mut self) -> Result<(), PatCovError> {
        if self.cov_true && self.cov_false {
            Err(PatCovError::AlreadyCovered)
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

struct PatCovIntRange<T>
where
    T: Copy + Clone + PartialEq + Ord,
{
    ranges: Vec<RangeInc<T>>,
    not_covered: Vec<RangeInc<T>>,
}

impl<T> PatCovIntRange<T>
where
    T: Copy + Clone + PartialEq + Ord + std::fmt::Display + PatCovIncrement<T>,
{
    fn new() -> PatCovIntRange<T> {
        PatCovIntRange {
            ranges: Vec::with_capacity(64),
            not_covered: Vec::with_capacity(64),
        }
    }

    fn reset(&mut self) {
        self.ranges.clear();
        self.not_covered.clear();
    }

    //@return result with already covered / partially covered checks
    fn cover(&mut self, new_range: RangeInc<T>) {
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

            // overlap
            let new_start = new_range.start.min(range.start);
            let mut new_end = new_range.end.max(range.end);
            let mut next_idx = idx + 1;
            let mut remove_range = next_idx..next_idx;

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
            return;
        }

        self.ranges.insert(idx, new_range);
    }

    fn cover_wild(&mut self, min: T, max: T) -> Result<(), PatCovError> {
        if self.ranges.len() == 1 {
            let first = self.ranges[0];
            if first.start == min && first.end == max {
                return Err(PatCovError::AlreadyCovered);
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
