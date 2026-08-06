//! Trama Pinker — snapshots históricos das projeções do catálogo de navegação.
//!
//! Este módulo é a autoridade de **domínio** dos snapshots históricos descritos
//! na Issue #384. Ele não tem relação com `src/projection.rs`, que projeta os
//! manifestos versionados em regiões geradas de documentos humanos (§12). A
//! coincidência da palavra "projeção" é terminológica, não estrutural:
//!
//! - `src/projection.rs` — projeções **documentais** derivadas dos manifestos;
//! - este módulo — snapshots **históricos** de medidas do catálogo de código
//!   (`src/navigation.jsonl`), hoje mantidas como literais em
//!   `tests/nav_cartography_tests.rs`.
//!
//! # O que é um snapshot
//!
//! Um snapshot congela três medidas de uma projeção estável de regiões:
//!
//! 1. quantidade de regiões;
//! 2. comprimento em bytes da projeção estável;
//! 3. FNV-1a 64 da projeção estável, em formato canônico.
//!
//! Ele registra também o predecessor opcional e as **regras de reconstrução**
//! necessárias para recompor, a partir do catálogo corrente, o estado histórico
//! que produziu aquelas medidas.
//!
//! # Fronteiras deste recorte (Estágio A da Issue #417, itens 2 e 3)
//!
//! Este módulo é **estritamente somente leitura e puro**:
//!
//! - não abre, lê nem escreve arquivo algum;
//! - não descobre a raiz do repositório;
//! - não migra as medidas históricas reais;
//! - não cria, prepara nem aceita candidatos;
//! - não expõe superfície de CLI.
//!
//! O ciclo de vida mutável, a migração real e a CLI pertencem a etapas
//! posteriores da campanha.
//!
//! # Separação entre drift e falha de harness
//!
//! `DRIFT` só pode ser declarado **depois** de uma reconstrução válida, quando
//! alguma das três medidas diverge. Schema inválido, ID inseguro, hash malformado,
//! seletor sem correspondência, consumo incorreto de overrides ou predecessor
//! inconsistente são `HARNESS_FAILURE` e nunca são reclassificados como drift.

use crate::nav::CodeRegion;
use std::fmt;

/// Versão única e atual do schema de snapshot.
pub const SNAPSHOT_SCHEMA: u64 = 1;

/// Prefixo canônico do hash FNV-1a 64 usado pela Trama.
pub const FNV_PREFIX: &str = "fnv1a64:";

/// Comprimento máximo de um identificador de snapshot.
pub const MAX_ID_LEN: usize = 64;

// @pinker-nav:start trama.snapshots.modelo
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Modelo imutável dos snapshots históricos de projeção do catálogo de navegação: schema versionado, ID estável, estados FROZEN/CANDIDATE, medidas (regiões, comprimento, FNV-1a 64 canônico), predecessor opcional, justificativa e regras de reconstrução tipadas com orçamento explícito de consumo.

/// Estado de um snapshot. Só existem dois.
///
/// `FROZEN` é imutável: nunca é atualizado implicitamente. `CANDIDATE` só nasce
/// por preparação explícita e só vira `FROZEN` por aceitação explícita — ambas
/// as operações pertencem a etapas posteriores da campanha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Frozen,
    Candidate,
}

impl SnapshotState {
    /// Forma canônica em texto, usada no TOML e nos relatórios.
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotState::Frozen => "FROZEN",
            SnapshotState::Candidate => "CANDIDATE",
        }
    }

    fn parse(value: &str) -> Option<SnapshotState> {
        match value {
            "FROZEN" => Some(SnapshotState::Frozen),
            "CANDIDATE" => Some(SnapshotState::Candidate),
            _ => None,
        }
    }
}

impl fmt::Display for SnapshotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// As três medidas congeladas de uma projeção estável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measures {
    /// Quantidade de regiões na projeção.
    pub regions: u64,
    /// Comprimento em bytes da projeção estável.
    pub length: u64,
    /// FNV-1a 64 da projeção estável.
    pub fnv1a64: u64,
}

impl Measures {
    /// Forma canônica do hash: `fnv1a64:` seguido de 16 dígitos hexadecimais
    /// minúsculos.
    pub fn fnv1a64_canonical(&self) -> String {
        format!("{}{:016x}", FNV_PREFIX, self.fnv1a64)
    }
}

/// Uma regra de reconstrução, com o seletor e o orçamento de consumo explícitos.
///
/// Toda regra declara quantas correspondências deve consumir. Consumo diferente
/// do declarado é falha de harness, nunca drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// Restaura o hash de uma região identificada pela chave.
    ///
    /// Consome exatamente uma correspondência. `from` é o hash esperado no
    /// catálogo corrente e `to` é o hash histórico restaurado; `expect_*` são
    /// expectativas opcionais que detectam mudança de path ou de metadata.
    OverrideHash {
        key: String,
        from: String,
        to: String,
        expect_file: Option<String>,
        expect_domain: Option<String>,
        expect_layer: Option<String>,
    },
    /// Remove todas as regiões com a chave indicada.
    ExcludeKey { key: String, expected_matches: u64 },
    /// Remove todas as regiões cujo path repo-relativo começa pelo prefixo.
    ExcludeFilePrefix {
        prefix: String,
        expected_matches: u64,
    },
}

impl Rule {
    /// Nome canônico da operação.
    pub fn op(&self) -> &'static str {
        match self {
            Rule::OverrideHash { .. } => "override-hash",
            Rule::ExcludeKey { .. } => "exclude-key",
            Rule::ExcludeFilePrefix { .. } => "exclude-file-prefix",
        }
    }

    /// Seletor textual da regra, usado na ordenação canônica e nos relatórios.
    pub fn selector(&self) -> &str {
        match self {
            Rule::OverrideHash { key, .. } => key.as_str(),
            Rule::ExcludeKey { key, .. } => key.as_str(),
            Rule::ExcludeFilePrefix { prefix, .. } => prefix.as_str(),
        }
    }

    /// Quantas correspondências a regra deve consumir.
    pub fn expected_matches(&self) -> u64 {
        match self {
            Rule::OverrideHash { .. } => 1,
            Rule::ExcludeKey {
                expected_matches, ..
            } => *expected_matches,
            Rule::ExcludeFilePrefix {
                expected_matches, ..
            } => *expected_matches,
        }
    }

    fn is_override(&self) -> bool {
        matches!(self, Rule::OverrideHash { .. })
    }

    /// Ordem canônica entre operações: exclusões antes de overrides, que é
    /// também a ordem de aplicação.
    fn op_rank(&self) -> u8 {
        match self {
            Rule::ExcludeKey { .. } => 0,
            Rule::ExcludeFilePrefix { .. } => 1,
            Rule::OverrideHash { .. } => 2,
        }
    }
}

