//! Guardião estrutural da decomposição física do núcleo de snapshots
//! históricos (#617, unidades NPS-2 e NPS-1 do inventário da #601).
//!
//! `src/nav_projection_snapshot.rs` é a autoridade de domínio dos snapshots
//! históricos das projeções do catálogo de navegação. A #617 desce o parser
//! TOML estrito e o módulo de teste para `src/nav_projection_snapshot/` sem
//! dividir essa autoridade: o schema, a separação entre falha de harness e
//! drift, a ordem das regras de reconstrução e o orçamento de consumo
//! continuam decididos numa definição só, e o pai continua sendo um arquivo,
//! não virou `mod.rs`.
//!
//! O corte cria três formas de cegueira silenciosa, e este arquivo fecha as
//! três e nada mais:
//!
//! 1. um oráculo textual que continuasse lendo só `src/nav_projection_snapshot.rs`
//!    seguiria verde e pararia de observar o parser inteiro — a OG-1 da #601.
//!    Os dois censos de fonte deste módulo passaram a ler
//!    `fonte_de_modulo::nav_projection_snapshot()`, e o teste abaixo prova que
//!    a lista lida por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ou ficar para trás no pai;
//! 3. o caminho público podia sumir: `parse` e `validate_rules` eram `pub` no
//!    pai e desceram para o irmão. Sem a reexportação,
//!    `pinker_v0::nav_projection_snapshot::parse` deixaria de existir — remoção
//!    de API pública disfarçada de decomposição física.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo dos testes movidos.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{nav_projection_snapshot, NAV_PROJECTION_SNAPSHOT_ARQUIVOS};
use pinker_v0::nav_projection_snapshot::{
    HarnessFailure, ProjectionSnapshot, Rule, SchemaAuthority,
};
use rust_source::codigo_executavel;

/// A região que a NPS-1 moveu, e o irmão onde passa a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[("trama.snapshots.parser", "parser.rs")];

/// As regiões que o corte deixou onde estavam. As duas primeiras são as
/// vizinhas imediatas do span da NPS-1 — a que vem antes e a que vem depois —,
/// e são elas que ficariam vermelhas se o corte tivesse escorregado uma região
/// para qualquer lado.
const REGIOES_RETIDAS: &[&str] = &[
    "trama.snapshots.medidas",
    "trama.snapshots.renderizacao",
    "trama.snapshots.modelo",
    "trama.snapshots.erros",
    "trama.snapshots.reconstrucao",
    "trama.snapshots.verificacao",
    "trama.snapshots.relatorio",
];

/// As definições que a NPS-1 moveu inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão, e nenhuma deixada para trás no pai.
///
/// `pub fn parse(text: &str)` aparece com a assinatura inteira de propósito:
/// `SnapshotState::parse` é outra função, continua no pai, e um prefixo curto
/// confundiria as duas.
const DEFINICOES_MOVIDAS: &[(&str, &str)] = &[
    ("enum Scalar", "parser.rs"),
    ("struct Table", "parser.rs"),
    ("struct RawDocument", "parser.rs"),
    ("pub fn parse(text: &str)", "parser.rs"),
    ("fn parse_raw(", "parser.rs"),
    ("enum Section", "parser.rs"),
    ("fn parse_value(", "parser.rs"),
    ("const ROOT_KEYS", "parser.rs"),
    ("const RECONSTRUCTION_KEYS", "parser.rs"),
    ("const MEASURES_KEYS", "parser.rs"),
    ("const RULE_KEYS_BY_OP", "parser.rs"),
    ("const RULE_KEYS:", "parser.rs"),
    ("fn allowed_keys_for_op(", "parser.rs"),
    ("fn reject_unknown(", "parser.rs"),
    ("fn require_text(", "parser.rs"),
    ("fn optional_text(", "parser.rs"),
    ("fn require_integer(", "parser.rs"),
    ("fn optional_list(", "parser.rs"),
    ("fn validate_id(", "parser.rs"),
    ("fn validate_hash(", "parser.rs"),
    ("fn validate_relative_path(", "parser.rs"),
    ("pub fn validate_rules(", "parser.rs"),
    ("fn require_nonempty(", "parser.rs"),
    ("fn build(", "parser.rs"),
    ("fn sort_rules(", "parser.rs"),
    ("fn build_rule(", "parser.rs"),
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["parser.rs"];

/// Os itens `pub` que cada irmão pode ter, e por quê. A lista é exaustiva: o
/// corte físico não promove nada.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[
    // NPS-1: `parse` e `validate_rules` já eram `pub` em
    // `src/nav_projection_snapshot.rs` antes do move, e o pai as reexporta
    // para preservar os dois caminhos públicos.
    ("parser.rs", &["pub fn parse(", "pub fn validate_rules("]),
    // NPS-2: o módulo de teste era privado e `#[cfg(test)]` no pai e continua
    // sendo; nada nele é público.
    ("tests.rs", &[]),
];

