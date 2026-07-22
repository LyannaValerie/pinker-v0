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

use pinker_v0::nav::{CodeCatalog, CodeIndex, CodeRegion};
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

fn stable_region_projection<'a>(regions: impl Iterator<Item = &'a CodeRegion>) -> String {
    let mut records: Vec<_> = regions
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

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (bytes.len() as u64) * 8;
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes(chunk[offset..offset + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    use std::fmt::Write as _;
    let mut digest = String::with_capacity(64);
    for word in state {
        write!(&mut digest, "{word:08x}").unwrap();
    }
    digest
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
    let nav_catalog_regions: Vec<_> = index
        .regions
        .iter()
        .filter(|region| region.file == "tests/nav_catalog_tests.rs")
        .collect();
    assert_eq!(nav_catalog_regions.len(), 6);
    assert!(nav_catalog_regions
        .iter()
        .all(|region| region.key.starts_with("evidencia.trama.nav-catalog.")));
    assert!(!index
        .regions
        .iter()
        .any(|region| region.file == "tests/nav_cartography_tests.rs"));
    for fixture_key in [
        "cfg.logica.curto-circuito",
        "runtime.exemplo.ficticio",
        "falso.teste.chave",
        "falso.pink.chave",
    ] {
        assert!(!index.regions.iter().any(|region| {
            region.file == "tests/nav_catalog_tests.rs" && region.key == fixture_key
        }));
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
/// (produção). A cápsula da 6E continua cobrindo somente produção, e o
/// `#[cfg(test)] mod tests` permanece fora da camada `runtime`; suas
/// evidências foram cartografadas posteriormente na camada `evidencia` pela
/// Onda 8J, sob o prefixo `evidencia.runtime.`. Verifica as 15 chaves
/// planejadas, a contagem exata de 15 regiões `runtime` e que todas apontam
/// para o arquivo do runtime nativo — nenhuma para `src/`. Não fixa o total
/// global do catálogo (§18.4).
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

    // A Onda 8E cartografa 534 testes de tests/interpreter_tests.rs (evidências
    // da execução interpretada da Pinker) em 46 regiões no domínio `interpreter`.
    // Quatro testes de build/backend ficam explicitamente adiados. As contagens
    // de #[test] por região estão congeladas no plano e a ordem física da suíte
    // é preservada. Chaves em ordem alfabética para diff estável.
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
        (
            "evidencia.interpreter.diagnostico-render-fonte-e-operador-bitnot",
            9,
        ),
        (
            "evidencia.interpreter.diagnostico-runtime-avaliacao-e-chamadas",
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
            15,
        ),
        (
            "evidencia.interpreter.entrada-argumentos-nomeados-e-flags",
            22,
        ),
        ("evidencia.interpreter.entrada-contexto-ambiente-e-saida", 9),
        (
            "evidencia.interpreter.execucao-chamadas-e-curto-circuito",
            7,
        ),
        ("evidencia.interpreter.execucao-cli-exemplos-basicos", 2),
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
            10,
        ),
        ("evidencia.interpreter.execucao-repl-e-render-erro-fonte", 7),
        ("evidencia.interpreter.fluxo-controle-lacos-basicos", 2),
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
            "evidencia.interpreter.ponteiros-array-fixo-e-cast-memoria-cli",
            4,
        ),
        ("evidencia.interpreter.ponteiros-seta-operacional", 14),
        ("evidencia.interpreter.processos-argv-explicito", 7),
        ("evidencia.interpreter.processos-captura-stderr", 19),
        ("evidencia.interpreter.processos-captura-stdout", 18),
        ("evidencia.interpreter.processos-entrada-stdin", 16),
        ("evidencia.interpreter.processos-externo-executar", 9),
        ("evidencia.interpreter.processos-pipeline", 10),
        ("evidencia.interpreter.tempo-unix-e-formatacao", 7),
        (
            "evidencia.interpreter.texto-dividir-substituir-juntar-e-buscar",
            27,
        ),
        ("evidencia.interpreter.texto-formatar-cli-exemplos", 4),
        ("evidencia.interpreter.texto-formatar-verso", 5),
        (
            "evidencia.interpreter.texto-io-por-handle-e-arquivos-releitura",
            20,
        ),
        (
            "evidencia.interpreter.texto-verso-e-io-textual-por-caminho",
            17,
        ),
        (
            "evidencia.interpreter.texto-verso-intrinsecas-consulta-transformacao",
            25,
        ),
    ];

    let expected_excluded_from_8e: HashSet<&str> = [
        "cli_build_gera_artefato_s_no_diretorio_padrao",
        "cli_build_com_imports_gera_artefato_no_out_dir",
        "cli_build_sem_arquivo_falha_com_uso",
        "cli_build_falha_semantica_retorna_erro",
    ]
    .into_iter()
    .collect();

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

    // O catálogo versionado deve ser exatamente a projeção canônica da fonte.
    // Esta equivalência cobre metadados (incluindo summary), faixas e hash, e
    // falha se qualquer linha do JSONL for adulterada sem regeneração.
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica (summary, faixa ou hash)"
    );

    // Cobertura estrutural: 534 testes pertencem a exatamente uma região 8E e
    // somente as quatro exclusões fechadas acima permanecem fora dessas regiões.
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", source_path.display()));
    let lines: Vec<_> = source.lines().collect();
    let mut owned_test_counts = vec![0usize; expected_interpreter_keys.len()];
    let mut total_test_count = 0usize;
    let mut mapped_test_count = 0usize;
    let mut found_excluded_from_8e = HashSet::new();

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
            [(index, key)] => {
                assert!(
                    !expected_excluded_from_8e.contains(test_name),
                    "teste excluído da Onda 8E '{test_name}' foi cartografado indevidamente pela região '{key}'"
                );
                owned_test_counts[*index] += 1;
                mapped_test_count += 1;
            }
            [] => {
                assert!(
                    expected_excluded_from_8e.contains(test_name),
                    "structural_test_region_not_found: arquivo {file}, linha {test_line}, função {test_name}"
                );
                assert!(
                    found_excluded_from_8e.insert(test_name),
                    "exclusão relativa à Onda 8E repetida na suíte: {test_name}"
                );
            }
            _ => panic!(
                "structural_test_region_ambiguous: arquivo {file}, linha {test_line}, função {test_name}, proprietárias {:?}",
                owners.iter().map(|(_, key)| *key).collect::<Vec<_>>()
            ),
        }
        total_test_count += 1;
    }

    assert_eq!(
        total_test_count, 538,
        "a suíte interpreter deveria manter exatamente 538 testes"
    );
    assert_eq!(
        mapped_test_count, 534,
        "a Onda 8E deveria cartografar exatamente 534 testes da suíte interpreter"
    );
    assert_eq!(
        found_excluded_from_8e, expected_excluded_from_8e,
        "as exclusões relativas à Onda 8E deveriam conter exatamente os quatro testes cli_build_*"
    );
    for ((key, expected), owned) in expected_interpreter_keys.iter().zip(owned_test_counts) {
        assert_eq!(
            owned, *expected,
            "região '{key}' deveria possuir {expected} #[test], obteve {owned}"
        );
    }

    // Preservação das evidências anteriores (Ondas 8B–8D) e crescimento do catálogo:
    // total de evidência = 111 anteriores + 46 da Onda 8E.
    let historical_evidence_total = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| region.layer.as_deref() == Some("evidencia"))
        .filter(|region| !region.key.starts_with("evidencia.backend-text."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .count();
    assert_eq!(
        historical_evidence_total, 157,
        "o estado histórico da Onda 8E deve conter 157 regiões de evidência (111 anteriores + 46 da Onda 8E)"
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

    let historical_catalog_total = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !region.key.starts_with("evidencia.backend-text."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .count();
    assert_eq!(
        historical_catalog_total, 340,
        "o estado histórico da Onda 8E deve totalizar 340 regiões"
    );
}

#[test]
fn onda_8f_cartografa_evidencias_do_backend_textual() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let expected_regions: [(&str, &str, &[&str], usize); 8] = [
        (
            "evidencia.backend-text.pipeline-helper",
            "tests/common/mod.rs",
            &["render_backend_text"],
            0,
        ),
        (
            "evidencia.backend-text.apresentacao-cli-helper",
            "tests/common/mod.rs",
            &["render_cli_pseudo_asm_output"],
            0,
        ),
        (
            "evidencia.backend-text.renderizacao-programa-minimo",
            "tests/backend_text_tests.rs",
            &["emite_funcao_simples"],
            1,
        ),
        (
            "evidencia.backend-text.renderizacao-controle-fluxo",
            "tests/backend_text_tests.rs",
            &["emite_if_else", "emite_if_sem_else"],
            2,
        ),
        (
            "evidencia.backend-text.renderizacao-chamada-binaria",
            "tests/backend_text_tests.rs",
            &["emite_chamada_direta_com_temporario_e_binaria"],
            1,
        ),
        (
            "evidencia.backend-text.renderizacao-chamada-void-retorno-nulo",
            "tests/backend_text_tests.rs",
            &["emite_return_vazio_e_funcao_nulo"],
            1,
        ),
        (
            "evidencia.backend-text.renderizacao-globais",
            "tests/backend_text_tests.rs",
            &["emite_constante_global_e_principal"],
            1,
        ),
        (
            "evidencia.backend-text.apresentacao-cli-pseudo-asm",
            "tests/backend_text_tests.rs",
            &["cli_pseudo_asm_header_estavel"],
            1,
        ),
    ];
    let expected_excluded_from_8f: [(&str, &str); 6] = [
        (
            "validador_cfg_falha_quando_cfg_invalida",
            "tests/backend_text_tests.rs",
        ),
        (
            "check_ignora_flags_de_emissao",
            "tests/backend_text_tests.rs",
        ),
        (
            "cli_build_gera_artefato_s_no_diretorio_padrao",
            "tests/interpreter_tests.rs",
        ),
        (
            "cli_build_com_imports_gera_artefato_no_out_dir",
            "tests/interpreter_tests.rs",
        ),
        (
            "cli_build_sem_arquivo_falha_com_uso",
            "tests/interpreter_tests.rs",
        ),
        (
            "cli_build_falha_semantica_retorna_erro",
            "tests/interpreter_tests.rs",
        ),
    ];

    let mut unique_catalog_keys = HashSet::new();
    for region in &catalog.regions {
        assert!(
            unique_catalog_keys.insert(region.key.as_str()),
            "chave duplicada no catálogo: {}",
            region.key
        );
    }
    let historical_catalog_total = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !region.key.starts_with("evidencia.backend-s."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .count();
    assert_eq!(
        historical_catalog_total, 348,
        "o estado histórico da Onda 8F deve totalizar 348 regiões"
    );

    let mut expected_keys: Vec<_> = expected_regions.iter().map(|entry| entry.0).collect();
    expected_keys.sort_unstable();
    let expected_key_set: HashSet<_> = expected_keys.iter().copied().collect();
    let mut backend_text_keys: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("evidencia.backend-text."))
        .map(|region| region.key.as_str())
        .collect();
    backend_text_keys.sort_unstable();
    assert_eq!(
        backend_text_keys, expected_keys,
        "a lista de regiões de evidência do backend textual deve ser exatamente a aprovada"
    );

    let mut source_cache = std::collections::HashMap::new();
    for (_, file, _, _) in expected_regions {
        source_cache.entry(file).or_insert_with(|| {
            fs::read_to_string(repository.join(file))
                .unwrap_or_else(|error| panic!("não foi possível ler {file}: {error}"))
        });
    }
    for &(key, file, owned_symbols, expected_test_count) in &expected_regions {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("região aprovada ausente: {key}"));
        assert_eq!(region.file, file, "arquivo divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some("backend-text"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert!(!region.summary.trim().is_empty(), "summary vazio em {key}");
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "ordem inválida dos marcadores em {key}"
        );

        let source = source_cache.get(file).unwrap();
        let lines: Vec<_> = source.lines().collect();
        let content = lines[(region.content_start - 1)..region.content_end].join("\n");
        assert!(!content.trim().is_empty(), "conteúdo vazio em {key}");
        for symbol in owned_symbols {
            let function_line = lines
                .iter()
                .position(|line| {
                    let line = line.trim();
                    line.starts_with(&format!("fn {symbol}("))
                        || line.starts_with(&format!("pub fn {symbol}("))
                })
                .map(|index| index + 1)
                .unwrap_or_else(|| panic!("símbolo aprovado ausente: {symbol}"));
            assert!(
                region.content_start <= function_line && function_line <= region.content_end,
                "símbolo {symbol} não pertence à região {key}"
            );
        }

        let owned_test_count = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim() == "#[test]")
            .filter(|(index, _)| {
                let line = index + 1;
                region.content_start <= line && line <= region.content_end
            })
            .count();
        assert_eq!(
            owned_test_count, expected_test_count,
            "contagem de testes divergente em {key}"
        );
    }

    let mut found_excluded_from_8f = HashSet::new();
    for &(expected_name, file) in &expected_excluded_from_8f {
        let source = fs::read_to_string(repository.join(file)).unwrap_or_else(|error| {
            panic!("não foi possível ler exclusão relativa {file}: {error}")
        });
        let lines: Vec<_> = source.lines().collect();
        let test_line = lines
            .iter()
            .position(|line| line.trim().starts_with(&format!("fn {expected_name}(")))
            .map(|index| index + 1)
            .unwrap_or_else(|| panic!("teste excluído da Onda 8F ausente: {expected_name}"));
        let owners_8f: Vec<_> = catalog
            .regions
            .iter()
            .filter(|region| expected_key_set.contains(region.key.as_str()))
            .filter(|region| {
                region.file == file
                    && region.content_start <= test_line
                    && test_line <= region.content_end
            })
            .map(|region| region.key.as_str())
            .collect();
        assert!(
            owners_8f.is_empty(),
            "teste {expected_name} deveria ficar fora das regiões 8F, mas pertence a {owners_8f:?}"
        );
        assert!(found_excluded_from_8f.insert(expected_name));
    }
    assert_eq!(found_excluded_from_8f.len(), 6);

    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    let previous_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !expected_key_set.contains(region.key.as_str()))
        .filter(|region| !region.key.starts_with("evidencia.backend-s."))
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .collect();
    assert_eq!(
        previous_regions.len(),
        340,
        "as 340 regiões anteriores devem ser preservadas"
    );
    let previous_projection = stable_region_projection(previous_regions.into_iter());
    assert_eq!(
        (
            previous_projection.len(),
            fnv1a64(previous_projection.as_bytes()),
        ),
        (145_064, 18_356_396_870_315_270_997),
        "a projeção estável das 340 entradas anteriores mudou"
    );
}

