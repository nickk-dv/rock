mod green;
mod kinds;
mod red;

pub use self::{
    green::{GreenNode, GreenNodeData, GreenToken, GreenTokenData},
    red::{RedNode, RedNodeData, RedToken, RedTokenData},
};
use kinds::SyntaxKind;
use std::sync::Arc;

#[derive(Copy, Clone)]
pub enum NodeOrToken<N, T> {
    Node(N),
    Token(T),
}

impl<N, T> NodeOrToken<N, T> {
    pub fn into_node(self) -> Option<N> {
        match self {
            NodeOrToken::Node(it) => Some(it),
            NodeOrToken::Token(_) => None,
        }
    }
    pub fn into_token(self) -> Option<T> {
        match self {
            NodeOrToken::Node(_) => None,
            NodeOrToken::Token(it) => Some(it),
        }
    }
}

#[test]
fn syntax_tree() {
    let ws = Arc::new(GreenTokenData::new(SyntaxKind::Whitespace, " ".to_string()));
    let one = Arc::new(GreenTokenData::new(SyntaxKind::LitInt, "1".to_string()));
    let two = Arc::new(GreenTokenData::new(SyntaxKind::LitInt, "2".to_string()));
    let star = Arc::new(GreenTokenData::new(SyntaxKind::Star, "*".to_string()));
    let plus = Arc::new(GreenTokenData::new(SyntaxKind::Plus, "+".to_string()));

    let expr_mul = Arc::new(GreenNodeData::new(
        SyntaxKind::BinExpr,
        vec![
            one.into(),
            ws.clone().into(),
            star.into(),
            ws.clone().into(),
            two.into(),
        ],
    ));
    assert_eq!(format!("{}", expr_mul), "1 * 2");
    println!("{}", expr_mul);

    let expr_add = Arc::new(GreenNodeData::new(
        SyntaxKind::BinExpr,
        vec![
            expr_mul.clone().into(),
            ws.clone().into(),
            plus.into(),
            ws.into(),
            expr_mul.into(),
        ],
    ));
    assert_eq!(format!("{}", expr_add), "1 * 2 + 1 * 2");
    println!("{}", expr_add);

    let red_expr_add = RedNodeData::new(expr_add);
    let mul2 = red_expr_add.children().nth(4).unwrap().into_node().unwrap();
    let three = Arc::new(GreenTokenData::new(SyntaxKind::LitInt, "3".to_string()));
    let new_root = mul2.replace_child(0, three.into());

    assert_eq!(format!("{}", new_root), "1 * 2 + 3 * 2");
    println!("{}", new_root);
}