/// Um snapshot histórico completo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    pub schema: u64,
    pub id: String,
    pub state: SnapshotState,
    pub predecessor: Option<String>,
    pub justification: Option<String>,
    pub measures: Measures,
    /// Quantidade declarada de regras `override-hash`, para detectar override
    /// ausente ou excedente antes de qualquer reconstrução.
    pub expected_overrides: u64,
    /// Quantidade declarada de regras de exclusão, pelo mesmo motivo.
    pub expected_exclusions: u64,
    /// Regras em ordem canônica.
    pub rules: Vec<Rule>,
}

impl ProjectionSnapshot {
    /// Path repo-relativo canônico do arquivo deste snapshot.
    ///
    /// Não abre nem escreve nada: apenas deriva o nome a partir do ID já
    /// validado. A escrita pertence a etapas posteriores.
    pub fn relative_path(&self) -> String {
        format!("{}{}.toml", SNAPSHOTS_DIR, self.id)
    }
}

/// Diretório repo-relativo canônico dos snapshots.
pub const SNAPSHOTS_DIR: &str = ".pinker/projections/";
// @pinker-nav:end trama.snapshots.modelo

// @pinker-nav:start trama.snapshots.erros
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Taxonomia fechada de falhas de harness dos snapshots históricos: erros de sintaxe TOML, violações estruturais do schema, identificadores e paths inseguros, e todas as formas de consumo incorreto de regras de reconstrução — nenhuma delas pode ser reclassificada como drift.

/// Erro de sintaxe do subconjunto TOML aceito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for TomlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "linha {}: {}", self.line, self.msg)
    }
}

/// Falha fatal de harness. Nunca é drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessFailure {
    /// Sintaxe TOML fora do subconjunto aceito.
    Toml(TomlError),
    /// Campo obrigatório ausente.
    MissingField { field: String },
    /// Campo presente com valor estruturalmente inválido.
    InvalidField { field: String, msg: String },
    /// `schema` ausente ou diferente de [`SNAPSHOT_SCHEMA`].
    SchemaUnknown { found: u64 },
    /// Identificador que não pode ser usado com segurança como nome de arquivo.
    IdUnsafe { field: String, value: String },
    /// Estado fora de `FROZEN`/`CANDIDATE`.
    StateUnknown { value: String },
    /// Hash fora do formato `fnv1a64:` + 16 hexadecimais minúsculos.
    HashInvalid { field: String, value: String },
    /// Predecessor idêntico ao próprio ID.
    PredecessorSelfReference { id: String },
    /// Path absoluto onde só se aceita path repo-relativo.
    PathAbsolute { field: String, value: String },
    /// Travessia (`..`) em path repo-relativo.
    PathTraversal { field: String, value: String },
    /// Regra sem operação declarada.
    RuleWithoutOperation { index: usize },
    /// Operação de regra desconhecida.
    RuleOperationUnknown { index: usize, op: String },
    /// Regra sem o seletor exigido pela operação.
    RuleWithoutSelector { index: usize, op: String },
    /// `expected_overrides` maior que a quantidade de regras `override-hash`.
    OverrideMissing { declared: u64, found: u64 },
    /// `expected_overrides` menor que a quantidade de regras `override-hash`.
    OverrideExcess { declared: u64, found: u64 },
    /// Duas regras `override-hash` para a mesma chave.
    OverrideRepeated { key: String },
    /// `expected_exclusions` maior que a quantidade de regras de exclusão.
    ExclusionMissing { declared: u64, found: u64 },
    /// `expected_exclusions` menor que a quantidade de regras de exclusão.
    ExclusionExcess { declared: u64, found: u64 },
    /// Duas regras de exclusão com o mesmo seletor.
    ExclusionRepeated { selector: String },
    /// Override cuja chave não existe mais no catálogo.
    RegionRemoved { key: String },
    /// Override cuja região sobreviveu com outra chave.
    KeyChanged { expected: String, found: String },
    /// Seletor de override com mais de uma correspondência.
    SelectorAmbiguous { key: String, matches: usize },
    /// Override que não foi consumido (nenhuma correspondência aplicada).
    OverrideNotConsumed { key: String },
    /// Path da região divergente da expectativa declarada.
    PathChanged {
        key: String,
        expected: String,
        found: String,
    },
    /// Metadata da região divergente da expectativa declarada.
    MetadataChanged {
        key: String,
        field: String,
        expected: String,
        found: String,
    },
    /// Hash corrente da região divergente do `from` declarado.
    OverrideStaleBase {
        key: String,
        expected: String,
        found: String,
    },
    /// Exclusão sem nenhuma correspondência.
    ExclusionNoMatch { selector: String },
    /// Exclusão que consumiu quantidade diferente da declarada.
    ExclusionPartiallyConsumed {
        selector: String,
        expected: u64,
        consumed: u64,
    },
}

