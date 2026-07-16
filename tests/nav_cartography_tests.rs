//! Trama Pinker — cartografia semântica do código (§20 da cartografia).
//!
//! Cobre: múltiplas regiões no mesmo arquivo, preservação de âncoras
//! existentes, domínio/camada válidos e determinismo do catálogo.

use pinker_v0::nav::{CodeCatalog, CodeIndex};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_src(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_carto_{name}_{now}"))
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const TWO_REGIONS: &str = "\
// @pinker-nav:start token.lexico.vocabulario
// @pinker-nav:domain lexico
// @pinker-nav:layer token
// @pinker-nav:summary Vocabulario.
pub enum TokenKind { A, B }
// @pinker-nav:end token.lexico.vocabulario

pub struct Token;

// @pinker-nav:start token.representacao.spans
// @pinker-nav:domain representacao
// @pinker-nav:layer token
// @pinker-nav:summary Spans.
pub struct Span { pub a: usize }
// @pinker-nav:end token.representacao.spans
";

#[test]
fn multiplas_regioes_no_mesmo_arquivo() {
    let dir = temp_src("multi");
    write(&dir, "token.rs", TWO_REGIONS);
    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.regions.len(), 2);
    let keys: Vec<&str> = index.regions.iter().map(|r| r.key.as_str()).collect();
    assert!(keys.contains(&"token.lexico.vocabulario"));
    assert!(keys.contains(&"token.representacao.spans"));
    // Mesmo arquivo, chaves distintas, sem sobreposição reportada.
    assert!(index.regions.iter().all(|r| r.file == "src/token.rs"));
    assert!(index.verify().is_empty(), "{:?}", index.verify());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dominio_e_camada_preservados() {
    let dir = temp_src("meta");
    write(&dir, "token.rs", TWO_REGIONS);
    let index = CodeIndex::scan(&dir).unwrap();
    let voc = index.region("token.lexico.vocabulario").unwrap();
    assert_eq!(voc.domain.as_deref(), Some("lexico"));
    assert_eq!(voc.layer.as_deref(), Some("token"));
    assert!(voc.hash.starts_with("fnv1a64:"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ancora_existente_preservada() {
    // A âncora histórica do curto-circuito não pode ser duplicada nem perdida.
    let dir = temp_src("preserve");
    write(
        &dir,
        "cfg_ir.rs",
        "// @pinker-nav:start cfg.logica.curto-circuito\n// @pinker-nav:domain logica\n// @pinker-nav:layer cfg\n// @pinker-nav:summary Curto-circuito.\nfn curto() { let _x = 1; }\n// @pinker-nav:end cfg.logica.curto-circuito\n",
    );
    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.regions.len(), 1);
    assert_eq!(index.regions[0].key, "cfg.logica.curto-circuito");
    assert!(index.verify().is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn catalogo_deterministico() {
    let dir = temp_src("determ");
    write(&dir, "token.rs", TWO_REGIONS);
    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.render_jsonl(), index.render_jsonl());
    fs::remove_dir_all(dir).unwrap();
}

/// O catálogo real versionado contém as chaves essenciais do frontend (Onda 4),
/// da checagem semântica (Onda 5A), da monomorfização no parser (Onda 5B), do
/// lowering AST→IR (Onda 5C), do lowering IR→CFG (Onda 5D) e as âncoras
/// históricas, todas únicas. Verifica presença e unicidade — não um número exato
/// permanente de regiões.
#[test]
fn catalogo_versionado_tem_chaves_essenciais_e_unicas() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    // Unicidade de todas as chaves.
    let mut keys: Vec<&str> = catalog.regions.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    let mut dedup = keys.clone();
    dedup.dedup();
    assert_eq!(keys.len(), dedup.len(), "chaves duplicadas no catálogo");

    // Chaves essenciais do frontend (Onda 4), da semântica (Onda 5A) e âncoras
    // históricas preservadas.
    for essential in [
        "lexer.fluxo.tokenizacao",
        "lexer.espacos-comentarios.consumo",
        "parser.fluxo.nucleo",
        "parser.tipos.gramatica",
        "parser.comandos.bloco",
        "parser.expressoes.precedencia",
        "cfg.logica.curto-circuito",
        "cfg.logica.slot-logico",
        // Checagem semântica (Onda 5A).
        "semantic.importacoes.familias",
        "semantic.tipos.sistema",
        "semantic.escopos.variaveis",
        "semantic.programa.duas-passagens",
        "semantic.expressoes.verificacao",
        "semantic.chamadas.despacho",
        // Monomorfização/especialização no parser (Onda 5B).
        "parser.genericos.identidade-especializacao",
        "parser.genericos.substituicao-ast",
        "parser.genericos.funcoes-instanciacao",
        "parser.genericos.leques-instanciacao",
        "parser.callbacks.substituicao-estatica",
        "parser.callbacks.instanciacao-estatica",
        // Lowering AST → IR (Onda 5C); modelo/validador da Onda 3 preservados.
        "ir.modelo.representacao",
        "ir.validacao.invariantes",
        "ir.lowering.programa-orquestracao",
        "ir.lowering.contexto-declaracoes",
        "ir.lowering.assinaturas-intrinsecos",
        "ir.lowering.comandos-controle",
        "ir.lowering.expressoes-valores",
        "ir.lowering.bindings-escopos",
        "ir.tipos.conversao-ast",
        "ir.renderizacao.textual",
        // Lowering IR → CFG (Onda 5D); modelo/validador/lógica preservados.
        "cfg.modelo.representacao",
        "cfg.validacao.invariantes",
        "cfg.lowering.programa-orquestracao",
        "cfg.lowering.funcoes-blocos",
        "cfg.lowering.instrucoes-controle",
        "cfg.lowering.valores-temporarios",
        "cfg.lowering.memoria-indireta",
        "cfg.lowering.construcao-blocos",
        "cfg.renderizacao.programa",
        "cfg.renderizacao.componentes",
    ] {
        assert!(
            catalog.region(essential).is_some(),
            "chave essencial ausente: {essential}"
        );
    }
}

/// A camada `ir` separa modelo, lowering, validação, tipos e renderização em
/// domínios distintos (Onda 5C sobre o modelo da Onda 3). Verifica que o lowering
/// tem várias regiões próprias e que os domínios não colidem — sem fixar total.
#[test]
fn camada_ir_separa_modelo_lowering_e_renderizacao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let ir_by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| r.layer.as_deref() == Some("ir") && r.domain.as_deref() == Some(domain))
            .map(|r| r.key.as_str())
            .collect()
    };

    // O lowering é o grosso da Onda 5C: várias regiões próprias.
    assert!(
        ir_by_domain("lowering").len() >= 5,
        "domínio ir.lowering deveria ter várias regiões: {:?}",
        ir_by_domain("lowering")
    );
    // Modelo, validação, tipos e renderização existem como domínios distintos.
    for domain in ["modelo", "validacao", "tipos", "renderizacao"] {
        assert!(
            !ir_by_domain(domain).is_empty(),
            "domínio ir.{domain} ausente na camada ir"
        );
    }
    // O lowering não invade o domínio do modelo.
    assert!(
        ir_by_domain("lowering")
            .iter()
            .all(|k| !ir_by_domain("modelo").contains(k)),
        "lowering e modelo não podem compartilhar chaves"
    );
}

