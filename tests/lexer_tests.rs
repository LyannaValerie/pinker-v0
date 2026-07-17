mod common;

use common::{parse_and_check, tokenize};
use pinker_v0::token::TokenKind;

// @pinker-nav:start evidencia.lexico.tokens-e-spans
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica o reconhecimento básico de tokens (palavras-chave, identificador, pontuação e EOF final) e a coerência de spans (linha/coluna e lexeme) nos casos presentes.
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
// @pinker-nav:end evidencia.lexico.tokens-e-spans

// @pinker-nav:start evidencia.lexico.diagnostico
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita o diagnóstico léxico de caractere inesperado: espera a mensagem exata (assert_eq) com a posição, ainda que acionada através de parse_and_check.
#[test]
fn erro_lexico_tem_formato_previsivel() {
    let err =
        parse_and_check("pacote main; carinho principal() -> bombom { nova x$ = 1; mimo 0; }")
            .unwrap_err()
            .to_string();
    assert_eq!(err, "Erro Léxico: caractere inesperado '$' em 1:52..1:53");
}
// @pinker-nav:end evidencia.lexico.diagnostico

// @pinker-nav:start evidencia.lexico.palavras-controle
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica que as palavras-chave de controle de fluxo (sempre/que, para/cada/em, quebrar, continuar) são tokenizadas com os kinds esperados nos casos presentes.
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
// @pinker-nav:end evidencia.lexico.palavras-controle

// @pinker-nav:start evidencia.lexico.operadores
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica a presença dos tokens de operadores bitwise (&, |, ^, <<, >>), módulo (%) e lógicos de curto-circuito (&&, ||) nos casos presentes.
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
// @pinker-nav:end evidencia.lexico.operadores

// @pinker-nav:start evidencia.lexico.tipos-fixos
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica que as palavras-chave dos tipos inteiros de largura fixa unsigned (u8/u16/u32/u64) e signed (i8/i16/i32/i64) são tokenizadas nos casos presentes.
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
// @pinker-nav:end evidencia.lexico.tipos-fixos

// @pinker-nav:start evidencia.lexico.palavras-de-construcao
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica que as palavras-chave de construções da linguagem (apelido, ninho, seta, fragil, sussurro com string literal, livre, virar, peso/alinhamento, trazer, verso) são tokenizadas nos casos presentes.
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
// @pinker-nav:end evidencia.lexico.palavras-de-construcao

// @pinker-nav:start evidencia.lexico.arrays-acessos-e-modificadores
// @pinker-nav:domain lexico
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica a tokenização da sintaxe de array fixo ([u8; 16]), dos tokens de acesso a campo/indexação (Dot/LBracket/RBracket) e do modificador muda como keyword — confirmando que 'mut' permanece identificador.
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
// @pinker-nav:end evidencia.lexico.arrays-acessos-e-modificadores
