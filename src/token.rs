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
    KwU8,
    KwU16,
    KwU32,
    KwU64,
    KwI8,
    KwI16,
    KwI32,
    KwI64,
    KwLogica,
    KwVerdade,
    KwFalso,
    Ident,
    IntLit,
    Plus,
    AmpAmp,
    Amp,
    PipePipe,
    Pipe,
    Caret,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    BangEq,
    Less,
    LessLess,
    LessEq,
    Greater,
    GreaterGreater,
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
            TokenKind::KwU8 => "KwU8",
            TokenKind::KwU16 => "KwU16",
            TokenKind::KwU32 => "KwU32",
            TokenKind::KwU64 => "KwU64",
            TokenKind::KwI8 => "KwI8",
            TokenKind::KwI16 => "KwI16",
            TokenKind::KwI32 => "KwI32",
            TokenKind::KwI64 => "KwI64",
            TokenKind::KwLogica => "KwLogica",
            TokenKind::KwVerdade => "KwVerdade",
            TokenKind::KwFalso => "KwFalso",
            TokenKind::Ident => "Ident",
            TokenKind::IntLit => "IntLit",
            TokenKind::Plus => "Plus",
            TokenKind::AmpAmp => "AmpAmp",
            TokenKind::Amp => "Amp",
            TokenKind::PipePipe => "PipePipe",
            TokenKind::Pipe => "Pipe",
            TokenKind::Caret => "Caret",
            TokenKind::Minus => "Minus",
            TokenKind::Star => "Star",
            TokenKind::Slash => "Slash",
            TokenKind::Percent => "Percent",
            TokenKind::Eq => "Eq",
            TokenKind::EqEq => "EqEq",
            TokenKind::BangEq => "BangEq",
            TokenKind::Less => "Less",
            TokenKind::LessLess => "LessLess",
            TokenKind::LessEq => "LessEq",
            TokenKind::Greater => "Greater",
            TokenKind::GreaterGreater => "GreaterGreater",
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
