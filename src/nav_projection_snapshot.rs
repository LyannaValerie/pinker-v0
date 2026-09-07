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

mod parser;
pub(crate) use parser::{
    build_rule, optional_list, parse_raw, reject_unknown, require_integer, require_text,
    sort_rules, validate_id,
};
pub use parser::{parse, validate_rules};

/// Primeira versão do formato de snapshot: lista plana de regras, sem
/// composição.
pub const SNAPSHOT_SCHEMA_V1: u64 = 1;

/// Segunda versão: acrescenta composição (`base_snapshot` e `recipes`) e as
/// operações `exclude-file` e `exclude-key-prefix`.
///
/// O significado do schema 1 fica preservado: um arquivo que declara
/// `schema = 1` continua sendo lista plana, e usar qualquer capacidade nova
/// nele é falha de harness, não interpretação silenciosa.
pub const SNAPSHOT_SCHEMA_V2: u64 = 2;

/// Terceira versão: acrescenta `override-region`, a restauração atômica de
/// `hash` e `summary` de uma mesma região.
///
/// Existe porque a reconstrução histórica real restaura `summary`, e `summary`
/// participa da projeção estável: sem essa operação, o formato não consegue
/// representar a própria história que deveria migrar.
pub const SNAPSHOT_SCHEMA_V3: u64 = 3;

/// Quarta versão: acrescenta `materialize-region`, a declaração explícita de uma
/// região que existia no estado histórico e não existe mais no catálogo
/// corrente.
///
/// Congelar a história nunca implicou que toda região histórica ficasse eterna
/// no código corrente. Até aqui o formato só sabia **tirar** região posterior e
/// **restaurar campo** de região presente; faltava representar a remoção
/// legítima, que a Issue #384 já exigia entre seus casos mínimos. Sem ela,
/// apagar uma única região do catálogo derrubava toda a cadeia congelada.
pub const SNAPSHOT_SCHEMA_V4: u64 = 4;

/// Quinta versão: acrescenta `to_file` a `override-region`, a restauração do
/// caminho de uma região estável que mudou de arquivo no catálogo corrente.
///
/// Até aqui `file` só podia ser **conferido** (`expect_file`) ou **afirmado por
/// inteiro** (`materialize-region`, que exige que a região não exista mais).
/// Faltava o caso em que a região continua existindo, com a mesma chave
/// estável, e apenas mudou de arquivo: a organização física do repositório
/// evolui, e a projeção congelada precisa continuar dizendo onde a região
/// estava. Sem esta capacidade, mover um arquivo cartografado derrubava toda a
/// cadeia congelada sem remédio representável — a única saída seria editar
/// snapshot `FROZEN`, que é byte-imutável.
///
/// A relocação é restauração de campo, não identidade: a seleção continua sendo
/// exclusivamente pela chave estável, e `expect_file` é a origem declarada,
/// obrigatória, que impede a regra de aceitar um arquivo corrente qualquer.
pub const SNAPSHOT_SCHEMA_V5: u64 = 5;

/// Versão máxima aceita do formato de snapshot, e a versão em que artefatos
/// novos nascem.
///
/// Versão aceita e versão do acervo são coisas distintas: os snapshots já
/// materializados continuam exatamente na versão em que foram congelados, e
/// nenhum deles é reescrito por causa de um bump.
pub const SNAPSHOT_SCHEMA: u64 = SNAPSHOT_SCHEMA_V5;

/// Schema do relatório de verificação, distinto do schema do artefato TOML.
pub const SNAPSHOT_REPORT_SCHEMA: u64 = 1;

/// Prefixo canônico do hash FNV-1a 64 usado pela Trama.
pub const FNV_PREFIX: &str = "fnv1a64:";

/// Comprimento máximo de um identificador de snapshot.
pub const MAX_ID_LEN: usize = 64;

// @pinker-nav:start trama.snapshots.modelo
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Modelo imutável dos snapshots históricos de projeção do catálogo de navegação: schema versionado, ID estável, estados FROZEN/CANDIDATE, medidas (regiões, comprimento, FNV-1a 64 canônico), predecessor opcional, justificativa e regras de reconstrução tipadas com orçamento explícito de consumo, incluindo a materialização de uma região histórica que não existe mais no catálogo corrente, com orçamento próprio e aplicação por último.

/// Qual formato está sendo interpretado.
///
/// Existe porque as duas autoridades têm versões próprias e conjuntos aceitos
/// diferentes: um diagnóstico de schema precisa dizer de qual formato fala, e
/// qual versão aquele formato de fato aceita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaAuthority {
    Snapshot,
    Recipe,
}