/// A visibilidade restrita que cada irmão pode ter. Também exaustiva, e por
/// isso separada de [`PUB_AUTORIZADO`]: `pub(crate)` não é superfície pública
/// da lib, mas também não é o privado do módulo.
///
/// Nenhum destes é novo. Os dezoito eram `pub(crate)` em
/// `src/nav_projection_snapshot.rs` antes do move e continuam alcançáveis pelo
/// mesmo caminho, `crate::nav_projection_snapshot::`, pela reexportação do
/// pai. Qualquer `pub(crate)` a mais é promoção, não decomposição física.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[
    (
        "parser.rs",
        &[
            "pub(crate) enum Scalar",
            "pub(crate) struct Table",
            "pub(crate) fn as_integer(",
            "pub(crate) fn get(",
            "pub(crate) struct RawDocument",
            "pub(crate) root: Table,",
            "pub(crate) reconstruction: Option<Table>,",
            "pub(crate) measures: Option<Table>,",
            "pub(crate) rules: Vec<Table>,",
            "pub(crate) fn parse_raw(",
            "pub(crate) fn reject_unknown(",
            "pub(crate) fn require_text(",
            "pub(crate) fn optional_text(",
            "pub(crate) fn require_integer(",
            "pub(crate) fn optional_list(",
            "pub(crate) fn validate_id(",
            "pub(crate) fn sort_rules(",
            "pub(crate) fn build_rule(",
        ],
    ),
    ("tests.rs", &[]),
];

/// A superfície pública do módulo, congelada item a item.
///
/// É o contrato `PUBLIC_PATHS_BEFORE == AFTER` da #617 na forma que um teste
/// consegue observar: se o move apagar um caminho, se a reexportação sumir ou
/// se o corte promover um item novo, a lista deixa de bater. Ela não impede a
/// evolução da API: impede que ela mude sem que alguém diga que mudou.
const API_PUBLICA_CONGELADA: &[&str] = &[
    "FNV_PREFIX",
    "MAX_ID_LEN",
    "SNAPSHOTS_DIR",
    "SNAPSHOT_REPORT_SCHEMA",
    "SNAPSHOT_SCHEMA",
    "SNAPSHOT_SCHEMA_V1",
    "SNAPSHOT_SCHEMA_V2",
    "SNAPSHOT_SCHEMA_V3",
    "SNAPSHOT_SCHEMA_V4",
    "SNAPSHOT_SCHEMA_V5",
    "Divergence",
    "HarnessFailure",
    "Measures",
    "Outcome",
    "ProjectionRegion",
    "ProjectionSnapshot",
    "Reconstruction",
    "Rule",
    "RuleConsumption",
    "SchemaAuthority",
    "SnapshotState",
    "StableFields",
    "StableProjectionRow",
    "TomlError",
    "VerifyReport",
    "apply_rules",
    "fnv1a64",
    "fnv1a64_canonical",
    "human_report",
    "json_report",
    "measure",
    "parse",
    "reconstruct",
    "render",
    "stable_projection",
    "validate_rules",
    "verify",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/nav_projection_snapshot")
}

fn fonte(nome: &str) -> &'static str {
    NAV_PROJECTION_SNAPSHOT_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

fn pai() -> &'static str {
    fonte("nav_projection_snapshot.rs")
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo que lê o módulo por `fonte_de_modulo`.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/nav_projection_snapshot/ legível")
        .map(|entrada| entrada.expect("entrada de diretório").path())
        .filter(|caminho| caminho.extension().is_some_and(|ext| ext == "rs"))
        .map(|caminho| {
            caminho
                .file_name()
                .expect("nome de arquivo")
                .to_str()
                .expect("utf-8")
                .to_string()
        })
        .collect();
    let declarados: BTreeSet<String> = NAV_PROJECTION_SNAPSHOT_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "nav_projection_snapshot.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/nav_projection_snapshot/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate e o que desceu deixa de existir sem
/// que nada fique vermelho. É a sensitivity M1 da #617.
#[test]
fn o_pai_inclui_o_irmao() {
    let codigo = codigo_executavel(pai());
    for (nome, _) in NAV_PROJECTION_SNAPSHOT_ARQUIVOS {
        if *nome == "nav_projection_snapshot.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/nav_projection_snapshot.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
        // O filho é detalhe físico, não caminho público. `pub mod parser;`
        // criaria `pinker_v0::nav_projection_snapshot::parser::parse` sem
        // apagar nenhum dos 37 itens congelados — ampliação de superfície que
        // o censo de itens não veria sozinho.
        assert_eq!(
            codigo.matches(&format!("pub {declaracao}")).count(),
            0,
            "src/nav_projection_snapshot.rs tornou o irmão `{modulo}` um caminho público"
        );
    }
    // O pai continua sendo um arquivo: a #617 mantém a forma que a #608
    // decidiu e a #610, a #612 e a #615 mantiveram.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608"
    );
}

