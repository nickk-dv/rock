use super::ast_layer::{self as cst, AstNode, AstNodeIterator};
use super::syntax_kind::{SyntaxKind, SyntaxSet};
use super::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use crate::ast;
use crate::support::AsStr;
use crate::text::TextRange;
use crate::token::Trivia;

const TAB_STR: &str = "    ";
const TAB_LEN: u32 = TAB_STR.len() as u32;
const WRAP_THRESHOLD: u32 = 90;
const SUBWRAP_IMPORT_SYMBOL: u32 = 60;

#[must_use]
pub fn format<'syn>(
    tree: &'syn SyntaxTree<'syn>,
    source: &'syn str,
    line_ranges: &'syn [TextRange],
    cache: &mut FormatterCache,
) -> String {
    let mut fmt = Formatter {
        tree,
        source,
        line_ranges,
        cache,
        line_num: 0,
        line_offset: 0,
        line_num_src: 0,
        tab_depth: 0,
    };

    fmt.cache.reset();
    source_file(&mut fmt, tree.source_file());

    let mut buffer_len: usize = 0;
    for event in fmt.cache.events.iter().copied() {
        buffer_len += match event {
            FormatEvent::Space => 1,
            FormatEvent::Newline => 1,
            FormatEvent::Char(c) => c.len_utf8(),
            FormatEvent::Tab { count } => (TAB_LEN * count) as usize,
            FormatEvent::Range { range_idx } => fmt.cache.range(range_idx).len() as usize,
            FormatEvent::String { string_idx } => fmt.cache.string(string_idx).len(),
            FormatEvent::Comment { spacing, range_idx } => {
                spacing as usize + fmt.cache.range(range_idx).len() as usize
            }
        };
    }

    let mut buffer = String::with_capacity(buffer_len);
    for event in fmt.cache.events.iter().copied() {
        match event {
            FormatEvent::Space => buffer.push(' '),
            FormatEvent::Newline => buffer.push('\n'),
            FormatEvent::Char(c) => buffer.push(c),
            FormatEvent::Tab { count } => {
                for _ in 0..count {
                    buffer.push_str(TAB_STR);
                }
            }
            FormatEvent::Range { range_idx } => {
                let range = fmt.cache.range(range_idx);
                let string = &fmt.source[range.as_usize()];
                buffer.push_str(string);
            }
            FormatEvent::String { string_idx } => {
                buffer.push_str(fmt.cache.string(string_idx));
            }
            FormatEvent::Comment { spacing, range_idx } => {
                for _ in 0..spacing {
                    buffer.push(' ');
                }
                let range = fmt.cache.range(range_idx);
                let comment = &fmt.source[range.as_usize()];
                buffer.push_str(comment.trim_end());
            }
        }
    }

    // buffer len prediction was correct
    // `>=` instead of `==` since comments can be trimmed
    assert!(buffer_len >= buffer.len());
    buffer
}

struct Formatter<'syn, 'cache> {
    tree: &'syn SyntaxTree<'syn>,
    source: &'syn str,
    line_ranges: &'syn [TextRange],
    cache: &'cache mut FormatterCache,
    line_num: u32,
    line_offset: u32,
    line_num_src: u32,
    tab_depth: u32,
}

pub struct FormatterCache {
    events: Vec<FormatEvent>,
    ranges: Vec<TextRange>,
    strings: Vec<&'static str>,
    comments: Vec<CommentPosition>,
}

#[derive(Copy, Clone)]
pub struct CommentPosition {
    line_num: u32,
    line_offset: u32,
    event_idx: u32,
}

#[derive(Copy, Clone)]
enum FormatEvent {
    Space,
    Newline,
    Char(char),
    Tab { count: u32 },
    Range { range_idx: u32 },
    String { string_idx: u32 },
    Comment { spacing: u16, range_idx: u32 },
}

impl<'syn, 'cache> Formatter<'syn, 'cache> {
    fn space(&mut self) {
        self.line_offset += 1;
        self.cache.events.push(FormatEvent::Space);
    }
    fn new_line(&mut self) {
        self.line_num += 1;
        self.line_offset = 0;
        self.cache.events.push(FormatEvent::Newline);
    }

    fn write(&mut self, c: char) {
        self.line_offset += 1;
        self.cache.events.push(FormatEvent::Char(c));
    }
    fn write_range(&mut self, range: TextRange) {
        self.line_offset += range.len();
        let range_idx = self.cache.ranges.len() as u32;
        self.cache.ranges.push(range);
        self.cache.events.push(FormatEvent::Range { range_idx });
    }
    fn write_str(&mut self, string: &'static str) {
        self.line_offset += string.len() as u32;
        let string_idx = self.cache.strings.len() as u32;
        self.cache.strings.push(string);
        self.cache.events.push(FormatEvent::String { string_idx });
    }
    #[must_use]
    fn write_comment(&mut self, range: TextRange) -> u32 {
        self.line_offset += range.len();
        let range_idx = self.cache.ranges.len() as u32;
        self.cache.ranges.push(range);
        let event_idx = self.cache.events.len() as u32;
        self.cache.events.push(FormatEvent::Comment {
            spacing: 0,
            range_idx,
        });
        event_idx
    }

    fn tab_inc(&mut self) {
        self.tab_depth += 1;
    }
    fn tab_dec(&mut self) {
        assert_ne!(self.tab_depth, 0);
        self.tab_depth -= 1;
    }
    fn tab_single(&mut self) {
        self.line_offset += TAB_LEN;
        self.cache.events.push(FormatEvent::Tab { count: 1 });
    }
    fn tab_depth(&mut self) {
        if self.tab_depth == 0 {
            return;
        }
        self.line_offset += TAB_LEN * self.tab_depth;
        self.cache.events.push(FormatEvent::Tab {
            count: self.tab_depth,
        });
    }