impl SchemaAuthority {
    pub fn as_str(&self) -> &'static str {
        match self {
            SchemaAuthority::Snapshot => "snapshot",
            SchemaAuthority::Recipe => "receita",
        }
    }

    /// Versões que este formato suporta, em texto.
    pub fn supported_versions(&self) -> &'static str {
        match self {
            SchemaAuthority::Snapshot => "1, 2, 3, 4 ou 5",
            SchemaAuthority::Recipe => "1, 2 ou 3",
        }
    }

    /// A versão declarada está dentro do que este formato aceita?
    pub fn supports(&self, schema: u64) -> bool {
        match self {
            SchemaAuthority::Snapshot => {
                (SNAPSHOT_SCHEMA_V1..=SNAPSHOT_SCHEMA_V5).contains(&schema)
            }
            SchemaAuthority::Recipe => (1..=3).contains(&schema),
        }
    }

    /// Código estável do erro de schema deste formato.
    pub fn schema_error_code(&self) -> &'static str {
        match self {
            SchemaAuthority::Snapshot => "E-SNAP-SCHEMA",
            SchemaAuthority::Recipe => "E-RECEITA-SCHEMA",
        }
    }
}

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
    /// Remove todas as regiões de um arquivo exato. Schema 2 em diante.
    ExcludeFile { file: String, expected_matches: u64 },
    /// Remove todas as regiões cuja chave começa pelo prefixo. Schema 2 em
    /// diante.
    ExcludeKeyPrefix {
        prefix: String,
        expected_matches: u64,
    },
    /// Restauração atômica de uma única região, selecionada por `key`.
    ///
    /// Restaura `hash`, `summary`, `file`, ou qualquer combinação deles — e
    /// nada além disso. Cada par `from`/`to` é individualmente opcional, mas ao
    /// menos um par completo precisa existir, e meio par é inválido.
    ///
    /// A relocação usa `expect_file` como origem declarada e `to_file` como
    /// destino histórico: o par é `expect_file`/`to_file` em vez de
    /// `from_file`/`to_file` porque `expect_file` já era, desde o schema 1, a
    /// única forma de dizer "o arquivo corrente desta região é exatamente
    /// este". Duplicá-la sob outro nome criaria duas guardas para o mesmo campo
    /// e uma pergunta sem resposta sobre qual delas manda.
    ///
    /// "Atômica" no sentido lógico da regra: **todas** as precondições — as
    /// expectativas de identidade e todos os `from` declarados — são validadas
    /// antes de qualquer campo ser alterado. Uma regra que restaura dois campos
    /// nunca deixa metade aplicada.
    ///
    /// Conta como **uma** regra de override, independentemente de alterar um ou
    /// dois campos.
    OverrideRegion {
        key: String,
        from_hash: Option<String>,
        to_hash: Option<String>,
        from_summary: Option<String>,
        to_summary: Option<String>,
        expect_file: Option<String>,
        /// Caminho histórico restaurado. Exige `expect_file` como origem.
        to_file: Option<String>,
        expect_domain: Option<String>,
        expect_layer: Option<String>,
    },
    /// Declara uma região que existia no estado histórico e **não existe mais**
    /// no catálogo corrente. Schema 4 em diante, somente em snapshot.
    ///
    /// As demais regras transformam o que o catálogo corrente oferece. Esta é a
    /// única que afirma um fato que o presente não tem mais como fornecer, e por
    /// isso carrega o fato inteiro — exatamente os campos que
    /// [`stable_projection`] lê, nem um a mais. Offsets de linha, símbolos e
    /// `phase` ficam de fora porque não participam da medida e porque uma região
    /// sem código corrente não tem posição de linha nenhuma.
    ///
    /// Aplica-se **por último**, depois de exclusões e overrides. Isso não é
    /// posição conveniente num `match`: é o que torna impossível excluir o que
    /// acabou de ser materializado, ou sobrescrever com override uma região que
    /// só passa a existir aqui.
    ///
    /// Não carrega guarda de valor corrente, por construção: ela declara um fato
    /// sobre um estado que o presente não tem mais como confirmar, e um `from_*`
    /// não teria contra o que casar. A guarda de drift continua sendo dos
    /// overrides, que exigem correspondência. Combinada com `exclude-key` sobre
    /// a mesma chave — a forma de modelar chave reaproveitada por outra região —
    /// a substituição é total e deliberada: quem escreve a regra está afirmando
    /// que a região corrente daquela chave não é a histórica.
    MaterializeRegion {
        key: String,
        kind: String,
        domain: Option<String>,
        layer: Option<String>,
        file: String,
        summary: String,
        hash: String,
        status: String,
    },
}