impl HarnessFailure {
    /// Código estável da falha, usado nos relatórios JSON e humano.
    pub fn code(&self) -> &'static str {
        match self {
            HarnessFailure::Toml(_) => "E-SNAP-TOML",
            HarnessFailure::MissingField { .. } => "E-SNAP-CAMPO-AUSENTE",
            HarnessFailure::InvalidField { .. } => "E-SNAP-CAMPO-INVALIDO",
            HarnessFailure::SchemaUnknown { .. } => "E-SNAP-SCHEMA",
            HarnessFailure::IdUnsafe { .. } => "E-SNAP-ID",
            HarnessFailure::StateUnknown { .. } => "E-SNAP-ESTADO",
            HarnessFailure::HashInvalid { .. } => "E-SNAP-HASH",
            HarnessFailure::PredecessorSelfReference { .. } => "E-SNAP-PREDECESSOR",
            HarnessFailure::PathAbsolute { .. } => "E-SNAP-PATH-ABSOLUTO",
            HarnessFailure::PathTraversal { .. } => "E-SNAP-PATH-TRAVESSIA",
            HarnessFailure::RuleWithoutOperation { .. } => "E-SNAP-REGRA-SEM-OPERACAO",
            HarnessFailure::RuleOperationUnknown { .. } => "E-SNAP-REGRA-OPERACAO",
            HarnessFailure::RuleWithoutSelector { .. } => "E-SNAP-REGRA-SEM-SELETOR",
            HarnessFailure::OverrideMissing { .. } => "E-SNAP-OVERRIDE-AUSENTE",
            HarnessFailure::OverrideExcess { .. } => "E-SNAP-OVERRIDE-EXCEDENTE",
            HarnessFailure::OverrideRepeated { .. } => "E-SNAP-OVERRIDE-REPETIDO",
            HarnessFailure::ExclusionMissing { .. } => "E-SNAP-EXCLUSAO-AUSENTE",
            HarnessFailure::ExclusionExcess { .. } => "E-SNAP-EXCLUSAO-EXCEDENTE",
            HarnessFailure::ExclusionRepeated { .. } => "E-SNAP-EXCLUSAO-REPETIDA",
            HarnessFailure::RegionRemoved { .. } => "E-SNAP-REGIAO-REMOVIDA",
            HarnessFailure::KeyChanged { .. } => "E-SNAP-KEY-ALTERADA",
            HarnessFailure::SelectorAmbiguous { .. } => "E-SNAP-SELETOR-AMBIGUO",
            HarnessFailure::OverrideNotConsumed { .. } => "E-SNAP-OVERRIDE-NAO-CONSUMIDO",
            HarnessFailure::PathChanged { .. } => "E-SNAP-PATH-ALTERADO",
            HarnessFailure::MetadataChanged { .. } => "E-SNAP-METADATA-ALTERADA",
            HarnessFailure::OverrideStaleBase { .. } => "E-SNAP-OVERRIDE-BASE",
            HarnessFailure::ExclusionNoMatch { .. } => "E-SNAP-EXCLUSAO-SEM-CORRESPONDENCIA",
            HarnessFailure::ExclusionPartiallyConsumed { .. } => "E-SNAP-EXCLUSAO-PARCIAL",
        }
    }
}

impl fmt::Display for HarnessFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.code())?;
        match self {
            HarnessFailure::Toml(err) => write!(f, "TOML fora do subconjunto aceito: {}", err),
            HarnessFailure::MissingField { field } => {
                write!(f, "campo obrigatório ausente: {}", field)
            }
            HarnessFailure::InvalidField { field, msg } => {
                write!(f, "campo '{}' inválido: {}", field, msg)
            }
            HarnessFailure::SchemaUnknown { found } => write!(
                f,
                "schema {} desconhecido; esta versão aceita somente schema {}",
                found, SNAPSHOT_SCHEMA
            ),
            HarnessFailure::IdUnsafe { field, value } => write!(
                f,
                "identificador inseguro em '{}': '{}' (use [a-z0-9] separados por '.', '-' ou '_', até {} caracteres)",
                field, value, MAX_ID_LEN
            ),
            HarnessFailure::StateUnknown { value } => write!(
                f,
                "estado '{}' desconhecido; aceitos: FROZEN, CANDIDATE",
                value
            ),
            HarnessFailure::HashInvalid { field, value } => write!(
                f,
                "hash inválido em '{}': '{}' (esperado {}<16 hexadecimais minúsculos>)",
                field, value, FNV_PREFIX
            ),
            HarnessFailure::PredecessorSelfReference { id } => write!(
                f,
                "predecessor igual ao próprio identificador '{}'",
                id
            ),
            HarnessFailure::PathAbsolute { field, value } => write!(
                f,
                "path absoluto em '{}': '{}' (esperado path repo-relativo)",
                field, value
            ),
            HarnessFailure::PathTraversal { field, value } => write!(
                f,
                "travessia de path em '{}': '{}'",
                field, value
            ),
            HarnessFailure::RuleWithoutOperation { index } => {
                write!(f, "regra {} sem campo 'op'", index)
            }
            HarnessFailure::RuleOperationUnknown { index, op } => write!(
                f,
                "regra {} com operação desconhecida '{}'",
                index, op
            ),
            HarnessFailure::RuleWithoutSelector { index, op } => write!(
                f,
                "regra {} da operação '{}' sem seletor",
                index, op
            ),
            HarnessFailure::OverrideMissing { declared, found } => write!(
                f,
                "override ausente: expected_overrides = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::OverrideExcess { declared, found } => write!(
                f,
                "override excedente: expected_overrides = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::OverrideRepeated { key } => {
                write!(f, "override repetido para a chave '{}'", key)
            }
            HarnessFailure::ExclusionMissing { declared, found } => write!(
                f,
                "exclusão ausente: expected_exclusions = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::ExclusionExcess { declared, found } => write!(
                f,
                "exclusão excedente: expected_exclusions = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::ExclusionRepeated { selector } => {
                write!(f, "exclusão repetida para o seletor '{}'", selector)
            }
            HarnessFailure::RegionRemoved { key } => write!(
                f,
                "região '{}' removida do catálogo: o override não tem correspondência",
                key
            ),
            HarnessFailure::KeyChanged { expected, found } => write!(
                f,
                "key alterada: o override esperava '{}' e a região correspondente agora é '{}'",
                expected, found
            ),
            HarnessFailure::SelectorAmbiguous { key, matches } => write!(
                f,
                "seletor ambíguo: '{}' corresponde a {} regiões",
                key, matches
            ),
            HarnessFailure::OverrideNotConsumed { key } => {
                write!(f, "override '{}' não consumido", key)
            }
            HarnessFailure::PathChanged {
                key,
                expected,
                found,
            } => write!(
                f,
                "path alterado na região '{}': esperado '{}', encontrado '{}'",
                key, expected, found
            ),
            HarnessFailure::MetadataChanged {
                key,
                field,
                expected,
                found,
            } => write!(
                f,
                "metadata alterada na região '{}': '{}' esperado '{}', encontrado '{}'",
                key, field, expected, found
            ),
            HarnessFailure::OverrideStaleBase {
                key,
                expected,
                found,
            } => write!(
                f,
                "base do override '{}' divergente: esperado '{}', encontrado '{}'",
                key, expected, found
            ),
            HarnessFailure::ExclusionNoMatch { selector } => write!(
                f,
                "exclusão sem correspondência para o seletor '{}'",
                selector
            ),
            HarnessFailure::ExclusionPartiallyConsumed {
                selector,
                expected,
                consumed,
            } => write!(
                f,
                "exclusão parcialmente consumida em '{}': esperado {}, consumido {}",
                selector, expected, consumed
            ),
        }
    }
}
// @pinker-nav:end trama.snapshots.erros