    fn wrap_line_break_based<N: AstNode<'syn>>(
        &mut self,
        mut node_iter: AstNodeIterator<'syn, N>,
    ) -> bool {
        let first = match node_iter.next() {
            Some(node) => node,
            None => return false,
        };
        let first_start = first.find_range(self.tree).start();

        // seek `line_num_src` to the first node's range.start
        let mut line_range = self.line_ranges[self.line_num_src as usize];
        while !line_range.contains_exclusive(first_start) {
            self.line_num_src += 1;
            line_range = self.line_ranges[self.line_num_src as usize];
        }

        // any subsequent node on different line indicates a wrap
        for node in node_iter {
            let range = node.find_range(self.tree);
            if !line_range.contains_exclusive(range.start()) {
                return true;
            }
        }
        false
    }

    fn wrap_content_len_based(&self, node: &Node) -> bool {
        let offset = self.line_offset + content_len(self, node);
        offset > WRAP_THRESHOLD
    }
}

impl FormatterCache {
    pub fn new() -> FormatterCache {
        FormatterCache {
            events: Vec::with_capacity(1024),
            ranges: Vec::with_capacity(512),
            strings: Vec::with_capacity(512),
            comments: Vec::with_capacity(128),
        }
    }
    #[inline]
    fn reset(&mut self) {
        self.events.clear();
        self.ranges.clear();
        self.strings.clear();
        self.comments.clear();
    }
    #[inline]
    fn range(&self, range_idx: u32) -> TextRange {
        self.ranges[range_idx as usize]
    }
    #[inline]
    fn string(&self, string_idx: u32) -> &'static str {
        self.strings[string_idx as usize]
    }
}

trait InterleaveFormat<'syn> {
    const COMMENT_ALIGN: u32;
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self);
}

impl<'syn> InterleaveFormat<'syn> for cst::Item<'syn> {
    const COMMENT_ALIGN: u32 = 36;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        item(fmt, node);
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::Variant<'syn> {
    const COMMENT_ALIGN: u32 = 24;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        variant(fmt, node);
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::Field<'syn> {
    const COMMENT_ALIGN: u32 = 24;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        field(fmt, node);
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::Stmt<'syn> {
    const COMMENT_ALIGN: u32 = 0;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        stmt(fmt, node, true);
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::Expr<'syn> {
    const COMMENT_ALIGN: u32 = 0;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        fmt.tab_depth();
        expr(fmt, node);
        fmt.write(',');
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::MatchArm<'syn> {
    const COMMENT_ALIGN: u32 = 0;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        match_arm(fmt, node);
    }
}
impl<'syn> InterleaveFormat<'syn> for cst::FieldInit<'syn> {
    const COMMENT_ALIGN: u32 = 0;
    #[inline(always)]
    fn interleaved_format(fmt: &mut Formatter<'syn, '_>, node: Self) {
        fmt.tab_depth();
        field_init(fmt, node);
        fmt.write(',');
    }
}

#[must_use]
fn content_empty(fmt: &mut Formatter, node: &Node) -> bool {
    for not in node.content {
        match *not {
            NodeOrToken::Node(_) => return false,
            NodeOrToken::Token(_) => {}
            NodeOrToken::Trivia(trivia_id) => {
                let trivia = fmt.tree.tokens().trivia(trivia_id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => return false,
                }
            }
        }
    }
    true
}

#[must_use]
fn content_len(fmt: &Formatter, node: &Node) -> u32 {
    let mut len = 0;
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                len += content_len(fmt, node);
            }
            NodeOrToken::Token(token_id) => {
                let range = fmt.tree.tokens().token_range(token_id);
                len += range.len();
            }
            NodeOrToken::Trivia(_) => {}
        }
    }
    len
}

fn trivia_lift(fmt: &mut Formatter, node: &Node, halt: SyntaxSet) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                if halt.contains(node.kind) {
                    continue;
                }
                trivia_lift(fmt, node, halt);
            }
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => {
                        fmt.tab_depth();
                        let _ = fmt.write_comment(range);
                        fmt.new_line();
                    }
                }
            }
            _ => {}
        }
    }
}

fn interleaved_node_list<'syn, N: AstNode<'syn> + InterleaveFormat<'syn>>(
    fmt: &mut Formatter<'syn, '_>,
    node_list: &Node<'syn>,
) {
    let mut first = true; // prevent first \n insertion
    let mut new_line = false; // prevent last \n insertion
    let comments_offset = fmt.cache.comments.len();

    let mut not_iter = node_list.content.iter().copied().peekable();
    while let Some(not) = not_iter.next() {
        match not {
            NodeOrToken::Token(_) => {}
            NodeOrToken::Node(node_id) => {
                if new_line {
                    new_line = false;
                    fmt.new_line();
                }
                first = false;

                let node = fmt.tree.node(node_id);
                let node = N::cast(node).unwrap();
                N::interleaved_format(fmt, node);

                // search for line comment on the same line
                while let Some(not_next) = not_iter.peek().copied() {
                    match not_next {
                        NodeOrToken::Token(_) => {
                            not_iter.next();
                        }
                        NodeOrToken::Node(_) => break,
                        NodeOrToken::Trivia(trivia_id) => {
                            let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                            match trivia {
                                Trivia::Whitespace => {
                                    let whitespace = &fmt.source[range.as_usize()];
                                    let mut new_lines: u32 = 0;

                                    for c in whitespace.chars() {
                                        if c == '\n' {
                                            new_lines += 1;
                                            break;
                                        }
                                    }
                                    if new_lines == 0 {
                                        not_iter.next();
                                    } else {
                                        break;
                                    }
                                }
                                Trivia::LineComment => {
                                    let line_num = fmt.line_num;
                                    let line_offset = fmt.line_offset;
                                    let event_idx = fmt.write_comment(range);

                                    fmt.cache.comments.push(CommentPosition {
                                        line_num,
                                        line_offset,
                                        event_idx,
                                    });
                                    not_iter.next();
                                    break;
                                }
                                Trivia::DocComment | Trivia::ModComment => break,
                            }
                        }
                    }
                }

                fmt.new_line();
            }
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {
                        if first {
                            continue;
                        }
                        let whitespace = &fmt.source[range.as_usize()];
                        let mut new_lines: u32 = 0;

                        for c in whitespace.chars() {
                            if c == '\n' {
                                new_lines += 1;
                                if new_lines == 2 {
                                    break;
                                }
                            }
                        }
                        if new_lines == 2 {
                            new_line = true;
                        }
                    }
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => {
                        if new_line {
                            new_line = false;
                            fmt.new_line();
                        }
                        first = false;

                        fmt.tab_depth();
                        let _ = fmt.write_comment(range);
                        fmt.new_line();
                    }
                }
            }
        }
    }

    if comments_offset == fmt.cache.comments.len() {
        return;
    }
    let comment_range = comments_offset..fmt.cache.comments.len();
    let trail_comments = &fmt.cache.comments[comment_range.clone()];

    let mut group_start = 0;
    while group_start < trail_comments.len() {
        let first = trail_comments[group_start];
        let mut line_num = first.line_num;
        let mut min_offset = first.line_offset;
        let mut max_offset = first.line_offset;

        let mut group_end = group_start + 1;
        while group_end < trail_comments.len() {
            let comment = &trail_comments[group_end];

            if comment.line_num == line_num + 1 {
                line_num = comment.line_num;
            } else {
                break;
            }

            let new_min = min_offset.min(comment.line_offset);
            let new_max = max_offset.max(comment.line_offset);
            let spacing = new_max - new_min;
            if spacing <= N::COMMENT_ALIGN {
                min_offset = new_min;
                max_offset = new_max;
            } else {
                break;
            }

            group_end += 1;
        }

        let group = &trail_comments[group_start..group_end];
        for comment in group {
            let extra_spacing = 1 + max_offset - comment.line_offset;
            let event_mut = &mut fmt.cache.events[comment.event_idx as usize];

            match event_mut {
                FormatEvent::Comment { spacing, .. } => *spacing = extra_spacing as u16,
                _ => unreachable!(),
            }
        }
        group_start = group_end;
    }

    fmt.cache.comments.truncate(comments_offset);
}

