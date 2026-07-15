//! Trama Pinker — Etapa 4 (Manifestos estruturados de mudança).
//!
//! Lê o bloco ` ```pinker-change ` do corpo de um PR, constrói um manifesto
//! versionado `.pinker/changes/pr-N.yaml` e gera um histórico mecânico derivado
//! (`.pinker/changes/index.jsonl`). Especificação, seções 14, 15, 17 e 21.
//!
//! O manifesto é a fonte estrutural; o corpo do PR é a origem humana. Nenhum
//! conteúdo narrativo é inventado — apenas os campos declarados são propagados.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const FENCE_OPEN: &str = "```pinker-change";
const FENCE_CLOSE: &str = "```";

/// Valores aceitos para `kind` (§11, enum do schema).
const KIND_ENUM: &[&str] = &["phase", "hotfix", "documentation", "parallel-phase"];
/// Valores aceitos para `status` (§11, enum do schema).
const STATUS_ENUM: &[&str] = &["completed", "in-progress", "planned"];

/// Mensagem canônica de violação de imutabilidade de manifesto (§10).
pub fn immutable_error(pr: u64) -> String {
    format!(
        "E-CHANGE-IMMUTABLE\n\
         O manifesto .pinker/changes/pr-{pr}.yaml já existe com conteúdo diferente.\n\
         Manifestos são imutáveis após a primeira importação; nenhuma alteração\n\
         silenciosa é permitida. Reverta a mudança ou registre um novo PR."
    )
}

/// Origem de um manifesto (presente no arquivo versionado).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Source {
    pub kind: String,
    pub number: u64,
    pub repository: Option<String>,
}

/// Manifesto estruturado de mudança.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    pub schema: u64,
    pub source: Option<Source>,
    pub kind: String,
    pub phase: Option<u64>,
    pub block: Option<u64>,
    pub title: String,
    pub area: Vec<String>,
    pub status: String,
    /// Famílias de documento derivado atualizadas (state, history, ...).
    pub updates: Vec<(String, bool)>,
    pub implemented: Vec<String>,
    pub pending_remove: Vec<String>,
    pub validation_required: Vec<String>,
    /// Campos de topo desconhecidos encontrados no bloco (§11 — rejeição).
    pub unknown_fields: Vec<String>,
    /// `source.type` declarado (para validar `github-pr`).
    pub source_type_present: Option<String>,
}

/// Falhas de manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    NoBlock,
    UnterminatedBlock,
    UnsupportedSchema {
        found: u64,
    },
    MissingField {
        field: String,
    },
    NumberMismatch {
        file: u64,
        source: u64,
    },
    UnknownField {
        field: String,
    },
    InvalidEnum {
        field: String,
        value: String,
        allowed: String,
    },
    InvalidSourceType {
        found: String,
    },
    InvalidNumber {
        found: i64,
    },
    InvalidIdFormat {
        field: String,
        value: String,
    },
    Io {
        path: String,
        msg: String,
    },
}

