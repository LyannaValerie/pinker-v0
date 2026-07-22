//! Runner local, estrito e sem dependências para tarefas operacionais Pinker.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const EXIT_ACCEPTED: i32 = 0;
pub const EXIT_BLOCKED: i32 = 1;
pub const EXIT_NEEDS_HUMAN: i32 = 2;

// @pinker-nav:start development.agent.spec
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Modelo e parser determinístico do formato line-oriented agent-spec-v1: campos escalares únicos, listas repetíveis explícitas e comandos ordenados; rejeita schema, campo e comando duplicado, shell implícito e valores malformados sem depender de crates externas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Program,
    Pinker,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: String,
    pub kind: CommandKind,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub expected_exit: i32,
    pub shell: bool,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub schema: u32,
    pub task_id: String,
    pub repo_root: PathBuf,
    pub worktree: PathBuf,
    pub delegated_root: PathBuf,
    pub expected_base: String,
    pub allowed_writes: Vec<String>,
    pub allowed_changes: Vec<String>,
    pub commands: Vec<CommandSpec>,
    pub checks: Vec<CheckSpec>,
    pub mutations: Vec<MutationSpec>,
    pub publication: Option<PublicationSpec>,
    pub accepted_verdict: String,
    pub blocked_verdict: String,
    pub human_verdict: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckSpec {
    Git(GitCheck),
    MarkerOnly(MarkerOnlyCheck),
    Projection(ProjectionCheck),
    PrBody(PrBodyCheck),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrBodyCheck {
    pub id: String,
    pub path: String,
    pub validation_pr_number: u64,
    pub expected_kind: String,
    pub expected_title: String,
    pub expected_areas: Vec<String>,
    pub expected_validations: Vec<String>,
    pub forbid_sentinel: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationSpec {
    pub repository: String,
    pub remote: String,
    pub base_branch: String,
    pub expected_base: String,
    pub head_branch: String,
    pub commit_message: String,
    pub changes: Vec<String>,
    pub pr_title: String,
    pub pr_body: String,
    pub draft: bool,
    pub required_checks: Vec<String>,
    pub defer_checks: bool,
    pub poll_seconds: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCheck {
    pub id: String,
    pub expected_head: Option<String>,
    pub expected_branch: Option<String>,
    pub require_clean: bool,
    pub diff_check: bool,
    pub expected_changes: Vec<String>,
    pub allowed_changes: Vec<String>,
    pub commit_count_after_base: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkerOnlyCheck {
    pub id: String,
    pub path: String,
    pub base_sha256: String,
    pub expected_regions: usize,
    pub expected_marker_lines: usize,
    pub expected_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionCheck {
    pub id: String,
    pub catalog: String,
    pub expected_total: usize,
    pub expected_evidence: usize,
    pub expected_runtime: usize,
    pub expected_length: usize,
    pub expected_fnv1a64: String,
    pub exclude_files: Vec<String>,
    pub exclude_keys: Vec<String>,
    pub override_hashes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationSpec {
    pub id: String,
    pub target: String,
    pub search_file: String,
    pub replacement_file: String,
    pub expected_matches: usize,
    pub probe_program: String,
    pub probe_argv: Vec<String>,
    pub probe_cwd: String,
    pub probe_expected_exit: i32,
    pub probe_stderr_contains: Option<String>,
}

#[derive(Default)]
struct CommandBuilder {
    kind: Option<CommandKind>,
    program: Option<String>,
    argv: Vec<String>,
    cwd: Option<String>,
    expected_exit: Option<i32>,
    shell: Option<bool>,
    env: BTreeMap<String, String>,
}

#[derive(Default)]
struct CheckBuilder {
    kind: Option<String>,
    values: BTreeMap<String, String>,
    repeated: BTreeMap<String, Vec<String>>,
    overrides: BTreeMap<String, String>,
}

#[derive(Default)]
struct MutationBuilder {
    values: BTreeMap<String, String>,
    argv: Vec<String>,
}

#[derive(Default)]
struct PublicationBuilder {
    values: BTreeMap<String, String>,
    changes: Vec<String>,
    required_checks: Vec<String>,
}

fn parse_bool(value: &str, line: usize) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("linha {line}: booleano inválido: {value}")),
    }
}

fn assign_once<T>(slot: &mut Option<T>, value: T, name: &str, line: usize) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("linha {line}: campo duplicado: {name}"));
    }
    *slot = Some(value);
    Ok(())
}

fn validate_id(id: &str, kind: &str, line: usize) -> Result<(), String> {
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(format!("linha {line}: id de {kind} inválido: {id}"));
    }
    Ok(())
}

fn required(map: &mut BTreeMap<String, String>, field: &str, id: &str) -> Result<String, String> {
    map.remove(field)
        .ok_or_else(|| format!("{id}.{field} ausente"))
}

fn number<T: std::str::FromStr>(value: String, field: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("valor numérico inválido em {field}"))
}

fn build_check(id: &str, mut builder: CheckBuilder) -> Result<CheckSpec, String> {
    let kind = builder
        .kind
        .ok_or_else(|| format!("check.{id}.kind ausente"))?;
    if builder.overrides.values().any(|value| {
        value.strip_prefix("fnv1a64:").map_or(true, |hex| {
            hex.len() != 16 || !hex.chars().all(|ch| ch.is_ascii_hexdigit())
        })
    }) {
        return Err(format!("check.{id}: override hash inválido"));
    }
    let result = match kind.as_str() {
        "git" => CheckSpec::Git(GitCheck {
            id: id.to_string(),
            expected_head: builder.values.remove("expected_head"),
            expected_branch: builder.values.remove("expected_branch"),
            require_clean: builder
                .values
                .remove("require_clean")
                .map(|v| parse_bool(&v, 0))
                .transpose()?
                .unwrap_or(false),
            diff_check: builder
                .values
                .remove("diff_check")
                .map(|v| parse_bool(&v, 0))
                .transpose()?
                .unwrap_or(false),
            expected_changes: builder
                .repeated
                .remove("expected_change")
                .unwrap_or_default(),
            allowed_changes: builder
                .repeated
                .remove("allowed_change")
                .unwrap_or_default(),
            commit_count_after_base: builder
                .values
                .remove("commit_count_after_base")
                .map(|v| number(v, "commit_count_after_base"))
                .transpose()?,
        }),
        "marker-only" => CheckSpec::MarkerOnly(MarkerOnlyCheck {
            id: id.to_string(),
            path: required(&mut builder.values, "path", id)?,
            base_sha256: required(&mut builder.values, "base_sha256", id)?,
            expected_regions: number(
                required(&mut builder.values, "expected_regions", id)?,
                "expected_regions",
            )?,
            expected_marker_lines: number(
                required(&mut builder.values, "expected_marker_lines", id)?,
                "expected_marker_lines",
            )?,
            expected_keys: builder.repeated.remove("expected_key").unwrap_or_default(),
        }),
        "projection" => CheckSpec::Projection(ProjectionCheck {
            id: id.to_string(),
            catalog: required(&mut builder.values, "catalog", id)?,
            expected_total: number(
                required(&mut builder.values, "expected_total", id)?,
                "expected_total",
            )?,
            expected_evidence: number(
                required(&mut builder.values, "expected_evidence", id)?,
                "expected_evidence",
            )?,
            expected_runtime: number(
                required(&mut builder.values, "expected_runtime", id)?,
                "expected_runtime",
            )?,
            expected_length: number(
                required(&mut builder.values, "expected_length", id)?,
                "expected_length",
            )?,
            expected_fnv1a64: required(&mut builder.values, "expected_fnv1a64", id)?,
            exclude_files: builder.repeated.remove("exclude_file").unwrap_or_default(),
            exclude_keys: builder.repeated.remove("exclude_key").unwrap_or_default(),
            override_hashes: std::mem::take(&mut builder.overrides),
        }),
        "pr-body" => CheckSpec::PrBody(PrBodyCheck {
            id: id.to_string(),
            path: required(&mut builder.values, "path", id)?,
            validation_pr_number: number(
                required(&mut builder.values, "validation_pr_number", id)?,
                "validation_pr_number",
            )?,
            expected_kind: required(&mut builder.values, "expected_kind", id)?,
            expected_title: required(&mut builder.values, "expected_title", id)?,
            expected_areas: builder.repeated.remove("expected_area").unwrap_or_default(),
            expected_validations: builder
                .repeated
                .remove("expected_validation")
                .unwrap_or_default(),
            forbid_sentinel: parse_bool(&required(&mut builder.values, "forbid_sentinel", id)?, 0)?,
        }),
        _ => return Err(format!("check.{id}.kind inválido: {kind}")),
    };
    if !builder.values.is_empty() || !builder.repeated.is_empty() || !builder.overrides.is_empty() {
        return Err(format!("check.{id}: campo incompatível com kind"));
    }
    Ok(result)
}

fn build_publication(mut builder: PublicationBuilder) -> Result<PublicationSpec, String> {
    let repository = required(&mut builder.values, "repository", "publication")?;
    let remote = required(&mut builder.values, "remote", "publication")?;
    let base_branch = required(&mut builder.values, "base_branch", "publication")?;
    let expected_base = required(&mut builder.values, "expected_base", "publication")?;
    let head_branch = required(&mut builder.values, "head_branch", "publication")?;
    if repository != "LyannaValerie/pinker-v0"
        || remote != "origin"
        || base_branch != "main"
        || !head_branch.starts_with("agents/")
    {
        return Err("publication: repository/remote/base/head fora da allowlist".to_string());
    }
    let draft = parse_bool(&required(&mut builder.values, "draft", "publication")?, 0)?;
    if draft {
        return Err("publication.draft só aceita false".to_string());
    }
    let result = PublicationSpec {
        repository,
        remote,
        base_branch,
        expected_base,
        head_branch,
        commit_message: required(&mut builder.values, "commit_message", "publication")?,
        changes: builder.changes,
        pr_title: required(&mut builder.values, "pr_title", "publication")?,
        pr_body: required(&mut builder.values, "pr_body", "publication")?,
        draft,
        required_checks: builder.required_checks,
        defer_checks: parse_bool(
            &required(&mut builder.values, "defer_checks", "publication")?,
            0,
        )?,
        poll_seconds: number(
            required(&mut builder.values, "poll_seconds", "publication")?,
            "publication.poll_seconds",
        )?,
        timeout_seconds: number(
            required(&mut builder.values, "timeout_seconds", "publication")?,
            "publication.timeout_seconds",
        )?,
    };
    if !builder.values.is_empty()
        || result.changes.is_empty()
        || result.required_checks.is_empty()
        || result.poll_seconds == 0
        || result.timeout_seconds == 0
    {
        return Err(
            "publication: campo incompatível, lista vazia ou intervalo inválido".to_string(),
        );
    }
    for (index, name) in result.required_checks.iter().enumerate() {
        if result.required_checks[..index].contains(name) {
            return Err(format!(
                "publication.required_check duplicado na spec: {name}"
            ));
        }
    }
    Ok(result)
}

fn build_mutation(id: &str, mut builder: MutationBuilder) -> Result<MutationSpec, String> {
    let result = MutationSpec {
        id: id.to_string(),
        target: required(&mut builder.values, "target", id)?,
        search_file: required(&mut builder.values, "search_file", id)?,
        replacement_file: required(&mut builder.values, "replacement_file", id)?,
        expected_matches: number(
            required(&mut builder.values, "expected_matches", id)?,
            "expected_matches",
        )?,
        probe_program: required(&mut builder.values, "probe_program", id)?,
        probe_argv: builder.argv,
        probe_cwd: builder
            .values
            .remove("probe_cwd")
            .unwrap_or_else(|| ".".to_string()),
        probe_expected_exit: number(
            required(&mut builder.values, "probe_expected_exit", id)?,
            "probe_expected_exit",
        )?,
        probe_stderr_contains: builder.values.remove("probe_stderr_contains"),
    };
    if !builder.values.is_empty() {
        return Err(format!("mutation.{id}: campo incompatível"));
    }
    if matches!(
        result.probe_program.as_str(),
        "sh" | "bash" | "/bin/sh" | "/bin/bash"
    ) {
        return Err(format!("mutation.{id}: shell implícito rejeitado"));
    }
    Ok(result)
}