impl Rule {
    /// Nome canônico da operação.
    pub fn op(&self) -> &'static str {
        match self {
            Rule::OverrideHash { .. } => "override-hash",
            Rule::ExcludeKey { .. } => "exclude-key",
            Rule::ExcludeFilePrefix { .. } => "exclude-file-prefix",
            Rule::ExcludeFile { .. } => "exclude-file",
            Rule::ExcludeKeyPrefix { .. } => "exclude-key-prefix",
            Rule::OverrideRegion { .. } => "override-region",
            Rule::MaterializeRegion { .. } => "materialize-region",
        }
    }

    /// Versão mínima que suporta esta operação **na autoridade indicada**.
    ///
    /// A matriz é por autoridade porque os dois formatos evoluíram em ritmos
    /// diferentes: `exclude-file` e `exclude-key-prefix` chegaram ao snapshot no
    /// schema 2, mas o formato de receita nasceu depois e já as trouxe na
    /// primeira versão.
    ///
    /// | operação | snapshot | receita |
    /// |---|---:|---:|
    /// | `override-hash` | 1 | 1 |
    /// | `exclude-key` | 1 | 1 |
    /// | `exclude-file-prefix` | 1 | 1 |
    /// | `exclude-file` | 2 | 1 |
    /// | `exclude-key-prefix` | 2 | 1 |
    /// | `override-region` | 3 | 2 |
    /// | `override-region` com `to_file` | 5 | 3 |
    /// | `materialize-region` | 4 | — |
    ///
    /// `materialize-region` não existe na autoridade de receita em versão
    /// alguma: uma receita é transformação reutilizável e não tem medida, estado
    /// nem predecessor para responder por um fato histórico. A rejeição é
    /// explícita em [`crate::nav_projection_recipe::parse_recipe`], com
    /// diagnóstico próprio, e não uma versão mínima inalcançável.
    pub fn min_schema(&self, authority: SchemaAuthority) -> u64 {
        match (self, authority) {
            (Rule::OverrideHash { .. }, _) => 1,
            (Rule::ExcludeKey { .. }, _) => 1,
            (Rule::ExcludeFilePrefix { .. }, _) => 1,
            (Rule::ExcludeFile { .. }, SchemaAuthority::Snapshot) => SNAPSHOT_SCHEMA_V2,
            (Rule::ExcludeFile { .. }, SchemaAuthority::Recipe) => 1,
            (Rule::ExcludeKeyPrefix { .. }, SchemaAuthority::Snapshot) => SNAPSHOT_SCHEMA_V2,
            (Rule::ExcludeKeyPrefix { .. }, SchemaAuthority::Recipe) => 1,
            (
                Rule::OverrideRegion {
                    to_file: Some(_), ..
                },
                SchemaAuthority::Snapshot,
            ) => SNAPSHOT_SCHEMA_V5,
            (
                Rule::OverrideRegion {
                    to_file: Some(_), ..
                },
                SchemaAuthority::Recipe,
            ) => 3,
            (Rule::OverrideRegion { .. }, SchemaAuthority::Snapshot) => SNAPSHOT_SCHEMA_V3,
            (Rule::OverrideRegion { .. }, SchemaAuthority::Recipe) => 2,
            (Rule::MaterializeRegion { .. }, _) => SNAPSHOT_SCHEMA_V4,
        }
    }

    /// Seletor textual da regra, usado na ordenação canônica e nos relatórios.
    pub fn selector(&self) -> &str {
        match self {
            Rule::OverrideHash { key, .. } => key.as_str(),
            Rule::ExcludeKey { key, .. } => key.as_str(),
            Rule::ExcludeFilePrefix { prefix, .. } => prefix.as_str(),
            Rule::ExcludeFile { file, .. } => file.as_str(),
            Rule::ExcludeKeyPrefix { prefix, .. } => prefix.as_str(),
            Rule::OverrideRegion { key, .. } => key.as_str(),
            Rule::MaterializeRegion { key, .. } => key.as_str(),
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
            Rule::ExcludeFile {
                expected_matches, ..
            } => *expected_matches,
            Rule::ExcludeKeyPrefix {
                expected_matches, ..
            } => *expected_matches,
            Rule::OverrideRegion { .. } => 1,
            Rule::MaterializeRegion { .. } => 1,
        }
    }

    /// Verdadeiro para as operações que restauram campos de uma região.
    ///
    /// `override-region` conta como **uma** regra de override, independentemente
    /// de restaurar um ou dois campos: o orçamento declarado é por regra, não
    /// por campo.
    pub fn is_override(&self) -> bool {
        matches!(
            self,
            Rule::OverrideHash { .. } | Rule::OverrideRegion { .. }
        )
    }

    /// Verdadeiro para a operação que declara um fato histórico ausente.
    ///
    /// Materialização não é override nem exclusão: ela tem orçamento próprio,
    /// porque contá-la em qualquer um dos outros dois mudaria o significado de
    /// um campo que os artefatos antigos já usam.
    pub fn is_materialization(&self) -> bool {
        matches!(self, Rule::MaterializeRegion { .. })
    }

    /// Ordem canônica entre operações: exclusões antes de overrides, que é
    /// também a ordem de aplicação.
    fn op_rank(&self) -> u8 {
        match self {
            Rule::ExcludeKey { .. } => 0,
            Rule::ExcludeKeyPrefix { .. } => 1,
            Rule::ExcludeFile { .. } => 2,
            Rule::ExcludeFilePrefix { .. } => 3,
            Rule::OverrideHash { .. } => 4,
            Rule::OverrideRegion { .. } => 5,
            Rule::MaterializeRegion { .. } => 6,
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
    /// Quantidade declarada de regras `materialize-region`. Schema 4 em diante.
    ///
    /// Orçamento próprio, e não uma linha a mais em `expected_exclusions`:
    /// materializar não é excluir, e somar as duas mudaria o significado de um
    /// campo que os artefatos antigos já usam. Ausente significa zero, como
    /// `base_snapshot` e `recipes`.
    pub expected_materializations: u64,
    /// Snapshot sobre o qual esta reconstrução se apoia. Schema 2 em diante.
    ///
    /// É a relação de **composição**, distinta de [`ProjectionSnapshot::predecessor`],
    /// que é a relação **histórica**. Elas coincidem em alguns snapshots e
    /// divergem em outros; tratá-las como a mesma coisa perderia a distinção.
    ///
    /// O campo resolve exclusivamente contra snapshots: nunca contra receitas.
    /// A separação é estrutural, e não há resolvedor polimórfico — por isso não
    /// existe falha de base ambígua.
    pub base_snapshot: Option<String>,
    /// Receitas aplicadas **na ordem declarada**, depois da base.
    ///
    /// Resolve exclusivamente contra receitas, nunca contra snapshots. A ordem
    /// é procedural e faz parte do significado: o renderer a preserva.
    pub recipes: Vec<String>,
    /// Regras em ordem canônica.
    pub rules: Vec<Rule>,
}

impl ProjectionSnapshot {
    /// Autoridade e capacidade deste modelo, independentemente de ele ter vindo
    /// de `parse` ou de ter sido construído em memória.
    pub fn validate_model(&self) -> Result<(), HarnessFailure> {
        validate_rules(self.schema, &self.rules, SchemaAuthority::Snapshot)
    }

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
// @pinker-nav:summary Taxonomia fechada de falhas de harness dos snapshots históricos: erros de sintaxe TOML, violações estruturais do schema, identificadores e paths inseguros, todas as formas de consumo incorreto de regras de reconstrução, e as falhas próprias da materialização histórica — colisão com região presente, declaração repetida do mesmo fato, orçamento divergente e operação fora da autoridade — nenhuma delas pode ser reclassificada como drift.

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
    /// `schema` ausente ou fora do conjunto aceito **pela autoridade indicada**.
    SchemaUnknown {
        authority: SchemaAuthority,
        found: u64,
    },
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
    /// Summary corrente da região divergente do `from_summary` declarado.
    OverrideStaleSummary {
        key: String,
        expected: String,
        found: String,
    },
    /// Campo conhecido pela gramática, mas que não pertence a esta operação.
    ///
    /// Distinto de "chave desconhecida": o campo existe no formato, só não nesta
    /// regra. Descartá-lo em silêncio seria aceitar uma declaração que o autor
    /// acredita ter efeito e não tem.
    FieldNotAllowedForOp { op: String, field: String },
    /// `override-region` sem nenhum par completo, ou com meio par.
    OverrideRegionPairInvalid { key: String, msg: String },
    /// Hash corrente da região divergente do `from` declarado.
    OverrideStaleBase {
        key: String,
        expected: String,
        found: String,
    },
    /// Exclusão sem nenhuma correspondência.
    ExclusionNoMatch { selector: String },
    /// Dois snapshots com o mesmo identificador.
    DuplicateSnapshot { id: String },
    /// Duas receitas com o mesmo identificador.
    DuplicateRecipe { id: String },
    /// `base_snapshot` aponta para um snapshot que não existe.
    BaseSnapshotMissing { id: String },
    /// Uma receita referenciada não existe.
    RecipeMissing { id: String },
    /// Ciclo no grafo de composição.
    CompositionCycle { path: String },
    /// A base foi reconstruída e não bate com as próprias medidas congeladas.
    BaseMeasuresDiverged {
        id: String,
        expected: Measures,
        observed: Measures,
    },
    /// Um snapshot congelado depende, direta ou transitivamente, de um candidato.
    FrozenDependsOnCandidate { frozen: String, candidate: String },
    /// Receita declarando campo que pertence exclusivamente a snapshot.
    RecipeHasSnapshotField { field: String },
    /// Capacidade usada num arquivo cuja versão não a suporta.
    ///
    /// Carrega a autoridade porque os dois formatos têm matrizes próprias: a
    /// mesma capacidade pode exigir versões diferentes em snapshot e em receita.
    CapabilityRequiresSchema {
        authority: SchemaAuthority,
        capability: String,
        found_schema: u64,
        required_schema: u64,
    },
    /// Snapshot que declara a si mesmo como base.
    SelfBase { id: String },
    /// Receita que declara a si mesma como passo.
    ///
    /// Separada de [`HarnessFailure::SelfBase`] de propósito: uma receita não
    /// tem base, e dizer que ela "declarou a si mesma como base" descreveria uma
    /// relação que não existe naquela autoridade.
    RecipeSelfStep { id: String },
    /// Exclusão que consumiu quantidade diferente da declarada.
    ExclusionPartiallyConsumed {
        selector: String,
        expected: u64,
        consumed: u64,
    },
    /// `expected_materializations` maior que a quantidade de regras.
    MaterializationMissing { declared: u64, found: u64 },
    /// `expected_materializations` menor que a quantidade de regras.
    MaterializationExcess { declared: u64, found: u64 },
    /// Duas regras materializando a mesma identidade histórica.
    ///
    /// Falha mesmo quando os campos são byte-idênticos: um fato histórico tem
    /// uma autoridade explícita, e duas declarações do mesmo fato deixariam
    /// ambígua qual delas o acervo está afirmando.
    MaterializationRepeated { key: String },
    /// A chave declarada como histórica já existe no estado reconstruído.
    ///
    /// Materializar nunca sobrescreve, nunca funde campos e nunca é ignorada em
    /// silêncio: se a região está presente, a declaração histórica está errada.
    MaterializationCollision { key: String },
    /// Operação declarada numa autoridade que não a possui.
    OperationOutsideAuthority {
        authority: SchemaAuthority,
        op: String,
    },
}

impl HarnessFailure {
    /// Código estável da falha, usado nos relatórios JSON e humano.
    pub fn code(&self) -> &'static str {
        match self {
            HarnessFailure::Toml(_) => "E-SNAP-TOML",
            HarnessFailure::MissingField { .. } => "E-SNAP-CAMPO-AUSENTE",
            HarnessFailure::InvalidField { .. } => "E-SNAP-CAMPO-INVALIDO",
            HarnessFailure::SchemaUnknown { authority, .. } => authority.schema_error_code(),
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
            HarnessFailure::OverrideStaleSummary { .. } => "E-SNAP-OVERRIDE-SUMMARY",
            HarnessFailure::FieldNotAllowedForOp { .. } => "E-SNAP-CAMPO-DA-OPERACAO",
            HarnessFailure::OverrideRegionPairInvalid { .. } => "E-SNAP-OVERRIDE-PAR",
            HarnessFailure::OverrideStaleBase { .. } => "E-SNAP-OVERRIDE-BASE",
            HarnessFailure::ExclusionNoMatch { .. } => "E-SNAP-EXCLUSAO-SEM-CORRESPONDENCIA",
            HarnessFailure::DuplicateSnapshot { .. } => "E-SNAP-SNAPSHOT-DUPLICADO",
            HarnessFailure::DuplicateRecipe { .. } => "E-SNAP-RECEITA-DUPLICADA",
            HarnessFailure::BaseSnapshotMissing { .. } => "E-SNAP-BASE-AUSENTE",
            HarnessFailure::RecipeMissing { .. } => "E-SNAP-RECEITA-AUSENTE",
            HarnessFailure::CompositionCycle { .. } => "E-SNAP-CICLO",
            HarnessFailure::BaseMeasuresDiverged { .. } => "E-SNAP-BASE-DIVERGENTE",
            HarnessFailure::FrozenDependsOnCandidate { .. } => "E-SNAP-CONGELADO-SOBRE-CANDIDATO",
            HarnessFailure::RecipeHasSnapshotField { .. } => "E-SNAP-RECEITA-CAMPO-DE-SNAPSHOT",
            HarnessFailure::CapabilityRequiresSchema { authority, .. } => match authority {
                SchemaAuthority::Snapshot => "E-SNAP-CAPACIDADE-SCHEMA",
                SchemaAuthority::Recipe => "E-RECEITA-CAPACIDADE-SCHEMA",
            },
            HarnessFailure::SelfBase { .. } => "E-SNAP-BASE-PROPRIA",
            HarnessFailure::RecipeSelfStep { .. } => "E-RECEITA-PASSO-PROPRIO",
            HarnessFailure::ExclusionPartiallyConsumed { .. } => "E-SNAP-EXCLUSAO-PARCIAL",
            HarnessFailure::MaterializationMissing { .. } => "E-SNAP-MATERIALIZACAO-AUSENTE",
            HarnessFailure::MaterializationExcess { .. } => "E-SNAP-MATERIALIZACAO-EXCEDENTE",
            HarnessFailure::MaterializationRepeated { .. } => "E-SNAP-MATERIALIZACAO-REPETIDA",
            HarnessFailure::MaterializationCollision { .. } => "E-SNAP-MATERIALIZACAO-COLISAO",
            HarnessFailure::OperationOutsideAuthority { .. } => {
                "E-SNAP-OPERACAO-FORA-DA-AUTORIDADE"
            }
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
            HarnessFailure::SchemaUnknown { authority, found } => write!(
                f,
                "schema {} desconhecido para {}; este formato aceita {}",
                found,
                authority.as_str(),
                authority.supported_versions()
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
            HarnessFailure::OverrideStaleSummary {
                key,
                expected,
                found,
            } => write!(
                f,
                "summary corrente da região '{}' divergente: esperado '{}', encontrado '{}'",
                key, expected, found
            ),
            HarnessFailure::FieldNotAllowedForOp { op, field } => write!(
                f,
                "campo '{}' não pertence à operação '{}'",
                field, op
            ),
            HarnessFailure::OverrideRegionPairInvalid { key, msg } => write!(
                f,
                "override-region de '{}' inválido: {}",
                key, msg
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
            HarnessFailure::DuplicateSnapshot { id } => {
                write!(f, "snapshot duplicado: '{}'", id)
            }
            HarnessFailure::DuplicateRecipe { id } => {
                write!(f, "receita duplicada: '{}'", id)
            }
            HarnessFailure::BaseSnapshotMissing { id } => {
                write!(f, "base_snapshot '{}' não existe entre os snapshots", id)
            }
            HarnessFailure::RecipeMissing { id } => {
                write!(f, "receita '{}' não existe", id)
            }
            HarnessFailure::CompositionCycle { path } => {
                write!(f, "ciclo no grafo de composição: {}", path)
            }
            HarnessFailure::BaseMeasuresDiverged {
                id,
                expected,
                observed,
            } => write!(
                f,
                "base '{}' não bate com as próprias medidas: esperado regioes={} comprimento={} {}, observado regioes={} comprimento={} {}",
                id,
                expected.regions,
                expected.length,
                expected.fnv1a64_canonical(),
                observed.regions,
                observed.length,
                observed.fnv1a64_canonical()
            ),
            HarnessFailure::FrozenDependsOnCandidate { frozen, candidate } => write!(
                f,
                "snapshot congelado '{}' depende do candidato '{}'",
                frozen, candidate
            ),
            HarnessFailure::RecipeHasSnapshotField { field } => write!(
                f,
                "receita declara '{}', que pertence a snapshot: receita não tem medidas, estado nem predecessor",
                field
            ),
            HarnessFailure::CapabilityRequiresSchema {
                authority,
                capability,
                found_schema,
                required_schema,
            } => write!(
                f,
                "'{}' exige schema {} de {}; o arquivo declara schema {}",
                capability,
                required_schema,
                authority.as_str(),
                found_schema
            ),
            HarnessFailure::SelfBase { id } => {
                write!(f, "snapshot '{}' declara a si mesmo como base", id)
            }
            HarnessFailure::RecipeSelfStep { id } => {
                write!(f, "receita '{}' declara a si mesma como passo", id)
            }
            HarnessFailure::ExclusionPartiallyConsumed {
                selector,
                expected,
                consumed,
            } => write!(
                f,
                "exclusão parcialmente consumida em '{}': esperado {}, consumido {}",
                selector, expected, consumed
            ),
            HarnessFailure::MaterializationMissing { declared, found } => write!(
                f,
                "materialização ausente: expected_materializations = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::MaterializationExcess { declared, found } => write!(
                f,
                "materialização excedente: expected_materializations = {}, encontradas {}",
                declared, found
            ),
            HarnessFailure::MaterializationRepeated { key } => write!(
                f,
                "região histórica '{}' materializada duas vezes: um fato, uma autoridade",
                key
            ),
            HarnessFailure::MaterializationCollision { key } => write!(
                f,
                "região '{}' já existe no estado: a materialização histórica não sobrescreve nem funde",
                key
            ),
            HarnessFailure::OperationOutsideAuthority { authority, op } => write!(
                f,
                "operação '{}' não existe na autoridade de {}",
                op,
                authority.as_str()
            ),
        }
    }
}
// @pinker-nav:end trama.snapshots.erros

// @pinker-nav:start trama.snapshots.medidas
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary FNV-1a 64 sobre bytes e projeção estável canônica de regiões: a forma exata (tupla Debug com schema, key, kind, domain, layer, file, summary, hash e status, uma por linha, ordenada lexicograficamente) que define o comprimento e o hash de toda medida histórica da cartografia, com um único formatador servindo tanto a região de código corrente quanto a linha histórica que já não tem fonte.

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

/// Uma região **como ela participa da projeção estável**: os oito campos que
/// [`stable_projection`] lê, e nada além disso.
///
/// [`CodeRegion`] tem dezessete campos e descreve uma região de código
/// **corrente**: posições de linha, símbolos, `phase`. Nenhum deles participa da
/// medida histórica, e nenhuma regra de reconstrução os lê — verificado regra a
/// regra. Carregar o tipo inteiro através da reconstrução era excedente
/// acidental, e foi justamente esse excedente que fez a materialização de uma
/// região removida parecer impossível: para inserir uma linha histórica era
/// preciso inventar nove campos que ninguém consulta, entre eles offsets
/// apontando para código que não existe mais.
///
/// A conversão a partir do catálogo corrente acontece uma vez, na borda de
/// entrada da reconstrução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRegion {
    pub key: String,
    pub kind: String,
    pub domain: Option<String>,
    pub layer: Option<String>,
    pub file: String,
    pub summary: String,
    pub hash: String,
    pub status: String,
}

impl From<&CodeRegion> for ProjectionRegion {
    fn from(region: &CodeRegion) -> ProjectionRegion {
        ProjectionRegion {
            key: region.key.clone(),
            kind: region.kind.clone(),
            domain: region.domain.clone(),
            layer: region.layer.clone(),
            file: region.file.clone(),
            summary: region.summary.clone(),
            hash: region.hash.clone(),
            status: region.status.clone(),
        }
    }
}

/// Os campos estáveis emprestados de uma região, prontos para serializar.
///
/// Existe para que a forma canônica continue tendo **uma** autoridade: tanto a
/// região de código corrente quanto a linha histórica materializada produzem
/// esta mesma visão, e só ela sabe virar registro.
pub struct StableFields<'a> {
    pub key: &'a str,
    pub kind: &'a str,
    pub domain: Option<&'a str>,
    pub layer: Option<&'a str>,
    pub file: &'a str,
    pub summary: &'a str,
    pub hash: &'a str,
    pub status: &'a str,
}

