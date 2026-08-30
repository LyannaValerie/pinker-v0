//! Contrato processual de pink nav cobertura-diff (#438).

use pinker_v0::nav::CodeCatalog;
use pinker_v0::nav_projection_snapshot::{
    measure, render, ProjectionSnapshot, SnapshotState, SNAPSHOT_SCHEMA_V1,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DOC_TOML_BASE: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

const DOC_TOML_WITH_PROJECTION: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"

[projections.state]
file = "docs/engine/state.md"
region = "engine.state.generated"
"#;

const DEVELOPMENT_PORTAL: &str = r#"---
pinker-doc: 1
id: development
domain: development
kind: portal
status: active
parent: atlas
---

# Desenvolvimento
"#;

const CONTRACT_DOC: &str = r#"---
pinker-doc: 1
id: development.diff-coverage
domain: development
kind: reference
status: active
parent: development
canonical_for:
  - development.diff-coverage
---

# Cobertura de diff

<!-- @pinker-doc:start
id: development.diff-coverage.contract
tags: [diff, cobertura]
aliases:
  - cobertura de diff
summary: Contrato explícito de cobertura.
-->
## Contrato

Somente leitura.
<!-- @pinker-doc:end development.diff-coverage.contract -->
"#;

const CODE: &str = r#"// @pinker-nav:start codigo.alvo
// @pinker-nav:domain fixture
// @pinker-nav:layer core
// @pinker-nav:symbol fixture::alvo|alvo|rust-function|declaration
// @pinker-nav:symbol fixture::alvo|alvo|rust-function|implementation
// @pinker-nav:symbol-doc fixture::alvo|development.diff-coverage.contract
// @pinker-nav:summary Região de produção explicitamente identificada.
fn alvo() -> i32 { 1 }
// @pinker-nav:end codigo.alvo
"#;

const EVIDENCE: &str = r#"// @pinker-nav:start evidencia.alvo
// @pinker-nav:domain fixture
// @pinker-nav:layer evidencia
// @pinker-nav:test-for fixture::alvo
// @pinker-nav:summary Teste explicitamente associado ao alvo.
#[test]
fn cobre_alvo() { assert_eq!(2 + 2, 4); }
// @pinker-nav:end evidencia.alvo
"#;

const MANIFEST: &str = r#"schema: 1
source:
  type: github-pr
  number: 438
kind: parallel-phase
block: 20
title: "Cobertura de diff"
area:
  - development.diff-coverage
status: completed
updates:
  state: true
  history: false
  roadmap: false
validation:
  required:
    - make ci
"#;

struct Repo(PathBuf);

impl Repo {
    fn new(label: &str) -> Repo {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pinker_diff_coverage_{label}_{}_{}_{}",
            std::process::id(),
            nonce,
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Repo(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, content: &str) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, content).unwrap();
}

fn fixture(label: &str) -> Repo {
    let repo = Repo::new(label);
    write(repo.path(), ".pinker/doc.toml", DOC_TOML_BASE);
    write(
        repo.path(),
        "docs/development/README.md",
        DEVELOPMENT_PORTAL,
    );
    write(
        repo.path(),
        "docs/development/diff-coverage.md",
        CONTRACT_DOC,
    );
    write(repo.path(), "src/alvo.rs", CODE);
    write(
        repo.path(),
        "runtime/pinker_rt/src/lib.rs",
        "pub fn runtime() {}\n",
    );
    fs::create_dir_all(repo.path().join("apps")).unwrap();
    write(repo.path(), "tests/evidence.rs", EVIDENCE);
    assert_success(&run(repo.path(), &["doc", "sincronizar"], ""));
    assert_success(&run(repo.path(), &["nav", "sincronizar"], ""));

    write(repo.path(), ".pinker/doc.toml", DOC_TOML_WITH_PROJECTION);
    write(repo.path(), ".pinker/changes/pr-438.yaml", MANIFEST);
    write(
        repo.path(),
        "docs/engine/state.md",
        "<!-- @pinker-generated:start engine.state.generated -->\n\
         <!-- @pinker-generated:end engine.state.generated -->\n",
    );
    fs::create_dir_all(repo.path().join(".pinker/projections/recipes")).unwrap();
    let catalog = CodeCatalog::load(&repo.path().join("src/navigation.jsonl")).unwrap();
    let snapshot = ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA_V1,
        id: "fixture-current".to_string(),
        state: SnapshotState::Frozen,
        predecessor: None,
        justification: Some("fixture corrente".to_string()),
        measures: measure(catalog.regions.iter()),
        expected_overrides: 0,
        expected_exclusions: 0,
        expected_materializations: 0,
        base_snapshot: None,
        recipes: Vec::new(),
        rules: Vec::new(),
    };
    write(
        repo.path(),
        ".pinker/projections/fixture-current.toml",
        &render(&snapshot),
    );
    repo
}

