// @pinker-nav:start lexer.identifiers.reserved-namespace
// @pinker-nav:domain identifiers
// @pinker-nav:layer lexer
// @pinker-nav:summary Single boundary of identifiers originating from the source: the `AnyIdentifier` scope namespaces of the `native_symbol` authority — the nineteen forms the compiler actually materializes (`__pinker_internal_`, `__anon_carinho_`, `__impl_`, `__gen_`, `__gen_leque_`, `__fnref_env_`, the iteration slots, the `tentar`/propagation targets, `__env`, `__ternario`, ...) — are refused with `E-SEMANTIC-RESERVED-NAMESPACE` in every identifier position: declaration of a function, variable, parameter, constant, alias, ninho, leque, trato, method and field, and also any reference. The reservation is of the owned form, not of the `__` superprefix common to them, so `__usuario` remains a legal Pinker identifier. Because it sits at the point where the source text becomes `TokenKind::Ident`, no downstream consumer can observe a reserved identifier; synthetic identifiers built directly by the compiler are not lexed and therefore do not cross this boundary. The list is not duplicated here: the canonical table is `native_symbol::PINKER_OWNED_NAMESPACES`.
use crate::error::PinkerError;
use crate::source_map::SourceId;
use crate::token::{Position, Span, Token, TokenKind};

/// Prefixo de identificador reservado ao compilador, derivado da autoridade
/// única de namespace Pinker-owned. É uma das formas da tabela canônica, não
/// a reserva inteira: a fronteira consulta `reserved_namespace`, não este
/// prefixo isolado.
pub const RESERVED_INTERNAL_PREFIX: &str = crate::native_symbol::COMPILER_INTERNAL_PREFIX;

/// Fronteira única dos identificadores originados da fonte.
///
/// Recusa os namespaces reservados de escopo `AnyIdentifier` antes de o texto
/// da fonte virar um `TokenKind::Ident`, de modo que a garantia não depende de
/// auditar cada consumidor de identificador.
fn consume_source_identifier(lexeme: &str, span: Span) -> Result<(), PinkerError> {
    if let Some(namespace) = crate::native_symbol::reserved_namespace(
        lexeme,
        crate::native_symbol::ReservedScope::AnyIdentifier,
    ) {
        return Err(PinkerError::Lexer {
            msg: crate::native_symbol::reserved_namespace_message(lexeme, namespace),
            span,
        });
    }
    Ok(())
}
// @pinker-nav:end lexer.identifiers.reserved-namespace