impl fmt::Display for ChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeError::NoBlock => write!(
                f,
                "E-CHANGE-BLOCK\nBloco ```pinker-change``` ausente no corpo do PR."
            ),
            ChangeError::UnterminatedBlock => write!(
                f,
                "E-CHANGE-BLOCK\nBloco ```pinker-change``` sem cerca de fechamento."
            ),
            ChangeError::UnsupportedSchema { found } => write!(
                f,
                "E-CHANGE-SCHEMA\nEsquema {} não suportado; esperado schema: 1.",
                found
            ),
            ChangeError::MissingField { field } => write!(
                f,
                "E-CHANGE-FIELD\nCampo obrigatório ausente no manifesto: '{}'.",
                field
            ),
            ChangeError::NumberMismatch { file, source } => write!(
                f,
                "E-CHANGE-NUMBER\nManifesto pr-{}.yaml declara source.number {}.",
                file, source
            ),
            ChangeError::UnknownField { field } => write!(
                f,
                "E-CHANGE-SCHEMA\nCampo desconhecido no manifesto: '{}'.",
                field
            ),
            ChangeError::InvalidEnum {
                field,
                value,
                allowed,
            } => write!(
                f,
                "E-CHANGE-SCHEMA\nValor inválido em '{}': '{}'. Aceitos: {}.",
                field, value, allowed
            ),
            ChangeError::InvalidSourceType { found } => write!(
                f,
                "E-CHANGE-SCHEMA\nsource.type inválido: '{}'; esperado 'github-pr'.",
                found
            ),
            ChangeError::InvalidNumber { found } => write!(
                f,
                "E-CHANGE-SCHEMA\nsource.number inválido: {}; esperado inteiro positivo.",
                found
            ),
            ChangeError::InvalidIdFormat { field, value } => write!(
                f,
                "E-CHANGE-SCHEMA\nId inválido em '{}': '{}' (formato [a-z0-9]+([._-][a-z0-9]+)*).",
                field, value
            ),
            ChangeError::Io { path, msg } => {
                write!(f, "E-CHANGE-IO\nFalha em '{}': {}", path, msg)
            }
        }
    }
}

// @pinker-nav:start trama.mudancas.manifesto
// @pinker-nav:domain mudancas
// @pinker-nav:layer trama
// @pinker-nav:summary Manifesto estruturado de mudança: extrai o bloco `pinker-change` do corpo do PR, aplica de fato o schema (enums de kind/status, rejeição de campos desconhecidos, source.type/número, formato de ids) e renderiza o YAML versionado determinístico.
impl Change {
    /// Extrai e interpreta o bloco `pinker-change` de um corpo de PR.
    pub fn parse_pr_body(body: &str) -> Result<Change, ChangeError> {
        let block = extract_block(body)?;
        Self::parse_block(&block)
    }

    /// Interpreta o conteúdo textual do bloco (sem as cercas).
    pub fn parse_block(block: &str) -> Result<Change, ChangeError> {
        let mut change = Change::default();
        let mut top: Option<String> = None;
        let mut sub: Option<String> = None;
        let mut source = Source::default();
        let mut has_source = false;

        for raw in block.lines() {
            let indent = raw.len() - raw.trim_start().len();
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(item) = trimmed.strip_prefix("- ") {
                let value = unquote(&strip_inline_comment(item.trim()));
                match (top.as_deref(), sub.as_deref()) {
                    (Some("area"), _) => change.area.push(value),
                    (Some("sections"), Some("implemented")) => change.implemented.push(value),
                    (Some("sections"), Some("pending_remove")) => change.pending_remove.push(value),
                    (Some("validation"), Some("required")) => {
                        change.validation_required.push(value)
                    }
                    _ => {}
                }
                continue;
            }

            let Some(colon) = trimmed.find(':') else {
                continue;
            };
            let key = trimmed[..colon].trim().to_string();
            let rest = strip_inline_comment(trimmed[colon + 1..].trim());

            if indent == 0 {
                top = None;
                sub = None;
                match key.as_str() {
                    "schema" => change.schema = rest.parse().unwrap_or(0),
                    "kind" => change.kind = unquote(&rest),
                    "phase" => change.phase = rest.parse().ok(),
                    "block" => change.block = rest.parse().ok(),
                    "title" => change.title = unquote(&rest),
                    "status" => change.status = unquote(&rest),
                    "area" | "sections" | "validation" | "source" | "updates" => {
                        if key == "source" {
                            has_source = true;
                        }
                        top = Some(key);
                    }
                    other => {
                        // Campo de topo desconhecido: registrado para rejeição (§11).
                        if !change.unknown_fields.iter().any(|f| f == other) {
                            change.unknown_fields.push(other.to_string());
                        }
                    }
                }
            } else if let Some(current) = top.as_deref() {
                match current {
                    "updates" => {
                        change.updates.push((key, rest == "true"));
                    }
                    "source" => {
                        has_source = true;
                        match key.as_str() {
                            "type" => source.kind = unquote(&rest),
                            "number" => source.number = rest.parse().unwrap_or(0),
                            "repository" => source.repository = Some(unquote(&rest)),
                            _ => {}
                        }
                    }
                    "sections" | "validation" => {
                        // subchave de bloco (implemented/pending_remove/required)
                        sub = Some(key);
                    }
                    _ => {}
                }
            }
        }

        if has_source {
            change.source_type_present = Some(source.kind.clone());
            change.source = Some(source);
        }

        Ok(change)
    }