/// A declaração do módulo de teste é a última coisa do pai. Subi-la para o
/// topo deixaria verde e cego qualquer censo que corte no primeiro
/// `#[cfg(test)]`.
#[test]
fn o_corte_no_primeiro_cfg_test_ainda_ve_a_producao_inteira_do_pai() {
    let corte = pai()
        .find("\n#[cfg(test)]")
        .expect("o pai declara o irmão de teste sob #[cfg(test)]");
    let depois: String = pai()[corte..]
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        depois, "#[cfg(test)] mod tests;",
        "depois do primeiro `#[cfg(test)]` o pai passou a ter conteúdo que os \
         oráculos que cortam ali deixam de observar"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem
/// implementação deixada para trás no arquivo antigo. São as sensitivities M2
/// e M3 da #617.
#[test]
fn cada_regiao_e_cada_definicao_movida_aparece_uma_vez_no_arquivo_certo() {
    let modulo = nav_projection_snapshot();
    for (chave, arquivo) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/nav_projection_snapshot/{arquivo}"
        );
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            pai().contains(&marcador),
            "a região {chave} não é da NPS-1 e deveria continuar em \
             src/nav_projection_snapshot.rs"
        );
    }

    let codigo = codigo_executavel(&modulo);
    let codigo_do_pai = codigo_executavel(pai());
    for (definicao, arquivo) in DEFINICOES_MOVIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo \
             nav_projection_snapshot"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            0,
            "a implementação de `{definicao}` ficou para trás em \
             src/nav_projection_snapshot.rs"
        );
        assert_eq!(
            codigo_executavel(fonte(arquivo)).matches(definicao).count(),
            1,
            "`{definicao}` deveria morar em src/nav_projection_snapshot/{arquivo}"
        );
    }

    // O módulo de teste viajou inteiro: uma declaração no pai, um corpo no
    // irmão, e nenhum bloco `mod tests {` deixado para trás.
    assert_eq!(
        codigo_do_pai.matches("mod tests;").count(),
        1,
        "o pai deveria declarar `mod tests;` exatamente uma vez"
    );
    assert_eq!(
        codigo.matches("mod tests {").count(),
        0,
        "o módulo de teste voltou a ser um bloco em vez de um arquivo"
    );
}

fn conferir_regiao_unica(modulo: &str, chave: &str) {
    for marcador in [
        format!("// @pinker-nav:start {chave}"),
        format!("// @pinker-nav:end {chave}"),
    ] {
        assert_eq!(
            modulo.matches(&marcador).count(),
            1,
            "`{marcador}` deveria aparecer exatamente uma vez no módulo \
             nav_projection_snapshot"
        );
    }
}

/// A decomposição é física: não promove nada. Cada irmão tem duas listas
/// exaustivas — o que pode ser `pub` e o que pode ter visibilidade restrita —,
/// e nada mais. É a sensitivity M4 da #617.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in NAV_PROJECTION_SNAPSHOT_ARQUIVOS {
        if *nome == "nav_projection_snapshot.rs" {
            continue;
        }
        let codigo = codigo_executavel(fonte);
        let autorizados = itens_autorizados(PUB_AUTORIZADO, nome, "`pub`");
        assert_eq!(
            codigo.matches("pub ").count(),
            autorizados.len(),
            "src/nav_projection_snapshot/{nome} tem mais itens `pub` do que o corte previa"
        );
        let restritos = itens_autorizados(PUB_RESTRITO_AUTORIZADO, nome, "visibilidade restrita");
        assert_eq!(
            codigo.matches("pub(").count(),
            restritos.len(),
            "src/nav_projection_snapshot/{nome} tem mais visibilidade restrita do que o \
             corte previa"
        );
        for item in autorizados.iter().chain(restritos.iter()) {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/nav_projection_snapshot/{nome} deveria conter `{item}` exatamente uma vez"
            );
        }
    }
}

/// A lista exaustiva de um irmão, ou o panic que recusa um irmão não
/// declarado: um arquivo novo não entra no módulo sem dizer o que expõe.
fn itens_autorizados(
    lista: &'static [(&'static str, &'static [&'static str])],
    nome: &str,
    classe: &str,
) -> &'static [&'static str] {
    lista
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, itens)| *itens)
        .unwrap_or_else(|| {
            panic!(
                "src/nav_projection_snapshot/{nome} não declarou que itens de {classe} o \
                 corte previa"
            )
        })
}

