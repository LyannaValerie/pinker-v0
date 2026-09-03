//! Modelos semânticos e renderização da superfície `pink nav projecao`.

use crate::automation::{ApplyReport, ChangeKind, CheckReport};
use crate::nav::CodeRegion;
use crate::nav_projection_lifecycle::{
    LifecycleApply, ProjectionError, ProjectionPlan, PROJECTION_CLI_SCHEMA,
};
use crate::nav_projection_recipe::{verify_composed, Library};
use crate::nav_projection_snapshot::{
    Divergence, HarnessFailure, Measures, Outcome, ProjectionSnapshot, Rule, VerifyReport,
};
use crate::nav_projection_store::{ArtifactError, ProjectionStore, StoredSnapshot};
use std::collections::{BTreeMap, BTreeSet};

// @pinker-nav:start trama.projecoes.relatorios
// @pinker-nav:domain projecoes
// @pinker-nav:layer relatorios
// @pinker-nav:summary Relatórios versionados da CLI de projeções: inventário, definição versus observado, verificação composta com causas raiz e dependentes bloqueados, e resumos de plano/apply derivados dos mesmos modelos para texto humano e JSON determinísticos sem paths absolutos.

#[derive(Debug, Clone)]
pub struct VerificationItem {
    pub path: String,
    pub report: VerifyReport,
}

#[derive(Debug, Clone)]
pub struct BlockedItem {
    pub snapshot: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CauseGroup {
    pub cause: String,
    pub blocked: Vec<BlockedItem>,
}

#[derive(Debug, Clone)]
pub struct VerificationBatch {
    pub results: Vec<VerificationItem>,
    pub causes: Vec<CauseGroup>,
    pub errors: Vec<ArtifactError>,
}

impl VerificationBatch {
    pub fn outcome(&self) -> &'static str {
        if !self.errors.is_empty()
            || !self.causes.is_empty()
            || self
                .results
                .iter()
                .any(|item| matches!(item.report.outcome, Outcome::HarnessFailure(_)))
        {
            "HARNESS_FAILURE"
        } else if self
            .results
            .iter()
            .any(|item| matches!(item.report.outcome, Outcome::Drift(_)))
        {
            "DRIFT"
        } else {
            "MATCH"
        }
    }
}

pub fn verify_one(
    store: &ProjectionStore,
    id: &str,
    catalog: &[CodeRegion],
) -> Result<VerificationItem, ProjectionError> {
    if let Some(error) = store.snapshot_error(id) {
        return Err(ProjectionError::Harness {
            path: Some(error.path.clone()),
            message: error.message.clone(),
        });
    }
    let stored = store
        .snapshot(id)
        .ok_or_else(|| ProjectionError::NotFound { id: id.to_string() })?;
    let library = store.library().map_err(|error| ProjectionError::Harness {
        path: None,
        message: error.to_string(),
    })?;
    Ok(VerificationItem {
        path: stored.path.clone(),
        report: verify_composed(&library, id, catalog),
    })
}

pub fn verify_all(store: &ProjectionStore, catalog: &[CodeRegion]) -> VerificationBatch {
    let library = store.library().unwrap_or_else(|_| Library::new());
    let mut all = Vec::new();
    for stored in store.snapshots() {
        all.push(VerificationItem {
            path: stored.path.clone(),
            report: verify_composed(&library, &stored.snapshot.id, catalog),
        });
    }

    let mut blocked_by: BTreeMap<String, Vec<BlockedItem>> = BTreeMap::new();
    let mut blocked_ids = BTreeSet::new();
    for item in &all {
        if let Outcome::HarnessFailure(HarnessFailure::BaseMeasuresDiverged { id, .. }) =
            &item.report.outcome
        {
            blocked_ids.insert(item.report.snapshot_id.clone());
            blocked_by.entry(id.clone()).or_default().push(BlockedItem {
                snapshot: item.report.snapshot_id.clone(),
                path: item.path.clone(),
                reason: format!("base '{}' não corresponde às próprias medidas", id),
            });
        }
    }
    let causes = blocked_by
        .into_iter()
        .map(|(cause, mut blocked)| {
            blocked.sort_by(|a, b| a.snapshot.cmp(&b.snapshot));
            CauseGroup { cause, blocked }
        })
        .collect();
    let results = all
        .into_iter()
        .filter(|item| !blocked_ids.contains(&item.report.snapshot_id))
        .collect();
    VerificationBatch {
        results,
        causes,
        errors: store.errors().to_vec(),
    }
}

