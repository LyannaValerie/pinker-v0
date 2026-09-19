// A #605 moveu implementação de `src/main.rs` para `src/pink_cli/`. Os
// oráculos abaixo leem o binário inteiro, não um arquivo só (#601, OG-1).
#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidence.tooling.f1.contracts
// @pinker-nav:domain tooling
// @pinker-nav:layer evidence
// @pinker-nav:summary Positive, negative and sensitivity contracts for doctor, nav impacto, the composed preflight and the lifecycle of the published baseline.
// POT/LPT: AUTHORITY #698
// POT/LPT: INVARIANT o preflight não conhece bloco `pinker-change`: a ausência
// dele nunca é achado, bloqueante ou não.

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn temp(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_f1_{name}_{stamp}"))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("executar pink")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout UTF-8")
}

#[test]
fn doctor_json_declara_identidades_e_proxima_acao() {
    let repo = root().to_string_lossy().into_owned();
    let output = run(&["doctor", "--repo", &repo, "--json"]);
    let json = stdout(&output);
    for field in [
        "binary_path",
        "binary_version",
        "binary_commit",
        "repo_root",
        "repo_head",
        "compatibility",
        "navigation_catalog",
        "projection_state",
        "available_subcommands",
        "recommended_next_action",
    ] {
        assert!(json.contains(&format!("\"{field}\":")), "{field}: {json}");
    }
    assert!(json.contains("\"doctor\"") && json.contains("\"verificar\""));
    if json.contains("HISTORICAL_AUTHORITY_UNVERIFIABLE") {
        assert!(!output.status.success());
    } else if option_env!("PINKER_BUILD_COMMIT").is_some() {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn identidade_binaria_json_independe_de_repositorio() {
    let output = run(&["--version-json"]);
    assert!(output.status.success());
    let json = stdout(&output);
    for field in ["binary_path", "binary_version", "binary_commit"] {
        assert!(json.contains(&format!("\"{field}\":")), "{field}: {json}");
    }
}

#[test]
fn doctor_repo_invalido_falha_cedo() {
    let missing = temp("missing");
    let output = run(&["doctor", "--repo", &missing.to_string_lossy(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("E-DOCTOR"));
}

#[test]
fn nav_impacto_diff_vazio_e_known_no_schema_migrado() {
    let repo = root().to_string_lossy().into_owned();
    let output = run(&[
        "nav",
        "impacto",
        "--diff",
        "HEAD...HEAD",
        "--repo",
        &repo,
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout(&output);
    assert!(json.contains("\"schema\":3"), "{json}");
    assert!(json.contains("\"changed_files\":[]"), "{json}");
    assert!(
        json.contains("\"changed_regions\":{\"status\":\"KNOWN\",\"reason\":null,\"items\":[]}")
    );
    assert!(
        !json.contains("projection_overrides_required"),
        "campo de capacidade aposentada ressuscitado: {json}"
    );
}

#[test]
fn nav_impacto_diff_real_expoe_relacoes_correntes_sem_campo_aposentado() {
    let repo = root().to_string_lossy().into_owned();
    let predecessor = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD^"])
        .current_dir(root())
        .output()
        .expect("TEST_SETUP_FAILURE: resolver predecessor local de HEAD");
    assert!(
        predecessor.status.success(),
        "TEST_SETUP_FAILURE: predecessor local de HEAD indisponível: {}",
        String::from_utf8_lossy(&predecessor.stderr)
    );
    let base = String::from_utf8(predecessor.stdout)
        .expect("TEST_SETUP_FAILURE: predecessor de HEAD não é UTF-8");
    let diff_spec = format!("{}...HEAD", base.trim());
    let premise = Command::new("git")
        .args(["diff", "--quiet", &diff_spec, "--"])
        .current_dir(root())
        .output()
        .expect("TEST_SETUP_FAILURE: provar que a fixture possui diff real");
    match premise.status.code() {
        Some(1) => {}
        Some(0) => panic!("TEST_SETUP_FAILURE: fixture {diff_spec} produziu diff vazio"),
        code => panic!(
            "TEST_SETUP_FAILURE: git diff falhou para {diff_spec} com {code:?}: {}",
            String::from_utf8_lossy(&premise.stderr)
        ),
    }
    let output = run(&[
        "nav", "impacto", "--diff", &diff_spec, "--repo", &repo, "--json",
    ]);
    assert!(
        output.status.success(),
        "PINK_BEHAVIOR_FAILURE: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout(&output);
    for field in [
        "changed_files",
        "changed_regions",
        "navigation_entries_affected",
        "projections_affected",
        "catalog_status",
    ] {
        assert!(
            json.contains(&format!("\"{field}\":")),
            "PINK_BEHAVIOR_FAILURE: {field}: {json}"
        );
    }
    assert!(
        !json.contains("\"changed_files\":[]"),
        "PINK_BEHAVIOR_FAILURE: {json}"
    );
    // A migração de schema é parte do contrato: o campo de override de projeção
    // foi retirado com a capacidade, e não renomeado nem preenchido com nulo.
    assert!(
        json.contains("\"schema\":3"),
        "PINK_BEHAVIOR_FAILURE: {json}"
    );
    assert!(
        !json.contains("projection_overrides_required") && !json.contains("overrides"),
        "PINK_BEHAVIOR_FAILURE: campo de capacidade aposentada presente: {json}"
    );
}

#[test]
fn nav_impacto_rejeita_ref_com_opcao_ou_espaco() {
    let repo = root().to_string_lossy().into_owned();
    for invalid in ["--stat", "HEAD bad"] {
        let output = run(&[
            "nav", "impacto", "--diff", invalid, "--repo", &repo, "--json",
        ]);
        assert_eq!(output.status.code(), Some(6), "{invalid}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("E-IMPACT-DIFF"));
    }
}

#[test]
fn preflight_unificado_compoe_campos_do_schema_migrado() {
    let output = run(&[
        "verificar",
        "--diff",
        "origin/main",
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    let json = stdout(&output);
    assert!(json.contains("\"schema\":3"), "{json}");
    for field in [
        "blocking",
        "recommended_actions",
        "doctor",
        "navigation_impact",
        "projection_validation",
        "documentary_state",
    ] {
        assert!(json.contains(&format!("\"{field}\":")), "{field}: {json}");
    }
    // Campos aposentados com a autoria de manifestos (#698).
    for retirado in ["pinker_change", "expected_deferred", "warnings"] {
        assert!(
            !json.contains(&format!("\"{retirado}\":")),
            "{retirado} ainda exposto: {json}"
        );
    }
    if option_env!("PINKER_BUILD_COMMIT").is_some() {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Controle causal de #698: sem bloco `pinker-change` em lugar nenhum, o
/// preflight de um candidato saudável continua READY.
#[test]
fn preflight_sem_bloco_pinker_change_nao_bloqueia() {
    let output = run(&[
        "verificar",
        "--diff",
        "HEAD...HEAD",
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    let json = stdout(&output);
    assert!(!json.contains("pinker_change"), "{json}");
    if option_env!("PINKER_BUILD_COMMIT").is_some() {
        assert_eq!(output.status.code(), Some(0), "{json}");
        assert!(json.contains("\"status\":\"READY\""), "{json}");
        assert!(json.contains("\"blocking\":[]"), "{json}");
    }
}

#[test]
fn sensitivity_mantem_composicao_em_uma_autoridade() {
    let main = fonte_de_modulo::pink_cli();
    let tooling = fs::read_to_string(root().join("src/tooling.rs")).unwrap();
    assert!(main.contains("tooling::collect_doctor"));
    assert!(main.contains("tooling::collect_impact"));
    assert!(main.contains("tooling::collect_preflight"));
    assert!(!main.contains("tooling::freeze_import"));
    assert_eq!(tooling.matches("diff_coverage::analyze(").count(), 1);
    assert_eq!(tooling.matches("project_state::collect(").count(), 1);
    // POT/LPT: INVARIANT o tooling lê o acervo histórico e não conhece autoria.
    assert_eq!(tooling.matches("change::Manifests::load").count(), 1);
    assert_eq!(tooling.matches("parse_pr_body").count(), 0);
    assert!(!tooling.contains("Command::new(\"sh\")"));
    assert!(!tooling.contains("Command::new(\"bash\")"));
}

#[test]
fn cli_rejeita_flags_incompletas_e_mistura_de_modos() {
    for args in [
        vec!["nav", "impacto", "--json"],
        vec!["verificar", "--json"],
        vec!["verificar", "--diff", "HEAD", "--corpo", "x"],
        vec!["doc", "importar-pr", "454"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2), "{args:?}");
    }
}

// @pinker-nav:end evidence.tooling.f1.contracts
