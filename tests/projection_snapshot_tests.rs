use pinker_v0::nav::CodeRegion;
use pinker_v0::projection_snapshot::{
    canonical_region_projection, fnv1a64, measure_regions, parse_snapshot, reconstruct_predecessor,
    reconstruct_predecessor_with_consumption, render_snapshot, render_verification_json,
    verify_snapshot, verify_snapshot_text, ProjectionConsumption, ProjectionField,
    ProjectionHarnessError, ProjectionMeasurement, ProjectionOperation, ProjectionRule,
    ProjectionSelector, ProjectionSnapshot, ProjectionVerification, SnapshotStatus,
};

const FROZEN: &str = include_str!("fixtures/projection_snapshots/frozen-valid.toml");
const CANDIDATE: &str = include_str!("fixtures/projection_snapshots/candidate-valid.toml");

fn region(key: &str, file: &str, summary: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("synthetic".to_string()),
        layer: Some("evidence".to_string()),
        phase: None,
        file: file.to_string(),
        start_marker: 1,
        content_start: 2,
        content_end: 2,
        end_marker: 3,
        summary: summary.to_string(),
        hash: "fnv1a64:0000000000000001".to_string(),
        status: "active".to_string(),
    }
}

fn regions() -> Vec<CodeRegion> {
    vec![
        region("synthetic.alpha", "src/a.rs", "alpha"),
        region("synthetic.beta", "src/b.rs", "beta"),
        region("synthetic.gamma", "src/shared.rs", "gamma"),
        region("synthetic.delta", "src/shared.rs", "delta"),
    ]
}

fn rule(
    operation: ProjectionOperation,
    selector: ProjectionSelector,
    consumption: ProjectionConsumption,
) -> ProjectionRule {
    ProjectionRule {
        operation,
        selector,
        field: None,
        replacement: None,
        expected: None,
        required: true,
        consumption,
    }
}

fn require_key(key: &str) -> ProjectionRule {
    rule(
        ProjectionOperation::RequireRegion,
        ProjectionSelector::Key(key.to_string()),
        ProjectionConsumption::ExactlyOnce,
    )
}

fn restore(key: &str, field: ProjectionField, replacement: &str) -> ProjectionRule {
    let mut restore = rule(
        ProjectionOperation::RestoreField,
        ProjectionSelector::Key(key.to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    restore.field = Some(field);
    restore.replacement = Some(replacement.to_string());
    restore
}

fn guard(selector: ProjectionSelector, field: ProjectionField, expected: &str) -> ProjectionRule {
    let mut guard = rule(
        ProjectionOperation::RequireRegion,
        selector,
        ProjectionConsumption::ExactlyOnce,
    );
    guard.field = Some(field);
    guard.expected = Some(expected.to_string());
    guard
}

fn reference_projection(regions: &[CodeRegion]) -> String {
    let mut records: Vec<_> = regions
        .iter()
        .map(|region| {
            format!(
                "{:?}\n",
                (
                    1,
                    region.key.as_str(),
                    region.kind.as_str(),
                    region.domain.as_deref(),
                    region.layer.as_deref(),
                    region.file.as_str(),
                    region.summary.as_str(),
                    region.hash.as_str(),
                    region.status.as_str(),
                )
            )
        })
        .collect();
    records.sort_unstable();
    records.concat()
}

fn snapshot_for(current: &[CodeRegion], rules: Vec<ProjectionRule>) -> ProjectionSnapshot {
    let reconstructed = reconstruct_predecessor(current, &rules).unwrap();
    ProjectionSnapshot {
        schema: 1,
        id: "synthetic.snapshot.1".to_string(),
        status: SnapshotStatus::Frozen,
        description: "Snapshot sintético.".to_string(),
        measurement: measure_regions(&reconstructed).unwrap(),
        predecessor: Some("synthetic.previous.1".to_string()),
        reconstruction: rules,
        justification: "Cobertura sintética.".to_string(),
    }
}

fn replace_line(text: &str, prefix: &str, replacement: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with(prefix) {
                replacement
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn parse_frozen_valido() {
    let snapshot = parse_snapshot(FROZEN).unwrap();
    assert_eq!(snapshot.status, SnapshotStatus::Frozen);
    assert_eq!(snapshot.id, "synthetic.base.2");
}

#[test]
fn parse_candidate_valido() {
    let snapshot = parse_snapshot(CANDIDATE).unwrap();
    assert_eq!(snapshot.status, SnapshotStatus::Candidate);
    assert_eq!(snapshot.predecessor.as_deref(), Some("synthetic.base.2"));
}

#[test]
fn schema_nao_suportado() {
    let text = replace_line(FROZEN, "schema =", "schema = 2");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::UnsupportedSchema { found: 2 })
    ));
}

#[test]
fn campo_obrigatorio_ausente() {
    let text = FROZEN
        .lines()
        .filter(|line| !line.starts_with("description ="))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::MissingField { field, .. }) if field == "description"
    ));
}

#[test]
fn campo_desconhecido() {
    let text = FROZEN.replace(
        "description = \"Base sintética congelada.\"\n",
        "description = \"Base sintética congelada.\"\nmystery = 1\n",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::UnknownField { field, .. }) if field == "mystery"
    ));
}

