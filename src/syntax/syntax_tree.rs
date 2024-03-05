use crate::text_range::*;
use std::rc::Rc;

type SyntaxNode = Rc<SyntaxNodeData>;

struct SyntaxNodeData {
    kind: SyntaxKind,
    parent: Option<SyntaxNode>,
    children: Vec<SyntaxElement>,
    range: TextRange,
}

struct SyntaxTokenData {
    kind: SyntaxKind,
    range: TextRange,
}

enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxTokenData),
}

struct SyntaxTree {
    source: String,
    root: SyntaxNode,
}

#[derive(Copy, Clone, Debug)]
enum SyntaxKind {
    // Nodes
    Proc,
    ParamList,
    Name,
    BinExpr,
    // Tokens
    Whitespace,
    KwProc,
    LCurly,
    RCurly,
    Star,
    NumberInt,
}

impl SyntaxNodeData {
    fn new(
        kind: SyntaxKind,
        parent: Option<SyntaxNode>,
        children: Vec<SyntaxElement>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            parent,
            children,
            range,
        }
    }
    fn kind(&self) -> SyntaxKind {
        self.kind
    }
    fn range(&self) -> TextRange {
        self.range
    }
    fn parent<'a>(self: &'a SyntaxNode) -> Option<&'a SyntaxNode> {
        self.parent.as_ref()
    }
    fn children<'a>(self: &'a SyntaxNode) -> impl Iterator<Item = &'a SyntaxElement> {
        self.children.iter()
    }
}

impl SyntaxTokenData {
    fn new(kind: SyntaxKind, range: TextRange) -> Self {
        Self { kind, range }
    }
    fn kind(&self) -> SyntaxKind {
        self.kind
    }
    fn range(&self) -> TextRange {
        self.range
    }
}

impl SyntaxElement {
    fn kind(&self) -> SyntaxKind {
        match self {
            SyntaxElement::Node(it) => it.kind(),
            SyntaxElement::Token(it) => it.kind(),
        }
    }
    fn range(&self) -> TextRange {
        match self {
            SyntaxElement::Node(it) => it.range(),
            SyntaxElement::Token(it) => it.range(),
        }
    }
}

impl From<SyntaxNode> for SyntaxElement {
    fn from(value: SyntaxNode) -> Self {
        Self::Node(value)
    }
}

impl From<SyntaxTokenData> for SyntaxElement {
    fn from(value: SyntaxTokenData) -> Self {
        Self::Token(value)
    }
}

impl SyntaxTree {
    fn new(source: String, root: SyntaxNode) -> Self {
        Self { source, root }
    }

    fn to_string(&self) -> String {
        let mut string = String::new();
        self.children_to_string(&self.root, &mut string);
        string
    }

    fn children_to_string(&self, node_data: &SyntaxNodeData, string: &mut String) {
        for element in node_data.children.iter() {
            match element {
                SyntaxElement::Node(node) => self.children_to_string(&node, string),
                SyntaxElement::Token(token) => {
                    let range = token.range().start().into()..token.range().end().into();
                    string.push_str(&self.source[range]);
                }
            }
        }
    }
}

#[test]
fn test_syntax_tree() {
    let three = SyntaxTokenData::new(SyntaxKind::NumberInt, TextRange::new(0.into(), 1.into()));
    let ws = SyntaxTokenData::new(SyntaxKind::Whitespace, TextRange::new(1.into(), 2.into()));
    let star = SyntaxTokenData::new(SyntaxKind::Star, TextRange::new(2.into(), 3.into()));
    let ws2 = SyntaxTokenData::new(SyntaxKind::Whitespace, TextRange::new(3.into(), 4.into()));
    let four = SyntaxTokenData::new(SyntaxKind::NumberInt, TextRange::new(4.into(), 5.into()));
    let mul_expr = SyntaxNodeData::new(
        SyntaxKind::BinExpr,
        None,
        vec![
            three.into(),
            ws.into(),
            star.into(),
            ws2.into(),
            four.into(),
        ],
        TextRange::new(0.into(), 5.into()),
    );
    let tree = SyntaxTree::new("3 * 4".into(), Rc::new(mul_expr));
    let tree_string = tree.to_string();
    assert_eq!(tree.source, tree_string);
    println!("{}", tree_string);
}
