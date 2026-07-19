mod common;

use common::{render_backend_s, render_cli_asm_s_output};

// @pinker-nav:start evidencia.backend-s.apresentacao-cli-asm-s
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Golden exato da apresentação sintética em memória de render_cli_asm_s_output: cabeçalho ASM .S textual, representação textual hospedada mínima com metadados de ABI e rodapé histórico; não executa processo CLI nem produz assembly montável.
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
// @pinker-nav:end evidencia.backend-s.apresentacao-cli-asm-s

// @pinker-nav:start evidencia.backend-s.renderizacao-fluxo-e-abi-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica por contains a representação .s textual de if/else e a ABI textual mínima de parâmetros e chamada, incluindo rótulos, branches, metadados abi.* e temporário de retorno; não comprova instruções x86, montagem, link ou execução.
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
// @pinker-nav:end evidencia.backend-s.renderizacao-fluxo-e-abi-textual

// @pinker-nav:start evidencia.backend-s.validacao-subset-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita o diagnóstico do subset .s textual ao recusar slot seta<bombom>, verificando apenas a mensagem clara de tipo ainda não suportado nesse caminho textual.
#[test]
fn asm_s_falha_clara_para_tipo_ainda_nao_suportado() {
    let code = "\
pacote main;
carinho usa_ptr(p: seta<bombom>) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }";

    let err = render_backend_s(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("backend .s textual ainda não suporta slot"));
}
// @pinker-nav:end evidencia.backend-s.validacao-subset-textual

// @pinker-nav:start evidencia.backend-s.freestanding-intencao-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica por contains que o modo livre expõe intenção freestanding na representação textual, com boot.entry, linker script mínimo, kernel stub, _start e laço de espera; não monta, linka, inicializa hardware nem executa esse material.
#[test]
fn asm_s_freestanding_exibe_boot_entry_e_linker_script_minimo() {
    let code = "\
pacote main;
livre;
carinho principal() -> bombom { mimo 0; }";

    let out = render_backend_s(code).unwrap();
    assert!(out.contains("; boot.entry principal -> _start"));
    assert!(out.contains("; linker.script.v0 (textual, mínimo):"));
    assert!(out.contains("; kernel.stub.v0 (experimental):"));
    assert!(out.contains(";   ENTRY(_start)"));
    assert!(out.contains(".text : { *(.text*) }"));
    assert!(out.contains(".globl _start"));
    assert!(out.contains("_start:"));
    assert!(out.contains("call principal"));
    assert!(out.contains(".Lpinker_hang:"));
    assert!(out.contains("jmp .Lpinker_hang"));
}
// @pinker-nav:end evidencia.backend-s.freestanding-intencao-textual
