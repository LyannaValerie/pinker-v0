//! Núcleo somente leitura dos snapshots históricos da cartografia da Trama.
//!
//! Estes snapshots congelam a projeção de regiões de código usada pelos gates
//! históricos de `nav`; eles não são as projeções documentais tratadas por
//! [`crate::projection`]. A primeira fatia trabalha exclusivamente sobre dados
//! já carregados em memória: não oferece CLI, descoberta de repositório,
//! escrita de snapshots nem lifecycle mutável.
//!
//! O parser implementa somente o subset TOML do schema 1:
//!
//! - atribuições escalares de uma linha e `[[reconstruction]]`;
//! - strings UTF-8 entre aspas duplas, com escapes `\"`, `\\`, `\n`, `\r` e
//!   `\t`;
//! - inteiros decimais sem sinal ou separadores e booleanos `true`/`false`;
//! - linhas vazias, whitespace externo e finais de linha LF ou CRLF;
//! - campos e propriedades na ordem textual canônica do schema.
//!
//! Comentários, comentários inline, outros tipos TOML, outras tabelas e
//! conteúdo depois de um valor não são suportados e falham de forma
//! estruturada. O renderer sempre produz LF e exatamente uma quebra final.

use crate::nav::CodeRegion;
use std::collections::BTreeSet;
use std::fmt;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Estado versionado de um snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotStatus {
    /// Registro histórico imutável.
    Frozen,
    /// Proposta ainda não aceita; esta fatia apenas a representa e verifica.
    Candidate,
}

impl SnapshotStatus {
    fn parse(value: &str, line: usize) -> Result<Self, ProjectionHarnessError> {
        match value {
            "FROZEN" => Ok(Self::Frozen),
            "CANDIDATE" => Ok(Self::Candidate),
            other => Err(ProjectionHarnessError::UnknownEnum {
                line,
                field: "status".to_string(),
                value: other.to_string(),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Frozen => "FROZEN",
            Self::Candidate => "CANDIDATE",
        }
    }
}

/// Três medidas canônicas de uma projeção histórica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionMeasurement {
    /// Quantidade de regiões projetadas.
    pub region_count: usize,
    /// Comprimento do payload projetado, em bytes UTF-8.
    pub projection_length: usize,
    /// FNV-1a 64 do payload projetado.
    pub fnv1a64: u64,
}

/// Campo estável presente na tupla histórica de projeção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionField {
    /// Chave da região.
    Key,
    /// Tipo da região.
    Kind,
    /// Domínio opcional.
    Domain,
    /// Camada opcional.
    Layer,
    /// Caminho repo-relativo.
    File,
    /// Resumo humano.
    Summary,
    /// Hash FNV canônico da região.
    Hash,
    /// Estado textual.
    Status,
}

impl ProjectionField {
    fn parse(value: &str, line: usize) -> Result<Self, ProjectionHarnessError> {
        match value {
            "key" => Ok(Self::Key),
            "kind" => Ok(Self::Kind),
            "domain" => Ok(Self::Domain),
            "layer" => Ok(Self::Layer),
            "file" => Ok(Self::File),
            "summary" => Ok(Self::Summary),
            "hash" => Ok(Self::Hash),
            "status" => Ok(Self::Status),
            other => Err(ProjectionHarnessError::UnknownEnum {
                line,
                field: "field".to_string(),
                value: other.to_string(),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Kind => "kind",
            Self::Domain => "domain",
            Self::Layer => "layer",
            Self::File => "file",
            Self::Summary => "summary",
            Self::Hash => "hash",
            Self::Status => "status",
        }
    }
}

/// Seletor repo-relativo usado por uma regra.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionSelector {
    /// Chave única.
    Key(String),
    /// Caminho repo-relativo, potencialmente com múltiplas regiões.
    File(String),
}

impl ProjectionSelector {
    fn kind(&self) -> &'static str {
        match self {
            Self::Key(_) => "key",
            Self::File(_) => "file",
        }
    }

    fn value(&self) -> &str {
        match self {
            Self::Key(value) | Self::File(value) => value,
        }
    }
}

/// Operação pura de reconstrução ou guarda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionOperation {
    /// Exclui uma região.
    ExcludeRegion,
    /// Exclui todas as regiões de um arquivo.
    ExcludeFile,
    /// Restaura um campo mutável.
    RestoreField,
    /// Guarda presença e, opcionalmente, valor.
    RequireRegion,
    /// Guarda ausência.
    RequireAbsence,
}

