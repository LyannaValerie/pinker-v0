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
use std::collections::HashSet;
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
fn scanner_exige_prefixo_estrito_para_metadados() {
    let dir = temp_src("meta_prefixo_estrito");
    let source = [
        "// @pinker-nav:start teste.meta.prefixo-estrito",
        "// @pinker-nav:domain teste",
        "// @pinker-nav:layer teste",
        "// @pinker-nav:summary Resumo verdadeiro.",
        "// Nota documental: @pinker-nav:summary Resumo falso.",
        "// texto anterior @pinker-nav:domain falso",
        "fn exemplo() {}",
        "// @pinker-nav:end teste.meta.prefixo-estrito",
    ]
    .join("\n");
    write(&dir, "meta_prefixo_estrito.rs", &source);

    let index = CodeIndex::scan(&dir).unwrap();
    assert_eq!(index.regions.len(), 1, "{:?}", index.scan_problems);
    let region = index.region("teste.meta.prefixo-estrito").unwrap();
    assert_eq!(region.key, "teste.meta.prefixo-estrito");
    assert_eq!(region.summary, "Resumo verdadeiro.");
    assert_eq!(region.domain.as_deref(), Some("teste"));
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

#[test]
fn camada_evidencia_frontend_cartografa_lexer_parser_common() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let expected_common_keys = ["evidencia.frontend.pipeline-basico"];
    let expected_lexer_keys = [
        "evidencia.lexico.tokens-e-spans",
        "evidencia.lexico.diagnostico",
        "evidencia.lexico.palavras-controle",
        "evidencia.lexico.operadores",
        "evidencia.lexico.tipos-fixos",
        "evidencia.lexico.palavras-de-construcao",
        "evidencia.lexico.arrays-acessos-e-modificadores",
    ];
    let expected_parser_keys = [
        "evidencia.parser.ast-basica-e-spans",
        "evidencia.parser.diagnostico-e-limites-literais",
        "evidencia.parser.controle-de-fluxo",
        "evidencia.parser.desugaring-para-cada",
        "evidencia.parser.diretivas-topo-e-asm-inline",
        "evidencia.parser.tipos-qualificados-e-verso",
        "evidencia.parser.expressoes-e-precedencia",
        "evidencia.parser.postfix-cast-deref-e-operadores-tipo",
        "evidencia.parser.tipos-numericos",
        "evidencia.parser.aliases-arrays-e-structs",
        "evidencia.parser.ponteiros-e-colecoes",
    ];

    assert_eq!(expected_common_keys.len(), 1);
    assert_eq!(expected_lexer_keys.len(), 7);
    assert_eq!(expected_parser_keys.len(), 11);
    assert_eq!(
        expected_common_keys.len() + expected_lexer_keys.len() + expected_parser_keys.len(),
        19
    );

    let mut planned_keys = HashSet::new();
    for (keys, file, domain) in [
        (&expected_common_keys[..], "tests/common/mod.rs", "frontend"),
        (&expected_lexer_keys[..], "tests/lexer_tests.rs", "lexico"),
        (&expected_parser_keys[..], "tests/parser_tests.rs", "parser"),
    ] {
        for &key in keys {
            assert!(
                planned_keys.insert(key),
                "chave de evidência repetida no plano da Onda 8B: {key}"
            );
            let region = catalog
                .region(key)
                .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
            assert_eq!(
                region.file, file,
                "chave '{key}' deveria apontar para {file}"
            );
            assert_eq!(
                region.layer.as_deref(),
                Some("evidencia"),
                "chave '{key}' deveria usar a camada evidencia"
            );
            assert_eq!(
                region.domain.as_deref(),
                Some(domain),
                "chave '{key}' deveria usar o domínio {domain}"
            );
            assert!(
                region.start_marker < region.content_start
                    && region.content_start <= region.content_end
                    && region.content_end < region.end_marker,
                "chave '{key}' deveria ter marcadores ordenados"
            );
        }
    }
    assert_eq!(
        planned_keys.len(),
        19,
        "o plano da Onda 8B perdeu uma chave"
    );

    let common_source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/common/mod.rs");
    let common_source = fs::read_to_string(&common_source_path).unwrap_or_else(|error| {
        panic!(
            "não foi possível ler {}: {error}",
            common_source_path.display()
        )
    });
    let common_region = catalog
        .region(expected_common_keys[0])
        .expect("região de helpers comuns deveria existir");
    let numbered_common_lines: Vec<_> = common_source.lines().enumerate().collect();
    let public_helpers_in_region: Vec<_> = numbered_common_lines
        .iter()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            let name = line.trim().strip_prefix("pub fn ")?.split_once('(')?.0;
            (common_region.content_start <= line_number && line_number <= common_region.content_end)
                .then_some(name)
        })
        .collect();
    assert_eq!(
        public_helpers_in_region,
        ["tokenize", "parse", "parse_and_check"],
        "a região comum deve conter exatamente os três helpers públicos básicos"
    );
    let render_ast_line = numbered_common_lines
        .iter()
        .find_map(|(index, line)| {
            (line.trim().starts_with("pub fn render_ast(")).then_some(index + 1)
        })
        .expect("helper render_ast deveria existir");
    assert!(
        common_region.content_end < render_ast_line,
        "render_ast deve ficar fora da região de helpers básicos"
    );

    let coverage_for = |file: &str, keys: &[&str], expected_count: usize| {
        let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
        let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("não foi possível ler {}: {error}", source_path.display())
        });
        let lines: Vec<_> = source.lines().collect();
        let mut owned_test_counts = vec![0usize; keys.len()];
        let mut test_count = 0usize;

        for (attribute_index, line) in lines.iter().enumerate() {
            if line.trim() != "#[test]" {
                continue;
            }

            let test_line = attribute_index + 1;
            let test_name = lines
                .iter()
                .skip(attribute_index + 1)
                .take(8)
                .find_map(|candidate| {
                    candidate
                        .trim()
                        .strip_prefix("fn ")?
                        .split_once('(')
                        .map(|(name, _)| name.trim())
                })
                .unwrap_or_else(|| {
                    panic!("structural_test_function_not_found: arquivo {file}, linha {test_line}")
                });
            let owners: Vec<_> = keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| {
                    let region = catalog
                        .region(key)
                        .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
                    (region.content_start <= test_line && test_line <= region.content_end)
                        .then_some((index, *key))
                })
                .collect();

            match owners.as_slice() {
                [] => panic!(
                    "structural_test_region_not_found: arquivo {file}, linha {test_line}, função {test_name}"
                ),
                [(index, _)] => owned_test_counts[*index] += 1,
                _ => panic!(
                    "structural_test_region_ambiguous: arquivo {file}, linha {test_line}, função {test_name}, proprietárias {:?}",
                    owners.iter().map(|(_, key)| *key).collect::<Vec<_>>()
                ),
            }
            test_count += 1;
        }

        assert_eq!(
            test_count, expected_count,
            "contagem de #[test] inesperada em {file}"
        );
        for (key, owned_test_count) in keys.iter().zip(owned_test_counts) {
            assert!(
                owned_test_count >= 1,
                "região '{key}' deveria possuir ao menos um #[test] em {file}"
            );
        }
        test_count
    };

    let lexer_test_count = coverage_for("tests/lexer_tests.rs", &expected_lexer_keys, 25);
    let parser_test_count = coverage_for("tests/parser_tests.rs", &expected_parser_keys, 36);
    assert_eq!(lexer_test_count, 25);
    assert_eq!(parser_test_count, 36);
    assert_eq!(lexer_test_count + parser_test_count, 61);

    let previous_sample = catalog
        .region("lexer.fluxo.tokenizacao")
        .expect("amostra de chave anterior deveria permanecer no catálogo");
    assert_ne!(
        previous_sample.layer.as_deref(),
        Some("evidencia"),
        "a amostra anterior não deveria ser reclassificada para evidencia"
    );

    assert!(
        catalog.regions.len() >= 202,
        "catálogo deveria conter ao menos 202 regiões após a Onda 8B"
    );
}