#[test]
fn campo_duplicado() {
    let text = FROZEN.replace(
        "description = \"Base sintética congelada.\"\n",
        "description = \"Base sintética congelada.\"\ndescription = \"Outra.\"\n",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::DuplicateField { field, .. }) if field == "description"
    ));
}

#[test]
fn enum_de_status_invalido() {
    let text = replace_line(FROZEN, "status =", "status = \"ARCHIVED\"");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::UnknownEnum { field, .. }) if field == "status"
    ));
}

#[test]
fn id_invalido() {
    let text = replace_line(FROZEN, "id =", "id = \"Synthetic Base\"");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidId { field, .. }) if field == "id"
    ));
}

#[test]
fn fnv_invalido() {
    let text = replace_line(
        FROZEN,
        "fnv1a64 =",
        "fnv1a64 = \"FNV1A64:0123456789ABCDEF\"",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidFnv { .. })
    ));
}

#[test]
fn path_absoluto_rejeitado() {
    let text = CANDIDATE
        .replace(
            "operation = \"EXCLUDE_REGION\"",
            "operation = \"EXCLUDE_FILE\"",
        )
        .replace("selector_type = \"key\"", "selector_type = \"file\"")
        .replace("selector = \"synthetic.gamma\"", "selector = \"/tmp/a.rs\"")
        .replace(
            "consumption = \"EXACTLY_ONCE\"",
            "consumption = \"ALL_MATCHES_AT_LEAST_ONE\"",
        );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidPath { .. })
    ));
}

#[test]
fn renderer_canonico() {
    let snapshot = parse_snapshot(FROZEN).unwrap();
    assert_eq!(render_snapshot(&snapshot).unwrap(), FROZEN);
}

#[test]
fn parse_render_parse_preserva_modelo() {
    let first = parse_snapshot(CANDIDATE).unwrap();
    let rendered = render_snapshot(&first).unwrap();
    assert_eq!(parse_snapshot(&rendered).unwrap(), first);
}

#[test]
fn render_parse_render_byte_identical() {
    let rendered = render_snapshot(&parse_snapshot(CANDIDATE).unwrap()).unwrap();
    let rendered_again = render_snapshot(&parse_snapshot(&rendered).unwrap()).unwrap();
    assert_eq!(rendered, rendered_again);
    assert!(rendered.ends_with('\n'));
    assert!(!rendered.ends_with("\n\n"));
}

#[test]
fn ordem_de_regras_preservada() {
    let second = r#"

[[reconstruction]]
operation = "REQUIRE_ABSENCE"
selector_type = "key"
selector = "synthetic.missing"
required = true
consumption = "EXACTLY_ONCE"
"#;
    let text = format!("{FROZEN}{second}");
    let snapshot = parse_snapshot(&text).unwrap();
    assert_eq!(
        snapshot
            .reconstruction
            .iter()
            .map(|item| item.operation)
            .collect::<Vec<_>>(),
        vec![
            ProjectionOperation::RequireRegion,
            ProjectionOperation::RequireAbsence
        ]
    );
}

#[test]
fn projecao_independente_da_ordem_de_entrada() {
    let forward = regions();
    let mut reverse = forward.clone();
    reverse.reverse();
    assert_eq!(
        canonical_region_projection(&forward).unwrap(),
        canonical_region_projection(&reverse).unwrap()
    );
}

#[test]
fn medicao_de_region_count() {
    assert_eq!(measure_regions(&regions()).unwrap().region_count, 4);
}

#[test]
fn medicao_de_projection_length() {
    let current = regions();
    let projection = canonical_region_projection(&current).unwrap();
    assert_eq!(
        measure_regions(&current).unwrap().projection_length,
        projection.as_bytes().len()
    );
}

#[test]
fn fnv1a64_conhecido() {
    assert_eq!(fnv1a64(b"hello"), 0xa430_d846_80aa_bd0b);
}

#[test]
fn reconstrucao_correta() {
    let mut restore = rule(
        ProjectionOperation::RestoreField,
        ProjectionSelector::Key("synthetic.alpha".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    restore.field = Some(ProjectionField::Summary);
    restore.replacement = Some("alpha anterior".to_string());
    let exclude = rule(
        ProjectionOperation::ExcludeRegion,
        ProjectionSelector::Key("synthetic.beta".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    let reconstructed = reconstruct_predecessor(&regions(), &[exclude, restore]).unwrap();
    assert_eq!(reconstructed.len(), 3);
    assert_eq!(reconstructed[0].summary, "alpha anterior");
    assert!(!reconstructed
        .iter()
        .any(|item| item.key == "synthetic.beta"));
}

#[test]
fn entrada_original_nao_modificada() {
    let current = regions();
    let before = current.clone();
    let exclude = rule(
        ProjectionOperation::ExcludeRegion,
        ProjectionSelector::Key("synthetic.beta".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    reconstruct_predecessor(&current, &[exclude]).unwrap();
    assert_eq!(current, before);
}

#[test]
fn override_consumido_exatamente_uma_vez() {
    let reconstructed =
        reconstruct_predecessor(&regions(), &[require_key("synthetic.alpha")]).unwrap();
    assert_eq!(reconstructed.len(), 4);
}

#[test]
fn override_ausente() {
    let mut restore = rule(
        ProjectionOperation::RestoreField,
        ProjectionSelector::Key("synthetic.missing".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    restore.field = Some(ProjectionField::Summary);
    restore.replacement = Some("anterior".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::OverrideMissing { .. })
    ));
}

#[test]
fn override_excedente() {
    let rule = rule(
        ProjectionOperation::RequireRegion,
        ProjectionSelector::File("src/shared.rs".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[rule]),
        Err(ProjectionHarnessError::OverrideExcess { matches: 2, .. })
    ));
}

#[test]
fn override_repetido() {
    let repeated = require_key("synthetic.alpha");
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[repeated.clone(), repeated]),
        Err(ProjectionHarnessError::OverrideRepeated { .. })
    ));
}

#[test]
fn override_nao_consumido() {
    let mut unconsumed = require_key("synthetic.alpha");
    unconsumed.required = false;
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[unconsumed]),
        Err(ProjectionHarnessError::OverrideUnconsumed { .. })
    ));
}

