//! Testes de ponta a ponta da importação de manifestos (Trama Pinker — Etapa 4).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

[projections.state]
file = "docs/engine/state.md"
region = "engine.state.generated"

[projections.history]
file = "docs/history/changes.md"
region = "change.history"
"#;

const STATE_DOC: &str = "---\npinker-doc: 1\nid: engine.state\ndomain: engine\nkind: reference\nstatus: active\nparent: engine\n---\n\n# Estado\n\n<!-- @pinker-generated:start engine.state.generated -->\n<!-- @pinker-generated:end engine.state.generated -->\n";

const HISTORY_DOC: &str = "---\npinker-doc: 1\nid: history.changes\ndomain: history\nkind: index\nstatus: active\nparent: history\n---\n\n# Mudanças\n\n<!-- @pinker-generated:start change.history -->\n<!-- @pinker-generated:end change.history -->\n";

const ENGINE_PORTAL: &str = "---\npinker-doc: 1\nid: engine\ndomain: engine\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# Engine\n\nPortal.\n";

const HISTORY_PORTAL: &str = "---\npinker-doc: 1\nid: history\ndomain: history\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# Histórico\n\nPortal.\n";

const BODY: &str = "## Resumo\nImplementa Resultado.\n\n```pinker-change\nschema: 1\nkind: phase\nphase: 241\nblock: 20\ntitle: Biblioteca predeclarada de Resultado\narea:\n  - language.result\nstatus: completed\nupdates:\n  state: true\n  history: true\nsections:\n  implemented:\n    - result.predeclared\nvalidation:\n  required:\n    - make ci\n```\n";

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_change_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("doc")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn fixture(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
    write(root, "body.md", BODY);
    write(root, "docs/engine/README.md", ENGINE_PORTAL);
    write(root, "docs/engine/state.md", STATE_DOC);
    write(root, "docs/history/README.md", HISTORY_PORTAL);
    write(root, "docs/history/changes.md", HISTORY_DOC);
}

#[test]
fn importa_pr_posterior_gera_manifesto_e_historico() {
    let root = temp_repo("import");
    fixture(&root);

    let body_path = root.join("body.md").to_string_lossy().to_string();
    let out = run(&root, &["importar-pr", "341", "--corpo", &body_path]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    assert!(manifest.contains("number: 341"), "{manifest}");
    assert!(manifest.contains("phase: 241"), "{manifest}");
    assert!(
        manifest.contains("title: Biblioteca predeclarada de Resultado"),
        "{manifest}"
    );

    let ledger = fs::read_to_string(root.join(".pinker/changes/index.jsonl")).unwrap();
    assert!(ledger.contains("\"pr\":341"), "{ledger}");

    // Sincroniza catálogo e projeções antes de verificar.
    let sync = run(&root, &["sincronizar"]);
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );

    // A projeção de histórico deve conter a mudança importada.
    let changes = fs::read_to_string(root.join("docs/history/changes.md")).unwrap();
    assert!(changes.contains("#341"), "{changes}");
    assert!(
        changes.contains("Biblioteca predeclarada de Resultado"),
        "{changes}"
    );

    let verify = run(&root, &["verificar"]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn importacao_e_idempotente() {
    let root = temp_repo("idem");
    fixture(&root);
    let body_path = root.join("body.md").to_string_lossy().to_string();

    run(&root, &["importar-pr", "341", "--corpo", &body_path]);
    let first = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    run(&root, &["importar-pr", "341", "--corpo", &body_path]);
    let second = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    assert_eq!(first, second);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn importar_pr_anterior_ao_marco_e_rejeitado_mesmo_com_corpo() {
    let root = temp_repo("reject");
    fixture(&root);
    let body_path = root.join("body.md").to_string_lossy().to_string();

    let out = run(&root, &["importar-pr", "329", "--corpo", &body_path]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-BASELINE"));
    assert!(!root.join(".pinker/changes/pr-329.yaml").exists());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corpo_sem_bloco_falha() {
    let root = temp_repo("noblock");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "body.md", "## Resumo\nsem bloco\n");
    let body_path = root.join("body.md").to_string_lossy().to_string();

    let out = run(&root, &["importar-pr", "341", "--corpo", &body_path]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-BLOCK"));

    fs::remove_dir_all(root).unwrap();
}