#[test]
fn onda_8g_cartografa_evidencias_do_backend_s_textual() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let expected_regions: [(&str, &str, &[&str], usize, &str); 7] = [
        (
            "evidencia.backend-s.pipeline-helper",
            "tests/common/mod.rs",
            &["render_backend_s"],
            0,
            "Executa o helper compartilhado render_backend_s inteiramente em memória: parse e checagem semântica, lowering e validação por IR, CFG e seleção, seguidos da emissão do backend .s textual via emit_from_selected. Não usa o helper do subset externo, assembler, linker nem execução nativa.",
        ),
        (
            "evidencia.backend-s.apresentacao-cli-helper",
            "tests/common/mod.rs",
            &["render_cli_asm_s_output"],
            0,
            "Monta a apresentação sintética de render_cli_asm_s_output em memória: concatena o cabeçalho `=== ASM .S (TEXTUAL) ===`, a saída de render_backend_s e o rodapé histórico de sucesso semântico. Não cria nem executa um processo CLI.",
        ),
        (
            "evidencia.backend-s.apresentacao-cli-asm-s",
            "tests/backend_s_tests.rs",
            &["asm_s_header_estavel"],
            1,
            "Golden exato da apresentação sintética em memória de render_cli_asm_s_output: cabeçalho ASM .S textual, representação textual hospedada mínima com metadados de ABI e rodapé histórico; não executa processo CLI nem produz assembly montável.",
        ),
        (
            "evidencia.backend-s.renderizacao-fluxo-e-abi-textual",
            "tests/backend_s_tests.rs",
            &[
                "asm_s_emite_if_else_simples",
                "asm_s_abi_minima_para_parametros_e_chamada",
            ],
            2,
            "Verifica por contains a representação .s textual de if/else e a ABI textual mínima de parâmetros e chamada, incluindo rótulos, branches, metadados abi.* e temporário de retorno; não comprova instruções x86, montagem, link ou execução.",
        ),
        (
            "evidencia.backend-s.validacao-subset-textual",
            "tests/backend_s_tests.rs",
            &["asm_s_falha_clara_para_tipo_ainda_nao_suportado"],
            1,
            "Exercita o diagnóstico do subset .s textual ao recusar slot seta<bombom>, verificando apenas a mensagem clara de tipo ainda não suportado nesse caminho textual.",
        ),
        (
            "evidencia.backend-s.freestanding-intencao-textual",
            "tests/backend_s_tests.rs",
            &["asm_s_freestanding_exibe_boot_entry_e_linker_script_minimo"],
            1,
            "Verifica por contains que o modo livre expõe intenção freestanding na representação textual, com boot.entry, linker script mínimo, kernel stub, _start e laço de espera; não monta, linka, inicializa hardware nem executa esse material.",
        ),
        (
            "evidencia.backend-s.build-cli-artefato-textual",
            "tests/interpreter_tests.rs",
            &[
                "cli_build_gera_artefato_s_no_diretorio_padrao",
                "cli_build_com_imports_gera_artefato_no_out_dir",
            ],
            2,
            "Exercita dois builds híbridos via processo `pink build`: exige sucesso, saída esperada, criação do artefato .s no diretório padrão ou em --out-dir e conteúdo textual mínimo, inclusive com import; não monta, linka nem executa o artefato.",
        ),
    ];
    let expected_test_counts = [0, 0, 1, 2, 1, 1, 2];
    assert_eq!(
        expected_regions.map(|region| region.3),
        expected_test_counts,
        "as contagens aprovadas da Onda 8G devem permanecer [0,0,1,2,1,1,2]"
    );

    let expected_keys: HashSet<_> = expected_regions.iter().map(|region| region.0).collect();
    assert_eq!(expected_keys.len(), 7, "as sete chaves 8G devem ser únicas");

    let mut unique_catalog_keys = HashSet::new();
    for region in &catalog.regions {
        assert!(
            unique_catalog_keys.insert(region.key.as_str()),
            "chave duplicada no catálogo: {}",
            region.key
        );
    }
    let historical_catalog_total = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .count();
    assert_eq!(
        historical_catalog_total, 355,
        "o estado histórico da Onda 8G deve totalizar 355 regiões"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
            .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .count(),
        172,
        "o estado histórico da Onda 8G deve totalizar 172 regiões de evidência"
    );
    let backend_s_evidence_keys: HashSet<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.domain.as_deref() == Some("backend-s")
                && region.layer.as_deref() == Some("evidencia")
        })
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        backend_s_evidence_keys, expected_keys,
        "o domínio backend-s deve conter exatamente as sete evidências 8G"
    );

    let mut source_cache = std::collections::HashMap::new();
    for (_, file, _, _, _) in expected_regions {
        source_cache.entry(file).or_insert_with(|| {
            fs::read_to_string(repository.join(file))
                .unwrap_or_else(|error| panic!("não foi possível ler {file}: {error}"))
        });
    }

    for &(key, file, owned_symbols, expected_test_count, expected_summary) in &expected_regions {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("região 8G ausente: {key}"));
        assert_eq!(region.file, file, "arquivo divergente para {key}");
        assert_eq!(region.kind, "region", "kind divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some("backend-s"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(
            region.summary, expected_summary,
            "summary divergente para {key}"
        );
        assert_eq!(region.status, "active", "status divergente para {key}");
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "ordem inválida dos marcadores em {key}"
        );

        let source = source_cache.get(file).unwrap();
        let lines: Vec<_> = source.lines().collect();
        let content_lines = &lines[(region.content_start - 1)..region.content_end];
        assert!(
            !content_lines.is_empty() && content_lines.iter().any(|line| !line.trim().is_empty()),
            "conteúdo vazio em {key}"
        );
        assert!(
            content_lines
                .iter()
                .all(|line| !line.contains("@pinker-nav:")),
            "marcador absorvido pelo conteúdo de {key}"
        );

        for symbol in owned_symbols {
            let function_line = lines
                .iter()
                .position(|line| {
                    let line = line.trim();
                    line.starts_with(&format!("fn {symbol}("))
                        || line.starts_with(&format!("pub fn {symbol}("))
                })
                .map(|index| index + 1)
                .unwrap_or_else(|| panic!("símbolo aprovado ausente: {symbol}"));
            assert!(
                region.content_start <= function_line && function_line <= region.content_end,
                "símbolo {symbol} não pertence à região {key}"
            );
        }

        let owned_test_count = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim() == "#[test]")
            .filter(|(index, _)| {
                let line = index + 1;
                region.content_start <= line && line <= region.content_end
            })
            .count();
        assert_eq!(
            owned_test_count, expected_test_count,
            "contagem de testes divergente em {key}"
        );
    }

    for file in [
        "tests/common/mod.rs",
        "tests/backend_s_tests.rs",
        "tests/interpreter_tests.rs",
    ] {
        let ordered: Vec<_> = expected_regions
            .iter()
            .filter(|region| region.1 == file)
            .map(|region| catalog.region(region.0).unwrap().start_marker)
            .collect();
        assert!(
            ordered.windows(2).all(|pair| pair[0] < pair[1]),
            "a ordem física das regiões 8G mudou em {file}"
        );
    }

    let expected_owned_tests: [(&str, &str, &str); 7] = [
        (
            "asm_s_header_estavel",
            "tests/backend_s_tests.rs",
            "evidencia.backend-s.apresentacao-cli-asm-s",
        ),
        (
            "asm_s_emite_if_else_simples",
            "tests/backend_s_tests.rs",
            "evidencia.backend-s.renderizacao-fluxo-e-abi-textual",
        ),
        (
            "asm_s_abi_minima_para_parametros_e_chamada",
            "tests/backend_s_tests.rs",
            "evidencia.backend-s.renderizacao-fluxo-e-abi-textual",
        ),
        (
            "asm_s_falha_clara_para_tipo_ainda_nao_suportado",
            "tests/backend_s_tests.rs",
            "evidencia.backend-s.validacao-subset-textual",
        ),
        (
            "asm_s_freestanding_exibe_boot_entry_e_linker_script_minimo",
            "tests/backend_s_tests.rs",
            "evidencia.backend-s.freestanding-intencao-textual",
        ),
        (
            "cli_build_gera_artefato_s_no_diretorio_padrao",
            "tests/interpreter_tests.rs",
            "evidencia.backend-s.build-cli-artefato-textual",
        ),
        (
            "cli_build_com_imports_gera_artefato_no_out_dir",
            "tests/interpreter_tests.rs",
            "evidencia.backend-s.build-cli-artefato-textual",
        ),
    ];
    for (test_name, file, expected_owner) in expected_owned_tests {
        let source = source_cache.get(file).unwrap();
        let test_line = source
            .lines()
            .position(|line| line.trim().starts_with(&format!("fn {test_name}(")))
            .map(|index| index + 1)
            .unwrap_or_else(|| panic!("teste 8G ausente: {test_name}"));
        let owners: Vec<_> = catalog
            .regions
            .iter()
            .filter(|region| {
                region.file == file
                    && region.content_start <= test_line
                    && test_line <= region.content_end
            })
            .map(|region| region.key.as_str())
            .collect();
        assert_eq!(
            owners,
            [expected_owner],
            "ownership divergente para {test_name}"
        );
    }

    let backend_s_source = source_cache.get("tests/backend_s_tests.rs").unwrap();
    assert_eq!(
        backend_s_source
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count(),
        5,
        "tests/backend_s_tests.rs deve manter exatamente 5 testes"
    );
    let common_source = source_cache.get("tests/common/mod.rs").unwrap();
    let external_helper_line = common_source
        .lines()
        .position(|line| {
            line.trim()
                .starts_with("pub fn render_backend_s_external_subset(")
        })
        .map(|index| index + 1)
        .expect("helper externo deve continuar presente e fora da Onda 8G");
    assert!(
        catalog.regions.iter().all(|region| {
            !expected_keys.contains(region.key.as_str())
                || region.file != "tests/common/mod.rs"
                || external_helper_line < region.content_start
                || region.content_end < external_helper_line
        }),
        "render_backend_s_external_subset deve permanecer fora das sete regiões 8G"
    );

    // tests/backend_s_external_toolchain_tests.rs saiu desta lista na Onda 8H e
    // tests/backend_nativo_tests.rs saiu na Onda 8I, que os cartografou; não resta
    // fronteira futura de suíte para a Onda 8G vigiar. A proteção não foi removida:
    // a cobertura completa daquele arquivo passou para
    // onda_8i_cartografa_evidencias_e_paridade_do_backend_nativo, que exige as 14
    // regiões, o ownership [2,5,7,2,3,1,7,1,1,18] e os 47 testes.
    let backend_nativo = "tests/backend_nativo_tests.rs";
    let backend_nativo_regions = catalog
        .regions
        .iter()
        .filter(|region| region.file == backend_nativo)
        .count();
    assert_eq!(
        backend_nativo_regions, 14,
        "tests/backend_nativo_tests.rs foi cartografado pela Onda 8I e deve manter 14 regiões"
    );
    // Redundância estrutural preservada da fronteira antiga: a Onda 8G continua
    // vigiando o tamanho real da suíte nativa, além das 14 regiões.
    let backend_nativo_source =
        fs::read_to_string(repository.join(backend_nativo)).expect("suíte do backend nativo");
    assert_eq!(
        backend_nativo_source
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count(),
        47,
        "{backend_nativo} deve manter exatamente 47 #[test]"
    );

    for future_without_owner in [
        "cli_build_sem_arquivo_falha_com_uso",
        "cli_build_falha_semantica_retorna_erro",
    ] {
        let file = "tests/interpreter_tests.rs";
        let source = source_cache.get(file).unwrap();
        let test_line = source
            .lines()
            .position(|line| {
                line.trim()
                    .starts_with(&format!("fn {future_without_owner}("))
            })
            .map(|index| index + 1)
            .unwrap_or_else(|| panic!("future sem owner ausente: {future_without_owner}"));
        let owners: Vec<_> = catalog
            .regions
            .iter()
            .filter(|region| {
                region.file == file
                    && region.content_start <= test_line
                    && test_line <= region.content_end
            })
            .map(|region| region.key.as_str())
            .collect();
        assert!(
            owners.is_empty(),
            "future {future_without_owner} deve permanecer sem owner global, obteve {owners:?}"
        );
    }

    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    let previous_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !expected_keys.contains(region.key.as_str()))
        .filter(|region| !region.key.starts_with("evidencia.backend-s-externo."))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .collect();
    assert_eq!(
        previous_regions.len(),
        348,
        "as 348 regiões anteriores devem ser preservadas semanticamente"
    );
    let previous_projection = stable_region_projection(previous_regions.into_iter());
    assert_eq!(
        (
            previous_projection.len(),
            fnv1a64(previous_projection.as_bytes()),
        ),
        (148_009, 1_387_240_491_465_620_435),
        "a projeção estável das 348 regiões anteriores mudou"
    );

    let onda_8f_complete = true;
    let onda_8_complete = false;
    let trama_complete = false;
    assert!(onda_8f_complete);
    assert!(!onda_8_complete);
    assert!(!trama_complete);
}

