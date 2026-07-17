//! Trama Pinker — cartografia semântica do código (§20 da cartografia).
//!
//! Cobre: múltiplas regiões no mesmo arquivo, preservação de âncoras
//! existentes, domínio/camada válidos e determinismo do catálogo. Onda 6D
//! acrescenta as raízes de código controladas (`trama.codigo.raizes`),
//! mantendo a separação entre catálogo, raízes e consulta. Onda 6E cartografa
//! o runtime nativo (`runtime/pinker_rt/src/lib.rs`, camada `runtime`),
//! concluindo a Onda 6. Onda 7 cartografa as superfícies operacionais:
//! `src/main.rs` (camada `cli`), `src/editor_tui.rs` (camada `editor`) e
//! `src/boot.rs` (camada `boot`).

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
    assert!(index.regions.iter().all(|r| r.file == "token.rs"));
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

#[test]
fn scanner_reconhece_apenas_marcadores_em_comentarios_reais() {
    let dir = temp_src("lexical");
    let false_start = "// @pinker-nav:start falso.literal.chave";
    let false_end = "// @pinker-nav:end falso.literal.chave";
    let source = [
        "let escaped = \"texto com \\\" e \\\\;",
        false_start,
        false_end,
        "ainda dentro da string\";",
        "let bytes = b\"",
        false_start,
        "\";",
        "let raw0 = r\"",
        false_start,
        "\";",
        "let raw1 = r#\"",
        false_start,
        "\"#;",
        "let raw2 = r##\"",
        false_start,
        "\"##;",
        "let raw_byte = br#\"",
        false_start,
        "\"#;",
        "/* comentário externo",
        false_start,
        "/* comentário aninhado",
        false_end,
        "*/",
        "*/",
        "/// @pinker-nav:start falso.doc.chave",
        "//! @pinker-nav:end falso.doc.chave",
        "let x = 1; // @pinker-nav:start falsa.depois.codigo",
        "// @pinker-nav:start verdadeiro.lexico.chave",
        "// @pinker-nav:domain teste",
        "// @pinker-nav:layer teste",
        "// @pinker-nav:summary Marcador canônico fora de literal.",
        "fn verdadeiro() {}",
        "// @pinker-nav:end verdadeiro.lexico.chave",
    ]
    .join("\n");
    write(&dir, "lexical.rs", &source);

    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.regions.len(), 1, "{:?}", index.scan_problems);
    assert_eq!(index.regions[0].key, "verdadeiro.lexico.chave");
    assert!(index.verify().is_empty(), "{:?}", index.verify());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn scanner_nao_confunde_lifetimes_com_literais_de_caractere() {
    let dir = temp_src("lifetimes");
    let source = [
        "fn f<'a>() { let _ = \"'\"; }",
        r#"fn chars() { let _ = 'a'; let _ = '\n'; let _ = '\\'; let _ = '\''; let _ = '"'; }"#,
        "// @pinker-nav:start teste.lifetime.primeiro",
        "// @pinker-nav:domain teste",
        "// @pinker-nav:layer teste",
        "fn primeiro() {}",
        "// @pinker-nav:end teste.lifetime.primeiro",
        "fn g<'a>() { let _: &'a str = \"it's\"; }",
        "// @pinker-nav:start teste.lifetime.segundo",
        "// @pinker-nav:domain teste",
        "// @pinker-nav:layer teste",
        "fn segundo() {}",
        "// @pinker-nav:end teste.lifetime.segundo",
    ]
    .join("\n");
    write(&dir, "lifetime.rs", &source);

    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.regions.len(), 2, "{:?}", index.scan_problems);
    assert!(index.region("teste.lifetime.primeiro").is_some());
    assert!(index.region("teste.lifetime.segundo").is_some());
    assert!(index.verify().is_empty(), "{:?}", index.verify());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn scanner_ignora_doc_comments_como_metadados_de_regiao() {
    let dir = temp_src("doc_comment_meta");
    let source = [
        "// @pinker-nav:start teste.meta.doc-comment",
        "// @pinker-nav:domain teste",
        "// @pinker-nav:layer teste",
        "// @pinker-nav:summary Resumo real.",
        "/// @pinker-nav:summary Resumo externo indevido.",
        "//! @pinker-nav:summary Resumo interno indevido.",
        "fn exemplo() {}",
        "// @pinker-nav:end teste.meta.doc-comment",
    ]
    .join("\n");
    write(&dir, "doc_comment.rs", &source);

    let index = CodeIndex::scan(&dir).unwrap();
    let region = index.region("teste.meta.doc-comment").unwrap();
    assert_eq!(region.summary, "Resumo real.");
    assert!(index.verify().is_empty(), "{:?}", index.verify());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn scanner_oficial_inclui_tests_sem_catalogar_fixtures_literais() {
    let dir = temp_src("official_tests_root");
    write(&dir, "src/lib.rs", "pub fn fonte() {}\n");
    write(
        &dir,
        "runtime/pinker_rt/src/lib.rs",
        "pub fn runtime() {}\n",
    );
    write(
        &dir,
        "tests/exemplo.rs",
        "// @pinker-nav:start tests.exemplo.real\n// @pinker-nav:domain teste\n// @pinker-nav:layer teste\n// @pinker-nav:summary Região real de fixture.\nfn exemplo() {}\n// @pinker-nav:end tests.exemplo.real\n",
    );
    let index = CodeIndex::scan_repo(&dir).unwrap();
    let region = index.region("tests.exemplo.real").unwrap();
    assert_eq!(region.file, "tests/exemplo.rs");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn scanner_do_repo_real_ignora_textos_de_fixture_nas_suites() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let index = CodeIndex::scan_repo(&root).unwrap();
    for file in [
        "tests/nav_catalog_tests.rs",
        "tests/nav_cartography_tests.rs",
    ] {
        assert!(
            !index.regions.iter().any(|region| region.file == file),
            "literal de fixture foi catalogado como região em {file}"
        );
    }
}

/// O catálogo real versionado contém as chaves essenciais do frontend (Onda 4),
/// da checagem semântica (Onda 5A), da monomorfização no parser (Onda 5B), do
/// lowering AST→IR (Onda 5C), do lowering IR→CFG (Onda 5D), da seleção→máquina
/// (Onda 5E), da execução hospedada (Onda 6A), do backend textual (Onda 6B), do
/// backend `.s` e ABI nativa (Onda 6C) e as âncoras históricas, todas únicas.
/// Verifica presença e unicidade — não um número exato permanente de regiões.
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
        // Seleção e máquina (Onda 5E); modelos/validadores preservados.
        "select.modelo.representacao",
        "select.validacao.invariantes",
        "select.lowering.programa-blocos",
        "select.lowering.instrucoes",
        "select.renderizacao.componentes",
        "machine.modelo.representacao",
        "machine.validacao.invariantes",
        "machine.lowering.instrucoes-pilha",
        "machine.lowering.terminadores",
        "machine.lowering.operandos-slots",
        "machine.renderizacao.apresentacao",
        // Execução hospedada / interpretador (Onda 6A).
        "interpreter.modelo.valores-estado",
        "interpreter.execucao.funcoes-fluxo",
        "interpreter.execucao.instrucoes-pilha",
        "interpreter.intrinsecos.listas",
        "interpreter.hospedeiro.servicos-auxiliares",
        "interpreter.diagnostico.stack-trace",
        // Backend textual (Onda 6B); validador preservado.
        "backend-text.validacao.invariantes",
        "backend-text.modelo.representacao",
        "backend-text.lowering.cfg-programa",
        "backend-text.lowering.selecao-programa",
        "backend-text.lowering.instrucoes-selecionadas",
        "backend-text.pipeline.emissao",
        "backend-text.renderizacao.programa",
        "backend-text.renderizacao.instrucoes",
        // Backend `.s` e ABI nativa (Onda 6C): superfície textual, modelo
        // externo, lowering externo, ABI, renderização e runtime.
        "backend-s.pipeline.textual-selecionado",
        "backend-s.pipeline.toolchain-externa",
        "backend-s.pipeline.nativo-runtime",
        "backend-s.validacao.subset-textual",
        "backend-s.modelo.callconv-externa",
        "backend-s.abi.registradores-argumentos",
        "backend-s.lowering.chamadas-sysv",
        "backend-s.abi.prologo-parametros",
        "backend-s.renderizacao.callconv-programa",
        "backend-s.runtime.simbolos-intrinsecas",
        "backend-s.renderizacao.abi-textual-programa",
        // Raízes de código controladas (Onda 6D).
        "trama.codigo.raizes",
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

/// As camadas `select` e `machine` (Onda 5E) separam modelo, lowering, validação e
/// renderização em domínios distintos, cada uma com várias regiões de lowering
/// próprias — sem fixar total.
#[test]
fn camadas_select_e_machine_separam_lowering_e_renderizacao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let by = |layer: &str, domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| r.layer.as_deref() == Some(layer) && r.domain.as_deref() == Some(domain))
            .map(|r| r.key.as_str())
            .collect()
    };

    for layer in ["select", "machine"] {
        assert!(
            by(layer, "lowering").len() >= 2,
            "camada {layer} deveria ter várias regiões de lowering: {:?}",
            by(layer, "lowering")
        );
        for domain in ["modelo", "validacao", "renderizacao"] {
            assert!(
                !by(layer, domain).is_empty(),
                "domínio {layer}.{domain} ausente"
            );
        }
        // O lowering não invade o domínio do modelo preservado.
        assert!(
            by(layer, "lowering")
                .iter()
                .all(|k| !by(layer, "modelo").contains(k)),
            "lowering e modelo de {layer} não podem compartilhar chaves"
        );
    }
}

