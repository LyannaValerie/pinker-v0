//! Composição read-only das autoridades operacionais usadas pela F1 (#454).
//!
//! Este módulo não recria catálogo, projeções, estado documental ou análise de
//! diff. Ele adapta as autoridades existentes para os contratos estruturados
//! de `pink doctor`, `pink nav impacto` e `pink verificar`.

use crate::automation::RepoRoot;
use crate::change;
use crate::diff_coverage::{self, CoverageAuthorities, RelationStatus};
use crate::doc::{self, DocConfig};
use crate::doc_index::DocCatalog;
use crate::nav::{self, CodeCatalog};
use crate::nav_projection_store::ProjectionStore;
use crate::project_state::{self, DomainDetails, DomainId, StateStatus};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

pub const TOOLING_SCHEMA: u64 = 1;
pub const AVAILABLE_SUBCOMMANDS: &[&str] = &[
    "build",
    "doc",
    "doctor",
    "editor",
    "estado",
    "nav",
    "repl",
    "verificar",
];

// @pinker-nav:start tooling.f1.doctor
// @pinker-nav:domain tooling
// @pinker-nav:layer preflight
// @pinker-nav:summary Identidade binária e Git, compatibilidade por ancestralidade e recomendação determinística compostas com o estado observacional vigente para o contrato JSON de pink doctor.

pub fn binary_commit() -> &'static str {
    option_env!("PINKER_BUILD_COMMIT").unwrap_or("UNKNOWN")
}

pub fn render_binary_identity_json() -> Result<String, String> {
    let binary_path = std::env::current_exe()
        .map_err(|error| format!("não foi possível resolver o executável atual: {error}"))?;
    Ok(format!(
        "{{\"schema\":{TOOLING_SCHEMA},\"binary_path\":{},\"binary_version\":{},\"binary_commit\":{}}}",
        json_string(&binary_path.to_string_lossy()),
        json_string(env!("CARGO_PKG_VERSION")),
        json_string(binary_commit()),
    ))
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_strings(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| json_string(value.as_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|error| format!("não foi possível executar git: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {} terminou com {}", args.join(" "), output.status)
        } else {
            stderr
        });
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|_| "git produziu saída que não é UTF-8".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Exact,
    CompatibleAncestor,
    Incompatible,
    UnknownBinaryCommit,
}