#[test]
fn capsula_nav_catalog_cartografa_suporte_e_seis_testes() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_path = "tests/nav_catalog_tests.rs";
    let source = fs::read_to_string(repository.join(target_path)).expect("fonte da cápsula");
    let lines: Vec<_> = source.lines().collect();
    let expected_keys = [
        "evidencia.trama.nav-catalog.fixture-config",
        "evidencia.trama.nav-catalog.process-support",
        "evidencia.trama.nav-catalog.sync-verify-roots",
        "evidencia.trama.nav-catalog.show-extraction",
        "evidencia.trama.nav-catalog.unbalanced-marker",
        "evidencia.trama.nav-catalog.required-roots",
    ];
    let support_keys = &expected_keys[..2];
    let evidence_keys = &expected_keys[2..];

    #[derive(Debug)]
    struct Span {
        key: String,
        start: usize,
        end: usize,
    }
    let mut spans = Vec::new();
    let mut active: Option<(String, usize)> = None;
    let mut marker_lines = 0;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("// @pinker-nav:") {
            continue;
        }
        marker_lines += 1;
        if let Some(key) = trimmed.strip_prefix("// @pinker-nav:start ") {
            assert!(active.is_none(), "regiões da cápsula não podem aninhar");
            active = Some((key.to_string(), index + 1));
        } else if let Some(key) = trimmed.strip_prefix("// @pinker-nav:end ") {
            let (open_key, start) = active.take().expect("fim sem início");
            assert_eq!(key, open_key, "marcadores inicial/final divergentes");
            spans.push(Span {
                key: open_key,
                start,
                end: index + 1,
            });
        }
    }
    assert!(active.is_none(), "região sem marcador final");
    assert_eq!(marker_lines, 30, "a cápsula exige exatamente 30 marcadores");
    assert_eq!(spans.len(), 6, "a cápsula exige exatamente seis regiões");
    let physical_keys: Vec<_> = spans.iter().map(|span| span.key.as_str()).collect();
    assert_eq!(
        physical_keys, expected_keys,
        "ordem física das seis regiões"
    );
    for pair in spans.windows(2) {
        assert!(pair[0].end < pair[1].start, "regiões não podem sobrepor");
    }
    assert_eq!(
        spans[0].start, 8,
        "docs e imports devem ficar fora das regiões"
    );
    assert!(lines[..7].iter().all(|line| !line.contains("@pinker-nav:")));

    let stripped: String = source
        .split_inclusive('\n')
        .filter(|line| {
            !line
                .trim_start()
                .strip_prefix("//")
                .is_some_and(|rest| rest.trim_start().starts_with("@pinker-nav:"))
        })
        .collect();
    assert_eq!(
        sha256_hex(stripped.as_bytes()),
        "9499fdd27a89d4c4c595d6ec0d15ed53424ecac3d9f503ca48f942673da03734",
        "retirar marcadores deve reconstruir exatamente a fonte-base"
    );

    #[derive(Default, Debug, PartialEq, Eq)]
    struct Owned {
        constants: Vec<String>,
        helpers: Vec<String>,
        tests: Vec<String>,
    }
    let mut ownership = Vec::new();
    for span in &spans {
        let mut owned = Owned::default();
        let mut test_attribute = false;
        for line in &lines[span.start..span.end - 1] {
            let trimmed = line.trim();
            if trimmed == "#[test]" {
                test_attribute = true;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("const ") {
                owned
                    .constants
                    .push(rest.split(':').next().unwrap().to_string());
                test_attribute = false;
            } else if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap().to_string();
                if test_attribute {
                    owned.tests.push(name);
                } else {
                    owned.helpers.push(name);
                }
                test_attribute = false;
            }
        }
        ownership.push((span.key.as_str(), owned));
    }
    let expected_ownership = [
        (
            expected_keys[0],
            Owned {
                constants: ["DOC_TOML", "SRC", "RUNTIME_LIB", "FALSO_TEST", "FALSO_PINK"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[1],
            Owned {
                helpers: ["temp_repo", "write", "fixture", "run"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[2],
            Owned {
                tests: ["sincronizar_e_verificar_do_codigo"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[3],
            Owned {
                tests: [
                    "mostrar_extrai_a_regiao",
                    "mostrar_extrai_regiao_do_runtime",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[4],
            Owned {
                tests: ["verificar_falha_quando_marcador_desbalanceado"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[5],
            Owned {
                tests: [
                    "sincronizar_falha_quando_tests_raiz_obrigatoria_ausente",
                    "sincronizar_falha_quando_raiz_obrigatoria_ausente",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
    ];
    assert_eq!(ownership, expected_ownership, "ownership exato da cápsula");
    assert!(ownership[..2]
        .iter()
        .all(|(_, owned)| owned.tests.is_empty()));
    assert!(ownership[2..]
        .iter()
        .all(|(_, owned)| !owned.tests.is_empty()));
    let owned_constants: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.constants.iter().map(String::as_str))
        .collect();
    let owned_helpers: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.helpers.iter().map(String::as_str))
        .collect();
    let owned_tests: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.tests.iter().map(String::as_str))
        .collect();
    assert_eq!(
        owned_constants,
        ["DOC_TOML", "SRC", "RUNTIME_LIB", "FALSO_TEST", "FALSO_PINK"]
    );
    assert_eq!(owned_helpers, ["temp_repo", "write", "fixture", "run"]);
    assert_eq!(
        owned_tests,
        [
            "sincronizar_e_verificar_do_codigo",
            "mostrar_extrai_a_regiao",
            "mostrar_extrai_regiao_do_runtime",
            "verificar_falha_quando_marcador_desbalanceado",
            "sincronizar_falha_quando_tests_raiz_obrigatoria_ausente",
            "sincronizar_falha_quando_raiz_obrigatoria_ausente",
        ]
    );
    for symbols in [&owned_constants, &owned_helpers, &owned_tests] {
        let unique: HashSet<_> = symbols.iter().copied().collect();
        assert_eq!(unique.len(), symbols.len(), "ownership duplicado");
    }

    let catalog_path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&catalog_path).expect("catálogo versionado");
    // Escopo desta cápsula: o catálogo tal como fechado por ela. As cápsulas
    // seguintes acrescentam arquivos próprios, excluídos aqui para que os
    // literais congelados 392/209/15 continuem valendo sem enfraquecimento.
    // Extensão de seletor: `tests/trama_query_tests.rs` (cápsula posterior) é
    // igualmente excluído; o conjunto congelado desta cápsula permanece 392.
    let capsule_scope: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .collect();
    assert_eq!(capsule_scope.len(), 392);
    assert_eq!(
        capsule_scope
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        209
    );
    assert_eq!(
        capsule_scope
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15
    );
    let mut target_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file == target_path)
        .collect();
    target_regions.sort_by_key(|region| region.start_marker);
    assert_eq!(target_regions.len(), 6);
    assert_eq!(
        target_regions
            .iter()
            .map(|region| region.key.as_str())
            .collect::<Vec<_>>(),
        expected_keys
    );
    for region in &target_regions {
        assert_eq!(region.domain.as_deref(), Some("trama"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.file, target_path);
        assert_eq!(region.kind, "region");
    }
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| support_keys.contains(&region.key.as_str()))
            .count(),
        2
    );
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| evidence_keys.contains(&region.key.as_str()))
            .count(),
        4
    );

    let historical: Vec<_> = capsule_scope
        .iter()
        .copied()
        .filter(|region| region.file != target_path)
        .collect();
    assert_eq!(historical.len(), 386, "conjunto histórico exato");
    let historical_projection = stable_region_projection(historical.into_iter());
    assert_eq!(
        (
            historical_projection.len(),
            fnv1a64(historical_projection.as_bytes())
        ),
        (168_339, 1_634_706_628_046_951_093)
    );
    let full_projection = stable_region_projection(capsule_scope.iter().copied());
    assert_eq!(
        (full_projection.len(), fnv1a64(full_projection.as_bytes())),
        (170_076, 12_143_728_175_883_859_804)
    );

    let regenerated = CodeIndex::scan_repo(&repository).expect("scan canônico");
    assert!(regenerated.verify().is_empty());
    assert_eq!(
        fs::read_to_string(catalog_path).expect("catálogo JSONL"),
        regenerated.render_jsonl(),
        "catálogo deve ser idêntico à regeneração canônica"
    );

    let gate_source = include_str!("nav_cartography_tests.rs");
    assert_eq!(
        gate_source
            .lines()
            .filter(|line| line.trim() == "fn onda_8_convergencia_fecha_cadeia_8a_8j() {")
            .count(),
        1,
        "gate de fechamento da Onda 8 deve permanecer presente"
    );
    let inventory = include_str!("../docs/development/code-navigation-inventory.md");
    let inventory_flat = inventory.split_whitespace().collect::<Vec<_>>().join(" ");
    for statement in [
        "primeira cápsula operacional/documental da Trama está completa",
        "tests/nav_catalog_tests.rs: 6 regiões",
        "2 suporte + 4 evidência",
        "5 constantes + 4 helpers + 6 testes",
        "catálogo atual = 392",
        "evidencia atual = 209",
        "runtime atual = 15",
        "onda_8_complete = true",
        "trama_complete = false",
        "próximo alvo: tests/doc_catalog_tests.rs",
        "alvo subsequente: tests/trama_query_tests.rs",
        "`apps/` permanece reservada à Onda 9",
        "repositórios sintéticos",
        "processos filhos",
        "asserções seletivas",
        "limpeza explícita, não RAII",
        "sobras após panic",
        "não prova a correção semântica completa da CLI",
        "não é validação exaustiva do catálogo",
        "não prova comportamento de processos independente de plataforma",
        "não conclui a Trama",
        "não ativa a Onda 9",
        "não oferece suporte a apps/",
    ] {
        assert!(
            inventory_flat.contains(statement),
            "contrato documental ausente: {statement}"
        );
    }
}

/// Segunda cápsula operacional/documental da Trama: `tests/doc_catalog_tests.rs`.
/// Congela as seis regiões marker-only (2 de suporte + 4 de evidência), o
/// ownership exato de 3 constantes + 4 helpers + 4 testes, a prova de que
/// retirar os marcadores reconstrói a fonte-base, os totais 398/215/15 e a
/// preservação integral das 392 regiões anteriores e das 386 da Onda 8.
#[test]
fn capsula_doc_catalog_cartografa_suporte_e_quatro_testes() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_path = "tests/doc_catalog_tests.rs";
    let source = fs::read_to_string(repository.join(target_path)).expect("fonte da cápsula");
    let lines: Vec<_> = source.lines().collect();
    let expected_keys = [
        "evidencia.trama.doc-catalog.fixture-config",
        "evidencia.trama.doc-catalog.process-support",
        "evidencia.trama.doc-catalog.sync-verify",
        "evidencia.trama.doc-catalog.stale-catalog",
        "evidencia.trama.doc-catalog.show-extraction",
        "evidencia.trama.doc-catalog.unbalanced-anchor",
    ];
    let support_keys = &expected_keys[..2];
    let evidence_keys = &expected_keys[2..];

    // A. e F. Estrutura física: seis regiões balanceadas, contíguas, sem
    // aninhamento nem sobreposição, na ordem exata declarada.
    #[derive(Debug)]
    struct Span {
        key: String,
        start: usize,
        end: usize,
    }
    let mut spans = Vec::new();
    let mut active: Option<(String, usize)> = None;
    let mut marker_lines = 0;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("// @pinker-nav:") {
            continue;
        }
        marker_lines += 1;
        if let Some(key) = trimmed.strip_prefix("// @pinker-nav:start ") {
            assert!(active.is_none(), "regiões da cápsula não podem aninhar");
            active = Some((key.to_string(), index + 1));
        } else if let Some(key) = trimmed.strip_prefix("// @pinker-nav:end ") {
            let (open_key, start) = active.take().expect("fim sem início");
            assert_eq!(key, open_key, "marcadores inicial/final divergentes");
            spans.push(Span {
                key: open_key,
                start,
                end: index + 1,
            });
        }
    }
    assert!(active.is_none(), "região sem marcador final");
    assert_eq!(marker_lines, 30, "a cápsula exige exatamente 30 marcadores");
    assert_eq!(spans.len(), 6, "a cápsula exige exatamente seis regiões");
    let physical_keys: Vec<_> = spans.iter().map(|span| span.key.as_str()).collect();
    assert_eq!(
        physical_keys, expected_keys,
        "ordem física das seis regiões"
    );
    for pair in spans.windows(2) {
        assert!(pair[0].end < pair[1].start, "regiões não podem sobrepor");
        for line in &lines[pair[0].end..pair[1].start - 1] {
            assert!(
                line.trim().is_empty(),
                "as regiões devem ser contíguas, separadas apenas por linhas em branco"
            );
        }
    }
    assert_eq!(
        spans[0].start, 8,
        "docs e imports devem ficar fora das regiões"
    );
    assert_eq!(
        spans[5].end,
        lines.len(),
        "a última região deve terminar no fim do arquivo"
    );
    assert!(lines[..7].iter().all(|line| !line.contains("@pinker-nav:")));

    // G. Prova marker-only contra a SHA-256 da fonte-base cartografada.
    let stripped: String = source
        .split_inclusive('\n')
        .filter(|line| {
            !line
                .trim_start()
                .strip_prefix("//")
                .is_some_and(|rest| rest.trim_start().starts_with("@pinker-nav:"))
        })
        .collect();
    assert_eq!(
        sha256_hex(stripped.as_bytes()),
        "c2759331fb4377521c4f51479ae732c74ef0e4121d4e5ee0415a41eb67a37dd3",
        "retirar marcadores deve reconstruir exatamente a fonte-base"
    );

    // E. Ownership exato: 3 constantes, 4 helpers, 4 testes, sem órfãos,
    // duplicatas ou sobreposição de propriedade.
    #[derive(Default, Debug, PartialEq, Eq)]
    struct Owned {
        constants: Vec<String>,
        helpers: Vec<String>,
        tests: Vec<String>,
    }
    let mut ownership = Vec::new();
    for span in &spans {
        let mut owned = Owned::default();
        let mut test_attribute = false;
        for line in &lines[span.start..span.end - 1] {
            let trimmed = line.trim();
            if trimmed == "#[test]" {
                test_attribute = true;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("const ") {
                owned
                    .constants
                    .push(rest.split(':').next().unwrap().to_string());
                test_attribute = false;
            } else if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap().to_string();
                if test_attribute {
                    owned.tests.push(name);
                } else {
                    owned.helpers.push(name);
                }
                test_attribute = false;
            }
        }
        ownership.push((span.key.as_str(), owned));
    }
    let expected_ownership = [
        (
            expected_keys[0],
            Owned {
                constants: ["DOC_TOML", "PORTAL", "CORE"].map(str::to_string).to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[1],
            Owned {
                helpers: ["temp_repo", "write", "fixture", "run"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[2],
            Owned {
                tests: ["sincronizar_gera_catalogo_e_verificar_aprova"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[3],
            Owned {
                tests: ["verificar_falha_quando_catalogo_desatualizado"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[4],
            Owned {
                tests: ["mostrar_extrai_apenas_a_secao"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[5],
            Owned {
                tests: ["verificar_detecta_ancora_desbalanceada"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
    ];
    assert_eq!(ownership, expected_ownership, "ownership exato da cápsula");
    assert!(ownership[..2]
        .iter()
        .all(|(_, owned)| owned.tests.is_empty()));
    assert!(ownership[2..]
        .iter()
        .all(|(_, owned)| owned.tests.len() == 1));
    let owned_constants: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.constants.iter().map(String::as_str))
        .collect();
    let owned_helpers: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.helpers.iter().map(String::as_str))
        .collect();
    let owned_tests: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.tests.iter().map(String::as_str))
        .collect();
    assert_eq!(owned_constants, ["DOC_TOML", "PORTAL", "CORE"]);
    assert_eq!(owned_helpers, ["temp_repo", "write", "fixture", "run"]);
    assert_eq!(
        owned_tests,
        [
            "sincronizar_gera_catalogo_e_verificar_aprova",
            "verificar_falha_quando_catalogo_desatualizado",
            "mostrar_extrai_apenas_a_secao",
            "verificar_detecta_ancora_desbalanceada",
        ]
    );
    for symbols in [&owned_constants, &owned_helpers, &owned_tests] {
        let unique: HashSet<_> = symbols.iter().copied().collect();
        assert_eq!(unique.len(), symbols.len(), "ownership duplicado");
    }
    // Nenhum símbolo do arquivo pode ficar órfão fora das seis regiões.
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("const ") || trimmed.starts_with("fn ")) {
            continue;
        }
        let number = index + 1;
        assert!(
            spans
                .iter()
                .any(|span| number > span.start && number < span.end),
            "símbolo órfão fora de região na linha {number}"
        );
    }

    // D. e H. Catálogo versionado: metadados exatos e totais 398/215/15.
    let catalog_path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&catalog_path).expect("catálogo versionado");
    // Extensão de seletor: `tests/trama_query_tests.rs` (cápsula posterior) é
    // excluído para que o instantâneo congelado 398/215/15 e as projeções
    // pinadas desta cápsula permaneçam exatos, sem enfraquecimento.
    let capsule_scope: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file != "tests/trama_query_tests.rs")
        .collect();
    assert_eq!(capsule_scope.len(), 398);
    assert_eq!(
        capsule_scope
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        215
    );
    assert_eq!(
        capsule_scope
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15
    );
    let mut all_keys: Vec<&str> = catalog.regions.iter().map(|r| r.key.as_str()).collect();
    all_keys.sort_unstable();
    let unique_keys: HashSet<&str> = all_keys.iter().copied().collect();
    assert_eq!(unique_keys.len(), all_keys.len(), "chaves duplicadas");

    let mut target_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file == target_path)
        .collect();
    target_regions.sort_by_key(|region| region.start_marker);
    assert_eq!(target_regions.len(), 6);
    assert_eq!(
        target_regions
            .iter()
            .map(|region| region.key.as_str())
            .collect::<Vec<_>>(),
        expected_keys
    );
    for region in &target_regions {
        assert_eq!(region.domain.as_deref(), Some("trama"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.file, target_path);
        assert_eq!(region.kind, "region");
        assert_eq!(region.status, "active");
    }
    // C. Classificação exata: duas de suporte e quatro de evidência.
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| support_keys.contains(&region.key.as_str()))
            .count(),
        2
    );
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| evidence_keys.contains(&region.key.as_str()))
            .count(),
        4
    );

    // I. Preservação integral das 392 regiões da base mergeada.
    let merged_base: Vec<_> = capsule_scope
        .iter()
        .copied()
        .filter(|region| region.file != target_path)
        .collect();
    assert_eq!(merged_base.len(), 392, "base mergeada exata");
    assert_eq!(
        merged_base
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        209
    );
    assert_eq!(
        merged_base
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15
    );
    // J. Preservação do conjunto histórico de 386 regiões da Onda 8.
    let historical: Vec<_> = merged_base
        .iter()
        .copied()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .collect();
    assert_eq!(historical.len(), 386, "conjunto histórico exato");
    let historical_projection = stable_region_projection(historical.into_iter());
    assert_eq!(
        (
            historical_projection.len(),
            fnv1a64(historical_projection.as_bytes())
        ),
        (168_339, 1_634_706_628_046_951_093),
        "a projeção estável das 386 regiões da Onda 8 mudou"
    );
    let merged_base_projection = stable_region_projection(merged_base.into_iter());
    assert_eq!(
        (
            merged_base_projection.len(),
            fnv1a64(merged_base_projection.as_bytes())
        ),
        (170_076, 12_143_728_175_883_859_804),
        "a projeção estável das 392 regiões da base mergeada mudou"
    );
    // K. Projeção completa desta cápsula, medida — não predita.
    let full_projection = stable_region_projection(capsule_scope.iter().copied());
    assert_eq!(
        (full_projection.len(), fnv1a64(full_projection.as_bytes())),
        (171_741, 7_038_069_266_194_794_117),
        "a projeção estável das 398 regiões mudou"
    );

    // L. Igualdade com a regeneração canônica do CodeIndex.
    let regenerated = CodeIndex::scan_repo(&repository).expect("scan canônico");
    assert!(regenerated.verify().is_empty());
    assert_eq!(
        fs::read_to_string(catalog_path).expect("catálogo JSONL"),
        regenerated.render_jsonl(),
        "catálogo deve ser idêntico à regeneração canônica"
    );

    // M. Gates anteriores permanecem presentes.
    let gate_source = include_str!("nav_cartography_tests.rs");
    for gate in [
        "onda_8_convergencia_fecha_cadeia_8a_8j",
        "capsula_nav_catalog_cartografa_suporte_e_seis_testes",
    ] {
        let declaration = format!("fn {gate}() {{");
        assert_eq!(
            gate_source
                .lines()
                .filter(|line| line.trim() == declaration)
                .count(),
            1,
            "gate anterior ausente ou duplicado: {gate}"
        );
    }

    // N. e O. Contrato documental e não-alegações explícitas.
    let inventory = include_str!("../docs/development/code-navigation-inventory.md");
    let inventory_flat = inventory.split_whitespace().collect::<Vec<_>>().join(" ");
    for statement in [
        "primeira cápsula operacional/documental da Trama está completa",
        "segunda cápsula operacional/documental da Trama está completa",
        "tests/doc_catalog_tests.rs: 6 regiões",
        "2 suporte + 4 evidência",
        "3 constantes + 4 helpers + 4 testes",
        "catálogo atual = 398",
        "evidencia atual = 215",
        "runtime atual = 15",
        "onda_8_complete = true",
        "trama_complete = false",
        "próximo alvo: tests/trama_query_tests.rs",
        "`apps/` permanece reservada à Onda 9",
        "repositórios sintéticos",
        "processos filhos",
        "asserções seletivas",
        "limpeza explícita, não RAII",
        "sobras após panic",
        "não prova a correção semântica completa da CLI",
        "não é validação exaustiva do catálogo documental",
        "não prova comportamento de processos independente de plataforma",
        "não conclui a Trama",
        "não ativa a Onda 9",
        "não oferece suporte a apps/",
    ] {
        assert!(
            inventory_flat.contains(statement),
            "contrato documental ausente: {statement}"
        );
    }
    assert!(
        !inventory_flat.contains("próximo alvo: apps/"),
        "`apps/` permanece reservada à Onda 9 e não pode ser o próximo alvo"
    );
}

/// Fecha cumulativamente a convergência cartográfica da Onda 8 sem substituir
/// os gates detalhados de 8A–8J. O congelamento cobre todos os campos estáveis
/// das 386 regiões; assim, remoção, troca ou alteração de qualquer chave de
/// evidência também altera a projeção.
#[test]
fn onda_8_convergencia_fecha_cadeia_8a_8j() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");
    // As cápsulas operacionais/documentais posteriores ao fechamento acrescentam
    // regiões novas em arquivos próprios; o conjunto congelado da Onda 8 é o
    // complemento exato desses arquivos e permanece com os mesmos literais.
    let historical: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .collect();

    assert_eq!(
        historical.len(),
        386,
        "a convergência da Onda 8 exige exatamente 386 regiões"
    );
    assert_eq!(
        historical
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        203,
        "a convergência da Onda 8 exige exatamente 203 regiões de evidência"
    );
    assert_eq!(
        historical
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15,
        "a convergência da Onda 8 preserva as 15 regiões de runtime"
    );

    let projection = stable_region_projection(historical.into_iter());
    assert_eq!(
        (projection.len(), fnv1a64(projection.as_bytes())),
        (168_339, 1_634_706_628_046_951_093),
        "a projeção estável das 386 regiões convergidas da Onda 8 mudou"
    );

    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    // Os gates históricos continuam sendo a autoridade detalhada de ownership
    // e dos contratos locais. Esta lista apenas fecha a cadeia declarada.
    let gate_source = include_str!("nav_cartography_tests.rs");
    let stage_gates = [
        "scanner_oficial_inclui_tests_sem_catalogar_fixtures_literais",
        "camada_evidencia_frontend_cartografa_lexer_parser_common",
        "onda_8c_cartografa_evidencias_semanticas",
        "onda_8d_cartografa_evidencias_do_pipeline",
        "onda_8e_cartografa_evidencias_da_execucao_interpretada",
        "onda_8f_cartografa_evidencias_do_backend_textual",
        "onda_8g_cartografa_evidencias_do_backend_s_textual",
        "onda_8h_cartografa_evidencias_da_toolchain_externa",
        "onda_8i_cartografa_evidencias_e_paridade_do_backend_nativo",
        "onda_8j_cartografa_evidencias_internas_do_runtime",
    ];
    assert_eq!(
        stage_gates.len(),
        10,
        "a cadeia declarada deve cobrir 8A–8J"
    );
    for gate in stage_gates {
        let declaration = format!("fn {gate}() {{");
        assert_eq!(
            gate_source.matches(&declaration).count(),
            1,
            "gate histórico ausente ou duplicado na cadeia 8A–8J: {gate}"
        );
    }

    let inventory = include_str!("../docs/development/code-navigation-inventory.md");
    for (statement, failure) in [
        (
            "Onda 8A–8J",
            "o inventário deve declarar a cadeia Onda 8A–8J",
        ),
        (
            "catálogo atual = 386",
            "o inventário deve registrar o catálogo atual = 386",
        ),
        (
            "evidencia atual = 203",
            "o inventário deve registrar evidencia atual = 203",
        ),
        (
            "runtime atual = 15",
            "o inventário deve registrar runtime atual = 15",
        ),
        (
            "onda_8_complete = true",
            "o inventário deve registrar onda_8_complete = true",
        ),
        (
            "trama_complete = false",
            "o inventário deve registrar trama_complete = false",
        ),
        (
            "tests/common/mod.rs helpers: 8",
            "o inventário deve registrar 8 helpers em tests/common/mod.rs",
        ),
        (
            "tests/nav_catalog_tests.rs",
            "o inventário deve retomar em tests/nav_catalog_tests.rs",
        ),
        (
            "TRAMA_WAVE_8_CONVERGENCE_AUDIT_ACCEPTED_IMPLEMENTATION_REQUIRED",
            "o inventário deve preservar o veredito aceito da auditoria de convergência",
        ),
    ] {
        assert!(inventory.contains(statement), "{failure}");
    }

    let onda_8_complete = true;
    let trama_complete = false;
    assert!(onda_8_complete);
    assert!(!trama_complete);
}

#[test]
fn onda_8h_cartografa_evidencias_da_toolchain_externa() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let central = "tests/backend_s_external_toolchain_tests.rs";
    let helper_file = "tests/common/mod.rs";

    // As nove regiões do arquivo central, em ordem física, com a contagem de testes aprovada.
    let expected_test_regions: [(&str, usize, &str); 9] = [
        (
            "evidencia.backend-s-externo.renderizacao-recortes-versionados",
            16,
            "Fornece fonte inline ou exemplos versionados (fase111–125) ao helper render_backend_s_external_subset, que executa parse, semântica, IR, CFG e seleção em memória e emite assembly via emit_external_toolchain_subset; valida por contains o cabeçalho do subset, `.globl main`, rótulos `.L<fn>_*`, `jmp`/`cmpq`/`jne`/`setb`, seção `.rodata`, movimentos de argumento e instruções de deref. Nenhum processo externo é criado: não monta, não linka e não executa; a evidência é sobre o texto emitido, não sobre a corretude do código de máquina.",
        ),
        (
            "evidencia.backend-s-externo.fronteira-ninho-heterogeneo",
            8,
            "Alterna aceitações e recusas dos exemplos de `ninho` heterogêneo nas camadas 1–4 (fase129–132): nos casos aceitos verifica por contains os deslocamentos e acessos emitidos no assembly; nos recusados verifica a mensagem de erro do subset externo montável. Todo o trabalho ocorre em memória via render_backend_s_external_subset; nenhuma ferramenta externa é chamada e nada é montado, ligado ou executado.",
        ),
        (
            "evidencia.backend-s-externo.fronteira-conversao-virar",
            3,
            "Fornece exemplos versionados de `virar` camadas 1 e 2 (duas aceitações) e um exemplo inválido (uma recusa); verifica textualmente as instruções de conversão emitidas e, no caso inválido, a mensagem de recusa. Execução em memória apenas; nenhuma ferramenta externa é chamada — sem assembler, linker ou binário.",
        ),
        (
            "evidencia.backend-s-externo.renderizacao-verso-rodata",
            2,
            "Fornece exemplos de `verso` camada 1 (inclusive um exemplo historicamente marcado como inválido, hoje aceito) e verifica por contains o layout length-prefixed `[.quad tamanho][.ascii bytes]` na seção `.rodata`. Validação apenas textual: não monta, não liga e não executa, e nada é provado sobre a leitura desse layout em tempo de execução.",
        ),
        (
            "evidencia.backend-s-externo.renderizacao-quebrar-continuar",
            3,
            "Fornece exemplos versionados de `quebrar`/`continuar` (fase126–128) em ordem física decrescente de camada — 3, 2, 1 — e verifica textualmente os rótulos e saltos emitidos. Execução somente em memória via render_backend_s_external_subset; nenhum processo externo, sem montagem, linkedição ou execução.",
        ),
        (
            "evidencia.backend-s-externo.execucao-real-recortes-versionados",
            22,
            "Cada teste renderiza o `.s` com render_backend_s_external_subset, grava o arquivo em diretório temporário único, detecta em tempo de execução um driver C (`cc`, `gcc` ou `clang`) e o invoca como responsável pela montagem e pela linkedição, executando em seguida o binário produzido e validando apenas `status.code()`. Nenhum stdout é validado e o stderr é usado somente como mensagem de falha. O caminho é hospedado com runtime_init=false e sem libpinker_rt.a. Todos são pulados silenciosamente fora de Linux x86_64 ou quando não há driver C — a suíte pode passar sem exercer esta evidência.",
        ),
        (
            "evidencia.backend-s-externo.execucao-real-abi-frame-interprocedural",
            9,
            "Mesmos limites da região anterior — renderização do `.s`, gravação em diretório temporário, driver C (`cc`, `gcc` ou `clang`) detectado em runtime responsável por montagem e linkedição, execução do binário, validação apenas de `status.code()`, nenhum stdout validado, stderr somente como mensagem de falha, runtime_init=false, sem libpinker_rt.a e skip silencioso fora de Linux x86_64 ou sem driver C — aplicados a locais, aritmética, chamadas, parâmetros, frame, memória de frame, composição interprocedural e programas lineares maiores. A suíte pode passar sem exercer esta evidência.",
        ),
        (
            "evidencia.backend-s-externo.fronteira-subset-textual",
            11,
            "Reúne os testes de fronteira que chamam render_backend_s_external_subset e inspecionam o resultado em memória: recusas com mensagem específica (fonte fora do subset, parâmetro não `bombom`, condição de laço fora do recorte, `quebrar` fora de laço, composto fora das camadas 1–2, store frágil, parâmetro `u16`), aceitações de fronteira (quatro parâmetros com ABI completa, `talvez`/`senão`) e uma matriz auditável do subset montável. Prova mensagens e trechos de texto; não monta, não linka e não executa.",
        ),
        (
            "evidencia.backend-s-externo.validacao-estrutural-sintetica",
            5,
            "Constrói à mão um `SelectedProgram` (globais, funções, blocos, terminadores) sem passar pelo front-end e chama emit_external_toolchain_subset diretamente, exigindo recusa para global duplicada, salto para rótulo inexistente, rótulo duplicado e ramificação com alvo verdadeiro ou falso inexistente, validando a mensagem de diagnóstico. Não há front-end, arquivo, assembler, linker nem execução.",
        ),
    ];
    let helper_region: (&str, &str) = (
        "evidencia.backend-s-externo.pipeline-helper",
        "Executa o helper compartilhado render_backend_s_external_subset inteiramente em memória: parse e checagem semântica, lowering e validação por IR, CFG e seleção, seguidos da emissão montável hospedada via emit_external_toolchain_subset, que usa runtime_init=false. Não invoca assembler, linker ou binário; as ferramentas externas são chamadas somente por testes de fluxo real que consomem sua saída.",
    );

    let expected_ownership = [16usize, 8, 3, 2, 3, 22, 9, 11, 5];
    assert_eq!(
        expected_test_regions.map(|region| region.1),
        expected_ownership,
        "as contagens aprovadas da Onda 8H devem permanecer [16,8,3,2,3,22,9,11,5]"
    );
    assert_eq!(
        expected_ownership.iter().sum::<usize>(),
        79,
        "a soma do ownership 8H deve ser 79"
    );

    let expected_keys: HashSet<&str> = expected_test_regions
        .iter()
        .map(|region| region.0)
        .chain(std::iter::once(helper_region.0))
        .collect();
    assert_eq!(expected_keys.len(), 10, "as dez chaves 8H devem ser únicas");

    // Catálogo: crescimento exato e ausência de chaves duplicadas.
    let mut unique_catalog_keys = HashSet::new();
    for region in &catalog.regions {
        assert!(
            unique_catalog_keys.insert(region.key.as_str()),
            "chave duplicada no catálogo: {}",
            region.key
        );
    }
    let historical_catalog_total = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .count();
    assert_eq!(
        historical_catalog_total, 365,
        "o estado histórico da Onda 8H deve totalizar 365 regiões"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .count(),
        182,
        "o estado histórico da Onda 8H deve totalizar 182 regiões de evidência"
    );
    let externo_keys: HashSet<_> = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("evidencia.backend-s-externo."))
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        externo_keys, expected_keys,
        "o prefixo evidencia.backend-s-externo. deve conter exatamente as dez chaves 8H"
    );

    let central_source =
        fs::read_to_string(repository.join(central)).expect("suíte central da Onda 8H");
    let helper_source =
        fs::read_to_string(repository.join(helper_file)).expect("helpers compartilhados");
    let central_lines: Vec<_> = central_source.lines().collect();
    let helper_lines: Vec<_> = helper_source.lines().collect();

    // Distribuição por arquivo: nove regiões no arquivo central, uma no helper compartilhado.
    let central_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file == central)
        .collect();
    assert_eq!(
        central_regions.len(),
        9,
        "a Onda 8H deve registrar exatamente nove regiões em {central}"
    );
    let helper_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| expected_keys.contains(region.key.as_str()) && region.file == helper_file)
        .collect();
    assert_eq!(
        helper_regions.len(),
        1,
        "a Onda 8H deve registrar exatamente uma região em {helper_file}"
    );

    // Metadados, marcadores e conteúdo de cada uma das dez regiões.
    for (key, file, expected_summary) in expected_test_regions
        .iter()
        .map(|region| (region.0, central, region.2))
        .chain(std::iter::once((
            helper_region.0,
            helper_file,
            helper_region.1,
        )))
    {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("região 8H ausente: {key}"));
        assert_eq!(region.file, file, "arquivo divergente para {key}");
        assert_eq!(region.kind, "region", "kind divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some("backend-s"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(
            region.summary, expected_summary,
            "summary divergente para {key}"
        );
        assert_eq!(region.status, "active", "status divergente para {key}");
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "ordem inválida dos marcadores em {key}"
        );

        let lines = if file == central {
            &central_lines
        } else {
            &helper_lines
        };
        let content_lines = &lines[(region.content_start - 1)..region.content_end];
        assert!(
            !content_lines.is_empty() && content_lines.iter().any(|line| !line.trim().is_empty()),
            "conteúdo vazio em {key}"
        );
        assert!(
            content_lines
                .iter()
                .all(|line| !line.contains("@pinker-nav:")),
            "marcador absorvido pelo conteúdo de {key}"
        );
    }

    // Ordem física declarada == ordem física real, sem sobreposição entre regiões.
    let ordered_markers: Vec<_> = expected_test_regions
        .iter()
        .map(|region| {
            let entry = catalog.region(region.0).unwrap();
            (entry.start_marker, entry.end_marker)
        })
        .collect();
    assert!(
        ordered_markers.windows(2).all(|pair| pair[0].1 < pair[1].0),
        "as nove regiões 8H devem ser disjuntas e seguir a ordem física declarada"
    );

    // Ownership estrutural: 79 testes, cada um pertencente a exatamente uma região.
    let test_lines: Vec<usize> = central_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "#[test]")
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(
        test_lines.len(),
        79,
        "{central} deve manter exatamente 79 testes"
    );

    let mut unowned_tests: Vec<usize> = Vec::new();
    let mut duplicate_ownership: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut observed_ownership = vec![0usize; expected_test_regions.len()];
    for &test_line in &test_lines {
        let owners: Vec<_> = central_regions
            .iter()
            .filter(|region| region.content_start <= test_line && test_line <= region.content_end)
            .map(|region| region.key.as_str())
            .collect();
        match owners.len() {
            0 => unowned_tests.push(test_line),
            1 => {
                let index = expected_test_regions
                    .iter()
                    .position(|region| region.0 == owners[0])
                    .unwrap_or_else(|| panic!("owner fora das nove regiões 8H: {}", owners[0]));
                observed_ownership[index] += 1;
            }
            _ => duplicate_ownership.push((test_line, owners)),
        }
    }
    assert!(
        unowned_tests.is_empty(),
        "unowned_tests deve ser vazio, obteve {unowned_tests:?}"
    );
    assert!(
        duplicate_ownership.is_empty(),
        "duplicate_ownership deve ser vazio, obteve {duplicate_ownership:?}"
    );
    assert_eq!(
        observed_ownership, expected_ownership,
        "o ownership real divergiu de [16,8,3,2,3,22,9,11,5]"
    );
    assert!(
        observed_ownership.iter().all(|count| *count > 0),
        "toda região 8H deve possuir ao menos um teste"
    );

    // Contiguidade: nenhum teste do arquivo central cai entre duas regiões vizinhas.
    for pair in ordered_markers.windows(2) {
        assert!(
            !test_lines
                .iter()
                .any(|line| pair[0].1 < *line && *line < pair[1].0),
            "há teste fora de região entre as linhas {} e {}",
            pair[0].1,
            pair[1].0
        );
    }

    // O helper externo pertence só à nova região de helper.
    let helper_line = helper_lines
        .iter()
        .position(|line| {
            line.trim()
                .starts_with("pub fn render_backend_s_external_subset(")
        })
        .map(|index| index + 1)
        .expect("helper render_backend_s_external_subset deve continuar presente");
    let helper_owners: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file == helper_file
                && region.content_start <= helper_line
                && helper_line <= region.content_end
        })
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        helper_owners,
        [helper_region.0],
        "render_backend_s_external_subset deve pertencer somente à região de helper 8H"
    );

    // As regiões vizinhas da Onda 8G permanecem semanticamente preservadas.
    let preserved_neighbours: [(&str, &str, &str); 2] = [
        (
            "evidencia.backend-s.pipeline-helper",
            "render_backend_s",
            "Executa o helper compartilhado render_backend_s inteiramente em memória: parse e checagem semântica, lowering e validação por IR, CFG e seleção, seguidos da emissão do backend .s textual via emit_from_selected. Não usa o helper do subset externo, assembler, linker nem execução nativa.",
        ),
        (
            "evidencia.backend-s.apresentacao-cli-helper",
            "render_cli_asm_s_output",
            "Monta a apresentação sintética de render_cli_asm_s_output em memória: concatena o cabeçalho `=== ASM .S (TEXTUAL) ===`, a saída de render_backend_s e o rodapé histórico de sucesso semântico. Não cria nem executa um processo CLI.",
        ),
    ];
    for (key, symbol, summary) in preserved_neighbours {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("região vizinha ausente: {key}"));
        assert_eq!(region.file, helper_file, "arquivo divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some("backend-s"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.summary, summary, "summary alterado em {key}");
        assert_eq!(region.status, "active");
        let symbol_line = helper_lines
            .iter()
            .position(|line| line.trim().starts_with(&format!("pub fn {symbol}(")))
            .map(|index| index + 1)
            .unwrap_or_else(|| panic!("símbolo vizinho ausente: {symbol}"));
        assert!(
            region.content_start <= symbol_line && symbol_line <= region.content_end,
            "{symbol} saiu da região vizinha {key}"
        );
        assert!(
            symbol_line != helper_line,
            "a região vizinha {key} não pode absorver o helper externo"
        );
    }

    // Fronteiras semânticas congeladas, medidas apenas sobre as linhas executáveis.
    let central_code = central_lines
        .iter()
        .filter(|line| !line.trim_start().starts_with("// @pinker-nav:"))
        .fold(String::new(), |mut code, line| {
            code.push_str(line);
            code.push('\n');
            code
        });
    // O último span termina na fronteira cartografada, não no fim do arquivo: os
    // helpers locais detect_cc_driver/unique_temp_dir ficam fora de qualquer teste.
    let coverage_end = central_regions
        .iter()
        .map(|region| region.content_end)
        .max()
        .expect("regiões 8H no arquivo central");
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (index, &start) in test_lines.iter().enumerate() {
        let end = test_lines
            .get(index + 1)
            .map(|next| next - 1)
            .unwrap_or(coverage_end);
        spans.push((start, end));
    }
    let body = |span: (usize, usize)| central_lines[(span.0 - 1)..span.1].join("\n");

    let external_tests: Vec<_> = spans
        .iter()
        .copied()
        .filter(|span| body(*span).contains("detect_cc_driver()"))
        .collect();
    assert_eq!(
        external_tests.len(),
        31,
        "devem existir exatamente 31 testes que montam, linkam e executam de forma condicional"
    );
    for span in &external_tests {
        let text = body(*span);
        assert!(
            text.contains("cfg!(all(target_os = \"linux\", target_arch = \"x86_64\"))"),
            "teste condicional sem guarda de plataforma na linha {}",
            span.0
        );
        assert!(
            text.contains("Command::new(&driver)") && text.contains("Command::new(&bin_path)"),
            "teste condicional sem invocação do driver externo e do binário na linha {}",
            span.0
        );
        assert!(
            text.contains("run.status.code()"),
            "teste condicional sem validação de status.code() na linha {}",
            span.0
        );
        // O skip é silencioso e nunca é promovido a sucesso incondicional.
        assert_eq!(
            text.matches("return;").count(),
            2,
            "teste condicional deve manter os dois retornos silenciosos na linha {}",
            span.0
        );
        let owners: Vec<_> = central_regions
            .iter()
            .filter(|region| region.content_start <= span.0 && span.0 <= region.content_end)
            .map(|region| region.key.as_str())
            .collect();
        assert!(
            owners == ["evidencia.backend-s-externo.execucao-real-recortes-versionados"]
                || owners
                    == ["evidencia.backend-s-externo.execucao-real-abi-frame-interprocedural"],
            "teste condicional fora das duas regiões de execução real: {owners:?}"
        );
    }
    let in_memory_tests = spans
        .iter()
        .filter(|span| !body(**span).contains("Command::new"))
        .count();
    assert_eq!(
        in_memory_tests, 48,
        "devem existir exatamente 48 testes sem qualquer processo externo"
    );
    assert_eq!(
        external_tests.len() + in_memory_tests,
        79,
        "as duas classes devem particionar os 79 testes"
    );

    let refusal_tests = spans
        .iter()
        .filter(|span| body(**span).contains("unwrap_err()"))
        .count();
    assert_eq!(refusal_tests, 18, "devem existir exatamente 18 recusas");

    let synthetic_tests: Vec<_> = spans
        .iter()
        .copied()
        .filter(|span| body(*span).contains("SelectedProgram {"))
        .collect();
    assert_eq!(
        synthetic_tests.len(),
        5,
        "devem existir exatamente 5 casos com SelectedProgram sintético"
    );
    for span in &synthetic_tests {
        let text = body(*span);
        assert!(
            text.contains("emit_external_toolchain_subset(&program)"),
            "caso sintético deve chamar emit_external_toolchain_subset diretamente na linha {}",
            span.0
        );
        assert!(
            !text.contains("render_backend_s_external_subset"),
            "caso sintético não deve passar pelo front-end na linha {}",
            span.0
        );
        let owners: Vec<_> = central_regions
            .iter()
            .filter(|region| region.content_start <= span.0 && span.0 <= region.content_end)
            .map(|region| region.key.as_str())
            .collect();
        assert_eq!(
            owners,
            ["evidencia.backend-s-externo.validacao-estrutural-sintetica"],
            "caso sintético fora da região estrutural"
        );
    }

    assert_eq!(
        central_code.matches(".stdout").count(),
        0,
        "o arquivo central não pode validar stdout"
    );
    assert_eq!(
        central_code.matches("stderr").count(),
        31,
        "stderr só pode aparecer nas 31 mensagens de falha da compilação"
    );
    assert_eq!(
        central_code
            .matches("String::from_utf8_lossy(&compile.stderr)")
            .count(),
        31,
        "todo uso de stderr deve ser mensagem de falha, nunca conteúdo validado"
    );
    // A menção a libpinker_rt.a existe apenas nos marcadores de honestidade; o código
    // executável do arquivo central não referencia o runtime nativo.
    assert!(
        !central_code.contains("pinker_rt"),
        "o arquivo central não pode referenciar pinker_rt"
    );

    // O caminho hospedado do helper externo usa runtime_init=false.
    let backend_s_source =
        fs::read_to_string(repository.join("src/backend_s.rs")).expect("src/backend_s.rs");
    let hosted_entry = backend_s_source
        .split("pub fn emit_external_toolchain_subset(")
        .nth(1)
        .expect("entrada hospedada emit_external_toolchain_subset");
    let hosted_body = hosted_entry
        .split("pub fn ")
        .next()
        .expect("corpo da entrada hospedada");
    assert!(
        hosted_body.contains("render_external_x86_64_linux_callconv_impl(&program, false)"),
        "a entrada hospedada deve renderizar com runtime_init=false"
    );
    let helper_region_entry = catalog.region(helper_region.0).unwrap();
    let helper_body = helper_lines
        [(helper_region_entry.content_start - 1)..helper_region_entry.content_end]
        .join("\n");
    assert!(
        helper_body.contains("backend_s::emit_external_toolchain_subset(&selected)"),
        "o helper 8H deve delegar ao caminho hospedado"
    );
    assert!(
        !helper_body.contains("emit_external_toolchain_subset_nativo"),
        "o helper 8H não pode usar o caminho nativo"
    );

    // O backend nativo deixou de ser fronteira futura na Onda 8I: a classificação de
    // "não iniciado" foi substituída pela cobertura completa de
    // onda_8i_cartografa_evidencias_e_paridade_do_backend_nativo. Aqui resta apenas a
    // fronteira de escopo — o arquivo é cartografado por outra onda e nenhuma das dez
    // chaves 8H pode alcançá-lo.
    let nativo = "tests/backend_nativo_tests.rs";
    let nativo_source =
        fs::read_to_string(repository.join(nativo)).expect("suíte do backend nativo");
    assert_eq!(
        nativo_source
            .lines()
            .filter(|line| line.trim() == "#[test]")
            .count(),
        47,
        "{nativo} deve manter exatamente 47 testes"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file == nativo)
            .count(),
        14,
        "{nativo} é cartografado pela Onda 8I e deve manter 14 regiões"
    );
    assert!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file == nativo)
            .all(|region| region.key.starts_with("evidencia.backend-nativo.")),
        "nenhuma chave 8H pode cartografar {nativo}"
    );

    // Regeneração canônica e preservação semântica das 355 regiões anteriores.
    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    let previous_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !expected_keys.contains(region.key.as_str()))
        .filter(|region| !region.key.starts_with("evidencia.backend-nativo."))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .collect();
    assert_eq!(
        previous_regions.len(),
        355,
        "as 355 regiões anteriores devem ser preservadas semanticamente"
    );
    let previous_projection = stable_region_projection(previous_regions.into_iter());
    assert_eq!(
        (
            previous_projection.len(),
            fnv1a64(previous_projection.as_bytes()),
        ),
        (150_870, 15_749_653_826_456_761_089),
        "a projeção estável das 355 regiões anteriores mudou"
    );

    let onda_8h_complete = true;
    let onda_8_complete = false;
    let trama_complete = false;
    assert!(onda_8h_complete);
    assert!(!onda_8_complete);
    assert!(!trama_complete);
}

