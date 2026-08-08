//! Estado consolidado, estruturado e somente leitura da Pinker (#387).
//!
//! Este módulo não cria autoridade. Ele adapta os modelos observacionais da
//! Trama, documentação, projeções, automation core e `pink agente` para uma
//! representação única consumível diretamente por interfaces internas.

use crate::agent::{self, AgentTerminalStatus};
use crate::automation::RepoRoot;
use crate::doc::{self, DocConfig};
use crate::doc_index::{DocCatalog, DocIndex};
use crate::nav::{self, CodeCatalog};
use crate::nav_projection_report;
use crate::nav_projection_snapshot::SnapshotState;
use crate::nav_projection_store::ProjectionStore;
use std::fmt;
use std::path::Path;

/// Schema público inicial da superfície `pink estado`.
pub const PROJECT_STATE_SCHEMA: u64 = 1;

// @pinker-nav:start project-state.modelo
// @pinker-nav:domain estado
// @pinker-nav:layer modelo
// @pinker-nav:summary Modelo tipado e versionado do estado consolidado: domínios em ordem fixa, estados explícitos, fontes atribuídas, warnings, blockers e operações pendentes sem root absoluto ou dados incidentais.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateStatus {
    Ok,
    Warning,
    Blocked,
    Unknown,
    Unavailable,
    Partial,
}

impl StateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StateStatus::Ok => "OK",
            StateStatus::Warning => "WARNING",
            StateStatus::Blocked => "BLOCKED",
            StateStatus::Unknown => "UNKNOWN",
            StateStatus::Unavailable => "UNAVAILABLE",
            StateStatus::Partial => "PARTIAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    RepoFile,
    Derived,
    LocalCheck,
    AgentSpec,
    Catalog,
    ProjectionStore,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::RepoFile => "repo_file",
            SourceKind::Derived => "derived",
            SourceKind::LocalCheck => "local_check",
            SourceKind::AgentSpec => "agent_spec",
            SourceKind::Catalog => "catalog",
            SourceKind::ProjectionStore => "projection_store",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: SourceKind,
    pub path: Option<String>,
    pub authority: String,
}

