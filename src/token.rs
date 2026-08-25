use crate::source_map::SourceId;

// @pinker-nav:start token.lexico.vocabulario
// @pinker-nav:domain lexico
// @pinker-nav:layer token
// @pinker-nav:summary Vocabulário canônico de tokens da Pinker: palavras-chave em português, operadores, delimitadores e literais que o léxico produz e o parser consome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    KwPacote,
    KwCarinho,
    KwMimo,
    KwTalvez,
    KwSenao,
    KwSempre,
    KwQue,
    KwPara,
    KwCada,
    KwEm,
    KwQuebrar,
    KwContinuar,
    KwEterno,
    KwNova,
    KwMuda,
    KwApelido,
    KwNinho,
    KwLeque,
    KwEncaixe,
    KwTentar,
    KwPropagar,
    KwTrato,
    KwImpl,
    KwSeta,
    KwFragil,
    KwSussurro,
    KwFalar,
    KwLivre,
    KwVirar,
    KwPeso,
    KwAlinhamento,
    KwTrazer,
    KwVerso,
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
    KwNope,
    KwRepetir,
    KwAte,
    KwEscolha,
    KwCaso,
    Question,
    FStringLit,
    Ident,
    IntLit,
    StringLit,
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
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
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
    Tilde,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semi,
    DotDot,
    Arrow,
    Eof,
}
// @pinker-nav:end token.lexico.vocabulario

// @pinker-nav:start token.representacao.spans
// @pinker-nav:domain representacao
// @pinker-nav:layer token
// @pinker-nav:summary Posições e spans de origem (linha/coluna) anexados a cada token e propagados aos diagnósticos; inclui a fusão de spans usada ao combinar nós.
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

/// Localização de diagnóstico.
///
/// `SOURCE_LOCATION = SOURCE_ID + SPAN`: além do par de posições, um span
/// carrega a identidade da unidade-fonte de onde as posições vieram. Sem essa
/// metade, um span produzido a partir de um módulo pode ser renderizado contra
/// o texto da raiz — a linha existe nos dois arquivos e nada denuncia a troca.
///
/// `SourceId::UNKNOWN` é o valor de span sintético: ausência de alegação de
/// fonte, nunca alegação de raiz. `Span::new`/`Span::single` continuam com a
/// mesma aridade e produzem `UNKNOWN`; a forma vinculada é `Span::em`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
    pub source: SourceId,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            source: SourceId::UNKNOWN,
        }
    }

    /// Span vinculado à unidade-fonte que o produziu.
    pub fn em(source: SourceId, start: Position, end: Position) -> Self {
        Self { start, end, source }
    }

    /// Mesma extensão, agora atribuída a uma fonte.
    pub fn com_fonte(self, source: SourceId) -> Self {
        Self { source, ..self }
    }

    /// Atribui a fonte apenas quando o span ainda não reivindica nenhuma.
    /// Um span já vinculado nunca é reatribuído: é isso que impede que um
    /// carimbo de fronteira reescreva a origem real de uma posição.
    pub fn com_fonte_padrao(self, source: SourceId) -> Self {
        if self.source.is_unknown() {
            self.com_fonte(source)
        } else {
            self
        }
    }

    /// Une duas extensões. A fonte resultante é a do span da esquerda; se ela
    /// for desconhecida, herda a da direita. Unir posições de fontes distintas
    /// não é uma operação com significado, e o lado esquerdo é quem já era o
    /// dono do diagnóstico.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            source: if self.source.is_unknown() {
                other.source
            } else {
                self.source
            },
        }
    }

    pub fn single(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
            source: SourceId::UNKNOWN,
        }
    }

    /// Span de uma posição só, vinculado à unidade-fonte.
    pub fn unica_em(source: SourceId, pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
            source,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
// @pinker-nav:end token.representacao.spans

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
            TokenKind::IntLit | TokenKind::StringLit | TokenKind::KwVerdade | TokenKind::KwFalso
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
            TokenKind::KwPara => "KwPara",
            TokenKind::KwCada => "KwCada",
            TokenKind::KwEm => "KwEm",
            TokenKind::KwQuebrar => "KwQuebrar",
            TokenKind::KwContinuar => "KwContinuar",
            TokenKind::KwEterno => "KwEterno",
            TokenKind::KwNova => "KwNova",
            TokenKind::KwMuda => "KwMuda",
            TokenKind::KwApelido => "KwApelido",
            TokenKind::KwNinho => "KwNinho",
            TokenKind::KwLeque => "KwLeque",
            TokenKind::KwEncaixe => "KwEncaixe",
            TokenKind::KwTentar => "KwTentar",
            TokenKind::KwPropagar => "KwPropagar",
            TokenKind::KwTrato => "KwTrato",
            TokenKind::KwImpl => "KwImpl",
            TokenKind::KwSeta => "KwSeta",
            TokenKind::KwFragil => "KwFragil",
            TokenKind::KwSussurro => "KwSussurro",
            TokenKind::KwFalar => "KwFalar",
            TokenKind::KwLivre => "KwLivre",
            TokenKind::KwVirar => "KwVirar",
            TokenKind::KwPeso => "KwPeso",
            TokenKind::KwAlinhamento => "KwAlinhamento",
            TokenKind::KwTrazer => "KwTrazer",
            TokenKind::KwVerso => "KwVerso",
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
            TokenKind::KwNope => "KwNope",
            TokenKind::KwRepetir => "KwRepetir",
            TokenKind::KwAte => "KwAte",
            TokenKind::KwEscolha => "KwEscolha",
            TokenKind::KwCaso => "KwCaso",
            TokenKind::Question => "Question",
            TokenKind::FStringLit => "FStringLit",
            TokenKind::Ident => "Ident",
            TokenKind::IntLit => "IntLit",
            TokenKind::StringLit => "StringLit",
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
            TokenKind::PlusEq => "PlusEq",
            TokenKind::MinusEq => "MinusEq",
            TokenKind::StarEq => "StarEq",
            TokenKind::SlashEq => "SlashEq",
            TokenKind::PercentEq => "PercentEq",
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
            TokenKind::Tilde => "Tilde",
            TokenKind::LParen => "LParen",
            TokenKind::RParen => "RParen",
            TokenKind::LBracket => "LBracket",
            TokenKind::RBracket => "RBracket",
            TokenKind::LBrace => "LBrace",
            TokenKind::RBrace => "RBrace",
            TokenKind::Comma => "Comma",
            TokenKind::Dot => "Dot",
            TokenKind::Colon => "Colon",
            TokenKind::Semi => "Semi",
            TokenKind::DotDot => "DotDot",
            TokenKind::Arrow => "Arrow",
            TokenKind::Eof => "Eof",
        }
    }
}