impl Compatibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Compatibility::Exact => "EXACT",
            Compatibility::CompatibleAncestor => "COMPATIBLE_ANCESTOR",
            Compatibility::Incompatible => "INCOMPATIBLE",
            Compatibility::UnknownBinaryCommit => "UNKNOWN_BINARY_COMMIT",
        }
    }

    pub fn usable(self) -> bool {
        matches!(
            self,
            Compatibility::Exact | Compatibility::CompatibleAncestor
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub binary_path: String,
    pub binary_version: String,
    pub binary_commit: String,
    pub repo_root: String,
    pub repo_head: String,
    pub compatibility: Compatibility,
    pub navigation_catalog: String,
    pub projection_state: String,
    pub available_subcommands: Vec<String>,
    pub recommended_next_action: String,
}

fn compatibility(repo: &Path, binary: &str, head: &str) -> Compatibility {
    if binary == "UNKNOWN" || binary.is_empty() {
        return Compatibility::UnknownBinaryCommit;
    }
    if binary == head {
        return Compatibility::Exact;
    }
    match Command::new("git")
        .current_dir(repo)
        .args(["merge-base", "--is-ancestor", binary, head])
        .status()
    {
        Ok(status) if status.success() => Compatibility::CompatibleAncestor,
        _ => Compatibility::Incompatible,
    }
}

fn navigation_status(state: &project_state::ProjectState) -> String {
    let Some(domain) = state.domain(DomainId::Trama) else {
        return "UNAVAILABLE".to_string();
    };
    let DomainDetails::Trama(details) = &domain.details else {
        return "UNAVAILABLE".to_string();
    };
    if !details.catalog_available || details.catalog_valid == Some(false) {
        "UNAVAILABLE".to_string()
    } else if !details.source_valid.unwrap_or(false) {
        "INVALID_SOURCE".to_string()
    } else if details.synchronized == Some(false) {
        "STALE".to_string()
    } else if details.synchronized == Some(true) {
        "CURRENT".to_string()
    } else {
        "UNKNOWN".to_string()
    }
}

fn projection_status(state: &project_state::ProjectState) -> String {
    let Some(domain) = state.domain(DomainId::Projections) else {
        return "UNAVAILABLE".to_string();
    };
    let DomainDetails::Projections(details) = &domain.details else {
        return "UNAVAILABLE".to_string();
    };
    match domain.status {
        StateStatus::Unavailable | StateStatus::Unknown | StateStatus::Partial => {
            "UNAVAILABLE".to_string()
        }
        StateStatus::Blocked => "BLOCKED".to_string(),
        StateStatus::Warning if details.verification == "DRIFT" => "STALE".to_string(),
        StateStatus::Warning => "WARNING".to_string(),
        StateStatus::Ok if details.verification == "MATCH" => "CURRENT".to_string(),
        StateStatus::Ok => details.verification.clone(),
    }
}

pub fn recommended_action(
    compatibility: Compatibility,
    navigation_catalog: &str,
    projection_state: &str,
) -> String {
    if !compatibility.usable() {
        return "rebuild or reinstall pink from the current Pinker release".to_string();
    }
    if navigation_catalog != "CURRENT" {
        return "pink nav sincronizar".to_string();
    }
    if projection_state != "CURRENT" {
        return "pink nav projecao verificar".to_string();
    }
    "no_action".to_string()
}

pub fn collect_doctor(repo: &Path) -> Result<DoctorReport, String> {
    let root = RepoRoot::discover(repo).map_err(|error| error.to_string())?;
    let repo_head = git_output(root.path(), &["rev-parse", "HEAD"])?;
    let binary_commit = binary_commit().to_string();
    let compatibility = compatibility(root.path(), &binary_commit, &repo_head);
    let state = project_state::collect(root.path()).map_err(|error| error.to_string())?;
    let navigation_catalog = navigation_status(&state);
    let projection_state = projection_status(&state);
    let binary_path = std::env::current_exe()
        .map_err(|error| format!("não foi possível resolver o executável atual: {error}"))?
        .to_string_lossy()
        .into_owned();
    let recommended_next_action =
        recommended_action(compatibility, &navigation_catalog, &projection_state);
    Ok(DoctorReport {
        binary_path,
        binary_version: env!("CARGO_PKG_VERSION").to_string(),
        binary_commit,
        repo_root: root.path().to_string_lossy().into_owned(),
        repo_head,
        compatibility,
        navigation_catalog,
        projection_state,
        available_subcommands: AVAILABLE_SUBCOMMANDS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        recommended_next_action,
    })
}

pub fn render_doctor_json(report: &DoctorReport) -> String {
    format!(
        "{{\"schema\":{TOOLING_SCHEMA},\"binary_path\":{},\"binary_version\":{},\"binary_commit\":{},\"repo_root\":{},\"repo_head\":{},\"compatibility\":{},\"navigation_catalog\":{},\"projection_state\":{},\"available_subcommands\":[{}],\"recommended_next_action\":{}}}",
        json_string(&report.binary_path),
        json_string(&report.binary_version),
        json_string(&report.binary_commit),
        json_string(&report.repo_root),
        json_string(&report.repo_head),
        json_string(report.compatibility.as_str()),
        json_string(&report.navigation_catalog),
        json_string(&report.projection_state),
        json_strings(&report.available_subcommands),
        json_string(&report.recommended_next_action),
    )
}
// @pinker-nav:end tooling.f1.doctor

// @pinker-nav:start tooling.f1.impact
// @pinker-nav:domain tooling
// @pinker-nav:layer navigation
// @pinker-nav:summary Adaptador read-only de git diff limitado, catálogo derivado atual e diff_coverage que preserva KNOWN, UNKNOWN e UNAVAILABLE no contrato JSON de pink nav impacto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactRelation {
    pub status: RelationStatus,
    pub reason: Option<String>,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactReport {
    pub diff: String,
    pub changed_files: Vec<ChangedFile>,
    pub changed_regions: ImpactRelation,
    pub navigation_entries_affected: ImpactRelation,
    pub projection_overrides_required: ImpactRelation,
    pub projections_affected: ImpactRelation,
    pub catalog_status: String,
}

fn validate_diff_spec(diff: &str) -> Result<(), String> {
    if diff.is_empty() || diff.len() > 512 || diff.starts_with('-') {
        return Err("E-IMPACT-DIFF: referência de diff inválida".to_string());
    }
    if diff
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '\0')
    {
        return Err("E-IMPACT-DIFF: referência de diff contém caracteres inválidos".to_string());
    }
    Ok(())
}

