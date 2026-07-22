//! Testes de ponta a ponta do catálogo documental (Trama Pinker — Etapa 2).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.trama.doc-catalog.fixture-config
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Agrupa a configuração documental e os dois documentos sintéticos usados pela suíte de catálogo documental.
const DOC_TOML: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

const PORTAL: &str = "---\npinker-doc: 1\nid: rosa\ndomain: rosa\nkind: portal\nstatus: active\nparent: atlas\ncanonical_for:\n  - rosa.territory\n---\n\n# Rosa\n\nPortal.\n";

const CORE: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\ntags: [rosa, identidade]\nsummary: Identidade de Rosa.\n-->\n## Identidade\n\nRosa e guia.\n<!-- @pinker-doc:end rosa.identity -->\n";
// @pinker-nav:end evidencia.trama.doc-catalog.fixture-config

// @pinker-nav:start evidencia.trama.doc-catalog.process-support
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fornece repositório temporário, escrita de arquivos, montagem da fixture e execução de pink doc para os quatro testes.
fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_cat_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
    write(root, "docs/rosa/README.md", PORTAL);
    write(root, "docs/rosa/core.md", CORE);
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
// @pinker-nav:end evidencia.trama.doc-catalog.process-support

// @pinker-nav:start evidencia.trama.doc-catalog.sync-verify
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa sincronização gerando o catálogo documental e verificação aprovando a fixture válida com a seção rosa.identity.
#[test]
fn sincronizar_gera_catalogo_e_verificar_aprova() {
    let root = temp_repo("sync");
    fixture(&root);

    let sync = run(&root, &["sincronizar"]);
    assert!(sync.status.success());

    let catalog = fs::read_to_string(root.join("docs/navigation.jsonl")).unwrap();
    assert!(catalog.contains("\"id\":\"rosa.identity\""), "{catalog}");
    assert!(catalog.contains("\"document\":\"rosa.core\""), "{catalog}");

    let verify = run(&root, &["verificar"]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.doc-catalog.sync-verify

// @pinker-nav:start evidencia.trama.doc-catalog.stale-catalog
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa pink doc verificar rejeitando catálogo documental ausente ou desatualizado com E-DOC-VERIFY.
#[test]
fn verificar_falha_quando_catalogo_desatualizado() {
    let root = temp_repo("drift");
    fixture(&root);
    // catálogo ausente => desatualizado
    let verify = run(&root, &["verificar"]);
    assert!(!verify.status.success());
    assert!(String::from_utf8_lossy(&verify.stderr).contains("E-DOC-VERIFY"));
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.doc-catalog.stale-catalog

// @pinker-nav:start evidencia.trama.doc-catalog.show-extraction
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa pink doc mostrar extraindo somente a seção rosa.identity sem vazar conteúdo do documento portal.
#[test]
fn mostrar_extrai_apenas_a_secao() {
    let root = temp_repo("mostrar");
    fixture(&root);
    run(&root, &["sincronizar"]);

    let out = run(&root, &["mostrar", "rosa.identity"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("## Identidade"), "{stdout}");
    assert!(
        !stdout.contains("Portal."),
        "não deve vazar outro documento: {stdout}"
    );
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.doc-catalog.show-extraction

// @pinker-nav:start evidencia.trama.doc-catalog.unbalanced-anchor
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa verificação rejeitando documento sintético com âncora @pinker-doc aberta sem fechamento.
#[test]
fn verificar_detecta_ancora_desbalanceada() {
    let root = temp_repo("unbal");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "docs/rosa/README.md", PORTAL);
    write(
        &root,
        "docs/rosa/core.md",
        "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\n-->\n## X\nsem fim\n",
    );
    run(&root, &["sincronizar"]);
    let verify = run(&root, &["verificar"]);
    assert!(!verify.status.success());
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.doc-catalog.unbalanced-anchor