    /// Valida de fato as restrições do schema (§11): versão, campos
    /// obrigatórios, rejeição de campos desconhecidos, enums de `kind`/`status`,
    /// `source.type`, número de PR positivo, tipos e formato de ids.
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.schema != 1 {
            return Err(ChangeError::UnsupportedSchema { found: self.schema });
        }
        if let Some(field) = self.unknown_fields.first() {
            return Err(ChangeError::UnknownField {
                field: field.clone(),
            });
        }
        if self.kind.is_empty() {
            return Err(ChangeError::MissingField {
                field: "kind".to_string(),
            });
        }
        if !KIND_ENUM.contains(&self.kind.as_str()) {
            return Err(ChangeError::InvalidEnum {
                field: "kind".to_string(),
                value: self.kind.clone(),
                allowed: KIND_ENUM.join(", "),
            });
        }
        if self.title.is_empty() {
            return Err(ChangeError::MissingField {
                field: "title".to_string(),
            });
        }
        if self.status.is_empty() {
            return Err(ChangeError::MissingField {
                field: "status".to_string(),
            });
        }
        if !STATUS_ENUM.contains(&self.status.as_str()) {
            return Err(ChangeError::InvalidEnum {
                field: "status".to_string(),
                value: self.status.clone(),
                allowed: STATUS_ENUM.join(", "),
            });
        }
        if let Some(source) = &self.source {
            if source.kind != "github-pr" {
                return Err(ChangeError::InvalidSourceType {
                    found: source.kind.clone(),
                });
            }
            if source.number == 0 {
                return Err(ChangeError::InvalidNumber { found: 0 });
            }
        } else if let Some(source_type) = &self.source_type_present {
            // `source:` presente no bloco mas sem `type` válido.
            if source_type != "github-pr" {
                return Err(ChangeError::InvalidSourceType {
                    found: source_type.clone(),
                });
            }
        }
        // Ids semânticos (área e seções) devem seguir o formato permitido.
        for id in self
            .area
            .iter()
            .chain(&self.implemented)
            .chain(&self.pending_remove)
        {
            if !valid_id(id) {
                return Err(ChangeError::InvalidIdFormat {
                    field: "id".to_string(),
                    value: id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Serializa o manifesto versionado (`.pinker/changes/pr-N.yaml`) de forma
    /// determinística — idempotente para o mesmo bloco de entrada.
    pub fn render_yaml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("schema: {}\n", self.schema));
        if let Some(source) = &self.source {
            out.push_str("source:\n");
            out.push_str(&format!("  type: {}\n", source.kind));
            out.push_str(&format!("  number: {}\n", source.number));
            if let Some(repo) = &source.repository {
                out.push_str(&format!("  repository: {}\n", repo));
            }
        }
        out.push_str(&format!("kind: {}\n", self.kind));
        if let Some(phase) = self.phase {
            out.push_str(&format!("phase: {}\n", phase));
        }
        if let Some(block) = self.block {
            out.push_str(&format!("block: {}\n", block));
        }
        out.push_str(&format!("title: {}\n", self.title));
        if !self.area.is_empty() {
            out.push_str("area:\n");
            for item in &self.area {
                out.push_str(&format!("  - {}\n", item));
            }
        }
        out.push_str(&format!("status: {}\n", self.status));
        if !self.updates.is_empty() {
            out.push_str("updates:\n");
            for (key, value) in &self.updates {
                out.push_str(&format!("  {}: {}\n", key, value));
            }
        }
        if !self.implemented.is_empty() || !self.pending_remove.is_empty() {
            out.push_str("sections:\n");
            if !self.implemented.is_empty() {
                out.push_str("  implemented:\n");
                for item in &self.implemented {
                    out.push_str(&format!("    - {}\n", item));
                }
            }
            if !self.pending_remove.is_empty() {
                out.push_str("  pending_remove:\n");
                for item in &self.pending_remove {
                    out.push_str(&format!("    - {}\n", item));
                }
            }
        }
        if !self.validation_required.is_empty() {
            out.push_str("validation:\n");
            out.push_str("  required:\n");
            for item in &self.validation_required {
                out.push_str(&format!("    - {}\n", item));
            }
        }
        out
    }

    /// Linha do histórico mecânico derivado (`.pinker/changes/index.jsonl`).
    pub fn ledger_json(&self) -> String {
        let pr = self.source.as_ref().map(|s| s.number).unwrap_or(0);
        let mut out = String::new();
        out.push_str("{\"schema\":1");
        out.push_str(&format!(",\"pr\":{}", pr));
        out.push_str(&format!(",\"kind\":{}", json_string(&self.kind)));
        if let Some(phase) = self.phase {
            out.push_str(&format!(",\"phase\":{}", phase));
        }
        if let Some(block) = self.block {
            out.push_str(&format!(",\"block\":{}", block));
        }
        out.push_str(&format!(",\"title\":{}", json_string(&self.title)));
        out.push_str(&format!(",\"status\":{}", json_string(&self.status)));
        if !self.area.is_empty() {
            let parts: Vec<String> = self.area.iter().map(|a| json_string(a)).collect();
            out.push_str(&format!(",\"area\":[{}]", parts.join(",")));
        }
        out.push('}');
        out
    }
}
// @pinker-nav:end trama.mudancas.manifesto

/// Manifestos carregados de `.pinker/changes/`.
#[derive(Debug, Clone, Default)]
pub struct Manifests {
    pub changes: Vec<Change>,
    pub problems: Vec<ChangeError>,
}

// @pinker-nav:start trama.mudancas.ledger
// @pinker-nav:domain mudancas
// @pinker-nav:layer trama
// @pinker-nav:summary Carrega e valida todos os manifestos `pr-N.yaml` de `.pinker/changes/` e renderiza o ledger mecânico (`index.jsonl`) ordenado por PR, fonte da visão humana e das projeções.
impl Manifests {
    /// Carrega e valida todos os `.pinker/changes/pr-*.yaml`.
    pub fn load(changes_dir: &Path) -> Manifests {
        let mut manifests = Manifests::default();
        let entries = match fs::read_dir(changes_dir) {
            Ok(entries) => entries,
            Err(_) => return manifests, // diretório ausente = zero manifestos
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_pr_yaml(p))
            .collect();
        files.sort();

        for file in files {
            let expected = pr_number_from_filename(&file);
            let text = match fs::read_to_string(&file) {
                Ok(text) => text,
                Err(err) => {
                    manifests.problems.push(ChangeError::Io {
                        path: file.display().to_string(),
                        msg: err.to_string(),
                    });
                    continue;
                }
            };
            match Change::parse_block(&text) {
                Ok(change) => {
                    if let Err(err) = change.validate() {
                        manifests.problems.push(err);
                    }
                    if let (Some(expected), Some(source)) = (expected, &change.source) {
                        if source.number != expected {
                            manifests.problems.push(ChangeError::NumberMismatch {
                                file: expected,
                                source: source.number,
                            });
                        }
                    }
                    manifests.changes.push(change);
                }
                Err(err) => manifests.problems.push(err),
            }
        }

        manifests
            .changes
            .sort_by_key(|c| c.source.as_ref().map(|s| s.number).unwrap_or(0));
        manifests
    }

    /// Histórico mecânico derivado, ordenado por número de PR.
    pub fn render_ledger(&self) -> String {
        let mut sorted = self.changes.clone();
        sorted.sort_by_key(|c| c.source.as_ref().map(|s| s.number).unwrap_or(0));
        let mut out = String::new();
        for change in &sorted {
            out.push_str(&change.ledger_json());
            out.push('\n');
        }
        out
    }
}
// @pinker-nav:end trama.mudancas.ledger

fn extract_block(body: &str) -> Result<String, ChangeError> {
    let lines: Vec<&str> = body.lines().collect();
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == FENCE_OPEN {
            start = Some(i + 1);
            break;
        }
    }
    let Some(start) = start else {
        return Err(ChangeError::NoBlock);
    };
    for (i, line) in lines.iter().enumerate().skip(start) {
        if line.trim() == FENCE_CLOSE {
            return Ok(lines[start..i].join("\n"));
        }
    }
    Err(ChangeError::UnterminatedBlock)
}

/// Formato permitido de id semântico: `[a-z0-9]+([._-][a-z0-9]+)*`.
fn valid_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let bytes = id.as_bytes();
    let is_alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let is_sep = |c: u8| c == b'.' || c == b'_' || c == b'-';
    if !is_alnum(bytes[0]) || !is_alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    let mut prev_sep = false;
    for &c in bytes {
        if is_alnum(c) {
            prev_sep = false;
        } else if is_sep(c) {
            if prev_sep {
                return false;
            }
            prev_sep = true;
        } else {
            return false;
        }
    }
    true
}

