//! Trama Pinker — relocação de região estável entre arquivos (#586).
//!
//! O acervo `FROZEN` é byte-imutável, e a organização física do repositório
//! continua evoluindo. Quando um arquivo cartografado muda de lugar, a região
//! continua a mesma — mesma chave estável, mesmo fato histórico — e apenas o
//! campo `file` da projeção estável diverge. Até o schema 4 esse campo só podia
//! ser **conferido** (`expect_file`) ou **afirmado por inteiro**
//! (`materialize-region`, que exige região ausente do catálogo corrente e não
//! existe na autoridade de receita). Não havia como restaurá-lo.
//!
//! Estes casos fixam a capacidade nova e, principalmente, o que ela **não** é:
//! `file` não vira identidade de região, a regra não aceita origem qualquer,
//! ausência de regra continua sendo drift e regra sobrando continua sendo falha
//! de orçamento.
//!
//! Nenhum caso aqui depende dos módulos de intrínsecas nem de qualquer domínio
//! particular: a fixture é sintética e neutra, porque a capacidade é geral.

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{
    parse_recipe, render_recipe, resolve, Library, RECIPE_SCHEMA_V2, RECIPE_SCHEMA_V3,
};
use pinker_v0::nav_projection_snapshot::{
    measure, parse, render, HarnessFailure, Measures, Outcome, ProjectionSnapshot, Rule,
    SchemaAuthority, SNAPSHOT_SCHEMA_V3, SNAPSHOT_SCHEMA_V5,
};

// ---------------------------------------------------------------------------
// Fixture sintética e neutra
// ---------------------------------------------------------------------------

const ARQUIVO_HISTORICO: &str = "src/antigo.rs";
const ARQUIVO_CORRENTE: &str = "src/familia/novo.rs";
const CHAVE: &str = "dominio.assunto.regiao";

