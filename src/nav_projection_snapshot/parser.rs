//! Parser TOML estrito do snapshot histórico, movido de
//! `src/nav_projection_snapshot.rs` pela unidade NPS-1 do inventário da #601
//! (Task #617).
//!
//! Só o arquivo mudou: a região cartografada, a ordem em que cada tabela é
//! aceita, cada rejeição e toda a validação estrutural e semântica do schema
//! continuam exatamente como estavam. A autoridade de domínio dos snapshots
//! históricos continua sendo o módulo `nav_projection_snapshot`, que ganhou um
//! arquivo e não uma segunda autoridade: o schema, o significado de
//! `FROZEN`/`CANDIDATE`, a separação entre falha de harness e drift, a ordem
//! das regras de reconstrução e o orçamento de consumo continuam decididos
//! numa única definição, aqui hospedada.
//!
//! `super` mudou de significado ao descer um nível, e o `use` abaixo devolve ao
//! irmão o vocabulário do pai — o modelo, a taxonomia de falhas, as versões de
//! schema e os limites — sem promover nada: um filho enxerga os itens privados
//! do pai por privacidade de módulo, e este `use` é privado.
//!
//! Nenhum item mudou de visibilidade. O pai reexporta exatamente os itens que
//! já eram alcançáveis por `nav_projection_snapshot::`, de modo que
//! `pinker_v0::nav_projection_snapshot::parse` e `::validate_rules` continuam
//! sendo os mesmos caminhos públicos de antes.

use super::*;

// @pinker-nav:start trama.snapshots.parser
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Parser TOML estrito do snapshot: aceita apenas tabelas conhecidas, rejeita chave desconhecida, chave duplicada, seção duplicada, string incompleta, escape não suportado, dado residual após o valor, número negativo e overflow, e aplica em seguida toda a validação estrutural e semântica do schema, incluindo o orçamento e a validação por campo do fato histórico materializado.

/// Valor escalar aceito pelo subconjunto TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scalar {
    Text(String),
    Integer(u64),
    /// Lista de textos, na ordem declarada. Usada apenas por `recipes`, cuja
    /// ordem é procedural.
    List(Vec<String>),
}

/// Uma tabela em construção: pares na ordem de aparição, com detecção de
/// duplicidade.
#[derive(Debug, Default)]
pub(crate) struct Table {
    pairs: Vec<(String, Scalar, usize)>,
}