/// A camada `interpreter` (Onda 6A) separa a execução hospedada em domínios
/// distintos — modelo, execução, intrínsecos, hospedeiro e diagnóstico —, cada um
/// com pelo menos uma região própria e sem chaves compartilhadas entre domínios.
/// Verifica presença e disjunção — sem fixar o total de regiões.
#[test]
fn camada_interpreter_separa_dominios_de_execucao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some("interpreter") && r.domain.as_deref() == Some(domain)
            })
            .map(|r| r.key.as_str())
            .collect()
    };

    let dominios = [
        "modelo",
        "execucao",
        "intrinsecos",
        "hospedeiro",
        "diagnostico",
    ];
    for domain in dominios {
        assert!(
            !by_domain(domain).is_empty(),
            "domínio interpreter.{domain} ausente"
        );
    }
    // Os domínios são disjuntos: nenhuma chave aparece em dois deles.
    for (i, a) in dominios.iter().enumerate() {
        for b in &dominios[i + 1..] {
            let da = by_domain(a);
            let db = by_domain(b);
            assert!(
                da.iter().all(|k| !db.contains(k)),
                "domínios interpreter.{a} e interpreter.{b} não podem compartilhar chaves"
            );
        }
    }
}

/// A camada `backend-text` (Onda 6B) separa modelo, lowering, pipeline,
/// renderização e validação em domínios distintos, com regiões próprias de
/// lowering e de renderização, preservando `backend-text.validacao.invariantes`.
/// Verifica presença e disjunção — sem fixar o total de regiões.
#[test]
fn camada_backend_text_separa_lowering_pipeline_e_renderizacao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some("backend-text") && r.domain.as_deref() == Some(domain)
            })
            .map(|r| r.key.as_str())
            .collect()
    };

    // Lowering e renderização têm mais de uma região próprias.
    assert!(
        by_domain("lowering").len() >= 2,
        "backend-text.lowering deveria ter várias regiões: {:?}",
        by_domain("lowering")
    );
    assert!(
        by_domain("renderizacao").len() >= 2,
        "backend-text.renderizacao deveria ter várias regiões: {:?}",
        by_domain("renderizacao")
    );
    // Modelo, pipeline e validação existem como domínios distintos.
    for domain in ["modelo", "pipeline", "validacao"] {
        assert!(
            !by_domain(domain).is_empty(),
            "domínio backend-text.{domain} ausente"
        );
    }
    // O lowering não invade a validação preservada.
    assert!(
        by_domain("lowering")
            .iter()
            .all(|k| !by_domain("validacao").contains(k)),
        "lowering e validacao de backend-text não podem compartilhar chaves"
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

/// A camada `backend-s` (Onda 6C) distingue as três entradas públicas
/// (`pipeline`), o modelo externo, a validação, o lowering externo, a ABI, a
/// renderização (montável e textual) e a integração com o runtime, em domínios
/// distintos e disjuntos. Cada um dos três caminhos públicos tem chave própria e
/// a representação `.s` textual (`renderizacao.abi-textual-*`) é separada do
/// renderer montável (`renderizacao.callconv-programa`). Verifica presença e
/// disjunção — sem fixar o total de regiões.
#[test]
fn camada_backend_s_separa_pipelines_lowering_abi_e_renderizacao() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let by_domain = |domain: &str| -> Vec<&str> {
        catalog
            .regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some("backend-s") && r.domain.as_deref() == Some(domain)
            })
            .map(|r| r.key.as_str())
            .collect()
    };

    // Os oito domínios da onda existem, cada um com ao menos uma região própria.
    let dominios = [
        "pipeline",
        "modelo",
        "validacao",
        "lowering",
        "abi",
        "renderizacao",
        "runtime",
        "dados",
    ];
    for domain in dominios {
        assert!(
            !by_domain(domain).is_empty(),
            "domínio backend-s.{domain} ausente"
        );
    }

    // Os domínios são disjuntos: nenhuma chave aparece em dois deles.
    for (i, a) in dominios.iter().enumerate() {
        for b in &dominios[i + 1..] {
            let da = by_domain(a);
            let db = by_domain(b);
            assert!(
                da.iter().all(|k| !db.contains(k)),
                "domínios backend-s.{a} e backend-s.{b} não podem compartilhar chaves"
            );
        }
    }

    // As três entradas públicas são caminhos distintos, com chave própria.
    let pipeline = by_domain("pipeline");
    for entrada in [
        "backend-s.pipeline.textual-selecionado",
        "backend-s.pipeline.toolchain-externa",
        "backend-s.pipeline.nativo-runtime",
    ] {
        assert!(
            pipeline.contains(&entrada),
            "entrada pública {entrada} ausente no domínio pipeline"
        );
    }

    // O lowering externo tem várias regiões próprias.
    assert!(
        by_domain("lowering").len() >= 4,
        "backend-s.lowering deveria ter várias regiões: {:?}",
        by_domain("lowering")
    );

    // A representação `.s` textual (baseada em `BackendTextProgram`) é separada do
    // renderer montável (baseado em `ExternalCallConvProgram`).
    let render = by_domain("renderizacao");
    assert!(
        render.contains(&"backend-s.renderizacao.callconv-programa"),
        "renderer montável ausente"
    );
    assert!(
        render
            .iter()
            .any(|k| k.starts_with("backend-s.renderizacao.abi-textual-")),
        "renderer `.s` textual ausente"
    );
}