// @pinker-nav:start trama.snapshots.medidas
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary FNV-1a 64 sobre bytes e projeção estável canônica de regiões: a forma exata (tupla Debug com schema, key, kind, domain, layer, file, summary, hash e status, uma por linha, ordenada lexicograficamente) que define o comprimento e o hash de toda medida histórica da cartografia.

/// FNV-1a 64 sobre bytes.
///
/// `src/nav.rs` tem uma implementação privada equivalente, mas ela recebe `&str`
/// e devolve a forma prefixada usada no hash **de região**. A medida histórica
/// de uma projeção precisa do `u64` cru sobre bytes para comparar com os
/// literais atuais da cartografia, e promover a versão de `nav` a pública
/// mudaria a autoridade de outro domínio para caber neste. Esta função é a
/// abstração menor e explícita exigida por esse caso; a duplicação é declarada,
/// não silenciosa.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Forma canônica do hash de uma projeção.
pub fn fnv1a64_canonical(bytes: &[u8]) -> String {
    format!("{}{:016x}", FNV_PREFIX, fnv1a64(bytes))
}

/// Projeção estável de um conjunto de regiões.
///
/// Esta é a forma que **define** as medidas históricas: um registro por região,
/// contendo apenas os campos estáveis (schema, key, kind, domain, layer, file,
/// summary, hash, status), ordenado lexicograficamente e concatenado. Posições
/// de linha ficam de fora de propósito: elas mudam com edições irrelevantes.
///
/// A forma é independente do root absoluto porque `CodeRegion::file` já é
/// repo-relativo. Nenhum estado externo — tempo, PID, usuário, locale, endereço
/// de memória ou iteração de `HashMap` — participa do resultado.
pub fn stable_projection<'a>(regions: impl Iterator<Item = &'a CodeRegion>) -> String {
    let mut records: Vec<String> = regions
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
    records.concat()
}

/// Mede um conjunto de regiões, produzindo as três medidas canônicas.
pub fn measure<'a>(regions: impl Iterator<Item = &'a CodeRegion>) -> Measures {
    let collected: Vec<&CodeRegion> = regions.collect();
    let projection = stable_projection(collected.iter().copied());
    Measures {
        regions: collected.len() as u64,
        length: projection.len() as u64,
        fnv1a64: fnv1a64(projection.as_bytes()),
    }
}
// @pinker-nav:end trama.snapshots.medidas

// @pinker-nav:start trama.snapshots.parser
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Parser TOML estrito do snapshot: aceita apenas tabelas conhecidas, rejeita chave desconhecida, chave duplicada, seção duplicada, string incompleta, escape não suportado, dado residual após o valor, número negativo e overflow, e aplica em seguida toda a validação estrutural e semântica do schema.

/// Valor escalar aceito pelo subconjunto TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Scalar {
    Text(String),
    Integer(u64),
}

/// Uma tabela em construção: pares na ordem de aparição, com detecção de
/// duplicidade.
#[derive(Debug, Default)]
struct Table {
    pairs: Vec<(String, Scalar, usize)>,
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

    fn get(&self, key: &str) -> Option<&Scalar> {
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
struct RawDocument {
    root: Table,
    reconstruction: Option<Table>,
    measures: Option<Table>,
    rules: Vec<Table>,
}

/// Interpreta o texto de um snapshot e valida o schema por inteiro.
///
/// Não toca no filesystem: recebe o conteúdo já em memória.
pub fn parse(text: &str) -> Result<ProjectionSnapshot, HarnessFailure> {
    let raw = parse_raw(text).map_err(HarnessFailure::Toml)?;
    build(raw)
}

fn parse_raw(text: &str) -> Result<RawDocument, TomlError> {
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
const RECONSTRUCTION_KEYS: [&str; 2] = ["expected_overrides", "expected_exclusions"];
const MEASURES_KEYS: [&str; 3] = ["regions", "length", "fnv1a64"];
const RULE_KEYS: [&str; 9] = [
    "op",
    "key",
    "from",
    "to",
    "expect_file",
    "expect_domain",
    "expect_layer",
    "prefix",
    "expected_matches",
];

fn reject_unknown(table: &Table, allowed: &[&str], scope: &str) -> Result<(), HarnessFailure> {
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

fn require_text(table: &Table, key: &str, scope: &str) -> Result<String, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Text(value)) => Ok(value.clone()),
        Some(Scalar::Integer(_)) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado texto entre aspas".to_string(),
        }),
        None => Err(HarnessFailure::MissingField {
            field: format!("{}{}", scope, key),
        }),
    }
}

fn optional_text(table: &Table, key: &str, scope: &str) -> Result<Option<String>, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Text(value)) => Ok(Some(value.clone())),
        Some(Scalar::Integer(_)) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado texto entre aspas".to_string(),
        }),
        None => Ok(None),
    }
}

fn require_integer(table: &Table, key: &str, scope: &str) -> Result<u64, HarnessFailure> {
    match table.get(key) {
        Some(Scalar::Integer(value)) => Ok(*value),
        Some(Scalar::Text(_)) => Err(HarnessFailure::InvalidField {
            field: format!("{}{}", scope, key),
            msg: "esperado inteiro, não texto".to_string(),
        }),
        None => Err(HarnessFailure::MissingField {
            field: format!("{}{}", scope, key),
        }),
    }
}

