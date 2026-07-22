//! Trama Pinker — sincronização segura (§8, §20 itens 12, 13, 14).
//!
//! `sincronizar` valida a árvore inteira antes de escrever; uma fonte inválida
//! nunca sobrescreve o último catálogo válido; a substituição é atômica.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.trama.sync.fixture-config
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Configurações e conteúdos válidos e inválidos usados pelas fixtures de sincronização.
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

const PORTAL: &str = "---\npinker-doc: 1\nid: rosa\ndomain: rosa\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# Rosa\n\nPortal.\n";

const CORE_OK: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\ntags: [rosa]\nsummary: Identidade.\n-->\n## Identidade\nRosa e guia.\n<!-- @pinker-doc:end rosa.identity -->\n";

// Âncora aberta sem fechamento: árvore inválida.
const CORE_BAD: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\n-->\n## Identidade\nsem fim\n";
// @pinker-nav:end evidencia.trama.sync.fixture-config

// @pinker-nav:start evidencia.trama.sync.process-support
// @pinker-nav:domain development
// @pinker-nav:layer support
// @pinker-nav:summary Helpers para repositórios temporários, escrita de fixtures e execução do subcomando doc.
fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_sync_{name}_{now}"))
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
// @pinker-nav:end evidencia.trama.sync.process-support

// @pinker-nav:start evidencia.trama.sync.invalid-source
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de que uma fonte inválida impede a sincronização e a criação do catálogo.
#[test]
fn sincronizar_recusa_fonte_invalida() {
    let root = temp_repo("reject_invalid");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "docs/rosa/README.md", PORTAL);
    write(&root, "docs/rosa/core.md", CORE_BAD);

    let out = doc(&root, &["sincronizar"]);
    assert!(
        !out.status.success(),
        "árvore inválida não deve sincronizar"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-SYNC"));
    // Nenhum catálogo foi materializado.
    assert!(!root.join("docs/navigation.jsonl").exists());

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.sync.invalid-source

// @pinker-nav:start evidencia.trama.sync.preserve-last-valid
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de preservação do último catálogo válido após falha de sincronização.
#[test]
fn ultimo_catalogo_valido_preservado_apos_falha() {
    let root = temp_repo("preserve");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "docs/rosa/README.md", PORTAL);
    write(&root, "docs/rosa/core.md", CORE_OK);

    // Primeiro sync válido.
    assert!(doc(&root, &["sincronizar"]).status.success());
    let valid = fs::read_to_string(root.join("docs/navigation.jsonl")).unwrap();
    assert!(valid.contains("rosa.identity"));

    // Torna a árvore inválida e tenta sincronizar de novo.
    write(&root, "docs/rosa/core.md", CORE_BAD);
    let out = doc(&root, &["sincronizar"]);
    assert!(!out.status.success());

    // O catálogo em disco continua sendo o último válido.
    let after = fs::read_to_string(root.join("docs/navigation.jsonl")).unwrap();
    assert_eq!(valid, after, "catálogo válido deve ser preservado");

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.sync.preserve-last-valid

// @pinker-nav:start evidencia.trama.sync.atomic-write
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de escrita atômica do catálogo sem arquivo temporário residual.
#[test]
fn escrita_e_atomica_sem_temporario_residual() {
    let root = temp_repo("atomic");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "docs/rosa/README.md", PORTAL);
    write(&root, "docs/rosa/core.md", CORE_OK);

    assert!(doc(&root, &["sincronizar"]).status.success());
    assert!(root.join("docs/navigation.jsonl").exists());
    // Nenhum arquivo temporário deve sobrar da substituição atômica.
    assert!(!root.join("docs/navigation.jsonl.tmp").exists());

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.sync.atomic-write
