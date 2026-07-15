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

/// Nenhum arquivo da tentativa temporária pode voltar à árvore.
#[test]
fn artefatos_temporarios_nao_existem() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        ".github/workflows/trama-temporary-runner.yml",
        ".github/workflows/trama-completion.yml",
        "scripts/trama_patch_chunks",
        "scripts/apply_trama_completion.py",
        "trama-run-error.log",
    ] {
        assert!(
            !root.join(rel).exists(),
            "artefato temporário não deve existir: {rel}"
        );
    }
    // Nenhum pacote Base64 da tentativa em nenhum lugar da árvore versionada.
    assert!(!has_b64(&root), "nenhum arquivo .b64 deve permanecer");
}

fn has_b64(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Não desce em .git nem em artefatos de build.
        if name == ".git" || name == "target" {
            continue;
        }
        if path.is_dir() {
            if has_b64(&path) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("b64") {
            return true;
        }
    }
    false
}

/// Auditoria de TODOS os workflows permanentes: nenhum pode escrever na branch,
/// fazer push/commit, reconstruir patches Base64 ou fazer checkout fixo de uma
/// branch experimental descartada (§9 da limpeza).
#[test]
fn nenhum_workflow_permanente_escreve_na_branch() {
    let dir = workflow_dir();
    let entries = std::fs::read_dir(dir).expect("diretório de workflows presente");
    let mut checked = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_yaml = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml") | Some("yaml")
        );
        if !is_yaml {
            continue;
        }
        checked += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("workflow legível");
        for forbidden in [
            "contents: write",
            "git push",
            "git commit",
            "base64 -d",
            "gzip -d",
            "apply_trama_completion",
            "trama_patch_chunks",
            "trama/completion-v2",
            "trama/bootstrap-runner",
        ] {
            assert!(
                !text.contains(forbidden),
                "workflow permanente '{name}' não pode conter '{forbidden}'"
            );
        }
    }
    assert!(checked >= 1, "deve haver ao menos um workflow permanente");
}