impl Scalar {
    pub(crate) fn as_integer(&self) -> Option<u64> {
        match self {
            Scalar::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

impl Table {
    fn insert(&mut self, key: String, value: Scalar, line: usize) -> Result<(), TomlError> {
        if self.pairs.iter().any(|(existing, _, _)| existing == &key) {
            return Err(TomlError {
                line,
                msg: format!("chave duplicada '{}'", key),
            });
        }
        self.pairs.push((key, value, line));
        Ok(())
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Scalar> {
        self.pairs
            .iter()
            .find(|(existing, _, _)| existing == key)
            .map(|(_, value, _)| value)
    }

    fn keys(&self) -> Vec<&str> {
        self.pairs.iter().map(|(key, _, _)| key.as_str()).collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct RawDocument {
    pub(crate) root: Table,
    pub(crate) reconstruction: Option<Table>,
    pub(crate) measures: Option<Table>,
    pub(crate) rules: Vec<Table>,
}

/// Interpreta o texto de um snapshot e valida o schema por inteiro.
///
/// Não toca no filesystem: recebe o conteúdo já em memória.
pub fn parse(text: &str) -> Result<ProjectionSnapshot, HarnessFailure> {
    let raw = parse_raw(text).map_err(HarnessFailure::Toml)?;
    build(raw)
}

pub(crate) fn parse_raw(text: &str) -> Result<RawDocument, TomlError> {
    let mut doc = RawDocument::default();
    let mut current = Section::Root;
    let mut seen_reconstruction = false;
    let mut seen_measures = false;

    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("[[") {
            let Some(name) = rest.strip_suffix("]]") else {
                return Err(TomlError {
                    line: line_no,
                    msg: "cabeçalho de array de tabelas sem ']]'".to_string(),
                });
            };
            if name.trim() != "rules" {
                return Err(TomlError {
                    line: line_no,
                    msg: format!("array de tabelas desconhecido '[[{}]]'", name.trim()),
                });
            }
            doc.rules.push(Table::default());
            current = Section::Rule(doc.rules.len() - 1);
            continue;
        }

        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                return Err(TomlError {
                    line: line_no,
                    msg: "cabeçalho de seção sem ']'".to_string(),
                });
            };
            match name.trim() {
                "reconstruction" => {
                    if seen_reconstruction {
                        return Err(TomlError {
                            line: line_no,
                            msg: "seção duplicada '[reconstruction]'".to_string(),
                        });
                    }
                    seen_reconstruction = true;
                    doc.reconstruction = Some(Table::default());
                    current = Section::Reconstruction;
                }
                "measures" => {
                    if seen_measures {
                        return Err(TomlError {
                            line: line_no,
                            msg: "seção duplicada '[measures]'".to_string(),
                        });
                    }
                    seen_measures = true;
                    doc.measures = Some(Table::default());
                    current = Section::Measures;
                }
                other => {
                    return Err(TomlError {
                        line: line_no,
                        msg: format!("seção desconhecida '[{}]'", other),
                    })
                }
            }
            continue;
        }

        let Some(eq) = line.find('=') else {
            return Err(TomlError {
                line: line_no,
                msg: "linha sem '=' (esperado 'chave = valor')".to_string(),
            });
        };
        let key = line[..eq].trim();
        if key.is_empty() {
            return Err(TomlError {
                line: line_no,
                msg: "chave vazia".to_string(),
            });
        }
        if !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(TomlError {
                line: line_no,
                msg: format!("chave '{}' fora do alfabeto aceito ([a-z0-9_])", key),
            });
        }
        let value = parse_value(line[eq + 1..].trim(), line_no)?;

        let table = match current {
            Section::Root => &mut doc.root,
            Section::Reconstruction => doc.reconstruction.as_mut().expect("seção registrada"),
            Section::Measures => doc.measures.as_mut().expect("seção registrada"),
            Section::Rule(idx) => &mut doc.rules[idx],
        };
        table.insert(key.to_string(), value, line_no)?;
    }

    Ok(doc)
}

enum Section {
    Root,
    Reconstruction,
    Measures,
    Rule(usize),
}

/// Interpreta um valor escalar e exige que nada sobre depois dele.
fn parse_value(input: &str, line: usize) -> Result<Scalar, TomlError> {
    if input.is_empty() {
        return Err(TomlError {
            line,
            msg: "valor vazio".to_string(),
        });
    }

    // Lista de textos: única forma agregada aceita, e só para `recipes`, cuja
    // ordem declarada é significado. Uma linha, sem aninhamento.
    if let Some(rest) = input.strip_prefix('[') {
        let Some(interior) = rest.strip_suffix(']') else {
            return Err(TomlError {
                line,
                msg: "lista sem ']' de fechamento na mesma linha".to_string(),
            });
        };
        let interior = interior.trim();
        if interior.is_empty() {
            return Ok(Scalar::List(Vec::new()));
        }
        let mut itens = Vec::new();
        for bruto in interior.split(',') {
            let item = bruto.trim();
            let Some(sem_aspas) = item.strip_prefix('"').and_then(|r| r.strip_suffix('"')) else {
                return Err(TomlError {
                    line,
                    msg: format!("item de lista fora do formato \"texto\": '{}'", item),
                });
            };
            if sem_aspas.contains('"') || sem_aspas.contains('\\') {
                return Err(TomlError {
                    line,
                    msg: "item de lista não aceita aspas nem escape".to_string(),
                });
            }
            itens.push(sem_aspas.to_string());
        }
        return Ok(Scalar::List(itens));
    }

    if let Some(rest) = input.strip_prefix('"') {
        let mut text = String::new();
        let mut chars = rest.char_indices();
        loop {
            let Some((offset, ch)) = chars.next() else {
                return Err(TomlError {
                    line,
                    msg: "string incompleta: aspas de fechamento ausentes".to_string(),
                });
            };
            match ch {
                '"' => {
                    let tail = rest[offset + 1..].trim();
                    if !tail.is_empty() && !tail.starts_with('#') {
                        return Err(TomlError {
                            line,
                            msg: format!("dado residual após o valor: '{}'", tail),
                        });
                    }
                    return Ok(Scalar::Text(text));
                }
                '\\' => {
                    let Some((_, escape)) = chars.next() else {
                        return Err(TomlError {
                            line,
                            msg: "string incompleta: escape sem caractere".to_string(),
                        });
                    };
                    match escape {
                        '"' => text.push('"'),
                        '\\' => text.push('\\'),
                        'n' => text.push('\n'),
                        'r' => text.push('\r'),
                        't' => text.push('\t'),
                        other => {
                            return Err(TomlError {
                                line,
                                msg: format!("escape não suportado '\\{}'", other),
                            })
                        }
                    }
                }
                other => text.push(other),
            }
        }
    }

    let token = match input.find('#') {
        Some(pos) => input[..pos].trim(),
        None => input,
    };
    if token.is_empty() {
        return Err(TomlError {
            line,
            msg: "valor vazio".to_string(),
        });
    }
    if token.starts_with('-') {
        return Err(TomlError {
            line,
            msg: format!("número negativo não é aceito: '{}'", token),
        });
    }
    if !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TomlError {
            line,
            msg: format!(
                "valor '{}' fora do subconjunto aceito (texto entre aspas ou inteiro)",
                token
            ),
        });
    }
    match token.parse::<u64>() {
        Ok(value) => Ok(Scalar::Integer(value)),
        Err(_) => Err(TomlError {
            line,
            msg: format!("overflow de inteiro em '{}'", token),
        }),
    }
}