fn bounded_git_diff(repo: &Path, diff: &str) -> Result<String, String> {
    validate_diff_spec(diff)?;
    let mut child = Command::new("git")
        .current_dir(repo)
        .args(["diff", "--no-ext-diff", "--unified=3", diff, "--"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("E-IMPACT-GIT: {error}"))?;
    let mut bytes = Vec::new();
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "E-IMPACT-GIT: stdout indisponível".to_string())?;
    stdout
        .by_ref()
        .take((diff_coverage::MAX_DIFF_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("E-IMPACT-IO: {error}"))?;
    if bytes.len() > diff_coverage::MAX_DIFF_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "E-IMPACT-SIZE: diff excede {} bytes",
            diff_coverage::MAX_DIFF_BYTES
        ));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("E-IMPACT-GIT: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "E-IMPACT-GIT: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(bytes).map_err(|_| "E-IMPACT-UTF8: diff não é UTF-8".to_string())
}

fn aggregate_status(statuses: impl Iterator<Item = RelationStatus>) -> RelationStatus {
    let mut result = RelationStatus::Known;
    for status in statuses {
        match status {
            RelationStatus::Unavailable => return RelationStatus::Unavailable,
            RelationStatus::Unknown => result = RelationStatus::Unknown,
            RelationStatus::Known => {}
        }
    }
    result
}

fn aggregate_reason(status: RelationStatus, domain: &str) -> Option<String> {
    match status {
        RelationStatus::Known => None,
        RelationStatus::Unknown => Some(format!(
            "ao menos um arquivo não possui relação explícita completa para {domain}"
        )),
        RelationStatus::Unavailable => Some(format!(
            "a autoridade necessária para relacionar {domain} está indisponível"
        )),
    }
}

