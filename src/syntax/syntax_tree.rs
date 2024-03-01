use std::{fmt, iter, sync::Arc};

#[derive(Copy, Clone)]
enum SyntaxKind {
    // Nodes
    Proc,
    Name,
    ParamList,
    BinExpr,
    // Tokens
    Whitespace,
    Ident,
    ProcKw,
    LitInt,
    Plus,
    Star,
}

#[derive(Copy, Clone)]
enum NodeOrToken<N, T> {
    Node(N),
    Token(T),
}

type Node = Arc<NodeData>;
struct NodeData {
    kind: SyntaxKind,
    children: Vec<NodeOrToken<Node, Token>>,
    len: usize,
}

type Token = Arc<TokenData>;
struct TokenData {
    kind: SyntaxKind,
    text: String,
}

impl NodeOrToken<Node, Token> {
    fn kind(&self) -> SyntaxKind {
        match self {
            NodeOrToken::Node(node) => node.kind(),
            NodeOrToken::Token(token) => token.kind(),
        }
    }
    fn text_len(&self) -> usize {
        match self {
            NodeOrToken::Node(node) => node.text_len(),
            NodeOrToken::Token(token) => token.text_len(),
        }
    }
}

impl From<Node> for NodeOrToken<Node, Token> {
    fn from(value: Node) -> Self {
        Self::Node(value)
    }
}

impl From<Token> for NodeOrToken<Node, Token> {
    fn from(value: Token) -> Self {
        Self::Token(value)
    }
}

impl fmt::Display for NodeOrToken<Node, Token> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeOrToken::Node(it) => fmt::Display::fmt(it, f),
            NodeOrToken::Token(it) => fmt::Display::fmt(it, f),
        }
    }
}

impl NodeData {
    fn new(kind: SyntaxKind, children: Vec<NodeOrToken<Node, Token>>) -> NodeData {
        let len = children.iter().map(|it| it.text_len()).sum();
        NodeData {
            kind,
            children,
            len,
        }
    }

    fn kind(&self) -> SyntaxKind {
        self.kind
    }
    fn text_len(&self) -> usize {
        self.len
    }
    fn children(&self) -> &[NodeOrToken<Node, Token>] {
        self.children.as_slice()
    }

    fn replace_child(&self, idx: usize, new_child: NodeOrToken<Node, Token>) -> NodeData {
        assert!(idx < self.children.len());
        let lhs_children = self.children.iter().take(idx).cloned();
        let rhs_children = self.children.iter().skip(idx + 1).cloned();
        let new_children = lhs_children
            .chain(iter::once(new_child))
            .chain(rhs_children)
            .collect();
        NodeData::new(self.kind, new_children)
    }
}

impl fmt::Display for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for child in self.children() {
            fmt::Display::fmt(child, f)?;
        }
        Ok(())
    }
}

impl TokenData {
    fn new(kind: SyntaxKind, text: String) -> TokenData {
        TokenData { kind, text }
    }

    fn kind(&self) -> SyntaxKind {
        self.kind
    }
    fn text(&self) -> &str {
        self.text.as_str()
    }
    fn text_len(&self) -> usize {
        self.text.len()
    }
}

impl fmt::Display for TokenData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.text(), f)
    }
}

#[test]
fn syntax_tree() {
    let ws = Arc::new(TokenData::new(SyntaxKind::Whitespace, " ".to_string()));
    let one = Arc::new(TokenData::new(SyntaxKind::LitInt, "1".to_string()));
    let two = Arc::new(TokenData::new(SyntaxKind::LitInt, "2".to_string()));
    let star = Arc::new(TokenData::new(SyntaxKind::Star, "*".to_string()));

    let expr_mul = Arc::new(NodeData::new(
        SyntaxKind::BinExpr,
        vec![
            one.into(),
            ws.clone().into(),
            star.into(),
            ws.clone().into(),
            two.into(),
        ],
    ));
    // "1 + 2"
    eprintln!("expr_mul: {}", expr_mul);

    let plus = Arc::new(TokenData::new(SyntaxKind::Plus, "+".to_string()));

    let expr_add = Arc::new(NodeData::new(
        SyntaxKind::BinExpr,
        vec![
            expr_mul.clone().into(),
            ws.clone().into(),
            plus.into(),
            ws.into(),
            expr_mul.into(),
        ],
    ));
    // "1 * 2 * 1 * 2"
    eprintln!("expr_add: {}", expr_add);
}
