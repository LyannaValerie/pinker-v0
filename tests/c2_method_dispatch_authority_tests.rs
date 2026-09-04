//! #442/C2 — a seleção de método de impl é decidida uma vez só.
//!
//! Antes da consolidação, `semantic::resolve_impl_method` e
//! `ir::resolve_impl_method` aplicavam, cada um sobre o seu índice, a mesma
//! regra: filtrar por alcance, ficar com o nível mais forte e classificar o
//! que sobrou. Nada construía essa concordância — ela era coincidência.
//!
//! Estes testes provam que a regra passou a ter dona única
//! (`method_dispatch`), que as duas fases a consomem, e que o comportamento
//! observável de despacho não mudou.

mod common;

use common::rust_source::codigo_executavel;
use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::method_dispatch::{
    select_impl_method, select_representative, DispatchCandidate, DispatchRelation,
    MethodSelection, RepresentativeSelection,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// A autoridade, exercitada diretamente
// ---------------------------------------------------------------------------

fn candidato(function_name: &str, trait_name: &str) -> DispatchCandidate {
    DispatchCandidate {
        function_name: function_name.to_string(),
        relation: Some(DispatchRelation {
            trait_name: trait_name.to_string(),
            fonte_da_relacao: None,
        }),
    }
}

/// Sem índice de composição não há nada a restringir: todo candidato alcança.
fn sem_composicao(
) -> HashMap<pinker_v0::source_map::SourceId, pinker_v0::module_resolve::TratosNoDespacho> {
    HashMap::new()
}

fn span() -> pinker_v0::token::Span {
    pinker_v0::token::Span::new(
        pinker_v0::token::Position::new(1, 1),
        pinker_v0::token::Position::new(1, 2),
    )
}

#[test]
fn um_candidato_alcancado_vence() {
    assert_eq!(
        select_impl_method(&sem_composicao(), span(), [candidato("__impl_a", "Marca")]),
        MethodSelection::Winner("__impl_a".to_string())
    );
}

#[test]
fn nenhum_candidato_e_no_match() {
    assert_eq!(
        select_impl_method(&sem_composicao(), span(), []),
        MethodSelection::NoMatch
    );
}

#[test]
fn dois_candidatos_no_mesmo_nivel_sao_ambiguos() {
    assert_eq!(
        select_impl_method(
            &sem_composicao(),
            span(),
            [
                candidato("__impl_a", "Marca"),
                candidato("__impl_b", "Outra")
            ]
        ),
        MethodSelection::Ambiguous
    );
}

/// Candidato sem relação conhecida entra no nível próprio, como sempre entrou.
#[test]
fn candidato_sem_relacao_participa_no_nivel_proprio() {
    let sem_relacao = DispatchCandidate {
        function_name: "__impl_sem_relacao".to_string(),
        relation: None,
    };
    assert_eq!(
        select_impl_method(&sem_composicao(), span(), [sem_relacao]),
        MethodSelection::Winner("__impl_sem_relacao".to_string())
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Materializado {
    simbolo: String,
    gerado: bool,
}

fn materializado(simbolo: &str, gerado: bool) -> Materializado {
    Materializado {
        simbolo: simbolo.to_string(),
        gerado,
    }
}

fn representante(
    mut candidatos: Vec<Materializado>,
) -> (RepresentativeSelection, Vec<Materializado>) {
    let selecao = select_representative(
        &mut candidatos,
        |candidato| candidato.simbolo.as_str(),
        |candidato| candidato.gerado,
    );
    (selecao, candidatos)
}

#[test]
fn o_explicito_vence_o_default_materializado() {
    let (selecao, ordenados) = representante(vec![
        materializado("__impl_z_explicito", false),
        materializado("__impl_a_default", true),
    ]);
    let RepresentativeSelection::Selected(index) = selecao else {
        panic!("explícito único não é conflito");
    };
    assert_eq!(ordenados[index].simbolo, "__impl_z_explicito");
}

/// Sem explícito, desempata a ordem total do símbolo — nunca a ordem de fonte.
#[test]
fn sem_explicito_desempata_a_ordem_total_do_simbolo() {
    let (selecao, ordenados) = representante(vec![
        materializado("__impl_z_default", true),
        materializado("__impl_a_default", true),
    ]);
    let RepresentativeSelection::Selected(index) = selecao else {
        panic!("nenhum explícito não é conflito");
    };
    assert_eq!(ordenados[index].simbolo, "__impl_a_default");
}

#[test]
fn dois_explicitos_sao_conflito_em_ordem_canonica() {
    let (selecao, ordenados) = representante(vec![
        materializado("__impl_z_explicito", false),
        materializado("__impl_a_explicito", false),
    ]);
    let RepresentativeSelection::ExplicitConflict {
        previous,
        conflicting,
    } = selecao
    else {
        panic!("dois explícitos são conflito");
    };
    assert_eq!(ordenados[previous].simbolo, "__impl_a_explicito");
    assert_eq!(ordenados[conflicting].simbolo, "__impl_z_explicito");
}

// ---------------------------------------------------------------------------
// Fechamento: nenhuma fase mantém regra própria de seleção
// ---------------------------------------------------------------------------

fn fonte(caminho: &str) -> String {
    fs::read_to_string(format!("{}/{caminho}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|erro| panic!("ler {caminho}: {erro}"))
}

/// As duas fases importam a autoridade sem apelido e a consultam exatamente uma
/// vez por decisão.
///
/// A proibição do apelido é o que torna a contagem por nome suficiente: sem
/// `as`, toda chamada precisa soletrar `select_impl_method` ou
/// `select_representative`, então uma segunda chamada viva aparece na contagem.
/// A varredura olha TODOS os `use` que nomeiam a autoridade, não só o primeiro,
/// e normaliza espaço em branco — comentário interposto num caminho é código
/// válido e não pode fazer o oráculo mentir em nenhuma das duas direções. A
/// contagem é de ocorrências em forma de chamada (`nome(`), então um
/// identificador vizinho que apenas contenha o nome não acusa.
///
/// Teto declarado: uma fase pode ligar a autoridade a um `let`/closure e chamar
/// a ligação várias vezes; a contagem continua em um. Isso NÃO é segunda
/// autoridade — é a mesma autoridade chamada de dois lugares —, e a regra que
/// importa continua guardada pela proibição do vocabulário de precedência e
/// pelos casos comportamentais. Regra nova escondida atrás de macro também
/// escapa a qualquer oráculo textual.
#[test]
fn as_duas_fases_consomem_a_autoridade_unica() {
    for caminho in ["src/semantic.rs", "src/ir.rs"] {
        let codigo = codigo_executavel(&fonte(caminho));
        let importacoes: Vec<String> = codigo
            .split(';')
            .map(|trecho| trecho.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|trecho| trecho.contains("use ") && trecho.contains("method_dispatch"))
            .collect();
        assert!(
            !importacoes.is_empty(),
            "{caminho} deixou de importar a autoridade"
        );
        for importacao in &importacoes {
            assert!(
                !importacao.split(' ').any(|palavra| palavra == "as"),
                "{caminho} importa a autoridade sob apelido: `{importacao}`"
            );
        }
        for decisao in ["select_impl_method", "select_representative"] {
            assert_eq!(
                codigo.matches(&format!("{decisao}(")).count(),
                1,
                "{caminho} deveria consultar `{decisao}` exatamente uma vez"
            );
        }
    }
}

/// Só as duas fases consultam a autoridade. Uma terceira camada que passe a
/// escolher vencedor aparece aqui mesmo sem tocar em `semantic` ou `ir`.
#[test]
fn ninguem_mais_consulta_a_autoridade_de_selecao() {
    let mut consultam = varredura_de_src(|codigo| {
        codigo.contains("select_impl_method") || codigo.contains("select_representative")
    });
    consultam.sort();
    assert_eq!(
        consultam,
        vec![
            "ir.rs".to_string(),
            "method_dispatch.rs".to_string(),
            "semantic.rs".to_string()
        ],
        "a autoridade de seleção passou a ser consultada por outra camada"
    );
}

/// `NivelDeDespacho` é o vocabulário da precedência e `nivel_de_despacho` é a
/// pergunta de alcance. Uma fase que volte a nomear qualquer um dos dois — por
/// caminho completo, por importação ou por apelido de importação, que ainda
/// precisa nomear o item — pode voltar a decidir sozinha qual candidato vence.
#[test]
fn nenhuma_fase_reintroduz_o_vocabulario_da_precedencia() {
    for caminho in ["src/semantic.rs", "src/ir.rs"] {
        let codigo = codigo_executavel(&fonte(caminho));
        // `TratosNoDespacho` fica de fora: a fase TRANSPORTA o índice para a
        // autoridade. Proibido é aplicar o nível, não segurar o índice.
        for termo in [
            "NivelDeDespacho",
            "nivel_de_despacho",
            "PorUnidadeImportada",
        ] {
            assert!(
                !codigo.contains(termo),
                "{caminho} voltou a aplicar `{termo}` por conta própria"
            );
        }
    }
}

/// Aplica `predicado` ao código executável de cada `.rs` de `src/`.
fn varredura_de_src(predicado: impl Fn(&str) -> bool) -> Vec<String> {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut encontrados = Vec::new();
    let mut pendentes = vec![raiz.clone()];
    while let Some(dir) = pendentes.pop() {
        for entrada in fs::read_dir(&dir).expect("diretório de fontes legível") {
            let caminho = entrada.expect("entrada legível").path();
            if caminho.is_dir() {
                pendentes.push(caminho);
                continue;
            }
            if !caminho.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let codigo = codigo_executavel(&fs::read_to_string(&caminho).expect("fonte legível"));
            if predicado(&codigo) {
                encontrados.push(
                    caminho
                        .strip_prefix(&raiz)
                        .expect("fonte dentro de src/")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    encontrados
}

/// Alcance continua sendo pergunta de `module_resolve`; quem a faz por conta do
/// despacho é só a autoridade de seleção.
#[test]
fn so_a_autoridade_de_selecao_consulta_o_alcance() {
    let mut consultam = varredura_de_src(|codigo| {
        codigo.contains("nivel_de_despacho") || codigo.contains("NivelDeDespacho")
    });
    consultam.sort();
    assert_eq!(
        consultam,
        vec![
            "method_dispatch.rs".to_string(),
            "module_resolve.rs".to_string()
        ],
        "o alcance passou a ser consultado fora da autoridade de seleção"
    );
}

// ---------------------------------------------------------------------------
// Comportamento observável: as duas metades continuam decidindo o mesmo
// ---------------------------------------------------------------------------

struct Caso {
    _dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(modulos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso C2");
    let mut raiz = None;
    for (nome, fonte) in modulos {
        let caminho = escrever(dir.path(), nome, fonte);
        if *nome == "main" {
            raiz = Some(caminho);
        }
    }
    Caso {
        raiz: raiz.expect("caso tem raiz `main`"),
        _dir: dir,
    }
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte)
        .unwrap_or_else(|erro| panic!("gravar {}: {erro}", caminho.display()));
    caminho
}

fn pink(caso_logico: &str, args: &[&str], alvo: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg(alvo)
        .logical_case(caso_logico)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar pink")
}

fn codigo(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

fn stdout(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stdout).into_owned()
}

const TRATO_MARCA: &str = "pacote mc2t;\n\n\
    trato Marca {\n    \
        carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n\
    }\n";

/// GOLDEN #579 — congela o comportamento CORRENTE, não o endossa.
///
/// ```text
/// THIS TEST PRESERVES CURRENT BEHAVIOR
/// IT DOES NOT ENDORSE POLICY_A OR POLICY_B
/// ```
///
/// A raiz nomeia o trato por import próprio e despacha por uma relação
/// declarada por `mc2impl`, unidade que ela nunca importou diretamente: a
/// relação só chega ao programa projetado por uma cadeia indireta. A #579
/// permanece aberta para decidir se esse alcance deve continuar existindo;
/// C2 apenas garante que a resposta, qualquer que venha a ser, terá uma só
/// consequência executiva.
#[test]
fn golden_579_o_alcance_corrente_do_trato_proprio_permanece_identico() {
    let caso = caso(&[
        ("mc2t", TRATO_MARCA),
        (
            "mc2impl",
            "pacote mc2impl;\ntrazer mc2t.Marca;\n\n\
             impl Marca para bombom {\n    \
                 carinho padrao(valor: bombom) -> bombom { mimo valor + 7; }\n\
             }\n\n\
             carinho fundo() -> bombom { mimo 1; }\n",
        ),
        (
            "mc2mid",
            "pacote mc2mid;\ntrazer mc2impl.fundo;\n\n\
             carinho meio() -> bombom { mimo fundo(); }\n",
        ),
        (
            "main",
            "pacote main;\ntrazer mc2t.Marca;\ntrazer mc2mid.meio;\n\n\
             carinho principal() -> bombom {\n    \
                 nova x: bombom = 10;\n    \
                 mimo x.padrao() + meio() - meio();\n\
             }\n",
        ),
    ]);

    let checagem = pink("c2-579-check", &["--check"], &caso.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));

    let execucao = pink("c2-579-run", &["--run"], &caso.raiz);
    assert_eq!(
        codigo(&execucao),
        17,
        "a relação indireta deixou de vencer: {}",
        stderr(&execucao)
    );
}

/// A precedência é observável nas duas metades ao mesmo tempo.
///
/// A raiz declara o próprio trato e implementa; duas unidades importadas
/// contribuem relações homônimas alcançáveis apenas pelo nível subordinado.
/// Com a precedência corrente, o nível próprio vence sozinho e o programa
/// executa. Invertê-la faria os dois subordinados sobreviverem juntos, e a
/// semântica recusaria a chamada por ambiguidade — o mesmo erro nas duas
/// fases, porque a regra é uma só.
#[test]
fn a_precedencia_do_nivel_proprio_vale_para_check_e_para_execucao() {
    let caso = caso(&[
        ("mc2t", TRATO_MARCA),
        (
            "mc2um",
            "pacote mc2um;\ntrazer mc2t.Marca;\n\n\
             impl Marca para bombom {}\n\n\
             carinho um() -> bombom { mimo 1; }\n",
        ),
        (
            "mc2dois",
            "pacote mc2dois;\n\n\
             trato Outra {\n    \
                 carinho padrao(valor: bombom) -> bombom { mimo valor + 2; }\n\
             }\n\n\
             impl Outra para bombom {}\n\n\
             carinho dois() -> bombom { mimo 2; }\n",
        ),
        (
            "main",
            "pacote main;\ntrazer mc2um.um;\ntrazer mc2dois.dois;\n\n\
             trato Propria {\n    \
                 carinho padrao(valor: bombom) -> bombom { mimo valor + 7; }\n\
             }\n\n\
             impl Propria para bombom {}\n\n\
             carinho principal() -> bombom {\n    \
                 nova x: bombom = 10;\n    \
                 mimo x.padrao() + um() + dois() - um() - dois();\n\
             }\n",
        ),
    ]);

    let checagem = pink("c2-precedencia-check", &["--check"], &caso.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));

    let execucao = pink("c2-precedencia-run", &["--run"], &caso.raiz);
    assert_eq!(
        codigo(&execucao),
        17,
        "o nível próprio deixou de preceder: {}",
        stderr(&execucao)
    );
}

/// A distinção explícito/default é fato de C5 e continua onde estava: C2 só
/// escolhe entre o que já foi materializado.
#[test]
fn o_override_explicito_continua_vencendo_o_default_materializado() {
    let com_default = caso(&[(
        "main",
        "pacote main;\n\n\
         trato Marca {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n\
         }\n\n\
         impl Marca para bombom {}\n\n\
         carinho principal() -> bombom {\n    \
             nova x: bombom = 10;\n    \
             mimo x.padrao();\n\
         }\n",
    )]);
    assert_eq!(
        codigo(&pink("c2-default", &["--run"], &com_default.raiz)),
        11
    );

    let com_override = caso(&[(
        "main",
        "pacote main;\n\n\
         trato Marca {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n\
         }\n\n\
         impl Marca para bombom {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 7; }\n\
         }\n\n\
         carinho principal() -> bombom {\n    \
             nova x: bombom = 10;\n    \
             mimo x.padrao();\n\
         }\n",
    )]);
    assert_eq!(
        codigo(&pink("c2-override", &["--run"], &com_override.raiz)),
        17
    );
}

/// Despacho por slot de objeto de trato NÃO é a mesma pergunta: o trato é
/// nomeado, então não há candidatos a comparar. C2 não o migrou; este é o
/// controle de não-regressão.
#[test]
fn o_despacho_por_objeto_de_trato_permanece_intacto() {
    let caso = caso(&[(
        "main",
        "pacote main;\n\n\
         trato Marca {\n    \
             carinho padrao(valor: si) -> bombom { mimo 1; }\n\
         }\n\n\
         impl Marca para bombom {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 7; }\n\
         }\n\n\
         carinho principal() -> bombom {\n    \
             nova x: bombom = 10;\n    \
             nova obj: trato<Marca> = x virar trato<Marca>;\n    \
             mimo obj.padrao();\n\
         }\n",
    )]);
    let execucao = pink("c2-objeto-de-trato", &["--run"], &caso.raiz);
    assert_eq!(codigo(&execucao), 17, "{}", stderr(&execucao));

    let ir = pink("c2-objeto-de-trato-ir", &["--ir"], &caso.raiz);
    assert_eq!(codigo(&ir), 0, "{}", stderr(&ir));
    assert!(
        stdout(&ir).contains("make_trait_object"),
        "o caso deixou de exercitar vtable de objeto de trato"
    );
}

/// A chamada qualificada nomeia o trato: é resolução de identidade, não
/// seleção entre candidatos. Continua como estava.
#[test]
fn a_chamada_qualificada_por_trato_permanece_intacta() {
    let caso = caso(&[(
        "main",
        "pacote main;\n\n\
         trato Marca {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n\
         }\n\n\
         trato Outra {\n    \
             carinho padrao(valor: bombom) -> bombom { mimo valor + 2; }\n\
         }\n\n\
         impl Marca para bombom {}\n\
         impl Outra para bombom {}\n\n\
         carinho principal() -> bombom {\n    \
             nova x: bombom = 10;\n    \
             mimo Marca.padrao(x);\n\
         }\n",
    )]);
    let execucao = pink("c2-qualificada", &["--run"], &caso.raiz);
    assert_eq!(
        codigo(&execucao),
        11,
        "a chamada qualificada mudou de vencedor: {}",
        stderr(&execucao)
    );
}
