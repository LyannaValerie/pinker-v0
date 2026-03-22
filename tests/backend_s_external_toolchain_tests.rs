mod common;

use common::render_backend_s_external_subset;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn asm_s_external_subset_emite_main_montavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(
        "# pinker v0 external toolchain subset (fase 74, linux x86_64, callconv minima)"
    ));
    assert!(out.contains(".globl main"));
    assert!(out.contains("movabsq $42, %rax"));
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
fn asm_s_external_subset_falha_clara_fora_do_subset() {
    let code = include_str!("../examples/fase74_backend_externo_call_dois_args_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 74)"));
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
    std::env::temp_dir().join(format!("pinker_phase74_{}", nanos))
}
