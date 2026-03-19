mod common;

use common::{parse_and_check, tokenize};
use pinker_v0::token::TokenKind;

#[test]
fn lexer_basico_reconhece_tokens() {
    let tokens = tokenize("pacote main; carinho principal() -> bombom { mimo 0; }").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwPacote);
    assert_eq!(tokens[1].kind, TokenKind::Ident);
    assert_eq!(tokens[2].kind, TokenKind::Semi);
    assert_eq!(tokens[3].kind, TokenKind::KwCarinho);
    assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn spans_lexicos_sao_coerentes() {
    let tokens = tokenize("nova x = 10;").unwrap();
    assert_eq!(tokens[0].span.start.line, 1);
    assert_eq!(tokens[0].span.start.col, 1);
    assert_eq!(tokens[0].span.end.col, 5);
    assert_eq!(tokens[3].lexeme, "10");
    assert_eq!(tokens[3].span.start.col, 10);
    assert_eq!(tokens[3].span.end.col, 12);
}

#[test]
fn erro_lexico_tem_formato_previsivel() {
    let err =
        parse_and_check("pacote main; carinho principal() -> bombom { nova x$ = 1; mimo 0; }")
            .unwrap_err()
            .to_string();
    assert_eq!(err, "Erro Léxico: caractere inesperado '$' em 1:52..1:53");
}

#[test]
fn lexer_reconhece_sempre_que() {
    let tokens = tokenize("sempre que verdade { mimo; }").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwSempre);
    assert_eq!(tokens[1].kind, TokenKind::KwQue);
}

#[test]
fn lexer_reconhece_quebrar() {
    let tokens = tokenize("quebrar;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwQuebrar);
}

#[test]
fn lexer_reconhece_continuar() {
    let tokens = tokenize("continuar;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwContinuar);
}

#[test]
fn lexer_reconhece_operadores_bitwise_basicos() {
    let tokens = tokenize("a & b | c ^ d << 1 >> 2;").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Amp));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Pipe));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Caret));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::LessLess));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::GreaterGreater));
}

#[test]
fn lexer_reconhece_operador_modulo() {
    let tokens = tokenize("a % b;").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Percent));
}

#[test]
fn lexer_reconhece_operadores_logicos_curto_circuito() {
    let tokens = tokenize("a && b || c;").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::AmpAmp));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::PipePipe));
}

#[test]
fn lexer_reconhece_tipos_unsigned_fixos() {
    let tokens = tokenize("u8 u16 u32 u64").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwU8));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwU16));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwU32));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwU64));
}

#[test]
fn lexer_reconhece_tipos_signed_fixos() {
    let tokens = tokenize("i8 i16 i32 i64").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwI8));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwI16));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwI32));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwI64));
}

#[test]
fn lexer_reconhece_keyword_apelido() {
    let tokens = tokenize("apelido Byte = u8;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwApelido);
}