#[test]
fn onda_8i_cartografa_evidencias_e_paridade_do_backend_nativo() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let central = "tests/backend_nativo_tests.rs";

    // As catorze regiões da Onda 8I, em ordem física: (chave, é_suporte, testes).
    // Regiões de suporte possuem zero testes por design.
    let expected_regions: [(&str, bool, usize); 14] = [
        ("evidencia.backend-nativo.suporte-lowering-memoria", true, 0),
        ("evidencia.backend-nativo.emissao-init-runtime", false, 2),
        (
            "evidencia.backend-nativo.emissao-abi-e-fluxo-textual",
            false,
            5,
        ),
        (
            "evidencia.backend-nativo.emissao-simbolos-runtime-textual",
            false,
            7,
        ),
        ("evidencia.backend-nativo.suporte-driver-c", true, 0),
        (
            "evidencia.backend-nativo.execucao-exit-fumaca-abi",
            false,
            2,
        ),
        (
            "evidencia.backend-nativo.paridade-stdout-colecoes",
            false,
            3,
        ),
        (
            "evidencia.backend-nativo.suporte-matriz-paridade-b11",
            true,
            0,
        ),
        ("evidencia.backend-nativo.paridade-marco-b11", false, 1),
        ("evidencia.backend-nativo.suporte-paridade-stdout", true, 0),
        (
            "evidencia.backend-nativo.paridade-stdout-programas-maiores",
            false,
            7,
        ),
        ("evidencia.backend-nativo.paridade-argv", false, 1),
        (
            "evidencia.backend-nativo.execucao-exit-controle-fluxo",
            false,
            1,
        ),
        (
            "evidencia.backend-nativo.paridade-stdout-fases-avancadas",
            false,
            18,
        ),
    ];

    let expected_keys: HashSet<&str> = expected_regions.iter().map(|entry| entry.0).collect();
    assert_eq!(
        expected_keys.len(),
        14,
        "as catorze chaves 8I devem ser únicas"
    );
    let support_keys: Vec<&str> = expected_regions
        .iter()
        .filter(|entry| entry.1)
        .map(|entry| entry.0)
        .collect();
    let evidence_regions: Vec<(&str, usize)> = expected_regions
        .iter()
        .filter(|entry| !entry.1)
        .map(|entry| (entry.0, entry.2))
        .collect();
    assert_eq!(
        support_keys.len(),
        4,
        "a Onda 8I tem quatro regiões de suporte"
    );
    assert_eq!(
        evidence_regions.len(),
        10,
        "a Onda 8I tem dez regiões de evidência"
    );

    let expected_ownership: Vec<usize> = evidence_regions.iter().map(|entry| entry.1).collect();
    assert_eq!(
        expected_ownership,
        vec![2usize, 5, 7, 2, 3, 1, 7, 1, 1, 18],
        "o ownership aprovado da Onda 8I é [2,5,7,2,3,1,7,1,1,18]"
    );
    assert_eq!(
        expected_ownership.iter().sum::<usize>(),
        47,
        "a soma do ownership 8I deve ser 47"
    );

    // Catálogo: total absoluto novo, sem chaves duplicadas.
    let mut unique_catalog_keys = HashSet::new();
    for region in &catalog.regions {
        assert!(
            unique_catalog_keys.insert(region.key.as_str()),
            "chave duplicada no catálogo: {}",
            region.key
        );
    }
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .count(),
        379,
        "o estado histórico da Onda 8I deve totalizar 379 regiões"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .filter(|region| !region.key.starts_with("evidencia.runtime."))
            .count(),
        196,
        "o estado histórico da Onda 8I deve totalizar 196 regiões de evidência"
    );
    let nativo_keys: HashSet<_> = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("evidencia.backend-nativo."))
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        nativo_keys, expected_keys,
        "o prefixo evidencia.backend-nativo. deve conter exatamente as catorze chaves 8I"
    );

    let central_source = fs::read_to_string(repository.join(central)).expect("suíte 8I");
    let central_lines: Vec<_> = central_source.lines().collect();

    let central_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file == central)
        .collect();
    assert_eq!(
        central_regions.len(),
        14,
        "a Onda 8I deve registrar exatamente catorze regiões em {central}"
    );

    // Metadados e conteúdo de cada região.
    for (key, is_support, _) in expected_regions {
        let region = catalog
            .region(key)
            .unwrap_or_else(|| panic!("região 8I ausente: {key}"));
        assert_eq!(region.file, central, "arquivo divergente para {key}");
        assert_eq!(region.kind, "region", "kind divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some("backend-s"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.status, "active", "status divergente para {key}");
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "ordem inválida dos marcadores em {key}"
        );
        let content_lines = &central_lines[(region.content_start - 1)..region.content_end];
        assert!(
            !content_lines.is_empty() && content_lines.iter().any(|line| !line.trim().is_empty()),
            "conteúdo vazio em {key}"
        );
        assert!(
            content_lines
                .iter()
                .all(|line| !line.contains("@pinker-nav:")),
            "marcador absorvido pelo conteúdo de {key}"
        );
        if is_support {
            assert!(
                region.summary.contains("sem ownership direto de testes"),
                "a região de suporte {key} deve declarar que não possui testes próprios"
            );
        }
    }

    // Ordem física declarada == ordem física real, sem sobreposição, e contiguidade:
    // nenhuma linha executável do arquivo fica fora de alguma das catorze regiões.
    let ordered_markers: Vec<_> = expected_regions
        .iter()
        .map(|entry| {
            let region = catalog.region(entry.0).unwrap();
            (
                region.start_marker,
                region.content_start,
                region.content_end,
                region.end_marker,
            )
        })
        .collect();
    assert!(
        ordered_markers.windows(2).all(|pair| pair[0].3 < pair[1].0),
        "as catorze regiões 8I devem ser disjuntas e seguir a ordem física declarada"
    );
    for pair in ordered_markers.windows(2) {
        let between = &central_lines[pair[0].3..(pair[1].0 - 1)];
        assert!(
            between.iter().all(|line| line.trim().is_empty()),
            "há código executável fora de região entre as linhas {} e {}",
            pair[0].3,
            pair[1].0
        );
    }

    // O arquivo não contém referência a marcadores fora das catorze regiões esperadas.
    assert_eq!(
        central_source.matches("@pinker-nav:start ").count(),
        14,
        "{central} deve conter exatamente catorze marcadores de início"
    );
    assert_eq!(
        central_source.matches("@pinker-nav:end ").count(),
        14,
        "{central} deve conter exatamente catorze marcadores de fim"
    );
    assert_eq!(
        central_lines
            .iter()
            .filter(|line| line.contains("@pinker-nav"))
            .count(),
        70,
        "{central} deve conter exatamente 70 linhas de marcador (14 × 5)"
    );
    assert!(
        central_lines
            .iter()
            .filter(|line| line.contains("@pinker-nav"))
            .all(|line| line.starts_with("// @pinker-nav:")),
        "toda linha de marcador em {central} deve ser um comentário canônico"
    );

    // Ownership estrutural: 47 testes, cada um pertencente a exatamente uma região.
    let test_lines: Vec<usize> = central_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "#[test]")
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(
        test_lines.len(),
        47,
        "{central} deve manter exatamente 47 testes"
    );

    let mut unowned_tests: Vec<usize> = Vec::new();
    let mut duplicate_ownership: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut observed_ownership = vec![0usize; expected_regions.len()];
    for &test_line in &test_lines {
        let owners: Vec<_> = central_regions
            .iter()
            .filter(|region| region.content_start <= test_line && test_line <= region.content_end)
            .map(|region| region.key.as_str())
            .collect();
        match owners.len() {
            0 => unowned_tests.push(test_line),
            1 => {
                let index = expected_regions
                    .iter()
                    .position(|entry| entry.0 == owners[0])
                    .unwrap_or_else(|| panic!("owner fora das catorze regiões 8I: {}", owners[0]));
                observed_ownership[index] += 1;
            }
            _ => duplicate_ownership.push((test_line, owners)),
        }
    }
    assert!(
        unowned_tests.is_empty(),
        "unowned_tests deve ser vazio, obteve {unowned_tests:?}"
    );
    assert!(
        duplicate_ownership.is_empty(),
        "duplicate_ownership deve ser vazio, obteve {duplicate_ownership:?}"
    );
    assert_eq!(
        observed_ownership,
        expected_regions
            .iter()
            .map(|entry| entry.2)
            .collect::<Vec<_>>(),
        "o ownership real divergiu do inventário aprovado"
    );
    assert!(
        expected_regions
            .iter()
            .zip(&observed_ownership)
            .all(|(entry, observed)| if entry.1 {
                *observed == 0
            } else {
                *observed > 0
            }),
        "toda região de evidência deve possuir ao menos um teste e toda região de suporte, nenhum"
    );

    // Os símbolos esperados de cada região de suporte. Cada um é definido uma
    // única vez no arquivo, e essa definição pertence à região aprovada e a
    // nenhuma outra. Usos e call sites fora da região seguem livres: só a linha
    // de definição é vigiada, sem qualquer leitura semântica do corpo.
    let support_symbols: [(&str, &[&str]); 4] = [
        (
            "evidencia.backend-nativo.suporte-lowering-memoria",
            &["fn lower_to_selected("],
        ),
        (
            "evidencia.backend-nativo.suporte-driver-c",
            &["fn detect_cc_driver("],
        ),
        (
            "evidencia.backend-nativo.suporte-matriz-paridade-b11",
            &[
                "struct ParidadeNativaCaso {",
                "const ARGVS_FASE221:",
                "const CASOS_PARIDADE_B11:",
                "fn separar_stdout_e_retorno_interpretador(",
                "fn paridade_stdout_e_exit(",
            ],
        ),
        (
            "evidencia.backend-nativo.suporte-paridade-stdout",
            &["fn paridade_stdout("],
        ),
    ];
    for (key, symbols) in support_symbols {
        let region = catalog.region(key).unwrap();
        for symbol in symbols {
            let definidas: Vec<usize> = central_lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.trim_start().starts_with(symbol))
                .map(|(index, _)| index + 1)
                .collect();
            assert_eq!(
                definidas.len(),
                1,
                "{symbol} deve ter exatamente uma definição em {central}, obteve {definidas:?}"
            );
            assert!(
                region.content_start <= definidas[0] && definidas[0] <= region.content_end,
                "o símbolo {symbol} deve ser definido dentro da região de suporte {key}"
            );
            for outra in &central_regions {
                assert!(
                    outra.key == key
                        || !(outra.content_start <= definidas[0]
                            && definidas[0] <= outra.content_end),
                    "{symbol} não pode ser definido também em {}",
                    outra.key
                );
            }
        }
    }

    // Spans de teste limitados pela região proprietária: o suporte físico entre
    // blocos de teste nunca é absorvido pelo teste anterior.
    let spans: Vec<(usize, usize, &str)> = test_lines
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            let owner = central_regions
                .iter()
                .find(|region| region.content_start <= start && start <= region.content_end)
                .expect("todo teste 8I tem região proprietária");
            let mut end = owner.content_end;
            if let Some(next) = test_lines.get(index + 1) {
                end = end.min(next - 1);
            }
            (start, end, owner.key.as_str())
        })
        .collect();
    let body = |span: &(usize, usize, &str)| central_lines[(span.0 - 1)..span.1].join("\n");
    let count = |predicate: &dyn Fn(&str) -> bool| {
        spans.iter().filter(|span| predicate(&body(span))).count()
    };

    let processual = |text: &str| {
        text.contains("paridade_stdout(")
            || text.contains("paridade_stdout_e_exit(")
            || text.contains("Command::new")
    };
    let compara_stdout = |text: &str| {
        text.contains("paridade_stdout(")
            || text.contains("paridade_stdout_e_exit(")
            || text.contains("run.stdout")
    };

    assert_eq!(
        count(&|text| !processual(text)),
        14,
        "devem existir 14 testes exclusivamente em memória"
    );
    assert_eq!(
        count(&processual),
        33,
        "devem existir 33 testes processuais"
    );
    assert_eq!(
        count(&|text| !processual(text)) + count(&processual),
        47,
        "as duas classes devem particionar os 47 testes"
    );

    // Classificação das catorze regiões: 3 de evidência exclusivamente textual,
    // 7 de evidência processual e 4 de suporte. Nenhuma região de evidência
    // mistura as duas naturezas. A classificação é da região, não uma afirmação
    // de que a evidência processual foi de fato exercida.
    let regioes_textuais = [
        "evidencia.backend-nativo.emissao-init-runtime",
        "evidencia.backend-nativo.emissao-abi-e-fluxo-textual",
        "evidencia.backend-nativo.emissao-simbolos-runtime-textual",
    ];
    let mut evidencia_textual = 0usize;
    let mut evidencia_processual = 0usize;
    for (key, is_support, _) in expected_regions {
        if is_support {
            continue;
        }
        let corpos: Vec<String> = spans
            .iter()
            .filter(|span| span.2 == key)
            .map(&body)
            .collect();
        assert!(!corpos.is_empty(), "região de evidência sem teste: {key}");
        let toda_textual = corpos.iter().all(|texto| !processual(texto));
        let toda_processual = corpos.iter().all(|texto| processual(texto));
        assert!(
            toda_textual || toda_processual,
            "a região {key} mistura evidência textual e evidência processual"
        );
        if toda_textual {
            assert!(
                regioes_textuais.contains(&key),
                "região exclusivamente textual não declarada: {key}"
            );
            evidencia_textual += 1;
        } else {
            evidencia_processual += 1;
        }
    }
    assert_eq!(
        evidencia_textual, 3,
        "a Onda 8I tem três regiões de evidência exclusivamente textual"
    );
    assert_eq!(
        evidencia_processual, 7,
        "a Onda 8I tem sete regiões de evidência processual"
    );
    assert_eq!(
        evidencia_textual + evidencia_processual + support_keys.len(),
        14,
        "3 regiões textuais + 7 processuais + 4 de suporte = 14"
    );

    // Emissão textual: caminho hospedado versus caminho nativo.
    let central_code = central_lines
        .iter()
        .filter(|line| !line.starts_with("// @pinker-nav:"))
        .fold(String::new(), |mut code, line| {
            code.push_str(line);
            code.push('\n');
            code
        });
    let chamadas_nativas = central_code
        .matches("emit_external_toolchain_subset_nativo(")
        .count();
    let chamadas_hospedadas = central_code
        .matches("emit_external_toolchain_subset(")
        .count();
    assert_eq!(
        chamadas_nativas, 1,
        "deve existir exatamente uma chamada direta ao emissor nativo"
    );
    assert_eq!(
        chamadas_hospedadas, 13,
        "devem existir exatamente 13 chamadas diretas ao emissor hospedado"
    );

    // Os sete testes de símbolos do runtime percorrem o caminho HOSPEDADO.
    let simbolos: Vec<_> = spans
        .iter()
        .filter(|span| span.2 == "evidencia.backend-nativo.emissao-simbolos-runtime-textual")
        .collect();
    assert_eq!(
        simbolos.len(),
        7,
        "devem existir sete testes de símbolos do runtime"
    );
    for span in &simbolos {
        let text = body(span);
        assert!(
            text.contains("emit_external_toolchain_subset(&selected)"),
            "teste de símbolos fora do caminho hospedado na linha {}",
            span.0
        );
        assert!(
            !text.contains("emit_external_toolchain_subset_nativo("),
            "teste de símbolos não pode usar o caminho nativo na linha {}",
            span.0
        );
        assert!(
            text.contains("pinker_"),
            "teste de símbolos sem referência textual a pinker_ na linha {}",
            span.0
        );
        assert!(
            !text.contains("Command::new"),
            "teste de símbolos não pode criar processo na linha {}",
            span.0
        );
    }

    // Inventário dos caminhos processuais: build nativo, montagem/linkedição e
    // execução do ELF. As contagens abaixo são estruturais — descrevem o que o
    // arquivo declara, não que a comparação tenha sido semanticamente exercida.
    assert_eq!(
        count(&|text| processual(text) && compara_stdout(text)),
        30,
        "devem existir 30 caminhos inventariados de comparação de stdout"
    );
    assert_eq!(
        count(&|text| processual(text) && !compara_stdout(text)),
        3,
        "devem existir 3 testes que declaram validar apenas o exit"
    );
    assert_eq!(
        count(&|text| text.contains("paridade_stdout_e_exit(")),
        1,
        "paridade_stdout_e_exit deve ter um único call site consumidor"
    );
    assert_eq!(
        count(&|text| text.contains("paridade_stdout(")),
        25,
        "devem existir 25 call sites de paridade_stdout"
    );
    assert_eq!(
        count(&|text| text.contains("argv") || text.contains("CASOS_PARIDADE_B11")),
        2,
        "devem existir dois pontos que declaram argv em nível de teste"
    );

    // Guardas de plataforma, driver e runtime: sete conjuntos escritos inline.
    // A cartografia inventaria onde essas guardas aparecem; não afirma qual
    // caminho elas realmente cobrem nem se algum teste foi degradado a no-op.
    let guarda_inline = |text: &str| {
        text.contains("cfg!(all(target_os = \"linux\", target_arch = \"x86_64\"))")
            && text.contains("detect_cc_driver().is_none()")
            && text.contains("runtime_lib.is_file()")
    };
    assert_eq!(
        count(&guarda_inline),
        7,
        "devem existir sete conjuntos de guardas escritos inline"
    );
    for span in spans.iter().filter(|span| guarda_inline(&body(span))) {
        let text = body(span);
        assert!(
            text.contains("\"--nativo\"") && text.contains("PINKER_RT_LIB"),
            "caminho com guarda inline sem referência textual a --nativo e PINKER_RT_LIB na linha {}",
            span.0
        );
        assert!(
            text.contains("run.status.code()"),
            "caminho com guarda inline sem referência textual a run.status.code() na linha {}",
            span.0
        );
    }

    // A matriz B11: 14 casos, 13 exemplos distintos, fase221 com e sem argv.
    let matriz = central_code
        .split("const CASOS_PARIDADE_B11: &[ParidadeNativaCaso] = &[")
        .nth(1)
        .expect("matriz CASOS_PARIDADE_B11")
        .split("\n];")
        .next()
        .expect("fim da matriz CASOS_PARIDADE_B11");
    assert_eq!(
        matriz.matches("ParidadeNativaCaso {").count(),
        14,
        "CASOS_PARIDADE_B11 deve conter 14 casos"
    );
    let exemplos: Vec<&str> = matriz
        .lines()
        .filter(|line| line.trim_start().starts_with("exemplo:"))
        .map(|line| line.split('"').nth(1).expect("exemplo entre aspas"))
        .collect();
    assert_eq!(exemplos.len(), 14, "todo caso B11 declara um exemplo");
    assert_eq!(
        exemplos.iter().collect::<HashSet<_>>().len(),
        13,
        "a matriz B11 cobre 13 exemplos distintos"
    );
    assert_eq!(
        exemplos
            .iter()
            .filter(|exemplo| exemplo.contains("fase221"))
            .count(),
        2,
        "fase221 deve aparecer duas vezes na matriz B11"
    );
    assert_eq!(
        matriz.matches("argv: ARGVS_FASE221").count(),
        1,
        "exatamente um caso B11 passa argv"
    );
    assert_eq!(
        matriz.matches("argv: &[],").count(),
        13,
        "os outros treze casos B11 não passam argv"
    );

    // Regeneração canônica e preservação das 365 regiões anteriores.
    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    let previous_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !expected_keys.contains(region.key.as_str()))
        .filter(|region| !region.key.starts_with("evidencia.runtime."))
        .collect();
    assert_eq!(
        previous_regions.len(),
        365,
        "as 365 regiões anteriores devem ser preservadas semanticamente"
    );
    let previous_projection = stable_region_projection(previous_regions.into_iter());
    assert_eq!(
        (
            previous_projection.len(),
            fnv1a64(previous_projection.as_bytes()),
        ),
        (157_379, 14_667_879_393_081_127_943),
        "a projeção estável das 365 regiões anteriores mudou"
    );

    let onda_8i_complete = true;
    let onda_8_complete = false;
    let trama_complete = false;
    assert!(onda_8i_complete);
    assert!(!onda_8_complete);
    assert!(!trama_complete);
}