//==================== SOURCE FILE ====================

fn source_file<'syn>(fmt: &mut Formatter<'syn, '_>, source_file: cst::SourceFile<'syn>) {
    interleaved_node_list::<cst::Item>(fmt, source_file.0);
}

fn item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::Item<'syn>) {
    match item {
        cst::Item::Proc(item) => proc_item(fmt, item),
        cst::Item::Enum(item) => enum_item(fmt, item),
        cst::Item::Struct(item) => struct_item(fmt, item),
        cst::Item::Const(item) => const_item(fmt, item),
        cst::Item::Global(item) => global_item(fmt, item),
        cst::Item::Import(item) => import_item(fmt, item),
        cst::Item::Directive(item) => directive_list(fmt, item, false),
    }
}

fn proc_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::ProcItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::BLOCK]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("proc");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    param_list(fmt, item.param_list(fmt.tree).unwrap());

    if let Some(poly_params) = item.poly_params(fmt.tree) {
        polymorph_params(fmt, poly_params);
    }
    fmt.space();
    ty(fmt, item.return_ty(fmt.tree).unwrap());

    if let Some(block_cst) = item.block(fmt.tree) {
        fmt.space();
        block(fmt, block_cst, false);
    } else {
        fmt.write(';');
    }
}

fn param_list<'syn>(fmt: &mut Formatter<'syn, '_>, param_list: cst::ParamList<'syn>) {
    if param_list.params(fmt.tree).next().is_none() {
        fmt.write('(');
        if param_list.t_dotdot(fmt.tree).is_some() {
            fmt.write_str("..");
        }
        fmt.write(')');
        return;
    }

    fmt.write('(');
    let wrap = fmt.wrap_line_break_based(param_list.params(fmt.tree));
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    for param_cst in param_list.params(fmt.tree) {
        if !first {
            fmt.write(',');
            if wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap {
            fmt.tab_single();
        }
        first = false;
        param(fmt, param_cst);
    }

    let is_variadic = param_list.t_dotdot(fmt.tree).is_some();
    if is_variadic {
        fmt.write(',');
        fmt.space();
        fmt.write_str("..");
    }

    if wrap {
        if !is_variadic {
            fmt.write(',');
        }
        fmt.new_line();
    }
    fmt.write(')');
}

fn param<'syn>(fmt: &mut Formatter<'syn, '_>, param: cst::Param<'syn>) {
    if param.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
        fmt.space();
    }
    name(fmt, param.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, param.ty(fmt.tree).unwrap());
}

fn enum_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::EnumItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::VARIANT_LIST]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("enum");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());

    if let Some(poly_params) = item.poly_params(fmt.tree) {
        polymorph_params(fmt, poly_params);
    }
    if let Some((basic, _)) = item.tag_ty(fmt.tree) {
        fmt.space();
        fmt.write_str(basic.as_str());
    }

    fmt.space();
    variant_list(fmt, item.variant_list(fmt.tree).unwrap());
}

fn variant_list<'syn>(fmt: &mut Formatter<'syn, '_>, variant_list: cst::VariantList<'syn>) {
    if content_empty(fmt, variant_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }
    fmt.write('{');
    fmt.new_line();
    fmt.tab_inc();
    interleaved_node_list::<cst::Variant>(fmt, variant_list.0);
    fmt.tab_dec();
    fmt.write('}');
}

fn variant<'syn>(fmt: &mut Formatter<'syn, '_>, variant: cst::Variant<'syn>) {
    trivia_lift(fmt, variant.0, SyntaxSet::empty());
    if let Some(dir_list) = variant.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }
    fmt.tab_depth();

    name(fmt, variant.name(fmt.tree).unwrap());
    if let Some(value) = variant.value(fmt.tree) {
        fmt.write_str(" = ");
        expr(fmt, value);
    } else if let Some(field_list) = variant.field_list(fmt.tree) {
        variant_field_list(fmt, field_list);
    }
    fmt.write(',');
}

fn variant_field_list<'syn>(
    fmt: &mut Formatter<'syn, '_>,
    field_list: cst::VariantFieldList<'syn>,
) {
    fmt.write('(');
    let mut first = true;
    for field_ty in field_list.fields(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        ty(fmt, field_ty);
    }
    fmt.write(')');
}

fn struct_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::StructItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::FIELD_LIST]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("struct");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());

    if let Some(poly_params) = item.poly_params(fmt.tree) {
        polymorph_params(fmt, poly_params);
    }
    fmt.space();
    field_list(fmt, item.field_list(fmt.tree).unwrap());
}

fn field_list<'syn>(fmt: &mut Formatter<'syn, '_>, field_list: cst::FieldList<'syn>) {
    if content_empty(fmt, field_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }
    fmt.write('{');
    fmt.new_line();
    fmt.tab_inc();
    interleaved_node_list::<cst::Field>(fmt, field_list.0);
    fmt.tab_dec();
    fmt.write('}');
}