#[test]
fn onda_8c_cartografa_evidencias_semanticas() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");
    let expected_semantic_keys = [
        "evidencia.semantica.entrada-principal",
        "evidencia.semantica.retornos",
        "evidencia.semantica.mutabilidade",
        "evidencia.semantica.chamadas",
        "evidencia.semantica.intrinsecas-entrada-ambiente",
        "evidencia.semantica.intrinsecas-caminhos-e-sistema",
        "evidencia.semantica.intrinsecas-argumentos-e-contexto",
        "evidencia.semantica.intrinsecas-arquivos-io",
        "evidencia.semantica.intrinsecas-texto-e-estruturados",
        "evidencia.semantica.intrinsecas-processos",
        "evidencia.semantica.funcoes-sem-retorno",
        "evidencia.semantica.controle-fluxo-e-diagnostico",
        "evidencia.semantica.operadores-logicos-e-bitwise",
        "evidencia.semantica.acesso-campos-e-indexacao",
        "evidencia.semantica.casts",
        "evidencia.semantica.peso-e-alinhamento",
        "evidencia.semantica.tipos-numericos-largura-fixa",
        "evidencia.semantica.aliases-arrays-e-ninhos",
        "evidencia.semantica.ponteiros-e-aritmetica",
        "evidencia.semantica.ninhos-diagnostico",
        "evidencia.semantica.aritmetica-modulo-e-literais",
        "evidencia.semantica.escrita-por-indice",
        "evidencia.semantica.listas",
        "evidencia.semantica.mapas",
        "evidencia.semantica.acaso",
        "evidencia.semantica.imports-por-familia",
        "evidencia.semantica.leques-simples",
        "evidencia.semantica.leques-com-carga",
        "evidencia.semantica.encaixe-e-bindings",
        "evidencia.semantica.leques-recursivos-e-multiplas-cargas",
        "evidencia.semantica.genericos",
        "evidencia.semantica.tratamento-de-erro",
        "evidencia.semantica.funcoes-locais-e-carinho",
        "evidencia.semantica.tratos-e-impls",
    ];
    assert_eq!(expected_semantic_keys.len(), 34);
    let mut planned_keys = HashSet::new();
    for key in expected_semantic_keys {
        assert!(
            planned_keys.insert(key),
            "chave repetida no plano da Onda 8C: {key}"
        );
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave ausente: {key}"));
        assert_eq!(region.file, "tests/semantic_tests.rs");
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.domain.as_deref(), Some("semantica"));
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "marcadores fora de ordem: {key}"
        );
    }

    let file = "tests/semantic_tests.rs";
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", source_path.display()));
    let lines: Vec<_> = source.lines().collect();
    let mut owned_test_counts = vec![0usize; expected_semantic_keys.len()];
    let mut test_count = 0usize;
    for (attribute_index, line) in lines.iter().enumerate() {
        if line.trim() != "#[test]" {
            continue;
        }
        let test_line = attribute_index + 1;
        let test_name = lines
            .iter()
            .skip(attribute_index + 1)
            .take(8)
            .find_map(|candidate| {
                candidate
                    .trim()
                    .strip_prefix("fn ")?
                    .split_once('(')
                    .map(|(name, _)| name.trim())
            })
            .unwrap_or_else(|| {
                panic!("structural_test_function_not_found: arquivo {file}, linha {test_line}")
            });
        let owners: Vec<_> = expected_semantic_keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| {
                let region = catalog
                    .region(key)
                    .unwrap_or_else(|| panic!("chave ausente: {key}"));
                (region.content_start <= test_line && test_line <= region.content_end)
                    .then_some((index, *key))
            })
            .collect();
        match owners.as_slice() {
            [(index, _)] => owned_test_counts[*index] += 1,
            [] => panic!(
                "structural_test_region_not_found: arquivo {file}, linha {test_line}, função {test_name}, chaves {:?}",
                expected_semantic_keys
            ),
            _ => panic!(
                "structural_test_region_ambiguous: arquivo {file}, linha {test_line}, função {test_name}, chaves {:?}",
                owners.iter().map(|(_, key)| *key).collect::<Vec<_>>()
            ),
        }
        test_count += 1;
    }
    assert_eq!(test_count, 340, "contagem de #[test] inesperada em {file}");
    for (key, count) in expected_semantic_keys.iter().zip(owned_test_counts) {
        assert!(
            count >= 1,
            "região '{key}' deveria possuir ao menos um #[test]"
        );
    }
    assert!(
        catalog.regions.len() >= 236,
        "catálogo deveria conter ao menos 236 regiões"
    );
}