impl ProjectionOperation {
    fn parse(value: &str, line: usize) -> Result<Self, ProjectionHarnessError> {
        match value {
            "EXCLUDE_REGION" => Ok(Self::ExcludeRegion),
            "EXCLUDE_FILE" => Ok(Self::ExcludeFile),
            "RESTORE_FIELD" => Ok(Self::RestoreField),
            "REQUIRE_REGION" => Ok(Self::RequireRegion),
            "REQUIRE_ABSENCE" => Ok(Self::RequireAbsence),
            other => Err(ProjectionHarnessError::UnknownEnum {
                line,
                field: "operation".to_string(),
                value: other.to_string(),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ExcludeRegion => "EXCLUDE_REGION",
            Self::ExcludeFile => "EXCLUDE_FILE",
            Self::RestoreField => "RESTORE_FIELD",
            Self::RequireRegion => "REQUIRE_REGION",
            Self::RequireAbsence => "REQUIRE_ABSENCE",
        }
    }
}

/// Cardinalidade declarada para o consumo de uma regra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionConsumption {
    /// Exige alvo único.
    ExactlyOnce,
    /// Exige ao menos um alvo e consome todos.
    AllMatchesAtLeastOne,
}

impl ProjectionConsumption {
    fn parse(value: &str, line: usize) -> Result<Self, ProjectionHarnessError> {
        match value {
            "EXACTLY_ONCE" => Ok(Self::ExactlyOnce),
            "ALL_MATCHES_AT_LEAST_ONE" => Ok(Self::AllMatchesAtLeastOne),
            other => Err(ProjectionHarnessError::UnknownEnum {
                line,
                field: "consumption".to_string(),
                value: other.to_string(),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ExactlyOnce => "EXACTLY_ONCE",
            Self::AllMatchesAtLeastOne => "ALL_MATCHES_AT_LEAST_ONE",
        }
    }
}

/// Regra ordenada do plano de reconstrução.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectionRule {
    /// Operação executada.
    pub operation: ProjectionOperation,
    /// Seletor avaliado no estado produzido pelas regras anteriores.
    pub selector: ProjectionSelector,
    /// Campo alvo de `RESTORE_FIELD` ou guardado por `REQUIRE_REGION`.
    pub field: Option<ProjectionField>,
    /// Valor histórico aplicado exclusivamente por `RESTORE_FIELD`.
    pub replacement: Option<String>,
    /// Valor corrente exigido exclusivamente por `REQUIRE_REGION`.
    pub expected: Option<String>,
    /// Deve ser `true`; regras opcionais não pertencem ao schema 1.
    pub required: bool,
    /// Cardinalidade de consumo.
    pub consumption: ProjectionConsumption,
}

/// Modelo completo de um snapshot histórico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    /// Versão, sempre `1`.
    pub schema: u32,
    /// ID estável.
    pub id: String,
    /// Estado do registro.
    pub status: SnapshotStatus,
    /// Descrição humana.
    pub description: String,
    /// Medida esperada.
    pub measurement: ProjectionMeasurement,
    /// ID predecessor opcional.
    pub predecessor: Option<String>,
    /// Regras ordenadas.
    pub reconstruction: Vec<ProjectionRule>,
    /// Justificativa humana.
    pub justification: String,
}

/// Resultado puro da reconstrução e cardinalidade consumida por regra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionReconstruction {
    /// Cópia reconstruída das regiões.
    pub regions: Vec<CodeRegion>,
    /// Quantidade de regiões correspondentes a cada regra, na mesma ordem.
    ///
    /// `REQUIRE_ABSENCE` consome a própria regra com cardinalidade regional
    /// zero; `EXCLUDE_FILE` registra todos os matches removidos.
    pub consumed_regions: Vec<usize>,
}

/// Campos de medição que divergiram depois de um harness válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionDrift {
    /// Medida registrada.
    pub expected: ProjectionMeasurement,
    /// Medida observada.
    pub observed: ProjectionMeasurement,
    /// Divergência de quantidade.
    pub region_count: bool,
    /// Divergência de comprimento.
    pub projection_length: bool,
    /// Divergência de FNV.
    pub fnv1a64: bool,
}

/// Resultado fechado de uma verificação somente leitura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionVerification {
    /// Medidas idênticas.
    Match { measurement: ProjectionMeasurement },
    /// Harness válido com medidas diferentes.
    Drift(ProjectionDrift),
    /// Falha anterior à comparação de medidas.
    HarnessFailure(ProjectionHarnessError),
}

/// Falhas estruturadas do harness de snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionHarnessError {
    InvalidToml {
        line: usize,
        message: String,
    },
    UnsupportedSchema {
        found: u32,
    },
    MissingField {
        section: String,
        field: String,
    },
    UnknownField {
        line: usize,
        section: String,
        field: String,
    },
    DuplicateField {
        line: usize,
        section: String,
        field: String,
    },
    UnknownSection {
        line: usize,
        section: String,
    },
    NonCanonicalOrder {
        line: usize,
        section: String,
        field: String,
    },
    InvalidNumber {
        line: usize,
        field: String,
        value: String,
    },
    UnknownEnum {
        line: usize,
        field: String,
        value: String,
    },
    InvalidId {
        field: String,
        value: String,
    },
    InvalidFnv {
        field: String,
        value: String,
    },
    InvalidPath {
        value: String,
    },
    InvalidString {
        field: String,
    },
    IncompleteRule {
        index: usize,
        message: String,
    },
    InvalidPredecessor {
        value: String,
    },
    OverrideMissing {
        rule: usize,
        selector: String,
    },
    OverrideExcess {
        rule: usize,
        selector: String,
        matches: usize,
    },
    OverrideRepeated {
        rule: usize,
        selector: String,
    },
    OverrideUnconsumed {
        rule: usize,
        selector: String,
    },
    UnexpectedRegionRemoval {
        rule: usize,
        selector: String,
    },
    UnexpectedRegionPresence {
        rule: usize,
        selector: String,
    },
    KeyChanged {
        rule: usize,
        expected: String,
        observed: String,
    },
    PathChanged {
        rule: usize,
        expected: String,
        observed: String,
    },
    MetadataChanged {
        rule: usize,
        field: ProjectionField,
        expected: String,
        observed: String,
    },
    DuplicateRegionKey {
        key: String,
    },
    ConflictingFieldOverride {
        rule: usize,
        selector: String,
        field: ProjectionField,
    },
    MeasurementUnavailable {
        message: String,
    },
}

impl fmt::Display for ProjectionHarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProjectionHarnessError {}

#[derive(Default)]
struct RawSnapshot {
    schema: Option<u32>,
    id: Option<String>,
    status: Option<SnapshotStatus>,
    description: Option<String>,
    region_count: Option<usize>,
    projection_length: Option<usize>,
    fnv1a64: Option<u64>,
    predecessor: Option<String>,
    justification: Option<String>,
    rules: Vec<RawRule>,
}

#[derive(Default)]
struct RawRule {
    operation: Option<ProjectionOperation>,
    selector_type: Option<String>,
    selector: Option<String>,
    field: Option<ProjectionField>,
    replacement: Option<String>,
    expected: Option<String>,
    required: Option<bool>,
    consumption: Option<ProjectionConsumption>,
}

