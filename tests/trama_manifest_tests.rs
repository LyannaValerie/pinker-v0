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
    import_with_mode(root, pr, body_rel, false)
}

fn import_with_mode(root: &Path, pr: &str, body_rel: &str, check: bool) -> std::process::Output {
    let body_path = root.join(body_rel).to_string_lossy().to_string();
    let mut command = Command::new(env!("CARGO_BIN_EXE_pink"));
    command.args(["doc", "importar-pr", pr, "--corpo", &body_path]);
    if check {
        command.arg("--check");
    }
    command
        .arg("--repo")
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

#[test]
fn pr411_legado_e_bloco_atual_sao_idempotentes_sem_reescrita() {
    const LEGACY: &str = "schema: 1\nsource:\n  type: github-pr\n  number: 411\n  repository: LyannaValerie/pinker-v0\nkind: phase\nphase: 248\nblock: 20\ntitle: Uniões estruturais tagged\narea:\n  - language.inline-assembly\n  - language.union-types\n  - runtime.integer-semantics\n  - runtime.public-memory\n  - development.test-integrity\nstatus: completed\nupdates:\n  state: true\n  history: true\n  roadmap: true\nvalidation:\n  required:\n    - make ci\n";
    const CURRENT_BODY: &str = "```pinker-change\nschema: 1\nkind: phase\nphase: 248\nblock: 20\ntitle: Uniões estruturais tagged\nstatus: completed\narea:\n  - language.inline-assembly\n  - language.union-types\n  - runtime.integer-semantics\n  - runtime.public-memory\n  - development.test-integrity\nupdates:\n  state: true\n  history: true\n  roadmap: true\nvalidation:\n  required:\n    - make ci\n```\n";

    let root = temp_repo("pr411_legacy");
    setup(&root);
    write(&root, "body.md", CURRENT_BODY);
    write(&root, ".pinker/changes/pr-411.yaml", LEGACY);
    let before = fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap();

    let checked = import_with_mode(&root, "411", "body.md", true);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(
        before,
        fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap()
    );

    let mutable = import_with_mode(&root, "411", "body.md", false);
    assert!(
        mutable.status.success(),
        "{}",
        String::from_utf8_lossy(&mutable.stderr)
    );
    assert_eq!(
        before,
        fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn toda_mudanca_estrutural_permanece_imutavel() {
    const BODY: &str = "```pinker-change\nschema: 1\nkind: phase\nphase: 248\nblock: 20\ntitle: Entrega canônica\narea:\n  - language.alpha\n  - runtime.beta\nstatus: completed\nupdates:\n  state: true\n  history: false\nsections:\n  implemented:\n    - feature.ready\n  pending_remove:\n    - feature.legacy\nvalidation:\n  required:\n    - make ci\n    - cargo test\n```\n";
    const EXISTING: &str = "schema: 1\nsource:\n  type: github-pr\n  number: 411\n  repository: LyannaValerie/pinker-v0\nkind: phase\nphase: 248\nblock: 20\ntitle: Entrega canônica\narea:\n  - language.alpha\n  - runtime.beta\nstatus: completed\nupdates:\n  state: true\n  history: false\nsections:\n  implemented:\n    - feature.ready\n  pending_remove:\n    - feature.legacy\nvalidation:\n  required:\n    - make ci\n    - cargo test\n";
    let cases = [
        ("schema", EXISTING.replacen("schema: 1", "schema: 2", 1)),
        (
            "repository",
            EXISTING.replace(
                "repository: LyannaValerie/pinker-v0",
                "repository: LyannaValerie/outro",
            ),
        ),
        ("number", EXISTING.replace("number: 411", "number: 412")),
        (
            "kind",
            EXISTING.replace("\nkind: phase\n", "\nkind: hotfix\n"),
        ),
        ("phase", EXISTING.replace("phase: 248", "phase: 247")),
        ("block", EXISTING.replace("block: 20", "block: 19")),
        (
            "title",
            EXISTING.replace("title: Entrega canônica", "title: Entrega alterada"),
        ),
        (
            "area",
            EXISTING.replace("  - runtime.beta", "  - runtime.gamma"),
        ),
        (
            "area_order",
            EXISTING.replace(
                "  - language.alpha\n  - runtime.beta",
                "  - runtime.beta\n  - language.alpha",
            ),
        ),
        (
            "status",
            EXISTING.replace("status: completed", "status: planned"),
        ),
        (
            "updates",
            EXISTING.replace("  history: false", "  history: true"),
        ),
        (
            "sections",
            EXISTING.replace("  - feature.ready", "  - feature.changed"),
        ),
        (
            "validation",
            EXISTING.replace("    - cargo test", "    - cargo check"),
        ),
    ];

    for (name, existing) in cases {
        let root = temp_repo(name);
        setup(&root);
        write(&root, "body.md", BODY);
        write(&root, ".pinker/changes/pr-411.yaml", &existing);
        let before = fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap();
        let out = import_with_mode(&root, "411", "body.md", false);
        assert_eq!(out.status.code(), Some(5), "{name}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-IMMUTABLE"),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            before,
            fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap(),
            "{name}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn manifesto_existente_invalido_falha_fechado_sem_reescrita() {
    const BODY: &str = "```pinker-change\nschema: 1\nkind: phase\nphase: 248\nblock: 20\ntitle: Entrega canônica\narea:\n  - language.alpha\nstatus: completed\n```\n";
    const VALID: &str = "schema: 1\nsource:\n  type: github-pr\n  number: 411\n  repository: LyannaValerie/pinker-v0\nkind: phase\nphase: 248\nblock: 20\ntitle: Entrega canônica\narea:\n  - language.alpha\nstatus: completed\n";
    let cases = [
        ("malformed", "schema: 1\nsource\n".to_string()),
        (
            "invalid_escape",
            VALID.replace("title: Entrega canônica", "title: \"Entrega\\q\""),
        ),
        (
            "unknown",
            VALID.replace("  repository:", "  unknown: value\n  repository:"),
        ),
        (
            "source",
            VALID.replace("type: github-pr", "type: delegated-input"),
        ),
        ("schema", VALID.replacen("schema: 1", "schema: 9", 1)),
        ("filename", VALID.replace("number: 411", "number: 412")),
    ];

    for (name, existing) in cases {
        let root = temp_repo(name);
        setup(&root);
        write(&root, "body.md", BODY);
        write(&root, ".pinker/changes/pr-411.yaml", &existing);
        let before = fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap();
        let out = import_with_mode(&root, "411", "body.md", false);
        assert_eq!(out.status.code(), Some(5), "{name}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("E-CHANGE-IMMUTABLE"));
        assert_eq!(
            before,
            fs::read(root.join(".pinker/changes/pr-411.yaml")).unwrap(),
            "{name}"
        );
        fs::remove_dir_all(root).unwrap();
    }
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