impl StableFields<'_> {
    /// O registro canônico, terminado em `\n`. Toda medida histórica da
    /// cartografia nasce daqui.
    fn record(&self) -> String {
        format!(
            "{:?}\n",
            (
                1,
                self.key,
                self.kind,
                self.domain,
                self.layer,
                self.file,
                self.summary,
                self.hash,
                self.status,
            )
        )
    }
}

/// O que basta para uma região entrar na projeção estável.
pub trait StableProjectionRow {
    fn stable_fields(&self) -> StableFields<'_>;
}

impl StableProjectionRow for CodeRegion {
    fn stable_fields(&self) -> StableFields<'_> {
        StableFields {
            key: self.key.as_str(),
            kind: self.kind.as_str(),
            domain: self.domain.as_deref(),
            layer: self.layer.as_deref(),
            file: self.file.as_str(),
            summary: self.summary.as_str(),
            hash: self.hash.as_str(),
            status: self.status.as_str(),
        }
    }
}

impl StableProjectionRow for ProjectionRegion {
    fn stable_fields(&self) -> StableFields<'_> {
        StableFields {
            key: self.key.as_str(),
            kind: self.kind.as_str(),
            domain: self.domain.as_deref(),
            layer: self.layer.as_deref(),
            file: self.file.as_str(),
            summary: self.summary.as_str(),
            hash: self.hash.as_str(),
            status: self.status.as_str(),
        }
    }
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
pub fn stable_projection<'a, R>(regions: impl Iterator<Item = &'a R>) -> String
where
    R: StableProjectionRow + 'a,
{
    let mut records: Vec<String> = regions
        .map(|region| region.stable_fields().record())
        .collect();
    records.sort_unstable();
    records.concat()
}