pub fn parse_spec_text(text: &str) -> Result<Spec, String> {
    let mut schema = None;
    let mut task_id = None;
    let mut repo_root = None;
    let mut worktree = None;
    let mut delegated_root = None;
    let mut expected_base = None;
    let mut accepted_verdict = None;
    let mut blocked_verdict = None;
    let mut human_verdict = None;
    let mut allowed_writes = Vec::new();
    let mut allowed_changes = Vec::new();
    let mut command_order = Vec::new();
    let mut commands: BTreeMap<String, CommandBuilder> = BTreeMap::new();
    let mut check_order = Vec::new();
    let mut checks: BTreeMap<String, CheckBuilder> = BTreeMap::new();
    let mut mutation_order = Vec::new();
    let mut mutations: BTreeMap<String, MutationBuilder> = BTreeMap::new();
    let mut publication: Option<PublicationBuilder> = None;

    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(format!("linha {line_no}: esperado chave = valor"));
        };
        let key = raw_key.trim();
        let value = raw_value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(format!("linha {line_no}: chave ou valor vazio"));
        }
        match key {
            "schema" => assign_once(
                &mut schema,
                value
                    .parse::<u32>()
                    .map_err(|_| format!("linha {line_no}: schema inválido"))?,
                key,
                line_no,
            )?,
            "task_id" => assign_once(&mut task_id, value.to_string(), key, line_no)?,
            "repo_root" => assign_once(&mut repo_root, PathBuf::from(value), key, line_no)?,
            "worktree" => assign_once(&mut worktree, PathBuf::from(value), key, line_no)?,
            "delegated_root" => {
                assign_once(&mut delegated_root, PathBuf::from(value), key, line_no)?
            }
            "expected_base" => assign_once(&mut expected_base, value.to_string(), key, line_no)?,
            "allowed_write" => allowed_writes.push(value.to_string()),
            "allowed_change" => allowed_changes.push(value.to_string()),
            "verdict.accepted" => {
                assign_once(&mut accepted_verdict, value.to_string(), key, line_no)?
            }
            "verdict.blocked" => {
                assign_once(&mut blocked_verdict, value.to_string(), key, line_no)?
            }
            "verdict.human" => assign_once(&mut human_verdict, value.to_string(), key, line_no)?,
            _ if key.starts_with("command.") => {
                let rest = &key[8..];
                let Some((id, field)) = rest.split_once('.') else {
                    return Err(format!("linha {line_no}: campo de comando inválido: {key}"));
                };
                if id.is_empty()
                    || !id
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                {
                    return Err(format!("linha {line_no}: id de comando inválido: {id}"));
                }
                if !commands.contains_key(id) {
                    command_order.push(id.to_string());
                    commands.insert(id.to_string(), CommandBuilder::default());
                }
                let command = commands.get_mut(id).expect("comando inserido");
                match field {
                    "kind" => {
                        let kind = match value {
                            "program" => CommandKind::Program,
                            "pinker" => CommandKind::Pinker,
                            _ => return Err(format!("linha {line_no}: kind inválido: {value}")),
                        };
                        assign_once(&mut command.kind, kind, key, line_no)?;
                    }
                    "program" => {
                        assign_once(&mut command.program, value.to_string(), key, line_no)?
                    }
                    "arg" => command.argv.push(value.to_string()),
                    "cwd" => assign_once(&mut command.cwd, value.to_string(), key, line_no)?,
                    "expect" => assign_once(
                        &mut command.expected_exit,
                        value
                            .parse::<i32>()
                            .map_err(|_| format!("linha {line_no}: exit esperado inválido"))?,
                        key,
                        line_no,
                    )?,
                    "shell" => assign_once(
                        &mut command.shell,
                        parse_bool(value, line_no)?,
                        key,
                        line_no,
                    )?,
                    _ if field.starts_with("env.") => {
                        let name = &field[4..];
                        if name.is_empty()
                            || !name.chars().all(|ch| {
                                ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_'
                            })
                        {
                            return Err(format!(
                                "linha {line_no}: variável não autorizável: {name}"
                            ));
                        }
                        if command
                            .env
                            .insert(name.to_string(), value.to_string())
                            .is_some()
                        {
                            return Err(format!("linha {line_no}: variável duplicada: {name}"));
                        }
                    }
                    _ => return Err(format!("linha {line_no}: campo desconhecido: {key}")),
                }
            }
            _ if key.starts_with("check.") => {
                let rest = &key[6..];
                let Some((id, field)) = rest.split_once('.') else {
                    return Err(format!("linha {line_no}: campo de check inválido: {key}"));
                };
                validate_id(id, "check", line_no)?;
                if !checks.contains_key(id) {
                    check_order.push(id.to_string());
                    checks.insert(id.to_string(), CheckBuilder::default());
                }
                let check = checks.get_mut(id).expect("check inserido");
                if field == "kind" {
                    assign_once(&mut check.kind, value.to_string(), key, line_no)?;
                } else if matches!(
                    field,
                    "expected_change"
                        | "allowed_change"
                        | "expected_key"
                        | "exclude_file"
                        | "exclude_key"
                        | "expected_area"
                        | "expected_validation"
                ) {
                    check
                        .repeated
                        .entry(field.to_string())
                        .or_default()
                        .push(value.to_string());
                } else if let Some(override_key) = field.strip_prefix("override_hash.") {
                    if override_key.is_empty() {
                        return Err(format!("linha {line_no}: override vazio"));
                    }
                    if check
                        .overrides
                        .insert(override_key.to_string(), value.to_string())
                        .is_some()
                    {
                        return Err(format!("linha {line_no}: campo duplicado: {key}"));
                    }
                } else if matches!(
                    field,
                    "expected_head"
                        | "expected_branch"
                        | "require_clean"
                        | "diff_check"
                        | "commit_count_after_base"
                        | "path"
                        | "base_sha256"
                        | "expected_regions"
                        | "expected_marker_lines"
                        | "catalog"
                        | "expected_total"
                        | "expected_evidence"
                        | "expected_runtime"
                        | "expected_length"
                        | "expected_fnv1a64"
                        | "validation_pr_number"
                        | "expected_kind"
                        | "expected_title"
                        | "forbid_sentinel"
                ) {
                    if check
                        .values
                        .insert(field.to_string(), value.to_string())
                        .is_some()
                    {
                        return Err(format!("linha {line_no}: campo duplicado: {key}"));
                    }
                } else {
                    return Err(format!("linha {line_no}: campo desconhecido: {key}"));
                }
            }
            _ if key.starts_with("publication.") => {
                let field = &key[12..];
                let publication = publication.get_or_insert_with(PublicationBuilder::default);
                if field == "change" {
                    publication.changes.push(value.to_string());
                } else if field == "required_check" {
                    publication.required_checks.push(value.to_string());
                } else if matches!(
                    field,
                    "repository"
                        | "remote"
                        | "base_branch"
                        | "expected_base"
                        | "head_branch"
                        | "commit_message"
                        | "pr_title"
                        | "pr_body"
                        | "draft"
                        | "defer_checks"
                        | "poll_seconds"
                        | "timeout_seconds"
                ) {
                    if publication
                        .values
                        .insert(field.to_string(), value.to_string())
                        .is_some()
                    {
                        return Err(format!("linha {line_no}: campo duplicado: {key}"));
                    }
                } else {
                    return Err(format!("linha {line_no}: campo desconhecido: {key}"));
                }
            }
            _ if key.starts_with("mutation.") => {
                let rest = &key[9..];
                let Some((id, field)) = rest.split_once('.') else {
                    return Err(format!(
                        "linha {line_no}: campo de mutation inválido: {key}"
                    ));
                };
                validate_id(id, "mutation", line_no)?;
                if !mutations.contains_key(id) {
                    mutation_order.push(id.to_string());
                    mutations.insert(id.to_string(), MutationBuilder::default());
                }
                let mutation = mutations.get_mut(id).expect("mutation inserida");
                if field == "probe_arg" {
                    mutation.argv.push(value.to_string());
                } else if matches!(
                    field,
                    "target"
                        | "search_file"
                        | "replacement_file"
                        | "expected_matches"
                        | "probe_program"
                        | "probe_cwd"
                        | "probe_expected_exit"
                        | "probe_stderr_contains"
                ) {
                    if mutation
                        .values
                        .insert(field.to_string(), value.to_string())
                        .is_some()
                    {
                        return Err(format!("linha {line_no}: campo duplicado: {key}"));
                    }
                } else {
                    return Err(format!("linha {line_no}: campo desconhecido: {key}"));
                }
            }
            _ => return Err(format!("linha {line_no}: campo desconhecido: {key}")),
        }
    }

    let schema = schema.ok_or("campo obrigatório ausente: schema")?;
    if schema != 1 {
        return Err(format!("schema não suportado: {schema}"));
    }
    let mut built = Vec::new();
    for id in command_order {
        let command = commands.remove(&id).expect("ordem consistente");
        let kind = command
            .kind
            .ok_or_else(|| format!("command.{id}.kind ausente"))?;
        let program = command
            .program
            .ok_or_else(|| format!("command.{id}.program ausente"))?;
        let shell = command.shell.unwrap_or(false);
        if !shell && matches!(program.as_str(), "sh" | "bash" | "/bin/sh" | "/bin/bash") {
            return Err(format!("command.{id}: shell implícito rejeitado"));
        }
        if matches!(kind, CommandKind::Pinker) && program != "pink" {
            return Err(format!(
                "command.{id}: comando Pinker tipado exige programa pink"
            ));
        }
        built.push(CommandSpec {
            id,
            kind,
            program,
            argv: command.argv,
            cwd: command.cwd.unwrap_or_else(|| ".".to_string()),
            expected_exit: command.expected_exit.unwrap_or(0),
            shell,
            env: command.env,
        });
    }
    let mut built_checks = Vec::new();
    for id in check_order {
        built_checks.push(build_check(
            &id,
            checks.remove(&id).expect("ordem consistente"),
        )?);
    }
    let mut built_mutations = Vec::new();
    for id in mutation_order {
        built_mutations.push(build_mutation(
            &id,
            mutations.remove(&id).expect("ordem consistente"),
        )?);
    }
    let spec = Spec {
        schema,
        task_id: task_id.ok_or("campo obrigatório ausente: task_id")?,
        repo_root: repo_root.ok_or("campo obrigatório ausente: repo_root")?,
        worktree: worktree.ok_or("campo obrigatório ausente: worktree")?,
        delegated_root: delegated_root.ok_or("campo obrigatório ausente: delegated_root")?,
        expected_base: expected_base.ok_or("campo obrigatório ausente: expected_base")?,
        allowed_writes,
        allowed_changes,
        commands: built,
        checks: built_checks,
        mutations: built_mutations,
        publication: publication.map(build_publication).transpose()?,
        accepted_verdict: accepted_verdict.ok_or("campo obrigatório ausente: verdict.accepted")?,
        blocked_verdict: blocked_verdict.ok_or("campo obrigatório ausente: verdict.blocked")?,
        human_verdict: human_verdict.ok_or("campo obrigatório ausente: verdict.human")?,
    };
    validate_paths(&spec)?;
    Ok(spec)
}

pub fn load_spec(path: &Path) -> Result<Spec, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("falha ao ler spec '{}': {err}", path.display()))?;
    parse_spec_text(&text)
}
// @pinker-nav:end development.agent.spec