#[test]
fn onda_8d_cartografa_evidencias_do_pipeline() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    // Chaves em ordem física: primeiro tests/ir_tests.rs, depois tests/ir_validate_tests.rs.
    let expected_ir_keys = [
        "evidencia.ir.lowering-programa",
        "evidencia.ir.renderizacao-estruturas-basicas",
        "evidencia.ir.renderizacao-cli",
        "evidencia.ir.lowering-controle-de-laco",
        "evidencia.ir.lowering-operacoes-textuais",
        "evidencia.ir.lowering-tipos-numericos",
        "evidencia.ir.lowering-tipos-compostos",
        "evidencia.ir.validacao-aceitacao-basica",
        "evidencia.ir.validacao-retorno-e-condicao",
        "evidencia.ir.validacao-chamadas-e-nulo",
        "evidencia.ir.validacao-estrutura-e-diagnostico",
    ];
    // Ordem física: primeiro tests/cfg_ir_tests.rs, depois tests/cfg_ir_validate_tests.rs.
    let expected_cfg_keys = [
        "evidencia.cfg.lowering-e-renderizacao-basica",
        "evidencia.cfg.renderizacao-cli",
        "evidencia.cfg.lowering-lacos",
        "evidencia.cfg.lowering-operadores-e-join",
        "evidencia.cfg.lowering-ponteiros-e-agregados",
        "evidencia.cfg.lowering-limite-asm",
        "evidencia.cfg.lowering-verso",
        "evidencia.cfg.lowering-curto-circuito",
        "evidencia.cfg.validacao-aceitacao-basica",
        "evidencia.cfg.validacao-blocos-e-alvos",
        "evidencia.cfg.validacao-condicao-e-retorno",
        "evidencia.cfg.validacao-chamada-e-referencias",
        "evidencia.cfg.validacao-alcancabilidade-e-renderizacao",
        "evidencia.cfg.validacao-diagnostico",
    ];
    let expected_select_keys = [
        "evidencia.select.blocos-e-terminadores",
        "evidencia.select.chamadas-e-operadores",
        "evidencia.select.renderizacao-cli",
        "evidencia.select.rejeicao-call-sem-destino",
        "evidencia.select.fluxos-de-laco",
        "evidencia.select.operadores-bitwise-e-modulo",
    ];
    // Ordem física: primeiro tests/abstract_machine_tests.rs, depois
    // tests/abstract_machine_stack_tests.rs.
    let expected_machine_keys = [
        "evidencia.machine.lowering-blocos-e-terminadores",
        "evidencia.machine.lowering-chamadas",
        "evidencia.machine.lowering-operadores-e-temporarios",
        "evidencia.machine.renderizacao-cli",
        "evidencia.machine.comparacao-representacoes",
        "evidencia.machine.validacao-programa-e-slots",
        "evidencia.machine.lowering-bitwise-e-modulo",
        "evidencia.machine.renderizacao-slots-e-temporarios",
        "evidencia.machine.renderizacao-chamadas",
        "evidencia.machine.renderizacao-terminadores-e-fluxos",
        "evidencia.machine.renderizacao-papeis-de-blocos",
        "evidencia.machine.renderizacao-programa-valido",
        "evidencia.machine.validacao-underflow-operadores",
        "evidencia.machine.validacao-chamadas-aridade-e-underflow",
        "evidencia.machine.validacao-formato-diagnostico",
        "evidencia.machine.validacao-branch",
        "evidencia.machine.renderizacao-branch-valido",
        "evidencia.machine.validacao-retorno",
        "evidencia.machine.renderizacao-retorno-valido",
        "evidencia.machine.validacao-pilha-retvoid-e-merges",
        "evidencia.machine.validacao-slots-existencia",
        "evidencia.machine.validacao-slots-tipados",
        "evidencia.machine.validacao-tipos-operacoes-e-retorno",
        "evidencia.machine.validacao-tipos-chamadas",
        "evidencia.machine.renderizacao-casos-validos",
        "evidencia.machine.validacao-programa-invalido",
        "evidencia.machine.renderizacao-cli-golden",
    ];

    assert_eq!(expected_ir_keys.len(), 11);
    assert_eq!(expected_cfg_keys.len(), 14);
    assert_eq!(expected_select_keys.len(), 6);
    assert_eq!(expected_machine_keys.len(), 27);
    assert_eq!(
        expected_ir_keys.len()
            + expected_cfg_keys.len()
            + expected_select_keys.len()
            + expected_machine_keys.len(),
        58,
        "a Onda 8D deveria planejar exatamente 58 regiões novas"
    );

    // Cada arquivo recebe a fatia física correspondente de sua trilha/domínio, com a
    // contagem de #[test] congelada no plano.
    let per_file: [(&[&str], &str, &str, usize); 7] = [
        (&expected_ir_keys[..7], "tests/ir_tests.rs", "ir", 25),
        (
            &expected_ir_keys[7..],
            "tests/ir_validate_tests.rs",
            "ir",
            7,
        ),
        (&expected_cfg_keys[..8], "tests/cfg_ir_tests.rs", "cfg", 23),
        (
            &expected_cfg_keys[8..],
            "tests/cfg_ir_validate_tests.rs",
            "cfg",
            15,
        ),
        (
            &expected_select_keys[..],
            "tests/instr_select_tests.rs",
            "select",
            12,
        ),
        (
            &expected_machine_keys[..11],
            "tests/abstract_machine_tests.rs",
            "machine",
            23,
        ),
        (
            &expected_machine_keys[11..],
            "tests/abstract_machine_stack_tests.rs",
            "machine",
            29,
        ),
    ];

    let mut planned_keys = HashSet::new();
    let mut total_test_count = 0usize;

    for (keys, file, domain, expected_count) in per_file {
        // Unicidade global das chaves e coerência de arquivo/camada/domínio/marcadores.
        for &key in keys {
            assert!(
                planned_keys.insert(key),
                "chave de evidência repetida no plano da Onda 8D: {key}"
            );
            let region = catalog
                .region(key)
                .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
            assert_eq!(
                region.file, file,
                "chave '{key}' deveria apontar para {file}"
            );
            assert_eq!(
                region.layer.as_deref(),
                Some("evidencia"),
                "chave '{key}' deveria usar a camada evidencia"
            );
            assert_eq!(
                region.domain.as_deref(),
                Some(domain),
                "chave '{key}' deveria usar o domínio {domain}"
            );
            assert!(
                region.start_marker < region.content_start
                    && region.content_start <= region.content_end
                    && region.content_end < region.end_marker,
                "chave '{key}' deveria ter marcadores ordenados"
            );
        }

        // Cobertura estrutural: todo #[test] pertence a exatamente uma região da trilha.
        let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
        let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("não foi possível ler {}: {error}", source_path.display())
        });
        let lines: Vec<_> = source.lines().collect();
        let mut owned_test_counts = vec![0usize; keys.len()];
        let mut test_count = 0usize;

        for (attribute_index, line) in lines.iter().enumerate() {
            if line.trim() != "#[test]" {
                continue;
            }
            let test_line = attribute_index + 1;
            let test_name = lines
                .iter()
                .skip(attribute_index + 1)
                .take(8)
                .find_map(|candidate| {
                    candidate
                        .trim()
                        .strip_prefix("fn ")?
                        .split_once('(')
                        .map(|(name, _)| name.trim())
                })
                .unwrap_or_else(|| {
                    panic!("structural_test_function_not_found: arquivo {file}, linha {test_line}")
                });
            let owners: Vec<_> = keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| {
                    let region = catalog
                        .region(key)
                        .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
                    (region.content_start <= test_line && test_line <= region.content_end)
                        .then_some((index, *key))
                })
                .collect();
            match owners.as_slice() {
                [(index, _)] => owned_test_counts[*index] += 1,
                [] => panic!(
                    "structural_test_region_not_found: arquivo {file}, linha {test_line}, função {test_name}"
                ),
                _ => panic!(
                    "structural_test_region_ambiguous: arquivo {file}, linha {test_line}, função {test_name}, proprietárias {:?}",
                    owners.iter().map(|(_, key)| *key).collect::<Vec<_>>()
                ),
            }
            test_count += 1;
        }

        assert_eq!(
            test_count, expected_count,
            "contagem de #[test] inesperada em {file}"
        );
        for (key, owned) in keys.iter().zip(owned_test_counts) {
            assert!(
                owned >= 1,
                "região '{key}' deveria possuir ao menos um #[test] em {file}"
            );
        }
        total_test_count += test_count;
    }

    assert_eq!(
        planned_keys.len(),
        58,
        "o plano da Onda 8D perdeu uma chave"
    );
    assert_eq!(
        total_test_count, 134,
        "a Onda 8D deveria cobrir exatamente 134 testes (M_TOTAL)"
    );

    // As 53 regiões de evidência anteriores devem permanecer: total de evidência = 53 + 58.
    let evidence_total = catalog
        .regions
        .iter()
        .filter(|region| region.layer.as_deref() == Some("evidencia"))
        .count();
    assert!(
        evidence_total >= 111,
        "catálogo deveria conter ao menos 111 regiões de evidência (53 anteriores + 58 da Onda 8D), obteve {evidence_total}"
    );
    for previous in [
        "evidencia.lexico.tokens-e-spans",
        "evidencia.parser.ast-basica-e-spans",
        "evidencia.semantica.entrada-principal",
    ] {
        let region = catalog
            .region(previous)
            .unwrap_or_else(|| panic!("região de evidência anterior ausente: {previous}"));
        assert_eq!(
            region.layer.as_deref(),
            Some("evidencia"),
            "região anterior '{previous}' deveria permanecer como evidencia"
        );
    }

    assert!(
        catalog.regions.len() >= 294,
        "catálogo deveria conter ao menos 294 regiões após a Onda 8D"
    );
}