pub fn render_inventory_human(store: &ProjectionStore) -> String {
    let mut out = String::new();
    out.push_str("projeções históricas\n");
    for stored in store.snapshots() {
        let snapshot = &stored.snapshot;
        out.push_str(&format!(
            "- {} [{}] {} regioes={} comprimento={} {}\n",
            snapshot.id,
            snapshot.state,
            stored.path,
            snapshot.measures.regions,
            snapshot.measures.length,
            snapshot.measures.fnv1a64_canonical()
        ));
        out.push_str(&format!(
            "  predecessor={} base={} recipes={}\n",
            option_human(snapshot.predecessor.as_deref()),
            option_human(snapshot.base_snapshot.as_deref()),
            if snapshot.recipes.is_empty() {
                "—".to_string()
            } else {
                snapshot.recipes.join(",")
            }
        ));
    }
    for error in store.errors() {
        out.push_str(&format!(
            "erro {} [{}]: {}\n",
            error.path,
            error.kind.as_str(),
            error.message
        ));
    }
    out
}

pub fn render_inventory_json(store: &ProjectionStore) -> String {
    let snapshots: Vec<String> = store.snapshots().map(inventory_snapshot_json).collect();
    format!(
        "{{\"schema\":{},\"command\":\"listar\",\"outcome\":{},\"snapshots\":[{}],\"errors\":[{}]}}",
        PROJECTION_CLI_SCHEMA,
        json_string(if store.errors().is_empty() { "MATCH" } else { "HARNESS_FAILURE" }),
        snapshots.join(","),
        errors_json(store.errors())
    )
}

pub fn render_show_human(stored: &StoredSnapshot, observed: Option<&VerifyReport>) -> String {
    let snapshot = &stored.snapshot;
    let mut out = String::new();
    out.push_str("definicao\n");
    out.push_str(&definition_human(snapshot, &stored.path));
    if let Some(report) = observed {
        out.push_str("observado\n");
        out.push_str(&verification_human(report));
    }
    out
}

pub fn render_show_json(stored: &StoredSnapshot, observed: Option<&VerifyReport>) -> String {
    format!(
        "{{\"schema\":{},\"command\":\"mostrar\",\"definicao\":{},\"observado\":{}}}",
        PROJECTION_CLI_SCHEMA,
        definition_json(&stored.snapshot, &stored.path),
        observed
            .map(verification_json)
            .unwrap_or_else(|| "null".to_string())
    )
}

pub fn render_verification_human(batch: &VerificationBatch) -> String {
    let mut out = format!("verificar: {}\n", batch.outcome());
    for result in &batch.results {
        out.push_str(&format!(
            "{} {}\n",
            result.path,
            result.report.outcome.as_str()
        ));
        out.push_str(&verification_human(&result.report));
    }
    for group in &batch.causes {
        out.push_str(&format!("causa {}\n", group.cause));
        for blocked in &group.blocked {
            out.push_str(&format!(
                "  bloqueado {} {}: {}\n",
                blocked.snapshot, blocked.path, blocked.reason
            ));
        }
    }
    for error in &batch.errors {
        out.push_str(&format!("erro {}: {}\n", error.path, error.message));
    }
    out
}

pub fn render_verification_json(batch: &VerificationBatch) -> String {
    let results: Vec<String> = batch
        .results
        .iter()
        .map(|item| {
            format!(
                "{{\"path\":{},\"verification\":{}}}",
                json_string(&item.path),
                verification_json(&item.report)
            )
        })
        .collect();
    let causes: Vec<String> = batch
        .causes
        .iter()
        .map(|group| {
            let blocked: Vec<String> = group
                .blocked
                .iter()
                .map(|item| {
                    format!(
                        "{{\"snapshot\":{},\"path\":{},\"reason\":{}}}",
                        json_string(&item.snapshot),
                        json_string(&item.path),
                        json_string(&item.reason)
                    )
                })
                .collect();
            format!(
                "{{\"cause\":{},\"blocked\":[{}]}}",
                json_string(&group.cause),
                blocked.join(",")
            )
        })
        .collect();
    format!(
        "{{\"schema\":{},\"command\":\"verificar\",\"outcome\":{},\"results\":[{}],\"causes\":[{}],\"errors\":[{}]}}",
        PROJECTION_CLI_SCHEMA,
        json_string(batch.outcome()),
        results.join(","),
        causes.join(","),
        errors_json(&batch.errors)
    )
}