/// Um identificador é seguro quando pode virar nome de arquivo sem ambiguidade.
fn validate_id(value: &str, field: &str) -> Result<(), HarnessFailure> {
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

fn build(raw: RawDocument) -> Result<ProjectionSnapshot, HarnessFailure> {
    reject_unknown(&raw.root, &ROOT_KEYS, "")?;

    let schema = match raw.root.get("schema") {
        Some(Scalar::Integer(value)) => *value,
        Some(Scalar::Text(_)) => {
            return Err(HarnessFailure::InvalidField {
                field: "schema".to_string(),
                msg: "esperado inteiro, não texto".to_string(),
            })
        }
        None => return Err(HarnessFailure::SchemaUnknown { found: 0 }),
    };
    if schema != SNAPSHOT_SCHEMA {
        return Err(HarnessFailure::SchemaUnknown { found: schema });
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

    let mut rules = Vec::with_capacity(raw.rules.len());
    for (index, table) in raw.rules.iter().enumerate() {
        rules.push(build_rule(table, index)?);
    }

    let found_overrides = rules.iter().filter(|rule| rule.is_override()).count() as u64;
    let found_exclusions = rules.len() as u64 - found_overrides;
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
            if !rule.is_override() && !other.is_override() && rule.op() == other.op() {
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
        measures: Measures {
            regions,
            length,
            fnv1a64,
        },
        expected_overrides,
        expected_exclusions,
        rules,
    })
}

fn sort_rules(rules: &mut [Rule]) {
    rules.sort_by(|a, b| {
        a.op_rank()
            .cmp(&b.op_rank())
            .then_with(|| a.selector().cmp(b.selector()))
    });
}

fn build_rule(table: &Table, index: usize) -> Result<Rule, HarnessFailure> {
    reject_unknown(table, &RULE_KEYS, &format!("rules[{}].", index))?;
    let scope = format!("rules[{}].", index);

    let op = match table.get("op") {
        Some(Scalar::Text(value)) => value.clone(),
        Some(Scalar::Integer(_)) => {
            return Err(HarnessFailure::InvalidField {
                field: format!("{}op", scope),
                msg: "esperado texto entre aspas".to_string(),
            })
        }
        None => return Err(HarnessFailure::RuleWithoutOperation { index }),
    };

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
            if table.get("prefix").is_some() || table.get("expected_matches").is_some() {
                return Err(HarnessFailure::InvalidField {
                    field: format!("{}op", scope),
                    msg: "'override-hash' não aceita 'prefix' nem 'expected_matches'".to_string(),
                });
            }
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
        other => Err(HarnessFailure::RuleOperationUnknown {
            index,
            op: other.to_string(),
        }),
    }
}
// @pinker-nav:end trama.snapshots.parser

// @pinker-nav:start trama.snapshots.renderizacao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Renderer TOML canônico: ordem fixa de campos e seções, regras ordenadas por operação e seletor, escaping mínimo e determinístico, sem qualquer dependência de root absoluto, PID, usuário, locale, tempo, HashMap ou endereço de memória — a saída é função apenas do modelo.

/// Escapa um texto para string básica TOML.
///
/// Cobre exatamente os escapes que o parser aceita, de modo que
/// `parse(render(x)) == x` para todo modelo válido.
fn toml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Serializa um snapshot na forma canônica.
///
/// A saída é estável: mesma entrada, mesmos bytes, em qualquer máquina, root,
/// usuário ou momento.
pub fn render(snapshot: &ProjectionSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!("schema = {}\n", snapshot.schema));
    out.push_str(&format!("id = {}\n", toml_escape(&snapshot.id)));
    out.push_str(&format!(
        "state = {}\n",
        toml_escape(snapshot.state.as_str())
    ));
    if let Some(predecessor) = &snapshot.predecessor {
        out.push_str(&format!("predecessor = {}\n", toml_escape(predecessor)));
    }
    if let Some(justification) = &snapshot.justification {
        out.push_str(&format!("justification = {}\n", toml_escape(justification)));
    }

    out.push_str("\n[reconstruction]\n");
    out.push_str(&format!(
        "expected_overrides = {}\n",
        snapshot.expected_overrides
    ));
    out.push_str(&format!(
        "expected_exclusions = {}\n",
        snapshot.expected_exclusions
    ));

    out.push_str("\n[measures]\n");
    out.push_str(&format!("regions = {}\n", snapshot.measures.regions));
    out.push_str(&format!("length = {}\n", snapshot.measures.length));
    out.push_str(&format!(
        "fnv1a64 = {}\n",
        toml_escape(&snapshot.measures.fnv1a64_canonical())
    ));

    let mut rules = snapshot.rules.clone();
    sort_rules(&mut rules);
    for rule in &rules {
        out.push_str("\n[[rules]]\n");
        out.push_str(&format!("op = {}\n", toml_escape(rule.op())));
        match rule {
            Rule::OverrideHash {
                key,
                from,
                to,
                expect_file,
                expect_domain,
                expect_layer,
            } => {
                out.push_str(&format!("key = {}\n", toml_escape(key)));
                out.push_str(&format!("from = {}\n", toml_escape(from)));
                out.push_str(&format!("to = {}\n", toml_escape(to)));
                if let Some(file) = expect_file {
                    out.push_str(&format!("expect_file = {}\n", toml_escape(file)));
                }
                if let Some(domain) = expect_domain {
                    out.push_str(&format!("expect_domain = {}\n", toml_escape(domain)));
                }
                if let Some(layer) = expect_layer {
                    out.push_str(&format!("expect_layer = {}\n", toml_escape(layer)));
                }
            }
            Rule::ExcludeKey {
                key,
                expected_matches,
            } => {
                out.push_str(&format!("key = {}\n", toml_escape(key)));
                out.push_str(&format!("expected_matches = {}\n", expected_matches));
            }
            Rule::ExcludeFilePrefix {
                prefix,
                expected_matches,
            } => {
                out.push_str(&format!("prefix = {}\n", toml_escape(prefix)));
                out.push_str(&format!("expected_matches = {}\n", expected_matches));
            }
        }
    }

    out
}
// @pinker-nav:end trama.snapshots.renderizacao

// @pinker-nav:start trama.snapshots.reconstrucao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Reconstrução pura do estado histórico a partir do catálogo corrente, com livro de consumo por regra: exclusões consomem exatamente o orçamento declarado e ao menos uma correspondência, overrides consomem exatamente uma, e ausência, excedente, ambiguidade, key alterada, path alterado, metadata alterada ou base divergente falham como harness, nunca como drift.

/// Consumo efetivo de uma regra durante a reconstrução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConsumption {
    pub op: &'static str,
    pub selector: String,
    pub expected: u64,
    pub consumed: u64,
}

