//! Testes de ponta a ponta da navegação do código (Trama Pinker — Etapa 3).

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

const SRC: &str = "// @pinker-nav:start cfg.logica.curto-circuito\n// @pinker-nav:domain logica\n// @pinker-nav:layer cfg\n// @pinker-nav:summary Curto-circuito.\nfn curto() {\n    let _x = 1;\n}\n// @pinker-nav:end cfg.logica.curto-circuito\n";

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_navcat_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
    write(root, "src/cfg_ir.rs", SRC);
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("nav")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

#[test]
fn sincronizar_e_verificar_do_codigo() {
    let root = temp_repo("sync");
    fixture(&root);

    let sync = run(&root, &["sincronizar"]);
    assert!(sync.status.success());
    let catalog = fs::read_to_string(root.join("src/navigation.jsonl")).unwrap();
    assert!(
        catalog.contains("\"key\":\"cfg.logica.curto-circuito\""),
        "{catalog}"
    );
    assert!(catalog.contains("\"hash\":\"fnv1a64:"), "{catalog}");

    let verify = run(&root, &["verificar"]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mostrar_extrai_a_regiao() {
    let root = temp_repo("mostrar");
    fixture(&root);
    run(&root, &["sincronizar"]);
    let out = run(&root, &["mostrar", "cfg.logica.curto-circuito"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("fn curto()"), "{stdout}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verificar_falha_quando_marcador_desbalanceado() {
    let root = temp_repo("unbal");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "src/a.rs", "// @pinker-nav:start x.y\nfn a() {}\n");
    run(&root, &["sincronizar"]);
    let verify = run(&root, &["verificar"]);
    assert!(!verify.status.success());
    assert!(String::from_utf8_lossy(&verify.stderr).contains("E-NAV-VERIFY"));
    fs::remove_dir_all(root).unwrap();
}
