//! Trama Pinker — restauração da identidade histórica de uma região renomeada
//! (#685).
//!
//! `key`, `domain` e `layer` participam da projeção estável: são a identidade
//! pela qual uma medida congelada reconhece a região. Uma renomeação corrente
//! autorizada apaga essa identidade, e o acervo `FROZEN` é byte-imutável — não
//! há para onde mover o fato histórico. Até o schema 3 de receita esses três
//! campos só podiam ser **conferidos**; `materialize-region`, que afirma o fato
//! inteiro, exige região ausente e não existe na autoridade de receita.
//!
//! Estes casos fixam a capacidade nova e, principalmente, o que ela **não** é:
//! a relação nunca é inferida, a colisão nunca vira escolha, a guarda velha
//! nunca é ignorada, a regra nunca aplica metade e a autoridade de snapshot
//! continua sem a capacidade.
//!
//! A fixture é sintética e neutra: a capacidade é geral e não pertence a
//! domínio nenhum.

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{
    parse_recipe, render_recipe, resolve, Library, RECIPE_SCHEMA_V3, RECIPE_SCHEMA_V4,
};
use pinker_v0::nav_projection_snapshot::{
    apply_rules, measure, parse, validate_rules, HarnessFailure, Measures, Outcome,
    ProjectionRegion, ProjectionSnapshot, Rule, SchemaAuthority, SNAPSHOT_SCHEMA_V3,
};

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

const CHAVE_HISTORICA: &str = "dominio.assunto.regiao";
const CHAVE_CORRENTE: &str = "domain.subject.region";
const DOMINIO_HISTORICO: &str = "dominio";
const DOMINIO_CORRENTE: &str = "domain";
const CAMADA_HISTORICA: &str = "camada";
const CAMADA_CORRENTE: &str = "layer";
const VIZINHA: &str = "outro.assunto.regiao";
const ARQUIVO: &str = "src/estavel.rs";
const HASH: &str = "fnv1a64:00000000000000a1";
const RESUMO: &str = "Resumo estável da região.";

