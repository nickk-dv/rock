use std::iter;

#[derive(Copy, Clone)]
enum NodeOrToken<N, T> {
    Node(N),
    Token(T),
}

#[derive(Clone)]
struct Node {
    kind: SyntaxKind,
    children: Vec<NodeOrToken<Node, Token>>,
    len: usize,
}

#[derive(Clone)]
struct Token {
    kind: SyntaxKind,
    text: String,
}

#[derive(Copy, Clone)]
enum SyntaxKind {
    Proc,
    ProcKw,
    Name,
    Ident,
    ParamList,
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

impl Node {
    fn new(kind: SyntaxKind, children: Vec<NodeOrToken<Node, Token>>) -> Node {
        let len = children.iter().map(|it| it.text_len()).sum();
        Node {
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

    fn replace_child(&self, idx: usize, new_child: NodeOrToken<Node, Token>) -> Node {
        assert!(idx < self.children.len());
        let left_children = self.children.iter().take(idx).cloned();
        let right_children = self.children.iter().take(idx + 1).cloned();
        let new_children = left_children
            .chain(iter::once(new_child))
            .chain(right_children)
            .collect();
        Node::new(self.kind, new_children)
    }
}

impl Token {
    fn new(kind: SyntaxKind, text: String) -> Token {
        Token { kind, text }
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
