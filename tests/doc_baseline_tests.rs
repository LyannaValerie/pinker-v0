//! Testes de ponta a ponta da Etapa 0 (Marco) da Trama Pinker.
//!
//! Exercitam o binário `pink doc` real: política forward-only, código de erro
//! `E-DOC-BASELINE` e exibição do marco.

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
fn importar_pr_anterior_ao_marco_e_rejeitado() {
    let root = temp_repo("reject_before");
    write_config(&root);

    let out = run_doc(&root, &["importar-pr", "329"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success(), "PR anterior deve falhar");
    assert!(stderr.contains("E-DOC-BASELINE"), "stderr: {stderr}");
    assert!(
        stderr.contains("O PR #329 é anterior ou igual ao marco documental #330."),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("PR #330, exclusivo"), "stderr: {stderr}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn importar_pr_igual_ao_marco_e_rejeitado() {
    let root = temp_repo("reject_equal");
    write_config(&root);

    let out = run_doc(&root, &["importar-pr", "330"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success(), "o próprio marco é exclusivo");
    assert!(stderr.contains("E-DOC-BASELINE"), "stderr: {stderr}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn importar_pr_posterior_ao_marco_e_elegivel() {
    let root = temp_repo("accept_after");
    write_config(&root);

    let out = run_doc(&root, &["importar-pr", "331"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "PR posterior deve ser aceito");
    assert!(
        stdout.contains("elegível para importação"),
        "stdout: {stdout}"
    );
    fs::remove_dir_all(root).unwrap();
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

    let out = run_doc(&root, &["importar-pr", "999"]);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success());
    assert!(stderr.contains("E-DOC-CONFIG"), "stderr: {stderr}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn numero_de_pr_invalido_e_rejeitado() {
    let root = temp_repo("bad_number");
    write_config(&root);

    let out = run_doc(&root, &["importar-pr", "abc"]);
    assert!(!out.status.success());
    fs::remove_dir_all(root).unwrap();
}