pub fn collect_impact(repo: &Path, diff: &str) -> Result<ImpactReport, String> {
    let root = RepoRoot::discover(repo).map_err(|error| error.to_string())?;
    let unified = bounded_git_diff(root.path(), diff)?;
    let config = DocConfig::load(root.path()).map_err(|error| error.to_string())?;
    let verification = nav::verify_repository(root.path(), &config.generated.code_index)
        .map_err(|error| error.to_string())?;
    if !verification.source_errors.is_empty() {
        return Err(format!(
            "E-IMPACT-CATALOG: fontes de navegação inválidos ({})",
            verification.source_errors.len()
        ));
    }
    let catalog_status = if verification.catalog_out_of_date {
        "STALE"
    } else {
        "CURRENT"
    }
    .to_string();
    let code = CodeCatalog::parse(&verification.index.render_jsonl(), "<derived-current>")
        .map_err(|error| error.to_string())?;
    let docs_path = root.path().join(&config.generated.docs_index);
    let docs = DocCatalog::load(&docs_path).ok();
    let projection_store = ProjectionStore::load(root.path()).ok();
    let manifests = change::Manifests::load(&root.path().join(".pinker/changes"));
    let coverage = diff_coverage::analyze(
        &unified,
        CoverageAuthorities {
            code: &code,
            docs: docs.as_ref(),
            projection_store: projection_store.as_ref(),
            doc_config: Some(&config),
            manifests: Some(&manifests),
        },
    )
    .map_err(|error| error.to_string())?;

    let changed_files = coverage
        .files
        .iter()
        .map(|file| ChangedFile {
            path: file.path.clone(),
            status: file.status.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    let region_status = aggregate_status(coverage.files.iter().map(|file| file.regions.status));
    let region_items = coverage
        .files
        .iter()
        .flat_map(|file| file.regions.items.iter().map(|region| region.id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let projection_status =
        aggregate_status(coverage.files.iter().map(|file| file.projections.status));
    let projection_items = coverage
        .files
        .iter()
        .flat_map(|file| {
            file.projections
                .items
                .iter()
                .map(|projection| projection.id.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let overrides = if coverage.files.is_empty() {
        ImpactRelation {
            status: RelationStatus::Known,
            reason: None,
            items: Vec::new(),
        }
    } else {
        ImpactRelation {
            status: RelationStatus::Unknown,
            reason: Some(
                "impacto de região não prova, sozinho, necessidade de override de projeção"
                    .to_string(),
            ),
            items: Vec::new(),
        }
    };
    let regions = ImpactRelation {
        status: region_status,
        reason: aggregate_reason(region_status, "regiões de navegação"),
        items: region_items,
    };
    Ok(ImpactReport {
        diff: diff.to_string(),
        changed_files,
        changed_regions: regions.clone(),
        navigation_entries_affected: regions,
        projection_overrides_required: overrides,
        projections_affected: ImpactRelation {
            status: projection_status,
            reason: aggregate_reason(projection_status, "projeções"),
            items: projection_items,
        },
        catalog_status,
    })
}

fn render_relation(relation: &ImpactRelation) -> String {
    format!(
        "{{\"status\":{},\"reason\":{},\"items\":[{}]}}",
        json_string(relation.status.as_str()),
        relation
            .reason
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        json_strings(&relation.items),
    )
}

pub fn render_impact_json(report: &ImpactReport) -> String {
    let files = report
        .changed_files
        .iter()
        .map(|file| {
            format!(
                "{{\"path\":{},\"status\":{}}}",
                json_string(&file.path),
                json_string(&file.status)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{TOOLING_SCHEMA},\"diff\":{},\"changed_files\":[{files}],\"changed_regions\":{},\"navigation_entries_affected\":{},\"projection_overrides_required\":{},\"projections_affected\":{},\"catalog_status\":{}}}",
        json_string(&report.diff),
        render_relation(&report.changed_regions),
        render_relation(&report.navigation_entries_affected),
        render_relation(&report.projection_overrides_required),
        render_relation(&report.projections_affected),
        json_string(&report.catalog_status),
    )
}
// @pinker-nav:end tooling.f1.impact

// @pinker-nav:start tooling.f1.freeze-import
// @pinker-nav:domain tooling
// @pinker-nav:layer documentation
// @pinker-nav:summary Importação freeze-aware que reutiliza change e doc verification, preserva artifact idempotente e impede escrita nas autoridades documentais congeladas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeImportClassification {
    ValidatedDeferredByFreeze,
    InvalidManifest,
    UnexpectedDocumentaryInconsistency,
}

impl FreezeImportClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            FreezeImportClassification::ValidatedDeferredByFreeze => "VALIDATED_DEFERRED_BY_FREEZE",
            FreezeImportClassification::InvalidManifest => "INVALID_MANIFEST",
            FreezeImportClassification::UnexpectedDocumentaryInconsistency => {
                "UNEXPECTED_DOCUMENTARY_INCONSISTENCY"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreezeImportReport {
    pub classification: FreezeImportClassification,
    pub pr: u64,
    pub artifact: Option<String>,
    pub detail: String,
}

fn normalize_lexical(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => out.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("artifact escapa da raiz lexical".to_string());
                }
            }
        }
    }
    Ok(out)
}

fn artifact_path(repo: &Path, raw: &Path) -> Result<PathBuf, String> {
    let absolute = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        repo.join(raw)
    };
    let normalized = normalize_lexical(&absolute)?;
    if let Ok(relative) = normalized.strip_prefix(repo) {
        let first = relative
            .components()
            .next()
            .and_then(|component| match component {
                Component::Normal(value) => value.to_str(),
                _ => None,
            });
        if matches!(first, Some("docs" | ".pinker" | "README.md" | "MANUAL.md")) {
            return Err("artifact aponta para autoridade documental congelada".to_string());
        }
    }
    Ok(normalized)
}

fn preserve_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        let existing = fs::read(path).map_err(|error| error.to_string())?;
        return if existing == bytes {
            Ok(())
        } else {
            Err("artifact existente possui conteúdo diferente".to_string())
        };
    }
    let parent = path
        .parent()
        .ok_or_else(|| "artifact não possui diretório pai".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("pink-artifact"),
        std::process::id()
    ));
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error.to_string())
        }
    }
}

