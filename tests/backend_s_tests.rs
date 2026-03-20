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
; pinker v0 textual .s (fase 54, abi textual minima, derivado de --selected)
; module main
; mode hospedado
; abi pinker.text.v0
.text
; abi.func principal
; abi.params []
; abi.ret @ret
; abi.frame prologue=.Lprincipal_prologue epilogue=.Lprincipal_epilogue
.globl principal
principal:
  .Lprincipal_prologue:
    ; abi.prologue (textual)
  ; slots params=[] locals=[]
  .Lprincipal_entry:
    ret @ret, 0
  .Lprincipal_epilogue:
    ; abi.epilogue (textual)
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
    assert!(out.contains("ret @ret, 1"));
    assert!(out.contains(".Lprincipal_else_1:"));
    assert!(out.contains("ret @ret, 0"));
}

#[test]
fn asm_s_abi_minima_para_parametros_e_chamada() {
    let code = "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }";
    let out = render_backend_s(code).unwrap();

    assert!(out.contains("; abi.func soma"));
    assert!(out.contains("; abi.params [@arg0=$%x#0, @arg1=$%y#0]"));
    assert!(out.contains("; abi.ret @ret"));
    assert!(out.contains("; abi.frame prologue=.Lsoma_prologue epilogue=.Lsoma_epilogue"));
    assert!(out.contains("call soma, 1, 2 ; abi.call [@arg0=1, @arg1=2] -> %t0"));
    assert!(out.contains("ret @ret, %t0"));
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
        .contains("backend .s textual da Fase 54 ainda não suporta slot"));
}
