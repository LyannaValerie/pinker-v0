mod common;

use common::{parse_and_check, tokenize};
use pinker_v0::token::TokenKind;

// @pinker-nav:start evidence.lexical.tokens-and-spans
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks the basic recognition of tokens (keywords, identifier, punctuation and final EOF) and the coherence of spans (line/column and lexeme) in the cases present.
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
// @pinker-nav:end evidence.lexical.tokens-and-spans

// @pinker-nav:start evidence.lexical.diagnostic
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Exercises the lexical diagnostic for an unexpected character: it expects the exact message (assert_eq) with the position, even though it is triggered through parse_and_check.
#[test]
fn erro_lexico_tem_formato_previsivel() {
    let err =
        parse_and_check("pacote main; carinho principal() -> bombom { nova x$ = 1; mimo 0; }")
            .unwrap_err()
            .to_string();
    assert_eq!(err, "Erro Léxico: caractere inesperado '$' em 1:52..1:53");
}
// @pinker-nav:end evidence.lexical.diagnostic

// @pinker-nav:start evidence.lexical.control-words
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks that the control-flow keywords (sempre/que, para/cada/em, quebrar, continuar) are tokenized with the expected kinds in the cases present.
#[test]
fn lexer_reconhece_sempre_que() {
    let tokens = tokenize("sempre que verdade { mimo; }").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwSempre);
    assert_eq!(tokens[1].kind, TokenKind::KwQue);
}

#[test]
fn lexer_reconhece_para_cada_em() {
    let tokens = tokenize("para cada item em itens { falar(item); }").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwPara);
    assert_eq!(tokens[1].kind, TokenKind::KwCada);
    assert_eq!(tokens[3].kind, TokenKind::KwEm);
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
// @pinker-nav:end evidence.lexical.control-words

// @pinker-nav:start evidence.lexical.operators
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks the presence of the bitwise operator tokens (&, |, ^, <<, >>), modulo (%) and short-circuit logical (&&, ||) in the cases present.
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
// @pinker-nav:end evidence.lexical.operators

// @pinker-nav:start evidence.lexical.fixed-types
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks that the keywords of the unsigned (u8/u16/u32/u64) and signed (i8/i16/i32/i64) fixed-width integer types are tokenized in the cases present.
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
// @pinker-nav:end evidence.lexical.fixed-types

// @pinker-nav:start evidence.lexical.construction-words
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks that the keywords of the language's constructs (apelido, ninho, seta, fragil, sussurro with a string literal, livre, virar, peso/alinhamento, trazer, verso) are tokenized in the cases present.
#[test]
fn lexer_reconhece_keyword_apelido() {
    let tokens = tokenize("apelido Byte = u8;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwApelido);
}

#[test]
fn lexer_reconhece_keyword_ninho() {
    let tokens = tokenize("ninho Ponto { x: bombom; }").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwNinho);
}

#[test]
fn lexer_reconhece_keyword_seta() {
    let tokens = tokenize("seta<bombom>").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwSeta);
}

#[test]
fn lexer_reconhece_keyword_fragil() {
    let tokens = tokenize("fragil seta<u8>").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwFragil);
}

#[test]
fn lexer_reconhece_keyword_sussurro_e_string_lit() {
    let tokens = tokenize(r#"sussurro("mov rax, 60", "syscall");"#).unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwSussurro);
    assert!(tokens.iter().any(|t| t.kind == TokenKind::StringLit));
}

#[test]
fn lexer_reconhece_keyword_livre() {
    let tokens = tokenize("livre;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwLivre);
}

#[test]
fn lexer_reconhece_keyword_virar() {
    let tokens = tokenize("x virar u8;").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwVirar));
}

#[test]
fn lexer_reconhece_keywords_peso_e_alinhamento() {
    let tokens = tokenize("peso(u16); alinhamento(seta<u8>);").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwPeso));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::KwAlinhamento));
}

#[test]
fn lexer_reconhece_keyword_trazer() {
    let tokens = tokenize("trazer util.soma;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwTrazer);
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Dot));
}

#[test]
fn lexer_reconhece_keyword_verso() {
    let tokens = tokenize("verso").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwVerso);
}
// @pinker-nav:end evidence.lexical.construction-words

// @pinker-nav:start evidence.lexical.arrays-accesses-and-modifiers
// @pinker-nav:domain lexical
// @pinker-nav:layer evidence
// @pinker-nav:summary Checks the tokenization of the fixed-array syntax ([u8; 16]), of the field-access/indexing tokens (Dot/LBracket/RBracket) and of the muda modifier as a keyword — confirming that 'mut' remains an identifier.
#[test]
fn lexer_reconhece_sintaxe_de_array_fixo() {
    let tokens = tokenize("[u8; 16]").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::LBracket);
    assert_eq!(tokens[1].kind, TokenKind::KwU8);
    assert_eq!(tokens[2].kind, TokenKind::Semi);
    assert_eq!(tokens[3].kind, TokenKind::IntLit);
    assert_eq!(tokens[4].kind, TokenKind::RBracket);
}

#[test]
fn lexer_reconhece_acesso_a_campo_e_indexacao() {
    let tokens = tokenize("obj.campo[1];").unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Dot));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::LBracket));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::RBracket));
}

#[test]
fn lexer_reconhece_muda_e_rejeita_mut_como_keyword() {
    let tokens = tokenize("nova muda x = 1;").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::KwNova);
    assert_eq!(tokens[1].kind, TokenKind::KwMuda);
    assert_eq!(tokens[2].kind, TokenKind::Ident);

    let tokens_mut = tokenize("nova mut x = 1;").unwrap();
    assert_eq!(tokens_mut[1].kind, TokenKind::Ident);
    assert_eq!(tokens_mut[1].lexeme, "mut");
}
// @pinker-nav:end evidence.lexical.arrays-accesses-and-modifiers
