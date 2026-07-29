mod common;

use common::{
    parse_and_check, render_backend_s_external_subset, render_backend_text, render_cfg_ir,
    render_ir, render_machine, render_selected,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    assert!(!asm.contains("pinker_publico_validar_"), "{asm}");
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
        ("examples/fase245_ponteiro_funcao_valido.pink", "42\n63\n"),
        ("examples/fase245_ponteiro_funcao_spill_valido.pink", "36\n"),
        (
            "examples/fase245_ponteiro_funcao_aridades_valido.pink",
            "0\n1\n3\n15\n21\n28\n36\n",
        ),
        (
            "examples/fase245_ponteiro_funcao_nulo_valido.pink",
            "verdade\nverdade\nverdade\nverdade\n245\n",
        ),
        (
            "examples/fase245_principal_endereco_valido.pink",
            "verdade\n",
        ),
        (
            "examples/fase245_ternario_ponteiro_funcao_valido.pink",
            "41\n42\n",
        ),
        (
            "examples/fase245_contrato_adulto_valido.pink",
            "42\n1\n63\n42\n245\n5\n",
        ),
        ("examples/fase245_aridade_12_valido.pink", "78\n"),
        (
            "examples/fase245_assinaturas_abi_valido.pink",
            "1\n2\n3\n4\n5\n6\n7\n8\nverdade\n10\nabi\nverdade\n",
        ),
        (
            "examples/fase245_abi_opacos_valido.pink",
            "1\n1\n1\n1\n1\n1\n1\nverdade\n7\n42\n10\n",
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
            "'alocar' excede o maior bloco representável pela plataforma",
        ),
        (
            "examples/fase246_double_free_invalido.pink",
            "E-RUNTIME-MEM-DOUBLE-FREE",
        ),
        (
            "examples/fase246_ponteiro_estrangeiro_invalido.pink",
            "E-RUNTIME-MEM-FOREIGN-FREE",
        ),
        (
            "examples/fase246_uso_apos_liberar_invalido.pink",
            "E-RUNTIME-MEM-USE-AFTER-FREE",
        ),
        (
            "examples/fase246_metadata_allocator_isolada_invalido.pink",
            "E-RUNTIME-MEM-ADDRESS-OVERFLOW",
        ),
        (
            "examples/fase246_escape_regiao_publica_invalido.pink",
            "E-RUNTIME-MEM-OUT-OF-BOUNDS",
        ),
        (
            "examples/fase246_limite_multibyte_invalido.pink",
            "E-RUNTIME-MEM-CROSS-BOUNDARY",
        ),
        (
            "examples/fase246_um_depois_invalido.pink",
            "E-RUNTIME-MEM-CROSS-BOUNDARY",
        ),
        (
            "examples/fase246_desalinhado_invalido.pink",
            "E-RUNTIME-MEM-MISALIGNED",
        ),
        (
            "examples/fase246_interior_free_invalido.pink",
            "E-RUNTIME-MEM-INTERIOR-FREE",
        ),
        ("examples/fase246_null_free_invalido.pink", "ponteiro nulo"),
        (
            "examples/fase245_ponteiro_nulo_chamada_invalida.pink",
            "chamada nula por ponteiro cru",
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
        ("examples/fase246_tamanho_minimo_valido.pink", "7\n"),
        ("examples/fase246_duas_alocacoes_valido.pink", "16\n"),
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

#[test]
fn fase246_tamanhos_alinhamentos_e_integracao_245_tem_paridade_interpretada() {
    for (path, stdout) in [
        (
            "examples/fase246_tamanhos_alinhamentos_valido.pink",
            "1\n2\n3\n4\n5\n6\n7\n8\nverdade\n",
        ),
        (
            "examples/fase246_inicializacao_zerada_valido.pink",
            "0\n0\n0\n",
        ),
        (
            "examples/fase246_escalares_fronteiras_aliases_valido.pink",
            "255\n65535\n4294967295\n9223372036854775808\n-128\n-32768\n-2147483648\n-9223372036854775808\n18446744073709551615\nverdade\nverdade\nverdade\nfalso\nfalso\n120\n22136\n99\n",
        ),
        ("examples/fases245_246_integracao_valido.pink", "246\n"),
        (
            "examples/fase246_reuso_endereco_valido.pink",
            "1\n2\n3\n",
        ),
        (
            "examples/fase246_retorno_ponteiro_inferido_valido.pink",
            "255\n254\n",
        ),
        (
            "examples/fase246_chamada_expressao_retorna_ponteiro_valido.pink",
            "253\n",
        ),
        (
            "examples/fase246_closure_captura_ponteiro_valido.pink",
            "252\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", path])
            .output()
            .expect("execução interpretada");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout);
    }
}

fn native_output_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("relógio do sistema")
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_{label}_{nanos}"))
}

fn build_native(example: &str, output_dir: &std::path::Path) -> std::path::PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(output_dir)
        .arg(example)
        .output()
        .expect("build nativo");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stem = std::path::Path::new(example)
        .file_stem()
        .expect("nome do exemplo");
    output_dir.join(stem)
}