pub fn render_plan_human(planning: &ProjectionPlan) -> String {
    let mut out = format!(
        "{}\noperacao: {}\ndigest: {}\n",
        planning.planned_outcome(),
        planning.operation.as_str(),
        planning.digest()
    );
    append_targets_human(&mut out, &planning.check);
    out
}

pub fn render_plan_json(command: &str, planning: &ProjectionPlan) -> String {
    format!(
        "{{\"schema\":{},\"command\":{},\"outcome\":{},\"operation\":{},\"digest\":{},\"summary\":{},\"targets\":{}}}",
        PROJECTION_CLI_SCHEMA,
        json_string(command),
        json_string(planning.planned_outcome()),
        json_string(planning.operation.as_str()),
        json_string(&planning.digest()),
        check_summary_json(&planning.check),
        check_targets_json(&planning.check)
    )
}

pub fn render_apply_human(applied: &LifecycleApply) -> String {
    let mut out = format!(
        "{}\ndigest: {}\n",
        applied.outcome, applied.report.plan_digest
    );
    append_apply_human(&mut out, &applied.report);
    out
}

pub fn render_apply_json(command: &str, applied: &LifecycleApply) -> String {
    format!(
        "{{\"schema\":{},\"command\":{},\"outcome\":{},\"digest\":{},\"applied\":{},\"failed\":{},\"not_attempted\":{},\"rollback_performed\":{}}}",
        PROJECTION_CLI_SCHEMA,
        json_string(command),
        json_string(applied.outcome),
        json_string(&applied.report.plan_digest),
        string_array_json(&applied.report.applied),
        applied.report.failed.as_deref().map(json_string).unwrap_or_else(|| "null".to_string()),
        string_array_json(&applied.report.not_attempted),
        applied.report.rollback_performed
    )
}

pub fn render_error_json(command: &str, error: &ProjectionError) -> String {
    let (outcome, message, written) = match error {
        ProjectionError::Authority(_) => ("IO_FAILURE", error.to_string(), false),
        ProjectionError::NotFound { .. } => ("NOT_FOUND", error.to_string(), false),
        ProjectionError::Harness { .. } => ("HARNESS_FAILURE", error.to_string(), false),
        ProjectionError::Policy { .. } => ("POLICY_VIOLATION", error.to_string(), false),
        ProjectionError::Drift { .. } => ("DRIFT", error.to_string(), false),
        ProjectionError::Automation(failure) => (failure.code(), failure.to_string(), false),
        ProjectionError::Apply(report) => (
            report
                .failure
                .as_ref()
                .map_or("IO_FAILURE", crate::automation::Failure::code),
            error.to_string(),
            !report.applied.is_empty(),
        ),
        ProjectionError::VerifyAfterApply { written, .. } => {
            ("VERIFY_AFTER_APPLY_FAILURE", error.to_string(), *written)
        }
    };
    format!(
        "{{\"schema\":{},\"command\":{},\"outcome\":{},\"message\":{},\"written\":{}}}",
        PROJECTION_CLI_SCHEMA,
        json_string(command),
        json_string(outcome),
        json_string(&message),
        written
    )
}

fn inventory_snapshot_json(stored: &StoredSnapshot) -> String {
    let snapshot = &stored.snapshot;
    format!(
        "{{\"id\":{},\"state\":{},\"predecessor\":{},\"base_snapshot\":{},\"recipes\":{},\"measures\":{},\"path\":{}}}",
        json_string(&snapshot.id),
        json_string(snapshot.state.as_str()),
        option_json(snapshot.predecessor.as_deref()),
        option_json(snapshot.base_snapshot.as_deref()),
        string_array_json(&snapshot.recipes),
        measures_json(&snapshot.measures),
        json_string(&stored.path)
    )
}

/// O terceiro orçamento, emitido apenas quando existe.
///
/// Os dois antigos saem sempre porque sempre existiram. Este segue a mesma regra
/// do artefato TOML — ausente significa zero —, e é o que mantém a saída de um
/// snapshot anterior ao schema 4 byte a byte como era.
fn materializations_human(snapshot: &ProjectionSnapshot) -> String {
    if snapshot.expected_materializations > 0 {
        format!(
            "expected_materializations: {}\n",
            snapshot.expected_materializations
        )
    } else {
        String::new()
    }
}

fn materializations_json(snapshot: &ProjectionSnapshot) -> String {
    if snapshot.expected_materializations > 0 {
        format!(
            ",\"expected_materializations\":{}",
            snapshot.expected_materializations
        )
    } else {
        String::new()
    }
}

