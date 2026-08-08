//! Lifecycle explícito de snapshots de projeção (`CANDIDATE → FROZEN`).
//!
//! Este adaptador valida invariantes de domínio e calcula bytes desejados. Toda
//! observação, autorização, proteção stale e escrita passam pelo automation
//! core; não existe `fs::write`, `rename` ou temporário neste módulo.

use crate::automation::{
    apply, check, observe, Allowlist, ApplyReport, Authorization, CheckReport, Failure,
    Outcome as AutomationOutcome, Plan, PlanBuilder, RepoRoot,
};
use crate::nav::CodeRegion;
use crate::nav_projection_recipe::{
    render_recipe, resolve, verify_composed, verify_frozen_dependencies, Library, Recipe,
    RECIPES_DIR, RECIPE_SCHEMA,
};
use crate::nav_projection_snapshot::{
    parse, render, validate_id, HarnessFailure, Measures, Outcome, ProjectionSnapshot,
    SnapshotState, VerifyReport, SNAPSHOTS_DIR, SNAPSHOT_SCHEMA,
};
use crate::nav_projection_store::{ArtifactError, ProjectionStore, StoreFailure};
use std::fmt;

/// Schema público da superfície `pink nav projecao`.
pub const PROJECTION_CLI_SCHEMA: u64 = 1;
pub const PROJECTION_PRODUCER: &str = "nav.projecao.lifecycle";
pub const RECIPE_PREFIX: &str = "normalizacao-corrente-para-";

// @pinker-nav:start trama.projecoes.lifecycle
// @pinker-nav:domain projecoes
// @pinker-nav:layer lifecycle
// @pinker-nav:summary Lifecycle adulto de CANDIDATE: preparar exige predecessor FROZEN e recipe própria vazia, calcula medidas pelo resolvedor real e planeja dois targets; aceitar exige candidato canônico MATCH, preserva todos os campos exceto state e planeja exatamente um target, sempre via automation core e autorização por digest.

