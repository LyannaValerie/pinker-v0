//! Corte da autoria universal de `pinker-change` (#698).
//!
//! POT/LPT: AUTHORITY #698
//! POT/LPT: INVARIANT NO_NEW_MANIFEST_REQUIRED != DELETE_HISTORICAL_LEDGER
//! POT/LPT: INVARIANT DOCUMENT_NAVIGATION != CONTINUOUS_PR_LEDGER_AUTHORING
//! POT/LPT: INVARIANT NONEXISTENT_WORKFLOW != FAILED_GATE
//!
//! Estes controles olham o repositório real: a main já acumulou merges muito
//! além do intervalo histórico preservado, e nenhum deles carrega manifesto.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn pink(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root())
        .output()
        .expect("executar pink")
}

fn manifest_prs() -> BTreeSet<u64> {
    fs::read_dir(root().join(".pinker/changes"))
        .expect("listar acervo")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_prefix("pr-")?
                .strip_suffix(".yaml")?
                .parse::<u64>()
                .ok()
        })
        .collect()
}

/// Maior PR de merge alcançável na main atual.
fn maior_pr_na_main() -> u64 {
    let output = Command::new("git")
        .arg("-C")
        .arg(root())
        .args(["log", "--merges", "--format=%s", "HEAD"])
        .output()
        .expect("executar git log local");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("git log UTF-8")
        .lines()
        .filter_map(|subject| {
            let rest = subject.strip_prefix("Merge pull request #")?;
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            rest[..digits].parse::<u64>().ok()
        })
        .max()
        .expect("a main precisa ter merges de PR")
}

/// M4 — a verificação documental corrente aprova um repositório cuja main tem
/// muitos PRs sem manifesto algum.
#[test]
fn doc_verificar_aprova_main_sem_manifesto_novo() {
    let manifests = manifest_prs();
    let ultimo_manifesto = *manifests.iter().max().expect("acervo não vazio");
    let ultimo_pr = maior_pr_na_main();
    assert!(
        ultimo_pr > ultimo_manifesto,
        "o controle exige main além do acervo: main #{ultimo_pr}, acervo #{ultimo_manifesto}"
    );

    let output = pink(&["doc", "verificar"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // O histórico mecânico deriva do acervo e só dele.
    let ledger = fs::read_to_string(root().join(".pinker/changes/index.jsonl")).expect("ledger");
    assert_eq!(ledger.lines().count(), manifests.len());
}

/// M5 — a rota documental não depende de manifesto nenhum.
#[test]
fn doc_rota_responde_sem_manifesto_novo() {
    let output = pink(&["doc", "rota", "trama"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).is_empty());
}

/// M8 — o acervo histórico continua legível e completo depois do corte.
#[test]
fn acervo_historico_permanece_intacto() {
    let manifests = manifest_prs();
    assert!(
        manifests.len() >= 21,
        "o acervo preservado encolheu: {}",
        manifests.len()
    );
    let exceptions =
        fs::read_to_string(root().join(".pinker/changes/historical-exceptions-v1.yaml"))
            .expect("exceções históricas preservadas");
    assert!(exceptions.contains("baseline_pr: 330"), "{exceptions}");
    assert!(
        exceptions.contains("cutover_merge_sha: 1df2a6afff423bc7564e7322880e24af683f6089"),
        "{exceptions}"
    );
}

/// M10 — a governança versionada segue os workflows realmente configurados e
/// não exige mais um portão remoto dedicado da Trama.
#[test]
fn governanca_nao_exige_portao_trama_remoto() {
    let agents = fs::read_to_string(root().join("AGENTS.md")).expect("AGENTS.md");
    for obsoleto in ["Trama remotos", "Trama remoto", "REMOTE_TRAMA"] {
        assert!(
            !agents.contains(obsoleto),
            "AGENTS.md ainda exige '{obsoleto}'"
        );
    }
    assert!(
        agents.contains(".github/workflows/"),
        "AGENTS.md precisa apontar os workflows realmente configurados"
    );
    assert!(
        agents.contains("Workflow inexistente não é"),
        "AGENTS.md precisa distinguir workflow inexistente de portão reprovado"
    );
}

/// M11 — o template e o guia de contribuição continuam pedindo corpo estruturado
/// de PR, sem pedir bloco `pinker-change`.
#[test]
fn template_e_contribuicao_nao_exigem_bloco() {
    let template = fs::read_to_string(root().join(".github/pull_request_template.md"))
        .expect("template de PR");
    let contributing = fs::read_to_string(root().join("CONTRIBUTING.md")).expect("CONTRIBUTING.md");

    for proibido in ["```pinker-change", "importar-pr", "<preencher-"] {
        assert!(!template.contains(proibido), "template cita '{proibido}'");
        assert!(
            !contributing.contains(proibido),
            "CONTRIBUTING cita '{proibido}'"
        );
    }

    // O corpo de PR continua governado: registro, validação e retenção.
    for exigido in ["## Registro mínimo", "## Validação"] {
        assert!(
            template.contains(exigido),
            "o template perdeu a seção '{exigido}'"
        );
    }
}