const ROOT_KEYS: [&str; 5] = ["schema", "id", "state", "predecessor", "justification"];
const RECONSTRUCTION_KEYS: [&str; 5] = [
    "expected_overrides",
    "expected_exclusions",
    "expected_materializations",
    "base_snapshot",
    "recipes",
];
const MEASURES_KEYS: [&str; 3] = ["regions", "length", "fnv1a64"];
/// Campos permitidos **por operação**, em tabela única.
///
/// [`RULE_KEYS`] é a união de todos os campos e só detecta chave desconhecida
/// pelo conjunto inteiro. Sem esta segunda camada, um campo legítimo de outra
/// operação — `from_summary` numa regra `override-hash`, por exemplo — passava
/// pelo filtro global e era **silenciosamente ignorado** pelo braço que não o lê.
///
/// A tabela é a fonte única: acrescentar capacidade a uma operação é editar uma
/// linha aqui, não lembrar de um `if` espalhado pelo braço correspondente.
const RULE_KEYS_BY_OP: [(&str, &[&str]); 7] = [
    (
        "override-hash",
        &[
            "op",
            "key",
            "from",
            "to",
            "expect_file",
            "expect_domain",
            "expect_layer",
        ],
    ),
    (
        "override-region",
        &[
            "op",
            "key",
            "from_hash",
            "to_hash",
            "from_summary",
            "to_summary",
            "expect_file",
            "to_file",
            "expect_domain",
            "expect_layer",
        ],
    ),
    ("exclude-key", &["op", "key", "expected_matches"]),
    ("exclude-key-prefix", &["op", "prefix", "expected_matches"]),
    ("exclude-file", &["op", "file", "expected_matches"]),
    ("exclude-file-prefix", &["op", "prefix", "expected_matches"]),
    (
        "materialize-region",
        &[
            "op", "key", "kind", "domain", "layer", "file", "summary", "hash", "status",
        ],
    ),
];

/// Campos permitidos para uma operação, ou `None` se a operação é desconhecida.
fn allowed_keys_for_op(op: &str) -> Option<&'static [&'static str]> {
    RULE_KEYS_BY_OP
        .iter()
        .find(|(nome, _)| *nome == op)
        .map(|(_, campos)| *campos)
}

const RULE_KEYS: [&str; 21] = [
    "op",
    "key",
    "from",
    "to",
    "from_hash",
    "to_hash",
    "from_summary",
    "to_summary",
    "expect_file",
    "to_file",
    "expect_domain",
    "expect_layer",
    "prefix",
    "file",
    "expected_matches",
    "kind",
    "domain",
    "layer",
    "summary",
    "hash",
    "status",
];

pub(crate) fn reject_unknown(
    table: &Table,
    allowed: &[&str],
    scope: &str,
) -> Result<(), HarnessFailure> {
    for key in table.keys() {
        if !allowed.contains(&key) {
            return Err(HarnessFailure::InvalidField {
                field: format!("{}{}", scope, key),
                msg: "chave desconhecida".to_string(),
            });
        }
    }
    Ok(())
}

pub(crate) fn require_text(
    table: &Table,
    key: &str,
    scope: &str,
) -> Result<String, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Text(value)) => Ok(value.clone()),
        Some(_) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado texto entre aspas".to_string(),
        }),
        None => Err(HarnessFailure::MissingField {
            field: format!("{}{}", scope, key),
        }),
    }
}