/// A Onda 8J cartografa exclusivamente o `#[cfg(test)] mod tests` de
/// `runtime/pinker_rt/src/lib.rs` — 22 testes e um helper — em sete regiões
/// `evidencia.runtime.*`. A produção do runtime já fora cartografada pela Onda
/// 6E em 15 regiões `runtime.*`, que esta onda não altera nem recartografa: a
/// cápsula da 6E continua cobrindo somente produção e o `mod tests` permanece
/// fora da camada `runtime`.
///
/// O gate é estritamente cartográfico. Prova catálogo (totais, conjunto exato
/// de chaves, regeneração canônica), arquivo (contagem de testes, helper
/// único, ordem física, contiguidade, ausência de sobreposição e de teste sem
/// dono), ownership `[4,2,3,5,4,3,1]`, classificação (camada `evidencia`,
/// evidência em memória, arquivo do runtime), preservação da produção (as 15
/// chaves, 155 símbolos produtivos com ownership único, 107 símbolos de ABI) e
/// congelamento das 379 regiões anteriores.
///
/// Não afirma correção semântica das asserções, segurança de memória, ausência
/// de vazamentos, equivalência com o interpretador, execução de ELF, completude
/// de ABI, dataflow, resolução de macros nem alcançabilidade. As relações entre
/// região de produção e evidência interna são estruturais, por menção/call site.
#[test]
fn onda_8j_cartografa_evidencias_internas_do_runtime() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&path).expect("catálogo de código versionado");

    let central = "runtime/pinker_rt/src/lib.rs";

    // As sete regiões da Onda 8J, em ordem física: (chave, domínio, testes).
    let expected_regions: [(&str, &str, usize); 7] = [
        ("evidencia.runtime.memoria-alocador", "memoria", 4),
        ("evidencia.runtime.inicializacao-abi", "inicializacao", 2),
        ("evidencia.runtime.texto-verso", "texto", 3),
        ("evidencia.runtime.listas-dinamicas", "listas", 5),
        ("evidencia.runtime.mapas-dinamicos", "mapas", 4),
        ("evidencia.runtime.leques-carga", "leques", 3),
        ("evidencia.runtime.mapas-iterador-snapshot", "mapas", 1),
    ];

    let expected_keys: HashSet<&str> = expected_regions.iter().map(|entry| entry.0).collect();
    assert_eq!(expected_keys.len(), 7, "as sete chaves 8J devem ser únicas");

    let expected_ownership: Vec<usize> = expected_regions.iter().map(|entry| entry.2).collect();
    assert_eq!(
        expected_ownership,
        vec![4usize, 2, 3, 5, 4, 3, 1],
        "o ownership aprovado da Onda 8J é [4,2,3,5,4,3,1]"
    );
    assert_eq!(
        expected_ownership.iter().sum::<usize>(),
        22,
        "a soma do ownership 8J deve ser 22"
    );

    // 1. CATÁLOGO.
    let mut unique_catalog_keys = HashSet::new();
    for region in &catalog.regions {
        assert!(
            unique_catalog_keys.insert(region.key.as_str()),
            "chave duplicada no catálogo: {}",
            region.key
        );
    }
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .count(),
        386,
        "a Onda 8J deve totalizar 386 regiões"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        203,
        "a Onda 8J deve totalizar 203 regiões de evidência"
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs")
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15,
        "a camada runtime permanece com as 15 regiões de produção da Onda 6E"
    );
    let evidencia_runtime_keys: HashSet<_> = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("evidencia.runtime."))
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        evidencia_runtime_keys, expected_keys,
        "o prefixo evidencia.runtime. deve conter exatamente as sete chaves 8J"
    );

    let regenerated = CodeIndex::scan_repo(&repository)
        .expect("regeneração canônica do catálogo a partir das fontes");
    assert!(
        regenerated.verify().is_empty(),
        "regeneração canônica inválida: {:?}",
        regenerated.verify()
    );
    let versioned = fs::read_to_string(&path).expect("catálogo JSONL versionado");
    assert_eq!(
        versioned,
        regenerated.render_jsonl(),
        "src/navigation.jsonl diverge da regeneração canônica"
    );

    // 2. ARQUIVO.
    let central_source = fs::read_to_string(repository.join(central)).expect("runtime nativo");
    let central_lines: Vec<_> = central_source.lines().collect();

    let evidence_regions: Vec<_> = expected_regions
        .iter()
        .map(|entry| {
            catalog
                .region(entry.0)
                .unwrap_or_else(|| panic!("região 8J ausente: {}", entry.0))
        })
        .collect();

    for (region, (key, domain, _)) in evidence_regions.iter().zip(expected_regions) {
        assert_eq!(region.file, central, "arquivo divergente para {key}");
        assert_eq!(region.kind, "region", "kind divergente para {key}");
        assert_eq!(region.domain.as_deref(), Some(domain), "domínio de {key}");
        assert_eq!(
            region.layer.as_deref(),
            Some("evidencia"),
            "camada de {key}"
        );
        assert_eq!(region.status, "active", "status divergente para {key}");
        assert!(
            region.start_marker < region.content_start
                && region.content_start <= region.content_end
                && region.content_end < region.end_marker,
            "ordem inválida dos marcadores em {key}"
        );
        let content_lines = &central_lines[(region.content_start - 1)..region.content_end];
        assert!(
            content_lines.iter().any(|line| !line.trim().is_empty()),
            "conteúdo vazio em {key}"
        );
        assert!(
            content_lines
                .iter()
                .all(|line| !line.contains("@pinker-nav:")),
            "marcador absorvido pelo conteúdo de {key}"
        );
    }

    // Ordem física declarada == ordem física real, sem sobreposição, contígua.
    let ordered: Vec<(usize, usize, usize, usize)> = evidence_regions
        .iter()
        .map(|region| {
            (
                region.start_marker,
                region.content_start,
                region.content_end,
                region.end_marker,
            )
        })
        .collect();
    assert!(
        ordered.windows(2).all(|pair| pair[0].3 < pair[1].0),
        "as sete regiões 8J devem ser disjuntas e seguir a ordem física declarada"
    );
    for pair in ordered.windows(2) {
        let between = &central_lines[pair[0].3..(pair[1].0 - 1)];
        assert!(
            between.iter().all(|line| line.trim().is_empty()),
            "há código fora de região entre as linhas {} e {}",
            pair[0].3,
            pair[1].0
        );
    }

    // O bloco cartografado abre no `#[cfg(test)]` e fecha no `}` do módulo.
    let bloco_inicio = ordered[0].1;
    let bloco_fim = ordered[6].2;
    assert_eq!(
        central_lines[bloco_inicio - 1].trim(),
        "#[cfg(test)]",
        "a primeira região 8J deve abrir no atributo #[cfg(test)]"
    );
    assert_eq!(
        central_lines[bloco_inicio].trim(),
        "mod tests {",
        "a primeira região 8J deve abranger a abertura de mod tests"
    );
    assert_eq!(
        central_lines[bloco_inicio + 1].trim(),
        "use super::*;",
        "a primeira região 8J deve abranger o use super::*;"
    );
    assert_eq!(
        central_lines[bloco_fim - 1],
        "}",
        "a última região 8J deve terminar no fechamento físico de mod tests"
    );
    assert_eq!(
        ordered[6].3,
        central_lines.len(),
        "o marcador de fim da última região 8J é a última linha do arquivo"
    );

    // Marcadores: 15 regiões de produção (6E) + 7 de evidência (8J).
    assert_eq!(
        central_source.matches("@pinker-nav:start ").count(),
        22,
        "{central} deve conter 22 marcadores de início (15 produção + 7 evidência)"
    );
    assert_eq!(
        central_source.matches("@pinker-nav:end ").count(),
        22,
        "{central} deve conter 22 marcadores de fim (15 produção + 7 evidência)"
    );
    assert_eq!(
        central_source
            .matches("@pinker-nav:start evidencia.runtime.")
            .count(),
        7,
        "{central} deve conter sete marcadores de início evidencia.runtime."
    );
    assert_eq!(
        central_source
            .matches("@pinker-nav:end evidencia.runtime.")
            .count(),
        7,
        "{central} deve conter sete marcadores de fim evidencia.runtime."
    );
    let marker_lines_bloco = central_lines
        .iter()
        .enumerate()
        .filter(|(index, line)| index + 1 >= ordered[0].0 && line.contains("@pinker-nav"))
        .count();
    assert_eq!(
        marker_lines_bloco, 35,
        "as sete regiões 8J somam 35 linhas de marcador (7 × 5)"
    );
    assert!(
        central_lines
            .iter()
            .filter(|line| line.contains("@pinker-nav"))
            .all(|line| line.trim_start().starts_with("// @pinker-nav:")),
        "toda linha de marcador em {central} deve ser um comentário canônico"
    );

    // 3. OWNERSHIP.
    let test_lines: Vec<usize> = central_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "#[test]")
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(
        test_lines.len(),
        22,
        "{central} deve manter exatamente 22 testes"
    );
    assert!(
        test_lines
            .iter()
            .all(|line| bloco_inicio <= *line && *line <= bloco_fim),
        "todo #[test] deve estar dentro do bloco cartografado"
    );

    let mut unowned_tests: Vec<usize> = Vec::new();
    let mut duplicate_ownership: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut observed_ownership = vec![0usize; expected_regions.len()];
    for &test_line in &test_lines {
        let owners: Vec<_> = evidence_regions
            .iter()
            .filter(|region| region.content_start <= test_line && test_line <= region.content_end)
            .map(|region| region.key.as_str())
            .collect();
        match owners.len() {
            0 => unowned_tests.push(test_line),
            1 => {
                let index = expected_regions
                    .iter()
                    .position(|entry| entry.0 == owners[0])
                    .unwrap_or_else(|| panic!("owner fora das sete regiões 8J: {}", owners[0]));
                observed_ownership[index] += 1;
            }
            _ => duplicate_ownership.push((test_line, owners)),
        }
    }
    assert!(
        unowned_tests.is_empty(),
        "unowned_tests deve ser vazio, obteve {unowned_tests:?}"
    );
    assert!(
        duplicate_ownership.is_empty(),
        "duplicate_ownership deve ser vazio, obteve {duplicate_ownership:?}"
    );
    let evidence_regions_without_tests: Vec<&str> = expected_regions
        .iter()
        .zip(&observed_ownership)
        .filter(|(_, observed)| **observed == 0)
        .map(|(entry, _)| entry.0)
        .collect();
    assert!(
        evidence_regions_without_tests.is_empty(),
        "evidence_regions_without_tests deve ser vazio, obteve {evidence_regions_without_tests:?}"
    );
    assert_eq!(
        observed_ownership, expected_ownership,
        "o ownership real divergiu do inventário aprovado"
    );
    assert_eq!(
        observed_ownership.iter().sum::<usize>(),
        22,
        "ownership_sum deve ser 22"
    );

    // O único helper aprovado do bloco é `verso_de`, e ele pertence a
    // `evidencia.runtime.texto-verso`. Helper = `fn` do bloco cujo atributo
    // anterior não é `#[test]`. Nenhuma leitura semântica do corpo.
    let helpers: Vec<(usize, &str)> = (bloco_inicio..=bloco_fim)
        .filter(|line| central_lines[line - 1].trim_start().starts_with("fn "))
        .filter(|line| central_lines[line - 2].trim() != "#[test]")
        .map(|line| (line, central_lines[line - 1].trim()))
        .collect();
    assert_eq!(
        helpers.len(),
        1,
        "o bloco de testes deve conter exatamente um helper, obteve {helpers:?}"
    );
    assert!(
        helpers[0].1.starts_with("fn verso_de("),
        "o único helper aprovado é verso_de, obteve {}",
        helpers[0].1
    );
    let texto_verso = catalog.region("evidencia.runtime.texto-verso").unwrap();
    assert!(
        texto_verso.content_start <= helpers[0].0 && helpers[0].0 <= texto_verso.content_end,
        "verso_de deve ser definido dentro de evidencia.runtime.texto-verso"
    );
    for region in &evidence_regions {
        assert!(
            region.key == "evidencia.runtime.texto-verso"
                || !(region.content_start <= helpers[0].0 && helpers[0].0 <= region.content_end),
            "verso_de não pode pertencer também a {}",
            region.key
        );
    }

    // 4. CLASSIFICAÇÃO: toda a evidência da Onda 8J é em memória. Nenhum teste
    // do bloco cria processo, toca o sistema de arquivos ou invoca toolchain.
    let bloco = central_lines[(bloco_inicio - 1)..bloco_fim].join("\n");
    for marca in [
        "Command::new",
        "std::process",
        "std::fs",
        "File::",
        "env!(",
        "cargo",
    ] {
        assert!(
            !bloco.contains(marca),
            "a evidência 8J é em memória: {marca} não pode aparecer no bloco de testes"
        );
    }

    // 5. PRODUÇÃO PRESERVADA (Onda 6E).
    let expected_production_keys: HashSet<&str> = HashSet::from([
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
    ]);
    let production_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.key.starts_with("runtime."))
        .collect();
    let production_keys: HashSet<&str> = production_regions
        .iter()
        .map(|region| region.key.as_str())
        .collect();
    assert_eq!(
        production_keys, expected_production_keys,
        "as quinze chaves runtime.* da Onda 6E devem permanecer exatamente as mesmas"
    );
    for region in &production_regions {
        assert_eq!(region.file, central, "região de produção fora do runtime");
        assert_eq!(region.layer.as_deref(), Some("runtime"));
        assert!(
            region.end_marker < ordered[0].0,
            "a região de produção {} invadiu o bloco de testes",
            region.key
        );
    }
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.file == central)
            .count(),
        22,
        "{central} deve registrar 22 regiões (15 de produção + 7 de evidência)"
    );

    // 155 símbolos produtivos: 147 definições textuais antes do bloco de testes
    // mais os 8 wrappers `pinker_formatar_verso_1..8` gerados por
    // `formatar_wrappers!`. Cada definição textual pertence a exatamente uma das
    // quinze regiões da 6E. Contagem estrutural; não resolve macros.
    let e_definicao = |line: &str| {
        let t = line
            .trim_start()
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ")
            .trim_start_matches("unsafe ")
            .trim_start_matches("extern \"C\" ");
        ["fn ", "struct ", "enum ", "const ", "static ", "type "]
            .iter()
            .any(|prefixo| t.starts_with(prefixo))
    };
    let definicoes: Vec<usize> = (1..bloco_inicio)
        .filter(|line| e_definicao(central_lines[line - 1]))
        .collect();
    assert_eq!(
        definicoes.len(),
        147,
        "a produção do runtime deve manter 147 definições textuais"
    );
    let wrappers_gerados = 8usize;
    assert_eq!(
        definicoes.len() + wrappers_gerados,
        155,
        "147 definições textuais + 8 wrappers gerados = 155 símbolos produtivos"
    );
    let mut definicoes_sem_dono: Vec<usize> = Vec::new();
    let mut definicoes_com_dono_duplo: Vec<usize> = Vec::new();
    for &linha in &definicoes {
        let donos = production_regions
            .iter()
            .filter(|region| region.content_start <= linha && linha <= region.content_end)
            .count();
        match donos {
            0 => definicoes_sem_dono.push(linha),
            1 => {}
            _ => definicoes_com_dono_duplo.push(linha),
        }
    }
    assert!(
        definicoes_sem_dono.is_empty() && definicoes_com_dono_duplo.is_empty(),
        "os 155 símbolos produtivos devem manter ownership único, obteve sem dono {definicoes_sem_dono:?} e duplicados {definicoes_com_dono_duplo:?}"
    );

    // 107 símbolos de ABI: 99 `extern "C" fn` nomeadas na produção mais os 8
    // wrappers gerados pela macro.
    let producao = central_lines[..(ordered[0].0 - 1)].join("\n");
    let abi_nomeadas = producao
        .match_indices("extern \"C\" fn ")
        .filter(|(index, marca)| {
            !producao[(index + marca.len())..].starts_with('$')
                && !producao[(index + marca.len())..].is_empty()
        })
        .count();
    assert_eq!(
        abi_nomeadas, 99,
        "a produção do runtime deve manter 99 funções extern \"C\" nomeadas"
    );
    assert_eq!(
        abi_nomeadas + wrappers_gerados,
        107,
        "99 funções ABI diretas + 8 wrappers gerados = 107 símbolos de ABI"
    );
    assert_eq!(
        producao.matches("formatar_wrappers!(").count(),
        1,
        "há exatamente uma expansão de formatar_wrappers!"
    );
    for indice in 1..=wrappers_gerados {
        assert!(
            producao.contains(&format!("pinker_formatar_verso_{indice},")),
            "o wrapper pinker_formatar_verso_{indice} deve continuar declarado na expansão"
        );
    }

    // 7. CONTAGENS FACTUAIS: seis regiões de produção têm alguma evidência
    // interna relacionada (por menção/call site de um símbolo de ABI que a
    // região define) e nove não têm. Relação estrutural, não semântica.
    let sem_evidencia_interna = [
        "runtime.conversoes.numero-texto",
        "runtime.texto.formatacao",
        "runtime.io.saida",
        "runtime.arquivos.io",
        "runtime.caminhos.sistema",
        "runtime.tempo.relogio",
        "runtime.aleatorio.gerador",
        "runtime.ambiente.argumentos",
        "runtime.processos.execucao",
    ];
    let mut com_evidencia = 0usize;
    let mut sem_evidencia: Vec<&str> = Vec::new();
    for region in &production_regions {
        let corpo = central_lines[(region.content_start - 1)..region.content_end].join("\n");
        let mut simbolos: Vec<String> = corpo
            .match_indices("extern \"C\" fn ")
            .map(|(index, marca)| {
                corpo[(index + marca.len())..]
                    .split(['(', '<', ' '])
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .filter(|nome| !nome.is_empty() && !nome.starts_with('$'))
            .collect();
        if corpo.contains("formatar_wrappers!(") {
            for indice in 1..=wrappers_gerados {
                simbolos.push(format!("pinker_formatar_verso_{indice}"));
            }
        }
        if simbolos
            .iter()
            .any(|nome| bloco.contains(&format!("{nome}(")))
        {
            com_evidencia += 1;
        } else {
            sem_evidencia.push(region.key.as_str());
        }
    }
    assert_eq!(
        com_evidencia, 6,
        "seis regiões de produção têm ao menos um teste interno relacionado"
    );
    sem_evidencia.sort_unstable();
    let mut esperado_sem: Vec<&str> = sem_evidencia_interna.to_vec();
    esperado_sem.sort_unstable();
    assert_eq!(
        sem_evidencia, esperado_sem,
        "os nove domínios de produção sem teste interno relacionado divergiram"
    );

    // 6. HISTÓRICO: as 379 regiões anteriores permanecem congeladas.
    let previous_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| {
            region.file != "tests/nav_catalog_tests.rs"
                && region.file != "tests/doc_catalog_tests.rs"
                && region.file != "tests/trama_query_tests.rs"
        })
        .filter(|region| !expected_keys.contains(region.key.as_str()))
        .collect();
    assert_eq!(
        previous_regions.len(),
        379,
        "as 379 regiões anteriores devem ser preservadas semanticamente"
    );
    let previous_projection = stable_region_projection(previous_regions.into_iter());
    assert_eq!(
        (
            previous_projection.len(),
            fnv1a64(previous_projection.as_bytes()),
        ),
        (165_434, 12_395_117_943_166_741_653),
        "a projeção estável das 379 regiões anteriores mudou"
    );

    let onda_8j_complete = true;
    let onda_8_complete = false;
    let trama_complete = false;
    assert!(onda_8j_complete);
    assert!(!onda_8_complete);
    assert!(!trama_complete);
}

