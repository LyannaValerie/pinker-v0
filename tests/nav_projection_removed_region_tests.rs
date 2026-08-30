//! Trama Pinker — reconstrução de região histórica removida do catálogo (#551).
//!
//! Congelar a história nunca implicou que toda região histórica ficasse eterna
//! no código corrente. Até o schema 3 o formato só sabia **tirar** região
//! posterior e **restaurar campo** de região presente; faltava representar a
//! remoção legítima, que a Issue #384 já pedia entre seus casos mínimos.
//!
//! Todas as fixtures são sintéticas. O acervo real de treze snapshots é coberto
//! por `nav_projection_authority_tests.rs` e não é tocado aqui: esta suíte prova
//! a **capacidade**, e nenhum snapshot congelado precisa usá-la para que ela
//! esteja correta.

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{parse_recipe, resolve, Library};
use pinker_v0::nav_projection_snapshot::{
    measure, parse, reconstruct, render, stable_projection, HarnessFailure, Measures, Outcome,
    ProjectionSnapshot, SnapshotState, SNAPSHOT_SCHEMA_V3, SNAPSHOT_SCHEMA_V4,
};

// ---------------------------------------------------------------------------
// Fixtures: o estado histórico tem `a`, `b` e `c`; o corrente perdeu `b`.
// ---------------------------------------------------------------------------

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
        summary: format!("Resumo de {key}."),
        hash: hash.to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

const HASH_A: &str = "fnv1a64:000000000000000a";
const HASH_B: &str = "fnv1a64:000000000000000b";
const HASH_C: &str = "fnv1a64:000000000000000c";

fn estado_historico() -> Vec<CodeRegion> {
    vec![
        region("a", "src/a.rs", HASH_A),
        region("b", "src/b.rs", HASH_B),
        region("c", "src/c.rs", HASH_C),
    ]
}

/// O catálogo corrente depois de `b` ser legitimamente removido do código.
fn catalogo_sem_b() -> Vec<CodeRegion> {
    vec![
        region("a", "src/a.rs", HASH_A),
        region("c", "src/c.rs", HASH_C),
    ]
}

fn medidas_historicas() -> Measures {
    measure(estado_historico().iter())
}

/// A regra que declara `b` por inteiro, exatamente com os campos que a projeção
/// estável lê.
const R_MATERIALIZA_B: &str = concat!(
    "op = \"materialize-region\"\n",
    "key = \"b\"\n",
    "kind = \"region\"\n",
    "domain = \"dominio\"\n",
    "layer = \"camada\"\n",
    "file = \"src/b.rs\"\n",
    "summary = \"Resumo de b.\"\n",
    "hash = \"fnv1a64:000000000000000b\"\n",
    "status = \"active\"\n",
);

fn snapshot_texto(id: &str, medidas: Measures, base: Option<&str>, regras: &str) -> String {
    let materializacoes = regras.matches("materialize-region").count();
    let orcamento = if materializacoes > 0 {
        format!("expected_materializations = {materializacoes}\n")
    } else {
        String::new()
    };
    let exclusoes = regras.matches("op = \"exclude").count();
    let overrides = regras.matches("op = \"override").count();
    let base_linha = base
        .map(|b| format!("base_snapshot = \"{b}\"\n"))
        .unwrap_or_default();
    format!(
        "schema = 4\nid = \"{id}\"\nstate = \"FROZEN\"\njustification = \"fixture sintetica\"\n\
         \n[reconstruction]\n{base_linha}expected_overrides = {overrides}\n\
         expected_exclusions = {exclusoes}\n{orcamento}\
         \n[measures]\nregions = {}\nlength = {}\nfnv1a64 = \"{}\"\n{regras}",
        medidas.regions,
        medidas.length,
        medidas.fnv1a64_canonical()
    )
}

fn regra(corpo: &str) -> String {
    format!("\n[[rules]]\n{corpo}")
}

fn snapshot_materializando_b() -> ProjectionSnapshot {
    parse(&snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &regra(R_MATERIALIZA_B),
    ))
    .expect("fixture canônica é válida")
}

// ---------------------------------------------------------------------------
// O caso central: a região removida volta, e a medida bate exatamente
// ---------------------------------------------------------------------------