fn is_pr_yaml(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("pr-") && n.ends_with(".yaml"))
        .unwrap_or(false)
}

fn pr_number_from_filename(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_prefix("pr-")?.strip_suffix(".yaml")?;
    stem.parse().ok()
}

/// Remove comentário inline no estilo YAML (` # ...`) fora de aspas.
///
/// Um `#` inicia comentário apenas no início do valor ou precedido por espaço,
/// e nunca dentro de aspas simples/duplas. Torna o parser tolerante a
/// comentários deixados no template do PR (ex.: `kind: phase  # phase | ...`).
fn strip_inline_comment(value: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_ws = true; // início do valor conta como precedido por espaço
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '#' && !in_single && !in_double && prev_ws {
            break;
        }
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
        prev_ws = ch.is_whitespace();
        out.push(ch);
    }
    out.trim_end().to_string()
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "## Resumo\nImplementa Resultado.\n\n```pinker-change\nschema: 1\nkind: phase\nphase: 241\nblock: 20\ntitle: Biblioteca predeclarada de Resultado<T,E>\narea:\n  - language.result\nstatus: completed\nupdates:\n  state: true\n  history: true\n  roadmap: false\nsections:\n  implemented:\n    - result.predeclared\n  pending_remove:\n    - result.standard-library\nvalidation:\n  required:\n    - make ci\n```\n\nfim\n";

    #[test]
    fn parses_pinker_change_block() {
        let change = Change::parse_pr_body(BODY).unwrap();
        assert_eq!(change.schema, 1);
        assert_eq!(change.kind, "phase");
        assert_eq!(change.phase, Some(241));
        assert_eq!(change.block, Some(20));
        assert_eq!(change.title, "Biblioteca predeclarada de Resultado<T,E>");
        assert_eq!(change.area, vec!["language.result"]);
        assert_eq!(change.status, "completed");
        assert_eq!(change.implemented, vec!["result.predeclared"]);
        assert_eq!(change.pending_remove, vec!["result.standard-library"]);
        assert_eq!(change.validation_required, vec!["make ci"]);
        assert!(change.updates.contains(&("state".to_string(), true)));
        assert!(change.updates.contains(&("roadmap".to_string(), false)));
    }

    #[test]
    fn strips_inline_template_comments() {
        // Bloco com comentários inline deixados no template do PR.
        let body = "```pinker-change\nschema: 1\nkind: phase  # phase | hotfix | documentation | parallel-phase\ntitle: build#42  # nota inline\nstatus: completed # ok\narea:\n  - language.result  # comentário no item\n```\n";
        let change = Change::parse_pr_body(body).unwrap();
        assert_eq!(change.kind, "phase");
        assert_eq!(change.status, "completed");
        // `#` colado (sem espaço antes) é mantido; o ` # nota inline` é removido.
        assert_eq!(change.title, "build#42");
        assert_eq!(change.area, vec!["language.result"]);
    }

    #[test]
    fn missing_block_is_error() {
        assert_eq!(
            Change::parse_pr_body("sem bloco algum").unwrap_err(),
            ChangeError::NoBlock
        );
    }

    #[test]
    fn yaml_roundtrip_is_idempotent() {
        let mut change = Change::parse_pr_body(BODY).unwrap();
        change.source = Some(Source {
            kind: "github-pr".to_string(),
            number: 341,
            repository: Some("LyannaValerie/pinker-v0".to_string()),
        });
        let once = change.render_yaml();
        let reparsed = Change::parse_block(&once).unwrap();
        let twice = reparsed.render_yaml();
        assert_eq!(once, twice);
        assert!(once.contains("number: 341"));
        assert!(once.contains("title: Biblioteca predeclarada"));
    }

    #[test]
    fn validate_rejects_invalid_enums() {
        let mut change = Change {
            schema: 1,
            kind: "banana".to_string(),
            title: "x".to_string(),
            status: "completed".to_string(),
            ..Default::default()
        };
        assert!(matches!(
            change.validate(),
            Err(ChangeError::InvalidEnum { .. })
        ));
        change.kind = "phase".to_string();
        change.status = "talvez".to_string();
        assert!(matches!(
            change.validate(),
            Err(ChangeError::InvalidEnum { .. })
        ));
    }

    #[test]
    fn validate_rejects_unknown_field() {
        let block = "schema: 1\nkind: phase\ntitle: x\nstatus: completed\nbanana: 42\n";
        let change = Change::parse_block(block).unwrap();
        assert!(matches!(
            change.validate(),
            Err(ChangeError::UnknownField { .. })
        ));
    }

    #[test]
    fn validate_rejects_bad_source_type_and_number() {
        let mut change = Change {
            schema: 1,
            kind: "phase".to_string(),
            title: "x".to_string(),
            status: "completed".to_string(),
            source: Some(Source {
                kind: "gitlab-mr".to_string(),
                number: 5,
                repository: None,
            }),
            ..Default::default()
        };
        assert!(matches!(
            change.validate(),
            Err(ChangeError::InvalidSourceType { .. })
        ));
        change.source = Some(Source {
            kind: "github-pr".to_string(),
            number: 0,
            repository: None,
        });
        assert!(matches!(
            change.validate(),
            Err(ChangeError::InvalidNumber { .. })
        ));
    }

    #[test]
    fn validate_rejects_bad_id_format() {
        let change = Change {
            schema: 1,
            kind: "phase".to_string(),
            title: "x".to_string(),
            status: "completed".to_string(),
            area: vec!["Nao Valido".to_string()],
            ..Default::default()
        };
        assert!(matches!(
            change.validate(),
            Err(ChangeError::InvalidIdFormat { .. })
        ));
    }

    #[test]
    fn validate_requires_core_fields() {
        let mut change = Change {
            schema: 1,
            ..Default::default()
        };
        assert!(change.validate().is_err());
        change.kind = "phase".to_string();
        change.title = "x".to_string();
        change.status = "completed".to_string();
        assert!(change.validate().is_ok());
    }

    #[test]
    fn ledger_line_has_pr_and_title() {
        let mut change = Change::parse_pr_body(BODY).unwrap();
        change.source = Some(Source {
            kind: "github-pr".to_string(),
            number: 341,
            repository: None,
        });
        let line = change.ledger_json();
        assert!(line.contains("\"pr\":341"));
        assert!(line.contains("\"phase\":241"));
        assert!(line.contains("\"status\":\"completed\""));
    }
}
