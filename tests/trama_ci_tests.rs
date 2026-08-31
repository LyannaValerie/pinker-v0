//! Trama Pinker — CI permanente somente leitura (§14; §20 item 29).
//!
//! Estes testes leem o workflow versionado e garantem que ele nunca escreve:
//! permissões mínimas, sem push, sem commit, sem reconstrução de Base64, e que
//! o runner temporário da tentativa anterior foi removido.

use std::path::PathBuf;

// @pinker-nav:start evidencia.trama.ci.workflow-path
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Helper workflow_dir resolve exclusivamente .github/workflows a partir de CARGO_MANIFEST_DIR para as evidências de CI permanente abaixo.
fn workflow_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows")
}
// @pinker-nav:end evidencia.trama.ci.workflow-path

// @pinker-nav:start evidencia.trama.ci.temporary-runner
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência negativa de que o workflow temporário trama-temporary-runner.yml foi removido da árvore versionada.
#[test]
fn runner_temporario_foi_removido() {
    let path = workflow_dir().join("trama-temporary-runner.yml");
    assert!(!path.exists(), "o runner temporário não deve existir");
}
// @pinker-nav:end evidencia.trama.ci.temporary-runner

// @pinker-nav:start evidencia.trama.ci.readonly-workflow
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeção textual do workflow permanente: contents read, gatilho pull_request, make ci e ausência de push, commit, Base64 ou upload de artefato.
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
// @pinker-nav:end evidencia.trama.ci.readonly-workflow

// @pinker-nav:start evidencia.trama.ci.change-validation
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Contrato do gatilho edited e da validação incondicional somente leitura de pinker-change, sem escapes por grep ou nada a validar.
/// Contrato do gatilho e da validação do bloco pinker-change.
#[test]
fn workflow_valida_bloco_incondicionalmente_e_reage_a_edicao() {
    let path = workflow_dir().join("trama.yml");
    let text = std::fs::read_to_string(path).expect("workflow permanente presente");

    // Editar o corpo do PR deve re-executar a validação: `edited` precisa estar
    // entre os tipos de evento (não faz parte dos três eventos padrão).
    assert!(
        text.contains("edited"),
        "o gatilho pull_request deve incluir o tipo 'edited'"
    );

    // O importador roda em modo --check (somente leitura).
    assert!(
        text.contains("importar-pr") && text.contains("--check"),
        "deve validar com `doc importar-pr ... --check`"
    );

    // Não pode mais existir o escape silencioso: PR posterior ao marco sem
    // bloco tem de falhar (E-CHANGE-BLOCK), nunca cair em "nada a validar".
    assert!(
        !text.contains("nada a validar"),
        "o workflow não pode ter o escape 'nada a validar'"
    );
    assert!(
        !text.contains("if grep"),
        "a presença do bloco não deve ser decidida por grep no Bash"
    );
}
// @pinker-nav:end evidencia.trama.ci.change-validation

// @pinker-nav:start evidencia.trama.ci.temporary-artifacts
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência de ausência dos artefatos da tentativa temporária e varredura recursiva por qualquer pacote .b64 restante.
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
// @pinker-nav:end evidencia.trama.ci.temporary-artifacts

// @pinker-nav:start evidencia.trama.ci.b64-scan
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Helper recursivo has_b64 ignora .git e target e detecta arquivos com extensão b64 em toda a árvore restante.
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
// @pinker-nav:end evidencia.trama.ci.b64-scan

// @pinker-nav:start evidencia.trama.ci.all-workflows-readonly
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Auditoria textual de todos os workflows YAML permanentes contra permissões de escrita, Git mutante, reconstrução de patches e branches experimentais.
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
// @pinker-nav:end evidencia.trama.ci.all-workflows-readonly
