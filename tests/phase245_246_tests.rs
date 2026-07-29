mod common;

use common::{
    parse_and_check, render_backend_s_external_subset, render_backend_text, render_cfg_ir,
    render_ir, render_machine, render_selected,
};
use std::process::Command;

#[test]
fn fase245_atravessa_pipeline_com_representacao_crua_distinta() {
    let code = include_str!("../examples/fase245_ponteiro_funcao_valido.pink");
    let ir = render_ir(code).expect("IR da fase 245");
    assert!(ir.contains("raw_fnref(dobrar)"), "{ir}");
    assert!(ir.contains("call_raw"), "{ir}");

    let cfg = render_cfg_ir(code).expect("CFG da fase 245");
    assert!(cfg.contains("raw_fnref(dobrar)"), "{cfg}");
    assert!(cfg.contains("call_raw"), "{cfg}");

    let selected = render_selected(code).expect("seleção da fase 245");
    assert!(selected.contains("call_raw"), "{selected}");

    let machine = render_machine(code).expect("máquina da fase 245");
    assert!(
        machine.contains("push_raw_function_ref dobrar"),
        "{machine}"
    );
    assert!(machine.contains("call_raw"), "{machine}");

    let text = render_backend_text(code).expect("backend textual da fase 245");
    assert!(text.contains("call_raw"), "{text}");
}

#[test]
fn fase245_backend_nativo_emite_endereco_e_call_sem_descritor_callable() {
    let code = include_str!("../examples/fase245_ponteiro_funcao_valido.pink");
    let asm = render_backend_s_external_subset(code).expect("assembly da fase 245");
    assert!(asm.contains("leaq dobrar(%rip)"), "{asm}");
    assert!(asm.contains("call *%r10"), "{asm}");
    assert!(!asm.contains(".Lpinker_fnref_dobrar"), "{asm}");
}

#[test]
fn fase245_diagnosticos_rejeitam_conversoes_e_assinaturas_invalidas() {
    let cases = [
        (
            include_str!("../examples/fase245_aridade_invalida.pink"),
            "aridade inválida",
        ),
        (
            include_str!("../examples/fase245_tipo_invalido.pink"),
            "tipo inválido no argumento 1",
        ),
        (
            include_str!("../examples/fase245_callable_para_cru_invalido.pink"),
            "tipo de inicialização incompatível",
        ),
        (
            include_str!("../examples/fase245_closure_capturante_invalida.pink"),
            "rejeita variável, callable ou closure",
        ),
        (
            include_str!("../examples/fase245_simbolo_inexistente_invalido.pink"),
            "símbolo de função 'ausente' não resolvido",
        ),
        (
            include_str!("../examples/fase245_abi_composto_invalido.pink"),
            "tipo ABI não suportado",
        ),
    ];
    for (code, expected) in cases {
        let error = parse_and_check(code).expect_err("programa deveria ser rejeitado");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn fase245_interpretador_cobre_valor_spill_e_retorno_nulo() {
    for (path, stdout) in [
        (
            "examples/fase245_ponteiro_funcao_valido.pink",
            "42\n63\n0\n",
        ),
        (
            "examples/fase245_ponteiro_funcao_spill_valido.pink",
            "36\n0\n",
        ),
        (
            "examples/fase245_ponteiro_funcao_nulo_valido.pink",
            "245\n0\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", path])
            .output()
            .expect("execução do interpretador");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout);
    }
}

#[test]
fn fase246_atravessa_pipeline_e_mapeia_runtime_publico() {
    let code = include_str!("../examples/fase246_memoria_explicita_valido.pink");
    for rendered in [
        render_ir(code).expect("IR"),
        render_cfg_ir(code).expect("CFG"),
        render_selected(code).expect("seleção"),
        render_machine(code).expect("máquina"),
    ] {
        assert!(rendered.contains("alocar"), "{rendered}");
        assert!(rendered.contains("liberar"), "{rendered}");
    }
    let asm = render_backend_s_external_subset(code).expect("assembly");
    assert!(asm.contains("call pinker_publico_alocar"), "{asm}");
    assert!(asm.contains("call pinker_publico_liberar"), "{asm}");
}

#[test]
fn fase246_interpretador_detecta_zero_double_free_estrangeiro_e_uaf() {
    for (path, expected) in [
        (
            "examples/fase246_tamanho_zero_invalido.pink",
            "'alocar' rejeita tamanho zero",
        ),
        (
            "examples/fase246_overflow_invalido.pink",
            "'alocar' excede o limite público de 16777216 bytes",
        ),
        (
            "examples/fase246_double_free_invalido.pink",
            "'liberar' detectou double free",
        ),
        (
            "examples/fase246_ponteiro_estrangeiro_invalido.pink",
            "'liberar' rejeita ponteiro estrangeiro",
        ),
        (
            "examples/fase246_uso_apos_liberar_invalido.pink",
            "uso após liberar detectado",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", path])
            .output()
            .expect("execução do caso inválido");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn fase246_tamanho_minimo_e_duas_regioes_independentes() {
    for (path, stdout) in [
        ("examples/fase246_tamanho_minimo_valido.pink", "7\n0\n"),
        ("examples/fase246_duas_alocacoes_valido.pink", "16\n0\n"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", path])
            .output()
            .expect("execução de região pública válida");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout);
    }
}