#[test]
fn fases245_246_elf_real_tem_paridade_de_stdout_e_exit() {
    for example in [
        "examples/fase245_ponteiro_funcao_spill_valido.pink",
        "examples/fase245_ponteiro_funcao_aridades_valido.pink",
        "examples/fase245_aridade_12_valido.pink",
        "examples/fase245_contrato_adulto_valido.pink",
        "examples/fase245_assinaturas_abi_valido.pink",
        "examples/fase245_abi_opacos_valido.pink",
        "examples/fase245_ponteiro_funcao_nulo_valido.pink",
        "examples/fase245_principal_endereco_valido.pink",
        "examples/fase245_ternario_ponteiro_funcao_valido.pink",
        "examples/fase246_memoria_explicita_valido.pink",
        "examples/fase246_tamanhos_alinhamentos_valido.pink",
        "examples/fase246_inicializacao_zerada_valido.pink",
        "examples/fase246_escalares_fronteiras_aliases_valido.pink",
        "examples/fase246_reuso_endereco_valido.pink",
        "examples/fase246_retorno_ponteiro_inferido_valido.pink",
        "examples/fase246_chamada_expressao_retorna_ponteiro_valido.pink",
        "examples/fase246_closure_captura_ponteiro_valido.pink",
        "examples/fases245_246_integracao_valido.pink",
    ] {
        let out_dir = native_output_dir("phase245_246_parity");
        let executable = build_native(example, &out_dir);
        if example.ends_with("fase246_escalares_fronteiras_aliases_valido.pink") {
            let assembly = fs::read_to_string(executable.with_extension("s"))
                .expect("assembly da matriz escalar");
            for opcode in [
                "movb %r10b",
                "movw %r10w",
                "movl %r10d",
                "movzbq",
                "movsbq",
                "movzwq",
                "movswq",
                "movslq",
                "setl",
                "setle",
                "setg",
                "setge",
            ] {
                assert!(
                    assembly.contains(opcode),
                    "opcode ausente: {opcode}\n{assembly}"
                );
            }
        }
        let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", example])
            .output()
            .expect("interpretador");
        let native = Command::new(&executable).output().expect("execução do ELF");
        assert_eq!(interpreted.status.code(), Some(0));
        assert_eq!(native.status.code(), Some(0));
        let program_stdout = String::from_utf8_lossy(&interpreted.stdout);
        assert_eq!(program_stdout, String::from_utf8_lossy(&native.stdout));
        fs::remove_dir_all(out_dir).expect("limpeza da fixture nativa");
    }
}

#[test]
fn fases245_246_erros_de_runtime_tem_paridade_nativa() {
    for (example, expected) in [
        (
            "examples/fase245_ponteiro_nulo_chamada_invalida.pink",
            "chamada nula por ponteiro cru de função",
        ),
        (
            "examples/fase246_limite_multibyte_invalido.pink",
            "E-RUNTIME-MEM-CROSS-BOUNDARY",
        ),
        (
            "examples/fase246_um_depois_invalido.pink",
            "E-RUNTIME-MEM-CROSS-BOUNDARY",
        ),
        (
            "examples/fase246_escape_regiao_publica_invalido.pink",
            "E-RUNTIME-MEM-OUT-OF-BOUNDS",
        ),
        (
            "examples/fase246_desalinhado_invalido.pink",
            "E-RUNTIME-MEM-MISALIGNED",
        ),
        (
            "examples/fase246_interior_free_invalido.pink",
            "E-RUNTIME-MEM-INTERIOR-FREE",
        ),
        ("examples/fase246_null_free_invalido.pink", "ponteiro nulo"),
        (
            "examples/fase246_double_free_invalido.pink",
            "E-RUNTIME-MEM-DOUBLE-FREE",
        ),
        (
            "examples/fase246_uso_apos_liberar_invalido.pink",
            "E-RUNTIME-MEM-USE-AFTER-FREE",
        ),
        (
            "examples/fase246_overflow_invalido.pink",
            "maior bloco representável",
        ),
    ] {
        let out_dir = native_output_dir("phase245_246_native_errors");
        let executable = build_native(example, &out_dir);
        let native = Command::new(&executable)
            .output()
            .expect("execução nativa inválida");
        assert!(!native.status.success(), "{example}");
        assert!(
            String::from_utf8_lossy(&native.stderr).contains(expected),
            "{example}: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        fs::remove_dir_all(out_dir).expect("limpeza de caso inválido");
    }
}

#[test]
fn fase246_falha_injetada_e_deterministica_nos_dois_modos() {
    let example = "examples/fase246_overflow_invalido.pink";
    let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", example])
        .output()
        .expect("falha interpretada determinística");
    assert!(!interpreted.status.success());
    assert!(
        String::from_utf8_lossy(&interpreted.stderr)
            .contains("'alocar' excede o maior bloco representável"),
        "{}",
        String::from_utf8_lossy(&interpreted.stderr)
    );

    let out_dir = native_output_dir("phase246_alloc_failure");
    let executable = build_native(example, &out_dir);
    let native = Command::new(executable)
        .output()
        .expect("falha nativa determinística");
    assert!(!native.status.success());
    assert!(
        String::from_utf8_lossy(&native.stderr)
            .contains("'alocar' excede o maior bloco representável"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    fs::remove_dir_all(out_dir).expect("limpeza da falha injetada");
}

#[test]
fn fases245_246_build_nativo_e_deterministico() {
    for example in [
        "examples/fase245_ponteiro_funcao_valido.pink",
        "examples/fase246_duas_alocacoes_valido.pink",
    ] {
        let out_a = native_output_dir("phase245_246_determinism_a");
        let out_b = native_output_dir("phase245_246_determinism_b");
        let exe_a = build_native(example, &out_a);
        let exe_b = build_native(example, &out_b);
        assert_eq!(
            fs::read(&exe_a).expect("ELF A"),
            fs::read(&exe_b).expect("ELF B")
        );
        assert_eq!(
            fs::read(exe_a.with_extension("s")).expect("assembly A"),
            fs::read(exe_b.with_extension("s")).expect("assembly B")
        );
        fs::remove_dir_all(out_a).expect("limpeza A");
        fs::remove_dir_all(out_b).expect("limpeza B");
    }
}
