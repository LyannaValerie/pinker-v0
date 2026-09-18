//! Trama Pinker — acervo histórico de manifestos: leitura, validação real de
//! schema e detecção de adulteração (§10, §11; §20 itens 15, 16, 17, 18).
//!
//! POT/LPT: AUTHORITY #698
//! POT/LPT: INVARIANT o acervo é histórico e finito; `pink doc` lê e valida os
//! manifestos aceitos e nunca cria, reescreve ou sintetiza um manifesto novo.

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
// @pinker-nav:summary Helpers que montam manifestos aceitos, repositórios temporários, arquivos, processos doc e configuração dos testes.
fn manifest(pr: u64, title: &str, kind: &str, status: &str) -> String {
    format!(
        "schema: 1\nsource:\n  type: github-pr\n  number: {pr}\n  repository: LyannaValerie/pinker-v0\nkind: {kind}\ntitle: {title}\narea:\n  - language.result\nstatus: {status}\n"
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

fn doc(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("doc")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn setup(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
}

/// Grava um manifesto aceito e sincroniza o histórico mecânico derivado.
fn acervo(root: &Path, pr: u64, content: &str) {
    setup(root);
    write(root, &format!(".pinker/changes/pr-{pr}.yaml"), content);
    let sync = doc(root, &["sincronizar"]);
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );
}
// @pinker-nav:end evidencia.trama.manifest.process-support

// @pinker-nav:start evidencia.trama.manifest.historical-read
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de que os manifestos aceitos permanecem legíveis, verificáveis e byte a byte intactos, e de que sincronizar não inventa manifesto para PR nenhum.
#[test]
fn manifesto_aceito_permanece_legivel_e_verificavel() {
    let root = temp_repo("read");
    let original = manifest(341, "Título real", "phase", "completed");
    acervo(&root, 341, &original);

    let verify = doc(&root, &["verificar"]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    // Bytes do manifesto aceito preservados.
    let on_disk = fs::read_to_string(root.join(".pinker/changes/pr-341.yaml")).unwrap();
    assert_eq!(on_disk, original);

    // Histórico mecânico derivado do acervo, e só dele.
    let ledger = fs::read_to_string(root.join(".pinker/changes/index.jsonl")).unwrap();
    assert!(ledger.contains("\"pr\":341"), "{ledger}");
    assert_eq!(ledger.lines().count(), 1, "{ledger}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sincronizar_nao_sintetiza_manifesto_novo() {
    let root = temp_repo("no_synth");
    acervo(
        &root,
        341,
        &manifest(341, "Título real", "phase", "completed"),
    );

    // Uma segunda sincronização não pode fabricar manifesto para PR algum.
    assert!(doc(&root, &["sincronizar"]).status.success());
    let mut nomes: Vec<String> = fs::read_dir(root.join(".pinker/changes"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    nomes.sort();
    assert_eq!(nomes, vec!["index.jsonl", "pr-341.yaml"], "{nomes:?}");

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.manifest.historical-read

// @pinker-nav:start evidencia.trama.manifest.tamper-detection
// @pinker-nav:domain development
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidência de que adulterar o payload aceito, o histórico mecânico ou o marco do acervo é detectado por pink doc verificar com E-DOC-VERIFY.
#[test]
fn payload_adulterado_e_detectado() {
    let root = temp_repo("tamper_payload");
    acervo(
        &root,
        341,
        &manifest(341, "Título real", "phase", "completed"),
    );

    write(
        &root,
        ".pinker/changes/pr-341.yaml",
        &manifest(341, "Título real", "banana", "completed"),
    );
    let verify = doc(&root, &["verificar"]);
    assert!(!verify.status.success());
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("E-DOC-VERIFY"), "{stderr}");
    assert!(stderr.contains("E-CHANGE-SCHEMA"), "{stderr}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn historico_mecanico_adulterado_e_detectado() {
    let root = temp_repo("tamper_ledger");
    acervo(
        &root,
        341,
        &manifest(341, "Título real", "phase", "completed"),
    );

    write(&root, ".pinker/changes/index.jsonl", "{\"schema\":1}\n");
    let verify = doc(&root, &["verificar"]);
    assert!(!verify.status.success());
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("dessincronizado"), "{stderr}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn numero_interno_divergente_e_detectado() {
    let root = temp_repo("tamper_number");
    setup(&root);
    write(
        &root,
        ".pinker/changes/pr-341.yaml",
        &manifest(342, "Título real", "phase", "completed"),
    );
    let verify = doc(&root, &["verificar"]);
    assert!(!verify.status.success());
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("E-CHANGE-NUMBER"), "{stderr}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manifesto_anterior_ao_marco_e_recusado_na_leitura() {
    let root = temp_repo("baseline");
    setup(&root);
    write(
        &root,
        ".pinker/changes/pr-329.yaml",
        &manifest(329, "Antigo", "phase", "completed"),
    );
    let verify = doc(&root, &["verificar"]);
    assert!(!verify.status.success());
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("E-DOC-BASELINE"), "{stderr}");
    assert!(stderr.contains("manifesto pr-329"), "{stderr}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn campo_desconhecido_e_detectado() {
    let root = temp_repo("tamper_field");
    setup(&root);
    let mut corrompido = manifest(341, "Título real", "phase", "completed");
    corrompido.push_str("banana: 42\n");
    write(&root, ".pinker/changes/pr-341.yaml", &corrompido);
    let verify = doc(&root, &["verificar"]);
    assert!(!verify.status.success());
    let stderr = String::from_utf8_lossy(&verify.stderr);
    assert!(stderr.contains("E-CHANGE-SCHEMA"), "{stderr}");

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.manifest.tamper-detection
