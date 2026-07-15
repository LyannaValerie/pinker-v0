//! Trama Pinker — Etapa 0 (Marco).
//!
//! Este módulo implementa a **política estrita anti-retroatividade** descrita
//! na especificação "Trama Pinker" (seções 15, 16 e 27). Ele lê a configuração
//! canônica em `.pinker/doc.toml` e decide se um dado PR pode ou não ser
//! importado pela ferramenta documental.
//!
//! Regra central (`forward-only`): somente PRs **posteriores** ao marco
//! (`baseline_pr`, por padrão exclusivo) entram no sistema. PRs anteriores ou
//! iguais são rejeitados com o código de erro `E-DOC-BASELINE`. Não há backfill
//! automático de PRs antigos.
//!
//! O módulo é deliberadamente puro: carrega e valida a configuração, e devolve
//! decisões. Toda impressão e término de processo é responsabilidade do CLI.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Caminho da configuração canônica, relativo à raiz do repositório.
pub const CONFIG_RELATIVE_PATH: &str = ".pinker/doc.toml";

/// Versão de esquema suportada por esta versão inicial da Trama.
pub const SUPPORTED_SCHEMA: u64 = 1;

/// Único modo aceito na versão inicial: apenas eventos posteriores ao marco.
pub const FORWARD_ONLY_MODE: &str = "forward-only";

/// Política de importação vinda da seção `[github]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubPolicy {
    pub mode: String,
    pub baseline_pr: u64,
    /// `false` => o próprio `baseline_pr` é exclusivo (não importável).
    pub baseline_inclusive: bool,
    pub baseline_commit: String,
    /// Repositório `owner/repo` registrado nos manifestos importados (opcional).
    pub repository: Option<String>,
}

/// Caminhos dos catálogos derivados, vindos da seção `[generated]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPaths {
    pub docs_index: String,
    pub code_index: String,
}

/// Mapeamento de uma projeção documental para sua região de destino (§12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocProjection {
    pub name: String,
    pub file: String,
    pub region: String,
}

/// Configuração completa de `.pinker/doc.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocConfig {
    pub schema: u64,
    pub github: GithubPolicy,
    pub generated: GeneratedPaths,
    /// Projeções documentais determinísticas (§12), ordenadas por nome.
    pub projections: Vec<DocProjection>,
}

/// Falhas ao carregar ou validar a configuração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Io { path: String, msg: String },
    Parse { line: usize, msg: String },
    MissingField { field: String },
    InvalidField { field: String, msg: String },
    UnsupportedSchema { found: u64 },
    UnsupportedMode { found: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, msg } => {
                write!(f, "E-DOC-CONFIG\nFalha ao ler '{}': {}", path, msg)
            }
            ConfigError::Parse { line, msg } => {
                write!(
                    f,
                    "E-DOC-CONFIG\nErro de sintaxe em {} (linha {}): {}",
                    CONFIG_RELATIVE_PATH, line, msg
                )
            }
            ConfigError::MissingField { field } => {
                write!(
                    f,
                    "E-DOC-CONFIG\nCampo obrigatório ausente em {}: '{}'",
                    CONFIG_RELATIVE_PATH, field
                )
            }
            ConfigError::InvalidField { field, msg } => {
                write!(
                    f,
                    "E-DOC-CONFIG\nCampo inválido em {}: '{}' — {}",
                    CONFIG_RELATIVE_PATH, field, msg
                )
            }
            ConfigError::UnsupportedSchema { found } => write!(
                f,
                "E-DOC-CONFIG\nEsquema {} não suportado; esta versão da Trama exige schema = {}.",
                found, SUPPORTED_SCHEMA
            ),
            ConfigError::UnsupportedMode { found } => write!(
                f,
                "E-DOC-CONFIG\nModo '{}' não suportado; a versão inicial só aceita mode = \"{}\".",
                found, FORWARD_ONLY_MODE
            ),
        }
    }
}

/// Rejeição de importação por violar o marco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRejection {
    pub pr: u64,
    pub baseline_pr: u64,
    pub inclusive: bool,
}