fn field<'syn>(fmt: &mut Formatter<'syn, '_>, field: cst::Field<'syn>) {
    trivia_lift(fmt, field.0, SyntaxSet::empty());
    if let Some(dir_list) = field.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }
    fmt.tab_depth();

    name(fmt, field.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, field.ty(fmt.tree).unwrap());
    fmt.write(',');
}

fn const_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::ConstItem<'syn>) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("const");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write('=');
    fmt.space();
    expr(fmt, item.value(fmt.tree).unwrap());
    fmt.write(';');
}

fn global_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::GlobalItem<'syn>) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("global");
    fmt.space();
    if item.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
        fmt.space();
    }
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write('=');
    fmt.space();

    if item.t_zeroed(fmt.tree).is_some() {
        fmt.write_str("zeroed");
    } else {
        expr(fmt, item.value(fmt.tree).unwrap());
    }
    fmt.write(';');
}

fn import_item<'syn>(fmt: &mut Formatter<'syn, '_>, item: cst::ImportItem<'syn>) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(dir_list) = item.dir_list(fmt.tree) {
        directive_list(fmt, dir_list, true);
    }

    fmt.write_str("import");
    fmt.space();
    if let Some(name_cst) = item.package(fmt.tree) {
        name(fmt, name_cst);
        fmt.write(':');
    }

    import_path(fmt, item.import_path(fmt.tree).unwrap());
    if let Some(rename) = item.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }

    if let Some(symbol_list) = item.import_symbol_list(fmt.tree) {
        fmt.write('.');
        import_symbol_list(fmt, symbol_list);
    } else {
        fmt.write(';');
    }
}

fn import_path(fmt: &mut Formatter, import_path: cst::ImportPath) {
    let mut first = true;
    for name_cst in import_path.names(fmt.tree) {
        if !first {
            fmt.write('/');
        }
        first = false;
        name(fmt, name_cst);
    }
}

fn import_symbol_list(fmt: &mut Formatter, import_symbol_list: cst::ImportSymbolList) {
    if import_symbol_list.import_symbols(fmt.tree).next().is_none() {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    let wrap = fmt.wrap_content_len_based(import_symbol_list.0);
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    let mut total_len = 0;

    for import_symbol_cst in import_symbol_list.import_symbols(fmt.tree) {
        let sub_wrap = total_len > SUBWRAP_IMPORT_SYMBOL;
        if sub_wrap {
            total_len = 0;
        }
        if wrap {
            total_len += content_len(fmt, import_symbol_cst.0);
        }

        if !first {
            fmt.write(',');
            if wrap && sub_wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap && (first || sub_wrap) {
            fmt.tab_single();
        }
        first = false;
        import_symbol(fmt, import_symbol_cst);
    }

    if wrap {
        fmt.write(',');
        fmt.new_line();
    }
    fmt.write('}');
}

fn import_symbol(fmt: &mut Formatter, import_symbol: cst::ImportSymbol) {
    name(fmt, import_symbol.name(fmt.tree).unwrap());
    if let Some(rename) = import_symbol.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }
}

fn import_symbol_rename(fmt: &mut Formatter, rename: cst::ImportSymbolRename) {
    fmt.space();
    fmt.write_str("as");
    fmt.space();
    if let Some(alias) = rename.alias(fmt.tree) {
        name(fmt, alias);
    } else {
        fmt.write('_');
    }
}

//==================== DIRECTIVE ====================

fn directive_list<'syn>(
    fmt: &mut Formatter<'syn, '_>,
    dir_list: cst::DirectiveList<'syn>,
    new_line: bool,
) {
    let mut first = true;
    for directive_cst in dir_list.directives(fmt.tree) {
        if !first {
            fmt.new_line();
        }
        first = false;
        fmt.tab_depth();
        directive(fmt, directive_cst);
    }
    if new_line {
        fmt.new_line();
    }
}

fn directive<'syn>(fmt: &mut Formatter<'syn, '_>, directive: cst::Directive<'syn>) {
    fmt.write('#');
    match directive {
        cst::Directive::Simple(dir) => {
            name(fmt, dir.name(fmt.tree).unwrap());
        }
        cst::Directive::WithType(dir) => {
            name(fmt, dir.name(fmt.tree).unwrap());
            fmt.write('(');
            ty(fmt, dir.ty(fmt.tree).unwrap());
            fmt.write(')');
        }
        cst::Directive::WithParams(dir) => {
            name(fmt, dir.name(fmt.tree).unwrap());
            directive_param_list(fmt, dir.param_list(fmt.tree).unwrap());
        }
    }
}

fn directive_param_list<'syn>(
    fmt: &mut Formatter<'syn, '_>,
    param_list: cst::DirectiveParamList<'syn>,
) {
    if param_list.params(fmt.tree).next().is_none() {
        fmt.write('(');
        fmt.write(')');
        return;
    }

    fmt.write('(');
    let wrap = fmt.wrap_line_break_based(param_list.params(fmt.tree));
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    for param in param_list.params(fmt.tree) {
        if !first {
            fmt.write(',');
            if wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap {
            fmt.tab_single();
        }
        first = false;
        directive_param(fmt, param);
    }

    if wrap {
        fmt.write(',');
        fmt.new_line();
    }
    fmt.write(')');
}

fn directive_param(fmt: &mut Formatter, param: cst::DirectiveParam) {
    name(fmt, param.name(fmt.tree).unwrap());
    fmt.space();
    fmt.write('=');
    fmt.space();
    let lit_string = param.value(fmt.tree).unwrap();
    fmt.write_range(lit_string.find_range(fmt.tree));
}

//==================== TYPE ====================

fn ty<'syn>(fmt: &mut Formatter<'syn, '_>, ty_cst: cst::Type<'syn>) {
    match ty_cst {
        cst::Type::Basic(ty_cst) => {
            let (basic, _) = ty_cst.basic(fmt.tree).unwrap();
            fmt.write_str(basic.as_str());
        }
        cst::Type::Custom(ty_cst) => path_type(fmt, ty_cst.path(fmt.tree).unwrap()),
        cst::Type::Reference(ty_cst) => ty_ref(fmt, ty_cst),
        cst::Type::MultiReference(ty_cst) => ty_multi_ref(fmt, ty_cst),
        cst::Type::Procedure(ty_cst) => ty_proc(fmt, ty_cst),
        cst::Type::ArraySlice(slice) => ty_slice(fmt, slice),
        cst::Type::ArrayStatic(array) => ty_array(fmt, array),
    }
}

