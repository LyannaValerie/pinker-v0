#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    KwPacote,
    KwCarinho,
    KwMimo,
    KwTalvez,
    KwSenao,
    KwSempre,
    KwQue,
    KwQuebrar,
    KwContinuar,
    KwEterno,
    KwNova,
    KwMut,
    KwBombom,
    KwLogica,
    KwVerdade,
    KwFalso,
    Ident,
    IntLit,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    EqEq,
    BangEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Bang,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semi,
    Arrow,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }

    pub fn single(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, span: Span) -> Self {
        Self { kind, lexeme, span }
    }
}

impl TokenKind {
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            TokenKind::IntLit | TokenKind::KwVerdade | TokenKind::KwFalso
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::KwPacote => "KwPacote",
            TokenKind::KwCarinho => "KwCarinho",
            TokenKind::KwMimo => "KwMimo",
            TokenKind::KwTalvez => "KwTalvez",
            TokenKind::KwSenao => "KwSenao",
            TokenKind::KwSempre => "KwSempre",
            TokenKind::KwQue => "KwQue",
            TokenKind::KwQuebrar => "KwQuebrar",
            TokenKind::KwContinuar => "KwContinuar",
            TokenKind::KwEterno => "KwEterno",
            TokenKind::KwNova => "KwNova",
            TokenKind::KwMut => "KwMut",
            TokenKind::KwBombom => "KwBombom",
            TokenKind::KwLogica => "KwLogica",
            TokenKind::KwVerdade => "KwVerdade",
            TokenKind::KwFalso => "KwFalso",
            TokenKind::Ident => "Ident",
            TokenKind::IntLit => "IntLit",
            TokenKind::Plus => "Plus",
            TokenKind::Minus => "Minus",
            TokenKind::Star => "Star",
            TokenKind::Slash => "Slash",
            TokenKind::Eq => "Eq",
            TokenKind::EqEq => "EqEq",
            TokenKind::BangEq => "BangEq",
            TokenKind::Less => "Less",
            TokenKind::LessEq => "LessEq",
            TokenKind::Greater => "Greater",
            TokenKind::GreaterEq => "GreaterEq",
            TokenKind::Bang => "Bang",
            TokenKind::LParen => "LParen",
            TokenKind::RParen => "RParen",
            TokenKind::LBrace => "LBrace",
            TokenKind::RBrace => "RBrace",
            TokenKind::Comma => "Comma",
            TokenKind::Colon => "Colon",
            TokenKind::Semi => "Semi",
            TokenKind::Arrow => "Arrow",
            TokenKind::Eof => "Eof",
        }
    }
}
