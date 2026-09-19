//! Trama Pinker — CI permanente somente leitura (§14; §20 item 29).
//!
//! Estes testes leem os workflows versionados e garantem que nenhum escreve:
//! permissões mínimas, sem push, sem commit, sem reconstrução de Base64, e que
//! nem o runner temporário nem o portão de autoria de `pinker-change` voltam.
//!
//! POT/LPT: AUTHORITY #698
//! POT/LPT: INVARIANT nenhum workflow exige bloco `pinker-change` de um PR.
//! POT/LPT: INVARIANT workflow inexistente não é portão remoto reprovado.

use std::path::PathBuf;

// @pinker-nav:start evidence.trama.ci.workflow-path
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary The workflow_dir helper resolves exclusively .github/workflows from CARGO_MANIFEST_DIR for the permanent CI evidence below.
fn workflow_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows")
}
// @pinker-nav:end evidence.trama.ci.workflow-path

// @pinker-nav:start evidence.trama.ci.temporary-runner
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary Negative evidence that the temporary workflow trama-temporary-runner.yml was removed from the versioned tree.
#[test]
fn runner_temporario_foi_removido() {
    let path = workflow_dir().join("trama-temporary-runner.yml");
    assert!(!path.exists(), "o runner temporário não deve existir");
}
// @pinker-nav:end evidence.trama.ci.temporary-runner

// @pinker-nav:start evidence.trama.ci.readonly-workflow
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary Textual inspection of the permanent CI workflow: contents read, pull_request trigger, make ci, and the absence of push, commit, Base64 or artifact upload.
#[test]
fn workflow_permanente_e_somente_leitura() {
    let path = workflow_dir().join("ci.yml");
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
// @pinker-nav:end evidence.trama.ci.readonly-workflow

// @pinker-nav:start evidence.trama.ci.change-authoring-retired
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary Contract of the #698 cutover: the dedicated authoring workflow was retired and no remaining workflow requires a pinker-change block, validates a PR body or calls importar-pr.
/// O workflow dedicado de autoria não existe mais.
#[test]
fn workflow_de_autoria_foi_retirado() {
    let path = workflow_dir().join("trama.yml");
    assert!(
        !path.exists(),
        "o workflow dedicado de autoria de pinker-change não deve existir"
    );
}

/// Nenhum workflow remanescente cobra o bloco de nenhum PR.
#[test]
fn nenhum_workflow_exige_bloco_pinker_change() {
    let dir = workflow_dir();
    let mut checked = 0usize;
    for entry in std::fs::read_dir(dir)
        .expect("diretório de workflows presente")
        .flatten()
    {
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml") | Some("yaml")
        ) {
            continue;
        }
        checked += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("workflow legível");
        for forbidden in [
            "pinker-change",
            "importar-pr",
            "E-CHANGE-BLOCK",
            "pull_request.body",
            "PR_BODY",
        ] {
            assert!(
                !text.contains(forbidden),
                "workflow '{name}' reintroduz a cobrança do bloco: '{forbidden}'"
            );
        }
    }
    assert!(checked >= 1, "deve haver ao menos um workflow permanente");
}
// @pinker-nav:end evidence.trama.ci.change-authoring-retired

// @pinker-nav:start evidence.trama.ci.temporary-artifacts
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary Evidence of the absence of the temporary attempt's artifacts and of the retired authoring workflow, plus a recursive scan for any remaining .b64 package.
/// Nenhum arquivo da tentativa temporária pode voltar à árvore.
#[test]
fn artefatos_temporarios_nao_existem() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        ".github/workflows/trama-temporary-runner.yml",
        ".github/workflows/trama-completion.yml",
        ".github/workflows/trama.yml",
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
// @pinker-nav:end evidence.trama.ci.temporary-artifacts

// @pinker-nav:start evidence.trama.ci.b64-scan
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary The recursive helper has_b64 ignores .git and target and detects files with a b64 extension in the whole remaining tree.
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
// @pinker-nav:end evidence.trama.ci.b64-scan

// @pinker-nav:start evidence.trama.ci.all-workflows-readonly
// @pinker-nav:domain trama
// @pinker-nav:layer evidence
// @pinker-nav:summary Textual audit of every permanent YAML workflow against write permissions, mutating Git, patch reconstruction and experimental branches.
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
// @pinker-nav:end evidence.trama.ci.all-workflows-readonly