fn ty_ref<'syn>(fmt: &mut Formatter<'syn, '_>, ty_cst: cst::TypeReference<'syn>) {
    fmt.write('&');
    let mut with_space = false;

    if ty_cst.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
        fmt.space();
        with_space = true;
    }

    let ref_ty = ty_cst.ref_ty(fmt.tree).unwrap();
    if !with_space && matches!(ref_ty, cst::Type::Reference(_)) {
        fmt.space();
    }
    ty(fmt, ref_ty);
}

fn ty_multi_ref<'syn>(fmt: &mut Formatter<'syn, '_>, ty_cst: cst::TypeMultiReference<'syn>) {
    fmt.write('[');
    fmt.write('&');
    if ty_cst.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
    }
    fmt.write(']');
    ty(fmt, ty_cst.ref_ty(fmt.tree).unwrap());
}

fn ty_proc<'syn>(fmt: &mut Formatter<'syn, '_>, proc_ty: cst::TypeProcedure<'syn>) {
    fmt.write_str("proc");
    fmt.write('(');

    let mut first = true;
    let param_list = proc_ty.param_list(fmt.tree).unwrap();

    for param_cst in param_list.params(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        param(fmt, param_cst);
    }

    if param_list.t_dotdot(fmt.tree).is_some() {
        if param_list.params(fmt.tree).next().is_some() {
            fmt.write(',');
            fmt.space();
        }
        fmt.write_str("..");
    }

    fmt.write(')');
    fmt.space();
    ty(fmt, proc_ty.return_ty(fmt.tree).unwrap());
}

fn ty_slice<'syn>(fmt: &mut Formatter<'syn, '_>, slice: cst::TypeArraySlice<'syn>) {
    fmt.write('[');
    if slice.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
    }
    fmt.write(']');
    ty(fmt, slice.elem_ty(fmt.tree).unwrap());
}

fn ty_array<'syn>(fmt: &mut Formatter<'syn, '_>, array: cst::TypeArrayStatic<'syn>) {
    fmt.write('[');
    expr(fmt, array.len(fmt.tree).unwrap());
    fmt.write(']');
    ty(fmt, array.elem_ty(fmt.tree).unwrap());
}

//==================== STMT ====================

fn block<'syn>(fmt: &mut Formatter<'syn, '_>, block: cst::Block<'syn>, carry: bool) {
    if content_empty(fmt, block.0) {
        fmt.write('{');
        if carry {
            fmt.new_line();
            fmt.tab_depth();
        }
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.new_line();
    fmt.tab_inc();
    interleaved_node_list::<cst::Stmt>(fmt, block.0);
    fmt.tab_dec();
    fmt.tab_depth();
    fmt.write('}');
}

fn stmt<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::Stmt<'syn>, tab: bool) {
    if tab {
        match stmt {
            cst::Stmt::WithDirective(_) => {}
            _ => fmt.tab_depth(),
        }
    }
    match stmt {
        cst::Stmt::Break(_) => stmt_break(fmt),
        cst::Stmt::Continue(_) => stmt_continue(fmt),
        cst::Stmt::Return(stmt) => stmt_return(fmt, stmt),
        cst::Stmt::Defer(stmt) => stmt_defer(fmt, stmt),
        cst::Stmt::For(stmt) => stmt_for(fmt, stmt),
        cst::Stmt::Local(stmt) => stmt_local(fmt, stmt),
        cst::Stmt::Assign(stmt) => stmt_assign(fmt, stmt, true),
        cst::Stmt::ExprSemi(stmt) => stmt_expr_semi(fmt, stmt),
        cst::Stmt::ExprTail(stmt) => stmt_expr_tail(fmt, stmt),
        cst::Stmt::WithDirective(stmt) => stmt_with_directive(fmt, stmt),
    }
}

fn stmt_break(fmt: &mut Formatter) {
    fmt.write_str("break");
    fmt.write(';');
}

fn stmt_continue(fmt: &mut Formatter) {
    fmt.write_str("continue");
    fmt.write(';');
}

fn stmt_return<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtReturn<'syn>) {
    fmt.write_str("return");
    if let Some(expr_cst) = stmt.expr(fmt.tree) {
        fmt.space();
        expr(fmt, expr_cst);
    }
    fmt.write(';');
}

fn stmt_defer<'syn>(fmt: &mut Formatter<'syn, '_>, defer: cst::StmtDefer<'syn>) {
    fmt.write_str("defer");
    fmt.space();

    if let Some(block_cst) = defer.block(fmt.tree) {
        block(fmt, block_cst, false);
    } else {
        stmt(fmt, defer.stmt(fmt.tree).unwrap(), false);
    }
}

fn stmt_for<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtFor<'syn>) {
    fmt.write_str("for");

    if let Some(header) = stmt.header_cond(fmt.tree) {
        fmt.space();
        expr(fmt, header.expr(fmt.tree).unwrap());
    } else if let Some(header) = stmt.header_elem(fmt.tree) {
        fmt.space();
        if header.t_ampersand(fmt.tree).is_some() {
            fmt.write('&');
            if header.t_mut(fmt.tree).is_some() {
                fmt.write_str("mut");
                fmt.space();
            }
        }
        name(fmt, header.value(fmt.tree).unwrap());
        if let Some(name_cst) = header.index(fmt.tree) {
            fmt.write(',');
            fmt.space();
            name(fmt, name_cst)
        }
        fmt.space();
        fmt.write_str("in");
        fmt.space();
        if header.t_rev(fmt.tree).is_some() {
            fmt.write_str("<<");
            fmt.space();
        }
        expr(fmt, header.expr(fmt.tree).unwrap());
    } else if let Some(header) = stmt.header_pat(fmt.tree) {
        fmt.space();
        fmt.write_str("let");
        fmt.space();
        pat(fmt, header.pat(fmt.tree).unwrap());
        fmt.space();
        fmt.write('=');
        fmt.space();
        expr(fmt, header.expr(fmt.tree).unwrap());
    }

    fmt.space();
    block(fmt, stmt.block(fmt.tree).unwrap(), false);
}

