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