/// Interpreta somente o subset TOML canônico do schema de snapshots.
pub fn parse_snapshot(text: &str) -> Result<ProjectionSnapshot, ProjectionHarnessError> {
    let mut raw = RawSnapshot::default();
    let mut root_seen = BTreeSet::new();
    let mut rule_seen: Vec<BTreeSet<String>> = Vec::new();
    let mut current_rule = None;
    let mut root_order = None;
    let mut rule_order = None;

    for (offset, source_line) in text.lines().enumerate() {
        let line_number = offset + 1;
        let line = source_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[reconstruction]]" {
            raw.rules.push(RawRule::default());
            rule_seen.push(BTreeSet::new());
            current_rule = Some(raw.rules.len() - 1);
            rule_order = None;
            continue;
        }
        if line.starts_with('[') {
            return Err(ProjectionHarnessError::UnknownSection {
                line: line_number,
                section: line.to_string(),
            });
        }
        let (field, value) = split_assignment(line, line_number)?;
        if let Some(index) = current_rule {
            let order =
                rule_field_order(field).ok_or_else(|| ProjectionHarnessError::UnknownField {
                    line: line_number,
                    section: "reconstruction".to_string(),
                    field: field.to_string(),
                })?;
            if !rule_seen[index].insert(field.to_string()) {
                return Err(ProjectionHarnessError::DuplicateField {
                    line: line_number,
                    section: "reconstruction".to_string(),
                    field: field.to_string(),
                });
            }
            enforce_order(&mut rule_order, order, line_number, "reconstruction", field)?;
            parse_rule_field(&mut raw.rules[index], field, value, line_number)?;
        } else {
            let order =
                root_field_order(field).ok_or_else(|| ProjectionHarnessError::UnknownField {
                    line: line_number,
                    section: "root".to_string(),
                    field: field.to_string(),
                })?;
            if !root_seen.insert(field.to_string()) {
                return Err(ProjectionHarnessError::DuplicateField {
                    line: line_number,
                    section: "root".to_string(),
                    field: field.to_string(),
                });
            }
            enforce_order(&mut root_order, order, line_number, "root", field)?;
            parse_root_field(&mut raw, field, value, line_number)?;
        }
    }

    finish_snapshot(raw)
}

fn split_assignment(
    line: &str,
    line_number: usize,
) -> Result<(&str, &str), ProjectionHarnessError> {
    let Some(separator) = line.find('=') else {
        return Err(ProjectionHarnessError::InvalidToml {
            line: line_number,
            message: "linha sem atribuição".to_string(),
        });
    };
    let field = line[..separator].trim();
    let value = line[separator + 1..].trim();
    if field.is_empty() || value.is_empty() {
        return Err(ProjectionHarnessError::InvalidToml {
            line: line_number,
            message: "atribuição incompleta".to_string(),
        });
    }
    Ok((field, value))
}

fn root_field_order(field: &str) -> Option<usize> {
    match field {
        "schema" => Some(0),
        "id" => Some(1),
        "status" => Some(2),
        "description" => Some(3),
        "region_count" => Some(4),
        "projection_length" => Some(5),
        "fnv1a64" => Some(6),
        "predecessor" => Some(7),
        "justification" => Some(8),
        _ => None,
    }
}

fn rule_field_order(field: &str) -> Option<usize> {
    match field {
        "operation" => Some(0),
        "selector_type" => Some(1),
        "selector" => Some(2),
        "field" => Some(3),
        "replacement" => Some(4),
        "expected" => Some(4),
        "required" => Some(5),
        "consumption" => Some(6),
        _ => None,
    }
}

fn enforce_order(
    previous: &mut Option<usize>,
    current: usize,
    line: usize,
    section: &str,
    field: &str,
) -> Result<(), ProjectionHarnessError> {
    if previous.is_some_and(|order| current < order) {
        return Err(ProjectionHarnessError::NonCanonicalOrder {
            line,
            section: section.to_string(),
            field: field.to_string(),
        });
    }
    *previous = Some(current);
    Ok(())
}

fn parse_root_field(
    raw: &mut RawSnapshot,
    field: &str,
    value: &str,
    line: usize,
) -> Result<(), ProjectionHarnessError> {
    match field {
        "schema" => raw.schema = Some(parse_u32(value, field, line)?),
        "id" => raw.id = Some(parse_string(value, line)?),
        "status" => {
            let value = parse_string(value, line)?;
            raw.status = Some(SnapshotStatus::parse(&value, line)?);
        }
        "description" => raw.description = Some(parse_string(value, line)?),
        "region_count" => raw.region_count = Some(parse_usize(value, field, line)?),
        "projection_length" => raw.projection_length = Some(parse_usize(value, field, line)?),
        "fnv1a64" => {
            let value = parse_string(value, line)?;
            raw.fnv1a64 = Some(parse_fnv(&value, field)?);
        }
        "predecessor" => raw.predecessor = Some(parse_string(value, line)?),
        "justification" => raw.justification = Some(parse_string(value, line)?),
        _ => {
            return Err(ProjectionHarnessError::UnknownField {
                line,
                section: "root".to_string(),
                field: field.to_string(),
            });
        }
    }
    Ok(())
}

