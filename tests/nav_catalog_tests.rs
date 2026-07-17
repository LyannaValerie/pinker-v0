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

const RUNTIME_LIB: &str = "// @pinker-nav:start runtime.exemplo.ficticio\n// @pinker-nav:domain exemplo\n// @pinker-nav:layer runtime\n// @pinker-nav:summary Regiao ficticia de teste no runtime.\npub fn exemplo() -> i32 {\n    42\n}\n// @pinker-nav:end runtime.exemplo.ficticio\n";

const FALSO_TEST: &str = "// @pinker-nav:start falso.teste.chave\n// @pinker-nav:domain falso\n// @pinker-nav:layer falso\nfn falso() {}\n// @pinker-nav:end falso.teste.chave\n";

const FALSO_PINK: &str = "-- @pinker-nav:start falso.pink.chave\n-- @pinker-nav:domain falso\n-- @pinker-nav:layer falso\n";

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
    write(root, "runtime/pinker_rt/src/lib.rs", RUNTIME_LIB);
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
    // Fontes fora das raízes controladas: não devem entrar no catálogo,
    // provando que a varredura multi-raiz é controlada, não irrestrita.
    write(&root, "tests/falso.rs", FALSO_TEST);
    write(&root, "apps/falso.pink", FALSO_PINK);

    let sync = run(&root, &["sincronizar"]);
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let catalog = fs::read_to_string(root.join("src/navigation.jsonl")).unwrap();
    assert!(
        catalog.contains("\"key\":\"cfg.logica.curto-circuito\""),
        "{catalog}"
    );
    assert!(catalog.contains("\"hash\":\"fnv1a64:"), "{catalog}");
    // A raiz `runtime/pinker_rt/src` está ativa: a região fictícia entra com
    // o caminho repo-relativo correto.
    assert!(
        catalog.contains("\"key\":\"runtime.exemplo.ficticio\""),
        "{catalog}"
    );
    assert!(
        catalog.contains("\"file\":\"runtime/pinker_rt/src/lib.rs\""),
        "{catalog}"
    );
    // `tests/` e `apps/` continuam desativadas (Onda 6D adia essas raízes).
    assert!(!catalog.contains("falso.teste.chave"), "{catalog}");
    assert!(!catalog.contains("falso.pink.chave"), "{catalog}");

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
fn mostrar_extrai_regiao_do_runtime() {
    let root = temp_repo("mostrar_runtime");
    fixture(&root);
    run(&root, &["sincronizar"]);
    let out = run(&root, &["mostrar", "runtime.exemplo.ficticio"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("pub fn exemplo"), "{stdout}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verificar_falha_quando_marcador_desbalanceado() {
    let root = temp_repo("unbal");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "src/a.rs", "// @pinker-nav:start x.y\nfn a() {}\n");
    // Raiz obrigatória precisa existir para que o desbalanceamento em `src/`
    // seja o único problema detectado (senão a raiz ausente falharia antes).
    fs::create_dir_all(root.join("runtime/pinker_rt/src")).unwrap();
    run(&root, &["sincronizar"]);
    let verify = run(&root, &["verificar"]);
    assert!(!verify.status.success());
    assert!(String::from_utf8_lossy(&verify.stderr).contains("E-NAV-VERIFY"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sincronizar_falha_quando_raiz_obrigatoria_ausente() {
    let root = temp_repo("missingroot");
    write(&root, ".pinker/doc.toml", DOC_TOML);
    write(&root, "src/cfg_ir.rs", SRC);
    // `runtime/pinker_rt/src` não existe: raiz obrigatória ausente.
    let sync = run(&root, &["sincronizar"]);
    assert!(!sync.status.success());
    assert!(String::from_utf8_lossy(&sync.stderr).contains("E-NAV-SCAN"));
    assert!(
        !root.join("src/navigation.jsonl").exists(),
        "catálogo não deveria ser escrito com raiz obrigatória ausente"
    );
    fs::remove_dir_all(root).unwrap();
}
