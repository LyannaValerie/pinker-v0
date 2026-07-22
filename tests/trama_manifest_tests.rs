//! Trama Pinker — manifestos imutáveis e validação real de schema
//! (§10, §11; §20 itens 15, 16, 17, 18).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.trama.manifest.fixture-config
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Configuração documental mínima usada pelas fixtures de manifesto.
const DOC_TOML: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"
repository = "LyannaValerie/pinker-v0"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;
// @pinker-nav:end evidencia.trama.manifest.fixture-config

// @pinker-nav:start evidencia.trama.manifest.process-support
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Helpers que montam corpos, repositórios temporários, arquivos, importações e configuração dos testes.
fn body(title: &str, kind: &str, status: &str) -> String {
    format!(
        "## Resumo\ntexto\n\n```pinker-change\nschema: 1\nkind: {kind}\ntitle: {title}\nstatus: {status}\narea:\n  - language.result\n```\n"
    )
}

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_man_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn import(root: &Path, pr: &str, body_rel: &str) -> std::process::Output {
    let body_path = root.join(body_rel).to_string_lossy().to_string();
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["doc", "importar-pr", pr, "--corpo", &body_path, "--repo"])
        .arg(root)
        .output()
        .expect("executar pink")
}

fn setup(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
}
// @pinker-nav:end evidencia.trama.manifest.process-support

// @pinker-nav:start evidencia.trama.manifest.idempotence-immutability
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de idempotência para conteúdo igual e imutabilidade para conteúdo divergente.
#[test]
fn manifesto_idempotente_com_conteudo_igual() {
    let root = temp_repo("idem");
    setup(&root);
    write(&root, "a.md", &body("Resultado", "phase", "completed"));

    assert!(import(&root, "341", "a.md").status.success());
    let first = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    let second = import(&root, "341", "a.md");
    assert!(second.status.success());
    let again = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    assert_eq!(first, again);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manifesto_imutavel_com_conteudo_diferente() {
    let root = temp_repo("immutable");
    setup(&root);
    write(&root, "a.md", &body("Resultado", "phase", "completed"));
    write(&root, "b.md", &body("Outro Titulo", "phase", "completed"));

    assert!(import(&root, "341", "a.md").status.success());
    let before = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();

    // Reimportar o MESMO PR com corpo diferente deve falhar e não reescrever.
    let out = import(&root, "341", "b.md");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-IMMUTABLE"));
    let after = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    assert_eq!(before, after, "manifesto imutável não pode mudar");

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.manifest.idempotence-immutability

// @pinker-nav:start evidencia.trama.manifest.enum-validation
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de rejeição dos valores inválidos dos enums kind e status.
#[test]
fn enum_de_kind_invalido_falha() {
    let root = temp_repo("kind");
    setup(&root);
    write(&root, "a.md", &body("Titulo", "banana", "completed"));
    let out = import(&root, "341", "a.md");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-SCHEMA"));
    assert!(!root.join(".pinker/changes/pr-341.yaml").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn enum_de_status_invalido_falha() {
    let root = temp_repo("status");
    setup(&root);
    write(&root, "a.md", &body("Titulo", "phase", "talvez"));
    let out = import(&root, "341", "a.md");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-SCHEMA"));
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.manifest.enum-validation

// @pinker-nav:start evidencia.trama.manifest.unknown-field
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de rejeição de campo desconhecido no manifesto de mudança.
#[test]
fn campo_desconhecido_falha() {
    let root = temp_repo("unknown");
    setup(&root);
    let body = "## Resumo\ntexto\n\n```pinker-change\nschema: 1\nkind: phase\ntitle: Titulo\nstatus: completed\nbanana: 42\n```\n";
    write(&root, "a.md", body);
    let out = import(&root, "341", "a.md");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-SCHEMA"));
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.manifest.unknown-field