#[test]
fn exclusao_por_arquivo_sem_correspondencia() {
    let exclude = rule(
        ProjectionOperation::ExcludeFile,
        ProjectionSelector::File("src/missing.rs".to_string()),
        ProjectionConsumption::AllMatchesAtLeastOne,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[exclude]),
        Err(ProjectionHarnessError::OverrideMissing { .. })
    ));
}

#[test]
fn exclusao_por_arquivo_com_multiplas_regioes() {
    let exclude = rule(
        ProjectionOperation::ExcludeFile,
        ProjectionSelector::File("src/shared.rs".to_string()),
        ProjectionConsumption::AllMatchesAtLeastOne,
    );
    let reconstructed = reconstruct_predecessor(&regions(), &[exclude]).unwrap();
    assert_eq!(reconstructed.len(), 2);
    assert!(reconstructed
        .iter()
        .all(|item| item.file != "src/shared.rs"));
}

#[test]
fn regiao_requerida_ausente() {
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[require_key("synthetic.removed")]),
        Err(ProjectionHarnessError::UnexpectedRegionRemoval { .. })
    ));
}

#[test]
fn ausencia_requerida_violada() {
    let absent = rule(
        ProjectionOperation::RequireAbsence,
        ProjectionSelector::Key("synthetic.alpha".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[absent]),
        Err(ProjectionHarnessError::UnexpectedRegionPresence { .. })
    ));
}

#[test]
fn key_alterada_e_harness_failure() {
    let mut guard = rule(
        ProjectionOperation::RequireRegion,
        ProjectionSelector::File("src/a.rs".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    guard.field = Some(ProjectionField::Key);
    guard.expected = Some("synthetic.original".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[guard]),
        Err(ProjectionHarnessError::KeyChanged { .. })
    ));
}

#[test]
fn path_alterado_e_harness_failure() {
    let mut guard = require_key("synthetic.alpha");
    guard.field = Some(ProjectionField::File);
    guard.expected = Some("src/original.rs".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[guard]),
        Err(ProjectionHarnessError::PathChanged { .. })
    ));
}

#[test]
fn metadata_alterada_e_harness_failure() {
    let mut guard = require_key("synthetic.alpha");
    guard.field = Some(ProjectionField::Summary);
    guard.expected = Some("resumo anterior".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[guard]),
        Err(ProjectionHarnessError::MetadataChanged {
            field: ProjectionField::Summary,
            ..
        })
    ));
}

#[test]
fn verificacao_match() {
    let current = regions();
    let snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    assert!(matches!(
        verify_snapshot(&snapshot, &current),
        ProjectionVerification::Match { .. }
    ));
}

#[test]
fn drift_em_region_count() {
    let current = regions();
    let mut snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    snapshot.measurement.region_count += 1;
    match verify_snapshot(&snapshot, &current) {
        ProjectionVerification::Drift(drift) => {
            assert!(drift.region_count);
            assert!(!drift.projection_length);
            assert!(!drift.fnv1a64);
        }
        other => panic!("esperava drift, obtive {other:?}"),
    }
}

#[test]
fn drift_em_projection_length() {
    let current = regions();
    let mut snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    snapshot.measurement.projection_length += 1;
    match verify_snapshot(&snapshot, &current) {
        ProjectionVerification::Drift(drift) => {
            assert!(!drift.region_count);
            assert!(drift.projection_length);
            assert!(!drift.fnv1a64);
        }
        other => panic!("esperava drift, obtive {other:?}"),
    }
}

#[test]
fn drift_em_fnv1a64() {
    let current = regions();
    let mut snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    snapshot.measurement.fnv1a64 ^= 1;
    match verify_snapshot(&snapshot, &current) {
        ProjectionVerification::Drift(drift) => {
            assert!(!drift.region_count);
            assert!(!drift.projection_length);
            assert!(drift.fnv1a64);
        }
        other => panic!("esperava drift, obtive {other:?}"),
    }
}

