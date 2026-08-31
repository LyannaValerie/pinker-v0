//! Autoridade canônica histórica da Issue #384: os snapshots e a receita
//! materializados em `.pinker/projections/`.
//!
//! Estas propriedades continuam válidas **depois** do cutover: elas descrevem a
//! autoridade nova, não a coexistência temporária com o mecanismo legado.

use pinker_v0::nav::{CodeCatalog, CodeRegion};
use pinker_v0::nav_projection_recipe::{self, Library, Recipe, RECIPES_DIR, RECIPE_SCHEMA};
use pinker_v0::nav_projection_snapshot::{
    self as snapshot, Outcome, ProjectionSnapshot, SnapshotState, SNAPSHOTS_DIR, SNAPSHOT_SCHEMA,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SNAPSHOTS_ESPERADOS: usize = 13;
const RECEITAS_ESPERADAS: usize = 1;
const RECEITA_NORMALIZACAO: &str = "normalizacao-corrente-para-historico";

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Arquivos `.toml` de um diretório, em ordem determinística e sem recursão.
fn arquivos_toml(dir: &Path) -> Vec<PathBuf> {
    let mut saida: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|erro| panic!("diretório {} ilegível: {erro}", dir.display()))
        .map(|entrada| entrada.expect("entrada de diretório").path())
        .filter(|caminho| caminho.is_file())
        .filter(|caminho| caminho.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    saida.sort();
    saida
}

fn carrega_snapshots() -> Vec<(PathBuf, ProjectionSnapshot)> {
    arquivos_toml(&raiz().join(SNAPSHOTS_DIR))
        .into_iter()
        .map(|caminho| {
            let texto = fs::read_to_string(&caminho).expect("snapshot legível");
            let modelo = snapshot::parse(&texto)
                .unwrap_or_else(|erro| panic!("{}: {erro:?}", caminho.display()));
            (caminho, modelo)
        })
        .collect()
}

fn carrega_receitas() -> Vec<(PathBuf, Recipe)> {
    arquivos_toml(&raiz().join(RECIPES_DIR))
        .into_iter()
        .map(|caminho| {
            let texto = fs::read_to_string(&caminho).expect("receita legível");
            let modelo = nav_projection_recipe::parse_recipe(&texto)
                .unwrap_or_else(|erro| panic!("{}: {erro:?}", caminho.display()));
            (caminho, modelo)
        })
        .collect()
}

fn biblioteca() -> Library {
    let mut library = Library::new();
    for (_, receita) in carrega_receitas() {
        library = library.with_recipe(receita).expect("receita única");
    }
    for (_, modelo) in carrega_snapshots() {
        library = library.with_snapshot(modelo).expect("snapshot único");
    }
    library
}

fn catalogo() -> Vec<CodeRegion> {
    CodeCatalog::load(&raiz().join("src/navigation.jsonl"))
        .expect("catálogo de código versionado")
        .regions
}

#[test]
fn biblioteca_reune_treze_snapshots_e_uma_receita() {
    let snapshots = carrega_snapshots();
    let receitas = carrega_receitas();
    assert_eq!(
        snapshots.len(),
        SNAPSHOTS_ESPERADOS,
        "a autoridade histórica tem exatamente {SNAPSHOTS_ESPERADOS} snapshots"
    );
    assert_eq!(
        receitas.len(),
        RECEITAS_ESPERADAS,
        "existe exatamente uma receita técnica de normalização"
    );
    assert_eq!(receitas[0].1.id, RECEITA_NORMALIZACAO);

    let library = biblioteca();
    assert_eq!(library.snapshot_ids().len(), SNAPSHOTS_ESPERADOS);
    assert_eq!(library.recipe_ids(), vec![RECEITA_NORMALIZACAO]);
}

#[test]
fn nome_do_arquivo_e_identificador_interno_coincidem() {
    for (caminho, modelo) in carrega_snapshots() {
        let stem = caminho.file_stem().expect("stem").to_str().expect("utf-8");
        assert_eq!(stem, modelo.id, "{}", caminho.display());
        assert_eq!(
            modelo.relative_path(),
            format!("{SNAPSHOTS_DIR}{}.toml", modelo.id)
        );
    }
    for (caminho, receita) in carrega_receitas() {
        let stem = caminho.file_stem().expect("stem").to_str().expect("utf-8");
        assert_eq!(stem, receita.id, "{}", caminho.display());
        assert_eq!(
            receita.relative_path(),
            format!("{RECIPES_DIR}{}.toml", receita.id)
        );
    }
}

#[test]
fn arquivos_reais_estao_na_forma_canonica_do_renderer() {
    for (caminho, modelo) in carrega_snapshots() {
        let bytes = fs::read_to_string(&caminho).expect("legível");
        assert_eq!(
            snapshot::render(&modelo),
            bytes,
            "{} não está na forma canônica do renderer",
            caminho.display()
        );
    }
    for (caminho, receita) in carrega_receitas() {
        let bytes = fs::read_to_string(&caminho).expect("legível");
        assert_eq!(
            nav_projection_recipe::render_recipe(&receita),
            bytes,
            "{} não está na forma canônica do renderer",
            caminho.display()
        );
    }
}

#[test]
fn nenhum_snapshot_frozen_depende_de_candidate() {
    let library = biblioteca();
    nav_projection_recipe::verify_frozen_dependencies(&library)
        .expect("nenhum FROZEN apoiado em CANDIDATE");
    for (_, modelo) in carrega_snapshots() {
        assert_eq!(
            modelo.state,
            SnapshotState::Frozen,
            "{} deveria estar congelado",
            modelo.id
        );
    }
}

#[test]
fn os_treze_snapshots_verificam_como_match_no_catalogo_atual() {
    let library = biblioteca();
    let base = catalogo();
    let mut verificados = 0;
    for (_, modelo) in carrega_snapshots() {
        let composicao = nav_projection_recipe::resolve(&library, &modelo.id, &base)
            .unwrap_or_else(|erro| panic!("{}: {erro:?}", modelo.id));
        assert_eq!(
            composicao.measures(),
            modelo.measures,
            "{}: medidas reconstruídas divergem das congeladas",
            modelo.id
        );
        verificados += 1;
    }
    assert_eq!(verificados, SNAPSHOTS_ESPERADOS);
}

#[test]
fn snapshot_sem_composicao_verifica_pelo_caminho_direto() {
    // O único snapshot cuja reconstrução parte do catálogo corrente não usa
    // base_snapshot; `verify` sozinho não resolve receitas, então o caminho
    // direto vale para os demais apenas via `resolve`. Aqui garantimos que
    // `verify` nunca reporta DRIFT silencioso para os que ele consegue avaliar.
    let base = catalogo();
    for (_, modelo) in carrega_snapshots() {
        if modelo.base_snapshot.is_some() || !modelo.recipes.is_empty() {
            continue;
        }
        let relatorio = snapshot::verify(&modelo, &base);
        assert!(
            matches!(relatorio.outcome, Outcome::Match),
            "{}: {:?}",
            modelo.id,
            relatorio.outcome
        );
    }
}

#[test]
fn a_receita_de_normalizacao_nao_e_reaplicada_na_composicao() {
    let library = biblioteca();
    let base = catalogo();
    let escopo = format!("recipe:{RECEITA_NORMALIZACAO}");
    for (_, modelo) in carrega_snapshots() {
        let composicao = nav_projection_recipe::resolve(&library, &modelo.id, &base)
            .unwrap_or_else(|erro| panic!("{}: {erro:?}", modelo.id));
        let aplicacoes = composicao
            .ledger
            .iter()
            .filter(|entrada| entrada.scope == escopo)
            .count();
        assert_eq!(
            aplicacoes, 1,
            "{}: a normalização deve ser aplicada exatamente uma vez na cadeia",
            modelo.id
        );
    }
}

#[test]
fn apenas_o_snapshot_terminal_consome_a_receita_diretamente() {
    let diretos: Vec<String> = carrega_snapshots()
        .into_iter()
        .filter(|(_, modelo)| modelo.recipes.contains(&RECEITA_NORMALIZACAO.to_string()))
        .map(|(_, modelo)| modelo.id)
        .collect();
    assert_eq!(
        diretos.len(),
        1,
        "só o snapshot que parte do catálogo corrente declara a receita: {diretos:?}"
    );
    let terminal = &diretos[0];
    let snapshots = carrega_snapshots();
    let modelo = snapshots
        .iter()
        .map(|(_, modelo)| modelo)
        .find(|modelo| &modelo.id == terminal)
        .expect("terminal presente");
    assert!(
        modelo.base_snapshot.is_none(),
        "o consumidor direto da normalização não se apoia em outro snapshot"
    );
}

#[test]
fn a_ordem_de_carregamento_nao_altera_o_resultado() {
    let base = catalogo();
    let mut direta = Library::new();
    for (_, receita) in carrega_receitas() {
        direta = direta.with_recipe(receita).expect("receita");
    }
    for (_, modelo) in carrega_snapshots() {
        direta = direta.with_snapshot(modelo).expect("snapshot");
    }

    let mut invertida = Library::new();
    for (_, receita) in carrega_receitas().into_iter().rev() {
        invertida = invertida.with_recipe(receita).expect("receita");
    }
    for (_, modelo) in carrega_snapshots().into_iter().rev() {
        invertida = invertida.with_snapshot(modelo).expect("snapshot");
    }

    for (_, modelo) in carrega_snapshots() {
        let a = nav_projection_recipe::resolve(&direta, &modelo.id, &base).expect("resolve direta");
        let b = nav_projection_recipe::resolve(&invertida, &modelo.id, &base)
            .expect("resolve invertida");
        assert_eq!(
            snapshot::stable_projection(a.regions.iter()),
            snapshot::stable_projection(b.regions.iter()),
            "{}: a ordem de carregamento mudou a reconstrução",
            modelo.id
        );
    }
}

#[test]
fn nenhum_arquivo_canonico_depende_de_root_absoluto() {
    let raiz_absoluta = raiz();
    let raiz_texto = raiz_absoluta.to_str().expect("root utf-8");
    let mut inspecionados = 0;
    for caminho in arquivos_toml(&raiz().join(SNAPSHOTS_DIR))
        .into_iter()
        .chain(arquivos_toml(&raiz().join(RECIPES_DIR)))
    {
        let texto = fs::read_to_string(&caminho).expect("legível");
        assert!(
            !texto.contains(raiz_texto),
            "{} cita o root absoluto",
            caminho.display()
        );
        for linha in texto.lines() {
            if let Some(valor) = linha.strip_prefix("expect_file = \"") {
                let arquivo = valor.trim_end_matches('"');
                assert!(
                    !arquivo.starts_with('/') && !arquivo.contains(".."),
                    "{}: expect_file não é repo-relativo: {arquivo}",
                    caminho.display()
                );
            }
        }
        inspecionados += 1;
    }
    assert_eq!(inspecionados, SNAPSHOTS_ESPERADOS + RECEITAS_ESPERADAS);
}

#[test]
fn identificadores_sao_estaveis_e_nao_derivam_de_medida() {
    let snapshots = carrega_snapshots();
    let ids: BTreeSet<&str> = snapshots.iter().map(|(_, m)| m.id.as_str()).collect();
    assert_eq!(ids.len(), SNAPSHOTS_ESPERADOS, "identificadores distintos");
    for (_, modelo) in &snapshots {
        for medida in [
            modelo.measures.regions.to_string(),
            modelo.measures.length.to_string(),
        ] {
            assert!(
                !modelo.id.contains(&medida),
                "{} carrega uma medida no identificador",
                modelo.id
            );
        }
        assert!(
            !modelo.id.contains("fnv"),
            "{} carrega o FNV no identificador",
            modelo.id
        );
    }
}

#[test]
fn predecessor_e_base_snapshot_sao_relacoes_distintas() {
    let snapshots = carrega_snapshots();
    let por_id: Vec<&ProjectionSnapshot> = snapshots.iter().map(|(_, m)| m).collect();
    let ids: BTreeSet<&str> = por_id.iter().map(|m| m.id.as_str()).collect();

    let mut com_predecessor = 0;
    let mut com_base = 0;
    let mut coincidem = 0;
    for modelo in &por_id {
        if let Some(anterior) = &modelo.predecessor {
            assert!(
                ids.contains(anterior.as_str()),
                "predecessor {anterior} ausente"
            );
            com_predecessor += 1;
        }
        if let Some(base) = &modelo.base_snapshot {
            assert!(ids.contains(base.as_str()), "base_snapshot {base} ausente");
            com_base += 1;
        }
        if modelo.predecessor.is_some() && modelo.predecessor == modelo.base_snapshot {
            coincidem += 1;
        }
    }
    assert!(com_predecessor > 0 && com_base > 0);
    assert_eq!(
        coincidem, 0,
        "nesta autoridade nenhuma das duas relações é sinônimo da outra"
    );
}

/// A versão em que este acervo foi congelado.
///
/// Não é `SNAPSHOT_SCHEMA`: a versão de **emissão** do formato avança quando o
/// formato ganha capacidade, e a #551 o levou ao 4 ao acrescentar
/// `materialize-region`. Os treze snapshots continuam exatamente como foram
/// congelados — reescrevê-los para acompanhar um bump seria mexer na história
/// para agradar a implementação.
const VERSAO_DO_ACERVO: u64 = 4;

/// A versão do acervo precisa continuar dentro do que o formato aceita. Falha em
/// tempo de compilação: um bump que abandonasse a versão congelada quebraria a
/// leitura dos treze artefatos, e isso não é assunto para descobrir em runtime.
const _: () = assert!(VERSAO_DO_ACERVO <= SNAPSHOT_SCHEMA);

/// Nenhuma versão aparece no diretório por acidente de conteúdo.
///
/// Sem esta guarda, um snapshot que por acaso não usa `override-region` poderia
/// ser emitido em `schema = 1` e continuar válido — e o acervo passaria a
/// misturar versões por acidente, não por decisão.
///
/// A guarda tem duas metades, porque há duas decisões distintas em jogo:
///
/// 1. **o acervo materializado é homogêneo** — os treze snapshots nomeados em
///    [`DISTRIBUICAO_CANONICA`] estão todos em [`VERSAO_DO_ACERVO`];
/// 2. **qualquer artefato do diretório está numa das duas versões legítimas** —
///    a do acervo congelado ou a de emissão corrente. Nada mais entra.
///
/// A segunda metade existe porque exigir a versão do acervo de *todo* arquivo
/// contradiz o emissor: `validate_candidate_shape` recusa candidato cujo schema
/// não seja `SNAPSHOT_SCHEMA`, então o próximo `pink nav projecao aceitar`
/// legítimo grava um schema 4 — e não haveria valor capaz de satisfazer as duas
/// pontas sem reescrever história congelada.
#[test]
fn o_acervo_usa_a_versao_corrente_de_cada_formato() {
    let do_acervo: BTreeSet<&str> = DISTRIBUICAO_CANONICA.iter().map(|(id, _)| *id).collect();
    let mut homogeneos = 0;
    for (caminho, modelo) in carrega_snapshots() {
        assert!(
            modelo.schema == VERSAO_DO_ACERVO || modelo.schema == SNAPSHOT_SCHEMA,
            "{} está em schema {}, que não é nem a versão do acervo nem a de emissão",
            caminho.display(),
            modelo.schema
        );
        if do_acervo.contains(modelo.id.as_str()) {
            assert_eq!(
                modelo.schema,
                VERSAO_DO_ACERVO,
                "{} destoa da versão única do acervo materializado",
                caminho.display()
            );
            homogeneos += 1;
        }
    }
    assert_eq!(
        homogeneos, SNAPSHOTS_ESPERADOS,
        "os treze snapshots do acervo precisam estar todos presentes"
    );
    for (caminho, receita) in carrega_receitas() {
        assert_eq!(
            receita.schema,
            RECIPE_SCHEMA,
            "{} não está na versão de emissão corrente do formato de receita",
            caminho.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Guarda de autoridade única das projeções históricas
// ---------------------------------------------------------------------------

/// Mapeamento provado dos casos comportamentais ainda exercitados pela
/// cartografia para os snapshots canônicos.
///
/// É uma tabela de **identidade** — caso → snapshot — e nada mais: não contém
/// `regions`, `length`, `fnv1a64` nem regra de reconstrução. Essas vivem
/// exclusivamente nos TOML de `.pinker/projections/`.
const DISTRIBUICAO_CANONICA: [(&str, usize); 13] = [
    ("capsula-doc-catalog", 4),
    ("capsula-nav-catalog", 5),
    ("capsula-trama-query", 3),
    ("onda-8-convergencia", 6),
    ("onda-8f-anterior", 1),
    ("onda-8g-anterior", 1),
    ("onda-8h-anterior", 1),
    ("onda-8i-anterior", 1),
    ("onda-8j-anterior", 1),
    ("onda-pink-agente-a", 2),
    ("onda-pink-agente-b", 3),
    ("onda-pink-agente-c", 2),
    ("onda-pink-agente-d", 1),
];

const CASOS_CARTOGRAFADOS_APOS_EXTRACAO: [(&str, usize); 9] = [
    ("capsula-doc-catalog", 2),
    ("capsula-nav-catalog", 3),
    ("capsula-trama-query", 1),
    ("onda-8-convergencia", 4),
    ("onda-8f-anterior", 1),
    ("onda-8g-anterior", 1),
    ("onda-8h-anterior", 1),
    ("onda-8i-anterior", 1),
    ("onda-8j-anterior", 1),
];
const CASOS_HISTORICOS_CARTOGRAFADOS_APOS_EXTRACAO: usize = 15;
const HARNESS: &str = include_str!("nav_cartography_tests.rs");

/// Identificadores citados por `verifica_snapshot_canonico("…")` no harness.
fn referencias_canonicas() -> Vec<&'static str> {
    let mut achados = Vec::new();
    let mut resto = HARNESS;
    while let Some(pos) = resto.find("verifica_snapshot_canonico(\"") {
        let depois = &resto[pos + "verifica_snapshot_canonico(\"".len()..];
        let fim = depois.find('"').expect("literal de identificador fechado");
        achados.push(&depois[..fim]);
        resto = &depois[fim..];
    }
    achados
}

#[test]
fn os_casos_historicos_referenciam_a_autoridade_canonica_por_id() {
    let refs = referencias_canonicas();
    assert_eq!(
        refs.len(),
        CASOS_HISTORICOS_CARTOGRAFADOS_APOS_EXTRACAO,
        "a cartografia deve referenciar exatamente {CASOS_HISTORICOS_CARTOGRAFADOS_APOS_EXTRACAO} casos \
         históricos; achados {}",
        refs.len()
    );
    let unicos: BTreeSet<&str> = refs.iter().copied().collect();
    assert_eq!(
        unicos.len(),
        CASOS_CARTOGRAFADOS_APOS_EXTRACAO.len(),
        "os casos devem cobrir os snapshots cartografados após a extração"
    );

    // Distribuição exata: impede tanto a perda de um caso quanto a repetição
    // artificial de outro para recompor a cardinalidade.
    for (id, esperado) in CASOS_CARTOGRAFADOS_APOS_EXTRACAO {
        let achado = refs.iter().filter(|candidato| **candidato == id).count();
        assert_eq!(
            achado, esperado,
            "{id}: esperados {esperado} casos históricos, achados {achado}"
        );
    }
    let declarado: usize = CASOS_CARTOGRAFADOS_APOS_EXTRACAO
        .iter()
        .map(|(_, n)| n)
        .sum();
    assert_eq!(declarado, CASOS_HISTORICOS_CARTOGRAFADOS_APOS_EXTRACAO);

    // Nenhum identificador inventado: todos resolvem na biblioteca real.
    let library = biblioteca();
    for id in &unicos {
        assert!(
            library.snapshot(id).is_some(),
            "identificador citado pela cartografia não existe na autoridade: {id}"
        );
    }
}

#[test]
fn a_cartografia_nao_mantem_segunda_autoridade_de_projecao() {
    for proibido in [
        "fn stable_region_projection",
        "stable_region_projection(",
        "fn fnv1a64",
    ] {
        assert!(
            !HARNESS.contains(proibido),
            "a cartografia voltou a calcular projeção/FNV histórico: {proibido}"
        );
    }
    // O harness estrutural residual não restaura campos históricos.
    for proibido in ["region.hash =", "region.summary ="] {
        assert!(
            !HARNESS.contains(proibido),
            "o harness estrutural voltou a restaurar campo histórico: {proibido}"
        );
    }
    // E não guarda medidas: nenhum literal de comprimento das 13 projeções.
    let snapshots = carrega_snapshots();
    assert_eq!(snapshots.len(), SNAPSHOTS_ESPERADOS);
    for (_, modelo) in snapshots {
        let comprimento = modelo.measures.length.to_string();
        assert!(
            !HARNESS.contains(&comprimento),
            "{}: o comprimento congelado reapareceu no harness",
            modelo.id
        );
        assert!(
            !HARNESS.contains(&modelo.measures.fnv1a64_canonical()),
            "{}: o FNV congelado reapareceu no harness",
            modelo.id
        );
    }
}