fn parse_rule_field(
    raw: &mut RawRule,
    field: &str,
    value: &str,
    line: usize,
) -> Result<(), ProjectionHarnessError> {
    match field {
        "operation" => {
            let value = parse_string(value, line)?;
            raw.operation = Some(ProjectionOperation::parse(&value, line)?);
        }
        "selector_type" => raw.selector_type = Some(parse_string(value, line)?),
        "selector" => raw.selector = Some(parse_string(value, line)?),
        "field" => {
            let value = parse_string(value, line)?;
            raw.field = Some(ProjectionField::parse(&value, line)?);
        }
        "replacement" => raw.replacement = Some(parse_string(value, line)?),
        "expected" => raw.expected = Some(parse_string(value, line)?),
        "required" => raw.required = Some(parse_bool(value, field, line)?),
        "consumption" => {
            let value = parse_string(value, line)?;
            raw.consumption = Some(ProjectionConsumption::parse(&value, line)?);
        }
        _ => {
            return Err(ProjectionHarnessError::UnknownField {
                line,
                section: "reconstruction".to_string(),
                field: field.to_string(),
            });
        }
    }
    Ok(())
}

fn parse_u32(value: &str, field: &str, line: usize) -> Result<u32, ProjectionHarnessError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProjectionHarnessError::InvalidNumber {
            line,
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    value
        .parse::<u32>()
        .map_err(|_| ProjectionHarnessError::InvalidNumber {
            line,
            field: field.to_string(),
            value: value.to_string(),
        })
}

fn parse_usize(value: &str, field: &str, line: usize) -> Result<usize, ProjectionHarnessError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProjectionHarnessError::InvalidNumber {
            line,
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    value
        .parse::<usize>()
        .map_err(|_| ProjectionHarnessError::InvalidNumber {
            line,
            field: field.to_string(),
            value: value.to_string(),
        })
}

fn parse_bool(value: &str, field: &str, line: usize) -> Result<bool, ProjectionHarnessError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ProjectionHarnessError::InvalidToml {
            line,
            message: format!("booleano inválido em {field}"),
        }),
    }
}

fn parse_string(value: &str, line: usize) -> Result<String, ProjectionHarnessError> {
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return Err(ProjectionHarnessError::InvalidToml {
            line,
            message: "string deve usar aspas duplas".to_string(),
        });
    }
    let mut output = String::new();
    let mut escaped = false;
    for character in value[1..value.len() - 1].chars() {
        if escaped {
            match character {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                _ => {
                    return Err(ProjectionHarnessError::InvalidToml {
                        line,
                        message: "escape de string não suportado".to_string(),
                    });
                }
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' || character.is_control() {
            return Err(ProjectionHarnessError::InvalidToml {
                line,
                message: "string contém caractere inválido".to_string(),
            });
        } else {
            output.push(character);
        }
    }
    if escaped {
        return Err(ProjectionHarnessError::InvalidToml {
            line,
            message: "escape incompleto".to_string(),
        });
    }
    Ok(output)
}