// @pinker-nav:start lexer.cursor.character-reading
// @pinker-nav:domain cursor
// @pinker-nav:layer lexer
// @pinker-nav:summary Reading cursor of the lexer: it advances one character keeping line and column correct, peeks at the next one without consuming it and consumes conditionally when the expected character matches. It is the only point that moves the position, and therefore the only source of a correct span.
pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    col: usize,
    /// Unidade-fonte que este léxico está lendo.
    ///
    /// É o único ponto do compilador que precisa saber a resposta: todo span
    /// de token nasce aqui, e tudo o que o parser e as fases seguintes
    /// produzem deriva de spans de token. Vincular a fonte na origem evita
    /// carimbá-la depois, quando já não se distingue quem produziu a posição.
    source: SourceId,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self::com_fonte(input, SourceId::UNKNOWN)
    }

    pub fn com_fonte(input: &'a str, source: SourceId) -> Self {
        Self {
            chars: input.char_indices().peekable(),
            line: 1,
            col: 1,
            source,
        }
    }

    pub fn fonte(&self) -> SourceId {
        self.source
    }

    /// Tokeniza vinculando cada span à unidade-fonte deste léxico.
    ///
    /// O erro léxico atravessa o mesmo carimbo: um diagnóstico do léxico de um
    /// módulo é tão originado no módulo quanto qualquer token dele.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, PinkerError> {
        let source = self.source;
        match self.tokenize_sem_fonte() {
            Ok(mut tokens) => {
                for token in &mut tokens {
                    token.span = token.span.com_fonte_padrao(source);
                }
                Ok(tokens)
            }
            Err(err) => Err(err.com_fonte_padrao(source)),
        }
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next();
        if let Some((_, c)) = next {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        next
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }
    // @pinker-nav:end lexer.cursor.character-reading

    // @pinker-nav:start lexer.whitespace-comments.consumption
    // @pinker-nav:domain comments
    // @pinker-nav:layer lexer
    // @pinker-nav:summary Consumes whitespace and comments between tokens: `//` line comments and `/* */` block comments with nesting; an unterminated block comment ends at the end of the source without producing a token.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('/') => {
                    let mut clone = self.chars.clone();
                    clone.next();
                    if let Some(&(_, '/')) = clone.peek() {
                        self.advance();
                        self.advance();
                        while let Some(c) = self.peek_char() {
                            if c == '\n' {
                                break;
                            }
                            self.advance();
                        }
                    } else if let Some(&(_, '*')) = clone.peek() {
                        self.advance();
                        self.advance();
                        let mut depth = 1u32;
                        while depth > 0 {
                            match self.advance() {
                                Some((_, '/')) => {
                                    if self.peek_char() == Some('*') {
                                        self.advance();
                                        depth += 1;
                                    }
                                }
                                Some((_, '*')) => {
                                    if self.peek_char() == Some('/') {
                                        self.advance();
                                        depth -= 1;
                                    }
                                }
                                Some(_) => {}
                                None => break,
                            }
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn current_pos(&self) -> Position {
        Position::new(self.line, self.col)
    }
    // @pinker-nav:end lexer.whitespace-comments.consumption

    // @pinker-nav:start lexer.flow.tokenization
    // @pinker-nav:domain lexical
    // @pinker-nav:layer lexer
    // @pinker-nav:summary Main tokenization loop: it consumes the source after whitespace/comments and dispatches on the first character to produce operators and delimiters (including multi-character ones such as `->`, `==`, `<<`), integer literals, simple and multi-line `"""` strings with escapes, identifiers distinguished from keywords by the canonical vocabulary, interpolated `$"..."` and `?`; it emits EOF at the end and reports an unexpected character and unterminated literals.
    fn tokenize_sem_fonte(&mut self) -> Result<Vec<Token>, PinkerError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let start_pos = self.current_pos();

            match self.advance() {
                Some((_, c)) => {
                    let mut lexeme = c.to_string();
                    let kind = match c {
                        '+' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::PlusEq
                            } else {
                                TokenKind::Plus
                            }
                        }
                        '&' => {
                            if self.match_char('&') {
                                lexeme.push('&');
                                TokenKind::AmpAmp
                            } else {
                                TokenKind::Amp
                            }
                        }
                        '|' => {
                            if self.match_char('|') {
                                lexeme.push('|');
                                TokenKind::PipePipe
                            } else {
                                TokenKind::Pipe
                            }
                        }
                        '^' => TokenKind::Caret,
                        '-' => {
                            if self.match_char('>') {
                                lexeme.push('>');
                                TokenKind::Arrow
                            } else if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::MinusEq
                            } else {
                                TokenKind::Minus
                            }
                        }
                        '*' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::StarEq
                            } else {
                                TokenKind::Star
                            }
                        }
                        '/' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::SlashEq
                            } else {
                                TokenKind::Slash
                            }
                        }
                        '%' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::PercentEq
                            } else {
                                TokenKind::Percent
                            }
                        }
                        '(' => TokenKind::LParen,
                        ')' => TokenKind::RParen,
                        '[' => TokenKind::LBracket,
                        ']' => TokenKind::RBracket,
                        '{' => TokenKind::LBrace,
                        '}' => TokenKind::RBrace,
                        ',' => TokenKind::Comma,
                        '.' => {
                            if self.match_char('.') {
                                lexeme.push('.');
                                TokenKind::DotDot
                            } else {
                                TokenKind::Dot
                            }
                        }
                        ':' => TokenKind::Colon,
                        ';' => TokenKind::Semi,
                        '=' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::EqEq
                            } else {
                                TokenKind::Eq
                            }
                        }
                        '~' => TokenKind::Tilde,
                        '!' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::BangEq
                            } else {
                                TokenKind::Bang
                            }
                        }
                        '<' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::LessEq
                            } else if self.match_char('<') {
                                lexeme.push('<');
                                TokenKind::LessLess
                            } else {
                                TokenKind::Less
                            }
                        }
                        '>' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::GreaterEq
                            } else if self.match_char('>') {
                                lexeme.push('>');
                                TokenKind::GreaterGreater
                            } else {
                                TokenKind::Greater
                            }
                        }
                        c if c.is_ascii_digit() => {
                            while let Some(next_c) = self.peek_char() {
                                if next_c.is_ascii_digit() {
                                    lexeme.push(next_c);
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                            TokenKind::IntLit
                        }
                        '"' => {
                            lexeme.clear();
                            let mut triple = false;
                            {
                                let mut probe = self.chars.clone();
                                if let Some(&(_, '"')) = probe.peek() {
                                    probe.next();
                                    if let Some(&(_, '"')) = probe.peek() {
                                        triple = true;
                                    }
                                }
                            }
                            if triple {
                                self.advance();
                                self.advance();
                                let mut closed = false;
                                while let Some(next_c) = self.peek_char() {
                                    if next_c == '"' {
                                        let mut probe = self.chars.clone();
                                        probe.next();
                                        let second = probe.peek().map(|&(_, c)| c);
                                        if second == Some('"') {
                                            probe.next();
                                            let third = probe.peek().map(|&(_, c)| c);
                                            if third == Some('"') {
                                                self.advance();
                                                self.advance();
                                                self.advance();
                                                closed = true;
                                                break;
                                            }
                                        }
                                    }
                                    lexeme.push(next_c);
                                    self.advance();
                                }
                                if !closed {
                                    return Err(PinkerError::Lexer {
                                        msg: "string multi-linha não terminada (esperado \"\"\")"
                                            .to_string(),
                                        span: Span::new(start_pos, self.current_pos()),
                                    });
                                }
                            } else {
                                let mut closed = false;
                                while let Some(next_c) = self.peek_char() {
                                    if next_c == '"' {
                                        self.advance();
                                        closed = true;
                                        break;
                                    }
                                    if next_c == '\n' {
                                        return Err(PinkerError::Lexer {
                                            msg: "string literal não pode quebrar linha nesta fase"
                                                .to_string(),
                                            span: Span::new(start_pos, self.current_pos()),
                                        });
                                    }
                                    if next_c == '\\' {
                                        self.advance();
                                        let escaped = match self.peek_char() {
                                            Some('n') => '\n',
                                            Some('t') => '\t',
                                            Some('r') => '\r',
                                            Some('0') => '\0',
                                            Some('\\') => '\\',
                                            Some('"') => '"',
                                            Some(other) => {
                                                return Err(PinkerError::Lexer {
                                                    msg: format!(
                                                        "sequência de escape inválida '\\{}'",
                                                        other
                                                    ),
                                                    span: Span::new(start_pos, self.current_pos()),
                                                });
                                            }
                                            None => {
                                                return Err(PinkerError::Lexer {
                                                    msg: "string literal não terminada após '\\'"
                                                        .to_string(),
                                                    span: Span::new(start_pos, self.current_pos()),
                                                });
                                            }
                                        };
                                        self.advance();
                                        lexeme.push(escaped);
                                        continue;
                                    }
                                    lexeme.push(next_c);
                                    self.advance();
                                }
                                if !closed {
                                    return Err(PinkerError::Lexer {
                                        msg: "string literal não terminada".to_string(),
                                        span: Span::new(start_pos, self.current_pos()),
                                    });
                                }
                            }
                            TokenKind::StringLit
                        }
                        c if c.is_alphabetic() || c == '_' => {
                            while let Some(next_c) = self.peek_char() {
                                if next_c.is_alphanumeric() || next_c == '_' {
                                    lexeme.push(next_c);
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                            match lexeme.as_str() {
                                "pacote" => TokenKind::KwPacote,
                                "carinho" => TokenKind::KwCarinho,
                                "mimo" => TokenKind::KwMimo,
                                "talvez" => TokenKind::KwTalvez,
                                "senao" => TokenKind::KwSenao,
                                "sempre" => TokenKind::KwSempre,
                                "que" => TokenKind::KwQue,
                                "para" => TokenKind::KwPara,
                                "cada" => TokenKind::KwCada,
                                "em" => TokenKind::KwEm,
                                "quebrar" => TokenKind::KwQuebrar,
                                "continuar" => TokenKind::KwContinuar,
                                "eterno" => TokenKind::KwEterno,
                                "nova" => TokenKind::KwNova,
                                "muda" => TokenKind::KwMuda,
                                "apelido" => TokenKind::KwApelido,
                                "ninho" => TokenKind::KwNinho,
                                "leque" => TokenKind::KwLeque,
                                "encaixe" => TokenKind::KwEncaixe,
                                "tentar" => TokenKind::KwTentar,
                                "propagar" => TokenKind::KwPropagar,
                                "trato" => TokenKind::KwTrato,
                                "impl" => TokenKind::KwImpl,
                                "seta" => TokenKind::KwSeta,
                                "fragil" => TokenKind::KwFragil,
                                "sussurro" => TokenKind::KwSussurro,
                                "falar" => TokenKind::KwFalar,
                                "livre" => TokenKind::KwLivre,
                                "virar" => TokenKind::KwVirar,
                                "peso" => TokenKind::KwPeso,
                                "alinhamento" => TokenKind::KwAlinhamento,
                                "trazer" => TokenKind::KwTrazer,
                                "verso" => TokenKind::KwVerso,
                                "bombom" => TokenKind::KwBombom,
                                "u8" => TokenKind::KwU8,
                                "u16" => TokenKind::KwU16,
                                "u32" => TokenKind::KwU32,
                                "u64" => TokenKind::KwU64,
                                "i8" => TokenKind::KwI8,
                                "i16" => TokenKind::KwI16,
                                "i32" => TokenKind::KwI32,
                                "i64" => TokenKind::KwI64,
                                "logica" => TokenKind::KwLogica,
                                "verdade" => TokenKind::KwVerdade,
                                "falso" => TokenKind::KwFalso,
                                "nope" => TokenKind::KwNope,
                                "repetir" => TokenKind::KwRepetir,
                                "ate" => TokenKind::KwAte,
                                "escolha" => TokenKind::KwEscolha,
                                "caso" => TokenKind::KwCaso,
                                _ => {
                                    consume_source_identifier(
                                        &lexeme,
                                        Span::new(start_pos, self.current_pos()),
                                    )?;
                                    TokenKind::Ident
                                }
                            }
                        }
                        '?' => TokenKind::Question,
                        '$' if self.peek_char() == Some('"') => {
                            self.advance();
                            lexeme.clear();
                            let mut closed = false;
                            while let Some(next_c) = self.peek_char() {
                                if next_c == '"' {
                                    self.advance();
                                    closed = true;
                                    break;
                                }
                                if next_c == '\n' {
                                    return Err(PinkerError::Lexer {
                                        msg: "string interpolada não pode quebrar linha"
                                            .to_string(),
                                        span: Span::new(start_pos, self.current_pos()),
                                    });
                                }
                                if next_c == '\\' {
                                    self.advance();
                                    let escaped = match self.peek_char() {
                                        Some('n') => '\n',
                                        Some('t') => '\t',
                                        Some('r') => '\r',
                                        Some('0') => '\0',
                                        Some('\\') => '\\',
                                        Some('"') => '"',
                                        Some('{') => '{',
                                        Some('}') => '}',
                                        Some(other) => {
                                            return Err(PinkerError::Lexer {
                                                msg: format!(
                                                    "sequência de escape inválida '\\{}'",
                                                    other
                                                ),
                                                span: Span::new(start_pos, self.current_pos()),
                                            });
                                        }
                                        None => {
                                            return Err(PinkerError::Lexer {
                                                msg: "string interpolada não terminada após '\\'"
                                                    .to_string(),
                                                span: Span::new(start_pos, self.current_pos()),
                                            });
                                        }
                                    };
                                    self.advance();
                                    lexeme.push(escaped);
                                    continue;
                                }
                                lexeme.push(next_c);
                                self.advance();
                            }
                            if !closed {
                                return Err(PinkerError::Lexer {
                                    msg: "string interpolada não terminada".to_string(),
                                    span: Span::new(start_pos, self.current_pos()),
                                });
                            }
                            TokenKind::FStringLit
                        }
                        _ => {
                            return Err(PinkerError::Lexer {
                                msg: format!("caractere inesperado '{}'", c),
                                span: Span::new(start_pos, self.current_pos()),
                            });
                        }
                    };

                    let span = Span::new(start_pos, self.current_pos());
                    tokens.push(Token::new(kind, lexeme, span));
                }
                None => {
                    let pos = self.current_pos();
                    tokens.push(Token::new(TokenKind::Eof, String::new(), Span::single(pos)));
                    break;
                }
            }
        }

        Ok(tokens)
    }
}
// @pinker-nav:end lexer.flow.tokenization
