use pinker_v0::doc::DocConfig;
use pinker_v0::tooling::{self, FreezeImportClassification};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const VALID_BODY: &str = "## Resumo\nF1 de tooling.\n\n```pinker-change\nschema: 1\nkind: parallel-phase\ntitle: Pink bootstrap preflight\nstatus: completed\narea:\n  - development.tooling\nupdates:\n  state: false\n  history: false\n  roadmap: false\nvalidation:\n  required:\n    - make ci\n```\n";

// @pinker-nav:start evidencia.tooling.f1.contracts
// @pinker-nav:domain tooling
// @pinker-nav:layer evidence
// @pinker-nav:summary Contratos positivos, negativos e de sensibilidade para doctor, nav impacto, import freeze-aware, preflight composto e lifecycle do baseline publicado.

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
    if option_env!("PINKER_BUILD_COMMIT").is_some() {
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
fn nav_impacto_diff_vazio_e_known_sem_overrides() {
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
    assert!(json.contains("\"changed_files\":[]"), "{json}");
    assert!(json.contains(
        "\"projection_overrides_required\":{\"status\":\"KNOWN\",\"reason\":null,\"items\":[]}"
    ));
}

#[test]
fn nav_impacto_diff_real_expoe_unknown_sem_falsa_precisao() {
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
        "projection_overrides_required",
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
    assert!(
        json.contains("\"projection_overrides_required\":{\"status\":\"UNKNOWN\""),
        "PINK_BEHAVIOR_FAILURE: {json}"
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
fn importar_pr_freeze_preserva_artifact_sem_mutar_autoridade() {
    let directory = temp("freeze");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    let artifact = directory.join("pr-454.yaml");
    fs::write(&body, VALID_BODY).unwrap();
    let ledger = root().join("docs/history/changes.md");
    let ledger_before = fs::read(&ledger).unwrap();
    let canonical = root().join(".pinker/changes/pr-454.yaml");
    assert!(!canonical.exists());
    let output = run(&[
        "doc",
        "importar-pr",
        "454",
        "--corpo",
        &body.to_string_lossy(),
        "--freeze",
        "--artifact",
        &artifact.to_string_lossy(),
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout(&output);
    assert!(json.contains("\"classification\":\"VALIDATED_DEFERRED_BY_FREEZE\""));
    assert!(json.contains("\"canonical_documentation_mutated\":false"));
    assert!(artifact.is_file());
    assert!(!canonical.exists());
    assert_eq!(fs::read(&ledger).unwrap(), ledger_before);
}

#[test]
fn importar_pr_freeze_invalido_nao_cria_artifact() {
    let directory = temp("invalid");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    let artifact = directory.join("artifact.yaml");
    fs::write(&body, "sem bloco pinker-change\n").unwrap();
    let output = run(&[
        "doc",
        "importar-pr",
        "454",
        "--corpo",
        &body.to_string_lossy(),
        "--freeze",
        "--artifact",
        &artifact.to_string_lossy(),
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(5));
    assert!(stdout(&output).contains("\"classification\":\"INVALID_MANIFEST\""));
    assert!(!artifact.exists());
}

#[test]
fn freeze_classifica_inconsistencia_documental_inesperada() {
    let directory = temp("inconsistent");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    let artifact = directory.join("artifact.yaml");
    fs::write(&body, VALID_BODY).unwrap();
    let config = DocConfig::load(&root()).unwrap();
    let report = tooling::freeze_import(&directory, &config, 454, &body, &artifact);
    assert_eq!(
        report.classification,
        FreezeImportClassification::UnexpectedDocumentaryInconsistency
    );
    assert!(!artifact.exists());
}

#[test]
fn freeze_rejeita_artifact_em_autoridade_canonica() {
    let directory = temp("canonical");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    fs::write(&body, VALID_BODY).unwrap();
    let config = DocConfig::load(&root()).unwrap();
    let artifact = root().join("docs/pr-454.yaml");
    let report = tooling::freeze_import(&root(), &config, 454, &body, &artifact);
    assert_eq!(
        report.classification,
        FreezeImportClassification::InvalidManifest
    );
    assert!(!artifact.exists());
}

#[test]
fn preflight_unificado_compoe_campos_e_deferred() {
    let directory = temp("preflight");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    fs::write(&body, VALID_BODY).unwrap();
    let output = run(&[
        "verificar",
        "--diff",
        "origin/main",
        "--documentation-frozen",
        "--corpo",
        &body.to_string_lossy(),
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    let json = stdout(&output);
    for field in [
        "blocking",
        "warnings",
        "expected_deferred",
        "recommended_actions",
        "doctor",
        "navigation_impact",
        "projection_validation",
        "pinker_change",
        "documentary_state",
    ] {
        assert!(json.contains(&format!("\"{field}\":")), "{field}: {json}");
    }
    assert!(json.contains("VALIDATED_DEFERRED_BY_FREEZE"), "{json}");
    if option_env!("PINKER_BUILD_COMMIT").is_some() {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn preflight_manifesto_invalido_bloqueia_antes_de_ci() {
    let directory = temp("preflight-invalid");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    fs::write(&body, "inválido\n").unwrap();
    let output = run(&[
        "verificar",
        "--diff",
        "HEAD...HEAD",
        "--documentation-frozen",
        "--corpo",
        &body.to_string_lossy(),
        "--repo",
        &root().to_string_lossy(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let json = stdout(&output);
    assert!(json.contains("\"status\":\"BLOCKED\""));
    assert!(json.contains("invalid_pinker_change"));
}

#[test]
fn baseline_script_exige_release_identidade_e_publicacao_atomica() {
    let script = fs::read_to_string(root().join("scripts/pink-baseline")).unwrap();
    for required in [
        "cargo build --locked --release --bin pink",
        "PINKER_BUILD_COMMIT",
        "--version-json",
        "forja-pink-bundle-v1",
        "forja-software-manifest-v1",
        "/opt/pinker/releases/pink/$commit",
        "mv -Tf \"$next_link\" /opt/pinker/bin/pink",
        "sha256sum",
    ] {
        assert!(script.contains(required), "ausente: {required}");
    }
    assert!(!script.contains("target/debug/pink"));
    let output = Command::new(root().join("scripts/pink-baseline"))
        .args(["publish", "--bundle", "/definitely/missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("publish exige root"));
}

#[test]
fn sensitivity_mantem_composicao_em_uma_autoridade() {
    let main = fs::read_to_string(root().join("src/main.rs")).unwrap();
    let tooling = fs::read_to_string(root().join("src/tooling.rs")).unwrap();
    assert!(main.contains("tooling::collect_doctor"));
    assert!(main.contains("tooling::collect_impact"));
    assert!(main.contains("tooling::collect_preflight"));
    assert!(main.contains("tooling::freeze_import"));
    assert_eq!(tooling.matches("diff_coverage::analyze(").count(), 1);
    assert_eq!(tooling.matches("project_state::collect(").count(), 1);
    assert_eq!(tooling.matches("change::Change::parse_pr_body").count(), 2);
    assert!(!tooling.contains("Command::new(\"sh\")"));
    assert!(!tooling.contains("Command::new(\"bash\")"));
}

#[test]
fn cli_rejeita_flags_incompletas_e_mistura_de_modos() {
    for args in [
        vec!["nav", "impacto", "--json"],
        vec!["verificar", "--json"],
        vec!["doc", "importar-pr", "454", "--freeze"],
        vec![
            "doc",
            "importar-pr",
            "454",
            "--check",
            "--freeze",
            "--corpo",
            "x",
            "--artifact",
            "y",
        ],
    ] {
        assert_eq!(run(&args).status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn artifact_path_relative_fora_de_docs_e_aceito_lexicalmente() {
    let directory = temp("relative");
    fs::create_dir_all(&directory).unwrap();
    let body = directory.join("body.md");
    fs::write(&body, VALID_BODY).unwrap();
    let config = DocConfig::load(&root()).unwrap();
    let report = tooling::freeze_import(
        &root(),
        &config,
        454,
        &body,
        Path::new("build/f1-artifact.yaml"),
    );
    assert_eq!(
        report.classification,
        FreezeImportClassification::ValidatedDeferredByFreeze
    );
    let artifact = root().join("build/f1-artifact.yaml");
    assert!(artifact.is_file());
    fs::remove_file(artifact).unwrap();
}
// @pinker-nav:end evidencia.tooling.f1.contracts