pub(crate) fn optional_text(
    table: &Table,
    key: &str,
    scope: &str,
) -> Result<Option<String>, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Text(value)) => Ok(Some(value.clone())),
        Some(_) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado texto entre aspas".to_string(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn require_integer(
    table: &Table,
    key: &str,
    scope: &str,
) -> Result<u64, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Integer(value)) => Ok(*value),
        Some(_) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado inteiro, não texto".to_string(),
        }),
        None => Err(HarnessFailure::MissingField {
            field: format!("{}{}", scope, key),
        }),
    }
}

pub(crate) fn optional_list(
    table: &Table,
    key: &str,
    scope: &str,
) -> Result<Vec<String>, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::List(itens)) => Ok(itens.clone()),
        Some(_) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado lista de textos".to_string(),
        }),
        None => Ok(Vec::new()),
    }
}

/// Um identificador é seguro quando pode virar nome de arquivo sem ambiguidade.
pub(crate) fn validate_id(value: &str, field: &str) -> Result<(), HarnessFailure> {
    let unsafe_id = || HarnessFailure::IdUnsafe {
        field: field.to_string(),
        value: value.to_string(),
    };
    if value.is_empty() || value.len() > MAX_ID_LEN {
        return Err(unsafe_id());
    }
    let bytes = value.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let separator = |b: u8| b == b'.' || b == b'-' || b == b'_';
    if !alnum(bytes[0]) || !alnum(bytes[bytes.len() - 1]) {
        return Err(unsafe_id());
    }
    let mut previous_separator = false;
    for &byte in bytes {
        if alnum(byte) {
            previous_separator = false;
        } else if separator(byte) {
            if previous_separator {
                return Err(unsafe_id());
            }
            previous_separator = true;
        } else {
            return Err(unsafe_id());
        }
    }
    Ok(())
}

fn validate_hash(value: &str, field: &str) -> Result<u64, HarnessFailure> {
    let invalid = || HarnessFailure::HashInvalid {
        field: field.to_string(),
        value: value.to_string(),
    };
    let Some(digits) = value.strip_prefix(FNV_PREFIX) else {
        return Err(invalid());
    };
    if digits.len() != 16
        || !digits
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(invalid());
    }
    u64::from_str_radix(digits, 16).map_err(|_| invalid())
}