impl fmt::Display for BaselineRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let relacao = if self.inclusive {
            "anterior"
        } else {
            "anterior ou igual"
        };
        let limite = if self.inclusive {
            "inclusivo"
        } else {
            "exclusivo"
        };
        write!(
            f,
            "E-DOC-BASELINE\n\
             O PR #{pr} é {relacao} ao marco documental #{baseline}.\n\
             A importação retroativa está desativada.\n\
             Marco atual:\n    \
             PR #{baseline}, {limite}",
            pr = self.pr,
            relacao = relacao,
            baseline = self.baseline_pr,
            limite = limite,
        )
    }
}

// @pinker-nav:start trama.documentos.marco
// @pinker-nav:domain documentos
// @pinker-nav:layer trama
// @pinker-nav:summary Carrega e valida `.pinker/doc.toml` (marco, política forward-only e projeções) e aplica o gate anti-retroatividade: PRs anteriores ou iguais ao baseline são rejeitados com E-DOC-BASELINE, sem backfill.
impl DocConfig {
    /// Carrega e valida a configuração a partir da raiz do repositório.
    pub fn load(repo_root: &Path) -> Result<DocConfig, ConfigError> {
        let path: PathBuf = repo_root.join(CONFIG_RELATIVE_PATH);
        let text = fs::read_to_string(&path).map_err(|err| ConfigError::Io {
            path: path.display().to_string(),
            msg: err.to_string(),
        })?;
        Self::parse(&text)
    }

    /// Interpreta o conteúdo textual de `doc.toml` (útil também para testes).
    pub fn parse(text: &str) -> Result<DocConfig, ConfigError> {
        let raw = RawToml::parse(text)?;

        let schema = raw.require_u64("", "schema")?;
        if schema != SUPPORTED_SCHEMA {
            return Err(ConfigError::UnsupportedSchema { found: schema });
        }

        let mode = raw.require_str("github", "mode")?.to_string();
        if mode != FORWARD_ONLY_MODE {
            return Err(ConfigError::UnsupportedMode { found: mode });
        }
        let baseline_pr = raw.require_u64("github", "baseline_pr")?;
        let baseline_inclusive = raw.require_bool("github", "baseline_inclusive")?;
        let baseline_commit = raw.require_str("github", "baseline_commit")?.to_string();
        let repository = raw.optional_str("github", "repository").map(str::to_string);

        let docs_index = raw.require_str("generated", "docs_index")?.to_string();
        let code_index = raw.require_str("generated", "code_index")?.to_string();

        let projections = raw.projections()?;

        Ok(DocConfig {
            schema,
            github: GithubPolicy {
                mode,
                baseline_pr,
                baseline_inclusive,
                baseline_commit,
                repository,
            },
            generated: GeneratedPaths {
                docs_index,
                code_index,
            },
            projections,
        })
    }

    /// Aplica a política do marco a um número de PR.
    ///
    /// - modo exclusivo (`baseline_inclusive = false`): rejeita `pr <= baseline`;
    /// - modo inclusivo (`baseline_inclusive = true`): rejeita `pr <  baseline`.
    pub fn baseline_gate(&self, pr: u64) -> Result<(), BaselineRejection> {
        let baseline = self.github.baseline_pr;
        let rejected = if self.github.baseline_inclusive {
            pr < baseline
        } else {
            pr <= baseline
        };
        if rejected {
            Err(BaselineRejection {
                pr,
                baseline_pr: baseline,
                inclusive: self.github.baseline_inclusive,
            })
        } else {
            Ok(())
        }
    }
}
// @pinker-nav:end trama.documentos.marco

/// Leitor mínimo e determinístico de um subconjunto de TOML.
///
/// Suporta apenas o necessário para `doc.toml`: cabeçalhos `[secao]`, pares
/// `chave = valor` com escalares (string entre aspas, inteiro ou booleano),
/// comentários iniciados por `#` e linhas em branco. Não há dependências
/// externas — coerente com a filosofia zero-dependência do compilador.
struct RawToml {
    /// Chaveado por `"secao.chave"` (seção vazia para chaves de topo).
    values: HashMap<String, RawScalar>,
}