fn definition_human(snapshot: &ProjectionSnapshot, path: &str) -> String {
    let mut out = format!(
        "artifact_schema: {}\nid: {}\nstate: {}\npredecessor: {}\njustification: {}\nbase_snapshot: {}\nrecipes: {}\nexpected_overrides: {}\nexpected_exclusions: {}\n{}measures: regioes={} comprimento={} {}\npath: {}\n",
        snapshot.schema,
        snapshot.id,
        snapshot.state,
        option_human(snapshot.predecessor.as_deref()),
        option_human(snapshot.justification.as_deref()),
        option_human(snapshot.base_snapshot.as_deref()),
        if snapshot.recipes.is_empty() { "—".to_string() } else { snapshot.recipes.join(",") },
        snapshot.expected_overrides,
        snapshot.expected_exclusions,
        materializations_human(snapshot),
        snapshot.measures.regions,
        snapshot.measures.length,
        snapshot.measures.fnv1a64_canonical(),
        path
    );
    for rule in &snapshot.rules {
        out.push_str(&format!(
            "rule: {} {} expected_matches={}\n",
            rule.op(),
            rule.selector(),
            rule.expected_matches()
        ));
    }
    out
}

fn definition_json(snapshot: &ProjectionSnapshot, path: &str) -> String {
    let rules: Vec<String> = snapshot.rules.iter().map(rule_json).collect();
    format!(
        "{{\"artifact_schema\":{},\"id\":{},\"state\":{},\"predecessor\":{},\"justification\":{},\"base_snapshot\":{},\"recipes\":{},\"expected_overrides\":{},\"expected_exclusions\":{}{},\"rules\":[{}],\"measures\":{},\"path\":{}}}",
        snapshot.schema,
        json_string(&snapshot.id),
        json_string(snapshot.state.as_str()),
        option_json(snapshot.predecessor.as_deref()),
        option_json(snapshot.justification.as_deref()),
        option_json(snapshot.base_snapshot.as_deref()),
        string_array_json(&snapshot.recipes),
        snapshot.expected_overrides,
        snapshot.expected_exclusions,
        materializations_json(snapshot),
        rules.join(","),
        measures_json(&snapshot.measures),
        json_string(path)
    )
}

fn verification_human(report: &VerifyReport) -> String {
    let mut out = format!(
        "id={} state={} outcome={}\nesperado: regioes={} comprimento={} {}\n",
        report.snapshot_id,
        report.state,
        report.outcome.as_str(),
        report.expected.regions,
        report.expected.length,
        report.expected.fnv1a64_canonical()
    );
    match report.observed {
        Some(observed) => out.push_str(&format!(
            "observado: regioes={} comprimento={} {}\n",
            observed.regions,
            observed.length,
            observed.fnv1a64_canonical()
        )),
        None => out.push_str("observado: —\n"),
    }
    if let Outcome::HarnessFailure(failure) = &report.outcome {
        out.push_str(&format!("falha: {} {}\n", failure.code(), failure));
    }
    out
}

fn verification_json(report: &VerifyReport) -> String {
    let divergences: Vec<String> = match &report.outcome {
        Outcome::Drift(items) => items.iter().map(divergence_json).collect(),
        _ => Vec::new(),
    };
    let failure = match &report.outcome {
        Outcome::HarnessFailure(failure) => format!(
            "{{\"code\":{},\"message\":{}}}",
            json_string(failure.code()),
            json_string(&failure.to_string().replace('\n', " "))
        ),
        _ => "null".to_string(),
    };
    format!(
        "{{\"snapshot\":{},\"state\":{},\"outcome\":{},\"expected\":{},\"observed\":{},\"divergences\":[{}],\"failure\":{}}}",
        json_string(&report.snapshot_id),
        json_string(report.state.as_str()),
        json_string(report.outcome.as_str()),
        measures_json(&report.expected),
        report.observed.as_ref().map(measures_json).unwrap_or_else(|| "null".to_string()),
        divergences.join(","),
        failure
    )
}