/// Um path repo-relativo não pode ser absoluto nem conter travessia.
fn validate_relative_path(value: &str, field: &str) -> Result<(), HarnessFailure> {
    if value.is_empty() {
        return Err(HarnessFailure::InvalidField {
            field: field.to_string(),
            msg: "path vazio".to_string(),
        });
    }
    if value.starts_with('/') {
        return Err(HarnessFailure::PathAbsolute {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    if value.split('/').any(|component| component == "..") {
        return Err(HarnessFailure::PathTraversal {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

/// A validação de autoridade e capacidade de um modelo, seja ele parseado ou
/// construído em memória.
///
/// Existe porque as duas coisas que este validador cobra — quais operações
/// pertencem a esta autoridade, e qual versão cada uma exige — são propriedades
/// do **modelo**, não do texto. Deixá-las só no parser tornava a regra
/// contornável: uma [`crate::nav_projection_recipe::Recipe`] construída
/// diretamente em Rust, com os campos públicos que ela tem, entrava numa
/// `Library` e materializava região sem passar por `parse_recipe`.
///
/// Por isso o mesmo validador roda nas duas fronteiras que importam:
///
/// ```text
/// ingestão   parse / parse_recipe
/// execução   Library::with_snapshot / Library::with_recipe
/// ```
///
/// A renderização continua sendo serialização pura do modelo — ela não decide
/// validade —, e quem tiver um modelo vindo da API e quiser saber antes de
/// serializar chama [`ProjectionSnapshot::validate_model`] ou
/// [`crate::nav_projection_recipe::Recipe::validate_model`].
pub fn validate_rules(
    schema: u64,
    rules: &[Rule],
    authority: SchemaAuthority,
) -> Result<(), HarnessFailure> {
    if !authority.supports(schema) {
        return Err(HarnessFailure::SchemaUnknown {
            authority,
            found: schema,
        });
    }
    for rule in rules {
        // Materializar afirma um fato histórico, e só um snapshot tem medidas,
        // estado e predecessor para responder por ele. A recusa é nomeada.
        if authority == SchemaAuthority::Recipe && rule.is_materialization() {
            return Err(HarnessFailure::OperationOutsideAuthority {
                authority,
                op: rule.op().to_string(),
            });
        }
        let exigido = rule.min_schema(authority);
        if exigido > schema {
            return Err(HarnessFailure::CapabilityRequiresSchema {
                authority,
                capability: format!("op '{}'", rule.op()),
                found_schema: schema,
                required_schema: exigido,
            });
        }
    }
    Ok(())
}

/// Campo textual obrigatório que também não pode ser vazio.
///
/// `summary` fica de fora desta regra de propósito: ele participa da medida e é
/// obrigatório, mas resumo vazio é um fato histórico possível, e inventar uma
/// proibição aqui seria política nova, não validação.
fn require_nonempty(table: &Table, field: &str, scope: &str) -> Result<String, HarnessFailure> {
    let value = require_text(table, field, scope)?;
    if value.is_empty() {
        return Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, field),
            msg: "valor vazio".to_string(),
        });
    }
    Ok(value)
}

fn build(raw: RawDocument) -> Result<ProjectionSnapshot, HarnessFailure> {
    reject_unknown(&raw.root, &ROOT_KEYS, "")?;

    let schema = match raw.root.get("schema") {
        Some(Scalar::Integer(value)) => *value,
        Some(_) => {
            return Err(HarnessFailure::InvalidField {
                field: "schema".to_string(),
                msg: "esperado inteiro, não texto".to_string(),
            })
        }
        None => {
            return Err(HarnessFailure::SchemaUnknown {
                authority: SchemaAuthority::Snapshot,
                found: 0,
            })
        }
    };
    if !(SNAPSHOT_SCHEMA_V1..=SNAPSHOT_SCHEMA_V5).contains(&schema) {
        return Err(HarnessFailure::SchemaUnknown {
            authority: SchemaAuthority::Snapshot,
            found: schema,
        });
    }

    let id = require_text(&raw.root, "id", "")?;
    validate_id(&id, "id")?;

    let state_text = require_text(&raw.root, "state", "")?;
    let Some(state) = SnapshotState::parse(&state_text) else {
        return Err(HarnessFailure::StateUnknown { value: state_text });
    };

    let predecessor = optional_text(&raw.root, "predecessor", "")?;
    if let Some(predecessor) = &predecessor {
        validate_id(predecessor, "predecessor")?;
        if predecessor == &id {
            return Err(HarnessFailure::PredecessorSelfReference { id: id.clone() });
        }
    }

    let justification = optional_text(&raw.root, "justification", "")?;
    if let Some(text) = &justification {
        if text.trim().is_empty() {
            return Err(HarnessFailure::InvalidField {
                field: "justification".to_string(),
                msg: "justificativa vazia".to_string(),
            });
        }
    }
    if justification.is_none() && (state == SnapshotState::Candidate || predecessor.is_some()) {
        return Err(HarnessFailure::MissingField {
            field: "justification".to_string(),
        });
    }

    let Some(measures_table) = raw.measures else {
        return Err(HarnessFailure::MissingField {
            field: "measures".to_string(),
        });
    };
    reject_unknown(&measures_table, &MEASURES_KEYS, "measures.")?;
    let regions = require_integer(&measures_table, "regions", "measures.")?;
    let length = require_integer(&measures_table, "length", "measures.")?;
    let hash_text = require_text(&measures_table, "fnv1a64", "measures.")?;
    let fnv1a64 = validate_hash(&hash_text, "measures.fnv1a64")?;

    let Some(reconstruction_table) = raw.reconstruction else {
        return Err(HarnessFailure::MissingField {
            field: "reconstruction".to_string(),
        });
    };
    reject_unknown(
        &reconstruction_table,
        &RECONSTRUCTION_KEYS,
        "reconstruction.",
    )?;
    let expected_overrides = require_integer(
        &reconstruction_table,
        "expected_overrides",
        "reconstruction.",
    )?;
    let expected_exclusions = require_integer(
        &reconstruction_table,
        "expected_exclusions",
        "reconstruction.",
    )?;
    // Orçamento próprio da materialização: capacidade do schema 4. Ausente
    // significa zero, e num arquivo anterior declará-lo é falha explícita —
    // nenhum schema antigo ganha interpretação nova em silêncio.
    let expected_materializations = match reconstruction_table.get("expected_materializations") {
        None => 0,
        Some(_) => {
            if schema < SNAPSHOT_SCHEMA_V4 {
                return Err(HarnessFailure::CapabilityRequiresSchema {
                    authority: SchemaAuthority::Snapshot,
                    capability: "reconstruction.expected_materializations".to_string(),
                    found_schema: schema,
                    required_schema: SNAPSHOT_SCHEMA_V4,
                });
            }
            require_integer(
                &reconstruction_table,
                "expected_materializations",
                "reconstruction.",
            )?
        }
    };

    // Composição: capacidade do schema 2. Num arquivo schema 1 ela é falha
    // explícita, nunca leitura silenciosa.
    let base_snapshot = optional_text(&reconstruction_table, "base_snapshot", "reconstruction.")?;
    let recipes = optional_list(&reconstruction_table, "recipes", "reconstruction.")?;
    if schema < SNAPSHOT_SCHEMA_V2 {
        if base_snapshot.is_some() {
            return Err(HarnessFailure::CapabilityRequiresSchema {
                authority: SchemaAuthority::Snapshot,
                capability: "reconstruction.base_snapshot".to_string(),
                found_schema: schema,
                required_schema: SNAPSHOT_SCHEMA_V2,
            });
        }
        if !recipes.is_empty() {
            return Err(HarnessFailure::CapabilityRequiresSchema {
                authority: SchemaAuthority::Snapshot,
                capability: "reconstruction.recipes".to_string(),
                found_schema: schema,
                required_schema: SNAPSHOT_SCHEMA_V2,
            });
        }
    }
    if let Some(base) = &base_snapshot {
        validate_id(base, "reconstruction.base_snapshot")?;
        if base == &id {
            return Err(HarnessFailure::SelfBase { id: id.clone() });
        }
    }
    for (posicao, receita) in recipes.iter().enumerate() {
        validate_id(receita, &format!("reconstruction.recipes[{}]", posicao))?;
        if recipes[..posicao].contains(receita) {
            return Err(HarnessFailure::InvalidField {
                field: format!("reconstruction.recipes[{}]", posicao),
                msg: format!("receita '{}' declarada duas vezes no mesmo escopo", receita),
            });
        }
    }

    let mut rules = Vec::with_capacity(raw.rules.len());
    for (index, table) in raw.rules.iter().enumerate() {
        rules.push(build_rule(table, index)?);
    }
    validate_rules(schema, &rules, SchemaAuthority::Snapshot)?;

    let found_overrides = rules.iter().filter(|rule| rule.is_override()).count() as u64;
    let found_materializations = rules
        .iter()
        .filter(|rule| rule.is_materialization())
        .count() as u64;
    let found_exclusions = rules.len() as u64 - found_overrides - found_materializations;
    if expected_overrides > found_overrides {
        return Err(HarnessFailure::OverrideMissing {
            declared: expected_overrides,
            found: found_overrides,
        });
    }
    if expected_overrides < found_overrides {
        return Err(HarnessFailure::OverrideExcess {
            declared: expected_overrides,
            found: found_overrides,
        });
    }
    if expected_exclusions > found_exclusions {
        return Err(HarnessFailure::ExclusionMissing {
            declared: expected_exclusions,
            found: found_exclusions,
        });
    }
    if expected_exclusions < found_exclusions {
        return Err(HarnessFailure::ExclusionExcess {
            declared: expected_exclusions,
            found: found_exclusions,
        });
    }
    if expected_materializations > found_materializations {
        return Err(HarnessFailure::MaterializationMissing {
            declared: expected_materializations,
            found: found_materializations,
        });
    }
    if expected_materializations < found_materializations {
        return Err(HarnessFailure::MaterializationExcess {
            declared: expected_materializations,
            found: found_materializations,
        });
    }

    for (position, rule) in rules.iter().enumerate() {
        for other in &rules[position + 1..] {
            if rule.selector() != other.selector() {
                continue;
            }
            if rule.is_override() && other.is_override() {
                return Err(HarnessFailure::OverrideRepeated {
                    key: rule.selector().to_string(),
                });
            }
            if rule.is_materialization() && other.is_materialization() {
                return Err(HarnessFailure::MaterializationRepeated {
                    key: rule.selector().to_string(),
                });
            }
            if !rule.is_override()
                && !rule.is_materialization()
                && !other.is_override()
                && !other.is_materialization()
                && rule.op() == other.op()
            {
                return Err(HarnessFailure::ExclusionRepeated {
                    selector: rule.selector().to_string(),
                });
            }
        }
    }

    sort_rules(&mut rules);

    Ok(ProjectionSnapshot {
        schema,
        id,
        state,
        predecessor,
        justification,
        base_snapshot,
        recipes,
        measures: Measures {
            regions,
            length,
            fnv1a64,
        },
        expected_overrides,
        expected_exclusions,
        expected_materializations,
        rules,
    })
}

pub(crate) fn sort_rules(rules: &mut [Rule]) {
    rules.sort_by(|a, b| {
        a.op_rank()
            .cmp(&b.op_rank())
            .then_with(|| a.selector().cmp(b.selector()))
    });
}

pub(crate) fn build_rule(table: &Table, index: usize) -> Result<Rule, HarnessFailure> {
    reject_unknown(table, &RULE_KEYS, &format!("rules[{}].", index))?;
    let scope = format!("rules[{}].", index);

    let op = match table.get("op") {
        Some(Scalar::Text(value)) => value.clone(),
        Some(_) => {
            return Err(HarnessFailure::InvalidField {
                field: format!("{}op", scope),
                msg: "esperado texto entre aspas".to_string(),
            })
        }
        None => return Err(HarnessFailure::RuleWithoutOperation { index }),
    };

    // Estriteza por operação: o filtro global só conhece a união dos campos.
    // Aqui cada operação responde pelos seus, e um campo que pertence a outra
    // falha explicitamente em vez de ser descartado em silêncio.
    let Some(permitidos) = allowed_keys_for_op(op.as_str()) else {
        return Err(HarnessFailure::RuleOperationUnknown { index, op });
    };
    for chave in table.keys() {
        if !permitidos.contains(&chave) {
            return Err(HarnessFailure::FieldNotAllowedForOp {
                op: op.clone(),
                field: chave.to_string(),
            });
        }
    }

    match op.as_str() {
        "override-hash" => {
            let key = match optional_text(table, "key", &scope)? {
                Some(key) => key,
                None => {
                    return Err(HarnessFailure::RuleWithoutSelector { index, op });
                }
            };
            if key.is_empty() {
                return Err(HarnessFailure::RuleWithoutSelector { index, op });
            }
            let from = require_text(table, "from", &scope)?;
            validate_hash(&from, &format!("{}from", scope))?;
            let to = require_text(table, "to", &scope)?;
            validate_hash(&to, &format!("{}to", scope))?;
            let expect_file = optional_text(table, "expect_file", &scope)?;
            if let Some(file) = &expect_file {
                validate_relative_path(file, &format!("{}expect_file", scope))?;
            }
            let expect_domain = optional_text(table, "expect_domain", &scope)?;
            let expect_layer = optional_text(table, "expect_layer", &scope)?;
            Ok(Rule::OverrideHash {
                key,
                from,
                to,
                expect_file,
                expect_domain,
                expect_layer,
            })
        }
        "exclude-key" => {
            let key = match optional_text(table, "key", &scope)? {
                Some(key) if !key.is_empty() => key,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            let expected_matches = require_integer(table, "expected_matches", &scope)?;
            if expected_matches == 0 {
                return Err(HarnessFailure::InvalidField {
                    field: format!("{}expected_matches", scope),
                    msg: "exclusão precisa consumir ao menos uma correspondência".to_string(),
                });
            }
            Ok(Rule::ExcludeKey {
                key,
                expected_matches,
            })
        }
        "exclude-file-prefix" => {
            let prefix = match optional_text(table, "prefix", &scope)? {
                Some(prefix) if !prefix.is_empty() => prefix,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            validate_relative_path(&prefix, &format!("{}prefix", scope))?;
            let expected_matches = require_integer(table, "expected_matches", &scope)?;
            if expected_matches == 0 {
                return Err(HarnessFailure::InvalidField {
                    field: format!("{}expected_matches", scope),
                    msg: "exclusão precisa consumir ao menos uma correspondência".to_string(),
                });
            }
            Ok(Rule::ExcludeFilePrefix {
                prefix,
                expected_matches,
            })
        }
        "override-region" => {
            let key = match optional_text(table, "key", &scope)? {
                Some(key) if !key.is_empty() => key,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            let from_hash = optional_text(table, "from_hash", &scope)?;
            let to_hash = optional_text(table, "to_hash", &scope)?;
            let from_summary = optional_text(table, "from_summary", &scope)?;
            let to_summary = optional_text(table, "to_summary", &scope)?;

            // Meio par é inválido: um `from` sem `to` não descreve restauração
            // alguma, e um `to` sem `from` seria mutação sem precondição.
            if from_hash.is_some() != to_hash.is_some() {
                return Err(HarnessFailure::OverrideRegionPairInvalid {
                    key,
                    msg: "'from_hash' e 'to_hash' precisam vir juntos".to_string(),
                });
            }
            if from_summary.is_some() != to_summary.is_some() {
                return Err(HarnessFailure::OverrideRegionPairInvalid {
                    key,
                    msg: "'from_summary' e 'to_summary' precisam vir juntos".to_string(),
                });
            }
            let expect_file = optional_text(table, "expect_file", &scope)?;
            let to_file = optional_text(table, "to_file", &scope)?;

            // `expect_file` é a origem declarada da relocação. Sem ela, `to_file`
            // seria mutação de caminho sem precondição — exatamente o meio par
            // que os outros dois campos já recusam.
            if to_file.is_some() && expect_file.is_none() {
                return Err(HarnessFailure::OverrideRegionPairInvalid {
                    key,
                    msg: "'to_file' exige 'expect_file' como origem declarada".to_string(),
                });
            }
            if from_hash.is_none() && from_summary.is_none() && to_file.is_none() {
                return Err(HarnessFailure::OverrideRegionPairInvalid {
                    key,
                    msg: "ao menos um par completo é obrigatório".to_string(),
                });
            }
            if let Some(valor) = &from_hash {
                validate_hash(valor, &format!("{}from_hash", scope))?;
            }
            if let Some(valor) = &to_hash {
                validate_hash(valor, &format!("{}to_hash", scope))?;
            }
            if let Some(file) = &expect_file {
                validate_relative_path(file, &format!("{}expect_file", scope))?;
            }
            if let Some(file) = &to_file {
                validate_relative_path(file, &format!("{}to_file", scope))?;
            }
            Ok(Rule::OverrideRegion {
                key,
                from_hash,
                to_hash,
                from_summary,
                to_summary,
                expect_file,
                to_file,
                expect_domain: optional_text(table, "expect_domain", &scope)?,
                expect_layer: optional_text(table, "expect_layer", &scope)?,
            })
        }
        "exclude-file" => {
            let file = match optional_text(table, "file", &scope)? {
                Some(file) if !file.is_empty() => file,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            validate_relative_path(&file, &format!("{}file", scope))?;
            let expected_matches = require_integer(table, "expected_matches", &scope)?;
            if expected_matches == 0 {
                return Err(HarnessFailure::InvalidField {
                    field: format!("{}expected_matches", scope),
                    msg: "exclusão precisa consumir ao menos uma correspondência".to_string(),
                });
            }
            Ok(Rule::ExcludeFile {
                file,
                expected_matches,
            })
        }
        "materialize-region" => {
            let key = match optional_text(table, "key", &scope)? {
                Some(key) if !key.is_empty() => key,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            // O fato histórico é declarado por inteiro. Cada campo obrigatório
            // participa da projeção estável, e nenhum campo que não participa é
            // aceito: a lista permitida por operação já recusou o resto.
            let kind = require_nonempty(table, "kind", &scope)?;
            let file = require_nonempty(table, "file", &scope)?;
            validate_relative_path(&file, &format!("{}file", scope))?;
            let summary = require_text(table, "summary", &scope)?;
            let hash = require_text(table, "hash", &scope)?;
            validate_hash(&hash, &format!("{}hash", scope))?;
            let status = require_nonempty(table, "status", &scope)?;
            Ok(Rule::MaterializeRegion {
                key,
                kind,
                domain: optional_text(table, "domain", &scope)?,
                layer: optional_text(table, "layer", &scope)?,
                file,
                summary,
                hash,
                status,
            })
        }
        "exclude-key-prefix" => {
            let prefix = match optional_text(table, "prefix", &scope)? {
                Some(prefix) if !prefix.is_empty() => prefix,
                _ => return Err(HarnessFailure::RuleWithoutSelector { index, op }),
            };
            let expected_matches = require_integer(table, "expected_matches", &scope)?;
            if expected_matches == 0 {
                return Err(HarnessFailure::InvalidField {
                    field: format!("{}expected_matches", scope),
                    msg: "exclusão precisa consumir ao menos uma correspondência".to_string(),
                });
            }
            Ok(Rule::ExcludeKeyPrefix {
                prefix,
                expected_matches,
            })
        }
        other => Err(HarnessFailure::RuleOperationUnknown {
            index,
            op: other.to_string(),
        }),
    }
}
// @pinker-nav:end trama.snapshots.parser