impl Source {
    pub fn new(kind: SourceKind, path: Option<&str>, authority: &str) -> Source {
        Source {
            kind,
            path: path.map(ToOwned::to_owned),
            authority: authority.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainId {
    Repository,
    Trama,
    Documentation,
    Projections,
    LocalChecks,
    Agent,
    Diagnostics,
}

impl DomainId {
    pub fn as_str(self) -> &'static str {
        match self {
            DomainId::Repository => "repository",
            DomainId::Trama => "trama",
            DomainId::Documentation => "documentation",
            DomainId::Projections => "projections",
            DomainId::LocalChecks => "local_checks",
            DomainId::Agent => "agent",
            DomainId::Diagnostics => "diagnostics",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityAvailability {
    pub id: String,
    pub status: StateStatus,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryState {
    pub root_discovered: bool,
    pub marker: String,
    pub authorities: Vec<AuthorityAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TramaState {
    pub catalog_path: Option<String>,
    pub catalog_available: bool,
    pub catalog_valid: Option<bool>,
    pub regions: Option<usize>,
    pub source_valid: Option<bool>,
    pub synchronized: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationState {
    pub catalog_path: Option<String>,
    pub catalog_available: bool,
    pub catalog_valid: Option<bool>,
    pub documents: Option<usize>,
    pub sections: Option<usize>,
    pub source_valid: Option<bool>,
    pub known_drift: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionItem {
    pub id: String,
    pub state: String,
    pub path: String,
    pub outcome: String,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionCause {
    pub cause: String,
    pub blocked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionsState {
    pub frozen: usize,
    pub candidate: usize,
    pub recipes: usize,
    pub verification: String,
    pub items: Vec<ProjectionItem>,
    pub causes: Vec<ProjectionCause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCheck {
    pub id: String,
    pub status: StateStatus,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalChecksState {
    pub checks: Vec<LocalCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub configured: bool,
    pub reason: Option<String>,
    pub terminal: Option<String>,
    pub publication: Option<String>,
    pub checks: Vec<AgentCheckState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCheckState {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: String,
    pub domain: DomainId,
    pub status: StateStatus,
    pub summary: String,
    pub reason: String,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsState {
    pub entries: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainDetails {
    Repository(RepositoryState),
    Trama(TramaState),
    Documentation(DocumentationState),
    Projections(ProjectionsState),
    LocalChecks(LocalChecksState),
    Agent(AgentState),
    Diagnostics(DiagnosticsState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainState {
    pub id: DomainId,
    pub status: StateStatus,
    pub source: Source,
    pub details: DomainDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub domain: DomainId,
    pub summary: String,
    pub source: Source,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOperation {
    pub id: String,
    pub domain: DomainId,
    pub kind: String,
    pub summary: String,
    pub source: Source,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectState {
    pub schema: u64,
    pub overall: StateStatus,
    pub domains: Vec<DomainState>,
    pub warnings: Vec<Finding>,
    pub blockers: Vec<Finding>,
    pub pending_operations: Vec<PendingOperation>,
}

impl ProjectState {
    pub fn domain(&self, id: DomainId) -> Option<&DomainState> {
        self.domains.iter().find(|domain| domain.id == id)
    }
}

// @pinker-nav:end project-state.modelo

// @pinker-nav:start project-state.coleta
// @pinker-nav:domain estado
// @pinker-nav:layer adaptadores
// @pinker-nav:summary Coleta somente leitura que reutiliza RepoRoot, verificadores doc/nav, ProjectionStore mais verify_all e o modelo observacional do agente; falhas de um domínio são preservadas sem apagar domínios independentes.

#[derive(Debug)]
pub enum CollectError {
    Root(crate::automation::Failure),
}

impl fmt::Display for CollectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectError::Root(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CollectError {}

struct CollectedDomain {
    domain: DomainState,
    diagnostics: Vec<Diagnostic>,
    warnings: Vec<Finding>,
    blockers: Vec<Finding>,
    pending: Vec<PendingOperation>,
}

impl CollectedDomain {
    fn plain(domain: DomainState) -> CollectedDomain {
        CollectedDomain {
            domain,
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            blockers: Vec::new(),
            pending: Vec::new(),
        }
    }
}

/// Coleta o estado consolidado. Depois que a raiz canônica é estabelecida,
/// falhas pertencem aos domínios e não abortam o relatório.
pub fn collect(repo: &Path, agent_spec: Option<&Path>) -> Result<ProjectState, CollectError> {
    let root = RepoRoot::discover(repo).map_err(CollectError::Root)?;
    let config = DocConfig::load(root.path());

    let trama = collect_trama(&root, config.as_ref().ok());
    let documentation = collect_documentation(&root, config.as_ref().ok());
    let projections = collect_projections(&root);
    let agent = collect_agent(agent_spec);

    let repository = collect_repository(
        config.is_ok(),
        &trama.domain,
        &documentation.domain,
        &projections.domain,
    );
    let local_checks =
        collect_local_checks(&trama.domain, &documentation.domain, &projections.domain);

    let mut collected = vec![
        repository,
        trama,
        documentation,
        projections,
        local_checks,
        agent,
    ];
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();
    let mut blockers = Vec::new();
    let mut pending_operations = Vec::new();
    for item in &mut collected {
        diagnostics.append(&mut item.diagnostics);
        warnings.append(&mut item.warnings);
        blockers.append(&mut item.blockers);
        pending_operations.append(&mut item.pending);
    }
    diagnostics.sort_by(|a, b| a.id.cmp(&b.id));
    warnings.sort_by(|a, b| a.id.cmp(&b.id));
    blockers.sort_by(|a, b| a.id.cmp(&b.id));
    pending_operations.sort_by(|a, b| a.id.cmp(&b.id));

    let diagnostics_status = if diagnostics
        .iter()
        .any(|entry| entry.status == StateStatus::Blocked)
    {
        StateStatus::Blocked
    } else if diagnostics
        .iter()
        .any(|entry| entry.status == StateStatus::Warning)
    {
        StateStatus::Warning
    } else if diagnostics.iter().any(|entry| {
        matches!(
            entry.status,
            StateStatus::Unknown | StateStatus::Unavailable | StateStatus::Partial
        )
    }) {
        StateStatus::Partial
    } else {
        StateStatus::Ok
    };
    collected.push(CollectedDomain::plain(DomainState {
        id: DomainId::Diagnostics,
        status: diagnostics_status,
        source: Source::new(SourceKind::Derived, None, "project-state.diagnostics"),
        details: DomainDetails::Diagnostics(DiagnosticsState {
            entries: diagnostics,
        }),
    }));

    let domains = collected
        .into_iter()
        .map(|item| item.domain)
        .collect::<Vec<_>>();
    let overall = derive_overall(&domains);
    Ok(ProjectState {
        schema: PROJECT_STATE_SCHEMA,
        overall,
        domains,
        warnings,
        blockers,
        pending_operations,
    })
}

fn derive_overall(domains: &[DomainState]) -> StateStatus {
    if domains
        .iter()
        .any(|domain| domain.status == StateStatus::Blocked)
    {
        StateStatus::Blocked
    } else if domains
        .iter()
        .any(|domain| domain.status == StateStatus::Warning)
    {
        StateStatus::Warning
    } else if domains.iter().any(|domain| {
        matches!(
            domain.status,
            StateStatus::Partial | StateStatus::Unknown | StateStatus::Unavailable
        )
    }) {
        StateStatus::Partial
    } else {
        StateStatus::Ok
    }
}

fn collect_repository(
    config_ok: bool,
    trama: &DomainState,
    documentation: &DomainState,
    projections: &DomainState,
) -> CollectedDomain {
    let config_status = if config_ok {
        StateStatus::Ok
    } else {
        StateStatus::Blocked
    };
    let status = if config_ok {
        StateStatus::Ok
    } else {
        StateStatus::Partial
    };
    let mut item = CollectedDomain::plain(DomainState {
        id: DomainId::Repository,
        status,
        source: Source::new(
            SourceKind::RepoFile,
            Some(doc::CONFIG_RELATIVE_PATH),
            "automation.repo-root",
        ),
        details: DomainDetails::Repository(RepositoryState {
            root_discovered: true,
            marker: doc::CONFIG_RELATIVE_PATH.to_string(),
            authorities: vec![
                AuthorityAvailability {
                    id: "doc_config".to_string(),
                    status: config_status,
                    source: Source::new(
                        SourceKind::RepoFile,
                        Some(doc::CONFIG_RELATIVE_PATH),
                        "trama.doc-config",
                    ),
                },
                AuthorityAvailability {
                    id: "code_catalog".to_string(),
                    status: availability_status(trama),
                    source: trama.source.clone(),
                },
                AuthorityAvailability {
                    id: "documentation_catalog".to_string(),
                    status: availability_status(documentation),
                    source: documentation.source.clone(),
                },
                AuthorityAvailability {
                    id: "projection_store".to_string(),
                    status: availability_status(projections),
                    source: projections.source.clone(),
                },
            ],
        }),
    });
    if !config_ok {
        let source = Source::new(
            SourceKind::RepoFile,
            Some(doc::CONFIG_RELATIVE_PATH),
            "trama.doc-config",
        );
        item.blockers.push(Finding {
            id: "repository.doc_config_invalid".to_string(),
            domain: DomainId::Repository,
            summary: "A configuração canônica do repositório não pôde ser validada.".to_string(),
            source: source.clone(),
            reason: "doc_config_invalid".to_string(),
        });
        item.diagnostics.push(Diagnostic {
            id: "repository.doc_config_invalid".to_string(),
            domain: DomainId::Repository,
            status: StateStatus::Blocked,
            summary: "Configuração canônica inválida ou ilegível.".to_string(),
            reason: "doc_config_invalid".to_string(),
            source,
        });
    }
    item
}

fn availability_status(domain: &DomainState) -> StateStatus {
    match domain.status {
        StateStatus::Ok | StateStatus::Warning => StateStatus::Ok,
        StateStatus::Partial | StateStatus::Unknown => StateStatus::Partial,
        StateStatus::Unavailable => StateStatus::Unavailable,
        StateStatus::Blocked => StateStatus::Blocked,
    }
}

fn collect_trama(root: &RepoRoot, config: Option<&DocConfig>) -> CollectedDomain {
    let source = Source::new(
        SourceKind::Catalog,
        config.map(|value| value.generated.code_index.as_str()),
        "trama.code-catalog",
    );
    let Some(config) = config else {
        return CollectedDomain::plain(DomainState {
            id: DomainId::Trama,
            status: StateStatus::Unavailable,
            source,
            details: DomainDetails::Trama(TramaState {
                catalog_path: None,
                catalog_available: false,
                catalog_valid: None,
                regions: None,
                source_valid: None,
                synchronized: None,
            }),
        });
    };
    let catalog_path = config.generated.code_index.clone();
    let catalog_result = CodeCatalog::load(&root.path().join(&catalog_path));
    let catalog_available = root.path().join(&catalog_path).is_file();
    let catalog_valid = catalog_result.is_ok();
    let verification = nav::verify_repository(root.path(), &catalog_path);
    let (regions, source_valid, synchronized, mut status) = match verification.as_ref() {
        Ok(report) => (
            Some(report.index.regions.len()),
            Some(report.source_errors.is_empty()),
            Some(!report.catalog_out_of_date),
            if !report.source_errors.is_empty() {
                StateStatus::Blocked
            } else if report.catalog_out_of_date {
                StateStatus::Warning
            } else {
                StateStatus::Ok
            },
        ),
        Err(_) => (None, None, None, StateStatus::Blocked),
    };
    if !catalog_valid {
        status = StateStatus::Blocked;
    }
    let mut item = CollectedDomain::plain(DomainState {
        id: DomainId::Trama,
        status,
        source: source.clone(),
        details: DomainDetails::Trama(TramaState {
            catalog_path: Some(catalog_path.clone()),
            catalog_available,
            catalog_valid: Some(catalog_valid),
            regions,
            source_valid,
            synchronized,
        }),
    });
    if !catalog_valid {
        let reason = if catalog_available {
            "code_catalog_invalid"
        } else {
            "code_catalog_missing"
        };
        add_blocker_and_diagnostic(
            &mut item,
            "trama.catalog_unavailable",
            DomainId::Trama,
            "O catálogo de código está ausente ou inválido.",
            reason,
            source.clone(),
        );
    }
    if let Ok(report) = &verification {
        if !report.source_errors.is_empty() {
            add_blocker_and_diagnostic(
                &mut item,
                "trama.source_invalid",
                DomainId::Trama,
                "Os marcadores de código não formam um catálogo válido.",
                "code_source_invalid",
                Source::new(SourceKind::LocalCheck, None, "trama.code-index.verify"),
            );
        }
        if report.catalog_out_of_date && catalog_valid {
            add_warning_and_diagnostic(
                &mut item,
                "trama.catalog_drift",
                DomainId::Trama,
                "O catálogo de código diverge das fontes marcadas.",
                "code_catalog_out_of_date",
                source.clone(),
            );
            item.pending.push(PendingOperation {
                id: "trama.sync_code_catalog".to_string(),
                domain: DomainId::Trama,
                kind: "synchronize_catalog".to_string(),
                summary: "Sincronizar o catálogo de código pela autoridade da Trama.".to_string(),
                source: source.clone(),
                reason: "code_catalog_out_of_date".to_string(),
            });
        }
    } else {
        add_blocker_and_diagnostic(
            &mut item,
            "trama.scan_failed",
            DomainId::Trama,
            "As raízes oficiais de código não puderam ser observadas.",
            "code_scan_failed",
            Source::new(SourceKind::LocalCheck, None, "trama.code-index.scan"),
        );
    }
    item
}

fn collect_documentation(root: &RepoRoot, config: Option<&DocConfig>) -> CollectedDomain {
    let source = Source::new(
        SourceKind::Catalog,
        config.map(|value| value.generated.docs_index.as_str()),
        "trama.documentation-catalog",
    );
    let Some(config) = config else {
        let scan = DocIndex::scan(&root.path().join("docs"));
        let (documents, sections, source_valid, status) = match scan {
            Ok(index) => {
                let valid = index.verify().is_empty();
                (
                    Some(index.documents.len()),
                    Some(index.sections.len()),
                    Some(valid),
                    if valid {
                        StateStatus::Partial
                    } else {
                        StateStatus::Blocked
                    },
                )
            }
            Err(_) => (None, None, None, StateStatus::Blocked),
        };
        return CollectedDomain::plain(DomainState {
            id: DomainId::Documentation,
            status,
            source,
            details: DomainDetails::Documentation(DocumentationState {
                catalog_path: None,
                catalog_available: false,
                catalog_valid: None,
                documents,
                sections,
                source_valid,
                known_drift: 0,
            }),
        });
    };
    let catalog_path = config.generated.docs_index.clone();
    let catalog_result = DocCatalog::load(&root.path().join(&catalog_path));
    let catalog_available = root.path().join(&catalog_path).is_file();
    let catalog_valid = catalog_result.is_ok();
    let verification = doc::verify_repository(root.path(), config);
    let (documents, sections, source_valid, known_drift, mut status) = match verification.as_ref() {
        Ok(report) => {
            let structural = !report.source_errors.is_empty()
                || !report.manifest_errors.is_empty()
                || report.projection_error.is_some();
            let drift = report.drift_count();
            (
                Some(report.index.documents.len()),
                Some(report.index.sections.len()),
                Some(report.source_errors.is_empty()),
                drift,
                if structural {
                    StateStatus::Blocked
                } else if drift > 0 {
                    StateStatus::Warning
                } else {
                    StateStatus::Ok
                },
            )
        }
        Err(_) => (None, None, None, 0, StateStatus::Blocked),
    };
    if !catalog_valid {
        status = StateStatus::Blocked;
    }
    let mut item = CollectedDomain::plain(DomainState {
        id: DomainId::Documentation,
        status,
        source: source.clone(),
        details: DomainDetails::Documentation(DocumentationState {
            catalog_path: Some(catalog_path),
            catalog_available,
            catalog_valid: Some(catalog_valid),
            documents,
            sections,
            source_valid,
            known_drift,
        }),
    });
    if !catalog_valid {
        add_blocker_and_diagnostic(
            &mut item,
            "documentation.catalog_unavailable",
            DomainId::Documentation,
            "O catálogo documental está ausente ou inválido.",
            if catalog_available {
                "documentation_catalog_invalid"
            } else {
                "documentation_catalog_missing"
            },
            source.clone(),
        );
    }
    if let Ok(report) = &verification {
        if !report.source_errors.is_empty() {
            add_blocker_and_diagnostic(
                &mut item,
                "documentation.source_invalid",
                DomainId::Documentation,
                "A documentação marcada contém divergências estruturais.",
                "documentation_source_invalid",
                Source::new(
                    SourceKind::LocalCheck,
                    None,
                    "trama.documentation-index.verify",
                ),
            );
        }
        if !report.manifest_errors.is_empty() || report.projection_error.is_some() {
            add_blocker_and_diagnostic(
                &mut item,
                "documentation.authority_invalid",
                DomainId::Documentation,
                "Manifestos ou projeções documentais não puderam ser validados.",
                "documentation_authority_invalid",
                Source::new(SourceKind::LocalCheck, None, "trama.documentation.verify"),
            );
        }
        if report.drift_count() > 0 {
            add_warning_and_diagnostic(
                &mut item,
                "documentation.drift",
                DomainId::Documentation,
                "A documentação derivada possui drift conhecido.",
                "documentation_drift",
                source.clone(),
            );
            item.pending.push(PendingOperation {
                id: "documentation.synchronize".to_string(),
                domain: DomainId::Documentation,
                kind: "synchronize_documentation".to_string(),
                summary: "Sincronizar catálogo, ledger ou projeções documentais.".to_string(),
                source: source.clone(),
                reason: "documentation_drift".to_string(),
            });
        }
    } else {
        add_blocker_and_diagnostic(
            &mut item,
            "documentation.scan_failed",
            DomainId::Documentation,
            "A árvore documental não pôde ser observada.",
            "documentation_scan_failed",
            Source::new(
                SourceKind::LocalCheck,
                None,
                "trama.documentation-index.scan",
            ),
        );
    }
    item
}

fn collect_projections(root: &RepoRoot) -> CollectedDomain {
    let source = Source::new(
        SourceKind::ProjectionStore,
        Some(".pinker/projections/"),
        "projection-store",
    );
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(_) => {
            let mut item = CollectedDomain::plain(DomainState {
                id: DomainId::Projections,
                status: StateStatus::Blocked,
                source: source.clone(),
                details: DomainDetails::Projections(ProjectionsState {
                    frozen: 0,
                    candidate: 0,
                    recipes: 0,
                    verification: "HARNESS_FAILURE".to_string(),
                    items: Vec::new(),
                    causes: Vec::new(),
                }),
            });
            add_blocker_and_diagnostic(
                &mut item,
                "projections.store_unavailable",
                DomainId::Projections,
                "A autoridade de projeções não pôde ser lida.",
                "projection_store_unavailable",
                source,
            );
            return item;
        }
    };
    let frozen = store
        .snapshots()
        .filter(|stored| stored.snapshot.state == SnapshotState::Frozen)
        .count();
    let candidate_ids = store
        .snapshots()
        .filter(|stored| stored.snapshot.state == SnapshotState::Candidate)
        .map(|stored| stored.snapshot.id.clone())
        .collect::<Vec<_>>();
    let recipes = store.recipes().count();
    let catalog = CodeCatalog::load(&root.path().join("src/navigation.jsonl"));
    let mut items = Vec::new();
    let mut causes = Vec::new();
    let (verification, mut status, batch) = match catalog {
        Ok(catalog) => {
            let batch = nav_projection_report::verify_all(&store, &catalog.regions);
            let status = match batch.outcome() {
                "MATCH" => StateStatus::Ok,
                "DRIFT" => StateStatus::Warning,
                _ => StateStatus::Blocked,
            };
            (batch.outcome().to_string(), status, Some(batch))
        }
        Err(_) => ("UNAVAILABLE".to_string(), StateStatus::Partial, None),
    };
    if !candidate_ids.is_empty() && status == StateStatus::Ok {
        status = StateStatus::Warning;
    }
    if let Some(batch) = &batch {
        for observed in &batch.results {
            items.push(ProjectionItem {
                id: observed.report.snapshot_id.clone(),
                state: observed.report.state.as_str().to_string(),
                path: observed.path.clone(),
                outcome: observed.report.outcome.as_str().to_string(),
                failure_code: match &observed.report.outcome {
                    crate::nav_projection_snapshot::Outcome::HarnessFailure(failure) => {
                        Some(failure.code().to_string())
                    }
                    _ => None,
                },
            });
        }
        for group in &batch.causes {
            let mut blocked = group
                .blocked
                .iter()
                .map(|item| item.snapshot.clone())
                .collect::<Vec<_>>();
            blocked.sort();
            for blocked_id in &blocked {
                if let Some(stored) = store.snapshot(blocked_id) {
                    items.push(ProjectionItem {
                        id: blocked_id.clone(),
                        state: stored.snapshot.state.as_str().to_string(),
                        path: stored.path.clone(),
                        outcome: "BLOCKED_BY_CAUSE".to_string(),
                        failure_code: Some("E-SNAP-BASE-DIVERGENTE".to_string()),
                    });
                }
            }
            causes.push(ProjectionCause {
                cause: group.cause.clone(),
                blocked,
            });
        }
    } else {
        for stored in store.snapshots() {
            items.push(ProjectionItem {
                id: stored.snapshot.id.clone(),
                state: stored.snapshot.state.as_str().to_string(),
                path: stored.path.clone(),
                outcome: "UNKNOWN".to_string(),
                failure_code: None,
            });
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    causes.sort_by(|a, b| a.cause.cmp(&b.cause));
    let mut item = CollectedDomain::plain(DomainState {
        id: DomainId::Projections,
        status,
        source: source.clone(),
        details: DomainDetails::Projections(ProjectionsState {
            frozen,
            candidate: candidate_ids.len(),
            recipes,
            verification: verification.clone(),
            items,
            causes,
        }),
    });
    if batch.is_none() {
        item.diagnostics.push(Diagnostic {
            id: "projections.catalog_unavailable".to_string(),
            domain: DomainId::Projections,
            status: StateStatus::Unavailable,
            summary: "O catálogo de código necessário à verificação composta está indisponível."
                .to_string(),
            reason: "projection_catalog_unavailable".to_string(),
            source: Source::new(
                SourceKind::Catalog,
                Some("src/navigation.jsonl"),
                "trama.code-catalog",
            ),
        });
    }
    if verification == "DRIFT" {
        add_warning_and_diagnostic(
            &mut item,
            "projections.drift",
            DomainId::Projections,
            "Ao menos uma projeção histórica diverge das medidas congeladas.",
            "projection_drift",
            source.clone(),
        );
    } else if verification == "HARNESS_FAILURE" {
        add_blocker_and_diagnostic(
            &mut item,
            "projections.harness_failure",
            DomainId::Projections,
            "A verificação composta encontrou falha de harness.",
            "projection_harness_failure",
            source.clone(),
        );
    }
    for id in candidate_ids {
        item.pending.push(PendingOperation {
            id: format!("projections.accept_candidate.{id}"),
            domain: DomainId::Projections,
            kind: "accept_projection_candidate".to_string(),
            summary: format!("Candidate '{}' aguarda decisão explícita de aceitação.", id),
            source: Source::new(
                SourceKind::RepoFile,
                Some(&format!(".pinker/projections/{id}.toml")),
                "projection-snapshot.lifecycle",
            ),
            reason: "projection_candidate_pending".to_string(),
        });
    }
    item
}

fn collect_local_checks(
    trama: &DomainState,
    documentation: &DomainState,
    projections: &DomainState,
) -> CollectedDomain {
    let checks = vec![
        LocalCheck {
            id: "trama.code_catalog.verify".to_string(),
            status: check_status(trama.status),
            source: Source::new(SourceKind::LocalCheck, None, "trama.code-index.verify"),
        },
        LocalCheck {
            id: "documentation.verify".to_string(),
            status: check_status(documentation.status),
            source: Source::new(SourceKind::LocalCheck, None, "trama.documentation.verify"),
        },
        LocalCheck {
            id: "projections.verify_composed".to_string(),
            status: check_status(projections.status),
            source: Source::new(SourceKind::LocalCheck, None, "projection-report.verify-all"),
        },
    ];
    let status = derive_check_status(&checks);
    CollectedDomain::plain(DomainState {
        id: DomainId::LocalChecks,
        status,
        source: Source::new(SourceKind::Derived, None, "project-state.local-checks"),
        details: DomainDetails::LocalChecks(LocalChecksState { checks }),
    })
}

fn check_status(status: StateStatus) -> StateStatus {
    match status {
        StateStatus::Partial => StateStatus::Unknown,
        other => other,
    }
}

fn derive_check_status(checks: &[LocalCheck]) -> StateStatus {
    if checks
        .iter()
        .any(|check| check.status == StateStatus::Blocked)
    {
        StateStatus::Blocked
    } else if checks
        .iter()
        .any(|check| check.status == StateStatus::Warning)
    {
        StateStatus::Warning
    } else if checks.iter().any(|check| {
        matches!(
            check.status,
            StateStatus::Unknown | StateStatus::Unavailable | StateStatus::Partial
        )
    }) {
        StateStatus::Partial
    } else {
        StateStatus::Ok
    }
}

fn collect_agent(spec: Option<&Path>) -> CollectedDomain {
    let source = Source::new(SourceKind::AgentSpec, None, "pink-agent-v1.status");
    let Some(spec) = spec else {
        return CollectedDomain::plain(DomainState {
            id: DomainId::Agent,
            status: StateStatus::Unavailable,
            source,
            details: DomainDetails::Agent(AgentState {
                configured: false,
                reason: Some("agent_spec_not_provided".to_string()),
                terminal: None,
                publication: None,
                checks: Vec::new(),
            }),
        });
    };
    let observation = agent::observe_status(spec);
    let publication = agent::observe_publication(spec);
    match observation {
        Ok(observation) => {
            let (publication_status, publication_invalid) = match publication {
                Ok(observation) => (observation.map(|value| value.status), false),
                Err(_) => (None, true),
            };
            let mut status = match observation.terminal {
                AgentTerminalStatus::Accepted => StateStatus::Ok,
                AgentTerminalStatus::Blocked | AgentTerminalStatus::NeedsHumanDecision => {
                    StateStatus::Blocked
                }
            };
            if publication_status
                .as_deref()
                .is_some_and(agent_publication_is_pending)
                && status == StateStatus::Ok
            {
                status = StateStatus::Warning;
            }
            let mut item = CollectedDomain::plain(DomainState {
                id: DomainId::Agent,
                status,
                source: source.clone(),
                details: DomainDetails::Agent(AgentState {
                    configured: true,
                    reason: None,
                    terminal: Some(observation.terminal.as_str().to_string()),
                    publication: publication_status.clone(),
                    checks: observation
                        .checks
                        .iter()
                        .map(|check| AgentCheckState {
                            id: check.id.clone(),
                            status: check.status.clone(),
                        })
                        .collect(),
                }),
            });
            match observation.terminal {
                AgentTerminalStatus::Accepted => {}
                AgentTerminalStatus::Blocked => add_blocker_and_diagnostic(
                    &mut item,
                    "agent.blocked",
                    DomainId::Agent,
                    "O estado terminal observado do agente é BLOCKED.",
                    "agent_blocked",
                    source.clone(),
                ),
                AgentTerminalStatus::NeedsHumanDecision => add_blocker_and_diagnostic(
                    &mut item,
                    "agent.needs_human_decision",
                    DomainId::Agent,
                    "O agente requer decisão humana explícita.",
                    "agent_needs_human_decision",
                    source.clone(),
                ),
            }
            if let Some(publication_status) = publication_status {
                if agent_publication_is_pending(&publication_status) {
                    item.pending.push(PendingOperation {
                        id: "agent.publication".to_string(),
                        domain: DomainId::Agent,
                        kind: "agent_publication".to_string(),
                        summary: "O lifecycle local do agente registra publicação pendente."
                            .to_string(),
                        source: Source::new(
                            SourceKind::LocalCheck,
                            None,
                            "pink-agent-v1.publication-state",
                        ),
                        reason: "agent_publication_pending".to_string(),
                    });
                }
            }
            if publication_invalid {
                item.diagnostics.push(Diagnostic {
                    id: "agent.publication_state_invalid".to_string(),
                    domain: DomainId::Agent,
                    status: StateStatus::Unknown,
                    summary: "O estado local de publicação não pôde ser observado.".to_string(),
                    reason: "agent_publication_state_invalid".to_string(),
                    source: source.clone(),
                });
                if item.domain.status == StateStatus::Ok {
                    item.domain.status = StateStatus::Partial;
                }
            }
            item
        }
        Err(_) => {
            let mut item = CollectedDomain::plain(DomainState {
                id: DomainId::Agent,
                status: StateStatus::Blocked,
                source: source.clone(),
                details: DomainDetails::Agent(AgentState {
                    configured: true,
                    reason: Some("agent_observation_failed".to_string()),
                    terminal: None,
                    publication: None,
                    checks: Vec::new(),
                }),
            });
            add_blocker_and_diagnostic(
                &mut item,
                "agent.observation_failed",
                DomainId::Agent,
                "A spec ou o estado local do agente não pôde ser observado.",
                "agent_observation_failed",
                source,
            );
            item
        }
    }
}

fn agent_publication_is_pending(status: &str) -> bool {
    matches!(
        status,
        "LOCAL_ACCEPTED"
            | "COMMIT_INTENT"
            | "COMMITTED"
            | "PUSH_INTENT"
            | "PUSHED"
            | "PR_INTENT"
            | "PR_CREATED"
            | "BODY_VERIFIED"
            | "CHECKS_PENDING"
    )
}

fn add_blocker_and_diagnostic(
    item: &mut CollectedDomain,
    id: &str,
    domain: DomainId,
    summary: &str,
    reason: &str,
    source: Source,
) {
    item.blockers.push(Finding {
        id: id.to_string(),
        domain,
        summary: summary.to_string(),
        source: source.clone(),
        reason: reason.to_string(),
    });
    item.diagnostics.push(Diagnostic {
        id: id.to_string(),
        domain,
        status: StateStatus::Blocked,
        summary: summary.to_string(),
        reason: reason.to_string(),
        source,
    });
}

fn add_warning_and_diagnostic(
    item: &mut CollectedDomain,
    id: &str,
    domain: DomainId,
    summary: &str,
    reason: &str,
    source: Source,
) {
    item.warnings.push(Finding {
        id: id.to_string(),
        domain,
        summary: summary.to_string(),
        source: source.clone(),
        reason: reason.to_string(),
    });
    item.diagnostics.push(Diagnostic {
        id: id.to_string(),
        domain,
        status: StateStatus::Warning,
        summary: summary.to_string(),
        reason: reason.to_string(),
        source,
    });
}

// @pinker-nav:end project-state.coleta

#[cfg(test)]
mod tests {
    use super::{
        derive_overall, DomainDetails, DomainId, DomainState, Source, SourceKind, StateStatus,
    };

    fn domain(status: StateStatus) -> DomainState {
        DomainState {
            id: DomainId::Agent,
            status,
            source: Source::new(SourceKind::Derived, None, "test"),
            details: DomainDetails::Agent(super::AgentState {
                configured: false,
                reason: None,
                terminal: None,
                publication: None,
                checks: Vec::new(),
            }),
        }
    }

    #[test]
    fn overall_tem_precedencia_deterministica() {
        assert_eq!(derive_overall(&[domain(StateStatus::Ok)]), StateStatus::Ok);
        assert_eq!(
            derive_overall(&[domain(StateStatus::Unavailable)]),
            StateStatus::Partial
        );
        assert_eq!(
            derive_overall(&[domain(StateStatus::Unknown), domain(StateStatus::Warning)]),
            StateStatus::Warning
        );
        assert_eq!(
            derive_overall(&[domain(StateStatus::Warning), domain(StateStatus::Blocked)]),
            StateStatus::Blocked
        );
    }
}