#[test]
fn regiao_historica_ausente_do_catalogo_corrente_e_reconstruida() {
    let reconstrucao =
        reconstruct(&catalogo_sem_b(), &snapshot_materializando_b()).expect("reconstrução válida");

    let mut chaves: Vec<&str> = reconstrucao
        .regions
        .iter()
        .map(|r| r.key.as_str())
        .collect();
    chaves.sort_unstable();
    assert_eq!(chaves, vec!["a", "b", "c"]);

    // As três medidas congeladas, uma a uma, e a forma estável byte a byte.
    let medidas = reconstrucao.measures();
    assert_eq!(medidas.regions, medidas_historicas().regions);
    assert_eq!(medidas.length, medidas_historicas().length);
    assert_eq!(medidas.fnv1a64, medidas_historicas().fnv1a64);
    assert_eq!(
        stable_projection(reconstrucao.regions.iter()),
        stable_projection(estado_historico().iter()),
        "a projeção estável reconstruída difere do estado histórico"
    );
}

#[test]
fn a_verificacao_do_snapshot_congelado_da_match() {
    let relatorio =
        pinker_v0::nav_projection_snapshot::verify(&snapshot_materializando_b(), &catalogo_sem_b());
    assert_eq!(relatorio.outcome, Outcome::Match);
    assert_eq!(relatorio.state, SnapshotState::Frozen);
}

#[test]
fn a_materializacao_entra_no_livro_de_consumo() {
    let reconstrucao =
        reconstruct(&catalogo_sem_b(), &snapshot_materializando_b()).expect("reconstrução válida");
    let entradas: Vec<_> = reconstrucao
        .ledger
        .iter()
        .filter(|e| e.op == "materialize-region")
        .collect();
    assert_eq!(entradas.len(), 1, "a regra precisa aparecer no ledger");
    assert_eq!(entradas[0].selector, "b");
    assert_eq!(entradas[0].expected, 1);
    assert_eq!(entradas[0].consumed, 1);
}

#[test]
fn a_reconstrucao_nao_altera_o_catalogo_corrente() {
    // N10: materializar é fato da reconstrução, não do código de hoje.
    let corrente = catalogo_sem_b();
    let copia = corrente.clone();
    let reconstrucao =
        reconstruct(&corrente, &snapshot_materializando_b()).expect("reconstrução válida");
    assert_eq!(
        corrente, copia,
        "a reconstrução mutou o catálogo de entrada"
    );
    assert!(
        !corrente.iter().any(|r| r.key == "b"),
        "a região histórica vazou para o catálogo corrente"
    );
    assert_eq!(reconstrucao.regions.len(), 3);
}

// ---------------------------------------------------------------------------
// Cadeia base_snapshot: o bloqueio nunca foi de uma projeção só
// ---------------------------------------------------------------------------

fn biblioteca_da_cadeia() -> Library {
    // A base reconstrói o estado com `b`; o filho é o mesmo estado sem `a`.
    let sem_a: Vec<CodeRegion> = estado_historico()
        .into_iter()
        .filter(|r| r.key != "a")
        .collect();
    let base = parse(&snapshot_texto(
        "base",
        medidas_historicas(),
        None,
        &regra(R_MATERIALIZA_B),
    ))
    .expect("base válida");
    let filho = parse(&snapshot_texto(
        "filho",
        measure(sem_a.iter()),
        Some("base"),
        &regra("op = \"exclude-key\"\nkey = \"a\"\nexpected_matches = 1\n"),
    ))
    .expect("filho válido");
    Library::new()
        .with_snapshot(base)
        .unwrap()
        .with_snapshot(filho)
        .unwrap()
}

#[test]
fn a_cadeia_de_base_reconstroi_exatamente_depois_da_remocao() {
    let library = biblioteca_da_cadeia();
    let corrente = catalogo_sem_b();

    let base = resolve(&library, "base", &corrente).expect("base reconstrói");
    assert_eq!(base.measures(), medidas_historicas());

    let filho = resolve(&library, "filho", &corrente).expect("filho reconstrói");
    let sem_a: Vec<CodeRegion> = estado_historico()
        .into_iter()
        .filter(|r| r.key != "a")
        .collect();
    assert_eq!(filho.measures(), measure(sem_a.iter()));
    assert_eq!(
        stable_projection(filho.regions.iter()),
        stable_projection(sem_a.iter())
    );
    assert_eq!(filho.verified_bases, vec!["base".to_string()]);
}