fn region(key: &str, file: &str, hash: &str, summary: &str) -> CodeRegion {
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
        summary: summary.to_string(),
        hash: hash.to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

const HASH_HISTORICO: &str = "fnv1a64:00000000000000a1";
const HASH_CORRENTE: &str = "fnv1a64:00000000000000b2";
const RESUMO_HISTORICO: &str = "Resumo histórico da região.";
const RESUMO_CORRENTE: &str = "Resumo corrente da região.";

/// Estado histórico: a região mora em `src/antigo.rs`.
fn historico() -> Vec<CodeRegion> {
    vec![
        region(CHAVE, ARQUIVO_HISTORICO, HASH_HISTORICO, RESUMO_HISTORICO),
        region(
            "outro.assunto.regiao",
            "src/estavel.rs",
            "fnv1a64:00000000000000c3",
            "Região que não se move.",
        ),
    ]
}

/// Catálogo corrente: mesma chave estável, arquivo novo, conteúdo novo.
fn corrente() -> Vec<CodeRegion> {
    vec![
        region(CHAVE, ARQUIVO_CORRENTE, HASH_CORRENTE, RESUMO_CORRENTE),
        region(
            "outro.assunto.regiao",
            "src/estavel.rs",
            "fnv1a64:00000000000000c3",
            "Região que não se move.",
        ),
    ]
}

/// Catálogo corrente em que apenas o arquivo mudou.
fn corrente_so_arquivo() -> Vec<CodeRegion> {
    vec![
        region(CHAVE, ARQUIVO_CORRENTE, HASH_HISTORICO, RESUMO_HISTORICO),
        region(
            "outro.assunto.regiao",
            "src/estavel.rs",
            "fnv1a64:00000000000000c3",
            "Região que não se move.",
        ),
    ]
}

fn medidas_historicas() -> Measures {
    measure(historico().iter())
}

fn snapshot_congelado(m: Measures, overrides: u64) -> ProjectionSnapshot {
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
         expected_overrides = {overrides}\n\
         expected_exclusions = 0\n\
         recipes = [\"norm\"]\n",
        schema = SNAPSHOT_SCHEMA_V3,
        regions = m.regions,
        length = m.length,
        fnv = m.fnv1a64_canonical(),
        overrides = overrides,
    );
    parse(&texto).expect("snapshot congelado válido")
}

fn receita(regras: &str, overrides: u64) -> String {
    format!(
        "schema = {schema}\n\
         id = \"norm\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = {overrides}\n\
         expected_exclusions = 0\n\
         {regras}",
        schema = RECIPE_SCHEMA_V3,
        overrides = overrides,
        regras = regras,
    )
}

/// Regra de relocação pura: só restaura o caminho.
fn regra_relocacao(key: &str, de: &str, para: &str) -> String {
    format!(
        "\n[[rules]]\nop = \"override-region\"\nkey = \"{key}\"\nexpect_file = \"{de}\"\nto_file = \"{para}\"\n"
    )
}

/// Monta a biblioteca e reconstrói o snapshot congelado contra um catálogo.
fn reconstruir(
    texto_receita: &str,
    overrides_snapshot: u64,
    catalogo: &[CodeRegion],
) -> Result<Measures, HarnessFailure> {
    let recipe = parse_recipe(texto_receita)?;
    let snapshot = snapshot_congelado(medidas_historicas(), overrides_snapshot);
    let library = Library::new()
        .with_snapshot(snapshot)?
        .with_recipe(recipe)?;
    Ok(resolve(&library, "congelado", catalogo)?.measures())
}

// ---------------------------------------------------------------------------
// Positivo
// ---------------------------------------------------------------------------

#[test]
fn relocacao_correta_reproduz_a_medida_historica() {
    let observado = reconstruir(
        &receita(
            &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
            1,
        ),
        0,
        &corrente_so_arquivo(),
    )
    .expect("reconstrução válida");
    assert_eq!(
        observado,
        medidas_historicas(),
        "a relocação precisa reproduzir a medida congelada byte a byte"
    );
}

#[test]
fn a_regiao_relocada_e_verificada_como_match() {
    let recipe = parse_recipe(&receita(
        &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
        1,
    ))
    .expect("receita válida");
    let snapshot = snapshot_congelado(medidas_historicas(), 0);
    let library = Library::new()
        .with_snapshot(snapshot)
        .unwrap()
        .with_recipe(recipe)
        .unwrap();
    let verificacao = pinker_v0::nav_projection_recipe::verify_composed(
        &library,
        "congelado",
        &corrente_so_arquivo(),
    );
    assert_eq!(verificacao.outcome, Outcome::Match);
}

// ---------------------------------------------------------------------------
// Negativos
// ---------------------------------------------------------------------------

#[test]
fn chave_inexistente_falha_como_harness() {
    let erro = reconstruir(
        &receita(
            &regra_relocacao("chave.que.nao.existe", ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
            1,
        ),
        0,
        &corrente_so_arquivo(),
    )
    .expect_err("chave inexistente não pode ser tolerada");
    // A origem declarada identifica exatamente uma região sob outra chave, e o
    // diagnóstico diz isso em vez de alegar remoção: a regra recusa, e recusa
    // pelo motivo certo.
    match erro {
        HarnessFailure::KeyChanged { expected, found } => {
            assert_eq!(expected, "chave.que.nao.existe");
            assert_eq!(found, CHAVE);
        }
        outro => panic!("esperado chave alterada, veio {outro:?}"),
    }
}

#[test]
fn arquivo_corrente_errado_falha_como_harness() {
    let erro = reconstruir(
        &receita(
            &regra_relocacao(CHAVE, "src/lugar/errado.rs", ARQUIVO_HISTORICO),
            1,
        ),
        0,
        &corrente_so_arquivo(),
    )
    .expect_err("origem declarada errada não pode ser tolerada");
    match erro {
        HarnessFailure::PathChanged {
            key,
            expected,
            found,
        } => {
            assert_eq!(key, CHAVE);
            assert_eq!(expected, "src/lugar/errado.rs");
            assert_eq!(found, ARQUIVO_CORRENTE);
        }
        outro => panic!("esperado path divergente, veio {outro:?}"),
    }
}

#[test]
fn relocacao_ausente_e_drift_e_nao_falha_de_harness() {
    let observado = reconstruir(&receita("", 0), 0, &corrente_so_arquivo())
        .expect("sem regra a reconstrução é válida; o que muda é a medida");
    assert_ne!(
        observado,
        medidas_historicas(),
        "sem a regra de relocação a projeção congelada precisa divergir"
    );
}

#[test]
fn relocacao_sobrando_falha_por_orcamento_ou_por_seletor() {
    // Regra a mais, orçamento declarado corretamente: o seletor não encontra
    // nada e a falha é de harness.
    let regras = format!(
        "{}{}",
        regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
        regra_relocacao("chave.sobrando", "src/x.rs", "src/y.rs")
    );
    let erro = reconstruir(&receita(&regras, 2), 0, &corrente_so_arquivo())
        .expect_err("regra sobrando não pode ser silenciosamente ignorada");
    assert!(
        matches!(erro, HarnessFailure::RegionRemoved { .. }),
        "esperado seletor sem correspondência, veio {erro:?}"
    );

    // Regra a mais sem declarar orçamento: o formato recusa antes de aplicar
    // qualquer coisa.
    let erro = reconstruir(&receita(&regras, 1), 0, &corrente_so_arquivo())
        .expect_err("orçamento divergente não pode passar");
    match erro {
        HarnessFailure::OverrideExcess { declared, found } => {
            assert_eq!(declared, 1);
            assert_eq!(found, 2);
        }
        outro => panic!("esperado excesso de override, veio {outro:?}"),
    }
}

#[test]
fn relocacao_conta_no_orcamento_de_override() {
    // Uma relocação declarada e nenhuma orçada é override ausente. É isto que
    // torna a contabilidade explícita: a operação nova não tem orçamento
    // paralelo nem escapa do que já existe.
    let erro = reconstruir(
        &receita(
            &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
            0,
        ),
        0,
        &corrente_so_arquivo(),
    )
    .expect_err("relocação fora do orçamento não pode passar");
    match erro {
        HarnessFailure::OverrideExcess { declared, found } => {
            assert_eq!(declared, 0);
            assert_eq!(found, 1);
        }
        outro => panic!("esperado excesso de override, veio {outro:?}"),
    }
}

#[test]
fn to_file_sem_expect_file_e_meio_par() {
    let texto = receita(
        &format!(
            "\n[[rules]]\nop = \"override-region\"\nkey = \"{CHAVE}\"\nto_file = \"{ARQUIVO_HISTORICO}\"\n"
        ),
        1,
    );
    match parse_recipe(&texto) {
        Err(HarnessFailure::OverrideRegionPairInvalid { key, msg }) => {
            assert_eq!(key, CHAVE);
            assert!(
                msg.contains("expect_file"),
                "a mensagem precisa nomear a origem ausente: {msg}"
            );
        }
        outro => panic!("esperado meio par inválido, veio {outro:?}"),
    }
}

#[test]
fn to_file_exige_a_versao_que_o_declara() {
    // Receita: capacidade do schema 3.
    let texto = receita(
        &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
        1,
    )
    .replace(
        &format!("schema = {RECIPE_SCHEMA_V3}"),
        &format!("schema = {RECIPE_SCHEMA_V2}"),
    );
    match parse_recipe(&texto) {
        Err(HarnessFailure::CapabilityRequiresSchema {
            authority,
            found_schema,
            required_schema,
            ..
        }) => {
            assert_eq!(authority, SchemaAuthority::Recipe);
            assert_eq!(found_schema, RECIPE_SCHEMA_V2);
            assert_eq!(required_schema, RECIPE_SCHEMA_V3);
        }
        outro => panic!("esperado capacidade por versão, veio {outro:?}"),
    }

    // Snapshot: capacidade do schema 5.
    let snapshot = format!(
        "schema = {schema}\n\
         id = \"congelado\"\n\
         state = \"FROZEN\"\n\
         \n\
         [measures]\n\
         regions = 2\n\
         length = 10\n\
         fnv1a64 = \"fnv1a64:0000000000000001\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = 1\n\
         expected_exclusions = 0\n\
         {regra}",
        schema = SNAPSHOT_SCHEMA_V3,
        regra = regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
    );
    match parse(&snapshot) {
        Err(HarnessFailure::CapabilityRequiresSchema {
            authority,
            found_schema,
            required_schema,
            ..
        }) => {
            assert_eq!(authority, SchemaAuthority::Snapshot);
            assert_eq!(found_schema, SNAPSHOT_SCHEMA_V3);
            assert_eq!(required_schema, SNAPSHOT_SCHEMA_V5);
        }
        outro => panic!("esperado capacidade por versão, veio {outro:?}"),
    }
}

#[test]
fn override_region_sem_to_file_continua_no_schema_antigo() {
    // Backward compatibility: a versão mínima só sobe para quem usa o campo
    // novo. Uma receita schema 2 com `override-region` clássico continua válida.
    let texto = format!(
        "schema = {RECIPE_SCHEMA_V2}\n\
         id = \"norm\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = 1\n\
         expected_exclusions = 0\n\
         \n\
         [[rules]]\n\
         op = \"override-region\"\n\
         key = \"{CHAVE}\"\n\
         from_hash = \"{HASH_CORRENTE}\"\n\
         to_hash = \"{HASH_HISTORICO}\"\n"
    );
    let recipe = parse_recipe(&texto).expect("receita schema 2 continua válida");
    assert_eq!(recipe.schema, RECIPE_SCHEMA_V2);
}

// ---------------------------------------------------------------------------
// Composição determinística
// ---------------------------------------------------------------------------

#[test]
fn relocacao_compoe_com_hash_e_summary_na_mesma_regra() {
    let regra = format!(
        "\n[[rules]]\n\
         op = \"override-region\"\n\
         key = \"{CHAVE}\"\n\
         from_hash = \"{HASH_CORRENTE}\"\n\
         to_hash = \"{HASH_HISTORICO}\"\n\
         from_summary = \"{RESUMO_CORRENTE}\"\n\
         to_summary = \"{RESUMO_HISTORICO}\"\n\
         expect_file = \"{ARQUIVO_CORRENTE}\"\n\
         to_file = \"{ARQUIVO_HISTORICO}\"\n"
    );
    let observado = reconstruir(&receita(&regra, 1), 0, &corrente()).expect("reconstrução válida");
    assert_eq!(
        observado,
        medidas_historicas(),
        "os três campos precisam ser restaurados pela mesma regra atômica"
    );
}

#[test]
fn a_ordem_textual_das_regras_nao_muda_o_resultado() {
    let relocar = regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO);
    let outra = format!(
        "\n[[rules]]\n\
         op = \"override-region\"\n\
         key = \"outro.assunto.regiao\"\n\
         from_hash = \"fnv1a64:00000000000000c3\"\n\
         to_hash = \"fnv1a64:00000000000000c3\"\n"
    );
    let direta = reconstruir(
        &receita(&format!("{relocar}{outra}"), 2),
        0,
        &corrente_so_arquivo(),
    )
    .expect("reconstrução válida");
    let invertida = reconstruir(
        &receita(&format!("{outra}{relocar}"), 2),
        0,
        &corrente_so_arquivo(),
    )
    .expect("reconstrução válida");
    assert_eq!(direta, invertida);
    assert_eq!(direta, medidas_historicas());
}

#[test]
fn a_relocacao_e_atomica_com_os_demais_campos() {
    // `from_hash` errado invalida a regra inteira: o caminho não pode ficar
    // relocado enquanto o hash não foi restaurado.
    let regra = format!(
        "\n[[rules]]\n\
         op = \"override-region\"\n\
         key = \"{CHAVE}\"\n\
         from_hash = \"fnv1a64:00000000000000ff\"\n\
         to_hash = \"{HASH_HISTORICO}\"\n\
         expect_file = \"{ARQUIVO_CORRENTE}\"\n\
         to_file = \"{ARQUIVO_HISTORICO}\"\n"
    );
    let erro = reconstruir(&receita(&regra, 1), 0, &corrente())
        .expect_err("precondição falsa invalida a regra inteira");
    assert!(
        matches!(erro, HarnessFailure::OverrideStaleBase { .. }),
        "esperado base de override divergente, veio {erro:?}"
    );
}

// ---------------------------------------------------------------------------
// `file` não é identidade de região
// ---------------------------------------------------------------------------

#[test]
fn a_selecao_continua_sendo_exclusivamente_pela_chave_estavel() {
    // A regra encontra a região mesmo com o arquivo trocado: mover não obriga a
    // cunhar chave nova. Se a seleção usasse o par (chave, arquivo), a mesma
    // regra falharia por seletor sem correspondência.
    let observado = reconstruir(
        &receita(
            &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
            1,
        ),
        0,
        &corrente_so_arquivo(),
    )
    .expect("a chave estável basta para selecionar a região movida");
    assert_eq!(observado, medidas_historicas());
}

#[test]
fn duas_regioes_com_a_mesma_chave_em_arquivos_distintos_sao_ambiguas() {
    // O complemento do caso acima: a identidade é a chave, então dois arquivos
    // com a mesma chave não são duas regiões distinguíveis pelo caminho — são
    // ambiguidade, e a regra recusa em vez de escolher.
    let mut catalogo = corrente_so_arquivo();
    catalogo.push(region(
        CHAVE,
        "src/terceiro.rs",
        HASH_HISTORICO,
        RESUMO_HISTORICO,
    ));
    let erro = reconstruir(
        &receita(
            &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
            1,
        ),
        0,
        &catalogo,
    )
    .expect_err("chave repetida é ambiguidade");
    match erro {
        HarnessFailure::SelectorAmbiguous { key, matches } => {
            assert_eq!(key, CHAVE);
            assert_eq!(matches, 2);
        }
        outro => panic!("esperado seletor ambíguo, veio {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// Serialização
// ---------------------------------------------------------------------------

#[test]
fn to_file_sobrevive_ao_round_trip_das_duas_autoridades() {
    let texto = receita(
        &regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
        1,
    );
    let recipe = parse_recipe(&texto).expect("receita válida");
    let reparsed =
        parse_recipe(&render_recipe(&recipe)).expect("render de receita reinterpretável");
    assert_eq!(recipe, reparsed);
    assert!(matches!(
        reparsed.rules.first(),
        Some(Rule::OverrideRegion { to_file: Some(f), .. }) if f == ARQUIVO_HISTORICO
    ));

    let snapshot_texto = format!(
        "schema = {schema}\n\
         id = \"congelado\"\n\
         state = \"FROZEN\"\n\
         \n\
         [measures]\n\
         regions = 2\n\
         length = 10\n\
         fnv1a64 = \"fnv1a64:0000000000000001\"\n\
         \n\
         [reconstruction]\n\
         expected_overrides = 1\n\
         expected_exclusions = 0\n\
         {regra}",
        schema = SNAPSHOT_SCHEMA_V5,
        regra = regra_relocacao(CHAVE, ARQUIVO_CORRENTE, ARQUIVO_HISTORICO),
    );
    let snapshot = parse(&snapshot_texto).expect("snapshot válido");
    let reparsed = parse(&render(&snapshot)).expect("render de snapshot reinterpretável");
    assert_eq!(snapshot, reparsed);
}