/// `parse` e `validate_rules` já eram `pub` antes da NPS-1, e
/// `pinker_v0::nav_projection_snapshot::parse` e `::validate_rules` são
/// caminhos públicos da lib. O move desce as duas definições um nível; sem a
/// reexportação do pai os caminhos sumiriam — remoção de API pública
/// disfarçada de decomposição física. As coerções abaixo são estáticas: se a
/// reexportação ou a assinatura mudarem, isto não compila. É a sensitivity M5
/// da #617.
const _PARSE_PRESERVADO: fn(&str) -> Result<ProjectionSnapshot, HarnessFailure> =
    pinker_v0::nav_projection_snapshot::parse;
const _VALIDATE_RULES_PRESERVADO: fn(u64, &[Rule], SchemaAuthority) -> Result<(), HarnessFailure> =
    pinker_v0::nav_projection_snapshot::validate_rules;

#[test]
fn o_pai_reexporta_o_que_desceu() {
    let codigo = codigo_executavel(pai());
    assert_eq!(
        codigo
            .matches("pub use parser::{parse, validate_rules};")
            .count(),
        1,
        "src/nav_projection_snapshot.rs deveria reexportar `parse` e `validate_rules` \
         exatamente uma vez"
    );
    // Os dezoito `pub(crate)` do irmão continuam alcançáveis pelo caminho de
    // antes; oito deles são consumidos por `src/nav_projection_recipe.rs`.
    for item in [
        "build_rule",
        "optional_list",
        "parse_raw",
        "reject_unknown",
        "require_integer",
        "require_text",
        "sort_rules",
        "validate_id",
    ] {
        assert!(
            codigo.contains(item),
            "src/nav_projection_snapshot.rs deixou de reexportar `{item}`"
        );
    }
}

/// A superfície pública do módulo não mudou de tamanho nem de conteúdo.
#[test]
fn a_superficie_publica_do_modulo_e_exatamente_a_congelada() {
    let modulo = nav_projection_snapshot();
    let observados: BTreeSet<&str> = modulo
        .lines()
        .filter_map(|linha| linha.strip_prefix("pub "))
        .filter_map(|resto| {
            for palavra in ["fn ", "const ", "struct ", "enum ", "trait ", "type "] {
                if let Some(nome) = resto.strip_prefix(palavra) {
                    return Some(
                        nome.split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                            .unwrap_or(""),
                    );
                }
            }
            None
        })
        .collect();
    let congelados: BTreeSet<&str> = API_PUBLICA_CONGELADA.iter().copied().collect();
    assert_eq!(
        observados, congelados,
        "a superfície pública de nav_projection_snapshot mudou; a #617 é decomposição \
         física e não muda API pública"
    );

    // Itens não são a única forma de caminho público: um `pub mod` ou um
    // segundo `pub use` acrescentaria caminhos sem mudar a contagem acima.
    let codigo = codigo_executavel(&modulo);
    assert_eq!(
        codigo.matches("pub mod ").count(),
        0,
        "o módulo passou a expor um submódulo público; o corte é físico e o irmão é privado"
    );
    assert_eq!(
        codigo.matches("pub use ").count(),
        1,
        "a única reexportação pública do módulo é a que devolve `parse` e `validate_rules` \
         ao caminho de antes"
    );
}

/// Um irmão de produção que ganhasse um `#[cfg(test)]` esconderia tudo o que
/// viesse depois de qualquer censo que corte ali.
#[test]
fn irmao_de_producao_nao_esconde_producao_atras_de_cfg_test() {
    for nome in IRMAOS_DE_PRODUCAO {
        assert_eq!(
            codigo_executavel(fonte(nome))
                .matches("#[cfg(test)]")
                .count(),
            0,
            "src/nav_projection_snapshot/{nome} é produção e não pode cortar o censo \
             com um `#[cfg(test)]`"
        );
    }
}

/// A autoridade de domínio continua sendo uma só.
///
/// O irmão hospeda implementação; ele não decide schema, não redeclara as
/// versões aceitas, não reclassifica falha de harness como drift e não
/// reimplementa a reconstrução. E o módulo continua sem relação com
/// `src/projection.rs`, que é a autoridade das projeções documentais.
#[test]
fn o_irmao_hospeda_implementacao_e_nao_cria_uma_segunda_autoridade() {
    let irmao = codigo_executavel(fonte("parser.rs"));
    for proibido in [
        "const SNAPSHOT_SCHEMA",
        "const RECIPE_SCHEMA",
        "enum Outcome",
        "enum SnapshotState",
        "fn reconstruct(",
        "fn apply_rules(",
        "fn verify(",
        "crate::projection",
    ] {
        assert_eq!(
            irmao.matches(proibido).count(),
            0,
            "src/nav_projection_snapshot/parser.rs passou a decidir `{proibido}`, \
             duplicando autoridade em vez de hospedar implementação"
        );
    }
}
