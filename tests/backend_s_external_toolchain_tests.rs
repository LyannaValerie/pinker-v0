mod common;

use common::render_backend_s_external_subset;
use pinker_v0::backend_s::emit_external_toolchain_subset;
use pinker_v0::cfg_ir::OperandIR;
use pinker_v0::instr_select::{
    SelectedBlock, SelectedFunction, SelectedProgram, SelectedTerminator,
};
use pinker_v0::ir::TypeIR;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn asm_s_external_subset_emite_main_montavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(
        "# pinker v0 external toolchain subset (fase 111, linux x86_64, frame/reg + memoria minima + multiplos blocos/labels + jmp incondicional + recusa branch condicional)"
    ));
    assert!(out.contains(".globl main"));
    assert!(out.contains("jmp .Lprincipal_entry"));
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("movabsq $42, %rax"));
}

#[test]
fn asm_s_external_subset_fase111_exemplo_versionado_emite_labels_e_jmp_incondicional() {
    let code = include_str!("../examples/fase111_blocos_labels_salto_incondicional_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("jmp .Lprincipal_entry"));
}

#[test]
fn asm_s_external_subset_fluxo_real_condicional() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let asm = render_backend_s_external_subset(code).unwrap();

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");

    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");

    assert_eq!(run.status.code(), Some(42));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_com_locais_e_aritmetica() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase73_backend_externo_locais_aritmetica_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("imulq"));
    assert!(asm.contains("subq"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(59));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_com_call_e_parametro_unico() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase74_backend_externo_call_minimo_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl main"));
    assert!(asm.contains(".globl dobro"));
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("call dobro"));
    assert!(asm.contains("imulq %r10, %rax"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(79));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase75_frame_registradores() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase75_backend_externo_frame_registradores_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("# frame: %rbp base"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(44));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase76_multiplos_parametros() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase76_backend_externo_multiplos_parametros_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("movq %rsi"));
    assert!(asm.contains("call soma2"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(41));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase77_memoria_frame_minima() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase77_backend_externo_memoria_frame_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq -"));
    assert!(asm.contains("(%rbp), %rax"));
    assert!(asm.contains("movq %rax, -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(23));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase78_composicao_interprocedural_linear() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code =
        include_str!("../examples/fase78_backend_externo_composicao_interprocedural_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call soma2"));
    assert!(asm.contains("call ajusta"));
    assert!(asm.contains("call combina"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(39));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase79_programa_linear_maior() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase79_backend_externo_programa_linear_maior_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call etapa"));
    assert!(asm.contains("call refina"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("subq %r10, %rax"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(207));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase80_cobertura_linear_auditavel_mais_ampla() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code =
        include_str!("../examples/fase80_backend_externo_cobertura_linear_ampla_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call base"));
    assert!(asm.contains("call mistura"));
    assert!(asm.contains("movq %rsi"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(167));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_falha_clara_fora_do_subset() {
    let code = include_str!("../examples/fase76_backend_externo_tres_args_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 84)"));
}

#[test]
fn asm_s_external_subset_falha_parametro_nao_bombom() {
    let code =
        include_str!("../examples/fase75_backend_externo_parametro_nao_bombom_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 84) aceita somente parâmetro `bombom`"));
}

#[test]
fn asm_s_external_subset_fase84_preserva_recusa_explicita_tres_parametros_por_funcao() {
    let code = include_str!(
        "../examples/fase81_backend_externo_recusa_explicita_tres_parametros_invalido.pink",
    );

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 84) recusa explicitamente 3+ parâmetros por função",
    ));
}

#[test]
fn asm_s_external_subset_fase84_recusa_explicita_talvez_senao() {
    let code = include_str!(
        "../examples/fase82_backend_externo_recusa_explicita_talvez_senao_invalido.pink",
    );

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 111) recusa branch condicional (`br`/`talvez`/`sempre que`)"
    ));
}

#[test]
fn asm_s_external_subset_fase84_recusa_explicita_sempre_que() {
    let code = include_str!(
        "../examples/fase84_backend_externo_recusa_explicita_sempre_que_invalido.pink",
    );

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 111) recusa branch condicional (`br`/`talvez`/`sempre que`)"
    ));
}

#[test]
fn asm_s_external_subset_fase84_matriz_fronteira_auditavel() {
    let casos_garantidos = [
        include_str!("../examples/fase73_backend_externo_locais_aritmetica_valido.pink"),
        include_str!("../examples/fase77_backend_externo_memoria_frame_valido.pink"),
        include_str!("../examples/fase78_backend_externo_composicao_interprocedural_valido.pink"),
    ];

    for code in casos_garantidos {
        let asm = render_backend_s_external_subset(code).expect("subset garantido deve emitir .s");
        assert!(asm.contains("# pinker v0 external toolchain subset (fase 111"));
    }

    let caso_rejeitado_tres_params = include_str!(
        "../examples/fase81_backend_externo_recusa_explicita_tres_parametros_invalido.pink",
    );
    let err_tres_params = render_backend_s_external_subset(caso_rejeitado_tres_params).unwrap_err();
    assert!(err_tres_params.to_string().contains(
        "subset externo montável (Fase 84) recusa explicitamente 3+ parâmetros por função"
    ));

    let caso_rejeitado_talvez = include_str!(
        "../examples/fase82_backend_externo_recusa_explicita_talvez_senao_invalido.pink",
    );
    let err_talvez = render_backend_s_external_subset(caso_rejeitado_talvez).unwrap_err();
    assert!(err_talvez.to_string().contains(
        "subset externo montável (Fase 111) recusa branch condicional (`br`/`talvez`/`sempre que`)"
    ));

    let caso_rejeitado_sempre_que = include_str!(
        "../examples/fase84_backend_externo_recusa_explicita_sempre_que_invalido.pink",
    );
    let err_sempre_que = render_backend_s_external_subset(caso_rejeitado_sempre_que).unwrap_err();
    assert!(err_sempre_que.to_string().contains(
        "subset externo montável (Fase 111) recusa branch condicional (`br`/`talvez`/`sempre que`)"
    ));
}

#[test]
fn asm_s_external_subset_fase111_falha_em_jmp_para_label_inexistente() {
    let mut slot_types = HashMap::new();
    slot_types.insert("x".to_string(), TypeIR::Bombom);
    let program = SelectedProgram {
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
            blocks: vec![SelectedBlock {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: SelectedTerminator::Jmp("sumiu".to_string()),
            }],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 111) encontrou `jmp` para label inexistente"));
}

#[test]
fn asm_s_external_subset_fase111_falha_em_label_duplicado() {
    let mut slot_types = HashMap::new();
    slot_types.insert("x".to_string(), TypeIR::Bombom);
    let program = SelectedProgram {
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
            blocks: vec![
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Jmp("entry".to_string()),
                },
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Ret(Some(OperandIR::Int(0))),
                },
            ],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 111) encontrou label duplicado em função"));
}

fn detect_cc_driver() -> Option<String> {
    ["cc", "gcc", "clang"].iter().find_map(|candidate| {
        let probe = Command::new(candidate).arg("--version").output().ok()?;
        if probe.status.success() {
            Some((*candidate).to_string())
        } else {
            None
        }
    })
}

fn unique_temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema inválido")
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_phase84_{}", nanos))
}