pub fn freeze_import(
    repo: &Path,
    config: &DocConfig,
    pr: u64,
    body_path: &Path,
    artifact: &Path,
) -> FreezeImportReport {
    let invalid = |detail: String| FreezeImportReport {
        classification: FreezeImportClassification::InvalidManifest,
        pr,
        artifact: None,
        detail,
    };
    let body = match fs::read_to_string(body_path) {
        Ok(body) => body,
        Err(error) => return invalid(format!("falha ao ler corpo: {error}")),
    };
    let mut manifest = match change::Change::parse_pr_body(&body) {
        Ok(manifest) => manifest,
        Err(error) => return invalid(error.to_string()),
    };
    if let Err(error) = manifest.validate() {
        return invalid(error.to_string());
    }
    let verification = match doc::verify_repository(repo, config) {
        Ok(verification) => verification,
        Err(error) => {
            return FreezeImportReport {
                classification: FreezeImportClassification::UnexpectedDocumentaryInconsistency,
                pr,
                artifact: None,
                detail: error.to_string(),
            }
        }
    };
    if !verification.is_ok() {
        return FreezeImportReport {
            classification: FreezeImportClassification::UnexpectedDocumentaryInconsistency,
            pr,
            artifact: None,
            detail: format!(
                "verificação documental vigente possui {} inconsistência(s)",
                verification.total_errors()
            ),
        };
    }
    manifest.source = Some(change::Source {
        kind: "github-pr".to_string(),
        number: pr,
        repository: config.github.repository.clone(),
    });
    let output = manifest.render_yaml();
    let path = match artifact_path(repo, artifact) {
        Ok(path) => path,
        Err(detail) => return invalid(detail),
    };
    if let Err(detail) = preserve_artifact(&path, output.as_bytes()) {
        return invalid(detail);
    }
    FreezeImportReport {
        classification: FreezeImportClassification::ValidatedDeferredByFreeze,
        pr,
        artifact: Some(path.to_string_lossy().into_owned()),
        detail: "pinker-change válido; evidência preservada sem mutar documentação congelada"
            .to_string(),
    }
}

pub fn render_freeze_import_json(report: &FreezeImportReport) -> String {
    format!(
        "{{\"schema\":{TOOLING_SCHEMA},\"classification\":{},\"pr\":{},\"artifact\":{},\"canonical_documentation_mutated\":false,\"detail\":{}}}",
        json_string(report.classification.as_str()),
        report.pr,
        report
            .artifact
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        json_string(&report.detail),
    )
}
// @pinker-nav:end tooling.f1.freeze-import