#[test]
fn n8_cadeia_sem_a_regra_adequada_continua_falhando() {
    // Sem materialização, a base perde `b` e a cadeia inteira cai — que é
    // exatamente o estado anterior a esta Task.
    let base = parse(&snapshot_texto("base", medidas_historicas(), None, ""))
        .expect("base sem regras é válida como artefato");
    let filho = parse(&snapshot_texto(
        "filho",
        medidas_historicas(),
        Some("base"),
        "",
    ))
    .expect("filho válido");
    let library = Library::new()
        .with_snapshot(base)
        .unwrap()
        .with_snapshot(filho)
        .unwrap();
    match resolve(&library, "filho", &catalogo_sem_b()) {
        Err(HarnessFailure::BaseMeasuresDiverged { id, .. }) => assert_eq!(id, "base"),
        outro => panic!("esperada base divergente, veio {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// Negativas
// ---------------------------------------------------------------------------

#[test]
fn n1_materializar_chave_ja_presente_e_falha_de_harness() {
    // O catálogo corrente ainda tem `b`: a declaração histórica está errada, e
    // materializar nunca sobrescreve nem funde.
    match reconstruct(&estado_historico(), &snapshot_materializando_b()) {
        Err(HarnessFailure::MaterializationCollision { key }) => assert_eq!(key, "b"),
        outro => panic!("esperada colisão, veio {outro:?}"),
    }
}

#[test]
fn n2_duas_materializacoes_da_mesma_chave_sao_falha_mesmo_identicas() {
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!("{}{}", regra(R_MATERIALIZA_B), regra(R_MATERIALIZA_B)),
    );
    match parse(&texto) {
        Err(HarnessFailure::MaterializationRepeated { key }) => assert_eq!(key, "b"),
        outro => panic!("esperada materialização repetida, veio {outro:?}"),
    }
}

#[test]
fn n3_orcamento_de_materializacao_e_exato_nas_duas_direcoes() {
    // Uma regra `materialize-region` sempre consome, por construção: ou insere,
    // ou falha por colisão. O que pode divergir é o orçamento declarado, e ele é
    // conferido antes de qualquer reconstrução, nas duas direções.
    let base = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &regra(R_MATERIALIZA_B),
    );
    let ausente = base.replace(
        "expected_materializations = 1",
        "expected_materializations = 2",
    );
    match parse(&ausente) {
        Err(HarnessFailure::MaterializationMissing { declared, found }) => {
            assert_eq!((declared, found), (2, 1))
        }
        outro => panic!("esperada materialização ausente, veio {outro:?}"),
    }
    let excedente = base.replace("expected_materializations = 1\n", "");
    match parse(&excedente) {
        Err(HarnessFailure::MaterializationExcess { declared, found }) => {
            assert_eq!((declared, found), (0, 1))
        }
        outro => panic!("esperada materialização excedente, veio {outro:?}"),
    }
    // E a materialização não é contada como exclusão nem como override: os dois
    // orçamentos antigos continuam significando o que sempre significaram.
    let como_exclusao = base.replace("expected_exclusions = 0", "expected_exclusions = 1");
    assert!(matches!(
        parse(&como_exclusao),
        Err(HarnessFailure::ExclusionMissing { .. })
    ));
    let como_override = base.replace("expected_overrides = 0", "expected_overrides = 1");
    assert!(matches!(
        parse(&como_override),
        Err(HarnessFailure::OverrideMissing { .. })
    ));
}

#[test]
fn n4_hash_historico_malformado_e_falha() {
    for ruim in [
        "fnv1a64:000000000000000B",
        "fnv1a64:000000000000b",
        "000000000000000b",
        "",
    ] {
        let texto = snapshot_texto(
            "historico",
            medidas_historicas(),
            None,
            &regra(&R_MATERIALIZA_B.replace(
                "hash = \"fnv1a64:000000000000000b\"",
                &format!("hash = \"{ruim}\""),
            )),
        );
        assert!(
            matches!(parse(&texto), Err(HarnessFailure::HashInvalid { .. })),
            "hash {ruim:?} deveria ser recusado"
        );
    }
}

#[test]
fn n5_path_e_metadata_historicos_invalidos_sao_falha() {
    let com = |campo: &str, valor: &str| {
        let corpo = R_MATERIALIZA_B
            .lines()
            .map(|linha| {
                if linha.starts_with(&format!("{campo} = ")) {
                    format!("{campo} = \"{valor}\"")
                } else {
                    linha.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        snapshot_texto(
            "historico",
            medidas_historicas(),
            None,
            &regra(&format!("{corpo}\n")),
        )
    };
    assert!(matches!(
        parse(&com("file", "/absoluto/b.rs")),
        Err(HarnessFailure::PathAbsolute { .. })
    ));
    assert!(matches!(
        parse(&com("file", "src/../b.rs")),
        Err(HarnessFailure::PathTraversal { .. })
    ));
    assert!(matches!(
        parse(&com("file", "")),
        Err(HarnessFailure::InvalidField { .. })
    ));
    assert!(matches!(
        parse(&com("key", "")),
        Err(HarnessFailure::RuleWithoutSelector { .. })
    ));
    assert!(matches!(
        parse(&com("kind", "")),
        Err(HarnessFailure::InvalidField { .. })
    ));
    assert!(matches!(
        parse(&com("status", "")),
        Err(HarnessFailure::InvalidField { .. })
    ));
    // Campo obrigatório ausente também falha: o fato histórico é declarado por
    // inteiro, nunca por default implícito.
    for obrigatorio in ["kind", "file", "summary", "hash", "status"] {
        let prefixo = format!("{obrigatorio} = ");
        let mut corpo = String::new();
        for linha in R_MATERIALIZA_B.lines().filter(|l| !l.starts_with(&prefixo)) {
            corpo.push_str(linha);
            corpo.push('\n');
        }
        let texto = snapshot_texto("historico", medidas_historicas(), None, &regra(&corpo));
        assert!(
            matches!(parse(&texto), Err(HarnessFailure::MissingField { .. })),
            "campo {obrigatorio} ausente deveria falhar"
        );
    }
}

#[test]
fn n6_schema_antigo_nao_ganha_a_operacao_nova() {
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &regra(R_MATERIALIZA_B),
    )
    .replace("schema = 4", "schema = 3")
    // Sem a linha de orçamento, o gatilho que responde é o da própria operação:
    // os dois são capacidades do schema 4, e o do orçamento é conferido antes.
    .replace("expected_materializations = 1\n", "");
    match parse(&texto) {
        Err(HarnessFailure::CapabilityRequiresSchema {
            capability,
            found_schema,
            required_schema,
            ..
        }) => {
            assert!(capability.contains("materialize-region"), "{capability}");
            assert_eq!(found_schema, SNAPSHOT_SCHEMA_V3);
            assert_eq!(required_schema, SNAPSHOT_SCHEMA_V4);
        }
        outro => panic!("esperada capacidade acima do schema, veio {outro:?}"),
    }
}

#[test]
fn n6b_orcamento_novo_tambem_e_gatilhado_pelo_schema() {
    let texto = concat!(
        "schema = 3\nid = \"antigo\"\nstate = \"FROZEN\"\n\n[reconstruction]\n",
        "expected_overrides = 0\nexpected_exclusions = 0\nexpected_materializations = 0\n",
        "\n[measures]\nregions = 0\nlength = 0\nfnv1a64 = \"fnv1a64:0000000000000000\"\n"
    );
    match parse(texto) {
        Err(HarnessFailure::CapabilityRequiresSchema { capability, .. }) => assert!(
            capability.contains("expected_materializations"),
            "{capability}"
        ),
        outro => panic!("esperada capacidade acima do schema, veio {outro:?}"),
    }
}

#[test]
fn n7_medida_congelada_alterada_vira_drift_e_nao_passa_despercebida() {
    let mut snapshot = snapshot_materializando_b();
    snapshot.measures.regions += 1;
    let relatorio = pinker_v0::nav_projection_snapshot::verify(&snapshot, &catalogo_sem_b());
    assert!(
        matches!(relatorio.outcome, Outcome::Drift(_)),
        "medida adulterada precisa aparecer: {:?}",
        relatorio.outcome
    );
}

#[test]
fn n9_override_sobre_regiao_ausente_nao_vira_materializacao() {
    // O override continua exigindo correspondência: ele nunca inventa a região
    // que a materialização declara.
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &regra(
            "op = \"override-region\"\nkey = \"b\"\n\
             from_hash = \"fnv1a64:000000000000000b\"\n\
             to_hash = \"fnv1a64:00000000000000bb\"\n",
        ),
    );
    let snapshot = parse(&texto).expect("snapshot válido");
    match reconstruct(&catalogo_sem_b(), &snapshot) {
        Err(HarnessFailure::RegionRemoved { key }) => assert_eq!(key, "b"),
        outro => panic!("esperada região removida, veio {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// Ordem de aplicação: as sequências sem significado são impossíveis
// ---------------------------------------------------------------------------

#[test]
fn materializar_e_depois_excluir_e_estruturalmente_impossivel() {
    // As exclusões rodam antes. Uma exclusão cujo único alvo seria a região
    // materializada não encontra correspondência e falha; ela nunca chega a
    // apagar o que a materialização declarou.
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!(
            "{}{}",
            regra(R_MATERIALIZA_B),
            regra("op = \"exclude-key\"\nkey = \"b\"\nexpected_matches = 1\n")
        ),
    );
    let snapshot = parse(&texto).expect("snapshot válido");
    match reconstruct(&catalogo_sem_b(), &snapshot) {
        Err(HarnessFailure::ExclusionNoMatch { selector }) => assert_eq!(selector, "b"),
        outro => panic!("esperada exclusão sem correspondência, veio {outro:?}"),
    }
}

#[test]
fn override_de_regiao_que_so_existe_por_materializacao_e_impossivel() {
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!(
            "{}{}",
            regra(R_MATERIALIZA_B),
            regra(
                "op = \"override-region\"\nkey = \"b\"\n\
                 from_hash = \"fnv1a64:000000000000000b\"\n\
                 to_hash = \"fnv1a64:00000000000000bb\"\n"
            )
        ),
    );
    let snapshot = parse(&texto).expect("snapshot válido");
    match reconstruct(&catalogo_sem_b(), &snapshot) {
        Err(HarnessFailure::RegionRemoved { key }) => assert_eq!(key, "b"),
        outro => panic!("esperada região removida, veio {outro:?}"),
    }
}

#[test]
fn excluir_a_corrente_e_materializar_a_historica_com_a_mesma_chave_e_valido() {
    // A sequência legítima: a chave foi reaproveitada por outra região. A
    // corrente sai pela exclusão, a histórica entra pela materialização.
    let corrente = vec![
        region("a", "src/a.rs", HASH_A),
        region("b", "src/outro.rs", "fnv1a64:00000000000000ff"),
        region("c", "src/c.rs", HASH_C),
    ];
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!(
            "{}{}",
            regra("op = \"exclude-key\"\nkey = \"b\"\nexpected_matches = 1\n"),
            regra(R_MATERIALIZA_B)
        ),
    );
    let snapshot = parse(&texto).expect("snapshot válido");
    let reconstrucao = reconstruct(&corrente, &snapshot).expect("reconstrução válida");
    assert_eq!(reconstrucao.measures(), medidas_historicas());
    assert_eq!(
        stable_projection(reconstrucao.regions.iter()),
        stable_projection(estado_historico().iter())
    );
}

#[test]
fn a_ordem_e_do_modelo_e_nao_da_ordem_textual() {
    // Declarar a materialização antes da exclusão dá exatamente o mesmo
    // resultado que declará-la depois.
    let corrente = vec![
        region("a", "src/a.rs", HASH_A),
        region("b", "src/outro.rs", "fnv1a64:00000000000000ff"),
        region("c", "src/c.rs", HASH_C),
    ];
    let exclusao = regra("op = \"exclude-key\"\nkey = \"b\"\nexpected_matches = 1\n");
    let materializacao = regra(R_MATERIALIZA_B);
    let uma = parse(&snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!("{exclusao}{materializacao}"),
    ))
    .expect("válido");
    let outra = parse(&snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &format!("{materializacao}{exclusao}"),
    ))
    .expect("válido");
    assert_eq!(uma.rules, outra.rules, "a ordem canônica não normalizou");
    assert_eq!(
        stable_projection(reconstruct(&corrente, &uma).unwrap().regions.iter()),
        stable_projection(reconstruct(&corrente, &outra).unwrap().regions.iter())
    );
}