/// A camada `trama` (navegação) separa catálogo, raízes controladas e
/// consulta em chaves próprias e disjuntas (Onda 6D introduz
/// `trama.codigo.raizes` sem sobrepor `trama.codigo.catalogo` ou
/// `trama.codigo.consulta`). Não fixa o total global do catálogo.
#[test]
fn camada_trama_separa_catalogo_raizes_e_consulta() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    for essential in [
        "trama.codigo.catalogo",
        "trama.codigo.raizes",
        "trama.codigo.consulta",
    ] {
        assert!(
            catalog.region(essential).is_some(),
            "chave essencial de navegação ausente: {essential}"
        );
    }
}

/// A camada `runtime` (Onda 6E) cartografa `runtime/pinker_rt/src/lib.rs`
/// (produção; `#[cfg(test)] mod tests` fica de fora, por decisão explícita da
/// cápsula). Verifica as 15 chaves planejadas, a contagem exata de 15 regiões
/// `runtime` e que todas apontam para o arquivo do runtime nativo — nenhuma
/// para `src/`. Não fixa o total global do catálogo (§18.4).
#[test]
fn camada_runtime_cartografa_o_runtime_nativo() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let expected_runtime_keys = [
        "runtime.inicializacao.bootstrap",
        "runtime.memoria.alocador",
        "runtime.texto.operacoes",
        "runtime.conversoes.numero-texto",
        "runtime.texto.formatacao",
        "runtime.io.saida",
        "runtime.listas.dinamicas",
        "runtime.mapas.dinamicos",
        "runtime.leques.variantes",
        "runtime.arquivos.io",
        "runtime.caminhos.sistema",
        "runtime.tempo.relogio",
        "runtime.aleatorio.gerador",
        "runtime.ambiente.argumentos",
        "runtime.processos.execucao",
    ];

    for key in expected_runtime_keys {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave runtime ausente no catálogo: {key}"));
        assert_eq!(
            region.file, "runtime/pinker_rt/src/lib.rs",
            "chave runtime '{key}' deveria apontar para runtime/pinker_rt/src/lib.rs"
        );
    }

    let runtime_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|r| r.layer.as_deref() == Some("runtime"))
        .collect();

    let expected_runtime_count = expected_runtime_keys.len();
    assert_eq!(
        runtime_regions.len(),
        expected_runtime_count,
        "esperava exatamente {expected_runtime_count} regiões na camada runtime"
    );

    assert!(
        runtime_regions
            .iter()
            .all(|r| r.file == "runtime/pinker_rt/src/lib.rs"),
        "toda região da camada runtime deve apontar para runtime/pinker_rt/src/lib.rs"
    );
    assert!(
        runtime_regions.iter().all(|r| !r.file.starts_with("src/")),
        "nenhuma região da camada runtime deve apontar para src/"
    );

    // Confirma a presença dos domínios principais do runtime nativo.
    for domain in [
        "inicializacao",
        "memoria",
        "listas",
        "mapas",
        "leques",
        "io",
        "arquivos",
        "caminhos",
        "processos",
    ] {
        assert!(
            runtime_regions
                .iter()
                .any(|r| r.domain.as_deref() == Some(domain)),
            "domínio runtime esperado ausente: {domain}"
        );
    }
}