fn stmt_local<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtLocal<'syn>) {
    fmt.write_str("let");
    fmt.space();
    bind(fmt, stmt.bind(fmt.tree).unwrap());

    if let Some(ty_cst) = stmt.ty(fmt.tree) {
        fmt.write(':');
        fmt.space();
        ty(fmt, ty_cst);
    }

    fmt.space();
    fmt.write('=');
    fmt.space();

    if stmt.t_zeroed(fmt.tree).is_some() {
        fmt.write_str("zeroed");
    } else if stmt.t_undefined(fmt.tree).is_some() {
        fmt.write_str("undefined");
    } else {
        expr(fmt, stmt.init(fmt.tree).unwrap());
    }
    fmt.write(';');
}

fn stmt_assign<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtAssign<'syn>, semi: bool) {
    expr(fmt, stmt.lhs(fmt.tree).unwrap());
    fmt.space();

    let (assign_op, _) = stmt.assign_op(fmt.tree).unwrap();
    match assign_op {
        ast::AssignOp::Assign => fmt.write('='),
        ast::AssignOp::Bin(bin_op) => {
            fmt.write_str(bin_op.as_str());
            fmt.write('=');
        }
    }

    fmt.space();
    expr(fmt, stmt.rhs(fmt.tree).unwrap());
    if semi {
        fmt.write(';');
    }
}

fn stmt_expr_semi<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtExprSemi<'syn>) {
    expr(fmt, stmt.expr(fmt.tree).unwrap());
    if stmt.t_semi(fmt.tree).is_some() {
        fmt.write(';');
    }
}

fn stmt_expr_tail<'syn>(fmt: &mut Formatter<'syn, '_>, stmt: cst::StmtExprTail<'syn>) {
    expr(fmt, stmt.expr(fmt.tree).unwrap());
}

fn stmt_with_directive<'syn>(
    fmt: &mut Formatter<'syn, '_>,
    stmt_dir: cst::StmtWithDirective<'syn>,
) {
    directive_list(fmt, stmt_dir.dir_list(fmt.tree).unwrap(), true);
    stmt(fmt, stmt_dir.stmt(fmt.tree).unwrap(), true);
}

//==================== EXPR ====================

fn expr<'syn>(fmt: &mut Formatter<'syn, '_>, expr: cst::Expr<'syn>) {
    match expr {
        cst::Expr::Paren(expr) => expr_paren(fmt, expr),
        cst::Expr::Lit(lit_cst) => lit(fmt, lit_cst),
        cst::Expr::If(expr) => expr_if(fmt, expr),
        cst::Expr::Block(block_cst) => block(fmt, block_cst, false),
        cst::Expr::Match(expr) => expr_match(fmt, expr),
        cst::Expr::Field(expr) => expr_field(fmt, expr),
        cst::Expr::Index(expr) => expr_index(fmt, expr),
        cst::Expr::Slice(expr) => expr_slice(fmt, expr),
        cst::Expr::Call(expr) => expr_call(fmt, expr),
        cst::Expr::Cast(expr) => expr_cast(fmt, expr),
        cst::Expr::Sizeof(expr) => expr_sizeof(fmt, expr),
        cst::Expr::Directive(dir) => directive(fmt, dir),
        cst::Expr::Item(expr) => expr_item(fmt, expr),
        cst::Expr::Variant(expr) => expr_variant(fmt, expr),
        cst::Expr::StructInit(expr) => expr_struct_init(fmt, expr),
        cst::Expr::ArrayInit(expr) => expr_array_init(fmt, expr),
        cst::Expr::ArrayRepeat(expr) => expr_array_repeat(fmt, expr),
        cst::Expr::Deref(expr) => expr_deref(fmt, expr),
        cst::Expr::Address(expr) => expr_address(fmt, expr),
        cst::Expr::Range(range_cst) => range(fmt, range_cst),
        cst::Expr::Unary(expr) => expr_unary(fmt, expr),
        cst::Expr::Binary(expr) => expr_binary(fmt, expr),
    }
}

fn expr_paren<'syn>(fmt: &mut Formatter<'syn, '_>, paren: cst::ExprParen<'syn>) {
    fmt.write('(');
    expr(fmt, paren.expr(fmt.tree).unwrap());
    fmt.write(')');
}

fn expr_if<'syn>(fmt: &mut Formatter<'syn, '_>, if_: cst::ExprIf<'syn>) {
    let mut branches = if_.branches(fmt.tree);

    let entry = branches.next().unwrap();
    fmt.write_str("if");
    fmt.space();
    expr(fmt, entry.cond(fmt.tree).unwrap());
    fmt.space();
    block(fmt, entry.block(fmt.tree).unwrap(), true);

    for branch in branches {
        fmt.space();
        fmt.write_str("else");
        fmt.space();
        fmt.write_str("if");
        fmt.space();
        expr(fmt, branch.cond(fmt.tree).unwrap());
        fmt.space();
        block(fmt, branch.block(fmt.tree).unwrap(), true);
    }

    if let Some(else_block) = if_.else_block(fmt.tree) {
        fmt.space();
        fmt.write_str("else");
        fmt.space();
        block(fmt, else_block, true);
    }
}

fn expr_match<'syn>(fmt: &mut Formatter<'syn, '_>, match_: cst::ExprMatch<'syn>) {
    fmt.write_str("match");
    fmt.space();
    expr(fmt, match_.on_expr(fmt.tree).unwrap());
    fmt.space();

    let match_arm_list = match_.match_arm_list(fmt.tree).unwrap();
    if content_empty(fmt, match_arm_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.new_line();
    fmt.tab_inc();
    interleaved_node_list::<cst::MatchArm>(fmt, match_arm_list.0);
    fmt.tab_dec();
    fmt.tab_depth();
    fmt.write('}');
}

fn match_arm<'syn>(fmt: &mut Formatter<'syn, '_>, match_arm: cst::MatchArm<'syn>) {
    fmt.tab_depth();
    pat(fmt, match_arm.pat(fmt.tree).unwrap());
    fmt.space();
    fmt.write_str("->");
    fmt.space();
    expr(fmt, match_arm.expr(fmt.tree).unwrap());
    fmt.write(',');
}