#[derive(Debug, Clone)]
struct RawScalar {
    text: String,
    quoted: bool,
}

impl RawToml {
    fn parse(text: &str) -> Result<RawToml, ConfigError> {
        let mut values = HashMap::new();
        let mut section = String::new();

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(rest) = line.strip_prefix('[') {
                let Some(name) = rest.strip_suffix(']') else {
                    return Err(ConfigError::Parse {
                        line: line_no,
                        msg: "cabeçalho de seção sem ']'".to_string(),
                    });
                };
                section = name.trim().to_string();
                continue;
            }

            let Some(eq) = line.find('=') else {
                return Err(ConfigError::Parse {
                    line: line_no,
                    msg: "linha sem '=' (esperado 'chave = valor')".to_string(),
                });
            };
            let key = line[..eq].trim();
            if key.is_empty() {
                return Err(ConfigError::Parse {
                    line: line_no,
                    msg: "chave vazia".to_string(),
                });
            }
            let value = parse_scalar(line[eq + 1..].trim(), line_no)?;
            let full_key = if section.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", section, key)
            };
            values.insert(full_key, value);
        }

        Ok(RawToml { values })
    }

    fn get(&self, section: &str, key: &str) -> Option<&RawScalar> {
        let full = if section.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", section, key)
        };
        self.values.get(&full)
    }

    fn require_str(&self, section: &str, key: &str) -> Result<&str, ConfigError> {
        let field = field_name(section, key);
        let scalar = self.get(section, key).ok_or(ConfigError::MissingField {
            field: field.clone(),
        })?;
        if !scalar.quoted {
            return Err(ConfigError::InvalidField {
                field,
                msg: "esperado texto entre aspas".to_string(),
            });
        }
        Ok(scalar.text.as_str())
    }

    fn optional_str(&self, section: &str, key: &str) -> Option<&str> {
        self.get(section, key)
            .filter(|scalar| scalar.quoted)
            .map(|scalar| scalar.text.as_str())
    }

    fn require_u64(&self, section: &str, key: &str) -> Result<u64, ConfigError> {
        let field = field_name(section, key);
        let scalar = self.get(section, key).ok_or(ConfigError::MissingField {
            field: field.clone(),
        })?;
        if scalar.quoted {
            return Err(ConfigError::InvalidField {
                field,
                msg: "esperado inteiro, não texto".to_string(),
            });
        }
        scalar
            .text
            .parse::<u64>()
            .map_err(|_| ConfigError::InvalidField {
                field,
                msg: format!("'{}' não é um inteiro válido", scalar.text),
            })
    }

    /// Extrai as projeções declaradas em `[projections.<name>]` (§12).
    /// Cada projeção precisa de `file` e `region`. Determinístico por nome.
    fn projections(&self) -> Result<Vec<DocProjection>, ConfigError> {
        use std::collections::BTreeSet;
        let mut names: BTreeSet<String> = BTreeSet::new();
        for key in self.values.keys() {
            if let Some(rest) = key.strip_prefix("projections.") {
                if let Some((name, _)) = rest.rsplit_once('.') {
                    names.insert(name.to_string());
                }
            }
        }
        let mut projections = Vec::new();
        for name in names {
            let section = format!("projections.{}", name);
            let file = self.require_str(&section, "file")?.to_string();
            let region = self.require_str(&section, "region")?.to_string();
            projections.push(DocProjection { name, file, region });
        }
        Ok(projections)
    }

    fn require_bool(&self, section: &str, key: &str) -> Result<bool, ConfigError> {
        let field = field_name(section, key);
        let scalar = self.get(section, key).ok_or(ConfigError::MissingField {
            field: field.clone(),
        })?;
        match scalar.text.as_str() {
            "true" if !scalar.quoted => Ok(true),
            "false" if !scalar.quoted => Ok(false),
            _ => Err(ConfigError::InvalidField {
                field,
                msg: "esperado booleano (true/false)".to_string(),
            }),
        }
    }
}