/// A Onda 7 cartografa as superfícies operacionais: `src/main.rs` (camada
/// `cli`, 15 regiões), `src/editor_tui.rs` (camada `editor`, 4 regiões) e
/// `src/boot.rs` (camada `boot`, 1 região). Verifica as 20 chaves planejadas,
/// a contagem exata por camada, que toda região `cli` aponta para
/// `src/main.rs`, toda `editor` para `src/editor_tui.rs` e toda `boot` para
/// `src/boot.rs` — sem cruzamento entre os três arquivos —, domínios
/// representativos por camada, e que uma amostra de chaves essenciais das
/// ondas anteriores (0-6E) permanece presente e fora de cli/editor/boot (ou
/// seja, nenhuma camada preexistente foi reclassificada). Não fixa o total
/// global do catálogo como invariante permanente (§18.4); usa um piso mínimo
/// compatível com o crescimento de ondas futuras.
#[test]
fn camada_operacional_cartografa_cli_editor_boot() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let expected_cli_keys = [
        "cli.config.modelos",
        "cli.ajuda.usage",
        "cli.parsing.subcomandos",
        "cli.parsing.roteamento",
        "cli.execucao.entrada",
        "cli.nav.consulta",
        "cli.nav.sincronizacao-verificacao",
        "cli.doc.consulta",
        "cli.doc.sincronizacao",
        "cli.doc.mudancas",
        "cli.doc.verificacao",
        "cli.execucao.editor-repl",
        "cli.analise.pipeline",
        "cli.build.nativo",
        "cli.modulos.importacao",
    ];
    let expected_editor_keys = [
        "editor.estado.modelo",
        "editor.sessao.comandos",
        "editor.render.saida",
        "editor.analise.checagem",
    ];
    let expected_boot_keys = ["boot.geracao.fronteira-freestanding"];

    for key in expected_cli_keys {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave cli ausente no catálogo: {key}"));
        assert_eq!(
            region.file, "src/main.rs",
            "chave cli '{key}' deveria apontar para src/main.rs"
        );
    }
    for key in expected_editor_keys {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave editor ausente no catálogo: {key}"));
        assert_eq!(
            region.file, "src/editor_tui.rs",
            "chave editor '{key}' deveria apontar para src/editor_tui.rs"
        );
    }
    for key in expected_boot_keys {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave boot ausente no catálogo: {key}"));
        assert_eq!(
            region.file, "src/boot.rs",
            "chave boot '{key}' deveria apontar para src/boot.rs"
        );
    }

    let cli_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|r| r.layer.as_deref() == Some("cli"))
        .collect();
    let editor_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|r| r.layer.as_deref() == Some("editor"))
        .collect();
    let boot_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|r| r.layer.as_deref() == Some("boot"))
        .collect();

    assert_eq!(
        cli_regions.len(),
        expected_cli_keys.len(),
        "esperava exatamente {} regiões na camada cli",
        expected_cli_keys.len()
    );
    assert_eq!(
        editor_regions.len(),
        expected_editor_keys.len(),
        "esperava exatamente {} regiões na camada editor",
        expected_editor_keys.len()
    );
    assert_eq!(
        boot_regions.len(),
        expected_boot_keys.len(),
        "esperava exatamente {} regiões na camada boot",
        expected_boot_keys.len()
    );

    assert!(
        cli_regions.iter().all(|r| r.file == "src/main.rs"),
        "toda região da camada cli deve apontar para src/main.rs"
    );
    assert!(
        editor_regions.iter().all(|r| r.file == "src/editor_tui.rs"),
        "toda região da camada editor deve apontar para src/editor_tui.rs"
    );
    assert!(
        boot_regions.iter().all(|r| r.file == "src/boot.rs"),
        "toda região da camada boot deve apontar para src/boot.rs"
    );

    // Sem cruzamento entre os arquivos das três camadas novas.
    assert!(
        cli_regions
            .iter()
            .all(|r| r.file != "src/editor_tui.rs" && r.file != "src/boot.rs"),
        "camada cli não deve cruzar com src/editor_tui.rs/src/boot.rs"
    );
    assert!(
        editor_regions
            .iter()
            .all(|r| r.file != "src/main.rs" && r.file != "src/boot.rs"),
        "camada editor não deve cruzar com src/main.rs/src/boot.rs"
    );
    assert!(
        boot_regions
            .iter()
            .all(|r| r.file != "src/main.rs" && r.file != "src/editor_tui.rs"),
        "camada boot não deve cruzar com src/main.rs/src/editor_tui.rs"
    );

    // Domínios representativos por camada (amostra, não exaustivo).
    for domain in [
        "config", "ajuda", "parsing", "execucao", "nav", "doc", "analise", "build", "modulos",
    ] {
        assert!(
            cli_regions
                .iter()
                .any(|r| r.domain.as_deref() == Some(domain)),
            "domínio cli esperado ausente: {domain}"
        );
    }
    for domain in ["estado", "sessao", "render", "analise"] {
        assert!(
            editor_regions
                .iter()
                .any(|r| r.domain.as_deref() == Some(domain)),
            "domínio editor esperado ausente: {domain}"
        );
    }
    assert!(
        boot_regions
            .iter()
            .any(|r| r.domain.as_deref() == Some("geracao")),
        "domínio boot esperado ausente: geracao"
    );

    // Amostra de chaves essenciais das ondas anteriores (0-6E): permanecem
    // presentes e continuam fora de cli/editor/boot — nenhuma camada
    // preexistente foi reclassificada por esta cápsula.
    let previous_sample = [
        "lexer.fluxo.tokenizacao",
        "parser.fluxo.nucleo",
        "semantic.programa.duas-passagens",
        "ir.modelo.representacao",
        "cfg.modelo.representacao",
        "select.modelo.representacao",
        "machine.modelo.representacao",
        "interpreter.modelo.valores-estado",
        "backend-text.modelo.representacao",
        "backend-s.pipeline.textual-selecionado",
        "trama.codigo.raizes",
        "runtime.inicializacao.bootstrap",
    ];
    for key in previous_sample {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave anterior deveria permanecer no catálogo: {key}"));
        let layer = region.layer.as_deref();
        assert!(
            layer != Some("cli") && layer != Some("editor") && layer != Some("boot"),
            "chave anterior '{key}' não deveria ter sido reclassificada para cli/editor/boot"
        );
    }

    // Piso mínimo compatível com o crescimento de ondas futuras (não fixa o
    // total global como teto).
    let new_total = expected_cli_keys.len() + expected_editor_keys.len() + expected_boot_keys.len();
    assert!(
        catalog.regions.len() >= previous_sample.len() + new_total,
        "catálogo deveria conter ao menos as regiões desta onda além das anteriores"
    );
}
