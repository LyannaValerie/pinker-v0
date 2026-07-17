//! Trama Pinker — consultas por catálogo, contratos CLI e códigos de saída.
//!
//! Cobre (§20): consulta lendo JSONL sem revarrer Markdown/Rust (1, 2),
//! `mostrar` detectando âncora/hash divergente (3), saída JSON estável (4),
//! limites de resultados (7), códigos de saída (8), catálogo ausente (9),
//! catálogo inválido (10) e ausência de resultados (11).

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

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

const PORTAL: &str = "---\npinker-doc: 1\nid: rosa\ndomain: rosa\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# Rosa\n\nPortal.\n";

const CORE: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\ntags: [rosa, identidade]\naliases:\n  - quem e rosa\nsummary: Identidade de Rosa.\n-->\n## Identidade\n\nRosa e a guia estetica.\n<!-- @pinker-doc:end rosa.identity -->\n";

const SRC: &str = "// @pinker-nav:start rosa.identidade.core\n// @pinker-nav:domain rosa\n// @pinker-nav:layer core\n// @pinker-nav:summary Identidade de Rosa no codigo.\nfn identidade() {\n    let _x = 1;\n}\n// @pinker-nav:end rosa.identidade.core\n";

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_q_{name}_{now}"))
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
    write(root, "src/rosa.rs", SRC);
    write(root, "runtime/pinker_rt/src/lib.rs", "pub fn _rt() {}\n");
    fs::create_dir_all(root.join("tests")).unwrap();
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

fn nav(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("nav")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn code(out: &std::process::Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

#[test]
fn consulta_documental_le_catalogo_sem_revarrer_markdown() {
    let root = temp_repo("doc_catalog_only");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    // Remove as fontes Markdown, preservando apenas o catálogo JSONL.
    fs::remove_file(root.join("docs/rosa/README.md")).unwrap();
    fs::remove_file(root.join("docs/rosa/core.md")).unwrap();

    // `buscar` continua respondendo — prova de que lê o catálogo, não a árvore.
    let out = doc(&root, &["buscar", "identidade"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("rosa.identity"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn consulta_de_codigo_le_catalogo_sem_revarrer_rust() {
    let root = temp_repo("nav_catalog_only");
    fixture(&root);
    assert!(nav(&root, &["sincronizar"]).status.success());

    fs::remove_file(root.join("src/rosa.rs")).unwrap();

    let out = nav(&root, &["buscar", "identidade"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("rosa.identidade.core"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mostrar_detecta_ancora_divergente() {
    let root = temp_repo("doc_drift");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    // Desloca o conteúdo inserindo linhas antes da âncora: o intervalo
    // registrado deixa de ser delimitado pela âncora esperada.
    let drifted = format!("linha nova\noutra linha\n{CORE}");
    write(&root, "docs/rosa/core.md", &drifted);

    let out = doc(&root, &["mostrar", "rosa.identity"]);
    assert_eq!(code(&out), 5, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-SOURCE"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mostrar_detecta_hash_divergente() {
    let root = temp_repo("nav_drift");
    fixture(&root);
    assert!(nav(&root, &["sincronizar"]).status.success());

    // Altera o conteúdo da região mantendo os marcadores: o hash diverge.
    let drifted = SRC.replace("let _x = 1;", "let _x = 999;");
    write(&root, "src/rosa.rs", &drifted);

    let out = nav(&root, &["mostrar", "rosa.identidade.core"]);
    assert_eq!(code(&out), 5, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Hash divergente"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saida_json_e_valida_e_estavel() {
    let root = temp_repo("json");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    let a = doc(&root, &["rota", "quem e rosa", "--json"]);
    let b = doc(&root, &["rota", "quem e rosa", "--json"]);
    assert_eq!(code(&a), 0);
    let sa = String::from_utf8_lossy(&a.stdout);
    let sb = String::from_utf8_lossy(&b.stdout);
    assert_eq!(sa, sb, "saída JSON deve ser estável");
    assert!(sa.contains("\"query\""));
    assert!(sa.contains("\"normalized\""));
    assert!(sa.contains("\"results\""));
    assert!(sa.contains("\"next\""));
    assert!(sa.contains("\"score\""));
    assert!(sa.contains("rosa.identity"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn limite_de_resultados_respeita_contornos() {
    let root = temp_repo("limite");
    fixture(&root);
    // Vários documentos com seções distintas casando o mesmo termo.
    for i in 0..6 {
        let content = format!(
            "---\npinker-doc: 1\nid: rosa.d{i}\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# D{i}\n\n<!-- @pinker-doc:start\nid: rosa.item{i}\ntags: [comum]\nsummary: Item comum {i}.\n-->\n## Item {i}\ncomum\n<!-- @pinker-doc:end rosa.item{i} -->\n"
        );
        write(&root, &format!("docs/rosa/d{i}.md"), &content);
    }
    assert!(doc(&root, &["sincronizar"]).status.success());

    let out = doc(&root, &["buscar", "comum", "--limite", "2"]);
    assert_eq!(code(&out), 0);
    let n = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("rosa.item"))
        .count();
    assert_eq!(n, 2, "limite deve cortar em 2");

    // Limite acima do máximo é fixado em 20 (não erro).
    let out_max = doc(&root, &["buscar", "comum", "--limite", "999"]);
    assert_eq!(code(&out_max), 0);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn catalogo_ausente_falha_com_codigo_3() {
    let root = temp_repo("missing_catalog");
    fixture(&root);
    // Não sincroniza: o catálogo não existe.
    let out = doc(&root, &["mostrar", "rosa.identity"]);
    assert_eq!(code(&out), 3, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn catalogo_invalido_falha_com_codigo_3() {
    let root = temp_repo("invalid_catalog");
    fixture(&root);
    write(&root, "docs/navigation.jsonl", "{isto nao e json valido\n");
    let out = doc(&root, &["buscar", "rosa"]);
    assert_eq!(code(&out), 3, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ausencia_de_resultados_nao_e_sucesso_silencioso() {
    let root = temp_repo("noresult");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());
    let out = doc(&root, &["buscar", "zzzznaoexistemesmo"]);
    assert_eq!(code(&out), 4, "consulta sem resultado deve sair 4");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn uso_invalido_sai_com_codigo_2() {
    let root = temp_repo("usage");
    fixture(&root);
    // Subcomando inexistente.
    let out = doc(&root, &["subcomando-inexistente"]);
    assert_eq!(code(&out), 2);
    fs::remove_dir_all(root).unwrap();
}