// @pinker-nav:start tooling.f1.unified-preflight
// @pinker-nav:domain tooling
// @pinker-nav:layer preflight
// @pinker-nav:summary Preflight único que compõe doctor, impacto, projeções, pinker-change, estado documental e freeze em blocking, warnings, deferred e ações recomendadas antes de make ci.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub doctor: DoctorReport,
    pub impact: Option<ImpactReport>,
    pub impact_error: Option<String>,
    pub projection_validation: String,
    pub pinker_change: String,
    pub documentary_state: String,
    pub blocking: Vec<Finding>,
    pub warnings: Vec<Finding>,
    pub expected_deferred: Vec<Finding>,
    pub recommended_actions: Vec<String>,
}

pub fn collect_preflight(
    repo: &Path,
    diff: &str,
    documentation_frozen: bool,
    body: Option<&Path>,
) -> Result<PreflightReport, String> {
    let root = RepoRoot::discover(repo).map_err(|error| error.to_string())?;
    let doctor = collect_doctor(root.path())?;
    let mut blocking = Vec::new();
    let mut warnings = Vec::new();
    let mut expected_deferred = Vec::new();
    let mut recommended = BTreeSet::new();
    if !doctor.compatibility.usable() {
        blocking.push(Finding {
            id: "baseline_incompatible".to_string(),
            detail: doctor.compatibility.as_str().to_string(),
        });
        recommended.insert(doctor.recommended_next_action.clone());
    } else if doctor.recommended_next_action != "no_action" {
        recommended.insert(doctor.recommended_next_action.clone());
    }
    if doctor.navigation_catalog != "CURRENT" {
        blocking.push(Finding {
            id: "navigation_catalog_not_current".to_string(),
            detail: doctor.navigation_catalog.clone(),
        });
        recommended.insert("pink nav sincronizar".to_string());
    }
    if doctor.projection_state != "CURRENT" {
        blocking.push(Finding {
            id: "projection_validation_not_current".to_string(),
            detail: doctor.projection_state.clone(),
        });
        recommended.insert("pink nav projecao verificar".to_string());
    }
    let (impact, impact_error) = match collect_impact(root.path(), diff) {
        Ok(impact) => (Some(impact), None),
        Err(error) => {
            blocking.push(Finding {
                id: "navigation_impact_unavailable".to_string(),
                detail: error.clone(),
            });
            (None, Some(error))
        }
    };
    let config = DocConfig::load(root.path()).map_err(|error| error.to_string())?;
    let documentary =
        doc::verify_repository(root.path(), &config).map_err(|error| error.to_string())?;
    let documentary_state = if documentary.is_ok() {
        "CONSISTENT"
    } else {
        blocking.push(Finding {
            id: "unexpected_documentary_inconsistency".to_string(),
            detail: format!("{} inconsistência(s)", documentary.total_errors()),
        });
        "INCONSISTENT"
    }
    .to_string();
    let pinker_change = if let Some(body_path) = body {
        match fs::read_to_string(body_path)
            .map_err(|error| error.to_string())
            .and_then(|body| {
                change::Change::parse_pr_body(&body).map_err(|error| error.to_string())
            })
            .and_then(|manifest| {
                manifest
                    .validate()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }) {
            Ok(()) => {
                if documentation_frozen {
                    expected_deferred.push(Finding {
                        id: "pinker_change_import".to_string(),
                        detail: "VALIDATED_DEFERRED_BY_FREEZE".to_string(),
                    });
                }
                "VALID"
            }
            Err(error) => {
                blocking.push(Finding {
                    id: "invalid_pinker_change".to_string(),
                    detail: error,
                });
                "INVALID"
            }
        }
    } else {
        warnings.push(Finding {
            id: "pinker_change_body_unavailable".to_string(),
            detail: "use --corpo para validar o corpo local da PR".to_string(),
        });
        "UNAVAILABLE"
    }
    .to_string();
    if !documentation_frozen {
        warnings.push(Finding {
            id: "documentation_freeze_not_declared".to_string(),
            detail: "o preflight não classificará obrigações documentais como deferred".to_string(),
        });
    }
    Ok(PreflightReport {
        projection_validation: doctor.projection_state.clone(),
        doctor,
        impact,
        impact_error,
        pinker_change,
        documentary_state,
        blocking,
        warnings,
        expected_deferred,
        recommended_actions: recommended.into_iter().collect(),
    })
}