/// Resultado de uma reconstrução bem-sucedida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconstruction {
    pub regions: Vec<CodeRegion>,
    pub ledger: Vec<RuleConsumption>,
}

impl Reconstruction {
    /// Medidas do estado reconstruído.
    pub fn measures(&self) -> Measures {
        measure(self.regions.iter())
    }
}

/// Reconstrói o estado histórico aplicando as regras ao catálogo corrente.
///
/// Função pura: não lê arquivos, não consulta relógio nem ambiente e não altera
/// a entrada. A ordem de aplicação é sempre exclusões e depois overrides,
/// independentemente da ordem textual das regras.
pub fn reconstruct(
    base: &[CodeRegion],
    snapshot: &ProjectionSnapshot,
) -> Result<Reconstruction, HarnessFailure> {
    let mut regions: Vec<CodeRegion> = base.to_vec();
    let mut ledger: Vec<RuleConsumption> = Vec::with_capacity(snapshot.rules.len());
    let mut rules = snapshot.rules.clone();
    sort_rules(&mut rules);

    for rule in &rules {
        match rule {
            Rule::ExcludeKey {
                key,
                expected_matches,
            } => {
                let consumed = regions.iter().filter(|region| &region.key == key).count() as u64;
                if consumed == 0 {
                    return Err(HarnessFailure::ExclusionNoMatch {
                        selector: key.clone(),
                    });
                }
                if consumed != *expected_matches {
                    return Err(HarnessFailure::ExclusionPartiallyConsumed {
                        selector: key.clone(),
                        expected: *expected_matches,
                        consumed,
                    });
                }
                regions.retain(|region| &region.key != key);
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: key.clone(),
                    expected: *expected_matches,
                    consumed,
                });
            }
            Rule::ExcludeFilePrefix {
                prefix,
                expected_matches,
            } => {
                let consumed = regions
                    .iter()
                    .filter(|region| region.file.starts_with(prefix.as_str()))
                    .count() as u64;
                if consumed == 0 {
                    return Err(HarnessFailure::ExclusionNoMatch {
                        selector: prefix.clone(),
                    });
                }
                if consumed != *expected_matches {
                    return Err(HarnessFailure::ExclusionPartiallyConsumed {
                        selector: prefix.clone(),
                        expected: *expected_matches,
                        consumed,
                    });
                }
                regions.retain(|region| !region.file.starts_with(prefix.as_str()));
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: prefix.clone(),
                    expected: *expected_matches,
                    consumed,
                });
            }
            Rule::OverrideHash {
                key,
                from,
                to,
                expect_file,
                expect_domain,
                expect_layer,
            } => {
                let matches: Vec<usize> = regions
                    .iter()
                    .enumerate()
                    .filter(|(_, region)| &region.key == key)
                    .map(|(index, _)| index)
                    .collect();
                if matches.is_empty() {
                    return Err(missing_override_failure(
                        &regions,
                        key,
                        expect_file.as_deref(),
                        expect_domain.as_deref(),
                        expect_layer.as_deref(),
                    ));
                }
                if matches.len() > 1 {
                    return Err(HarnessFailure::SelectorAmbiguous {
                        key: key.clone(),
                        matches: matches.len(),
                    });
                }
                let region = &mut regions[matches[0]];
                if let Some(expected) = expect_file {
                    if &region.file != expected {
                        return Err(HarnessFailure::PathChanged {
                            key: key.clone(),
                            expected: expected.clone(),
                            found: region.file.clone(),
                        });
                    }
                }
                if let Some(expected) = expect_domain {
                    let found = region.domain.clone().unwrap_or_default();
                    if &found != expected {
                        return Err(HarnessFailure::MetadataChanged {
                            key: key.clone(),
                            field: "domain".to_string(),
                            expected: expected.clone(),
                            found,
                        });
                    }
                }
                if let Some(expected) = expect_layer {
                    let found = region.layer.clone().unwrap_or_default();
                    if &found != expected {
                        return Err(HarnessFailure::MetadataChanged {
                            key: key.clone(),
                            field: "layer".to_string(),
                            expected: expected.clone(),
                            found,
                        });
                    }
                }
                if &region.hash != from {
                    return Err(HarnessFailure::OverrideStaleBase {
                        key: key.clone(),
                        expected: from.clone(),
                        found: region.hash.clone(),
                    });
                }
                region.hash.clone_from(to);
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: key.clone(),
                    expected: 1,
                    consumed: 1,
                });
            }
        }
    }

    for entry in &ledger {
        if entry.consumed == 0 {
            return Err(HarnessFailure::OverrideNotConsumed {
                key: entry.selector.clone(),
            });
        }
    }

    Ok(Reconstruction { regions, ledger })
}

/// Distingue "região removida" de "key alterada" quando um override não
/// encontra correspondência.
///
/// Se a regra declarou path e metadata e existe exatamente uma região com essa
/// identidade sob outra chave, a causa é key alterada. Caso contrário, a região
/// foi removida.
fn missing_override_failure(
    regions: &[CodeRegion],
    key: &str,
    expect_file: Option<&str>,
    expect_domain: Option<&str>,
    expect_layer: Option<&str>,
) -> HarnessFailure {
    if let Some(file) = expect_file {
        let candidates: Vec<&CodeRegion> = regions
            .iter()
            .filter(|region| {
                region.file == file
                    && expect_domain.map_or(true, |d| region.domain.as_deref() == Some(d))
                    && expect_layer.map_or(true, |l| region.layer.as_deref() == Some(l))
            })
            .collect();
        if candidates.len() == 1 {
            return HarnessFailure::KeyChanged {
                expected: key.to_string(),
                found: candidates[0].key.clone(),
            };
        }
    }
    HarnessFailure::RegionRemoved {
        key: key.to_string(),
    }
}
// @pinker-nav:end trama.snapshots.reconstrucao

// @pinker-nav:start trama.snapshots.verificacao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Verificação somente leitura de um snapshot contra o catálogo corrente, produzindo MATCH, DRIFT com a lista exata de medidas divergentes, ou HARNESS_FAILURE — drift só existe depois de reconstrução válida e falha de harness jamais é reclassificada.

/// Resultado tipado de uma verificação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Match,
    Drift(Vec<Divergence>),
    HarnessFailure(HarnessFailure),
}