// @pinker-nav:start development.agent.paths
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Confinamento lexical e canônico de repo, worktree, diretório delegado, cwd, escritas e mudanças permitidas; rejeita componentes pai, raízes fora do repositório canônico e qualquer cwd fora do worktree.
fn lexical_clean(path: &Path) -> Result<PathBuf, String> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => clean.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => clean.push(value),
            Component::ParentDir => {
                return Err(format!("caminho com fuga '..': {}", path.display()))
            }
        }
    }
    Ok(clean)
}

fn inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn resolve_under(root: &Path, value: &Path) -> Result<PathBuf, String> {
    let candidate = if value.is_absolute() {
        lexical_clean(value)?
    } else {
        lexical_clean(&root.join(value))?
    };
    if !inside(&candidate, root) {
        return Err(format!(
            "caminho fora da raiz autorizada: {}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

pub fn validate_paths(spec: &Spec) -> Result<(), String> {
    if !spec.repo_root.is_absolute()
        || !spec.worktree.is_absolute()
        || !spec.delegated_root.is_absolute()
    {
        return Err("repo_root, worktree e delegated_root devem ser absolutos".to_string());
    }
    let repo = lexical_clean(&spec.repo_root)?;
    let worktree = lexical_clean(&spec.worktree)?;
    let delegated = lexical_clean(&spec.delegated_root)?;
    if !inside(&worktree, &repo) || !inside(&delegated, &repo) {
        return Err("worktree e delegated_root devem ficar dentro de repo_root".to_string());
    }
    for path in &spec.allowed_writes {
        resolve_under(&delegated, Path::new(path))?;
    }
    for path in &spec.allowed_changes {
        resolve_under(&worktree, Path::new(path))?;
    }
    for command in &spec.commands {
        resolve_under(&worktree, Path::new(&command.cwd))?;
    }
    for check in &spec.checks {
        match check {
            CheckSpec::Git(check) => {
                for value in check.expected_changes.iter().chain(&check.allowed_changes) {
                    validate_relative_path(value)?;
                }
            }
            CheckSpec::MarkerOnly(check) => {
                validate_relative_path(&check.path)?;
                resolve_under(&worktree, Path::new(&check.path))?;
            }
            CheckSpec::Projection(check) => {
                validate_relative_path(&check.catalog)?;
                resolve_under(&worktree, Path::new(&check.catalog))?;
                for value in &check.exclude_files {
                    validate_relative_path(value)?;
                }
                if check.exclude_keys.iter().any(|value| value.is_empty()) {
                    return Err("exclude_key vazio".to_string());
                }
            }
            CheckSpec::PrBody(check) => {
                validate_relative_path(&check.path)?;
                resolve_under(&delegated, Path::new(&check.path))?;
            }
        }
    }
    for mutation in &spec.mutations {
        validate_relative_path(&mutation.target)?;
        resolve_under(&worktree, Path::new(&mutation.target))?;
        resolve_under(&delegated, Path::new(&mutation.search_file))?;
        resolve_under(&delegated, Path::new(&mutation.replacement_file))?;
        resolve_under(&worktree, Path::new(&mutation.probe_cwd))?;
    }
    if let Some(publication) = &spec.publication {
        validate_relative_path(&publication.pr_body)?;
        resolve_under(&delegated, Path::new(&publication.pr_body))?;
        for change in &publication.changes {
            validate_relative_path(change)?;
            resolve_under(&worktree, Path::new(change))?;
        }
        if publication.expected_base != spec.expected_base {
            return Err("publication.expected_base diverge de expected_base".to_string());
        }
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(format!("path relativo inválido: {value}"));
    }
    Ok(())
}
// @pinker-nav:end development.agent.paths

// @pinker-nav:start development.agent.artifacts
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Escrita atômica de JSON/Markdown, append durável de eventos monotônicos, escape JSON, SHA-256 zero-dependency e manifesto terminal ordenado que deliberadamente exclui a si próprio.
fn json_escape(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("arquivo sem pai: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("falha ao criar '{}': {err}", parent.display()))?;
    let tmp = parent.join(format!(
        ".{}.agent-tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&tmp, content)
        .map_err(|err| format!("falha ao escrever '{}': {err}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|err| format!("falha ao substituir '{}': {err}", path.display()))
}

fn append_event(
    path: &Path,
    sequence: u64,
    command: &str,
    status: &str,
    exit: Option<i32>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    writeln!(
        file,
        "{{\"sequence\":{sequence},\"command\":{},\"status\":{},\"exit_code\":{}}}",
        json_escape(command),
        json_escape(status),
        exit.map_or("null".to_string(), |code| code.to_string())
    )
    .map_err(|err| err.to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let o = i * 4;
            *word = u32::from_be_bytes(chunk[o..o + 4].try_into().expect("quatro bytes"));
        }
        for i in 16..64 {
            let a = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let b = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(a)
                .wrapping_add(w[i - 7])
                .wrapping_add(b);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut digest = String::with_capacity(64);
    for value in state {
        write!(&mut digest, "{value:08x}").expect("escrita em String");
    }
    digest
}

fn artifact_manifest(root: &Path) -> Result<String, String> {
    let artifacts = root.join("artefatos");
    let mut entries = Vec::new();
    for entry in fs::read_dir(artifacts).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_file()
            || path.file_name().and_then(|v| v.to_str()) == Some("artifact-manifest.json")
        {
            continue;
        }
        let bytes = fs::read(&path).map_err(|err| err.to_string())?;
        entries.push((
            path.file_name().unwrap().to_string_lossy().into_owned(),
            sha256_hex(&bytes),
        ));
    }
    entries.sort();
    let body = entries
        .iter()
        .map(|(name, hash)| format!("    {}: {}", json_escape(name), json_escape(hash)))
        .collect::<Vec<_>>()
        .join(",\n");
    Ok(format!("{{\n  \"schema\": 1,\n  \"algorithm\": \"SHA-256\",\n  \"self_included\": false,\n  \"artifacts\": {{\n{body}\n  }}\n}}\n"))
}
// @pinker-nav:end development.agent.artifacts

// @pinker-nav:start development.agent.runner
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Executor de processos estruturados ou Pinker tipado: resolve deterministicamente o executável corrente e o substituto indicado pelo sufixo Linux ` (deleted)`, mantém cwd e env confinados, shell somente quando declarado, captura simultânea de stdout/stderr, persistência por comando, eco terminal, duração e comparação do código observado com o esperado.
#[derive(Clone, Debug)]
struct CommandResult {
    id: String,
    status: &'static str,
    exit_code: Option<i32>,
    expected_exit: i32,
    duration_ms: u128,
    shell: bool,
}

fn resolve_pinker_executable(current: &Path) -> Result<PathBuf, String> {
    if current.exists() {
        return Ok(current.to_path_buf());
    }

    let display = current.to_string_lossy();
    let replacement = display
        .strip_suffix(" (deleted)")
        .map(PathBuf::from)
        .ok_or_else(|| format!("executável Pinker corrente indisponível: {display}"))?;
    if replacement.exists() {
        Ok(replacement)
    } else {
        Err(format!(
            "executável Pinker corrente indisponível: {display}; substituto ausente: {}",
            replacement.display()
        ))
    }
}

fn execute_one(spec: &Spec, command: &CommandSpec) -> Result<(CommandResult, Output), String> {
    let cwd = resolve_under(&spec.worktree, Path::new(&command.cwd))?;
    let mut process = if command.shell {
        let mut shell = Command::new("/bin/sh");
        shell.arg("-c").arg(&command.program);
        shell.args(&command.argv);
        shell
    } else if matches!(command.kind, CommandKind::Pinker) {
        let current = env::current_exe().map_err(|err| err.to_string())?;
        let executable = resolve_pinker_executable(&current)?;
        let mut pink = Command::new(executable);
        pink.args(&command.argv);
        pink
    } else {
        let mut program = Command::new(&command.program);
        program.args(&command.argv);
        program
    };
    process.current_dir(cwd).env_clear();
    for key in ["PATH", "LANG", "LC_ALL", "TERM"] {
        if let Some(value) = env::var_os(key) {
            process.env(key, value);
        }
    }
    for (key, value) in &command.env {
        process.env(key, value);
    }
    let started = Instant::now();
    let output = process
        .output()
        .map_err(|err| format!("falha de harness em '{}': {err}", command.id))?;
    let exit_code = output.status.code();
    let status = if exit_code == Some(command.expected_exit) {
        "PASSED"
    } else {
        "FAILED"
    };
    Ok((
        CommandResult {
            id: command.id.clone(),
            status,
            exit_code,
            expected_exit: command.expected_exit,
            duration_ms: started.elapsed().as_millis(),
            shell: command.shell,
        },
        output,
    ))
}

// @pinker-nav:end development.agent.runner

// @pinker-nav:start development.agent.git-diff
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Verifica Git de modo somente leitura: HEAD, branch, limpeza, contagem desde a base, conjunto exato e ordenado do porcelain v1, subconjunto permitido e git diff --check, preservando inclusive a primeira linha.
fn run_git_check(spec: &Spec, check: &GitCheck) -> Result<String, String> {
    let changed = changed_paths(spec)?;
    let changed_set: BTreeSet<_> = changed.iter().cloned().collect();
    let expected: BTreeSet<_> = check.expected_changes.iter().cloned().collect();
    let allowed: BTreeSet<_> = check.allowed_changes.iter().cloned().collect();
    let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    let branch = git_output(&spec.worktree, &["branch", "--show-current"])?;
    let count = check
        .commit_count_after_base
        .map(|_| {
            git_output(
                &spec.worktree,
                &[
                    "rev-list",
                    "--count",
                    &format!("{}..HEAD", spec.expected_base),
                ],
            )?
            .parse::<u64>()
            .map_err(|_| "contagem Git inválida".to_string())
        })
        .transpose()?;
    let diff = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args(["diff", "--check"])
        .output()
        .map_err(|err| err.to_string())?;
    let exact = check.expected_changes.is_empty() || changed_set == expected;
    let allowed_subset = allowed.is_subset(&changed_set);
    let clean_ok = !check.require_clean || changed.is_empty();
    let head_ok = check
        .expected_head
        .as_ref()
        .map_or(true, |value| value == &head);
    let branch_ok = check
        .expected_branch
        .as_ref()
        .map_or(true, |value| value == &branch);
    let count_ok = check.commit_count_after_base == count;
    let diff_ok = !check.diff_check || diff.status.success();
    let passed = exact && allowed_subset && clean_ok && head_ok && branch_ok && count_ok && diff_ok;
    let paths = changed
        .iter()
        .map(|value| json_escape(value))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{{\"id\":{},\"kind\":\"git\",\"passed\":{passed},\"head\":{},\"branch\":{},\"clean\":{},\"commit_count_after_base\":{},\"exact\":{exact},\"allowed_subset\":{allowed_subset},\"diff_check\":{diff_ok},\"paths\":[{paths}]}}", json_escape(&check.id), json_escape(&head), json_escape(&branch), changed.is_empty(), count.map_or("null".to_string(), |value| value.to_string())))
}
// @pinker-nav:end development.agent.git-diff

// @pinker-nav:start development.agent.marker-only
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Reconstrói fontes removendo somente linhas de comentários Pinker reais, com estado léxico alinhado à política canônica, e prova SHA-256 base, quantidade, cinco linhas por região, balanceamento e ordem exata das chaves sem modificar o arquivo.
#[derive(Clone, Copy)]
enum MarkerLex {
    Code,
    String,
    ByteString,
    Raw(usize),
    RawByte(usize),
    Block(usize),
}

fn raw_start(bytes: &[u8]) -> Option<(usize, usize, bool)> {
    let (prefix, byte) = if bytes.starts_with(b"br") {
        (2, true)
    } else if bytes.starts_with(b"r") {
        (1, false)
    } else {
        return None;
    };
    let mut pos = prefix;
    while bytes.get(pos) == Some(&b'#') {
        pos += 1;
    }
    (bytes.get(pos) == Some(&b'\"')).then_some((pos + 1, pos - prefix, byte))
}

fn char_len(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'\'') {
        return None;
    }
    if bytes.get(1) == Some(&b'\\') {
        return (bytes.get(3) == Some(&b'\'')).then_some(4);
    }
    let ch = std::str::from_utf8(bytes.get(1..)?).ok()?.chars().next()?;
    let close = 1 + ch.len_utf8();
    (bytes.get(close) == Some(&b'\'')).then_some(close + 1)
}

fn real_comment<'a>(line: &'a str, state: &mut MarkerLex) -> Option<&'a str> {
    let bytes = line.as_bytes();
    let first = bytes.iter().position(|byte| !byte.is_ascii_whitespace());
    let mut i = 0;
    while i < bytes.len() {
        match *state {
            MarkerLex::Code => {
                if bytes[i..].starts_with(b"//") {
                    return (Some(i) == first).then_some(&line[i..]);
                }
                if bytes[i..].starts_with(b"/*") {
                    *state = MarkerLex::Block(1);
                    i += 2;
                } else if let Some((len, hashes, byte)) = raw_start(&bytes[i..]) {
                    *state = if byte {
                        MarkerLex::RawByte(hashes)
                    } else {
                        MarkerLex::Raw(hashes)
                    };
                    i += len;
                } else if bytes[i..].starts_with(b"b\"") {
                    *state = MarkerLex::ByteString;
                    i += 2;
                } else if let Some(len) = char_len(&bytes[i..]) {
                    i += len;
                } else if bytes[i] == b'\"' {
                    *state = MarkerLex::String;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            MarkerLex::String | MarkerLex::ByteString => {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == b'\"' {
                    *state = MarkerLex::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            MarkerLex::Raw(hashes) | MarkerLex::RawByte(hashes) => {
                if bytes[i] == b'\"' && bytes[i + 1..].starts_with(&vec![b'#'; hashes]) {
                    *state = MarkerLex::Code;
                    i += hashes + 1;
                } else {
                    i += 1;
                }
            }
            MarkerLex::Block(depth) => {
                if bytes[i..].starts_with(b"/*") {
                    *state = MarkerLex::Block(depth + 1);
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    *state = if depth == 1 {
                        MarkerLex::Code
                    } else {
                        MarkerLex::Block(depth - 1)
                    };
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
    None
}

fn marker_key(comment: &str, marker: &str) -> Option<String> {
    let body = comment.strip_prefix("//")?;
    if body.starts_with('/') || body.starts_with('!') {
        return None;
    }
    let rest = body.trim_start().strip_prefix(marker)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let key = rest.trim();
    (!key.is_empty()).then(|| key.to_string())
}

fn is_pinker_marker(comment: &str) -> bool {
    let Some(body) = comment.strip_prefix("//") else {
        return false;
    };
    if body.starts_with('/') || body.starts_with('!') {
        return false;
    }
    matches!(
        body.split_whitespace().next(),
        Some(
            "@pinker-nav:domain"
                | "@pinker-nav:layer"
                | "@pinker-nav:summary"
                | "@pinker-nav:kind"
                | "@pinker-nav:status"
                | "@pinker-nav:phase"
        )
    ) || marker_key(comment, "@pinker-nav:start").is_some()
        || marker_key(comment, "@pinker-nav:end").is_some()
}

fn run_marker_check(spec: &Spec, check: &MarkerOnlyCheck) -> Result<String, String> {
    let path = resolve_under(&spec.worktree, Path::new(&check.path))?;
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let text = std::str::from_utf8(&bytes).map_err(|err| err.to_string())?;
    let mut state = MarkerLex::Code;
    let mut rebuilt = Vec::new();
    let mut open: Option<(String, usize)> = None;
    let mut keys = Vec::new();
    let mut marker_lines = 0usize;
    let mut valid = true;
    for line in text.split_inclusive('\n') {
        let raw = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or(line.strip_suffix('\n').unwrap_or(line));
        let comment = real_comment(raw, &mut state);
        if let Some(comment) = comment.filter(|value| is_pinker_marker(value)) {
            marker_lines += 1;
            if let Some(key) = marker_key(comment, "@pinker-nav:start") {
                if open.is_some() {
                    valid = false;
                }
                open = Some((key.clone(), marker_lines));
                keys.push(key);
            } else if let Some(key) = marker_key(comment, "@pinker-nav:end") {
                match open.take() {
                    Some((start, first)) if start == key && marker_lines - first + 1 == 5 => {}
                    _ => valid = false,
                }
            } else if open.is_none() {
                valid = false;
            }
        } else {
            rebuilt.extend_from_slice(line.as_bytes());
        }
    }
    valid &= open.is_none();
    let reconstructed = sha256_hex(&rebuilt);
    valid &= reconstructed == check.base_sha256;
    valid &= keys.len() == check.expected_regions;
    valid &= marker_lines == check.expected_marker_lines;
    valid &= keys == check.expected_keys;
    let key_json = keys
        .iter()
        .map(|value| json_escape(value))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{{\"id\":{},\"kind\":\"marker-only\",\"passed\":{valid},\"path\":{},\"regions\":{},\"marker_lines\":{marker_lines},\"reconstructed_sha256\":{},\"keys\":[{key_json}]}}", json_escape(&check.id), json_escape(&check.path), keys.len(), json_escape(&reconstructed)))
}
// @pinker-nav:end development.agent.marker-only

// @pinker-nav:start development.agent.projection
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Carrega catálogo JSONL validado, rejeita chaves duplicadas, aplica exclusões exatas e overrides consumidos, ordena a projeção estável de nove campos e mede total, evidence/runtime, bytes e FNV-1a64 sem escrever nas fontes.
fn fnv1a64_number(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn run_projection_check(spec: &Spec, check: &ProjectionCheck) -> Result<String, String> {
    let path = resolve_under(&spec.worktree, Path::new(&check.catalog))?;
    let mut catalog = crate::nav::CodeCatalog::load(&path).map_err(|err| err.to_string())?;
    let mut all_keys = BTreeSet::new();
    if catalog
        .regions
        .iter()
        .any(|region| !all_keys.insert(region.key.clone()))
    {
        return Err("catálogo contém chave duplicada".to_string());
    }
    catalog.regions.retain(|region| {
        !check.exclude_files.iter().any(|file| file == &region.file)
            && !check.exclude_keys.iter().any(|key| key == &region.key)
    });
    let mut used = BTreeSet::new();
    for region in &mut catalog.regions {
        if let Some(hash) = check.override_hashes.get(&region.key) {
            region.hash.clone_from(hash);
            used.insert(region.key.clone());
        }
    }
    if used.len() != check.override_hashes.len() {
        return Err("override de hash não usado".to_string());
    }
    let mut records = catalog
        .regions
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
                    region.status.as_str()
                )
            )
        })
        .collect::<Vec<_>>();
    records.sort_unstable();
    let projection = records.concat();
    let total = catalog.regions.len();
    let evidence = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("evidencia."))
        .count();
    let runtime = catalog
        .regions
        .iter()
        .filter(|region| region.layer.as_deref() == Some("runtime"))
        .count();
    let length = projection.len();
    let hash = format!("{:016x}", fnv1a64_number(projection.as_bytes()));
    let passed = total == check.expected_total
        && evidence == check.expected_evidence
        && runtime == check.expected_runtime
        && length == check.expected_length
        && hash == check.expected_fnv1a64;
    Ok(format!("{{\"id\":{},\"kind\":\"projection\",\"passed\":{passed},\"total\":{total},\"evidence\":{evidence},\"runtime\":{runtime},\"length\":{length},\"fnv1a64\":{},\"overrides_used\":{}}}", json_escape(&check.id), json_escape(&hash), used.len()))
}
// @pinker-nav:end development.agent.projection

// @pinker-nav:start development.agent.pr-body
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Valida um único bloco pinker-change UTF-8 confinado ao delegado, sem sentinel ou comentário interno, compara kind, title, áreas e validações e registra execução canônica e SHA-256.
fn run_pr_body_check(spec: &Spec, check: &PrBodyCheck) -> Result<String, String> {
    let path = resolve_under(&spec.delegated_root, Path::new(&check.path))?;
    let bytes = fs::read(&path).map_err(|err| format!("falha ao ler body: {err}"))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "body não é UTF-8".to_string())?;
    let opens = text.match_indices("```pinker-change").collect::<Vec<_>>();
    if opens.len() != 1 {
        return Err(format!(
            "body exige exatamente um bloco pinker-change; observado {}",
            opens.len()
        ));
    }
    let tail = &text[opens[0].0 + "```pinker-change".len()..];
    let end = tail
        .find("```")
        .ok_or("bloco pinker-change sem fechamento")?;
    let block = &tail[..end];
    if block.lines().any(|line| line.trim_start().starts_with('#')) {
        return Err("comentário dentro do bloco pinker-change".to_string());
    }
    if check.forbid_sentinel && block.contains("<preencher-") {
        return Err("sentinel proibido no body".to_string());
    }
    let scalar = |name: &str| -> Option<String> {
        block.lines().find_map(|line| {
            line.trim()
                .strip_prefix(&format!("{name}:"))
                .map(|value| value.trim().to_string())
        })
    };
    let list = |name: &str| -> Vec<String> {
        let mut active = false;
        let mut values = Vec::new();
        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed == format!("{name}:") {
                active = true;
                continue;
            }
            if active {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    values.push(value.trim().to_string());
                } else if !trimmed.is_empty() && !line.starts_with(' ') {
                    break;
                }
            }
        }
        values
    };
    let kind = scalar("kind").ok_or("kind ausente no body")?;
    let title = scalar("title").ok_or("title ausente no body")?;
    let areas = list("area");
    let required = block
        .find("  required:")
        .map(|start| &block[start..])
        .unwrap_or("");
    let validations = required
        .lines()
        .skip(1)
        .take_while(|line| line.trim().starts_with("- ") || line.trim().is_empty())
        .filter_map(|line| line.trim().strip_prefix("- ").map(str::to_string))
        .collect::<Vec<_>>();
    if kind != check.expected_kind
        || title != check.expected_title
        || areas != check.expected_areas
        || validations != check.expected_validations
    {
        return Err("kind/title/area/validation divergente no body".to_string());
    }
    let current = env::current_exe().map_err(|err| err.to_string())?;
    let pink = resolve_pinker_executable(&current)?;
    let argv = [
        "doc".to_string(),
        "importar-pr".to_string(),
        check.validation_pr_number.to_string(),
        "--corpo".to_string(),
        path.display().to_string(),
        "--check".to_string(),
        "--repo".to_string(),
        spec.worktree.display().to_string(),
    ];
    let output = Command::new(&pink)
        .args(&argv)
        .output()
        .map_err(|err| err.to_string())?;
    atomic_write(
        &spec
            .delegated_root
            .join(format!("logs/{}.stdout.txt", check.id)),
        &output.stdout,
    )?;
    atomic_write(
        &spec
            .delegated_root
            .join(format!("logs/{}.stderr.txt", check.id)),
        &output.stderr,
    )?;
    if !output.status.success() {
        return Err(format!(
            "validação canônica do body falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let digest = sha256_hex(&bytes);
    atomic_write(
        &spec.delegated_root.join("artefatos/pr-body.json"),
        format!(
            "{{\n  \"path\": {},\n  \"sha256\": {},\n  \"program\": {},\n  \"argv\": [{}],\n  \"exit_code\": 0\n}}\n",
            json_escape(&check.path),
            json_escape(&digest),
            json_escape(&pink.display().to_string()),
            argv.iter().map(|arg| json_escape(arg)).collect::<Vec<_>>().join(",")
        ).as_bytes(),
    )?;
    Ok(format!(
        "{{\"id\":{},\"kind\":\"pr-body\",\"passed\":true,\"sha256\":{}}}",
        json_escape(&check.id),
        json_escape(&digest)
    ))
}
// @pinker-nav:end development.agent.pr-body

fn run_checks(spec: &Spec) -> Result<(bool, Vec<String>), String> {
    let mut results = Vec::new();
    for check in &spec.checks {
        let (id, kind, outcome) = match check {
            CheckSpec::Git(check) => (&check.id, "git", run_git_check(spec, check)),
            CheckSpec::MarkerOnly(check) => {
                (&check.id, "marker-only", run_marker_check(spec, check))
            }
            CheckSpec::Projection(check) => {
                (&check.id, "projection", run_projection_check(spec, check))
            }
            CheckSpec::PrBody(check) => (&check.id, "pr-body", run_pr_body_check(spec, check)),
        };
        let result = match outcome {
            Ok(result) => result,
            Err(error) => format!(
                "{{\"id\":{},\"kind\":{},\"passed\":false,\"error\":{}}}",
                json_escape(id),
                json_escape(kind),
                json_escape(&error)
            ),
        };
        results.push(result);
    }
    let passed = results
        .iter()
        .all(|result| result.contains("\"passed\":true"));
    Ok((passed, results))
}

#[derive(Clone, Debug)]
struct PublicationState {
    status: String,
    spec_hash: String,
    candidate: String,
    parent: String,
    tree: String,
    pr_number: Option<u64>,
    pr_url: Option<String>,
    body_digest: String,
}

fn json_text_field(text: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let tail = text.split_once(&needle)?.1.trim_start();
    if !tail.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in tail[1..].chars() {
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

fn json_u64_field(text: &str, field: &str) -> Option<u64> {
    let needle = format!("\"{field}\":");
    let tail = text.split_once(&needle)?.1.trim_start();
    tail.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

fn publication_state_path(spec: &Spec) -> PathBuf {
    spec.delegated_root.join("estado/publication-state.json")
}

fn save_publication_state(spec: &Spec, state: &PublicationState) -> Result<(), String> {
    atomic_write(
        &publication_state_path(spec),
        format!(
            "{{\n  \"schema\": 1,\n  \"status\": {},\n  \"spec_hash\": {},\n  \"candidate\": {},\n  \"parent\": {},\n  \"tree\": {},\n  \"pr_number\": {},\n  \"pr_url\": {},\n  \"body_digest\": {}\n}}\n",
            json_escape(&state.status),
            json_escape(&state.spec_hash),
            json_escape(&state.candidate),
            json_escape(&state.parent),
            json_escape(&state.tree),
            state.pr_number.map_or("null".to_string(), |n| n.to_string()),
            state.pr_url.as_ref().map_or("null".to_string(), |v| json_escape(v)),
            json_escape(&state.body_digest)
        ).as_bytes(),
    )
}

fn load_publication_state(spec: &Spec) -> Result<PublicationState, String> {
    let text = fs::read_to_string(publication_state_path(spec)).map_err(|err| err.to_string())?;
    Ok(PublicationState {
        status: json_text_field(&text, "status").ok_or("publication status ausente")?,
        spec_hash: json_text_field(&text, "spec_hash").ok_or("publication spec_hash ausente")?,
        candidate: json_text_field(&text, "candidate").unwrap_or_default(),
        parent: json_text_field(&text, "parent").unwrap_or_default(),
        tree: json_text_field(&text, "tree").unwrap_or_default(),
        pr_number: json_u64_field(&text, "pr_number"),
        pr_url: json_text_field(&text, "pr_url"),
        body_digest: json_text_field(&text, "body_digest").unwrap_or_default(),
    })
}

fn publication_event(spec: &Spec, status: &str) -> Result<(), String> {
    let path = spec.delegated_root.join("estado/publication-events.jsonl");
    let sequence = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .count() as u64
        + 1;
    append_event(&path, sequence, "publication", status, None)
}

fn set_publication_status(
    spec: &Spec,
    state: &mut PublicationState,
    status: &str,
) -> Result<(), String> {
    state.status = status.to_string();
    save_publication_state(spec, state)?;
    publication_event(spec, status)
}

fn run_captured(
    spec: &Spec,
    prefix: &str,
    program: &str,
    args: &[String],
) -> Result<Output, String> {
    let count = fs::read_dir(spec.delegated_root.join("logs"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
                .count()
                + 1
        })
        .unwrap_or(1);
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| err.to_string())?;
    atomic_write(
        &spec
            .delegated_root
            .join(format!("logs/{prefix}-{count}.stdout.txt")),
        &output.stdout,
    )?;
    atomic_write(
        &spec
            .delegated_root
            .join(format!("logs/{prefix}-{count}.stderr.txt")),
        &output.stderr,
    )?;
    Ok(output)
}

fn run_gh(spec: &Spec, operation: &str, args: &[String]) -> Result<Output, String> {
    let allowed = matches!(
        args.get(0..2).map(|v| (v[0].as_str(), v[1].as_str())),
        Some(("pr", "list" | "create" | "view" | "checks"))
    );
    if !allowed {
        return Err("comando GH não autorizado".to_string());
    }
    run_captured(spec, &format!("gh-{operation}"), "gh", args)
}

fn output_text(output: &Output, label: &str) -> Result<String, String> {
    if !output.status.success() {
        return Err(format!(
            "{label} falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout.clone())
        .map(|value| value.trim().to_string())
        .map_err(|_| format!("{label}: stdout não UTF-8"))
}

fn publication(spec: &Spec) -> Result<&PublicationSpec, String> {
    spec.publication
        .as_ref()
        .ok_or("publication ausente".to_string())
}

fn exact_changed(spec: &Spec, publication: &PublicationSpec) -> Result<(), String> {
    let changed = changed_paths(spec)?;
    let mut expected = publication.changes.clone();
    expected.sort();
    expected.dedup();
    if changed != expected {
        return Err(format!(
            "changed set divergente: esperado {expected:?}, observado {changed:?}"
        ));
    }
    let cached = git_output(&spec.worktree, &["diff", "--cached", "--name-only"])?;
    if !cached.is_empty() {
        return Err("index pré-staged".to_string());
    }
    if publication
        .changes
        .iter()
        .any(|path| matches!(path.as_str(), "Cargo.toml" | "Cargo.lock"))
    {
        return Err("Cargo não pode integrar publication.change".to_string());
    }
    let diff = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args(["diff", "--check"])
        .output()
        .map_err(|err| err.to_string())?;
    if !diff.status.success() {
        return Err("git diff --check falhou".to_string());
    }
    Ok(())
}

fn remote_head(spec: &Spec, publication: &PublicationSpec) -> Result<Option<String>, String> {
    let output = run_captured(
        spec,
        "git-ls-remote",
        "git",
        &[
            "-C".to_string(),
            spec.worktree.display().to_string(),
            "ls-remote".to_string(),
            "--heads".to_string(),
            publication.remote.clone(),
            format!("refs/heads/{}", publication.head_branch),
        ],
    )?;
    let text = output_text(&output, "git ls-remote")?;
    Ok(text.split_whitespace().next().map(str::to_string))
}

fn list_pr_numbers(spec: &Spec, publication: &PublicationSpec) -> Result<Vec<u64>, String> {
    let output = run_gh(
        spec,
        "pr-list",
        &[
            "pr".into(),
            "list".into(),
            "--repo".into(),
            publication.repository.clone(),
            "--state".into(),
            "all".into(),
            "--head".into(),
            publication.head_branch.clone(),
            "--base".into(),
            publication.base_branch.clone(),
            "--limit".into(),
            "10".into(),
            "--json".into(),
            "number".into(),
            "--jq".into(),
            ".[].number".into(),
        ],
    )?;
    let text = output_text(&output, "gh pr list")?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.trim()
                .parse::<u64>()
                .map_err(|_| "número de PR inválido".to_string())
        })
        .collect()
}

struct RemotePr {
    number: u64,
    state: String,
    draft: bool,
    merged: bool,
    base: String,
    head: String,
    sha: String,
    title: String,
    url: String,
    auto_merge: bool,
    body: String,
}

fn read_pr(spec: &Spec, publication: &PublicationSpec, number: u64) -> Result<RemotePr, String> {
    let fields = run_gh(spec, "pr-view", &[
        "pr".into(), "view".into(), number.to_string(), "--repo".into(), publication.repository.clone(),
        "--json".into(), "state,isDraft,mergedAt,baseRefName,headRefName,headRefOid,title,url,autoMergeRequest".into(),
        "--jq".into(), "[.state,.isDraft,(.mergedAt != null),.baseRefName,.headRefName,.headRefOid,.title,.url,(.autoMergeRequest != null)] | @tsv".into(),
    ])?;
    let fields = output_text(&fields, "gh pr view")?;
    let values = fields.split('\t').collect::<Vec<_>>();
    if values.len() != 9 {
        return Err("gh pr view retornou campos incompatíveis".to_string());
    }
    let body = run_gh(
        spec,
        "pr-view-body",
        &[
            "pr".into(),
            "view".into(),
            number.to_string(),
            "--repo".into(),
            publication.repository.clone(),
            "--json".into(),
            "body".into(),
            "--jq".into(),
            ".body".into(),
        ],
    )?;
    let body = String::from_utf8(body.stdout).map_err(|_| "body remoto não UTF-8".to_string())?;
    atomic_write(
        &spec.delegated_root.join("artefatos/pr-body-remote.md"),
        body.as_bytes(),
    )?;
    Ok(RemotePr {
        number,
        state: values[0].to_string(),
        draft: values[1] == "true",
        merged: values[2] == "true",
        base: values[3].to_string(),
        head: values[4].to_string(),
        sha: values[5].to_string(),
        title: values[6].to_string(),
        url: values[7].to_string(),
        auto_merge: values[8] == "true",
        body,
    })
}

fn require_compatible_pr(
    pr: &RemotePr,
    publication: &PublicationSpec,
    candidate: &str,
) -> Result<(), String> {
    if pr.state != "OPEN"
        || pr.draft
        || pr.merged
        || pr.base != publication.base_branch
        || pr.head != publication.head_branch
        || pr.sha != candidate
        || pr.title != publication.pr_title
        || pr.auto_merge
    {
        return Err("PR remota incompatível".to_string());
    }
    Ok(())
}

fn semantic_body(value: &str) -> String {
    value.replace("\r\n", "\n").trim_end().to_string()
}

fn verify_remote_body(
    spec: &Spec,
    publication: &PublicationSpec,
    pr: &RemotePr,
) -> Result<String, String> {
    let local_path = resolve_under(&spec.delegated_root, Path::new(&publication.pr_body))?;
    let local = fs::read_to_string(local_path).map_err(|err| err.to_string())?;
    if semantic_body(&local) != semantic_body(&pr.body) {
        return Err("body remoto diverge semanticamente do local".to_string());
    }
    let check = spec
        .checks
        .iter()
        .find_map(|check| match check {
            CheckSpec::PrBody(check) => Some(check.clone()),
            _ => None,
        })
        .ok_or("check pr-body ausente")?;
    let mut real = check;
    real.validation_pr_number = pr.number;
    run_pr_body_check(spec, &real)?;
    Ok(sha256_hex(pr.body.as_bytes()))
}

// @pinker-nav:start development.agent.publication
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Publica com precondições estritas, intenções duráveis, staging por paths exatos, commit único, push normal e criação ou reconciliação de uma única PR sem edição, merge ou auto-merge.
pub fn publicar(spec_path: &Path) -> Result<i32, String> {
    let spec_bytes = fs::read(spec_path).map_err(|err| err.to_string())?;
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let publication = publication(&spec)?.clone();
    if publication.draft {
        return Err("draft true rejeitado".to_string());
    }
    let spec_hash = sha256_hex(&spec_bytes);
    let local_result = fs::read_to_string(spec.delegated_root.join("artefatos/resultado.json"))
        .map_err(|_| "executar não comprovado para o spec".to_string())?;
    let sensitivity = fs::read_to_string(spec.delegated_root.join("artefatos/sensitivity.json"))
        .map_err(|_| "sensibilidade não comprovada para o spec".to_string())?;
    if !local_result.contains("\"status\": \"ACCEPTED\"")
        || !sensitivity.contains("\"passed\":true")
        || !sensitivity.contains("\"undetected\":[]")
        || !sensitivity.contains("\"harness_errors\":[]")
        || !sensitivity.contains("\"restoration_verified\":true")
    {
        return Err("execução local ou sensibilidade não aceita".to_string());
    }
    let actual_manifest =
        fs::read_to_string(spec.delegated_root.join("artefatos/artifact-manifest.json"))
            .map_err(|_| "manifesto de artefatos ausente".to_string())?;
    if actual_manifest != artifact_manifest(&spec.delegated_root)? {
        return Err("manifesto de artefatos divergente".to_string());
    }
    atomic_write(
        &spec
            .delegated_root
            .join("artefatos/publication-spec.sha256"),
        format!("{spec_hash}\n").as_bytes(),
    )?;
    let mut state = PublicationState {
        status: "LOCAL_ACCEPTED".into(),
        spec_hash,
        candidate: String::new(),
        parent: publication.expected_base.clone(),
        tree: String::new(),
        pr_number: None,
        pr_url: None,
        body_digest: String::new(),
    };
    save_publication_state(&spec, &state)?;
    publication_event(&spec, "LOCAL_ACCEPTED")?;
    let current = env::current_exe().map_err(|err| err.to_string())?;
    let pink = resolve_pinker_executable(&current)?;
    for surface in ["nav", "doc"] {
        let output = run_captured(
            &spec,
            &format!("pink-{surface}-verify"),
            &pink.display().to_string(),
            &[surface.to_string(), "verificar".to_string()],
        )?;
        if !output.status.success() {
            return Err(format!("pink {surface} verificar falhou"));
        }
    }
    let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    let branch = git_output(&spec.worktree, &["branch", "--show-current"])?;
    let count = git_output(
        &spec.worktree,
        &[
            "rev-list",
            "--count",
            &format!("{}..HEAD", publication.expected_base),
        ],
    )?;
    if head != publication.expected_base || branch != publication.head_branch || count != "0" {
        return Err("base/branch/commit count de publicação divergente".to_string());
    }
    exact_changed(&spec, &publication)?;
    let (checks_passed, _) = run_checks(&spec)?;
    if !checks_passed {
        return Err("checks locais não aceitos".to_string());
    }
    if remote_head(&spec, &publication)?.is_some()
        || !list_pr_numbers(&spec, &publication)?.is_empty()
    {
        return Err("branch remota ou PR preexistente".to_string());
    }
    set_publication_status(&spec, &mut state, "COMMIT_INTENT")?;
    for path in &publication.changes {
        let output = Command::new("git")
            .arg("-C")
            .arg(&spec.worktree)
            .args(["add", "--", path])
            .output()
            .map_err(|err| err.to_string())?;
        if !output.status.success() {
            return Err("git add por path falhou".to_string());
        }
    }
    let mut indexed = git_output(&spec.worktree, &["diff", "--cached", "--name-only"])?
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    indexed.sort();
    let mut expected = publication.changes.clone();
    expected.sort();
    if indexed != expected {
        return Err("index não corresponde ao conjunto exato".to_string());
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args(["commit", "-m", &publication.commit_message])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "commit falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    state.candidate = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    state.parent = git_output(&spec.worktree, &["rev-parse", "HEAD^"])?;
    state.tree = git_output(&spec.worktree, &["rev-parse", "HEAD^{tree}"])?;
    if state.parent != publication.expected_base
        || git_output(&spec.worktree, &["show", "-s", "--format=%s", "HEAD"])?
            != publication.commit_message
        || !changed_paths(&spec)?.is_empty()
    {
        return Err("commit candidato incompatível".to_string());
    }
    set_publication_status(&spec, &mut state, "COMMITTED")?;
    set_publication_status(&spec, &mut state, "PUSH_INTENT")?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args([
            "push",
            "--set-upstream",
            &publication.remote,
            &publication.head_branch,
        ])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "push falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if remote_head(&spec, &publication)?.as_deref() != Some(state.candidate.as_str()) {
        return Err("remote SHA diverge do candidate".to_string());
    }
    set_publication_status(&spec, &mut state, "PUSHED")?;
    set_publication_status(&spec, &mut state, "PR_INTENT")?;
    let numbers = list_pr_numbers(&spec, &publication)?;
    if numbers.is_empty() {
        let body = resolve_under(&spec.delegated_root, Path::new(&publication.pr_body))?;
        let output = run_gh(
            &spec,
            "pr-create",
            &[
                "pr".into(),
                "create".into(),
                "--repo".into(),
                publication.repository.clone(),
                "--base".into(),
                publication.base_branch.clone(),
                "--head".into(),
                publication.head_branch.clone(),
                "--title".into(),
                publication.pr_title.clone(),
                "--body-file".into(),
                body.display().to_string(),
            ],
        )?;
        let _ = output_text(&output, "gh pr create")?;
    }
    let numbers = list_pr_numbers(&spec, &publication)?;
    if numbers.len() != 1 {
        return Err("quantidade de PRs incompatível".to_string());
    }
    let pr = read_pr(&spec, &publication, numbers[0])?;
    require_compatible_pr(&pr, &publication, &state.candidate)?;
    state.pr_number = Some(pr.number);
    state.pr_url = Some(pr.url.clone());
    set_publication_status(&spec, &mut state, "PR_CREATED")?;
    state.body_digest = verify_remote_body(&spec, &publication, &pr)?;
    set_publication_status(&spec, &mut state, "BODY_VERIFIED")?;
    set_publication_status(&spec, &mut state, "CHECKS_PENDING")?;
    atomic_write(&spec.delegated_root.join("artefatos/publication.json"), format!(
        "{{\n  \"status\": \"CHECKS_PENDING\",\n  \"candidate\": {},\n  \"commit\": {},\n  \"parent\": {},\n  \"tree\": {},\n  \"pr_number\": {},\n  \"pr_url\": {}\n}}\n",
        json_escape(&state.candidate), json_escape(&state.candidate), json_escape(&state.parent),
        json_escape(&state.tree), pr.number, json_escape(&pr.url)
    ).as_bytes())?;
    Ok(EXIT_ACCEPTED)
}
// @pinker-nav:end development.agent.publication

enum ChecksResult {
    Success,
    Pending,
    Blocked(String),
}

// @pinker-nav:start development.agent.remote-checks
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Consulta somente checks exatos do SHA candidato, persiste eventos e snapshots, aceita apenas SUCCESS e distingue pendência, ausência, duplicidade e conclusões bloqueantes sem rerun ou bypass.

/// Categoria agregada de um check requerido após consolidar todas as ocorrências.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Success,
    Pending,
    Blocked,
}

/// Ocorrências e agregação de um único check requerido.
#[derive(Debug, Clone)]
pub struct RequiredCheckOccurrence {
    pub name: String,
    pub states: Vec<String>,
    pub aggregate: CheckState,
    pub blocking: Option<String>,
}

/// Resultado puro da classificação de todos os checks requeridos.
#[derive(Debug, Clone)]
pub struct CheckClassification {
    pub occurrences: Vec<RequiredCheckOccurrence>,
    pub aggregate: CheckState,
    pub missing: Vec<String>,
    pub extras: Vec<String>,
    pub blocking_reason: Option<String>,
}

/// Categoriza um único estado bruto de check em SUCCESS, pendente, bloqueante
/// ou desconhecido. Um mesmo SHA pode expor múltiplas linhas por check quando
/// `push` e `pull_request` disparam o mesmo job; a multiplicidade, sozinha,
/// nunca bloqueia.
fn categorize_check_state(state: &str) -> CheckState {
    match state.trim().to_ascii_uppercase().as_str() {
        "SUCCESS" => CheckState::Success,
        "PENDING" | "QUEUED" | "IN_PROGRESS" | "EXPECTED" => CheckState::Pending,
        "FAILURE" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "SKIPPED" | "NEUTRAL"
        | "STALE" | "STARTUP_FAILURE" => CheckState::Blocked,
        _ => CheckState::Blocked,
    }
}

/// Função pura: agrega todas as ocorrências observadas de cada check requerido
/// e aplica a precedência `BLOCKED > PENDING > SUCCESS`. Zero ocorrências conta
/// como pendente (ausente); qualquer estado desconhecido bloqueia. Registra
/// ocorrências, agregação, ausentes, extras e o primeiro motivo bloqueante.
pub fn classify_required_check_states(
    required: &[String],
    observed: &[(String, String)],
) -> CheckClassification {
    let mut occurrences = Vec::new();
    let mut missing = Vec::new();
    let mut aggregate = CheckState::Success;
    let mut blocking_reason: Option<String> = None;
    for name in required {
        let states = observed
            .iter()
            .filter(|(candidate, _)| candidate == name)
            .map(|(_, state)| state.clone())
            .collect::<Vec<_>>();
        if states.is_empty() {
            missing.push(name.clone());
        }
        let mut occurrence_state = if states.is_empty() {
            CheckState::Pending
        } else {
            CheckState::Success
        };
        let mut occurrence_blocking = None;
        for state in &states {
            match categorize_check_state(state) {
                CheckState::Success => {}
                CheckState::Pending => {
                    if occurrence_state != CheckState::Blocked {
                        occurrence_state = CheckState::Pending;
                    }
                }
                CheckState::Blocked => {
                    occurrence_state = CheckState::Blocked;
                    if occurrence_blocking.is_none() {
                        occurrence_blocking =
                            Some(format!("check {name} bloqueante: {}", state.trim()));
                    }
                }
            }
        }
        match occurrence_state {
            CheckState::Blocked => {
                aggregate = CheckState::Blocked;
                if blocking_reason.is_none() {
                    blocking_reason.clone_from(&occurrence_blocking);
                }
            }
            CheckState::Pending => {
                if aggregate != CheckState::Blocked {
                    aggregate = CheckState::Pending;
                }
            }
            CheckState::Success => {}
        }
        occurrences.push(RequiredCheckOccurrence {
            name: name.clone(),
            states,
            aggregate: occurrence_state,
            blocking: occurrence_blocking,
        });
    }
    let mut extras = Vec::new();
    for (name, _) in observed {
        if !required.iter().any(|req| req == name) && !extras.contains(name) {
            extras.push(name.clone());
        }
    }
    CheckClassification {
        occurrences,
        aggregate,
        missing,
        extras,
        blocking_reason,
    }
}

fn read_required_checks(
    spec: &Spec,
    publication: &PublicationSpec,
    pr: &RemotePr,
) -> Result<ChecksResult, String> {
    if pr.sha.is_empty() {
        return Err("candidate remoto vazio".to_string());
    }
    let output = run_gh(
        spec,
        "checks",
        &[
            "pr".into(),
            "checks".into(),
            pr.number.to_string(),
            "--repo".into(),
            publication.repository.clone(),
            "--json".into(),
            "name,state".into(),
            "--jq".into(),
            ".[] | [.name,.state] | @tsv".into(),
        ],
    )?;
    let text = String::from_utf8(output.stdout).map_err(|_| "checks não UTF-8".to_string())?;
    atomic_write(
        &spec.delegated_root.join("artefatos/checks.json"),
        format!(
            "{{\"candidate\":{},\"raw\":{}}}\n",
            json_escape(&pr.sha),
            json_escape(&text)
        )
        .as_bytes(),
    )?;
    let event_path = spec.delegated_root.join("estado/check-events.jsonl");
    let sequence = fs::read_to_string(&event_path)
        .unwrap_or_default()
        .lines()
        .count() as u64
        + 1;
    append_event(
        &event_path,
        sequence,
        "checks",
        "OBSERVED",
        output.status.code(),
    )?;
    let observed = text
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(name, state)| (name.to_string(), state.to_string()))
        .collect::<Vec<_>>();
    let classification = classify_required_check_states(&publication.required_checks, &observed);
    Ok(match classification.aggregate {
        CheckState::Success => ChecksResult::Success,
        CheckState::Pending => ChecksResult::Pending,
        CheckState::Blocked => ChecksResult::Blocked(
            classification
                .blocking_reason
                .unwrap_or_else(|| "check bloqueante".to_string()),
        ),
    })
}
// @pinker-nav:end development.agent.remote-checks

fn verify_commit_identity(
    spec: &Spec,
    publication: &PublicationSpec,
    state: &PublicationState,
) -> Result<(), String> {
    let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    let parent = git_output(&spec.worktree, &["rev-parse", "HEAD^"])?;
    let tree = git_output(&spec.worktree, &["rev-parse", "HEAD^{tree}"])?;
    let message = git_output(&spec.worktree, &["show", "-s", "--format=%s", "HEAD"])?;
    let mut changed = git_output(
        &spec.worktree,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    )?
    .lines()
    .map(str::to_string)
    .collect::<Vec<_>>();
    changed.sort();
    let mut expected = publication.changes.clone();
    expected.sort();
    if head != state.candidate
        || parent != state.parent
        || tree != state.tree
        || parent != publication.expected_base
        || message != publication.commit_message
        || changed != expected
    {
        return Err("identidade do commit candidato diverge".to_string());
    }
    Ok(())
}

// @pinker-nav:start development.agent.resume
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Retoma de forma idempotente reconciliando spec, commit, branch remota, PR, body e SHA dos checks; estados ACCEPTED e BLOCKED são somente leitura e timeout permanece retomável.
pub fn retomar(spec_path: &Path) -> Result<i32, String> {
    let spec_bytes = fs::read(spec_path).map_err(|err| err.to_string())?;
    let spec = load_spec(spec_path)?;
    let publication = publication(&spec)?.clone();
    let mut state = load_publication_state(&spec)?;
    if state.status == "ACCEPTED" {
        return Ok(EXIT_ACCEPTED);
    }
    if state.status == "BLOCKED" {
        return Ok(EXIT_BLOCKED);
    }
    if state.spec_hash != sha256_hex(&spec_bytes) {
        return Err("spec hash divergente".to_string());
    }
    if state.candidate.is_empty() {
        let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
        if head == publication.expected_base {
            if state.status != "COMMIT_INTENT" {
                return Err("commit ausente sem intenção reconciliável".to_string());
            }
            exact_changed(&spec, &publication)?;
            for path in &publication.changes {
                let output = Command::new("git")
                    .arg("-C")
                    .arg(&spec.worktree)
                    .args(["add", "--", path])
                    .output()
                    .map_err(|err| err.to_string())?;
                if !output.status.success() {
                    return Err("git add de retomada falhou".to_string());
                }
            }
            let output = Command::new("git")
                .arg("-C")
                .arg(&spec.worktree)
                .args(["commit", "-m", &publication.commit_message])
                .output()
                .map_err(|err| err.to_string())?;
            if !output.status.success() {
                return Err("commit de retomada falhou".to_string());
            }
        }
        state.candidate = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
        state.parent = git_output(&spec.worktree, &["rev-parse", "HEAD^"])?;
        state.tree = git_output(&spec.worktree, &["rev-parse", "HEAD^{tree}"])?;
        set_publication_status(&spec, &mut state, "COMMITTED")?;
    }
    verify_commit_identity(&spec, &publication, &state)?;
    match remote_head(&spec, &publication)? {
        Some(remote) if remote == state.candidate => {
            if matches!(state.status.as_str(), "COMMITTED" | "PUSH_INTENT") {
                set_publication_status(&spec, &mut state, "PUSHED")?;
            }
        }
        Some(_) => return Err("remote head divergente".to_string()),
        None => {
            set_publication_status(&spec, &mut state, "PUSH_INTENT")?;
            let output = Command::new("git")
                .arg("-C")
                .arg(&spec.worktree)
                .args([
                    "push",
                    "--set-upstream",
                    &publication.remote,
                    &publication.head_branch,
                ])
                .output()
                .map_err(|err| err.to_string())?;
            if !output.status.success() {
                return Err("push de retomada falhou".to_string());
            }
            if remote_head(&spec, &publication)?.as_deref() != Some(state.candidate.as_str()) {
                return Err("remote head divergente após push".to_string());
            }
            set_publication_status(&spec, &mut state, "PUSHED")?;
        }
    }
    let mut numbers = list_pr_numbers(&spec, &publication)?;
    if numbers.is_empty() {
        set_publication_status(&spec, &mut state, "PR_INTENT")?;
        let body = resolve_under(&spec.delegated_root, Path::new(&publication.pr_body))?;
        let output = run_gh(
            &spec,
            "pr-create",
            &[
                "pr".into(),
                "create".into(),
                "--repo".into(),
                publication.repository.clone(),
                "--base".into(),
                publication.base_branch.clone(),
                "--head".into(),
                publication.head_branch.clone(),
                "--title".into(),
                publication.pr_title.clone(),
                "--body-file".into(),
                body.display().to_string(),
            ],
        )?;
        let _ = output_text(&output, "gh pr create")?;
        numbers = list_pr_numbers(&spec, &publication)?;
    }
    if numbers.len() != 1 || state.pr_number.is_some_and(|number| number != numbers[0]) {
        return Err("identidade da PR diverge".to_string());
    }
    let pr = read_pr(&spec, &publication, numbers[0])?;
    require_compatible_pr(&pr, &publication, &state.candidate)?;
    state.pr_number = Some(pr.number);
    state.pr_url = Some(pr.url.clone());
    if matches!(state.status.as_str(), "PUSHED" | "PR_INTENT") {
        set_publication_status(&spec, &mut state, "PR_CREATED")?;
    }
    let digest = verify_remote_body(&spec, &publication, &pr)?;
    if !state.body_digest.is_empty() && digest != state.body_digest {
        return Err("body digest divergente".to_string());
    }
    state.body_digest = digest;
    if state.status == "PR_CREATED" {
        set_publication_status(&spec, &mut state, "BODY_VERIFIED")?;
    }
    let started = Instant::now();
    loop {
        let current = read_pr(&spec, &publication, pr.number)?;
        require_compatible_pr(&current, &publication, &state.candidate)?;
        match read_required_checks(&spec, &publication, &current)? {
            ChecksResult::Success => {
                set_publication_status(&spec, &mut state, "ACCEPTED")?;
                atomic_write(&spec.delegated_root.join("artefatos/publication.json"), format!(
                    "{{\"status\":\"ACCEPTED\",\"candidate\":{},\"pr_number\":{},\"all_required\":\"SUCCESS\"}}\n",
                    json_escape(&state.candidate), current.number
                ).as_bytes())?;
                return Ok(EXIT_ACCEPTED);
            }
            ChecksResult::Blocked(reason) => {
                set_publication_status(&spec, &mut state, "BLOCKED")?;
                return Err(reason);
            }
            ChecksResult::Pending => {
                set_publication_status(&spec, &mut state, "CHECKS_PENDING")?;
            }
        }
        if started.elapsed().as_secs() >= publication.timeout_seconds {
            set_publication_status(&spec, &mut state, "NEEDS_HUMAN_DECISION")?;
            return Ok(EXIT_NEEDS_HUMAN);
        }
        std::thread::sleep(std::time::Duration::from_secs(publication.poll_seconds));
    }
}
// @pinker-nav:end development.agent.resume

// @pinker-nav:start development.agent.sensitivity
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Executa mutações reversíveis confinadas: snapshot e SHA-256, match exato, substituição atômica, probe sem shell, captura de saída/duração/exit, restauração obrigatória dos bytes e detecção de efeitos laterais, parando diante de falha de restauração.
pub fn sensibilidade(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let event_path = spec.delegated_root.join("estado/mutation-events.jsonl");
    atomic_write(&event_path, b"")?;
    let before_paths = changed_paths(&spec)?;
    let mut results = Vec::new();
    let mut blocked = false;
    let mut restoration_verified = true;
    for (index, mutation) in spec.mutations.iter().enumerate() {
        if blocked && !restoration_verified {
            break;
        }
        let target = resolve_under(&spec.worktree, Path::new(&mutation.target))?;
        let search_path = resolve_under(&spec.delegated_root, Path::new(&mutation.search_file))?;
        let replacement_path =
            resolve_under(&spec.delegated_root, Path::new(&mutation.replacement_file))?;
        let original = fs::read(&target).map_err(|err| err.to_string())?;
        let original_hash = sha256_hex(&original);
        let search = fs::read(&search_path).map_err(|err| err.to_string())?;
        let replacement = fs::read(&replacement_path).map_err(|err| err.to_string())?;
        let matches = if search.is_empty() {
            0
        } else {
            original
                .windows(search.len())
                .filter(|window| *window == search.as_slice())
                .count()
        };
        let mut status = "HARNESS_ERROR";
        let mut exit = None;
        let mut duration = 0;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        if !search.is_empty() && matches == mutation.expected_matches {
            let changed = replace_bytes(&original, &search, &replacement);
            atomic_write(&target, &changed)?;
            let cwd = resolve_under(&spec.worktree, Path::new(&mutation.probe_cwd))?;
            let executable = if mutation.probe_program == "pink" {
                resolve_pinker_executable(&env::current_exe().map_err(|err| err.to_string())?)?
            } else {
                PathBuf::from(&mutation.probe_program)
            };
            let started = Instant::now();
            match Command::new(executable)
                .args(&mutation.probe_argv)
                .current_dir(cwd)
                .output()
            {
                Ok(output) => {
                    duration = started.elapsed().as_millis();
                    exit = output.status.code();
                    stdout = output.stdout;
                    stderr = output.stderr;
                    let stderr_ok = mutation
                        .probe_stderr_contains
                        .as_ref()
                        .map_or(true, |needle| {
                            String::from_utf8_lossy(&stderr).contains(needle)
                        });
                    status = if exit == Some(mutation.probe_expected_exit) && stderr_ok {
                        "DETECTED"
                    } else {
                        "UNDETECTED"
                    };
                }
                Err(err) => stderr = err.to_string().into_bytes(),
            }
            if atomic_write(&target, &original).is_err()
                || fs::read(&target)
                    .map(|bytes| sha256_hex(&bytes))
                    .ok()
                    .as_deref()
                    != Some(original_hash.as_str())
            {
                restoration_verified = false;
                status = "HARNESS_ERROR";
            }
        }
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/mutation-{}.stdout.txt", mutation.id)),
            &stdout,
        )?;
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/mutation-{}.stderr.txt", mutation.id)),
            &stderr,
        )?;
        let after_paths = changed_paths(&spec)?;
        if after_paths != before_paths {
            status = "HARNESS_ERROR";
        }
        blocked |= status != "DETECTED";
        append_event(&event_path, (index + 1) as u64, &mutation.id, status, exit)?;
        results.push(format!("{{\"id\":{},\"status\":{},\"matches\":{matches},\"exit_code\":{},\"duration_ms\":{duration}}}", json_escape(&mutation.id), json_escape(status), exit.map_or("null".to_string(), |value| value.to_string())));
    }
    let body = results.join(",\n    ");
    let detected = results
        .iter()
        .filter(|item| item.contains("\"status\":\"DETECTED\""))
        .count();
    let undetected = results
        .iter()
        .filter(|item| item.contains("\"status\":\"UNDETECTED\""))
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let harness_errors = results
        .iter()
        .filter(|item| item.contains("\"status\":\"HARNESS_ERROR\""))
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let report = format!("{{\n  \"schema\":1,\n  \"passed\":{},\n  \"detected\":{detected},\n  \"undetected\":[{undetected}],\n  \"harness_errors\":[{harness_errors}],\n  \"restoration_verified\":{restoration_verified},\n  \"mutations\":[\n    {body}\n  ]\n}}\n", !blocked);
    atomic_write(
        &spec.delegated_root.join("artefatos/sensitivity.json"),
        report.as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/restoration.json"),
        format!("{{\"restoration_verified\":{restoration_verified}}}\n").as_bytes(),
    )?;
    let manifest = artifact_manifest(&spec.delegated_root)?;
    atomic_write(
        &spec.delegated_root.join("artefatos/artifact-manifest.json"),
        manifest.as_bytes(),
    )?;
    Ok(if blocked { EXIT_BLOCKED } else { EXIT_ACCEPTED })
}

fn replace_bytes(input: &[u8], search: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        if input[pos..].starts_with(search) {
            out.extend_from_slice(replacement);
            pos += search.len();
        } else {
            out.push(input[pos]);
            pos += 1;
        }
    }
    out
}
// @pinker-nav:end development.agent.sensitivity

// @pinker-nav:start development.agent.lifecycle
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Ciclo iniciar/executar/verificar/status/relatorio: snapshots Git, execução fail-fast com NOT_RUN, validação de escopo exato, estados ACCEPTED/BLOCKED, códigos de saída mecânicos e emissão dos artefatos terminais canônicos.
fn git_output(worktree: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(args)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn workspace_json(spec: &Spec) -> Result<String, String> {
    let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    let branch = git_output(&spec.worktree, &["branch", "--show-current"])?;
    let status = git_output(&spec.worktree, &["status", "--porcelain"])?;
    Ok(format!(
        "{{\n  \"head\": {},\n  \"branch\": {},\n  \"clean\": {},\n  \"status\": {}\n}}\n",
        json_escape(&head),
        json_escape(&branch),
        status.is_empty(),
        json_escape(&status)
    ))
}

fn ensure_layout(spec: &Spec) -> Result<(), String> {
    for relative in ["artefatos", "estado", "logs"] {
        fs::create_dir_all(spec.delegated_root.join(relative)).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn iniciar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let before = workspace_json(&spec)?;
    atomic_write(&spec.delegated_root.join("artefatos/environment.json"), format!("{{\n  \"schema\": 1,\n  \"task_id\": {},\n  \"repo_root\": {},\n  \"worktree\": {},\n  \"delegated_root\": {}\n}}\n", json_escape(&spec.task_id), json_escape(&spec.repo_root.display().to_string()), json_escape(&spec.worktree.display().to_string()), json_escape(&spec.delegated_root.display().to_string())).as_bytes())?;
    atomic_write(
        &spec.delegated_root.join("artefatos/workspace-before.json"),
        before.as_bytes(),
    )?;
    atomic_write(&spec.delegated_root.join("estado/run-state.json"), format!("{{\n  \"schema\": 1,\n  \"status\": \"READY\",\n  \"task_id\": {},\n  \"last_sequence\": 0\n}}\n", json_escape(&spec.task_id)).as_bytes())?;
    Ok(EXIT_ACCEPTED)
}

fn changed_paths(spec: &Spec) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = stdout
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn scope_ok(spec: &Spec, changed: &[String]) -> bool {
    let allowed: BTreeSet<&str> = spec.allowed_changes.iter().map(String::as_str).collect();
    changed.iter().all(|path| allowed.contains(path.as_str()))
}

fn results_json(results: &[CommandResult]) -> String {
    results.iter().map(|result| format!("    {{\"id\":{},\"status\":{},\"exit_code\":{},\"expected_exit\":{},\"duration_ms\":{},\"shell\":{}}}", json_escape(&result.id), json_escape(result.status), result.exit_code.map_or("null".to_string(), |value| value.to_string()), result.expected_exit, result.duration_ms, result.shell)).collect::<Vec<_>>().join(",\n")
}

pub fn executar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let _ = iniciar(spec_path)?;
    let event_path = spec.delegated_root.join("estado/command-events.jsonl");
    atomic_write(&event_path, b"")?;
    let mut results = Vec::new();
    let mut blocked = false;
    for (index, command) in spec.commands.iter().enumerate() {
        let sequence = (index + 1) as u64;
        if blocked {
            append_event(&event_path, sequence, &command.id, "NOT_RUN", None)?;
            results.push(CommandResult {
                id: command.id.clone(),
                status: "NOT_RUN",
                exit_code: None,
                expected_exit: command.expected_exit,
                duration_ms: 0,
                shell: command.shell,
            });
            continue;
        }
        let (result, output) = execute_one(&spec, command)?;
        std::io::stdout()
            .write_all(&output.stdout)
            .map_err(|err| err.to_string())?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(|err| err.to_string())?;
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/{}.stdout.txt", command.id)),
            &output.stdout,
        )?;
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/{}.stderr.txt", command.id)),
            &output.stderr,
        )?;
        append_event(
            &event_path,
            sequence,
            &command.id,
            result.status,
            result.exit_code,
        )?;
        blocked = result.status == "FAILED";
        results.push(result);
    }
    let changed = changed_paths(&spec)?;
    let scope = scope_ok(&spec, &changed);
    blocked |= !scope;
    let status = if blocked { "BLOCKED" } else { "ACCEPTED" };
    let verdict = if blocked {
        &spec.blocked_verdict
    } else {
        &spec.accepted_verdict
    };
    let changed_json = changed
        .iter()
        .map(|path| json_escape(path))
        .collect::<Vec<_>>()
        .join(",");
    atomic_write(
        &spec.delegated_root.join("artefatos/workspace-after.json"),
        workspace_json(&spec)?.as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/scope.json"),
        format!("{{\n  \"valid\": {scope},\n  \"changed_files\": [{changed_json}]\n}}\n")
            .as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/validation.json"),
        format!(
            "{{\n  \"status\": {},\n  \"commands\": [\n{}\n  ]\n}}\n",
            json_escape(status),
            results_json(&results)
        )
        .as_bytes(),
    )?;
    let result = format!("{{\n  \"status\": {},\n  \"verdict\": {},\n  \"task_id\": {},\n  \"scope_valid\": {scope},\n  \"commands\": [\n{}\n  ]\n}}\n", json_escape(status), json_escape(verdict), json_escape(&spec.task_id), results_json(&results));
    atomic_write(
        &spec.delegated_root.join("artefatos/resultado.json"),
        result.as_bytes(),
    )?;
    let report = format!("# Relatório pink agente\n\n- task: `{}`\n- status: **{}**\n- verdict: `{}`\n- comandos: {}\n- escopo válido: {}\n", spec.task_id, status, verdict, results.len(), scope);
    atomic_write(
        &spec.delegated_root.join("artefatos/RELATORIO.md"),
        report.as_bytes(),
    )?;
    atomic_write(&spec.delegated_root.join("estado/run-state.json"), format!("{{\n  \"schema\": 1,\n  \"status\": {},\n  \"verdict\": {},\n  \"last_sequence\": {}\n}}\n", json_escape(status), json_escape(verdict), results.len()).as_bytes())?;
    let manifest = artifact_manifest(&spec.delegated_root)?;
    atomic_write(
        &spec.delegated_root.join("artefatos/artifact-manifest.json"),
        manifest.as_bytes(),
    )?;
    Ok(if blocked { EXIT_BLOCKED } else { EXIT_ACCEPTED })
}

pub fn verificar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let changed = changed_paths(&spec)?;
    if !scope_ok(&spec, &changed) {
        return Ok(EXIT_BLOCKED);
    }
    let expected_manifest = artifact_manifest(&spec.delegated_root)?;
    let actual_manifest =
        fs::read_to_string(spec.delegated_root.join("artefatos/artifact-manifest.json"))
            .map_err(|err| err.to_string())?;
    if expected_manifest != actual_manifest {
        return Ok(EXIT_BLOCKED);
    }
    if spec.checks.is_empty() {
        return Ok(EXIT_ACCEPTED);
    }
    let (passed, results) = run_checks(&spec)?;
    let select = |kind: &str| {
        results
            .iter()
            .filter(|result| result.contains(&format!("\"kind\":\"{kind}\"")))
            .cloned()
            .collect::<Vec<_>>()
            .join(",\n  ")
    };
    atomic_write(
        &spec.delegated_root.join("artefatos/git-checks.json"),
        format!("[\n  {}\n]\n", select("git")).as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/marker-only.json"),
        format!("[\n  {}\n]\n", select("marker-only")).as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/projections.json"),
        format!("[\n  {}\n]\n", select("projection")).as_bytes(),
    )?;
    let manifest = artifact_manifest(&spec.delegated_root)?;
    atomic_write(
        &spec.delegated_root.join("artefatos/artifact-manifest.json"),
        manifest.as_bytes(),
    )?;
    Ok(if passed { EXIT_ACCEPTED } else { EXIT_BLOCKED })
}

pub fn status(spec_path: &Path, json: bool) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let result = fs::read_to_string(spec.delegated_root.join("artefatos/resultado.json"))
        .map_err(|err| err.to_string())?;
    if json {
        print!("{result}");
    } else if result.contains("\"status\": \"ACCEPTED\"") {
        println!("ACCEPTED");
    } else if result.contains("NEEDS_HUMAN_DECISION") {
        println!("NEEDS_HUMAN_DECISION");
    } else {
        println!("BLOCKED");
    }
    Ok(if result.contains("\"status\": \"ACCEPTED\"") {
        EXIT_ACCEPTED
    } else if result.contains("NEEDS_HUMAN_DECISION") {
        EXIT_NEEDS_HUMAN
    } else {
        EXIT_BLOCKED
    })
}

pub fn relatorio(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let report = fs::read_to_string(spec.delegated_root.join("artefatos/RELATORIO.md"))
        .map_err(|err| err.to_string())?;
    print!("{report}");
    Ok(if report.contains("**ACCEPTED**") {
        EXIT_ACCEPTED
    } else {
        EXIT_BLOCKED
    })
}

pub fn timestamp_metadata() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
// @pinker-nav:end development.agent.lifecycle

#[cfg(test)]
mod executable_resolution_tests {
    use super::resolve_pinker_executable;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pink-agent-resolver-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("diretório temporário do resolver");
        root
    }

    #[test]
    fn resolve_existing_current_executable() {
        let root = test_root("existing");
        let current = root.join("pink");
        fs::write(&current, b"replacement").expect("executável corrente sintético");

        assert_eq!(resolve_pinker_executable(&current).unwrap(), current);
        fs::remove_dir_all(root).expect("limpeza do resolver");
    }

    #[test]
    fn resolve_deleted_current_executable_to_replacement() {
        let root = test_root("deleted");
        let replacement = root.join("pink");
        fs::write(&replacement, b"replacement").expect("substituto sintético");
        let deleted = PathBuf::from(format!("{} (deleted)", replacement.display()));

        assert_eq!(resolve_pinker_executable(&deleted).unwrap(), replacement);
        fs::remove_dir_all(root).expect("limpeza do resolver");
    }

    #[test]
    fn reject_deleted_current_executable_without_replacement() {
        let root = test_root("missing");
        let replacement = root.join("pink");
        let deleted = PathBuf::from(format!("{} (deleted)", replacement.display()));

        let error = resolve_pinker_executable(&deleted).unwrap_err();
        assert!(error.contains("substituto ausente"), "{error}");
        fs::remove_dir_all(root).expect("limpeza do resolver");
    }
}