#[derive(Debug)]
pub enum ProjectionError {
    Authority(StoreFailure),
    NotFound {
        id: String,
    },
    Harness {
        path: Option<String>,
        message: String,
    },
    Policy {
        message: String,
    },
    Drift {
        report: Box<VerifyReport>,
    },
    Automation(Failure),
    Apply(Box<ApplyReport>),
    VerifyAfterApply {
        message: String,
        written: bool,
    },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectionError::Authority(error) => write!(f, "{error}"),
            ProjectionError::NotFound { id } => write!(f, "projeção '{}' não encontrada", id),
            ProjectionError::Harness { path, message } => match path {
                Some(path) => write!(f, "HARNESS_FAILURE: {}: {}", path, message),
                None => write!(f, "HARNESS_FAILURE: {}", message),
            },
            ProjectionError::Policy { message } => {
                write!(f, "POLICY_VIOLATION: {message}")
            }
            ProjectionError::Drift { report } => write!(
                f,
                "DRIFT: candidato '{}' não corresponde ao catálogo corrente",
                report.snapshot_id
            ),
            ProjectionError::Automation(failure) => write!(f, "{failure}"),
            ProjectionError::Apply(report) => match &report.failure {
                Some(failure) => write!(f, "{failure}"),
                None => write!(f, "aplicação interrompida sem causa"),
            },
            ProjectionError::VerifyAfterApply { message, written } => write!(
                f,
                "VERIFY_AFTER_APPLY_FAILURE (written={}): {}",
                written, message
            ),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl From<StoreFailure> for ProjectionError {
    fn from(value: StoreFailure) -> Self {
        ProjectionError::Authority(value)
    }
}

impl From<Failure> for ProjectionError {
    fn from(value: Failure) -> Self {
        ProjectionError::Automation(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperation {
    Prepare,
    Accept,
}

impl LifecycleOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleOperation::Prepare => "PREPARE_CANDIDATE",
            LifecycleOperation::Accept => "ACCEPT_CANDIDATE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionPlan {
    pub operation: LifecycleOperation,
    pub plan: Plan,
    pub check: CheckReport,
    pub desired_snapshot: ProjectionSnapshot,
    pub desired_recipe: Option<Recipe>,
    recipe_before: Option<Vec<u8>>,
}

impl ProjectionPlan {
    pub fn digest(&self) -> String {
        self.plan.digest()
    }

    pub fn planned_outcome(&self) -> &'static str {
        if self.check.outcome == AutomationOutcome::Match {
            "NO_CHANGE"
        } else {
            match self.operation {
                LifecycleOperation::Prepare => "CANDIDATE_PLANNED",
                LifecycleOperation::Accept => "FROZEN_PLANNED",
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleApply {
    pub outcome: &'static str,
    pub report: ApplyReport,
}

pub fn recipe_id(id: &str) -> String {
    format!("{RECIPE_PREFIX}{id}")
}

pub fn snapshot_path(id: &str) -> String {
    format!("{SNAPSHOTS_DIR}{id}.toml")
}

pub fn recipe_path(id: &str) -> String {
    format!("{RECIPES_DIR}{}.toml", recipe_id(id))
}

pub fn empty_recipe(id: &str) -> Recipe {
    Recipe {
        schema: RECIPE_SCHEMA,
        id: recipe_id(id),
        steps: Vec::new(),
        expected_overrides: 0,
        expected_exclusions: 0,
        rules: Vec::new(),
    }
}

/// Planeja a preparação. Não escreve.
pub fn plan_prepare(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    id: &str,
    predecessor: &str,
    justification: &str,
) -> Result<ProjectionPlan, ProjectionError> {
    validate_lifecycle_input(id, predecessor, justification)?;
    let store = ProjectionStore::load(root.path())?;

    if let Some(error) = store.snapshot_error(id) {
        return Err(policy_artifact(
            error,
            "snapshot candidato existente inválido",
        ));
    }
    if let Some(existing) = store.snapshot(id) {
        if existing.snapshot.state == SnapshotState::Frozen {
            return Err(policy(format!(
                "'{}' já é FROZEN; preparar nunca sobrescreve história",
                id
            )));
        }
        if !existing.canonical {
            return Err(policy(format!(
                "candidate '{}' existente não está na forma canônica",
                id
            )));
        }
        validate_candidate_shape(&existing.snapshot, id)?;
    }

    let predecessor_snapshot = match store.snapshot(predecessor) {
        Some(stored) => &stored.snapshot,
        None if store.snapshot_error(predecessor).is_some() => {
            return Err(policy(format!(
                "predecessor '{}' é estruturalmente inválido",
                predecessor
            )))
        }
        None => return Err(policy(format!("predecessor '{}' não existe", predecessor))),
    };
    if predecessor_snapshot.state != SnapshotState::Frozen {
        return Err(policy(format!(
            "predecessor '{}' deve ser FROZEN",
            predecessor
        )));
    }

    let desired_recipe = empty_recipe(id);
    validate_existing_recipe(&store, id, &desired_recipe)?;

    let mut candidate = ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA,
        id: id.to_string(),
        state: SnapshotState::Candidate,
        predecessor: Some(predecessor.to_string()),
        justification: Some(justification.to_string()),
        measures: Measures {
            regions: 0,
            length: 0,
            fnv1a64: 0,
        },
        expected_overrides: 0,
        expected_exclusions: 0,
        base_snapshot: None,
        recipes: vec![desired_recipe.id.clone()],
        rules: Vec::new(),
    };

    let provisional = provisional_library(&store, id, &candidate, &desired_recipe)?;
    candidate.measures = resolve(&provisional, id, catalog)
        .map_err(harness_failure)?
        .measures();
    // O round-trip aplica a validação estrutural completa ao modelo construído.
    candidate = parse(&render(&candidate)).map_err(harness_failure)?;

    let snapshot_target = snapshot_path(id);
    let recipe_target = recipe_path(id);
    let allowlist = Allowlist::new(&[snapshot_target.as_str(), recipe_target.as_str()])
        .map_err(|cause| policy(cause.to_string()))?;
    let plan = PlanBuilder::new(PROJECTION_PRODUCER, allowlist)
        .desire(&snapshot_target, render(&candidate).into_bytes())?
        .desire(&recipe_target, render_recipe(&desired_recipe).into_bytes())?
        .build()?;
    let observed = observe(root, &plan)?;
    let report = check(&plan, &observed)?;
    Ok(ProjectionPlan {
        operation: LifecycleOperation::Prepare,
        plan,
        check: report,
        desired_snapshot: candidate,
        desired_recipe: Some(desired_recipe),
        recipe_before: None,
    })
}

/// Aplica uma preparação recalculada e verifica os dois artefatos no disco.
pub fn apply_prepare(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    planning: &ProjectionPlan,
    digest: &str,
) -> Result<LifecycleApply, ProjectionError> {
    if planning.operation != LifecycleOperation::Prepare {
        return Err(policy("plano não pertence a preparar".to_string()));
    }
    let report = apply(
        root,
        &planning.plan,
        &Authorization::for_digest(digest),
        &planning.check,
    );
    if report.failure.is_some() {
        return Err(ProjectionError::Apply(Box::new(report)));
    }
    verify_prepared(root, catalog, planning).map_err(|message| {
        ProjectionError::VerifyAfterApply {
            message,
            written: !report.applied.is_empty(),
        }
    })?;
    let outcome = if report.outcome == Some(AutomationOutcome::NoChange) {
        "NO_CHANGE"
    } else {
        "CANDIDATE_PREPARED"
    };
    Ok(LifecycleApply { outcome, report })
}

/// Planeja a transição única CANDIDATE → FROZEN. Não escreve.
pub fn plan_accept(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    id: &str,
) -> Result<ProjectionPlan, ProjectionError> {
    validate_id(id, "id").map_err(harness_failure)?;
    let store = ProjectionStore::load(root.path())?;
    if let Some(error) = store.snapshot_error(id) {
        return Err(artifact_harness(error));
    }
    if let Some(error) = store.errors().first() {
        return Err(artifact_harness(error));
    }
    let stored = store
        .snapshot(id)
        .ok_or_else(|| ProjectionError::NotFound { id: id.to_string() })?;
    let candidate = &stored.snapshot;
    if candidate.state != SnapshotState::Candidate {
        return Err(policy(format!(
            "'{}' está em {}; aceitar é uma transição única de CANDIDATE",
            id, candidate.state
        )));
    }
    if !stored.canonical {
        return Err(policy(format!(
            "candidate '{}' não está na forma canônica",
            id
        )));
    }
    validate_candidate_shape(candidate, id)?;
    validate_predecessor(&store, candidate)?;
    let desired_recipe = empty_recipe(id);
    let recipe = validate_existing_recipe(&store, id, &desired_recipe)?
        .ok_or_else(|| policy(format!("recipe própria de '{}' está ausente", id)))?;

    let library = store.library().map_err(harness_failure)?;
    verify_frozen_dependencies(&library).map_err(harness_failure)?;
    let verification = verify_composed(&library, id, catalog);
    match &verification.outcome {
        Outcome::Match => {}
        Outcome::Drift(_) => {
            return Err(ProjectionError::Drift {
                report: Box::new(verification),
            })
        }
        Outcome::HarnessFailure(failure) => return Err(harness_failure(failure.clone())),
    }

    let mut frozen = candidate.clone();
    frozen.state = SnapshotState::Frozen;
    ensure_only_state_changed(candidate, &frozen)?;
    let target = snapshot_path(id);
    let allowlist =
        Allowlist::new(&[target.as_str()]).map_err(|cause| policy(cause.to_string()))?;
    let plan = PlanBuilder::new(PROJECTION_PRODUCER, allowlist)
        .desire(&target, render(&frozen).into_bytes())?
        .build()?;
    if plan.targets().len() != 1 {
        return Err(policy(
            "aceitar deve possuir exatamente um target".to_string(),
        ));
    }
    let observed = observe(root, &plan)?;
    let report = check(&plan, &observed)?;
    Ok(ProjectionPlan {
        operation: LifecycleOperation::Accept,
        plan,
        check: report,
        desired_snapshot: frozen,
        desired_recipe: None,
        recipe_before: Some(recipe.bytes.clone()),
    })
}

/// Aplica uma aceitação e executa a verificação pós-escrita de domínio.
pub fn apply_accept(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    planning: &ProjectionPlan,
    digest: &str,
) -> Result<LifecycleApply, ProjectionError> {
    if planning.operation != LifecycleOperation::Accept {
        return Err(policy("plano não pertence a aceitar".to_string()));
    }
    let report = apply(
        root,
        &planning.plan,
        &Authorization::for_digest(digest),
        &planning.check,
    );
    if report.failure.is_some() {
        return Err(ProjectionError::Apply(Box::new(report)));
    }
    verify_accepted(root, catalog, planning).map_err(|message| {
        ProjectionError::VerifyAfterApply {
            message,
            written: !report.applied.is_empty(),
        }
    })?;
    Ok(LifecycleApply {
        outcome: "FROZEN_ACCEPTED",
        report,
    })
}

fn verify_prepared(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    planning: &ProjectionPlan,
) -> Result<(), String> {
    let store = ProjectionStore::load(root.path()).map_err(|error| error.to_string())?;
    let id = &planning.desired_snapshot.id;
    let stored = store
        .snapshot(id)
        .ok_or_else(|| format!("candidate '{}' ausente depois do apply", id))?;
    if stored.snapshot != planning.desired_snapshot || !stored.canonical {
        return Err(format!("candidate '{}' divergiu dos bytes planejados", id));
    }
    let desired_recipe = planning
        .desired_recipe
        .as_ref()
        .ok_or_else(|| "recipe desejada ausente no plano de preparar".to_string())?;
    let recipe = store
        .recipe(&desired_recipe.id)
        .ok_or_else(|| format!("recipe '{}' ausente depois do apply", desired_recipe.id))?;
    if recipe.recipe != *desired_recipe || !recipe.canonical {
        return Err(format!(
            "recipe '{}' divergiu dos bytes planejados",
            recipe.recipe.id
        ));
    }
    let library = store.library().map_err(|error| error.to_string())?;
    let verification = verify_composed(&library, id, catalog);
    if verification.outcome != Outcome::Match {
        return Err(format!(
            "candidate '{}' não verifica MATCH depois do apply: {}",
            id,
            verification.outcome.as_str()
        ));
    }
    Ok(())
}

fn verify_accepted(
    root: &RepoRoot,
    catalog: &[CodeRegion],
    planning: &ProjectionPlan,
) -> Result<(), String> {
    let store = ProjectionStore::load(root.path()).map_err(|error| error.to_string())?;
    if let Some(error) = store.errors().first() {
        return Err(format!("{}: {}", error.path, error.message));
    }
    let id = &planning.desired_snapshot.id;
    let stored = store
        .snapshot(id)
        .ok_or_else(|| format!("snapshot '{}' ausente depois da aceitação", id))?;
    if stored.snapshot != planning.desired_snapshot
        || stored.snapshot.state != SnapshotState::Frozen
        || !stored.canonical
    {
        return Err(format!(
            "snapshot '{}' não reabriu como FROZEN canônico",
            id
        ));
    }
    let own_recipe = store
        .recipe(&recipe_id(id))
        .ok_or_else(|| format!("recipe própria de '{}' desapareceu", id))?;
    if planning.recipe_before.as_deref() != Some(own_recipe.bytes.as_slice()) {
        return Err(format!("aceitar modificou a recipe própria de '{}'", id));
    }
    let library = store.library().map_err(|error| error.to_string())?;
    verify_frozen_dependencies(&library).map_err(|error| error.to_string())?;
    let verification = verify_composed(&library, id, catalog);
    if verification.outcome != Outcome::Match {
        return Err(format!(
            "snapshot '{}' não verifica MATCH depois da aceitação: {}",
            id,
            verification.outcome.as_str()
        ));
    }
    Ok(())
}

fn validate_lifecycle_input(
    id: &str,
    predecessor: &str,
    justification: &str,
) -> Result<(), ProjectionError> {
    validate_id(id, "id").map_err(harness_failure)?;
    if predecessor.trim().is_empty() {
        return Err(policy("--predecessor é obrigatório".to_string()));
    }
    validate_id(predecessor, "predecessor")
        .map_err(|_| policy("predecessor inválido".to_string()))?;
    if predecessor == id {
        return Err(policy(
            "candidate não pode ser o próprio predecessor".to_string(),
        ));
    }
    if justification.trim().is_empty() {
        return Err(policy("--justificativa deve ser não vazia".to_string()));
    }
    Ok(())
}

fn validate_candidate_shape(
    snapshot: &ProjectionSnapshot,
    id: &str,
) -> Result<(), ProjectionError> {
    let expected_recipe = recipe_id(id);
    if snapshot.schema != SNAPSHOT_SCHEMA
        || snapshot.base_snapshot.is_some()
        || snapshot.recipes != [expected_recipe]
        || snapshot.expected_overrides != 0
        || snapshot.expected_exclusions != 0
        || !snapshot.rules.is_empty()
        || snapshot.predecessor.is_none()
        || snapshot
            .justification
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
    {
        return Err(policy(format!(
            "candidate '{}' não cumpre a forma de nova raiz do Stage E",
            id
        )));
    }
    Ok(())
}

fn validate_predecessor(
    store: &ProjectionStore,
    candidate: &ProjectionSnapshot,
) -> Result<(), ProjectionError> {
    let predecessor = candidate
        .predecessor
        .as_deref()
        .ok_or_else(|| policy("candidate sem predecessor explícito".to_string()))?;
    if predecessor == candidate.id {
        return Err(policy("candidate é o próprio predecessor".to_string()));
    }
    let stored = store
        .snapshot(predecessor)
        .ok_or_else(|| policy(format!("predecessor '{}' não existe", predecessor)))?;
    if stored.snapshot.state != SnapshotState::Frozen {
        return Err(policy(format!(
            "predecessor '{}' deve ser FROZEN",
            predecessor
        )));
    }
    Ok(())
}

fn validate_existing_recipe<'a>(
    store: &'a ProjectionStore,
    id: &str,
    desired: &Recipe,
) -> Result<Option<&'a crate::nav_projection_store::StoredRecipe>, ProjectionError> {
    if let Some(error) = store.recipe_error(&desired.id) {
        return Err(policy_artifact(error, "recipe própria inválida"));
    }
    let Some(stored) = store.recipe(&desired.id) else {
        return Ok(None);
    };
    if stored.recipe != *desired || !stored.canonical {
        return Err(policy(format!(
            "recipe própria de '{}' contém manutenção semântica, id divergente ou forma não canônica; sobrescrita automática recusada",
            id
        )));
    }
    Ok(Some(stored))
}

fn provisional_library(
    store: &ProjectionStore,
    id: &str,
    candidate: &ProjectionSnapshot,
    recipe: &Recipe,
) -> Result<Library, ProjectionError> {
    let mut library = Library::new();
    for stored in store.recipes() {
        if stored.recipe.id != recipe.id {
            library = library
                .with_recipe(stored.recipe.clone())
                .map_err(harness_failure)?;
        }
    }
    library = library
        .with_recipe(recipe.clone())
        .map_err(harness_failure)?;
    for stored in store.snapshots() {
        if stored.snapshot.id != id {
            library = library
                .with_snapshot(stored.snapshot.clone())
                .map_err(harness_failure)?;
        }
    }
    library
        .with_snapshot(candidate.clone())
        .map_err(harness_failure)
}

fn ensure_only_state_changed(
    candidate: &ProjectionSnapshot,
    frozen: &ProjectionSnapshot,
) -> Result<(), ProjectionError> {
    let mut expected = candidate.clone();
    expected.state = SnapshotState::Frozen;
    if &expected != frozen {
        return Err(policy(
            "aceitação tentou modificar campo diferente de state".to_string(),
        ));
    }
    Ok(())
}

fn harness_failure(failure: HarnessFailure) -> ProjectionError {
    ProjectionError::Harness {
        path: None,
        message: failure.to_string().replace('\n', " "),
    }
}

fn artifact_harness(error: &ArtifactError) -> ProjectionError {
    ProjectionError::Harness {
        path: Some(error.path.clone()),
        message: error.message.clone(),
    }
}

fn policy_artifact(error: &ArtifactError, prefix: &str) -> ProjectionError {
    policy(format!("{}: {}: {}", prefix, error.path, error.message))
}

fn policy(message: String) -> ProjectionError {
    ProjectionError::Policy { message }
}

// @pinker-nav:end trama.projecoes.lifecycle