impl Outcome {
    /// Nome canônico do resultado.
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Match => "MATCH",
            Outcome::Drift(_) => "DRIFT",
            Outcome::HarnessFailure(_) => "HARNESS_FAILURE",
        }
    }
}

/// Uma medida divergente entre o snapshot e o estado reconstruído.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub measure: &'static str,
    pub expected: String,
    pub observed: String,
}

/// Relatório completo de uma verificação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub snapshot_id: String,
    pub state: SnapshotState,
    pub predecessor: Option<String>,
    pub expected: Measures,
    /// Ausente quando a reconstrução falhou: sem reconstrução válida não há
    /// medida observada e, portanto, não pode haver drift.
    pub observed: Option<Measures>,
    pub outcome: Outcome,
    pub ledger: Vec<RuleConsumption>,
}

/// Verifica um snapshot contra um catálogo corrente. Somente leitura e pura.
pub fn verify(snapshot: &ProjectionSnapshot, base: &[CodeRegion]) -> VerifyReport {
    match reconstruct(base, snapshot) {
        Err(failure) => VerifyReport {
            snapshot_id: snapshot.id.clone(),
            state: snapshot.state,
            predecessor: snapshot.predecessor.clone(),
            expected: snapshot.measures,
            observed: None,
            outcome: Outcome::HarnessFailure(failure),
            ledger: Vec::new(),
        },
        Ok(reconstruction) => {
            let observed = reconstruction.measures();
            let mut divergences = Vec::new();
            if observed.regions != snapshot.measures.regions {
                divergences.push(Divergence {
                    measure: "regions",
                    expected: snapshot.measures.regions.to_string(),
                    observed: observed.regions.to_string(),
                });
            }
            if observed.length != snapshot.measures.length {
                divergences.push(Divergence {
                    measure: "length",
                    expected: snapshot.measures.length.to_string(),
                    observed: observed.length.to_string(),
                });
            }
            if observed.fnv1a64 != snapshot.measures.fnv1a64 {
                divergences.push(Divergence {
                    measure: "fnv1a64",
                    expected: snapshot.measures.fnv1a64_canonical(),
                    observed: observed.fnv1a64_canonical(),
                });
            }
            let outcome = if divergences.is_empty() {
                Outcome::Match
            } else {
                Outcome::Drift(divergences)
            };
            VerifyReport {
                snapshot_id: snapshot.id.clone(),
                state: snapshot.state,
                predecessor: snapshot.predecessor.clone(),
                expected: snapshot.measures,
                observed: Some(observed),
                outcome,
                ledger: reconstruction.ledger,
            }
        }
    }
}
// @pinker-nav:end trama.snapshots.verificacao

// @pinker-nav:start trama.snapshots.relatorio
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Relatórios determinísticos derivados do mesmo modelo de verificação: texto humano sem códigos ANSI e JSON de uma linha com ordem fixa de chaves, escaping explícito e nenhum path absoluto, PID, usuário, locale ou tempo.

/// Escapa um texto para string JSON.
///
/// As quatro implementações existentes no repositório (`src/nav.rs`,
/// `src/main.rs`, `src/doc_index.rs`, `src/change.rs`) são todas privadas dos
/// seus módulos: não há autoridade pública a reutilizar. Promover uma delas
/// mudaria a superfície de outro domínio para acomodar este, o que o contrato da
/// campanha proíbe; esta cópia é declarada e coberta por teste de escaping.
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
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn measures_json(measures: &Measures) -> String {
    format!(
        "{{\"regions\":{},\"length\":{},\"fnv1a64\":{}}}",
        measures.regions,
        measures.length,
        json_string(&measures.fnv1a64_canonical())
    )
}

/// Relatório JSON determinístico de uma verificação.
///
/// Uma linha, ordem de chaves fixa, sem códigos ANSI e sem qualquer path
/// absoluto: o modelo só carrega identificadores e paths repo-relativos.
pub fn json_report(report: &VerifyReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", SNAPSHOT_SCHEMA));
    out.push_str(&format!(
        ",\"snapshot\":{}",
        json_string(&report.snapshot_id)
    ));
    out.push_str(&format!(
        ",\"state\":{}",
        json_string(report.state.as_str())
    ));
    match &report.predecessor {
        Some(predecessor) => {
            out.push_str(&format!(",\"predecessor\":{}", json_string(predecessor)))
        }
        None => out.push_str(",\"predecessor\":null"),
    }
    out.push_str(&format!(
        ",\"outcome\":{}",
        json_string(report.outcome.as_str())
    ));
    out.push_str(&format!(
        ",\"expected\":{}",
        measures_json(&report.expected)
    ));
    match &report.observed {
        Some(observed) => out.push_str(&format!(",\"observed\":{}", measures_json(observed))),
        None => out.push_str(",\"observed\":null"),
    }

    out.push_str(",\"divergences\":[");
    if let Outcome::Drift(divergences) = &report.outcome {
        for (index, divergence) in divergences.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"measure\":{},\"expected\":{},\"observed\":{}}}",
                json_string(divergence.measure),
                json_string(&divergence.expected),
                json_string(&divergence.observed)
            ));
        }
    }
    out.push(']');

    match &report.outcome {
        Outcome::HarnessFailure(failure) => {
            out.push_str(&format!(
                ",\"failure\":{{\"code\":{},\"message\":{}}}",
                json_string(failure.code()),
                json_string(&failure.to_string().replace('\n', " "))
            ));
        }
        _ => out.push_str(",\"failure\":null"),
    }

    out.push_str(",\"consumption\":[");
    for (index, entry) in report.ledger.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"op\":{},\"selector\":{},\"expected\":{},\"consumed\":{}}}",
            json_string(entry.op),
            json_string(&entry.selector),
            entry.expected,
            entry.consumed
        ));
    }
    out.push_str("]}");
    out
}