fn field_name(section: &str, key: &str) -> String {
    if section.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", section, key)
    }
}

/// Extrai um escalar TOML: string entre aspas, ou token não-quotado com
/// comentário `#` opcional à direita.
fn parse_scalar(input: &str, line_no: usize) -> Result<RawScalar, ConfigError> {
    if let Some(rest) = input.strip_prefix('"') {
        let Some(end) = rest.find('"') else {
            return Err(ConfigError::Parse {
                line: line_no,
                msg: "string sem aspas de fechamento".to_string(),
            });
        };
        return Ok(RawScalar {
            text: rest[..end].to_string(),
            quoted: true,
        });
    }

    // Valor não-quotado: descarta comentário à direita e espaços.
    let unquoted = match input.find('#') {
        Some(pos) => input[..pos].trim(),
        None => input.trim(),
    };
    if unquoted.is_empty() {
        return Err(ConfigError::Parse {
            line: line_no,
            msg: "valor vazio".to_string(),
        });
    }
    Ok(RawScalar {
        text: unquoted.to_string(),
        quoted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# comentário de topo
schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "15e22d4d510f298282c11cafeb21718859f9493a"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

    fn sample_config() -> DocConfig {
        DocConfig::parse(SAMPLE).expect("configuração de amostra válida")
    }

    #[test]
    fn parses_all_fields() {
        let cfg = sample_config();
        assert_eq!(cfg.schema, 1);
        assert_eq!(cfg.github.mode, "forward-only");
        assert_eq!(cfg.github.baseline_pr, 330);
        assert!(!cfg.github.baseline_inclusive);
        assert_eq!(
            cfg.github.baseline_commit,
            "15e22d4d510f298282c11cafeb21718859f9493a"
        );
        assert_eq!(cfg.generated.docs_index, "docs/navigation.jsonl");
        assert_eq!(cfg.generated.code_index, "src/navigation.jsonl");
    }

    #[test]
    fn exclusive_baseline_rejects_pr_at_and_below_marco() {
        let cfg = sample_config();
        assert!(cfg.baseline_gate(329).is_err());
        assert!(cfg.baseline_gate(330).is_err());
        assert!(cfg.baseline_gate(331).is_ok());
    }

    #[test]
    fn inclusive_baseline_allows_the_marco_itself() {
        let mut cfg = sample_config();
        cfg.github.baseline_inclusive = true;
        assert!(cfg.baseline_gate(329).is_err());
        assert!(cfg.baseline_gate(330).is_ok());
        assert!(cfg.baseline_gate(331).is_ok());
    }

    #[test]
    fn rejection_message_matches_spec() {
        let cfg = sample_config();
        let rejection = cfg.baseline_gate(329).unwrap_err();
        let rendered = rejection.to_string();
        assert!(rendered.starts_with("E-DOC-BASELINE"));
        assert!(rendered.contains("O PR #329 é anterior ou igual ao marco documental #330."));
        assert!(rendered.contains("A importação retroativa está desativada."));
        assert!(rendered.contains("PR #330, exclusivo"));
    }

    #[test]
    fn unsupported_mode_is_rejected() {
        let text = SAMPLE.replace("forward-only", "backfill");
        let err = DocConfig::parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedMode { .. }));
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let text = SAMPLE.replace("schema = 1", "schema = 2");
        let err = DocConfig::parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedSchema { found: 2 }));
    }

    #[test]
    fn missing_required_field_is_reported() {
        let text = SAMPLE.replace("baseline_pr = 330\n", "");
        let err = DocConfig::parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::MissingField { .. }));
    }

    #[test]
    fn quoted_where_integer_expected_is_invalid() {
        let text = SAMPLE.replace("baseline_pr = 330", "baseline_pr = \"330\"");
        let err = DocConfig::parse(&text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidField { .. }));
    }
}