/// Terceira cápsula operacional/documental da Trama: `tests/trama_query_tests.rs`.
/// Congela as oito regiões marker-only (2 de suporte + 6 de evidência), o
/// ownership exato de 4 constantes + 6 helpers + 10 testes, a prova de que
/// retirar os marcadores reconstrói a fonte-base, os totais 406/223/15 e a
/// preservação integral das 398 regiões predecessoras, das 392 pós-nav-catalog
/// e das 386 da Onda 8. O texto semelhante a marcador dentro do literal Rust
/// `SRC` permanece conteúdo de fixture e não é contado como marcador real.
#[test]
fn capsula_trama_query_cartografa_suporte_e_dez_testes() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_path = "tests/trama_query_tests.rs";
    let source = fs::read_to_string(repository.join(target_path)).expect("fonte da cápsula");
    let lines: Vec<_> = source.lines().collect();
    let expected_keys = [
        "evidencia.trama.query.fixture-config",
        "evidencia.trama.query.process-support",
        "evidencia.trama.query.catalog-only",
        "evidencia.trama.query.source-drift",
        "evidencia.trama.query.json-stability",
        "evidencia.trama.query.result-limit",
        "evidencia.trama.query.catalog-errors",
        "evidencia.trama.query.query-exit-codes",
    ];
    let support_keys = &expected_keys[..2];
    let evidence_keys = &expected_keys[2..];

    // A. e F. Estrutura física: oito regiões balanceadas, contíguas, sem
    // aninhamento nem sobreposição, na ordem exata declarada.
    #[derive(Debug)]
    struct Span {
        key: String,
        start: usize,
        end: usize,
    }
    let mut spans = Vec::new();
    let mut active: Option<(String, usize)> = None;
    let mut marker_lines = 0;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("// @pinker-nav:") {
            continue;
        }
        marker_lines += 1;
        if let Some(key) = trimmed.strip_prefix("// @pinker-nav:start ") {
            assert!(active.is_none(), "regiões da cápsula não podem aninhar");
            active = Some((key.to_string(), index + 1));
        } else if let Some(key) = trimmed.strip_prefix("// @pinker-nav:end ") {
            let (open_key, start) = active.take().expect("fim sem início");
            assert_eq!(key, open_key, "marcadores inicial/final divergentes");
            spans.push(Span {
                key: open_key,
                start,
                end: index + 1,
            });
        }
    }
    assert!(active.is_none(), "região sem marcador final");
    assert_eq!(marker_lines, 40, "a cápsula exige exatamente 40 marcadores");
    // Distinção literal-de-string: o texto `@pinker-nav:` embutido no literal
    // Rust `SRC` aparece em uma linha física adicional que NÃO é marcador real.
    let text_occurrences = lines
        .iter()
        .filter(|line| line.contains("@pinker-nav:"))
        .count();
    assert_eq!(
        text_occurrences, 41,
        "40 marcadores reais + 1 linha de fixture SRC com texto semelhante a marcador"
    );
    assert_eq!(spans.len(), 8, "a cápsula exige exatamente oito regiões");
    let physical_keys: Vec<_> = spans.iter().map(|span| span.key.as_str()).collect();
    assert_eq!(
        physical_keys, expected_keys,
        "ordem física das oito regiões"
    );
    for pair in spans.windows(2) {
        assert!(pair[0].end < pair[1].start, "regiões não podem sobrepor");
        for line in &lines[pair[0].end..pair[1].start - 1] {
            assert!(
                line.trim().is_empty(),
                "as regiões devem ser contíguas, separadas apenas por linhas em branco"
            );
        }
    }
    assert_eq!(
        spans[0].start, 13,
        "docs e imports devem ficar fora das regiões"
    );
    assert_eq!(
        spans[7].end,
        lines.len(),
        "a última região deve terminar no fim do arquivo"
    );
    assert!(lines[..12]
        .iter()
        .all(|line| !line.contains("@pinker-nav:")));

    // G. Prova marker-only contra a SHA-256 da fonte-base cartografada.
    let stripped: String = source
        .split_inclusive('\n')
        .filter(|line| {
            !line
                .trim_start()
                .strip_prefix("//")
                .is_some_and(|rest| rest.trim_start().starts_with("@pinker-nav:"))
        })
        .collect();
    assert_eq!(
        sha256_hex(stripped.as_bytes()),
        "65ba7f360174b1ef15ab5b9e246e3a0fce8e888359798898986969ff9c016f3f",
        "retirar marcadores deve reconstruir exatamente a fonte-base"
    );

    // E. Ownership exato: 4 constantes, 6 helpers, 10 testes, sem órfãos,
    // duplicatas ou sobreposição de propriedade.
    #[derive(Default, Debug, PartialEq, Eq)]
    struct Owned {
        constants: Vec<String>,
        helpers: Vec<String>,
        tests: Vec<String>,
    }
    let mut ownership = Vec::new();
    for span in &spans {
        let mut owned = Owned::default();
        let mut test_attribute = false;
        for line in &lines[span.start..span.end - 1] {
            let trimmed = line.trim();
            if trimmed == "#[test]" {
                test_attribute = true;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("const ") {
                owned
                    .constants
                    .push(rest.split(':').next().unwrap().to_string());
                test_attribute = false;
            } else if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap().to_string();
                if test_attribute {
                    owned.tests.push(name);
                } else {
                    owned.helpers.push(name);
                }
                test_attribute = false;
            }
        }
        ownership.push((span.key.as_str(), owned));
    }
    let expected_ownership = [
        (
            expected_keys[0],
            Owned {
                constants: ["DOC_TOML", "PORTAL", "CORE", "SRC"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[1],
            Owned {
                helpers: ["temp_repo", "write", "fixture", "doc", "nav", "code"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[2],
            Owned {
                tests: [
                    "consulta_documental_le_catalogo_sem_revarrer_markdown",
                    "consulta_de_codigo_le_catalogo_sem_revarrer_rust",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[3],
            Owned {
                tests: [
                    "mostrar_detecta_ancora_divergente",
                    "nav_mostrar_detecta_hash_divergente",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[4],
            Owned {
                tests: ["saida_json_e_valida_e_estavel"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[5],
            Owned {
                tests: ["limite_de_resultados_respeita_contornos"]
                    .map(str::to_string)
                    .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[6],
            Owned {
                tests: [
                    "catalogo_ausente_falha_com_codigo_3",
                    "catalogo_invalido_falha_com_codigo_3",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
        (
            expected_keys[7],
            Owned {
                tests: [
                    "ausencia_de_resultados_nao_e_sucesso_silencioso",
                    "uso_invalido_sai_com_codigo_2",
                ]
                .map(str::to_string)
                .to_vec(),
                ..Owned::default()
            },
        ),
    ];
    assert_eq!(ownership, expected_ownership, "ownership exato da cápsula");
    // C. Classificação: as duas regiões de suporte não possuem testes.
    assert!(ownership[..2]
        .iter()
        .all(|(_, owned)| owned.tests.is_empty()));
    assert!(ownership[2..]
        .iter()
        .all(|(_, owned)| !owned.tests.is_empty()));
    let owned_constants: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.constants.iter().map(String::as_str))
        .collect();
    let owned_helpers: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.helpers.iter().map(String::as_str))
        .collect();
    let owned_tests: Vec<_> = ownership
        .iter()
        .flat_map(|(_, owned)| owned.tests.iter().map(String::as_str))
        .collect();
    assert_eq!(owned_constants, ["DOC_TOML", "PORTAL", "CORE", "SRC"]);
    assert_eq!(
        owned_helpers,
        ["temp_repo", "write", "fixture", "doc", "nav", "code"]
    );
    assert_eq!(
        owned_tests,
        [
            "consulta_documental_le_catalogo_sem_revarrer_markdown",
            "consulta_de_codigo_le_catalogo_sem_revarrer_rust",
            "mostrar_detecta_ancora_divergente",
            "nav_mostrar_detecta_hash_divergente",
            "saida_json_e_valida_e_estavel",
            "limite_de_resultados_respeita_contornos",
            "catalogo_ausente_falha_com_codigo_3",
            "catalogo_invalido_falha_com_codigo_3",
            "ausencia_de_resultados_nao_e_sucesso_silencioso",
            "uso_invalido_sai_com_codigo_2",
        ]
    );
    assert_eq!(owned_constants.len(), 4);
    assert_eq!(owned_helpers.len(), 6);
    assert_eq!(owned_tests.len(), 10);
    for symbols in [&owned_constants, &owned_helpers, &owned_tests] {
        let unique: HashSet<_> = symbols.iter().copied().collect();
        assert_eq!(unique.len(), symbols.len(), "ownership duplicado");
    }
    // Nenhum símbolo do arquivo pode ficar órfão fora das oito regiões.
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("const ") || trimmed.starts_with("fn ")) {
            continue;
        }
        let number = index + 1;
        assert!(
            spans
                .iter()
                .any(|span| number > span.start && number < span.end),
            "símbolo órfão fora de região na linha {number}"
        );
    }

    // D. e H. Catálogo versionado: metadados exatos e totais 406/223/15.
    let catalog_path = repository.join("src/navigation.jsonl");
    let catalog = CodeCatalog::load(&catalog_path).expect("catálogo versionado");
    assert_eq!(catalog.regions.len(), 406);
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        223
    );
    assert_eq!(
        catalog
            .regions
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15
    );
    let mut all_keys: Vec<&str> = catalog.regions.iter().map(|r| r.key.as_str()).collect();
    all_keys.sort_unstable();
    let unique_keys: HashSet<&str> = all_keys.iter().copied().collect();
    assert_eq!(unique_keys.len(), all_keys.len(), "chaves duplicadas");

    let mut target_regions: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file == target_path)
        .collect();
    target_regions.sort_by_key(|region| region.start_marker);
    assert_eq!(target_regions.len(), 8);
    assert_eq!(
        target_regions
            .iter()
            .map(|region| region.key.as_str())
            .collect::<Vec<_>>(),
        expected_keys
    );
    for region in &target_regions {
        assert_eq!(region.domain.as_deref(), Some("trama"));
        assert_eq!(region.layer.as_deref(), Some("evidencia"));
        assert_eq!(region.file, target_path);
        assert_eq!(region.kind, "region");
        assert_eq!(region.status, "active");
    }
    // C. Classificação exata: duas de suporte e seis de evidência.
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| support_keys.contains(&region.key.as_str()))
            .count(),
        2
    );
    assert_eq!(
        target_regions
            .iter()
            .filter(|region| evidence_keys.contains(&region.key.as_str()))
            .count(),
        6
    );

    // I. Preservação integral das 398 regiões da base predecessora.
    let predecessor: Vec<_> = catalog
        .regions
        .iter()
        .filter(|region| region.file != target_path)
        .collect();
    assert_eq!(predecessor.len(), 398, "base predecessora exata");
    assert_eq!(
        predecessor
            .iter()
            .filter(|region| region.layer.as_deref() == Some("evidencia"))
            .count(),
        215
    );
    assert_eq!(
        predecessor
            .iter()
            .filter(|region| region.layer.as_deref() == Some("runtime"))
            .count(),
        15
    );
    let predecessor_projection = stable_region_projection(predecessor.iter().copied());
    assert_eq!(
        (
            predecessor_projection.len(),
            fnv1a64(predecessor_projection.as_bytes())
        ),
        (171_741, 7_038_069_266_194_794_117),
        "a projeção estável das 398 regiões predecessoras mudou"
    );
    // Preservação das 392 regiões pós-nav-catalog.
    let post_nav: Vec<_> = predecessor
        .iter()
        .copied()
        .filter(|region| region.file != "tests/doc_catalog_tests.rs")
        .collect();
    assert_eq!(post_nav.len(), 392, "conjunto pós-nav-catalog exato");
    let post_nav_projection = stable_region_projection(post_nav.iter().copied());
    assert_eq!(
        (
            post_nav_projection.len(),
            fnv1a64(post_nav_projection.as_bytes())
        ),
        (170_076, 12_143_728_175_883_859_804),
        "a projeção estável das 392 regiões pós-nav-catalog mudou"
    );
    // J. Preservação do conjunto histórico de 386 regiões da Onda 8.
    let historical: Vec<_> = post_nav
        .iter()
        .copied()
        .filter(|region| region.file != "tests/nav_catalog_tests.rs")
        .collect();
    assert_eq!(historical.len(), 386, "conjunto histórico exato");
    let historical_projection = stable_region_projection(historical.iter().copied());
    assert_eq!(
        (
            historical_projection.len(),
            fnv1a64(historical_projection.as_bytes())
        ),
        (168_339, 1_634_706_628_046_951_093),
        "a projeção estável das 386 regiões da Onda 8 mudou"
    );
    // K. Projeção completa desta cápsula, medida — não predita.
    let full_projection = stable_region_projection(catalog.regions.iter());
    assert_eq!(
        (full_projection.len(), fnv1a64(full_projection.as_bytes())),
        (174_073, 6_978_817_443_380_137_045),
        "a projeção estável das 406 regiões mudou"
    );

    // L. Igualdade com a regeneração canônica do CodeIndex.
    let regenerated = CodeIndex::scan_repo(&repository).expect("scan canônico");
    assert!(regenerated.verify().is_empty());
    assert_eq!(
        fs::read_to_string(catalog_path).expect("catálogo JSONL"),
        regenerated.render_jsonl(),
        "catálogo deve ser idêntico à regeneração canônica"
    );

    // M. Gates anteriores permanecem presentes.
    let gate_source = include_str!("nav_cartography_tests.rs");
    for gate in [
        "onda_8_convergencia_fecha_cadeia_8a_8j",
        "capsula_nav_catalog_cartografa_suporte_e_seis_testes",
        "capsula_doc_catalog_cartografa_suporte_e_quatro_testes",
    ] {
        let declaration = format!("fn {gate}() {{");
        assert_eq!(
            gate_source
                .lines()
                .filter(|line| line.trim() == declaration)
                .count(),
            1,
            "gate anterior ausente ou duplicado: {gate}"
        );
    }

    // N. e O. Contrato documental e não-alegações explícitas.
    let inventory = include_str!("../docs/development/code-navigation-inventory.md");
    let inventory_flat = inventory.split_whitespace().collect::<Vec<_>>().join(" ");
    for statement in [
        "terceira cápsula operacional/documental da Trama está completa",
        "tests/trama_query_tests.rs: 8 regiões",
        "2 suporte + 6 evidência",
        "4 constantes + 6 helpers + 10 testes",
        "catálogo atual = 406",
        "evidencia atual = 223",
        "runtime atual = 15",
        "onda_8_complete = true",
        "trama_complete = false",
        "ADDITIONAL_CAPSULES_REQUIRED",
        "demais suítes operacionais de `tests/`",
        "tests/trama_ci_tests.rs",
        "tests/trama_manifest_tests.rs",
        "tests/trama_projection_tests.rs",
        "tests/trama_scale_tests.rs",
        "tests/trama_sync_tests.rs",
        "tests/trama_template_tests.rs",
        "`apps/` permanece reservada à Onda 9",
        "repositórios sintéticos",
        "processos filhos",
        "asserções seletivas",
        "limpeza explícita, não RAII",
        "sobras após panic",
        "não prova a correção semântica completa da CLI",
        "não é validação exaustiva do catálogo",
        "não prova comportamento de processos independente de plataforma",
        "não conclui a Trama",
        "não ativa a Onda 9",
        "não oferece suporte a apps/",
    ] {
        assert!(
            inventory_flat.contains(statement),
            "contrato documental ausente: {statement}"
        );
    }
    assert!(
        !inventory_flat.contains("próximo alvo: apps/"),
        "`apps/` permanece reservada à Onda 9 e não pode ser o próximo alvo"
    );
    assert!(
        !inventory_flat.contains("trama_complete = true"),
        "a Trama permanece incompleta até um fechamento formal separado"
    );
}