/// Mede um conjunto de regiões, produzindo as três medidas canônicas.
pub fn measure<'a, R>(regions: impl Iterator<Item = &'a R>) -> Measures
where
    R: StableProjectionRow + 'a,
{
    let collected: Vec<&R> = regions.collect();
    let projection = stable_projection(collected.iter().copied());
    Measures {
        regions: collected.len() as u64,
        length: projection.len() as u64,
        fnv1a64: fnv1a64(projection.as_bytes()),
    }
}
// @pinker-nav:end trama.snapshots.medidas

// @pinker-nav:start trama.snapshots.renderizacao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Renderer TOML canônico: ordem fixa de campos e seções, regras ordenadas por operação e seletor, campos opcionais e orçamento de materialização emitidos apenas quando existem, escaping mínimo e determinístico, sem qualquer dependência de root absoluto, PID, usuário, locale, tempo, HashMap ou endereço de memória — a saída é função apenas do modelo.

/// Escapa um texto para string básica TOML.
///
/// Cobre exatamente os escapes que o parser aceita, de modo que
/// `parse(render(x)) == x` para todo modelo válido.
pub(crate) fn toml_escape(value: &str) -> String {
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

/// Renderiza o corpo de uma regra (tudo menos o cabeçalho `[[rules]]`).
///
/// Compartilhada pelas duas autoridades: snapshot e receita escrevem regras com
/// exatamente a mesma forma canônica.
pub(crate) fn render_rule_body(rule: &Rule) -> String {
    let mut out = String::new();
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
        Rule::ExcludeKeyPrefix {
            prefix,
            expected_matches,
        }
        | Rule::ExcludeFilePrefix {
            prefix,
            expected_matches,
        } => {
            out.push_str(&format!("prefix = {}\n", toml_escape(prefix)));
            out.push_str(&format!("expected_matches = {}\n", expected_matches));
        }
        Rule::ExcludeFile {
            file,
            expected_matches,
        } => {
            out.push_str(&format!("file = {}\n", toml_escape(file)));
            out.push_str(&format!("expected_matches = {}\n", expected_matches));
        }
        Rule::OverrideRegion {
            key,
            from_hash,
            to_hash,
            from_summary,
            to_summary,
            expect_file,
            to_file,
            expect_domain,
            expect_layer,
        } => {
            out.push_str(&format!("key = {}\n", toml_escape(key)));
            if let Some(valor) = from_hash {
                out.push_str(&format!("from_hash = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = to_hash {
                out.push_str(&format!("to_hash = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = from_summary {
                out.push_str(&format!("from_summary = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = to_summary {
                out.push_str(&format!("to_summary = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = expect_file {
                out.push_str(&format!("expect_file = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = to_file {
                out.push_str(&format!("to_file = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = expect_domain {
                out.push_str(&format!("expect_domain = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = expect_layer {
                out.push_str(&format!("expect_layer = {}\n", toml_escape(valor)));
            }
        }
        Rule::MaterializeRegion {
            key,
            kind,
            domain,
            layer,
            file,
            summary,
            hash,
            status,
        } => {
            out.push_str(&format!("key = {}\n", toml_escape(key)));
            out.push_str(&format!("kind = {}\n", toml_escape(kind)));
            if let Some(valor) = domain {
                out.push_str(&format!("domain = {}\n", toml_escape(valor)));
            }
            if let Some(valor) = layer {
                out.push_str(&format!("layer = {}\n", toml_escape(valor)));
            }
            out.push_str(&format!("file = {}\n", toml_escape(file)));
            out.push_str(&format!("summary = {}\n", toml_escape(summary)));
            out.push_str(&format!("hash = {}\n", toml_escape(hash)));
            out.push_str(&format!("status = {}\n", toml_escape(status)));
        }
    }
    out
}

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
    if let Some(base) = &snapshot.base_snapshot {
        out.push_str(&format!("base_snapshot = {}\n", toml_escape(base)));
    }
    if !snapshot.recipes.is_empty() {
        // A ordem declarada é procedural e faz parte do significado: o renderer
        // a preserva em vez de canonicalizar por nome.
        let itens: Vec<String> = snapshot.recipes.iter().map(|r| toml_escape(r)).collect();
        out.push_str(&format!("recipes = [{}]\n", itens.join(", ")));
    }
    out.push_str(&format!(
        "expected_overrides = {}\n",
        snapshot.expected_overrides
    ));
    out.push_str(&format!(
        "expected_exclusions = {}\n",
        snapshot.expected_exclusions
    ));
    // Emitido apenas quando existe. Ausente significa zero, e é por isso que os
    // snapshots já congelados continuam byte-idênticos ao que o renderer produz.
    if snapshot.expected_materializations > 0 {
        out.push_str(&format!(
            "expected_materializations = {}\n",
            snapshot.expected_materializations
        ));
    }

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
        out.push_str(&render_rule_body(rule));
    }

    out
}
// @pinker-nav:end trama.snapshots.renderizacao

// @pinker-nav:start trama.snapshots.reconstrucao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Reconstrução pura do estado histórico a partir do catálogo corrente, com livro de consumo por regra: exclusões consomem exatamente o orçamento declarado e ao menos uma correspondência, overrides e materializações consomem exatamente uma, a ordem fixa é exclusões, overrides e por último materializações, e ausência, excedente, ambiguidade, key alterada, path alterado, metadata alterada, colisão com região presente ou base divergente falham como harness, nunca como drift.

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
    pub regions: Vec<ProjectionRegion>,
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
/// a entrada. A ordem de aplicação é fixa e independe da ordem textual das
/// regras: exclusões, depois overrides, depois materializações.
///
/// A materialização vem por último porque é isso que torna estruturalmente
/// impossível excluir o que acabou de ser declarado, ou aplicar override sobre
/// uma região que só passa a existir ali — as duas sequências que não têm
/// significado. Nenhuma delas precisa de código de exceção.
///
/// A relocação de caminho (`to_file`) acontece **dentro** da fase de override,
/// na mesma regra atômica que restaura `hash` e `summary`, e não como uma etapa
/// própria. Não existe ordem entre relocar e restaurar conteúdo porque não há
/// duas operações para ordenar.
///
/// A consequência sobre as exclusões por arquivo é **de escopo**, e a distinção
/// importa: dentro de um mesmo conjunto de regras, `exclude-file` e
/// `exclude-file-prefix` correm antes e portanto casam com o caminho corrente.
/// Um escopo posterior — o snapshot que aplica uma receita, por exemplo — já
/// recebe o estado com o caminho histórico e casa contra ele. Cada escopo
/// enxerga a saída do anterior, que é a mesma regra de composição que vale para
/// `hash` e `summary`.
pub fn reconstruct(
    base: &[CodeRegion],
    snapshot: &ProjectionSnapshot,
) -> Result<Reconstruction, HarnessFailure> {
    let entrada: Vec<ProjectionRegion> = base.iter().map(ProjectionRegion::from).collect();
    let (regions, ledger) = apply_rules(entrada, &snapshot.rules)?;
    Ok(Reconstruction { regions, ledger })
}

/// Aplica um conjunto de regras a um estado, validando o consumo **no escopo
/// deste conjunto**.
///
/// É a unidade compartilhada por snapshots e receitas: cada escopo valida o
/// próprio consumo, e nenhum consumo é contado duas vezes, porque cada regra
/// pertence a exatamente um escopo.
pub fn apply_rules(
    entrada: Vec<ProjectionRegion>,
    regras: &[Rule],
) -> Result<(Vec<ProjectionRegion>, Vec<RuleConsumption>), HarnessFailure> {
    let mut regions: Vec<ProjectionRegion> = entrada;
    let mut ledger: Vec<RuleConsumption> = Vec::with_capacity(regras.len());
    let mut rules = regras.to_vec();
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
            Rule::ExcludeFile {
                file,
                expected_matches,
            } => {
                let consumed = regions.iter().filter(|region| region.file == *file).count() as u64;
                if consumed == 0 {
                    return Err(HarnessFailure::ExclusionNoMatch {
                        selector: file.clone(),
                    });
                }
                if consumed != *expected_matches {
                    return Err(HarnessFailure::ExclusionPartiallyConsumed {
                        selector: file.clone(),
                        expected: *expected_matches,
                        consumed,
                    });
                }
                regions.retain(|region| region.file != *file);
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: file.clone(),
                    expected: *expected_matches,
                    consumed,
                });
            }
            Rule::ExcludeKeyPrefix {
                prefix,
                expected_matches,
            } => {
                let consumed = regions
                    .iter()
                    .filter(|region| region.key.starts_with(prefix.as_str()))
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
                regions.retain(|region| !region.key.starts_with(prefix.as_str()));
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: prefix.clone(),
                    expected: *expected_matches,
                    consumed,
                });
            }
            Rule::OverrideRegion {
                key,
                from_hash,
                to_hash,
                from_summary,
                to_summary,
                expect_file,
                to_file,
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

                // Fase de validação. Nenhum campo é tocado até que **todas** as
                // precondições passem: identidade, metadata e cada `from`
                // declarado. É isto que torna a regra atômica no sentido lógico.
                {
                    let region = &regions[matches[0]];
                    if let Some(esperado) = expect_file {
                        if &region.file != esperado {
                            return Err(HarnessFailure::PathChanged {
                                key: key.clone(),
                                expected: esperado.clone(),
                                found: region.file.clone(),
                            });
                        }
                    }
                    if let Some(esperado) = expect_domain {
                        let encontrado = region.domain.clone().unwrap_or_default();
                        if &encontrado != esperado {
                            return Err(HarnessFailure::MetadataChanged {
                                key: key.clone(),
                                field: "domain".to_string(),
                                expected: esperado.clone(),
                                found: encontrado,
                            });
                        }
                    }
                    if let Some(esperado) = expect_layer {
                        let encontrado = region.layer.clone().unwrap_or_default();
                        if &encontrado != esperado {
                            return Err(HarnessFailure::MetadataChanged {
                                key: key.clone(),
                                field: "layer".to_string(),
                                expected: esperado.clone(),
                                found: encontrado,
                            });
                        }
                    }
                    if let Some(esperado) = from_hash {
                        if &region.hash != esperado {
                            return Err(HarnessFailure::OverrideStaleBase {
                                key: key.clone(),
                                expected: esperado.clone(),
                                found: region.hash.clone(),
                            });
                        }
                    }
                    if let Some(esperado) = from_summary {
                        if &region.summary != esperado {
                            return Err(HarnessFailure::OverrideStaleSummary {
                                key: key.clone(),
                                expected: esperado.clone(),
                                found: region.summary.clone(),
                            });
                        }
                    }
                }

                // Fase de mutação. Só se chega aqui com tudo validado.
                let region = &mut regions[matches[0]];
                if let Some(valor) = to_hash {
                    region.hash.clone_from(valor);
                }
                if let Some(valor) = to_summary {
                    region.summary.clone_from(valor);
                }
                if let Some(valor) = to_file {
                    region.file.clone_from(valor);
                }
                ledger.push(RuleConsumption {
                    op: rule.op(),
                    selector: key.clone(),
                    expected: 1,
                    consumed: 1,
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
            Rule::MaterializeRegion {
                key,
                kind,
                domain,
                layer,
                file,
                summary,
                hash,
                status,
            } => {
                if regions.iter().any(|region| &region.key == key) {
                    return Err(HarnessFailure::MaterializationCollision { key: key.clone() });
                }
                regions.push(ProjectionRegion {
                    key: key.clone(),
                    kind: kind.clone(),
                    domain: domain.clone(),
                    layer: layer.clone(),
                    file: file.clone(),
                    summary: summary.clone(),
                    hash: hash.clone(),
                    status: status.clone(),
                });
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

    Ok((regions, ledger))
}

/// Distingue "região removida" de "key alterada" quando um override não
/// encontra correspondência.
///
/// Se a regra declarou path e metadata e existe exatamente uma região com essa
/// identidade sob outra chave, a causa é key alterada. Caso contrário, a região
/// foi removida.
fn missing_override_failure(
    regions: &[ProjectionRegion],
    key: &str,
    expect_file: Option<&str>,
    expect_domain: Option<&str>,
    expect_layer: Option<&str>,
) -> HarnessFailure {
    if let Some(file) = expect_file {
        let candidates: Vec<&ProjectionRegion> = regions
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
    out.push_str(&format!("{{\"schema\":{}", SNAPSHOT_REPORT_SCHEMA));
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
mod tests;
