use crate::error::PinkerError;
use crate::token::{Position, Span, Token, TokenKind};

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.char_indices().peekable(),
            line: 1,
            col: 1,
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

    pub fn tokenize(&mut self) -> Result<Vec<Token>, PinkerError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let start_pos = self.current_pos();

            match self.advance() {
                Some((_, c)) => {
                    let mut lexeme = c.to_string();
                    let kind = match c {
                        '+' => TokenKind::Plus,
                        '-' => {
                            if self.match_char('>') {
                                lexeme.push('>');
                                TokenKind::Arrow
                            } else {
                                TokenKind::Minus
                            }
                        }
                        '*' => TokenKind::Star,
                        '/' => TokenKind::Slash,
                        '(' => TokenKind::LParen,
                        ')' => TokenKind::RParen,
                        '{' => TokenKind::LBrace,
                        '}' => TokenKind::RBrace,
                        ',' => TokenKind::Comma,
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
                            } else {
                                TokenKind::Less
                            }
                        }
                        '>' => {
                            if self.match_char('=') {
                                lexeme.push('=');
                                TokenKind::GreaterEq
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
                                "quebrar" => TokenKind::KwQuebrar,
                                "eterno" => TokenKind::KwEterno,
                                "nova" => TokenKind::KwNova,
                                "mut" => TokenKind::KwMut,
                                "bombom" => TokenKind::KwBombom,
                                "logica" => TokenKind::KwLogica,
                                "verdade" => TokenKind::KwVerdade,
                                "falso" => TokenKind::KwFalso,
                                _ => TokenKind::Ident,
                            }
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