fn finish_snapshot(raw: RawSnapshot) -> Result<ProjectionSnapshot, ProjectionHarnessError> {
    let schema = required(raw.schema, "root", "schema")?;
    if schema != 1 {
        return Err(ProjectionHarnessError::UnsupportedSchema { found: schema });
    }
    let snapshot = ProjectionSnapshot {
        schema,
        id: required(raw.id, "root", "id")?,
        status: required(raw.status, "root", "status")?,
        description: required(raw.description, "root", "description")?,
        measurement: ProjectionMeasurement {
            region_count: required(raw.region_count, "root", "region_count")?,
            projection_length: required(raw.projection_length, "root", "projection_length")?,
            fnv1a64: required(raw.fnv1a64, "root", "fnv1a64")?,
        },
        predecessor: raw.predecessor,
        reconstruction: raw
            .rules
            .into_iter()
            .enumerate()
            .map(|(index, rule)| finish_rule(index, rule))
            .collect::<Result<Vec<_>, _>>()?,
        justification: required(raw.justification, "root", "justification")?,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn required<T>(value: Option<T>, section: &str, field: &str) -> Result<T, ProjectionHarnessError> {
    value.ok_or_else(|| ProjectionHarnessError::MissingField {
        section: section.to_string(),
        field: field.to_string(),
    })
}

fn finish_rule(index: usize, raw: RawRule) -> Result<ProjectionRule, ProjectionHarnessError> {
    let operation = required(raw.operation, "reconstruction", "operation")?;
    let selector_type = required(raw.selector_type, "reconstruction", "selector_type")?;
    let selector_value = required(raw.selector, "reconstruction", "selector")?;
    let selector = match selector_type.as_str() {
        "key" => ProjectionSelector::Key(selector_value),
        "file" => ProjectionSelector::File(selector_value),
        _ => {
            return Err(ProjectionHarnessError::IncompleteRule {
                index,
                message: "selector_type deve ser key ou file".to_string(),
            });
        }
    };
    Ok(ProjectionRule {
        operation,
        selector,
        field: raw.field,
        replacement: raw.replacement,
        expected: raw.expected,
        required: required(raw.required, "reconstruction", "required")?,
        consumption: required(raw.consumption, "reconstruction", "consumption")?,
    })
}

/// Valida o modelo sem consultar disco ou estado externo.
pub fn validate_snapshot(snapshot: &ProjectionSnapshot) -> Result<(), ProjectionHarnessError> {
    if snapshot.schema != 1 {
        return Err(ProjectionHarnessError::UnsupportedSchema {
            found: snapshot.schema,
        });
    }
    validate_id(&snapshot.id, "id")?;
    if let Some(predecessor) = &snapshot.predecessor {
        validate_id(predecessor, "predecessor")?;
        if predecessor == &snapshot.id {
            return Err(ProjectionHarnessError::InvalidPredecessor {
                value: predecessor.clone(),
            });
        }
    }
    if snapshot.description.trim().is_empty() {
        return Err(ProjectionHarnessError::MissingField {
            section: "root".to_string(),
            field: "description".to_string(),
        });
    }
    validate_string_value("description", &snapshot.description)?;
    if snapshot.justification.trim().is_empty() {
        return Err(ProjectionHarnessError::MissingField {
            section: "root".to_string(),
            field: "justification".to_string(),
        });
    }
    validate_string_value("justification", &snapshot.justification)?;
    if snapshot.measurement.region_count == 0 || snapshot.measurement.projection_length == 0 {
        return Err(ProjectionHarnessError::MeasurementUnavailable {
            message: "medidas devem ser positivas".to_string(),
        });
    }
    for (index, rule) in snapshot.reconstruction.iter().enumerate() {
        validate_rule(index, rule)?;
    }
    Ok(())
}

fn validate_id(value: &str, field: &str) -> Result<(), ProjectionHarnessError> {
    if !valid_id(value) {
        return Err(ProjectionHarnessError::InvalidId {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    valid_identifier(value, false)
}

fn valid_key(value: &str) -> bool {
    valid_identifier(value, true)
}

fn valid_identifier(value: &str, allow_underscore: bool) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut expect_alnum = true;
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            expect_alnum = false;
        } else if (matches!(byte, b'.' | b'-') || (allow_underscore && byte == b'_'))
            && !expect_alnum
        {
            expect_alnum = true;
        } else {
            return false;
        }
    }
    !expect_alnum
}

fn valid_repo_path(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value.as_bytes().get(1).copied() == Some(b':')
    {
        return false;
    }
    value
        .split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn validate_rule(index: usize, rule: &ProjectionRule) -> Result<(), ProjectionHarnessError> {
    if !rule.required {
        return Err(ProjectionHarnessError::OverrideUnconsumed {
            rule: index,
            selector: rule.selector.value().to_string(),
        });
    }
    match &rule.selector {
        ProjectionSelector::Key(key) if !valid_key(key) => {
            return Err(ProjectionHarnessError::InvalidId {
                field: "selector".to_string(),
                value: key.clone(),
            });
        }
        ProjectionSelector::File(path) if !valid_repo_path(path) => {
            return Err(ProjectionHarnessError::InvalidPath {
                value: path.clone(),
            });
        }
        _ => {}
    }
    match rule.operation {
        ProjectionOperation::ExcludeRegion => {
            require_key_selector(index, rule)?;
            require_consumption(index, rule, ProjectionConsumption::ExactlyOnce)?;
            forbid_field_values(index, rule)?;
        }
        ProjectionOperation::ExcludeFile => {
            if !matches!(rule.selector, ProjectionSelector::File(_)) {
                return incomplete(index, "EXCLUDE_FILE exige seletor file");
            }
            require_consumption(index, rule, ProjectionConsumption::AllMatchesAtLeastOne)?;
            forbid_field_values(index, rule)?;
        }
        ProjectionOperation::RestoreField => {
            require_key_selector(index, rule)?;
            require_consumption(index, rule, ProjectionConsumption::ExactlyOnce)?;
            let field = rule
                .field
                .ok_or_else(|| ProjectionHarnessError::IncompleteRule {
                    index,
                    message: "RESTORE_FIELD exige field".to_string(),
                })?;
            if matches!(field, ProjectionField::Key | ProjectionField::File) {
                return incomplete(index, "RESTORE_FIELD não pode alterar key ou file");
            }
            if rule.replacement.is_none() {
                return incomplete(index, "RESTORE_FIELD exige replacement");
            }
            if rule.expected.is_some() {
                return incomplete(index, "RESTORE_FIELD não aceita expected");
            }
        }
        ProjectionOperation::RequireRegion => {
            require_consumption(index, rule, ProjectionConsumption::ExactlyOnce)?;
            if rule.replacement.is_some() {
                return incomplete(index, "REQUIRE_REGION não aceita replacement");
            }
            if rule.field.is_some() != rule.expected.is_some() {
                return incomplete(index, "REQUIRE_REGION exige field e expected em conjunto");
            }
        }
        ProjectionOperation::RequireAbsence => {
            require_key_selector(index, rule)?;
            require_consumption(index, rule, ProjectionConsumption::ExactlyOnce)?;
            forbid_field_values(index, rule)?;
        }
    }
    if let (Some(field), Some(value)) = (
        rule.field,
        rule.replacement.as_deref().or(rule.expected.as_deref()),
    ) {
        validate_field_value(field, value)?;
    }
    Ok(())
}

fn validate_field_value(field: ProjectionField, value: &str) -> Result<(), ProjectionHarnessError> {
    validate_string_value(field.as_str(), value)?;
    match field {
        ProjectionField::Key => {
            if valid_key(value) {
                Ok(())
            } else {
                Err(ProjectionHarnessError::InvalidId {
                    field: "expected".to_string(),
                    value: value.to_string(),
                })
            }
        }
        ProjectionField::File => {
            if valid_repo_path(value) {
                Ok(())
            } else {
                Err(ProjectionHarnessError::InvalidPath {
                    value: value.to_string(),
                })
            }
        }
        ProjectionField::Hash => {
            parse_fnv(value, "hash")?;
            Ok(())
        }
        ProjectionField::Kind | ProjectionField::Status if value.is_empty() => {
            Err(ProjectionHarnessError::MeasurementUnavailable {
                message: format!("valor vazio incompatível com {}", field.as_str()),
            })
        }
        ProjectionField::Domain
        | ProjectionField::Layer
        | ProjectionField::Summary
        | ProjectionField::Kind
        | ProjectionField::Status => Ok(()),
    }
}

fn validate_string_value(field: &str, value: &str) -> Result<(), ProjectionHarnessError> {
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(ProjectionHarnessError::InvalidString {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn require_key_selector(index: usize, rule: &ProjectionRule) -> Result<(), ProjectionHarnessError> {
    if !matches!(rule.selector, ProjectionSelector::Key(_)) {
        return incomplete(index, "operação exige seletor key");
    }
    Ok(())
}

fn require_consumption(
    index: usize,
    rule: &ProjectionRule,
    expected: ProjectionConsumption,
) -> Result<(), ProjectionHarnessError> {
    if rule.consumption != expected {
        return incomplete(index, "cardinalidade de consumo incompatível");
    }
    Ok(())
}

fn forbid_field_values(index: usize, rule: &ProjectionRule) -> Result<(), ProjectionHarnessError> {
    if rule.field.is_some() || rule.replacement.is_some() || rule.expected.is_some() {
        return incomplete(index, "operação não aceita field/replacement/expected");
    }
    Ok(())
}

fn incomplete<T>(index: usize, message: &str) -> Result<T, ProjectionHarnessError> {
    Err(ProjectionHarnessError::IncompleteRule {
        index,
        message: message.to_string(),
    })
}

fn parse_fnv(value: &str, field: &str) -> Result<u64, ProjectionHarnessError> {
    let Some(hex) = value.strip_prefix("fnv1a64:") else {
        return Err(ProjectionHarnessError::InvalidFnv {
            field: field.to_string(),
            value: value.to_string(),
        });
    };
    if hex.len() != 16
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProjectionHarnessError::InvalidFnv {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    u64::from_str_radix(hex, 16).map_err(|_| ProjectionHarnessError::InvalidFnv {
        field: field.to_string(),
        value: value.to_string(),
    })
}

/// Renderiza TOML canônico com ordem fixa e uma única quebra final.
pub fn render_snapshot(snapshot: &ProjectionSnapshot) -> Result<String, ProjectionHarnessError> {
    validate_snapshot(snapshot)?;
    let mut output = String::new();
    output.push_str("schema = 1\n");
    push_string_field(&mut output, "id", &snapshot.id);
    push_string_field(&mut output, "status", snapshot.status.as_str());
    push_string_field(&mut output, "description", &snapshot.description);
    output.push_str(&format!(
        "region_count = {}\nprojection_length = {}\n",
        snapshot.measurement.region_count, snapshot.measurement.projection_length
    ));
    push_string_field(
        &mut output,
        "fnv1a64",
        &format!("fnv1a64:{:016x}", snapshot.measurement.fnv1a64),
    );
    if let Some(predecessor) = &snapshot.predecessor {
        push_string_field(&mut output, "predecessor", predecessor);
    }
    push_string_field(&mut output, "justification", &snapshot.justification);
    for rule in &snapshot.reconstruction {
        output.push_str("\n[[reconstruction]]\n");
        push_string_field(&mut output, "operation", rule.operation.as_str());
        push_string_field(&mut output, "selector_type", rule.selector.kind());
        push_string_field(&mut output, "selector", rule.selector.value());
        if let Some(field) = rule.field {
            push_string_field(&mut output, "field", field.as_str());
        }
        if let Some(replacement) = &rule.replacement {
            push_string_field(&mut output, "replacement", replacement);
        }
        if let Some(expected) = &rule.expected {
            push_string_field(&mut output, "expected", expected);
        }
        output.push_str("required = true\n");
        push_string_field(&mut output, "consumption", rule.consumption.as_str());
    }
    Ok(output)
}

fn push_string_field(output: &mut String, field: &str, value: &str) {
    output.push_str(field);
    output.push_str(" = \"");
    output.push_str(&escape_string(value));
    output.push_str("\"\n");
}

fn escape_string(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            other => output.push(other),
        }
    }
    output
}

/// Produz exatamente os bytes usados pelos gates históricos atuais.
pub fn canonical_region_projection(
    regions: &[CodeRegion],
) -> Result<String, ProjectionHarnessError> {
    validate_regions(regions)?;
    let mut records: Vec<String> = regions
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
    Ok(records.concat())
}

fn validate_regions(regions: &[CodeRegion]) -> Result<(), ProjectionHarnessError> {
    let mut keys = BTreeSet::new();
    for region in regions {
        if !valid_key(&region.key) {
            return Err(ProjectionHarnessError::KeyChanged {
                rule: usize::MAX,
                expected: "chave canônica".to_string(),
                observed: region.key.clone(),
            });
        }
        if !keys.insert(region.key.clone()) {
            return Err(ProjectionHarnessError::DuplicateRegionKey {
                key: region.key.clone(),
            });
        }
        if !valid_repo_path(&region.file) {
            return Err(ProjectionHarnessError::PathChanged {
                rule: usize::MAX,
                expected: "path repo-relativo".to_string(),
                observed: region.file.clone(),
            });
        }
        parse_fnv(&region.hash, "hash").map_err(|_| ProjectionHarnessError::MetadataChanged {
            rule: usize::MAX,
            field: ProjectionField::Hash,
            expected: "fnv1a64 canônico".to_string(),
            observed: region.hash.clone(),
        })?;
        if region.kind.is_empty()
            || region.status.is_empty()
            || region.domain.as_ref().is_some_and(String::is_empty)
            || region.layer.as_ref().is_some_and(String::is_empty)
        {
            return Err(ProjectionHarnessError::MeasurementUnavailable {
                message: format!("metadata incompleta em {}", region.key),
            });
        }
    }
    Ok(())
}

/// Calcula FNV-1a 64 com aritmética modular definida pelo algoritmo.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

/// Mede a projeção histórica canônica das regiões recebidas.
pub fn measure_regions(
    regions: &[CodeRegion],
) -> Result<ProjectionMeasurement, ProjectionHarnessError> {
    let projection = canonical_region_projection(regions)?;
    Ok(ProjectionMeasurement {
        region_count: regions.len(),
        projection_length: projection.len(),
        fnv1a64: fnv1a64(projection.as_bytes()),
    })
}

/// Reconstrói o predecessor em uma cópia e exige consumo integral das regras.
pub fn reconstruct_predecessor(
    current: &[CodeRegion],
    rules: &[ProjectionRule],
) -> Result<Vec<CodeRegion>, ProjectionHarnessError> {
    reconstruct_predecessor_with_consumption(current, rules).map(|result| result.regions)
}

/// Reconstrói o predecessor e expõe a cardinalidade exata consumida por regra.
pub fn reconstruct_predecessor_with_consumption(
    current: &[CodeRegion],
    rules: &[ProjectionRule],
) -> Result<ProjectionReconstruction, ProjectionHarnessError> {
    validate_regions(current)?;
    let mut regions = current.to_vec();
    let mut seen = BTreeSet::new();
    let mut restored_fields = BTreeSet::new();
    let mut consumed = vec![None; rules.len()];

    for (index, rule) in rules.iter().enumerate() {
        validate_rule(index, rule)?;
        if !seen.insert(rule.clone()) {
            return Err(ProjectionHarnessError::OverrideRepeated {
                rule: index,
                selector: rule.selector.value().to_string(),
            });
        }
        if rule.operation == ProjectionOperation::RestoreField {
            let field = rule
                .field
                .ok_or_else(|| ProjectionHarnessError::IncompleteRule {
                    index,
                    message: "RESTORE_FIELD exige field".to_string(),
                })?;
            if !restored_fields.insert((rule.selector.clone(), field)) {
                return Err(ProjectionHarnessError::ConflictingFieldOverride {
                    rule: index,
                    selector: rule.selector.value().to_string(),
                    field,
                });
            }
        }
        let matches = matching_positions(&regions, &rule.selector);
        let consumed_regions = matches.len();
        match rule.operation {
            ProjectionOperation::ExcludeRegion | ProjectionOperation::RestoreField => {
                require_exactly_one(index, rule, matches.len())?;
            }
            ProjectionOperation::RequireRegion => {
                if matches.is_empty() {
                    return Err(ProjectionHarnessError::UnexpectedRegionRemoval {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                    });
                }
                if matches.len() > 1 {
                    return Err(ProjectionHarnessError::OverrideExcess {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                        matches: matches.len(),
                    });
                }
            }
            ProjectionOperation::ExcludeFile => {
                if matches.is_empty() {
                    return Err(ProjectionHarnessError::OverrideMissing {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                    });
                }
            }
            ProjectionOperation::RequireAbsence => {
                if !matches.is_empty() {
                    return Err(ProjectionHarnessError::UnexpectedRegionPresence {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                    });
                }
            }
        }

        match rule.operation {
            ProjectionOperation::ExcludeRegion | ProjectionOperation::ExcludeFile => {
                for position in matches.into_iter().rev() {
                    regions.remove(position);
                }
            }
            ProjectionOperation::RestoreField => {
                let position = matches.first().copied().ok_or_else(|| {
                    ProjectionHarnessError::OverrideMissing {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                    }
                })?;
                let field = rule
                    .field
                    .ok_or_else(|| ProjectionHarnessError::IncompleteRule {
                        index,
                        message: "RESTORE_FIELD exige field".to_string(),
                    })?;
                let replacement = rule.replacement.as_deref().ok_or_else(|| {
                    ProjectionHarnessError::IncompleteRule {
                        index,
                        message: "RESTORE_FIELD exige replacement".to_string(),
                    }
                })?;
                set_field(index, &mut regions[position], field, replacement)?;
                validate_region(&regions[position], index)?;
            }
            ProjectionOperation::RequireRegion => {
                let position = matches.first().copied().ok_or_else(|| {
                    ProjectionHarnessError::UnexpectedRegionRemoval {
                        rule: index,
                        selector: rule.selector.value().to_string(),
                    }
                })?;
                guard_expected(index, &regions[position], rule)?;
            }
            ProjectionOperation::RequireAbsence => {}
        }
        consumed[index] = Some(consumed_regions);
    }

    if let Some(index) = consumed.iter().position(Option::is_none) {
        return Err(ProjectionHarnessError::OverrideUnconsumed {
            rule: index,
            selector: rules[index].selector.value().to_string(),
        });
    }
    validate_regions(&regions)?;
    let consumed_regions = consumed.into_iter().flatten().collect();
    Ok(ProjectionReconstruction {
        regions,
        consumed_regions,
    })
}

fn validate_region(region: &CodeRegion, rule: usize) -> Result<(), ProjectionHarnessError> {
    if !valid_key(&region.key) {
        return Err(ProjectionHarnessError::KeyChanged {
            rule,
            expected: "chave canônica".to_string(),
            observed: region.key.clone(),
        });
    }
    if !valid_repo_path(&region.file) {
        return Err(ProjectionHarnessError::PathChanged {
            rule,
            expected: "path repo-relativo".to_string(),
            observed: region.file.clone(),
        });
    }
    parse_fnv(&region.hash, "hash").map_err(|_| ProjectionHarnessError::MetadataChanged {
        rule,
        field: ProjectionField::Hash,
        expected: "fnv1a64 canônico".to_string(),
        observed: region.hash.clone(),
    })?;
    if region.kind.is_empty()
        || region.status.is_empty()
        || region.domain.as_ref().is_some_and(String::is_empty)
        || region.layer.as_ref().is_some_and(String::is_empty)
    {
        return Err(ProjectionHarnessError::MeasurementUnavailable {
            message: format!("metadata incompleta em {}", region.key),
        });
    }
    Ok(())
}

fn matching_positions(regions: &[CodeRegion], selector: &ProjectionSelector) -> Vec<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, region)| match selector {
            ProjectionSelector::Key(key) => &region.key == key,
            ProjectionSelector::File(path) => &region.file == path,
        })
        .map(|(index, _)| index)
        .collect()
}

fn require_exactly_one(
    index: usize,
    rule: &ProjectionRule,
    matches: usize,
) -> Result<(), ProjectionHarnessError> {
    if matches == 0 {
        return Err(ProjectionHarnessError::OverrideMissing {
            rule: index,
            selector: rule.selector.value().to_string(),
        });
    }
    if matches > 1 {
        return Err(ProjectionHarnessError::OverrideExcess {
            rule: index,
            selector: rule.selector.value().to_string(),
            matches,
        });
    }
    Ok(())
}

fn field_value(region: &CodeRegion, field: ProjectionField) -> String {
    match field {
        ProjectionField::Key => region.key.clone(),
        ProjectionField::Kind => region.kind.clone(),
        ProjectionField::Domain => match &region.domain {
            Some(value) => value.clone(),
            None => String::new(),
        },
        ProjectionField::Layer => match &region.layer {
            Some(value) => value.clone(),
            None => String::new(),
        },
        ProjectionField::File => region.file.clone(),
        ProjectionField::Summary => region.summary.clone(),
        ProjectionField::Hash => region.hash.clone(),
        ProjectionField::Status => region.status.clone(),
    }
}

fn guard_expected(
    index: usize,
    region: &CodeRegion,
    rule: &ProjectionRule,
) -> Result<(), ProjectionHarnessError> {
    let (Some(field), Some(expected)) = (rule.field, rule.expected.as_deref()) else {
        return Ok(());
    };
    let observed = field_value(region, field);
    if observed == expected {
        return Ok(());
    }
    match field {
        ProjectionField::Key => Err(ProjectionHarnessError::KeyChanged {
            rule: index,
            expected: expected.to_string(),
            observed,
        }),
        ProjectionField::File => Err(ProjectionHarnessError::PathChanged {
            rule: index,
            expected: expected.to_string(),
            observed,
        }),
        _ => Err(ProjectionHarnessError::MetadataChanged {
            rule: index,
            field,
            expected: expected.to_string(),
            observed,
        }),
    }
}

fn set_field(
    index: usize,
    region: &mut CodeRegion,
    field: ProjectionField,
    replacement: &str,
) -> Result<(), ProjectionHarnessError> {
    match field {
        ProjectionField::Kind => region.kind = replacement.to_string(),
        ProjectionField::Domain => {
            region.domain = (!replacement.is_empty()).then(|| replacement.to_string())
        }
        ProjectionField::Layer => {
            region.layer = (!replacement.is_empty()).then(|| replacement.to_string())
        }
        ProjectionField::Summary => region.summary = replacement.to_string(),
        ProjectionField::Hash => region.hash = replacement.to_string(),
        ProjectionField::Status => region.status = replacement.to_string(),
        ProjectionField::Key | ProjectionField::File => {
            return Err(ProjectionHarnessError::IncompleteRule {
                index,
                message: "RESTORE_FIELD não pode alterar key ou file".to_string(),
            });
        }
    }
    Ok(())
}

/// Verifica somente depois de parse/modelo, reconstrução, consumo e medição.
pub fn verify_snapshot(
    snapshot: &ProjectionSnapshot,
    current: &[CodeRegion],
) -> ProjectionVerification {
    let observed = (|| {
        validate_snapshot(snapshot)?;
        let predecessor = reconstruct_predecessor(current, &snapshot.reconstruction)?;
        measure_regions(&predecessor)
    })();
    let observed = match observed {
        Ok(measurement) => measurement,
        Err(error) => return ProjectionVerification::HarnessFailure(error),
    };
    let expected = snapshot.measurement;
    if observed == expected {
        ProjectionVerification::Match {
            measurement: observed,
        }
    } else {
        ProjectionVerification::Drift(ProjectionDrift {
            expected,
            observed,
            region_count: expected.region_count != observed.region_count,
            projection_length: expected.projection_length != observed.projection_length,
            fnv1a64: expected.fnv1a64 != observed.fnv1a64,
        })
    }
}

/// Faz parse e verificação sem permitir que uma falha de entrada vire drift.
pub fn verify_snapshot_text(text: &str, current: &[CodeRegion]) -> ProjectionVerification {
    match parse_snapshot(text) {
        Ok(snapshot) => verify_snapshot(&snapshot, current),
        Err(error) => ProjectionVerification::HarnessFailure(error),
    }
}

/// Relatório JSON puro, com chaves e campos em ordem fixa.
pub fn render_verification_json(
    snapshot_id: &str,
    verification: &ProjectionVerification,
) -> String {
    let mut output = format!(
        "{{\"schema\":1,\"snapshot\":\"{}\",",
        escape_json(snapshot_id)
    );
    match verification {
        ProjectionVerification::Match { measurement } => {
            output.push_str("\"result\":\"MATCH\",\"expected\":");
            push_measurement_json(&mut output, measurement);
            output.push_str(",\"observed\":");
            push_measurement_json(&mut output, measurement);
            output.push_str(",\"drift\":null,\"error\":null}");
        }
        ProjectionVerification::Drift(drift) => {
            output.push_str("\"result\":\"DRIFT\",\"expected\":");
            push_measurement_json(&mut output, &drift.expected);
            output.push_str(",\"observed\":");
            push_measurement_json(&mut output, &drift.observed);
            output.push_str(&format!(
                ",\"drift\":{{\"region_count\":{},\"projection_length\":{},\"fnv1a64\":{}}},\"error\":null}}",
                drift.region_count, drift.projection_length, drift.fnv1a64
            ));
        }
        ProjectionVerification::HarnessFailure(error) => {
            output.push_str("\"result\":\"HARNESS_FAILURE\",\"expected\":null,\"observed\":null,\"drift\":null,\"error\":\"");
            output.push_str(&escape_json(&error.to_string()));
            output.push_str("\"}");
        }
    }
    output.push('\n');
    output
}

fn push_measurement_json(output: &mut String, measurement: &ProjectionMeasurement) {
    output.push_str(&format!(
        "{{\"region_count\":{},\"projection_length\":{},\"fnv1a64\":\"fnv1a64:{:016x}\"}}",
        measurement.region_count, measurement.projection_length, measurement.fnv1a64
    ));
}

fn escape_json(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output
}