fn rule_json(rule: &Rule) -> String {
    let mut fields = vec![
        format!("\"op\":{}", json_string(rule.op())),
        format!("\"selector\":{}", json_string(rule.selector())),
        format!("\"expected_matches\":{}", rule.expected_matches()),
    ];
    match rule {
        Rule::OverrideHash {
            from,
            to,
            expect_file,
            expect_domain,
            expect_layer,
            ..
        } => {
            fields.push(format!("\"from\":{}", json_string(from)));
            fields.push(format!("\"to\":{}", json_string(to)));
            fields.push(format!(
                "\"expect_file\":{}",
                option_json(expect_file.as_deref())
            ));
            fields.push(format!(
                "\"expect_domain\":{}",
                option_json(expect_domain.as_deref())
            ));
            fields.push(format!(
                "\"expect_layer\":{}",
                option_json(expect_layer.as_deref())
            ));
        }
        Rule::OverrideRegion {
            from_hash,
            to_hash,
            from_summary,
            to_summary,
            expect_file,
            to_file,
            expect_domain,
            expect_layer,
            ..
        } => {
            fields.push(format!(
                "\"from_hash\":{}",
                option_json(from_hash.as_deref())
            ));
            fields.push(format!("\"to_hash\":{}", option_json(to_hash.as_deref())));
            fields.push(format!(
                "\"from_summary\":{}",
                option_json(from_summary.as_deref())
            ));
            fields.push(format!(
                "\"to_summary\":{}",
                option_json(to_summary.as_deref())
            ));
            fields.push(format!(
                "\"expect_file\":{}",
                option_json(expect_file.as_deref())
            ));
            // Campo aditivo: como `expect_domain` e `expect_layer`, sai `null`
            // quando ausente. Por isso não move `PROJECTION_CLI_SCHEMA` — o
            // relatório continua sendo a mesma forma, com um campo opcional a
            // mais que o modelo passou a ter.
            fields.push(format!("\"to_file\":{}", option_json(to_file.as_deref())));
            fields.push(format!(
                "\"expect_domain\":{}",
                option_json(expect_domain.as_deref())
            ));
            fields.push(format!(
                "\"expect_layer\":{}",
                option_json(expect_layer.as_deref())
            ));
        }
        _ => {}
    }
    format!("{{{}}}", fields.join(","))
}

fn divergence_json(divergence: &Divergence) -> String {
    format!(
        "{{\"measure\":{},\"expected\":{},\"observed\":{}}}",
        json_string(divergence.measure),
        json_string(&divergence.expected),
        json_string(&divergence.observed)
    )
}

fn errors_json(errors: &[ArtifactError]) -> String {
    errors
        .iter()
        .map(|error| {
            format!(
                "{{\"path\":{},\"kind\":{},\"message\":{}}}",
                json_string(&error.path),
                json_string(error.kind.as_str()),
                json_string(&error.message)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn check_summary_json(report: &CheckReport) -> String {
    let summary = report.summary();
    format!(
        "{{\"create\":{},\"replace\":{},\"remove\":{},\"no_change\":{}}}",
        summary[0].1, summary[1].1, summary[2].1, summary[3].1
    )
}

fn check_targets_json(report: &CheckReport) -> String {
    report
        .targets
        .iter()
        .map(|target| {
            format!(
                "{{\"path\":{},\"change\":{}}}",
                json_string(&target.path),
                json_string(target.change.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join(",")
        .pipe(|items| format!("[{items}]"))
}

fn append_targets_human(out: &mut String, report: &CheckReport) {
    for target in &report.targets {
        out.push_str(&format!(
            "target: {} {}\n",
            target.path,
            target.change.as_str()
        ));
    }
}

fn append_apply_human(out: &mut String, report: &ApplyReport) {
    for path in &report.applied {
        out.push_str(&format!("applied: {}\n", path));
    }
    if let Some(path) = &report.failed {
        out.push_str(&format!("failed: {}\n", path));
    }
    for path in &report.not_attempted {
        out.push_str(&format!("not_attempted: {}\n", path));
    }
    out.push_str(&format!(
        "rollback_performed: {}\n",
        report.rollback_performed
    ));
}

fn measures_json(measures: &Measures) -> String {
    format!(
        "{{\"regions\":{},\"length\":{},\"fnv1a64\":{}}}",
        measures.regions,
        measures.length,
        json_string(&measures.fnv1a64_canonical())
    )
}

fn option_json(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn option_human(value: Option<&str>) -> String {
    value.unwrap_or("—").to_string()
}

fn string_array_json(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32))
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}
impl<T> Pipe for T {}

#[allow(dead_code)]
fn _change_kind_is_exhaustive(kind: ChangeKind) -> &'static str {
    kind.as_str()
}

// @pinker-nav:end trama.projecoes.relatorios
