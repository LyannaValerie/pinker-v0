//! Trama Pinker — escala do catálogo (§9; §20 itens 27 e 28).
//!
//! Um catálogo sintético com mais de 5.000 entradas é carregado e consultado
//! sem qualquer limite arbitrário ligado ao tamanho do JSONL.

use std::fmt::Write as _;
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

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_scale_{name}_{now}"))
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

fn synthetic_catalog(entries: usize) -> String {
    let mut out = String::with_capacity(entries * 160);
    // Um documento proprietário.
    out.push_str("{\"schema\":2,\"record\":\"document\",\"id\":\"scale\",\"domain\":\"scale\",\"kind\":\"index\",\"status\":\"active\",\"file\":\"docs/scale.md\"}\n");
    for i in 0..entries {
        let _ = writeln!(
            out,
            "{{\"schema\":2,\"record\":\"section\",\"id\":\"scale.s{i:05}\",\"document\":\"scale\",\"file\":\"docs/scale.md\",\"start\":{s},\"end\":{e},\"title\":\"Secao {i}\",\"tags\":[\"comum\"],\"summary\":\"Entrada sintetica {i}.\"}}",
            s = i * 2 + 1,
            e = i * 2 + 2,
        );
    }
    out
}

#[test]
fn catalogo_com_mais_de_5000_entradas_e_consultavel() {
    let root = temp_repo("scale");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    let catalog = synthetic_catalog(5000);
    // Confirma a escala do arquivo.
    assert!(catalog.lines().count() >= 5001);
    write(&root, "docs/navigation.jsonl", &catalog);

    // Busca por termo comum a todas as entradas: não há limite ligado ao
    // tamanho do JSONL; o corte é apenas o --limite da consulta (padrão 10).
    let out = doc(&root, &["buscar", "comum"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let n = stdout.lines().filter(|l| l.starts_with("scale.s")).count();
    assert_eq!(n, 10, "padrão de buscar é 10 resultados");

    // Uma entrada específica é recuperável por id exato.
    let mostrar = doc(&root, &["mostrar", "scale.s04999", "--json"]);
    // O mostrar tenta abrir a fonte (docs/scale.md) que não existe → exit 5,
    // mas a LOCALIZAÇÃO pelo catálogo funcionou (não é erro de "não encontrado").
    assert_eq!(mostrar.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&mostrar.stderr).contains("E-DOC-SOURCE"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordenacao_determinista_em_escala() {
    let root = temp_repo("order");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "docs/navigation.jsonl", &synthetic_catalog(5000));

    let a = doc(&root, &["buscar", "comum", "--limite", "20"]);
    let b = doc(&root, &["buscar", "comum", "--limite", "20"]);
    assert_eq!(
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout),
        "ordenação deve ser determinística"
    );

    fs::remove_dir_all(root).unwrap();
}
