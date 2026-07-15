//! Trama Pinker — CI permanente somente leitura (§14; §20 item 29).
//!
//! Estes testes leem o workflow versionado e garantem que ele nunca escreve:
//! permissões mínimas, sem push, sem commit, sem reconstrução de Base64, e que
//! o runner temporário da tentativa anterior foi removido.

use std::path::PathBuf;

fn workflow_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows")
}

#[test]
fn runner_temporario_foi_removido() {
    let path = workflow_dir().join("trama-temporary-runner.yml");
    assert!(!path.exists(), "o runner temporário não deve existir");
}

#[test]
fn workflow_permanente_e_somente_leitura() {
    let path = workflow_dir().join("trama.yml");
    let text = std::fs::read_to_string(path).expect("workflow permanente presente");

    // Permissões mínimas de leitura declaradas.
    assert!(
        text.contains("contents: read"),
        "deve declarar contents: read"
    );
    // Nunca eleva para escrita.
    assert!(
        !text.contains("contents: write"),
        "não pode ter contents: write"
    );
    // Roda em pull_request.
    assert!(text.contains("pull_request"), "deve rodar em pull_request");
    // Executa make ci.
    assert!(text.contains("make ci"), "deve executar make ci");

    // Nunca faz push, commit, nem reconstrói patches Base64.
    for forbidden in ["git push", "git commit", "base64 -d", "upload-artifact"] {
        assert!(
            !text.contains(forbidden),
            "workflow permanente não pode conter '{forbidden}'"
        );
    }
}
