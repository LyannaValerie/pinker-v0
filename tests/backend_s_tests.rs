mod common;

use common::{render_backend_s, render_cli_asm_s_output};

#[test]
fn asm_s_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_cli_asm_s_output(code).unwrap();
    assert_eq!(
        out,
        "\
=== ASM .S (TEXTUAL) ===
; pinker v0 textual .s (fase 53, derivado de --selected)
; module main
.text
.globl principal
principal:
  ; slots params=[] locals=[]
  .Lprincipal_entry:
    ret 0
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn asm_s_emite_if_else_simples() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_backend_s(code).unwrap();
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("br 1, .Lprincipal_then_0, .Lprincipal_else_1"));
    assert!(out.contains(".Lprincipal_then_0:"));
    assert!(out.contains("ret 1"));
    assert!(out.contains(".Lprincipal_else_1:"));
    assert!(out.contains("ret 0"));
}

#[test]
fn asm_s_falha_clara_para_tipo_ainda_nao_suportado() {
    let code = "\
pacote main;
carinho usa_ptr(p: seta<bombom>) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }";

    let err = render_backend_s(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("backend .s textual da Fase 53 ainda não suporta slot"));
}