#[test]
fn harness_failure_nao_classificado_como_drift() {
    let current = regions();
    let mut snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    snapshot.reconstruction = vec![require_key("synthetic.missing")];
    assert!(matches!(
        verify_snapshot(&snapshot, &current),
        ProjectionVerification::HarnessFailure(
            ProjectionHarnessError::UnexpectedRegionRemoval { .. }
        )
    ));
}

#[test]
fn relatorio_estruturado_deterministico() {
    let current = regions();
    let snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    let verification = verify_snapshot(&snapshot, &current);
    let first = render_verification_json(&snapshot.id, &verification);
    let second = render_verification_json(&snapshot.id, &verification);
    assert_eq!(first, second);
    assert_eq!(first.matches("\"result\":\"MATCH\"").count(), 1);
    assert!(first.ends_with('\n'));
}

#[test]
fn roots_absolutos_distintos_com_resultado_identico() {
    fn from_root(_root: &str) -> Vec<CodeRegion> {
        regions()
    }
    let first = measure_regions(&from_root("/tmp/synthetic-one")).unwrap();
    let second = measure_regions(&from_root("/var/tmp/synthetic-two")).unwrap();
    assert_eq!(first, second);
}

#[test]
fn nenhuma_escrita_nas_operacoes_publicas() {
    let source = include_str!("../src/projection_snapshot.rs");
    for forbidden in [
        "fs::write",
        "File::create",
        "OpenOptions",
        "rename",
        "remove_file",
        "remove_dir",
        "create_dir",
        "Command",
        "\ngit ",
        "network",
    ] {
        assert!(
            !source.contains(forbidden),
            "superfície proibida: {forbidden}"
        );
    }

    let current = regions();
    let snapshot = snapshot_for(&current, vec![require_key("synthetic.alpha")]);
    let parsed = parse_snapshot(&render_snapshot(&snapshot).unwrap()).unwrap();
    let reconstructed = reconstruct_predecessor(&current, &parsed.reconstruction).unwrap();
    let verification = verify_snapshot(&parsed, &current);
    assert_eq!(reconstructed, current);
    assert!(matches!(verification, ProjectionVerification::Match { .. }));
}

#[test]
fn numero_invalido_rejeitado_sem_fallback() {
    let text = replace_line(FROZEN, "region_count =", "region_count = -1");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidNumber { field, .. }) if field == "region_count"
    ));
}

#[test]
fn seletor_ambiguo_rejeitado() {
    let ambiguous = rule(
        ProjectionOperation::RequireRegion,
        ProjectionSelector::File("src/shared.rs".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[ambiguous]),
        Err(ProjectionHarnessError::OverrideExcess { .. })
    ));
}

#[test]
fn regra_incompleta_rejeitada() {
    let text = CANDIDATE.replace(
        "operation = \"EXCLUDE_REGION\"",
        "operation = \"RESTORE_FIELD\"",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn predecessor_invalido_rejeitado() {
    let text = replace_line(
        CANDIDATE,
        "predecessor =",
        "predecessor = \"synthetic next\"",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidId { field, .. }) if field == "predecessor"
    ));
}

#[test]
fn medicao_expoe_exatamente_tres_campos() {
    let measurement = measure_regions(&regions()).unwrap();
    let ProjectionMeasurement {
        region_count,
        projection_length,
        fnv1a64,
    } = measurement;
    assert_eq!(region_count, 4);
    assert!(projection_length > 0);
    assert_ne!(fnv1a64, 0);
}

#[test]
fn parser_aceita_crlf_e_whitespace_externo() {
    let text = FROZEN.lines().fold(String::new(), |mut text, line| {
        text.push_str("  ");
        text.push_str(line);
        text.push_str("  \r\n");
        text
    });
    assert_eq!(parse_snapshot(&text).unwrap().id, "synthetic.base.2");
}

#[test]
fn parser_aceita_strings_escapadas_e_utf8() {
    let text = replace_line(
        FROZEN,
        "description =",
        r#"description = "Café \"rosa\" \\ linha\nseguinte""#,
    );
    assert_eq!(
        parse_snapshot(&text).unwrap().description,
        "Café \"rosa\" \\ linha\nseguinte"
    );
}

#[test]
fn parser_aceita_reconstrucao_vazia() {
    let text = FROZEN
        .split("\n[[reconstruction]]")
        .next()
        .unwrap()
        .to_string()
        + "\n";
    let snapshot = parse_snapshot(&text).unwrap();
    assert!(snapshot.reconstruction.is_empty());
}

#[test]
fn parser_rejeita_ordem_nao_canonica_na_raiz() {
    let text = FROZEN.replacen(
        "schema = 1\nid = \"synthetic.base.2\"",
        "id = \"synthetic.base.2\"\nschema = 1",
        1,
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::NonCanonicalOrder {
            section,
            field,
            ..
        }) if section == "root" && field == "schema"
    ));
}

#[test]
fn parser_rejeita_ordem_nao_canonica_em_regra() {
    let text = FROZEN.replacen(
        "selector_type = \"key\"\nselector = \"synthetic.alpha\"",
        "selector = \"synthetic.alpha\"\nselector_type = \"key\"",
        1,
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::NonCanonicalOrder {
            section,
            field,
            ..
        }) if section == "reconstruction" && field == "selector_type"
    ));
}

