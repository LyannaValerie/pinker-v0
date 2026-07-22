//! Trama Pinker — projeções documentais determinísticas
//! (§12; §20 itens 21, 22, 23, 24, 25, 26).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.trama.projection.fixture-config
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Quatro constantes definem configurações documentais completas ou sem state e corpos de PR para projeções integrais e isoladas.
const DOC_TOML_FULL: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"
repository = "LyannaValerie/pinker-v0"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"

[projections.history]
file = "docs/history/changes.md"
region = "change.history"

[projections.state]
file = "docs/engine/state.md"
region = "engine.state.generated"

[projections.roadmap]
file = "docs/roadmap/generated.md"
region = "roadmap.generated"
"#;

// doc.toml sem consumidor para `state` (para o teste de flag sem consumidor).
const DOC_TOML_NO_STATE: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"
repository = "LyannaValerie/pinker-v0"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"

[projections.history]
file = "docs/history/changes.md"
region = "change.history"
"#;

const BODY: &str = "## Resumo\ntexto\n\n```pinker-change\nschema: 1\nkind: phase\nphase: 241\nblock: 20\ntitle: Biblioteca de Resultado\narea:\n  - language.result\nstatus: completed\nupdates:\n  state: true\n  history: true\n  roadmap: true\nsections:\n  implemented:\n    - result.predeclared\n```\n";

const BODY_STATE_ONLY: &str = "## Resumo\ntexto\n\n```pinker-change\nschema: 1\nkind: phase\ntitle: X\nstatus: completed\nupdates:\n  state: true\n```\n";
// @pinker-nav:end evidencia.trama.projection.fixture-config

// @pinker-nav:start evidencia.trama.projection.process-support
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Sete helpers constroem portais, destinos, repositórios temporários, arquivos, processos doc, importação e a fixture documental completa.
fn portal(id: &str, domain: &str) -> String {
    format!("---\npinker-doc: 1\nid: {id}\ndomain: {domain}\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# {id}\n\nPortal.\n")
}

fn target(id: &str, domain: &str, region: &str, human: &str) -> String {
    format!(
        "---\npinker-doc: 1\nid: {id}\ndomain: {domain}\nkind: index\nstatus: active\nparent: {domain}\n---\n\n# Doc {id}\n\n{human}\n\n<!-- @pinker-generated:start {region} -->\n<!-- @pinker-generated:end {region} -->\n"
    )
}

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_proj_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn doc(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("doc")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn import(root: &Path, pr: &str) -> std::process::Output {
    let body_path = root.join("body.md").to_string_lossy().to_string();
    doc(root, &["importar-pr", pr, "--corpo", &body_path])
}

fn full_fixture(root: &Path, body: &str) {
    write(root, ".pinker/doc.toml", DOC_TOML_FULL);
    write(root, "body.md", body);
    write(root, "docs/engine/README.md", &portal("engine", "engine"));
    write(
        root,
        "docs/history/README.md",
        &portal("history", "history"),
    );
    write(
        root,
        "docs/roadmap/README.md",
        &portal("roadmap", "roadmap"),
    );
    write(
        root,
        "docs/engine/state.md",
        &target(
            "engine.state",
            "engine",
            "engine.state.generated",
            "TEXTO HUMANO DE ESTADO",
        ),
    );
    write(
        root,
        "docs/history/changes.md",
        &target(
            "history.changes",
            "history",
            "change.history",
            "TEXTO HUMANO DE HISTORICO",
        ),
    );
    write(
        root,
        "docs/roadmap/generated.md",
        &target(
            "roadmap.generated",
            "roadmap",
            "roadmap.generated",
            "TEXTO HUMANO DE ROADMAP",
        ),
    );
}
// @pinker-nav:end evidencia.trama.projection.process-support

// @pinker-nav:start evidencia.trama.projection.families-human-preservation
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de projeção conjunta de history, state e roadmap com preservação das regiões humanas dos documentos.
#[test]
fn projecoes_history_state_roadmap_e_regioes_humanas() {
    let root = temp_repo("full");
    full_fixture(&root, BODY);
    assert!(import(&root, "341").status.success());
    let sync = doc(&root, &["sincronizar"]);
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );

    // (21) história
    let history = fs::read_to_string(root.join("docs/history/changes.md")).unwrap();
    assert!(history.contains("#341"), "{history}");
    assert!(history.contains("Biblioteca de Resultado"), "{history}");
    // (26) região humana preservada
    assert!(history.contains("TEXTO HUMANO DE HISTORICO"), "{history}");

    // (22) estado
    let state = fs::read_to_string(root.join("docs/engine/state.md")).unwrap();
    assert!(state.contains("Manifestos processados: 1"), "{state}");
    assert!(state.contains("result.predeclared"), "{state}");
    assert!(state.contains("TEXTO HUMANO DE ESTADO"), "{state}");

    // (23) roadmap
    let roadmap = fs::read_to_string(root.join("docs/roadmap/generated.md")).unwrap();
    assert!(roadmap.contains("#341"), "{roadmap}");
    assert!(roadmap.contains("TEXTO HUMANO DE ROADMAP"), "{roadmap}");

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.projection.families-human-preservation

// @pinker-nav:start evidencia.trama.projection.idempotence
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de que sincronizações documentais repetidas são idempotentes e permanecem aprovadas por verificar.
#[test]
fn projecoes_sao_idempotentes() {
    let root = temp_repo("idem");
    full_fixture(&root, BODY);
    assert!(import(&root, "341").status.success());
    assert!(doc(&root, &["sincronizar"]).status.success());
    let once = fs::read_to_string(root.join("docs/history/changes.md")).unwrap();
    // Segundo sync não deve alterar nada.
    assert!(doc(&root, &["sincronizar"]).status.success());
    let twice = fs::read_to_string(root.join("docs/history/changes.md")).unwrap();
    assert_eq!(once, twice);
    // E `verificar` aprova.
    let verify = doc(&root, &["verificar"]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.projection.idempotence

// @pinker-nav:start evidencia.trama.projection.missing-consumer
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de erro quando uma atualização state não possui consumidor de projeção configurado.
#[test]
fn flag_updates_sem_consumidor_causa_erro() {
    let root = temp_repo("no_consumer");
    write(&root, ".pinker/doc.toml", DOC_TOML_NO_STATE);
    write(&root, "body.md", BODY_STATE_ONLY);
    write(
        &root,
        "docs/history/README.md",
        &portal("history", "history"),
    );
    write(
        &root,
        "docs/history/changes.md",
        &target("history.changes", "history", "change.history", "H"),
    );
    assert!(import(&root, "341").status.success());

    // `state: true` sem [projections.state] configurado deve falhar.
    let sync = doc(&root, &["sincronizar"]);
    assert!(!sync.status.success());
    assert!(String::from_utf8_lossy(&sync.stderr).contains("E-PROJ-CONSUMER"));

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.projection.missing-consumer