/// A camada `cfg` separa modelo, lowering, lógica, validação e renderização em
/// domínios distintos (Onda 5D sobre modelo/lógica históricos). Verifica que o
/// lowering tem várias regiões próprias, que as âncoras `cfg.logica.*` seguem no
/// domínio `logica` e que os domínios não colidem — sem fixar total.
#[test]
fn camada_cfg_separa_lowering_logica_e_renderizacao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let cfg_by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| r.layer.as_deref() == Some("cfg") && r.domain.as_deref() == Some(domain))
            .map(|r| r.key.as_str())
            .collect()
    };

    // O lowering é o grosso da Onda 5D: várias regiões próprias.
    assert!(
        cfg_by_domain("lowering").len() >= 5,
        "domínio cfg.lowering deveria ter várias regiões: {:?}",
        cfg_by_domain("lowering")
    );
    // Modelo, lógica, validação e renderização existem como domínios distintos.
    for domain in ["modelo", "logica", "validacao", "renderizacao"] {
        assert!(
            !cfg_by_domain(domain).is_empty(),
            "domínio cfg.{domain} ausente na camada cfg"
        );
    }
    // As duas âncoras históricas de curto-circuito seguem no domínio `logica`.
    let logica = cfg_by_domain("logica");
    for historica in ["cfg.logica.curto-circuito", "cfg.logica.slot-logico"] {
        assert!(
            logica.contains(&historica),
            "âncora histórica {historica} deveria permanecer no domínio logica"
        );
    }
    // O lowering não invade a lógica especializada preservada.
    assert!(
        cfg_by_domain("lowering")
            .iter()
            .all(|k| !logica.contains(k)),
        "lowering e logica não podem compartilhar chaves"
    );
}

/// Busca vertical por domínio: a Onda 5B distingue `genericos` (substituição de
/// parâmetros de tipo) de `callbacks` (especialização de parâmetros-função). Cada
/// domínio deve ter regiões próprias na camada `parser`, sem se confundirem.
#[test]
fn dominios_verticais_genericos_e_callbacks_distintos() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| r.layer.as_deref() == Some("parser") && r.domain.as_deref() == Some(domain))
            .map(|r| r.key.as_str())
            .collect()
    };

    let genericos = by_domain("genericos");
    let callbacks = by_domain("callbacks");
    assert!(
        genericos.len() >= 2,
        "domínio genericos deveria ter regiões próprias: {genericos:?}"
    );
    assert!(
        callbacks.len() >= 2,
        "domínio callbacks deveria ter regiões próprias: {callbacks:?}"
    );
    // Os dois domínios não se sobrepõem em chaves.
    assert!(
        genericos.iter().all(|k| !callbacks.contains(k)),
        "genericos e callbacks não podem compartilhar chaves"
    );
}