fn run(root: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn coverage(root: &Path, diff: &str, json: bool) -> Output {
    let mut args = vec!["nav", "cobertura-diff"];
    if json {
        args.push("--json");
    }
    run(root, &args, diff)
}

fn code_line() -> usize {
    CODE.lines()
        .position(|line| line.starts_with("fn alvo"))
        .unwrap()
        + 1
}

fn doc_line() -> usize {
    CONTRACT_DOC
        .lines()
        .position(|line| line == "Somente leitura.")
        .unwrap()
        + 1
}

fn complete_diff() -> String {
    format!(
        "diff --git a/src/alvo.rs b/src/alvo.rs\n\
         --- a/src/alvo.rs\n\
         +++ b/src/alvo.rs\n\
         @@ -{code} +{code} @@\n\
         -fn alvo() -> i32 {{ 0 }}\n\
         +fn alvo() -> i32 {{ 1 }}\n\
         diff --git a/docs/development/diff-coverage.md b/docs/development/diff-coverage.md\n\
         --- a/docs/development/diff-coverage.md\n\
         +++ b/docs/development/diff-coverage.md\n\
         @@ -{doc} +{doc} @@\n\
         -Escrita.\n\
         +Somente leitura.\n\
         diff --git a/.pinker/changes/pr-438.yaml b/.pinker/changes/pr-438.yaml\n\
         --- a/.pinker/changes/pr-438.yaml\n\
         +++ b/.pinker/changes/pr-438.yaml\n\
         @@ -1 +1 @@\n\
         -schema: 0\n\
         +schema: 1\n",
        code = code_line(),
        doc = doc_line(),
    )
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_success(output: &Output) {
    assert_eq!(code(output), 0, "stderr={}", stderr(output));
}

fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(root: &Path, current: &Path, out: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(&path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// @pinker-nav:start evidencia.diff-cobertura.cli
// @pinker-nav:domain diff-coverage
// @pinker-nav:layer evidencia
// @pinker-nav:test-for pinker_v0::diff_coverage::analyze
// @pinker-nav:test-for pinker_v0::diff_coverage::CoverageReport
// @pinker-nav:summary Prova a CLI de cobertura com regiões, docs, snapshots, projeções documentais e testes explícitos; cobre UNKNOWN, deleção pura, malformed input, catálogos, determinismo entre roots, read-only e ausência estrutural de Git, rede, subprocessos ou heurística no derivador.

#[test]
fn relaciona_todas_as_superficies_por_autoridades_explicitas() {
    let repo = fixture("known");
    let diff = complete_diff();
    let output = coverage(repo.path(), &diff, true);
    assert_success(&output);
    assert!(output.stderr.is_empty());
    let json = stdout(&output);
    assert!(json.starts_with("{\"schema\":1,\"source\":\"stdin-unified-diff\""));
    for expected in [
        "\"path\":\"src/alvo.rs\"",
        "\"id\":\"codigo.alvo\"",
        "\"id\":\"development.diff-coverage.contract\"",
        "\"id\":\"fixture-current\",\"kind\":\"navigation-snapshot\"",
        "\"region\":\"evidencia.alvo\"",
        "\"id\":\"state\",\"kind\":\"documentation\"",
        "\"source\":\"code-catalog\"",
        "\"source\":\"symbol-index\"",
        "\"source\":\"projection-store\"",
        "\"source\":\"doc-projection-config\"",
    ] {
        assert!(json.contains(expected), "ausente {expected}: {json}");
    }

    let human = coverage(repo.path(), &diff, false);
    assert_success(&human);
    let human = stdout(&human);
    for expected in [
        "regiões: KNOWN",
        "documentos: KNOWN",
        "projeções: KNOWN",
        "testes: KNOWN",
        "codigo.alvo",
        "fixture-current",
        "evidencia.alvo",
    ] {
        assert!(human.contains(expected), "ausente {expected}: {human}");
    }
}

#[test]
fn arquivo_sem_metadados_e_delecao_pura_publicam_unknown_e_avisos() {
    let repo = fixture("unknown");
    write(repo.path(), "misc.txt", "linha\n");
    let added = "diff --git a/misc.txt b/misc.txt\n--- a/misc.txt\n+++ b/misc.txt\n@@ -1 +1 @@\n-antiga\n+linha\n";
    let output = coverage(repo.path(), added, true);
    assert_success(&output);
    let json = stdout(&output);
    assert!(json.contains("\"regions\":{\"status\":\"UNKNOWN\""));
    assert!(json.contains("W-DIFF-REGION-UNKNOWN"));
    assert!(json.contains("W-DIFF-DOCUMENT-UNKNOWN"));
    assert!(json.contains("W-DIFF-TEST-UNKNOWN"));

    let line = code_line();
    let deleted = format!(
        "--- a/src/alvo.rs\n+++ b/src/alvo.rs\n@@ -{line} +{next},0 @@\n-fn removida() {{}}\n",
        next = line.saturating_sub(1)
    );
    let output = coverage(repo.path(), &deleted, true);
    assert_success(&output);
    let json = stdout(&output);
    assert!(json.contains("W-DIFF-DELETION-ONLY"));
    assert!(json.contains("\"changed_lines\":[]"));
    assert!(json.contains("\"regions\":{\"status\":\"UNKNOWN\""));
}

#[test]
fn entrada_malformada_path_inseguro_e_uso_invalido_falham_fechado() {
    let repo = fixture("negative");
    let malformed = coverage(
        repo.path(),
        "--- a/src/alvo.rs\n+++ b/src/alvo.rs\n@@ -1,2 +1,2 @@\n-a\n+b\n",
        false,
    );
    assert_eq!(code(&malformed), 6);
    assert!(stderr(&malformed).contains("E-DIFF-FORMAT"));

    let traversal = coverage(
        repo.path(),
        "--- a/src/alvo.rs\n+++ b/../fora.rs\n@@ -1 +1 @@\n-a\n+b\n",
        false,
    );
    assert_eq!(code(&traversal), 6);
    assert!(stderr(&traversal).contains("E-DIFF-PATH"));

    let positional = run(repo.path(), &["nav", "cobertura-diff", "arquivo.diff"], "");
    assert_eq!(code(&positional), 2);
    assert!(stderr(&positional).contains("não aceita argumentos posicionais"));
}

#[test]
fn catalogo_de_codigo_invalido_falha_e_docs_ausentes_ficam_unavailable() {
    let invalid = fixture("invalid_catalog");
    write(invalid.path(), "src/navigation.jsonl", "{invalid\n");
    let output = coverage(invalid.path(), &complete_diff(), true);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("E-NAV-CATALOG"));

    let missing_docs = fixture("missing_docs");
    fs::remove_file(missing_docs.path().join("docs/navigation.jsonl")).unwrap();
    let output = coverage(missing_docs.path(), &complete_diff(), true);
    assert_success(&output);
    assert!(stdout(&output).contains("\"documents\":{\"status\":\"UNAVAILABLE\""));
}

#[test]
fn repeticao_roots_distintos_e_modo_humano_sao_deterministicos() {
    let first = fixture("root_a");
    let second = fixture("root_b");
    let diff = complete_diff();
    let a = stdout(&coverage(first.path(), &diff, true));
    let repeated = stdout(&coverage(first.path(), &diff, true));
    let b = stdout(&coverage(second.path(), &diff, true));
    assert_eq!(a, repeated);
    assert_eq!(a, b);
    assert!(!a.contains(first.path().to_string_lossy().as_ref()));
    assert!(!a.contains(second.path().to_string_lossy().as_ref()));

    let human_a = stdout(&coverage(first.path(), &diff, false));
    let human_b = stdout(&coverage(second.path(), &diff, false));
    assert_eq!(human_a, human_b);
}

#[test]
fn consulta_nao_escreve_nem_altera_mtime() {
    let repo = fixture("read_only");
    let before = snapshot(repo.path());
    assert_success(&coverage(repo.path(), &complete_diff(), false));
    assert_success(&coverage(repo.path(), &complete_diff(), true));
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn derivador_nao_contem_git_rede_subprocesso_escrita_ou_vocabulario_heuristico() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/diff_coverage.rs"))
            .unwrap();
    for forbidden in [
        "std::process",
        "Command::new",
        "TcpStream",
        "reqwest",
        "fs::write",
        "File::create",
        "git diff",
    ] {
        assert!(
            !source.contains(forbidden),
            "derivador contém fronteira proibida: {forbidden}"
        );
    }
}
// @pinker-nav:end evidencia.diff-cobertura.cli