fn render_findings(findings: &[Finding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!(
                "{{\"id\":{},\"detail\":{}}}",
                json_string(&finding.id),
                json_string(&finding.detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub fn render_preflight_json(report: &PreflightReport) -> String {
    let status = if !report.blocking.is_empty() {
        "BLOCKED"
    } else if !report.warnings.is_empty() {
        "WARNING"
    } else {
        "READY"
    };
    let impact = report
        .impact
        .as_ref()
        .map(render_impact_json)
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"schema\":{TOOLING_SCHEMA},\"status\":{},\"blocking\":[{}],\"warnings\":[{}],\"expected_deferred\":[{}],\"recommended_actions\":[{}],\"doctor\":{},\"navigation_impact\":{},\"navigation_impact_error\":{},\"projection_validation\":{},\"pinker_change\":{},\"documentary_state\":{}}}",
        json_string(status),
        render_findings(&report.blocking),
        render_findings(&report.warnings),
        render_findings(&report.expected_deferred),
        json_strings(&report.recommended_actions),
        render_doctor_json(&report.doctor),
        impact,
        report
            .impact_error
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        json_string(&report.projection_validation),
        json_string(&report.pinker_change),
        json_string(&report.documentary_state),
    )
}

pub fn preflight_exit_code(report: &PreflightReport) -> i32 {
    if report.blocking.is_empty() {
        0
    } else {
        1
    }
}
// @pinker-nav:end tooling.f1.unified-preflight

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_action_has_deterministic_priority() {
        assert_eq!(
            recommended_action(Compatibility::Incompatible, "STALE", "STALE"),
            "rebuild or reinstall pink from the current Pinker release"
        );
        assert_eq!(
            recommended_action(Compatibility::Exact, "STALE", "STALE"),
            "pink nav sincronizar"
        );
        assert_eq!(
            recommended_action(Compatibility::Exact, "CURRENT", "STALE"),
            "pink nav projecao verificar"
        );
        assert_eq!(
            recommended_action(Compatibility::Exact, "CURRENT", "CURRENT"),
            "no_action"
        );
    }

    #[test]
    fn invalid_diff_specs_are_rejected_before_git() {
        for invalid in ["", "--stat", "origin/main...HEAD bad", "\n"] {
            assert!(validate_diff_spec(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_diff_spec("origin/main...HEAD").is_ok());
    }

    #[test]
    fn empty_diff_has_known_empty_override_requirement() {
        let report = ImpactReport {
            diff: "HEAD...HEAD".to_string(),
            changed_files: Vec::new(),
            changed_regions: ImpactRelation {
                status: RelationStatus::Known,
                reason: None,
                items: Vec::new(),
            },
            navigation_entries_affected: ImpactRelation {
                status: RelationStatus::Known,
                reason: None,
                items: Vec::new(),
            },
            projection_overrides_required: ImpactRelation {
                status: RelationStatus::Known,
                reason: None,
                items: Vec::new(),
            },
            projections_affected: ImpactRelation {
                status: RelationStatus::Known,
                reason: None,
                items: Vec::new(),
            },
            catalog_status: "CURRENT".to_string(),
        };
        let json = render_impact_json(&report);
        assert!(json.contains("\"projection_overrides_required\":{\"status\":\"KNOWN\""));
    }
}