#[test]
fn onda_8e_cartografa_evidencias_da_execucao_interpretada() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    // A Onda 8E cartografa tests/interpreter_tests.rs (evidências da execução
    // interpretada da Pinker) em 46 regiões de evidência no domínio `interpreter`.
    // As contagens de #[test] por região estão congeladas no plano e a ordem
    // física da suíte é preservada. Chaves em ordem alfabética para diff estável.
    let expected_interpreter_keys: [(&str, usize); 46] = [
        ("evidencia.interpreter.aleatoriedade-semente", 8),
        ("evidencia.interpreter.arquivos-csv-json-cli-exemplos", 7),
        ("evidencia.interpreter.arquivos-csv-serializacao", 6),
        (
            "evidencia.interpreter.arquivos-e-ambiente-fallback-cli-exemplos",
            12,
        ),
        (
            "evidencia.interpreter.arquivos-handle-fechado-e-fluxo-completo",
            11,
        ),
        (
            "evidencia.interpreter.arquivos-introspeccao-caminho-e-diretorios",
            23,
        ),
        ("evidencia.interpreter.arquivos-json-serializacao", 7),
        (
            "evidencia.interpreter.checagem-cli-modulos-e-recortes-linguagem",
            20,
        ),
        ("evidencia.interpreter.colecoes-iteracao-lista-e-mapa", 18),
        ("evidencia.interpreter.colecoes-lista-bombom", 17),
        ("evidencia.interpreter.colecoes-mapa-verso-bombom", 7),
        ("evidencia.interpreter.diagnostico-cli-exit-nonzero", 1),
        (
            "evidencia.interpreter.diagnostico-render-fonte-e-operador-bitnot",
            9,
        ),
        (
            "evidencia.interpreter.diagnostico-runtime-aritmetica-e-stack-trace",
            7,
        ),
        (
            "evidencia.interpreter.diagnostico-runtime-execucao-invalida",
            3,
        ),
        ("evidencia.interpreter.diagnostico-simbolo-inexistente", 2),
        (
            "evidencia.interpreter.diagnostico-stack-trace-truncamento",
            4,
        ),
        (
            "evidencia.interpreter.entrada-argumentos-e-ambiente-cli-exemplos",
            16,
        ),
        (
            "evidencia.interpreter.entrada-argumentos-flags-contexto-ambiente",
            31,
        ),
        (
            "evidencia.interpreter.execucao-chamadas-e-curto-circuito",
            7,
        ),
        ("evidencia.interpreter.execucao-cli-exemplos-basicos", 3),
        (
            "evidencia.interpreter.execucao-funcoes-usuario-tratos-e-genericos",
            18,
        ),
        (
            "evidencia.interpreter.execucao-nucleo-estado-aritmetica-fluxo",
            10,
        ),
        (
            "evidencia.interpreter.execucao-operadores-aritmeticos-relacionais-e-sinais",
            12,
        ),
        (
            "evidencia.interpreter.execucao-operadores-e-fluxo-cli-exemplos",
            12,
        ),
        (
            "evidencia.interpreter.execucao-recursao-e-fluxo-interpretador-e-cli",
            13,
        ),
        ("evidencia.interpreter.execucao-repl-e-render-erro-fonte", 7),
        (
            "evidencia.interpreter.fluxo-controle-lacos-quebrar-continuar",
            2,
        ),
        (
            "evidencia.interpreter.leques-trazer-recursos-e-programas-brinquedo",
            20,
        ),
        (
            "evidencia.interpreter.ponteiros-boot-freestanding-e-subset-nativo",
            21,
        ),
        (
            "evidencia.interpreter.ponteiros-escrita-indice-e-array-fixo",
            4,
        ),
        (
            "evidencia.interpreter.ponteiros-indexacao-e-array-operacional",
            4,
        ),
        ("evidencia.interpreter.ponteiros-seta-operacional", 13),
        ("evidencia.interpreter.processos-argv-explicito", 7),
        ("evidencia.interpreter.processos-captura-stderr", 19),
        ("evidencia.interpreter.processos-captura-stdout", 18),
        ("evidencia.interpreter.processos-entrada-stdin", 16),
        ("evidencia.interpreter.processos-externo-executar", 9),
        ("evidencia.interpreter.processos-pipeline", 10),
        ("evidencia.interpreter.tempo-unix-e-formatacao", 7),
        ("evidencia.interpreter.texto-dividir-juntar-e-buscar", 27),
        ("evidencia.interpreter.texto-formatar-cli-exemplos", 4),
        ("evidencia.interpreter.texto-formatar-verso", 5),
        (
            "evidencia.interpreter.texto-io-por-handle-e-arquivos-releitura",
            20,
        ),
        (
            "evidencia.interpreter.texto-verso-e-io-textual-por-caminho",
            16,
        ),
        (
            "evidencia.interpreter.texto-verso-intrinsecas-consulta-transformacao",
            25,
        ),
    ];

    let file = "tests/interpreter_tests.rs";
    let mut planned_keys = HashSet::new();

    // Unicidade global das chaves e coerência de arquivo/camada/domínio/marcadores.
    for &(key, _) in expected_interpreter_keys.iter() {
        assert!(
            planned_keys.insert(key),
            "chave de evidência repetida no plano da Onda 8E: {key}"
        );
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
        assert_eq!(
            region.file, file,
            "chave '{key}' deveria apontar para {file}"
        );
        assert_eq!(
            region.layer.as_deref(),
            Some("evidencia"),
            "chave '{key}' deveria usar a camada evidencia"
        );
        assert_eq!(
            region.domain.as_deref(),
            Some("interpreter"),
            "chave '{key}' deveria usar o domínio interpreter"
        );
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "chave '{key}' deveria ter marcadores ordenados"
        );
    }
    assert_eq!(
        planned_keys.len(),
        46,
        "o plano da Onda 8E deveria ter exatamente 46 regiões"
    );

    // Cobertura estrutural: todo #[test] da suíte pertence a exatamente uma região.
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", source_path.display()));
    let lines: Vec<_> = source.lines().collect();
    let mut owned_test_counts = vec![0usize; expected_interpreter_keys.len()];
    let mut test_count = 0usize;

    for (attribute_index, line) in lines.iter().enumerate() {
        if line.trim() != "#[test]" {
            continue;
        }
        let test_line = attribute_index + 1;
        let test_name = lines
            .iter()
            .skip(attribute_index + 1)
            .take(8)
            .find_map(|candidate| {
                candidate
                    .trim()
                    .strip_prefix("fn ")?
                    .split_once('(')
                    .map(|(name, _)| name.trim())
            })
            .unwrap_or_else(|| {
                panic!("structural_test_function_not_found: arquivo {file}, linha {test_line}")
            });
        let owners: Vec<_> = expected_interpreter_keys
            .iter()
            .enumerate()
            .filter_map(|(index, (key, _))| {
                let region = catalog
                    .region(key)
                    .unwrap_or_else(|| panic!("chave de evidência ausente no catálogo: {key}"));
                (region.content_start <= test_line && test_line <= region.content_end)
                    .then_some((index, *key))
            })
            .collect();
        match owners.as_slice() {
            [(index, _)] => owned_test_counts[*index] += 1,
            [] => panic!(
                "structural_test_region_not_found: arquivo {file}, linha {test_line}, função {test_name}"
            ),
            _ => panic!(
                "structural_test_region_ambiguous: arquivo {file}, linha {test_line}, função {test_name}, proprietárias {:?}",
                owners.iter().map(|(_, key)| *key).collect::<Vec<_>>()
            ),
        }
        test_count += 1;
    }

    assert_eq!(
        test_count, 538,
        "a Onda 8E deveria cobrir exatamente 538 testes da suíte interpreter"
    );
    for ((key, expected), owned) in expected_interpreter_keys.iter().zip(owned_test_counts) {
        assert_eq!(
            owned, *expected,
            "região '{key}' deveria possuir {expected} #[test], obteve {owned}"
        );
    }

    // Preservação das evidências anteriores (Ondas 8B–8D) e crescimento do catálogo:
    // total de evidência = 111 anteriores + 46 da Onda 8E.
    let evidence_total = catalog
        .regions
        .iter()
        .filter(|region| region.layer.as_deref() == Some("evidencia"))
        .count();
    assert!(
        evidence_total >= 157,
        "catálogo deveria conter ao menos 157 regiões de evidência (111 anteriores + 46 da Onda 8E), obteve {evidence_total}"
    );
    for previous in [
        "evidencia.ir.lowering-programa",
        "evidencia.select.blocos-e-terminadores",
        "evidencia.machine.renderizacao-cli-golden",
    ] {
        let region = catalog
            .region(previous)
            .unwrap_or_else(|| panic!("região de evidência anterior ausente: {previous}"));
        assert_eq!(
            region.layer.as_deref(),
            Some("evidencia"),
            "região anterior '{previous}' deveria permanecer como evidencia"
        );
    }

    assert!(
        catalog.regions.len() >= 340,
        "catálogo deveria conter ao menos 340 regiões após a Onda 8E"
    );
}