fn expr_field<'syn>(fmt: &mut Formatter<'syn, '_>, field: cst::ExprField<'syn>) {
    expr(fmt, field.target(fmt.tree).unwrap());
    fmt.write('.');
    name(fmt, field.name(fmt.tree).unwrap());
}

fn expr_index<'syn>(fmt: &mut Formatter<'syn, '_>, index: cst::ExprIndex<'syn>) {
    expr(fmt, index.target(fmt.tree).unwrap());
    fmt.write('[');
    expr(fmt, index.index(fmt.tree).unwrap());
    fmt.write(']');
}

fn expr_slice<'syn>(fmt: &mut Formatter<'syn, '_>, slice: cst::ExprSlice<'syn>) {
    expr(fmt, slice.target(fmt.tree).unwrap());
    fmt.write('[');
    fmt.write(':');
    if slice.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
        fmt.space();
    }
    expr(fmt, slice.range_(fmt.tree).unwrap());
    fmt.write(']');
}

fn expr_call<'syn>(fmt: &mut Formatter<'syn, '_>, call: cst::ExprCall<'syn>) {
    expr(fmt, call.target(fmt.tree).unwrap());
    args_list(fmt, call.args_list(fmt.tree).unwrap());
}

fn expr_cast<'syn>(fmt: &mut Formatter<'syn, '_>, cast: cst::ExprCast<'syn>) {
    expr(fmt, cast.target(fmt.tree).unwrap());
    fmt.space();
    fmt.write_str("as");
    fmt.space();
    ty(fmt, cast.into_ty(fmt.tree).unwrap());
}

fn expr_sizeof<'syn>(fmt: &mut Formatter<'syn, '_>, sizeof: cst::ExprSizeof<'syn>) {
    fmt.write_str("sizeof");
    fmt.write('(');
    ty(fmt, sizeof.ty(fmt.tree).unwrap());
    fmt.write(')');
}

fn expr_item<'syn>(fmt: &mut Formatter<'syn, '_>, expr: cst::ExprItem<'syn>) {
    path_expr(fmt, expr.path(fmt.tree).unwrap());
    if let Some(args_list_cst) = expr.args_list(fmt.tree) {
        args_list(fmt, args_list_cst);
    }
}

fn expr_variant<'syn>(fmt: &mut Formatter<'syn, '_>, variant: cst::ExprVariant<'syn>) {
    fmt.write('.');
    name(fmt, variant.name(fmt.tree).unwrap());
    if let Some(args_list_cst) = variant.args_list(fmt.tree) {
        args_list(fmt, args_list_cst);
    }
}

fn expr_struct_init<'syn>(fmt: &mut Formatter<'syn, '_>, struct_init: cst::ExprStructInit<'syn>) {
    if let Some(path_cst) = struct_init.path(fmt.tree) {
        path_expr(fmt, path_cst);
    }
    fmt.write('.');
    field_init_list(fmt, struct_init.field_init_list(fmt.tree).unwrap());
}

fn field_init_list<'syn>(fmt: &mut Formatter<'syn, '_>, field_init_list: cst::FieldInitList<'syn>) {
    if content_empty(fmt, field_init_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    let wrap = fmt.wrap_line_break_based(field_init_list.field_inits(fmt.tree));
    let empty = field_init_list.field_inits(fmt.tree).next().is_none();

    if wrap || empty {
        fmt.write('{');
        fmt.new_line();
        fmt.tab_inc();
        interleaved_node_list::<cst::FieldInit>(fmt, field_init_list.0);
        fmt.tab_dec();
        fmt.tab_depth();
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.space();
    let mut first = true;
    for field_init_cst in field_init_list.field_inits(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        field_init(fmt, field_init_cst);
    }
    fmt.space();
    fmt.write('}');
}

fn field_init<'syn>(fmt: &mut Formatter<'syn, '_>, field_init: cst::FieldInit<'syn>) {
    name(fmt, field_init.name(fmt.tree).unwrap());
    if let Some(expr_cst) = field_init.expr(fmt.tree) {
        fmt.write(':');
        fmt.space();
        expr(fmt, expr_cst);
    }
}

fn expr_array_init<'syn>(fmt: &mut Formatter<'syn, '_>, array_init: cst::ExprArrayInit<'syn>) {
    fmt.write('[');

    let mut first = true;
    for expr_cst in array_init.input(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        expr(fmt, expr_cst);
    }

    fmt.write(']');
}

fn expr_array_repeat<'syn>(
    fmt: &mut Formatter<'syn, '_>,
    array_repeat: cst::ExprArrayRepeat<'syn>,
) {
    fmt.write('[');
    expr(fmt, array_repeat.value(fmt.tree).unwrap());
    fmt.write(';');
    fmt.space();
    expr(fmt, array_repeat.len(fmt.tree).unwrap());
    fmt.write(']');
}

fn expr_deref<'syn>(fmt: &mut Formatter<'syn, '_>, deref: cst::ExprDeref<'syn>) {
    fmt.write('*');
    expr(fmt, deref.expr(fmt.tree).unwrap());
}

fn expr_address<'syn>(fmt: &mut Formatter<'syn, '_>, address: cst::ExprAddress<'syn>) {
    fmt.write('&');
    let mut with_space = false;

    if address.t_mut(fmt.tree).is_some() {
        fmt.write_str("mut");
        fmt.space();
        with_space = true;
    }

    let expr_cst = address.expr(fmt.tree).unwrap();
    if !with_space && matches!(expr_cst, cst::Expr::Address(_)) {
        fmt.space();
    }
    expr(fmt, expr_cst);
}

fn expr_unary<'syn>(fmt: &mut Formatter<'syn, '_>, unary: cst::ExprUnary<'syn>) {
    let (un_op, _) = unary.un_op(fmt.tree).unwrap();
    fmt.write_str(un_op.as_str());
    expr(fmt, unary.rhs(fmt.tree).unwrap());
}