#[test]
fn parser_rejeita_comentario_em_linha_independente() {
    let text = format!("# comentário não suportado\n{FROZEN}");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidToml { line: 1, .. })
    ));
}

#[test]
fn parser_rejeita_comentario_inline() {
    let text = FROZEN.replacen("schema = 1", "schema = 1 # comentário", 1);
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidNumber {
            line: 1,
            field,
            ..
        }) if field == "schema"
    ));
}

#[test]
fn parser_rejeita_escape_toml_fora_do_subset() {
    let text = replace_line(
        FROZEN,
        "description =",
        r#"description = "\u0061 não é suportado""#,
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidToml { .. })
    ));
}

#[test]
fn parser_rejeita_string_com_aspas_simples() {
    let text = replace_line(FROZEN, "description =", "description = 'texto'");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidToml { .. })
    ));
}

#[test]
fn parser_rejeita_array_toml() {
    let text = replace_line(FROZEN, "description =", "description = [\"texto\"]");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidToml { .. })
    ));
}

#[test]
fn parser_rejeita_tabela_desconhecida() {
    let text = format!("{FROZEN}\n[measurement]\n");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::UnknownSection { .. })
    ));
}

#[test]
fn parser_rejeita_array_table_incompleta_repetida() {
    let text = format!("{FROZEN}\n[[reconstruction]]\n");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::MissingField { section, field })
            if section == "reconstruction" && field == "operation"
    ));
}

#[test]
fn parser_rejeita_conteudo_solto_ao_final() {
    let text = format!("{FROZEN}\nconteúdo solto\n");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidToml { .. })
    ));
}

#[test]
fn parser_detecta_duplicata_separada_por_outro_campo() {
    let text = FROZEN.replacen(
        "region_count = 2",
        "region_count = 2\ndescription = \"duplicada\"",
        1,
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::DuplicateField { field, .. })
            if field == "description"
    ));
}

#[test]
fn parser_detecta_duplicata_separada_em_regra() {
    let text = FROZEN.replacen(
        "consumption = \"EXACTLY_ONCE\"",
        "consumption = \"EXACTLY_ONCE\"\nselector = \"synthetic.alpha\"",
        1,
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::DuplicateField { field, .. })
            if field == "selector"
    ));
}

#[test]
fn parser_rejeita_schema_com_sinal() {
    let text = FROZEN.replacen("schema = 1", "schema = +1", 1);
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidNumber { field, .. }) if field == "schema"
    ));
}

#[test]
fn parser_rejeita_overflow_de_usize() {
    let text = replace_line(
        FROZEN,
        "region_count =",
        "region_count = 999999999999999999999999999999999999",
    );
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidNumber { field, .. }) if field == "region_count"
    ));
}

#[test]
fn parser_rejeita_underscore_em_id_de_snapshot() {
    let text = replace_line(FROZEN, "id =", "id = \"synthetic_base.2\"");
    assert!(matches!(
        parse_snapshot(&text),
        Err(ProjectionHarnessError::InvalidId { field, .. }) if field == "id"
    ));
}

#[test]
fn renderer_canonico_de_reconstrucao_vazia() {
    let mut snapshot = parse_snapshot(FROZEN).unwrap();
    snapshot.reconstruction.clear();
    let rendered = render_snapshot(&snapshot).unwrap();
    assert_eq!(
        render_snapshot(&parse_snapshot(&rendered).unwrap()).unwrap(),
        rendered
    );
    assert_eq!(rendered.matches("[[reconstruction]]").count(), 0);
    assert!(rendered.ends_with('\n'));
    assert!(!rendered.ends_with("\n\n"));
}