/// Relatório humano determinístico de uma verificação. Sem códigos ANSI.
pub fn human_report(report: &VerifyReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "snapshot {} [{}] {}\n",
        report.snapshot_id,
        report.state,
        report.outcome.as_str()
    ));
    match &report.predecessor {
        Some(predecessor) => out.push_str(&format!("predecessor: {}\n", predecessor)),
        None => out.push_str("predecessor: —\n"),
    }
    out.push_str(&format!(
        "esperado: regioes={} comprimento={} {}\n",
        report.expected.regions,
        report.expected.length,
        report.expected.fnv1a64_canonical()
    ));
    match &report.observed {
        Some(observed) => out.push_str(&format!(
            "observado: regioes={} comprimento={} {}\n",
            observed.regions,
            observed.length,
            observed.fnv1a64_canonical()
        )),
        None => out.push_str("observado: — (reconstrucao invalida)\n"),
    }
    match &report.outcome {
        Outcome::Match => {}
        Outcome::Drift(divergences) => {
            for divergence in divergences {
                out.push_str(&format!(
                    "drift {}: esperado {} observado {}\n",
                    divergence.measure, divergence.expected, divergence.observed
                ));
            }
        }
        Outcome::HarnessFailure(failure) => {
            for line in failure.to_string().lines() {
                out.push_str(&format!("harness {}\n", line));
            }
        }
    }
    for entry in &report.ledger {
        out.push_str(&format!(
            "consumo {} {}: {}/{}\n",
            entry.op, entry.selector, entry.consumed, entry.expected
        ));
    }
    out
}
// @pinker-nav:end trama.snapshots.relatorio

#[cfg(test)]
mod tests {
    use super::*;

    fn region(key: &str, file: &str, hash: &str) -> CodeRegion {
        CodeRegion {
            key: key.to_string(),
            kind: "region".to_string(),
            domain: Some("dominio".to_string()),
            layer: Some("camada".to_string()),
            phase: None,
            file: file.to_string(),
            start_marker: 1,
            content_start: 2,
            content_end: 3,
            end_marker: 4,
            summary: format!("Resumo de {}.", key),
            hash: hash.to_string(),
            status: "active".to_string(),
        }
    }

    fn base_catalog() -> Vec<CodeRegion> {
        vec![
            region("a.b.um", "src/um.rs", "fnv1a64:0000000000000001"),
            region("a.b.dois", "src/dois.rs", "fnv1a64:0000000000000002"),
            region("posterior.novo", "src/novo.rs", "fnv1a64:0000000000000003"),
        ]
    }

    const VALID: &str = concat!(
        "schema = 1\n",
        "id = \"exemplo-historico\"\n",
        "state = \"FROZEN\"\n",
        "predecessor = \"exemplo-anterior\"\n",
        "justification = \"fixture sintetica\"\n",
        "\n[reconstruction]\n",
        "expected_overrides = 1\n",
        "expected_exclusions = 1\n",
        "\n[measures]\n",
        "regions = 2\n",
        "length = 0\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
        "\n[[rules]]\n",
        "op = \"exclude-key\"\n",
        "key = \"posterior.novo\"\n",
        "expected_matches = 1\n",
        "\n[[rules]]\n",
        "op = \"override-hash\"\n",
        "key = \"a.b.um\"\n",
        "from = \"fnv1a64:0000000000000001\"\n",
        "to = \"fnv1a64:00000000000000ff\"\n",
    );

    #[test]
    fn parse_aceita_snapshot_valido() {
        let snapshot = parse(VALID).expect("snapshot valido");
        assert_eq!(snapshot.schema, SNAPSHOT_SCHEMA);
        assert_eq!(snapshot.id, "exemplo-historico");
        assert_eq!(snapshot.state, SnapshotState::Frozen);
        assert_eq!(snapshot.predecessor.as_deref(), Some("exemplo-anterior"));
        assert_eq!(snapshot.rules.len(), 2);
        // Ordem canônica: exclusões antes de overrides.
        assert_eq!(snapshot.rules[0].op(), "exclude-key");
        assert_eq!(snapshot.rules[1].op(), "override-hash");
    }

    #[test]
    fn render_e_parse_sao_estaveis() {
        let snapshot = parse(VALID).expect("snapshot valido");
        let rendered = render(&snapshot);
        let reparsed = parse(&rendered).expect("render canônico volta a interpretar");
        assert_eq!(snapshot, reparsed);
        assert_eq!(rendered, render(&reparsed));
    }

    #[test]
    fn reconstrucao_consome_regras_exatamente() {
        let snapshot = parse(VALID).expect("snapshot valido");
        let reconstruction = reconstruct(&base_catalog(), &snapshot).expect("reconstrucao valida");
        assert_eq!(reconstruction.regions.len(), 2);
        assert_eq!(
            reconstruction
                .regions
                .iter()
                .find(|region| region.key == "a.b.um")
                .map(|region| region.hash.as_str()),
            Some("fnv1a64:00000000000000ff")
        );
        assert_eq!(reconstruction.ledger.len(), 2);
        assert!(reconstruction
            .ledger
            .iter()
            .all(|entry| entry.consumed == entry.expected));
    }

    #[test]
    fn medida_e_independente_da_ordem_de_entrada() {
        let mut invertido = base_catalog();
        invertido.reverse();
        assert_eq!(measure(base_catalog().iter()), measure(invertido.iter()));
    }

    #[test]
    fn schema_desconhecido_e_falha_de_harness() {
        let text = VALID.replace("schema = 1", "schema = 2");
        assert!(matches!(
            parse(&text),
            Err(HarnessFailure::SchemaUnknown { found: 2 })
        ));
    }

    #[test]
    fn chave_desconhecida_e_rejeitada() {
        let text = format!("{}extra = 1\n", VALID);
        assert!(matches!(
            parse(&text),
            Err(HarnessFailure::InvalidField { .. })
        ));
    }

    #[test]
    fn chave_duplicada_e_rejeitada() {
        let text = VALID.replace(
            "state = \"FROZEN\"",
            "state = \"FROZEN\"\nstate = \"FROZEN\"",
        );
        assert!(matches!(parse(&text), Err(HarnessFailure::Toml(_))));
    }

    #[test]
    fn hash_invalido_e_rejeitado() {
        let text = VALID.replace("fnv1a64:0000000000000000", "fnv1a64:XYZ");
        assert!(matches!(
            parse(&text),
            Err(HarnessFailure::HashInvalid { .. })
        ));
    }

    #[test]
    fn falha_de_harness_nao_produz_medida_observada() {
        let text = VALID.replace("key = \"a.b.um\"", "key = \"a.b.inexistente\"");
        let snapshot = parse(&text).expect("snapshot valido");
        let report = verify(&snapshot, &base_catalog());
        assert!(matches!(report.outcome, Outcome::HarnessFailure(_)));
        assert!(report.observed.is_none());
    }
}