// ---------------------------------------------------------------------------
// Autoridade: materializar é do snapshot, não da receita
// ---------------------------------------------------------------------------

#[test]
fn receita_nao_pode_materializar_regiao_historica() {
    for schema in [1, 2] {
        let texto = format!(
            "schema = {schema}\nid = \"tentativa\"\n\n[reconstruction]\n\
             expected_overrides = 0\nexpected_exclusions = 0\n{}",
            regra(R_MATERIALIZA_B)
        );
        match parse_recipe(&texto) {
            Err(HarnessFailure::OperationOutsideAuthority { op, .. }) => {
                assert_eq!(op, "materialize-region")
            }
            outro => panic!("receita schema {schema} deveria recusar, veio {outro:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Parser e renderer
// ---------------------------------------------------------------------------

#[test]
fn parse_de_render_devolve_o_mesmo_modelo() {
    let modelo = snapshot_materializando_b();
    let texto = render(&modelo);
    assert_eq!(parse(&texto).expect("render é parseável"), modelo);
    assert_eq!(
        render(&parse(&texto).unwrap()),
        texto,
        "render não é ponto fixo"
    );
}

#[test]
fn o_renderer_emite_a_regra_em_ordem_canonica_de_campos() {
    let texto = render(&snapshot_materializando_b());
    let corpo = texto
        .split("[[rules]]\n")
        .nth(1)
        .expect("uma regra renderizada");
    let campos: Vec<&str> = corpo
        .lines()
        .filter(|l| l.contains(" = "))
        .map(|l| l.split(" = ").next().unwrap())
        .collect();
    assert_eq!(
        campos,
        vec!["op", "key", "kind", "domain", "layer", "file", "summary", "hash", "status"]
    );
    assert!(texto.contains("expected_materializations = 1\n"));
}

#[test]
fn campos_opcionais_ausentes_nao_sao_emitidos() {
    let mut corpo = String::new();
    for linha in R_MATERIALIZA_B
        .lines()
        .filter(|l| !l.starts_with("domain = ") && !l.starts_with("layer = "))
    {
        corpo.push_str(linha);
        corpo.push('\n');
    }
    let sem_metadata = vec![
        region("a", "src/a.rs", HASH_A),
        {
            let mut r = region("b", "src/b.rs", HASH_B);
            r.domain = None;
            r.layer = None;
            r
        },
        region("c", "src/c.rs", HASH_C),
    ];
    let texto = snapshot_texto(
        "historico",
        measure(sem_metadata.iter()),
        None,
        &regra(&corpo),
    );
    let modelo = parse(&texto).expect("opcionais podem faltar");
    let renderizado = render(&modelo);
    assert!(!renderizado.contains("domain = "));
    assert!(!renderizado.contains("layer = "));
    assert_eq!(parse(&renderizado).unwrap(), modelo);
    let reconstrucao = reconstruct(&catalogo_sem_b(), &modelo).expect("reconstrução válida");
    assert_eq!(reconstrucao.measures(), measure(sem_metadata.iter()));
}

#[test]
fn o_orcamento_zero_nao_e_emitido_e_o_artefato_antigo_continua_byte_estavel() {
    // É por isto que os treze snapshots congelados não precisam ser reescritos:
    // ausente significa zero, exatamente como `base_snapshot` e `recipes`.
    let texto_v3 = concat!(
        "schema = 3\n",
        "id = \"antigo\"\n",
        "state = \"FROZEN\"\n",
        "\n[reconstruction]\n",
        "expected_overrides = 0\n",
        "expected_exclusions = 0\n",
        "\n[measures]\n",
        "regions = 0\n",
        "length = 0\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
    );
    let modelo = parse(texto_v3).expect("schema 3 continua válido");
    assert_eq!(modelo.expected_materializations, 0);
    assert_eq!(
        render(&modelo),
        texto_v3,
        "o artefato antigo mudou de bytes"
    );
}

#[test]
fn campo_de_outra_operacao_nao_e_aceito_na_materializacao() {
    let texto = snapshot_texto(
        "historico",
        medidas_historicas(),
        None,
        &regra(&format!("{R_MATERIALIZA_B}expect_file = \"src/b.rs\"\n")),
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::FieldNotAllowedForOp { .. })
    ));
}

#[test]
fn a_materializacao_carrega_exatamente_os_campos_da_projecao_estavel() {
    // A guarda contra dois desvios simétricos: guardar campo que a medida não lê
    // (offset, símbolo, fase) e esquecer campo que ela lê.
    let texto = render(&snapshot_materializando_b());
    for ausente in [
        "start_marker",
        "content_start",
        "content_end",
        "end_marker",
        "phase",
        "symbols",
        "related_symbols",
        "test_for",
        "symbol_docs",
    ] {
        assert!(
            !texto.contains(ausente),
            "campo sem participação na projeção estável foi armazenado: {ausente}"
        );
    }
    let registro = stable_projection(estado_historico().iter());
    for presente in [
        "\"b\"",
        "\"region\"",
        "\"dominio\"",
        "\"camada\"",
        "src/b.rs",
    ] {
        assert!(registro.contains(presente), "{presente} sumiu da projeção");
    }
}
