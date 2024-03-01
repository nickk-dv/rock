#[derive(Copy, Clone)]
pub enum SyntaxKind {
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
