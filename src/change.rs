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
}

/// Falhas de manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    NoBlock,
    UnterminatedBlock,
    UnsupportedSchema { found: u64 },
    MissingField { field: String },
    NumberMismatch { file: u64, source: u64 },
    Io { path: String, msg: String },
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
            ChangeError::Io { path, msg } => {
                write!(f, "E-CHANGE-IO\nFalha em '{}': {}", path, msg)
            }
        }
    }
}

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
                let value = unquote(item.trim());
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
            let rest = trimmed[colon + 1..].trim().to_string();

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
                    _ => {}
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
            change.source = Some(source);
        }

        Ok(change)
    }

    /// Valida campos obrigatórios e o esquema.
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.schema != 1 {
            return Err(ChangeError::UnsupportedSchema { found: self.schema });
        }
        if self.kind.is_empty() {
            return Err(ChangeError::MissingField {
                field: "kind".to_string(),
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

/// Manifestos carregados de `.pinker/changes/`.
#[derive(Debug, Clone, Default)]
pub struct Manifests {
    pub changes: Vec<Change>,
    pub problems: Vec<ChangeError>,
}

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