#[test]
fn renderer_roundtrip_cobre_cada_operacao() {
    let operations = vec![
        rule(
            ProjectionOperation::ExcludeRegion,
            ProjectionSelector::Key("synthetic.alpha".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
        rule(
            ProjectionOperation::ExcludeFile,
            ProjectionSelector::File("src/a.rs".to_string()),
            ProjectionConsumption::AllMatchesAtLeastOne,
        ),
        restore(
            "synthetic.alpha",
            ProjectionField::Summary,
            "resumo anterior",
        ),
        guard(
            ProjectionSelector::Key("synthetic.alpha".to_string()),
            ProjectionField::Summary,
            "alpha",
        ),
        rule(
            ProjectionOperation::RequireAbsence,
            ProjectionSelector::Key("synthetic.absent".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
    ];
    for operation in operations {
        let snapshot = ProjectionSnapshot {
            schema: 1,
            id: "synthetic.operation.1".to_string(),
            status: SnapshotStatus::Frozen,
            description: "Operação sintética.".to_string(),
            measurement: ProjectionMeasurement {
                region_count: 1,
                projection_length: 1,
                fnv1a64: 1,
            },
            predecessor: None,
            reconstruction: vec![operation],
            justification: "Roundtrip.".to_string(),
        };
        let rendered = render_snapshot(&snapshot).unwrap();
        assert_eq!(parse_snapshot(&rendered).unwrap(), snapshot);
    }
}

#[test]
fn renderer_escapa_strings_e_preserva_unicode() {
    let mut snapshot = parse_snapshot(FROZEN).unwrap();
    snapshot.description = "Café \"rosa\" \\ linha\nseguinte".to_string();
    snapshot.justification = "日本語".to_string();
    let rendered = render_snapshot(&snapshot).unwrap();
    assert!(rendered.contains(r#"description = "Café \"rosa\" \\ linha\nseguinte""#));
    assert_eq!(parse_snapshot(&rendered).unwrap(), snapshot);
}

#[test]
fn renderer_rejeita_controle_sem_escape_no_subset() {
    let mut snapshot = parse_snapshot(FROZEN).unwrap();
    snapshot.description = "controle \u{0008}".to_string();
    assert!(matches!(
        render_snapshot(&snapshot),
        Err(ProjectionHarnessError::InvalidString { field })
            if field == "description"
    ));
}

#[test]
fn renderer_preserva_valor_opcional_vazio() {
    let mut snapshot = parse_snapshot(FROZEN).unwrap();
    snapshot.reconstruction = vec![restore("synthetic.alpha", ProjectionField::Domain, "")];
    let rendered = render_snapshot(&snapshot).unwrap();
    assert!(rendered.contains("replacement = \"\"\n"));
    assert_eq!(parse_snapshot(&rendered).unwrap(), snapshot);
}

#[test]
fn renderer_preserva_ordem_de_multiplas_regras() {
    let mut snapshot = parse_snapshot(FROZEN).unwrap();
    snapshot.reconstruction = vec![
        require_key("synthetic.alpha"),
        rule(
            ProjectionOperation::RequireAbsence,
            ProjectionSelector::Key("synthetic.beta".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
    ];
    let rendered = render_snapshot(&snapshot).unwrap();
    let parsed = parse_snapshot(&rendered).unwrap();
    assert_eq!(parsed.reconstruction, snapshot.reconstruction);
}

#[test]
fn projecao_e_byte_compativel_com_referencia_historica() {
    let mut alpha = region("synthetic.alpha", "src/nested/a.rs", "");
    alpha.domain = None;
    alpha.layer = None;
    alpha.summary = "café \"rosa\" \\ fim".to_string();
    let mut beta = region("synthetic.beta", "tests/b.rs", "日本語");
    beta.domain = Some("domínio".to_string());
    beta.layer = None;
    let input = vec![beta, alpha];
    assert_eq!(
        canonical_region_projection(&input).unwrap(),
        reference_projection(&input)
    );
}

#[test]
fn fnv1a64_cobre_vetores_vazio_ascii_e_utf8() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"hello"), 0xa430_d846_80aa_bd0b);
    assert_eq!(fnv1a64("café".as_bytes()), 0x48e8_823a_cfa4_0d89);
}

#[test]
fn fnv1a64_cobre_projecao_sintetica_multirregiao() {
    let input = regions();
    let reference = reference_projection(&input);
    let measurement = measure_regions(&input).unwrap();
    assert_eq!(measurement.region_count, 4);
    assert_eq!(measurement.projection_length, reference.len());
    assert_eq!(measurement.fnv1a64, fnv1a64(reference.as_bytes()));
}

#[test]
fn require_region_expected_tem_semantica_propria() {
    let guard = guard(
        ProjectionSelector::Key("synthetic.alpha".to_string()),
        ProjectionField::Summary,
        "alpha",
    );
    assert_eq!(
        reconstruct_predecessor(&regions(), &[guard]).unwrap(),
        regions()
    );
}

#[test]
fn require_region_rejeita_replacement() {
    let mut guard = require_key("synthetic.alpha");
    guard.field = Some(ProjectionField::Summary);
    guard.expected = Some("alpha".to_string());
    guard.replacement = Some("não é guarda".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[guard]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn restore_field_rejeita_expected() {
    let mut restore = restore("synthetic.alpha", ProjectionField::Summary, "anterior");
    restore.expected = Some("alpha".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn exclude_region_rejeita_propriedade_sem_sentido() {
    let mut exclude = rule(
        ProjectionOperation::ExcludeRegion,
        ProjectionSelector::Key("synthetic.alpha".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    exclude.expected = Some("indevido".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[exclude]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn exclude_file_rejeita_propriedade_sem_sentido() {
    let mut exclude = rule(
        ProjectionOperation::ExcludeFile,
        ProjectionSelector::File("src/shared.rs".to_string()),
        ProjectionConsumption::AllMatchesAtLeastOne,
    );
    exclude.field = Some(ProjectionField::Summary);
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[exclude]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn require_absence_rejeita_propriedade_sem_sentido() {
    let mut absent = rule(
        ProjectionOperation::RequireAbsence,
        ProjectionSelector::Key("synthetic.absent".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    absent.replacement = Some("indevido".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[absent]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn restore_field_rejeita_selector_de_arquivo() {
    let mut restore = rule(
        ProjectionOperation::RestoreField,
        ProjectionSelector::File("src/a.rs".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    restore.field = Some(ProjectionField::Summary);
    restore.replacement = Some("anterior".to_string());
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn require_absence_rejeita_selector_de_arquivo() {
    let absent = rule(
        ProjectionOperation::RequireAbsence,
        ProjectionSelector::File("src/missing.rs".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[absent]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn restore_field_rejeita_mutacao_de_key() {
    let restore = restore("synthetic.alpha", ProjectionField::Key, "synthetic.other");
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn restore_field_rejeita_mutacao_de_path() {
    let restore = restore("synthetic.alpha", ProjectionField::File, "src/other.rs");
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::IncompleteRule { .. })
    ));
}

#[test]
fn restore_field_rejeita_hash_incompativel() {
    let restore = restore("synthetic.alpha", ProjectionField::Hash, "não-fnv");
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[restore]),
        Err(ProjectionHarnessError::InvalidFnv { .. })
    ));
}

#[test]
fn restore_field_cobre_todos_os_campos_mutaveis() {
    let rules = vec![
        restore("synthetic.alpha", ProjectionField::Kind, "evidence"),
        restore("synthetic.alpha", ProjectionField::Domain, ""),
        restore("synthetic.alpha", ProjectionField::Layer, ""),
        restore("synthetic.alpha", ProjectionField::Summary, ""),
        restore(
            "synthetic.alpha",
            ProjectionField::Hash,
            "fnv1a64:0000000000000002",
        ),
        restore("synthetic.alpha", ProjectionField::Status, "frozen"),
    ];
    let reconstructed = reconstruct_predecessor(&regions(), &rules).unwrap();
    let alpha = reconstructed
        .iter()
        .find(|region| region.key == "synthetic.alpha")
        .unwrap();
    assert_eq!(alpha.kind, "evidence");
    assert_eq!(alpha.domain, None);
    assert_eq!(alpha.layer, None);
    assert_eq!(alpha.summary, "");
    assert_eq!(alpha.hash, "fnv1a64:0000000000000002");
    assert_eq!(alpha.status, "frozen");
}

#[test]
fn restore_field_rejeita_mesmo_campo_duas_vezes() {
    let rules = vec![
        restore("synthetic.alpha", ProjectionField::Summary, "primeiro"),
        restore("synthetic.alpha", ProjectionField::Summary, "segundo"),
    ];
    assert!(matches!(
        reconstruct_predecessor(&regions(), &rules),
        Err(ProjectionHarnessError::ConflictingFieldOverride {
            field: ProjectionField::Summary,
            ..
        })
    ));
}

#[test]
fn regra_posterior_enxerga_exclusao_anterior() {
    let rules = vec![
        rule(
            ProjectionOperation::ExcludeRegion,
            ProjectionSelector::Key("synthetic.alpha".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
        rule(
            ProjectionOperation::RequireAbsence,
            ProjectionSelector::Key("synthetic.alpha".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
    ];
    let reconstructed = reconstruct_predecessor(&regions(), &rules).unwrap();
    assert!(!reconstructed
        .iter()
        .any(|region| region.key == "synthetic.alpha"));
}

#[test]
fn conflito_exclusao_depois_restauracao_e_fatal() {
    let rules = vec![
        rule(
            ProjectionOperation::ExcludeRegion,
            ProjectionSelector::Key("synthetic.alpha".to_string()),
            ProjectionConsumption::ExactlyOnce,
        ),
        restore("synthetic.alpha", ProjectionField::Summary, "anterior"),
    ];
    assert!(matches!(
        reconstruct_predecessor(&regions(), &rules),
        Err(ProjectionHarnessError::OverrideMissing { rule: 1, .. })
    ));
}

#[test]
fn reconstrucao_e_deterministica_em_execucoes_repetidas() {
    let rules = vec![
        rule(
            ProjectionOperation::ExcludeFile,
            ProjectionSelector::File("src/shared.rs".to_string()),
            ProjectionConsumption::AllMatchesAtLeastOne,
        ),
        restore("synthetic.alpha", ProjectionField::Summary, "anterior"),
    ];
    let first = reconstruct_predecessor(&regions(), &rules).unwrap();
    let second = reconstruct_predecessor(&regions(), &rules).unwrap();
    assert_eq!(first, second);
}

#[test]
fn duplicidade_de_key_falha_antes_de_consumo_ambiguo() {
    let mut input = regions();
    input[1].key = "synthetic.alpha".to_string();
    assert!(matches!(
        reconstruct_predecessor(&input, &[require_key("synthetic.alpha")]),
        Err(ProjectionHarnessError::DuplicateRegionKey { .. })
    ));
}

#[test]
fn falha_de_restauracao_nao_modifica_entrada() {
    let input = regions();
    let before = input.clone();
    let invalid = restore("synthetic.alpha", ProjectionField::Hash, "inválido");
    assert!(reconstruct_predecessor(&input, &[invalid]).is_err());
    assert_eq!(input, before);
}

#[test]
fn exclude_region_sem_match_falha() {
    let exclude = rule(
        ProjectionOperation::ExcludeRegion,
        ProjectionSelector::Key("synthetic.missing".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert!(matches!(
        reconstruct_predecessor(&regions(), &[exclude]),
        Err(ProjectionHarnessError::OverrideMissing { .. })
    ));
}

#[test]
fn require_absence_sem_match_e_consumida() {
    let absent = rule(
        ProjectionOperation::RequireAbsence,
        ProjectionSelector::Key("synthetic.missing".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    assert_eq!(
        reconstruct_predecessor(&regions(), &[absent]).unwrap(),
        regions()
    );
}

#[test]
fn exclusao_por_arquivo_reporta_cardinalidade_total_deterministica() {
    let exclude = rule(
        ProjectionOperation::ExcludeFile,
        ProjectionSelector::File("src/shared.rs".to_string()),
        ProjectionConsumption::AllMatchesAtLeastOne,
    );
    let first = reconstruct_predecessor_with_consumption(&regions(), &[exclude.clone()]).unwrap();
    let second = reconstruct_predecessor_with_consumption(&regions(), &[exclude]).unwrap();
    assert_eq!(first.consumed_regions, vec![2]);
    assert_eq!(first, second);
}

#[test]
fn require_absence_reporta_zero_regioes_e_regra_consumida() {
    let absent = rule(
        ProjectionOperation::RequireAbsence,
        ProjectionSelector::Key("synthetic.missing".to_string()),
        ProjectionConsumption::ExactlyOnce,
    );
    let result = reconstruct_predecessor_with_consumption(&regions(), &[absent]).unwrap();
    assert_eq!(result.consumed_regions, vec![0]);
    assert_eq!(result.regions, regions());
}

#[test]
fn verificacao_textual_converte_parse_failure_em_harness_failure() {
    assert!(matches!(
        verify_snapshot_text("não é TOML", &regions()),
        ProjectionVerification::HarnessFailure(ProjectionHarnessError::InvalidToml { .. })
    ));
}

#[test]
fn verificacao_preserva_schema_failure() {
    let mut snapshot = snapshot_for(&regions(), vec![]);
    snapshot.schema = 2;
    assert!(matches!(
        verify_snapshot(&snapshot, &regions()),
        ProjectionVerification::HarnessFailure(ProjectionHarnessError::UnsupportedSchema {
            found: 2
        })
    ));
}

#[test]
fn verificacao_preserva_validation_failure() {
    let mut snapshot = snapshot_for(&regions(), vec![]);
    snapshot.description.clear();
    assert!(matches!(
        verify_snapshot(&snapshot, &regions()),
        ProjectionVerification::HarnessFailure(ProjectionHarnessError::MissingField {
            field,
            ..
        }) if field == "description"
    ));
}

#[test]
fn verificacao_preserva_consumption_failure() {
    let mut snapshot = snapshot_for(&regions(), vec![]);
    snapshot.reconstruction = vec![restore(
        "synthetic.missing",
        ProjectionField::Summary,
        "anterior",
    )];
    assert!(matches!(
        verify_snapshot(&snapshot, &regions()),
        ProjectionVerification::HarnessFailure(ProjectionHarnessError::OverrideMissing { .. })
    ));
}

#[test]
fn verificacao_preserva_measurement_failure() {
    let mut invalid = regions();
    invalid[0].kind.clear();
    let snapshot = snapshot_for(&regions(), vec![]);
    assert!(matches!(
        verify_snapshot(&snapshot, &invalid),
        ProjectionVerification::HarnessFailure(
            ProjectionHarnessError::MeasurementUnavailable { .. }
        )
    ));
}

#[test]
fn drift_agrega_todos_os_campos_divergentes() {
    let current = regions();
    let mut snapshot = snapshot_for(&current, vec![]);
    snapshot.measurement.region_count += 1;
    snapshot.measurement.projection_length += 1;
    snapshot.measurement.fnv1a64 ^= 1;
    match verify_snapshot(&snapshot, &current) {
        ProjectionVerification::Drift(drift) => {
            assert!(drift.region_count);
            assert!(drift.projection_length);
            assert!(drift.fnv1a64);
        }
        other => panic!("esperava DRIFT agregado, obtive {other:?}"),
    }
}

#[test]
fn verificacao_com_reconstrucao_vazia_pode_dar_match() {
    let current = regions();
    let snapshot = snapshot_for(&current, vec![]);
    assert!(matches!(
        verify_snapshot(&snapshot, &current),
        ProjectionVerification::Match { .. }
    ));
}

#[test]
fn relatorio_harness_failure_preserva_variante_original() {
    let verification = verify_snapshot_text("schema = 1\ncampo = 2\n", &regions());
    let json = render_verification_json("synthetic.invalid.1", &verification);
    assert!(json.contains("\"result\":\"HARNESS_FAILURE\""));
    assert!(json.contains("UnknownField"));
    assert!(!json.contains("\"result\":\"DRIFT\""));
}

#[test]
fn auditoria_de_pureza_cobre_superficies_proibidas() {
    let source = include_str!("../src/projection_snapshot.rs");
    for forbidden in [
        "std::fs",
        "fs::",
        "OpenOptions",
        "Command",
        "Tcp",
        "Udp",
        "reqwest",
        "env::set",
        "set_current_dir",
        "create_dir",
        "remove_file",
        "remove_dir",
    ] {
        assert!(
            !source.contains(forbidden),
            "superfície proibida: {forbidden}"
        );
    }
}