fn expr_binary<'syn>(fmt: &mut Formatter<'syn, '_>, binary: cst::ExprBinary<'syn>) {
    expr(fmt, binary.lhs(fmt.tree).unwrap());
    fmt.space();
    let (bin_op, _) = binary.bin_op(fmt.tree).unwrap();
    fmt.write_str(bin_op.as_str());
    fmt.space();
    expr(fmt, binary.rhs(fmt.tree).unwrap());
}

//==================== PAT ====================

fn pat<'syn>(fmt: &mut Formatter<'syn, '_>, pat: cst::Pat<'syn>) {
    match pat {
        cst::Pat::Wild(_) => fmt.write('_'),
        cst::Pat::Lit(pat) => pat_lit(fmt, pat),
        cst::Pat::Item(pat) => pat_item(fmt, pat),
        cst::Pat::Variant(pat) => pat_variant(fmt, pat),
        cst::Pat::Or(pat) => pat_or(fmt, pat),
    }
}

fn pat_lit(fmt: &mut Formatter, pat: cst::PatLit) {
    if let Some((un_op, _)) = pat.un_op(fmt.tree) {
        fmt.write_str(un_op.as_str());
    }
    lit(fmt, pat.lit(fmt.tree).unwrap())
}

fn pat_item<'syn>(fmt: &mut Formatter<'syn, '_>, pat: cst::PatItem<'syn>) {
    path_expr(fmt, pat.path(fmt.tree).unwrap());
    if let Some(bind_list_cst) = pat.bind_list(fmt.tree) {
        bind_list(fmt, bind_list_cst);
    }
}

fn pat_variant(fmt: &mut Formatter, pat: cst::PatVariant) {
    fmt.write('.');
    name(fmt, pat.name(fmt.tree).unwrap());
    if let Some(bind_list_cst) = pat.bind_list(fmt.tree) {
        bind_list(fmt, bind_list_cst);
    }
}

fn pat_or<'syn>(fmt: &mut Formatter<'syn, '_>, pat_or: cst::PatOr<'syn>) {
    let wrap = fmt.wrap_line_break_based(pat_or.pats(fmt.tree));
    let mut first = true;

    for pat_cst in pat_or.pats(fmt.tree) {
        if !first {
            if wrap {
                fmt.new_line();
                fmt.tab_depth();
            } else {
                fmt.space();
            }
            fmt.write('|');
            fmt.space();
        }
        first = false;
        pat(fmt, pat_cst);
    }
}

fn lit(fmt: &mut Formatter, lit: cst::Lit) {
    fmt.write_range(lit.find_range(fmt.tree));
}

fn range<'syn>(fmt: &mut Formatter<'syn, '_>, range: cst::Range<'syn>) {
    match range {
        cst::Range::Full(_) => fmt.write_str(".."),
        cst::Range::ToExclusive(range) => {
            fmt.write_str("..<");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
        cst::Range::ToInclusive(range) => {
            fmt.write_str("..=");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
        cst::Range::From(range) => {
            expr(fmt, range.start(fmt.tree).unwrap());
            fmt.write_str("..");
        }
        cst::Range::Exclusive(range) => {
            expr(fmt, range.start(fmt.tree).unwrap());
            fmt.write_str("..<");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
        cst::Range::Inclusive(range) => {
            expr(fmt, range.start(fmt.tree).unwrap());
            fmt.write_str("..=");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
    }
}

//==================== COMMON ====================

fn name(fmt: &mut Formatter, name: cst::Name) {
    let range = name.ident(fmt.tree).unwrap();
    fmt.write_range(range);
}

fn bind(fmt: &mut Formatter, bind: cst::Bind) {
    if let Some(name_cst) = bind.name(fmt.tree) {
        if bind.t_mut(fmt.tree).is_some() {
            fmt.write_str("mut");
            fmt.space();
        }
        name(fmt, name_cst);
    } else {
        fmt.write('_');
    }
}

fn bind_list(fmt: &mut Formatter, bind_list: cst::BindList) {
    fmt.write('(');
    let mut first = true;
    for bind_cst in bind_list.binds(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        bind(fmt, bind_cst);
    }
    fmt.write(')');
}

fn args_list<'syn>(fmt: &mut Formatter<'syn, '_>, args_list: cst::ArgsList<'syn>) {
    if content_empty(fmt, args_list.0) {
        fmt.write('(');
        fmt.write(')');
        return;
    }

    let wrap = fmt.wrap_line_break_based(args_list.exprs(fmt.tree));
    let empty = args_list.exprs(fmt.tree).next().is_none();

    if wrap || empty {
        fmt.write('(');
        fmt.new_line();
        fmt.tab_inc();
        interleaved_node_list::<cst::Expr>(fmt, args_list.0);
        fmt.tab_dec();
        fmt.tab_depth();
        fmt.write(')');
        return;
    }

    fmt.write('(');
    let mut first = true;
    for expr_cst in args_list.exprs(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        expr(fmt, expr_cst);
    }
    fmt.write(')');
}

fn path_type<'syn>(fmt: &mut Formatter<'syn, '_>, path: cst::Path<'syn>) {
    let mut first = true;
    for segment in path.segments(fmt.tree) {
        if !first {
            fmt.write('.');
        }
        first = false;
        name(fmt, segment.name(fmt.tree).unwrap());
        if let Some(poly_args) = segment.poly_args(fmt.tree) {
            polymorph_args(fmt, poly_args);
        }
    }
}

fn path_expr<'syn>(fmt: &mut Formatter<'syn, '_>, path: cst::Path<'syn>) {
    let mut first = true;
    for segment in path.segments(fmt.tree) {
        if !first {
            fmt.write('.');
        }
        first = false;
        name(fmt, segment.name(fmt.tree).unwrap());
        if let Some(poly_args) = segment.poly_args(fmt.tree) {
            fmt.write(':');
            polymorph_args(fmt, poly_args);
        }
    }
}

fn polymorph_args<'syn>(fmt: &mut Formatter<'syn, '_>, poly_args: cst::PolymorphArgs<'syn>) {
    fmt.write('(');
    let mut first = true;
    for ty_cst in poly_args.types(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        ty(fmt, ty_cst);
    }
    fmt.write(')');
}

fn polymorph_params(fmt: &mut Formatter, poly_params: cst::PolymorphParams) {
    fmt.write('(');
    let mut first = true;
    for name_cst in poly_params.names(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        name(fmt, name_cst);
    }
    fmt.write(')');
}
