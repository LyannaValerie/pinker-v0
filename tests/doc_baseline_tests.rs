//! Testes de ponta a ponta da Etapa 0 (Marco) da Trama Pinker.
//!
//! Exercitam o binário `pink doc` real: exibição do marco, erro de configuração
//! ausente e a ausência definitiva da superfície de autoria de manifestos.
//!
//! POT/LPT: AUTHORITY #698
//! POT/LPT: INVARIANT `pink doc` não possui subcomando de importação; o marco
//! sobrevive como fronteira de leitura do acervo histórico.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DOC_TOML: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "15e22d4d510f298282c11cafeb21718859f9493a"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_doc_{name}_{now}"))
}

fn write_config(root: &Path) {
    let path = root.join(".pinker/doc.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, DOC_TOML).unwrap();
}

fn run_doc(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("doc")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar binário pink")
}

#[test]
fn marco_exibe_configuracao() {
    let root = temp_repo("marco");
    write_config(&root);

    let out = run_doc(&root, &["marco"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success());
    assert!(stdout.contains("PR #330, exclusivo"), "stdout: {stdout}");
    assert!(stdout.contains("forward-only"), "stdout: {stdout}");
    assert!(
        stdout.contains("15e22d4d510f298282c11cafeb21718859f9493a"),
        "stdout: {stdout}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configuracao_ausente_falha_com_erro_claro() {
    let root = temp_repo("missing_config");
    fs::create_dir_all(&root).unwrap();

    let out = run_doc(&root, &["marco"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success());
    assert!(stderr.contains("E-DOC-CONFIG"), "stderr: {stderr}");
    fs::remove_dir_all(root).unwrap();
}

/// Controle causal do corte de #698: a superfície de autoria não existe mais.
#[test]
fn importar_pr_nao_e_mais_um_subcomando() {
    let root = temp_repo("no_import");
    write_config(&root);

    let out = run_doc(&root, &["importar-pr", "331"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success(), "stderr: {stderr}");
    assert!(
        !stderr.contains("elegível para importação"),
        "stderr: {stderr}"
    );
    fs::remove_dir_all(root).unwrap();
}

/// A ajuda do comando não pode reanunciar a obrigação retirada.
#[test]
fn ajuda_de_doc_nao_menciona_autoria_de_manifesto() {
    let root = temp_repo("help");
    write_config(&root);

    let out = run_doc(&root, &["--help"]);
    let texto = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    for proibido in ["importar-pr", "pinker-change", "--corpo", "--freeze"] {
        assert!(!texto.contains(proibido), "ajuda ainda cita '{proibido}'");
    }
    fs::remove_dir_all(root).unwrap();
}