fn region(key: &str, domain: &str, layer: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some(domain.to_string()),
        layer: Some(layer.to_string()),
        phase: None,
        file: ARQUIVO.to_string(),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: RESUMO.to_string(),
        hash: HASH.to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

fn vizinha() -> CodeRegion {
    let mut outra = region(VIZINHA, "vizinho", "vizinha");
    outra.hash = "fnv1a64:00000000000000c3".to_string();
    outra
}

/// Estado histórico: a identidade que a medida congelada conhece.
fn historico() -> Vec<CodeRegion> {
    vec![
        region(CHAVE_HISTORICA, DOMINIO_HISTORICO, CAMADA_HISTORICA),
        vizinha(),
    ]
}

/// Catálogo corrente com a renomeação já aplicada nos três campos.
fn corrente() -> Vec<CodeRegion> {
    vec![
        region(CHAVE_CORRENTE, DOMINIO_CORRENTE, CAMADA_CORRENTE),
        vizinha(),
    ]
}

/// Catálogo corrente em que apenas um dos três campos mudou.
fn corrente_parcial(key: &str, domain: &str, layer: &str) -> Vec<CodeRegion> {
    vec![region(key, domain, layer), vizinha()]
}

fn medidas_historicas() -> Measures {
    measure(historico().iter())
}

fn snapshot_congelado() -> ProjectionSnapshot {
    let m = medidas_historicas();
    let texto = format!(
        "schema = {schema}\n\
         id = \"congelado\"\n\
         state = \"FROZEN\"\n\
         \n\
         [measures]\n\
         regions = {regions}\n\
         length = {length}\n\
         fnv1a64 = \"{fnv}\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = 0\n\
         expected_exclusions = 0\n\
         recipes = [\"norm\"]\n",
        schema = SNAPSHOT_SCHEMA_V3,
        regions = m.regions,
        length = m.length,
        fnv = m.fnv1a64_canonical(),
    );
    parse(&texto).expect("snapshot congelado válido")
}

fn receita(regras: &str, overrides: u64, schema: u64) -> String {
    format!(
        "schema = {schema}\n\
         id = \"norm\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = {overrides}\n\
         expected_exclusions = 0\n\
         {regras}"
    )
}

/// Regra de restauração de identidade, montada campo a campo.
fn regra_identidade(
    key: &str,
    to_key: Option<&str>,
    dominio: Option<(&str, &str)>,
    camada: Option<(&str, &str)>,
) -> String {
    let mut out = format!("\n[[rules]]\nop = \"override-region\"\nkey = \"{key}\"\n");
    if let Some(valor) = to_key {
        out.push_str(&format!("to_key = \"{valor}\"\n"));
    }
    if let Some((corrente, historico)) = dominio {
        out.push_str(&format!("expect_domain = \"{corrente}\"\n"));
        out.push_str(&format!("to_domain = \"{historico}\"\n"));
    }
    if let Some((corrente, historico)) = camada {
        out.push_str(&format!("expect_layer = \"{corrente}\"\n"));
        out.push_str(&format!("to_layer = \"{historico}\"\n"));
    }
    out
}

fn reconstruir(texto_receita: &str, catalogo: &[CodeRegion]) -> Result<Measures, HarnessFailure> {
    let recipe = parse_recipe(texto_receita)?;
    let library = Library::new()
        .with_snapshot(snapshot_congelado())?
        .with_recipe(recipe)?;
    Ok(resolve(&library, "congelado", catalogo)?.measures())
}

fn aplicar(regra: Rule, entrada: Vec<CodeRegion>) -> Result<Vec<ProjectionRegion>, HarnessFailure> {
    let projetadas: Vec<ProjectionRegion> = entrada.iter().map(ProjectionRegion::from).collect();
    apply_rules(projetadas, std::slice::from_ref(&regra)).map(|(regions, _)| regions)
}

fn regra_modelo(
    key: &str,
    to_key: Option<&str>,
    expect_domain: Option<&str>,
    to_domain: Option<&str>,
    expect_layer: Option<&str>,
    to_layer: Option<&str>,
) -> Rule {
    Rule::OverrideRegion {
        key: key.to_string(),
        to_key: to_key.map(str::to_string),
        from_hash: None,
        to_hash: None,
        from_summary: None,
        to_summary: None,
        expect_file: None,
        to_file: None,
        expect_domain: expect_domain.map(str::to_string),
        to_domain: to_domain.map(str::to_string),
        expect_layer: expect_layer.map(str::to_string),
        to_layer: to_layer.map(str::to_string),
    }
}

// ---------------------------------------------------------------------------
// C1..C4 — positivos
// ---------------------------------------------------------------------------

#[test]
fn c1_chave_renomeada_reconstroi_a_identidade_historica() {
    let observado = reconstruir(
        &receita(
            &regra_identidade(CHAVE_CORRENTE, Some(CHAVE_HISTORICA), None, None),
            1,
            RECIPE_SCHEMA_V4,
        ),
        &corrente_parcial(CHAVE_CORRENTE, DOMINIO_HISTORICO, CAMADA_HISTORICA),
    )
    .expect("reconstrução válida");
    assert_eq!(observado, medidas_historicas());
}

#[test]
fn c2_dominio_renomeado_reconstroi_a_identidade_historica() {
    let observado = reconstruir(
        &receita(
            &regra_identidade(
                CHAVE_HISTORICA,
                None,
                Some((DOMINIO_CORRENTE, DOMINIO_HISTORICO)),
                None,
            ),
            1,
            RECIPE_SCHEMA_V4,
        ),
        &corrente_parcial(CHAVE_HISTORICA, DOMINIO_CORRENTE, CAMADA_HISTORICA),
    )
    .expect("reconstrução válida");
    assert_eq!(observado, medidas_historicas());
}

#[test]
fn c3_camada_renomeada_reconstroi_a_identidade_historica() {
    let observado = reconstruir(
        &receita(
            &regra_identidade(
                CHAVE_HISTORICA,
                None,
                None,
                Some((CAMADA_CORRENTE, CAMADA_HISTORICA)),
            ),
            1,
            RECIPE_SCHEMA_V4,
        ),
        &corrente_parcial(CHAVE_HISTORICA, DOMINIO_HISTORICO, CAMADA_CORRENTE),
    )
    .expect("reconstrução válida");
    assert_eq!(observado, medidas_historicas());
}

#[test]
fn c4_os_tres_campos_sao_reconstruidos_por_uma_regra_atomica() {
    let recipe_text = receita(
        &regra_identidade(
            CHAVE_CORRENTE,
            Some(CHAVE_HISTORICA),
            Some((DOMINIO_CORRENTE, DOMINIO_HISTORICO)),
            Some((CAMADA_CORRENTE, CAMADA_HISTORICA)),
        ),
        1,
        RECIPE_SCHEMA_V4,
    );
    let observado = reconstruir(&recipe_text, &corrente()).expect("reconstrução válida");
    assert_eq!(observado, medidas_historicas());

    // Uma regra, não três: o orçamento de override não muda por campo.
    let recipe = parse_recipe(&recipe_text).expect("receita válida");
    assert_eq!(recipe.rules.len(), 1);
    assert_eq!(recipe.expected_overrides, 1);

    let library = Library::new()
        .with_snapshot(snapshot_congelado())
        .unwrap()
        .with_recipe(recipe)
        .unwrap();
    let verificacao =
        pinker_v0::nav_projection_recipe::verify_composed(&library, "congelado", &corrente());
    assert_eq!(verificacao.outcome, Outcome::Match);
}

#[test]
fn sem_a_regra_a_renomeacao_e_drift_e_nao_falha_de_harness() {
    let observado = reconstruir(&receita("", 0, RECIPE_SCHEMA_V4), &corrente())
        .expect("sem regra a reconstrução é válida; o que muda é a medida");
    assert_ne!(observado, medidas_historicas());
}

// ---------------------------------------------------------------------------
// C6..C8 — negativos de aplicação
// ---------------------------------------------------------------------------

#[test]
fn c6_guarda_velha_recusa_antes_de_qualquer_mutacao() {
    // A guarda de domínio não bate com a região corrente. A regra também
    // restauraria a chave; nada pode ter sido aplicado.
    let regra = regra_modelo(
        CHAVE_CORRENTE,
        Some(CHAVE_HISTORICA),
        Some("dominio-que-nao-e-o-corrente"),
        Some(DOMINIO_HISTORICO),
        None,
        None,
    );
    let erro = aplicar(regra, corrente()).expect_err("guarda divergente recusa");
    match erro {
        HarnessFailure::MetadataChanged {
            key, field, found, ..
        } => {
            assert_eq!(key, CHAVE_CORRENTE);
            assert_eq!(field, "domain");
            assert_eq!(found, DOMINIO_CORRENTE);
        }
        outro => panic!("esperado metadata divergente, veio {outro:?}"),
    }
}

#[test]
fn nenhum_campo_e_aplicado_quando_a_ultima_precondicao_falha() {
    // `to_key` e `to_domain` são válidos; a guarda de camada é que está velha.
    // Se a regra aplicasse em ordem, a chave e o domínio já teriam mudado.
    let regra = regra_modelo(
        CHAVE_CORRENTE,
        Some(CHAVE_HISTORICA),
        Some(DOMINIO_CORRENTE),
        Some(DOMINIO_HISTORICO),
        Some("camada-que-nao-e-a-corrente"),
        Some(CAMADA_HISTORICA),
    );
    let entrada = corrente();
    assert!(aplicar(regra, entrada.clone()).is_err());

    // A prova de que nada foi aplicado: a mesma entrada, com a guarda correta,
    // continua partindo da identidade corrente intacta.
    let boa = regra_modelo(
        CHAVE_CORRENTE,
        Some(CHAVE_HISTORICA),
        Some(DOMINIO_CORRENTE),
        Some(DOMINIO_HISTORICO),
        Some(CAMADA_CORRENTE),
        Some(CAMADA_HISTORICA),
    );
    let saida = aplicar(boa, entrada).expect("regra íntegra aplica");
    let alvo = saida
        .iter()
        .find(|region| region.key == CHAVE_HISTORICA)
        .expect("identidade histórica restaurada");
    assert_eq!(alvo.domain.as_deref(), Some(DOMINIO_HISTORICO));
    assert_eq!(alvo.layer.as_deref(), Some(CAMADA_HISTORICA));
}

#[test]
fn c7_identidade_historica_ocupada_recusa_em_vez_de_escolher() {
    let regra = regra_modelo(CHAVE_CORRENTE, Some(VIZINHA), None, None, None, None);
    let erro = aplicar(regra, corrente()).expect_err("colisão recusa");
    match erro {
        HarnessFailure::IdentityRestorationCollision { key, to_key } => {
            assert_eq!(key, CHAVE_CORRENTE);
            assert_eq!(to_key, VIZINHA);
        }
        outro => panic!("esperado colisão de identidade, veio {outro:?}"),
    }
}

#[test]
fn c8_seletor_ambiguo_recusa() {
    let mut catalogo = corrente();
    let mut gemea = region(CHAVE_CORRENTE, DOMINIO_CORRENTE, CAMADA_CORRENTE);
    gemea.file = "src/outro.rs".to_string();
    catalogo.push(gemea);
    let regra = regra_modelo(
        CHAVE_CORRENTE,
        Some(CHAVE_HISTORICA),
        None,
        None,
        None,
        None,
    );
    let erro = aplicar(regra, catalogo).expect_err("seletor ambíguo recusa");
    match erro {
        HarnessFailure::SelectorAmbiguous { key, matches } => {
            assert_eq!(key, CHAVE_CORRENTE);
            assert_eq!(matches, 2);
        }
        outro => panic!("esperado seletor ambíguo, veio {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// Formato: pares, ruído e ida e volta
// ---------------------------------------------------------------------------

#[test]
fn restaurar_metadata_sem_declarar_a_origem_e_recusado() {
    for (campo, regra) in [
        (
            "domain",
            format!(
                "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_CORRENTE}\"\nto_domain = \"{DOMINIO_HISTORICO}\"\n"
            ),
        ),
        (
            "layer",
            format!(
                "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_CORRENTE}\"\nto_layer = \"{CAMADA_HISTORICA}\"\n"
            ),
        ),
    ] {
        let erro = parse_recipe(&receita(&regra, 1, RECIPE_SCHEMA_V4))
            .expect_err("destino sem origem declarada é meio par");
        match erro {
            HarnessFailure::OverrideRegionPairInvalid { msg, .. } => {
                assert!(msg.contains(&format!("to_{campo}")), "{msg}");
                assert!(msg.contains(&format!("expect_{campo}")), "{msg}");
            }
            outro => panic!("esperado par inválido para {campo}, veio {outro:?}"),
        }
    }
}

#[test]
fn restaurar_o_valor_que_a_regiao_ja_tem_e_recusado() {
    for regra in [
        format!(
            "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_CORRENTE}\"\nto_key = \"{CHAVE_CORRENTE}\"\n"
        ),
        format!(
            "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_CORRENTE}\"\nexpect_domain = \"{DOMINIO_CORRENTE}\"\nto_domain = \"{DOMINIO_CORRENTE}\"\n"
        ),
        format!(
            "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_CORRENTE}\"\nexpect_layer = \"{CAMADA_CORRENTE}\"\nto_layer = \"{CAMADA_CORRENTE}\"\n"
        ),
    ] {
        let erro = parse_recipe(&receita(&regra, 1, RECIPE_SCHEMA_V4))
            .expect_err("restauração sem efeito é recusada");
        assert!(
            matches!(erro, HarnessFailure::OverrideRegionPairInvalid { .. }),
            "veio {erro:?}"
        );
    }
}

#[test]
fn a_forma_canonica_sobrevive_a_ida_e_volta() {
    let texto = receita(
        &regra_identidade(
            CHAVE_CORRENTE,
            Some(CHAVE_HISTORICA),
            Some((DOMINIO_CORRENTE, DOMINIO_HISTORICO)),
            Some((CAMADA_CORRENTE, CAMADA_HISTORICA)),
        ),
        1,
        RECIPE_SCHEMA_V4,
    );
    let primeira = parse_recipe(&texto).expect("receita válida");
    let rendida = render_recipe(&primeira);
    let segunda = parse_recipe(&rendida).expect("forma canônica reparseável");
    assert_eq!(primeira, segunda);
    assert_eq!(rendida, render_recipe(&segunda));
    assert!(rendida.contains("to_key = "));
    assert!(rendida.contains("to_domain = "));
    assert!(rendida.contains("to_layer = "));
}

// ---------------------------------------------------------------------------
// C14, C15 — autoridade e compatibilidade de schema
// ---------------------------------------------------------------------------

#[test]
fn c14_receita_em_schema_antigo_nao_ganha_a_capacidade_nova() {
    let erro = parse_recipe(&receita(
        &regra_identidade(CHAVE_CORRENTE, Some(CHAVE_HISTORICA), None, None),
        1,
        RECIPE_SCHEMA_V3,
    ))
    .expect_err("schema 3 não conhece restauração de identidade");
    match erro {
        HarnessFailure::CapabilityRequiresSchema {
            authority,
            found_schema,
            required_schema,
            ..
        } => {
            assert_eq!(authority, SchemaAuthority::Recipe);
            assert_eq!(found_schema, RECIPE_SCHEMA_V3);
            assert_eq!(required_schema, RECIPE_SCHEMA_V4);
        }
        outro => panic!("esperado capacidade acima do schema, veio {outro:?}"),
    }
}

#[test]
fn c14_receita_em_schema_antigo_preserva_a_semantica_antiga() {
    // O mesmo texto sem a capacidade nova continua válido em schema 3 e produz
    // exatamente a mesma reconstrução de antes.
    let regra = format!(
        "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE_HISTORICA}\"\nexpect_file = \"{ARQUIVO}\"\nto_file = \"src/historico.rs\"\n"
    );
    let recipe = parse_recipe(&receita(&regra, 1, RECIPE_SCHEMA_V3)).expect("schema 3 íntegro");
    assert_eq!(recipe.schema, RECIPE_SCHEMA_V3);
    assert_eq!(recipe.rules.len(), 1);
    assert!(!recipe.rules[0].restores_historical_identity());
}

#[test]
fn c15_materialize_region_continua_fora_da_receita() {
    let regra = format!(
        "\n[[rules]]\nop = \"materialize-region\"\nkey = \"{CHAVE_HISTORICA}\"\nkind = \"region\"\nfile = \"{ARQUIVO}\"\nsummary = \"x\"\nhash = \"{HASH}\"\nstatus = \"active\"\n"
    );
    let erro = parse_recipe(&receita(&regra, 0, RECIPE_SCHEMA_V4))
        .expect_err("materializar não pertence à receita");
    assert!(
        matches!(
            erro,
            HarnessFailure::OperationOutsideAuthority {
                authority: SchemaAuthority::Recipe,
                ..
            }
        ),
        "veio {erro:?}"
    );
}

#[test]
fn restaurar_identidade_continua_fora_do_snapshot() {
    let regra = regra_modelo(
        CHAVE_CORRENTE,
        Some(CHAVE_HISTORICA),
        None,
        None,
        None,
        None,
    );
    let erro = validate_rules(
        pinker_v0::nav_projection_snapshot::SNAPSHOT_SCHEMA,
        std::slice::from_ref(&regra),
        SchemaAuthority::Snapshot,
    )
    .expect_err("snapshot não normaliza corrente para histórico");
    match erro {
        HarnessFailure::CapabilityOutsideAuthority { authority, .. } => {
            assert_eq!(authority, SchemaAuthority::Snapshot);
        }
        outro => panic!("esperado capacidade fora da autoridade, veio {outro:?}"),
    }

    // E a mesma regra, na autoridade de receita e no schema certo, é aceita: a
    // recusa é de autoridade, não de forma.
    validate_rules(
        RECIPE_SCHEMA_V4,
        std::slice::from_ref(&regra),
        SchemaAuthority::Recipe,
    )
    .expect("a receita possui a capacidade");
}
