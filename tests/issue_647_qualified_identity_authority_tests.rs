mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::method_identity::{
    resolve_qualified_impl_method, MethodIdentity, QualifiedMethodResolution,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Prova da #647/U-03A: a identidade qualificada de método de `impl` tem UMA autoridade. Duas metades. A metade estrutural conta decisores: nenhum arquivo de `src/**` fora de `src/method_identity.rs` pode comparar as três componentes da identidade — trato, alvo resolvido e método — contra um índice de funções materializadas, nem consultar esse índice por chave completa; se a semântica ou a IR voltar a decidir localmente, a contagem passa de um e o teste falha, mesmo que o comportamento continue idêntico. A metade comportamental prova que a regra da autoridade é exatamente a correspondência das três componentes, e que `--check` e o lowering concordam sobre o mesmo programa nas três formas de chamada e nos quatro ambientes de import: nenhuma componente pode ser ignorada sem que alguma célula mude de veredito ou de valor executado. O oráculo é o valor observado e a mensagem renderizada, nunca a ausência de erro.

// ---------------------------------------------------------------------------
// Metade estrutural — quantos decisores existem?
// ---------------------------------------------------------------------------

/// O arquivo que detém a autoridade. Toda outra unidade de `src/**` é
/// consumidora e não pode conter a regra.
const AUTORIDADE: &str = "src/method_identity.rs";

fn raiz_do_repositorio() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fontes_rust(dir: &Path, encontradas: &mut Vec<PathBuf>) {
    for entrada in fs::read_dir(dir).expect("ler diretório de fontes") {
        let caminho = entrada.expect("entrada de diretório").path();
        if caminho.is_dir() {
            fontes_rust(&caminho, encontradas);
        } else if caminho.extension().is_some_and(|ext| ext == "rs") {
            encontradas.push(caminho);
        }
    }
}

/// Fatia o texto em corpos de função pelo início de cada `fn`.
///
/// É deliberadamente grosseiro: o que importa é não deixar duas funções
/// distintas colarem num único trecho, porque a pergunta é se as três
/// comparações vivem JUNTAS na mesma decisão.
fn corpos_de_funcao(texto: &str) -> Vec<String> {
    let mut corpos: Vec<String> = Vec::new();
    let mut atual = String::new();
    for linha in texto.lines() {
        let limpa = linha.trim_start();
        let comeca_funcao = limpa.starts_with("fn ")
            || limpa.starts_with("pub fn ")
            || limpa.starts_with("pub(crate) fn ")
            || limpa.starts_with("pub(super) fn ")
            || limpa.starts_with("async fn ");
        if comeca_funcao && !atual.is_empty() {
            corpos.push(std::mem::take(&mut atual));
        }
        atual.push_str(linha);
        atual.push('\n');
    }
    if !atual.is_empty() {
        corpos.push(atual);
    }
    corpos
}

/// Janela, em caracteres, olhada de cada lado de um `==`.
///
/// Grande o bastante para alcançar `meta.identity.trait_name` do outro lado do
/// operador; pequena o bastante para não colar comparações vizinhas.
const JANELA: usize = 40;

/// Quais componentes da identidade este corpo COMPARA?
///
/// Um campo conta como comparado quando aparece ao lado de um `==`, de
/// qualquer um dos dois lados: `a.trait_name == x` e `x == a.trait_name` são a
/// mesma decisão escrita ao contrário, e um oráculo que só reconhece uma das
/// duas grafias aceita a duplicação escrita na outra. `a.eq(b)` e `a.ne(b)` são
/// a mesma comparação por extenso e entram pela mesma porta.
///
/// Mencionar o campo não basta: `trait_name: identity.trait_name.clone()`
/// constrói um candidato de despacho e não decide identidade nenhuma.
fn componentes_comparados(corpo: &str) -> (bool, bool, bool) {
    // `a.eq(b)` é `a == b` escrito por extenso. Um oráculo que só reconhece o
    // operador aceita a mesma decisão reescrita como chamada de método.
    let normalizado = corpo.replace(".eq(", " == ").replace(".ne(", " == ");
    let compacto = normalizado.split_whitespace().collect::<Vec<_>>().join(" ");
    let partes: Vec<&str> = compacto.split("==").collect();
    let (mut trato, mut alvo, mut metodo) = (false, false, false);
    for janela in partes.windows(2) {
        let esquerda: String = janela[0]
            .chars()
            .rev()
            .take(JANELA)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let direita: String = janela[1].chars().take(JANELA).collect();
        for lado in [esquerda.as_str(), direita.as_str()] {
            trato |= lado.contains(".trait_name");
            alvo |= lado.contains(".target");
            metodo |= lado.contains(".method_name");
        }
    }
    (trato, alvo, metodo)
}

/// Uma decisão de identidade qualificada compara as TRÊS componentes contra as
/// entradas de um índice.
///
/// Duas componentes não bastam e não são a mesma pergunta: `(alvo, método)` é a
/// construção de candidatos da chamada NÃO qualificada, que pertence a
/// `method_dispatch` (#590/#591); `(trato, método)` é a cobrança de cobertura de
/// contrato do trato. Nenhuma das duas resolve identidade.
///
/// Construir a chave e consultar um índice com ela é a mesma decisão por outra
/// forma; era assim que a IR resolvia antes da #647.
fn decide_identidade_qualificada(corpo: &str) -> bool {
    let (trato, alvo, metodo) = componentes_comparados(corpo);
    let compara_as_tres = trato && alvo && metodo;
    let consulta_pela_chave = corpo.contains("MethodIdentity::new(")
        && (corpo.contains(".get(")
            || corpo.contains(".find(")
            || corpo.contains(".any(")
            || corpo.contains(".position("));
    compara_as_tres || consulta_pela_chave
}

/// Primeira linha que declara a função, para nomear o decisor no relatório.
fn assinatura(corpo: &str) -> String {
    corpo
        .lines()
        .find(|linha| linha.contains("fn "))
        .unwrap_or("<sem assinatura>")
        .trim()
        .to_string()
}

#[test]
fn a_regra_de_correspondencia_qualificada_existe_uma_unica_vez_em_src() {
    let src = raiz_do_repositorio().join("src");
    let mut arquivos = Vec::new();
    fontes_rust(&src, &mut arquivos);
    arquivos.sort();
    assert!(
        arquivos.len() > 20,
        "a varredura não encontrou as fontes de src/: {} arquivos",
        arquivos.len()
    );

    // Um decisor é uma FUNÇÃO, não um arquivo: duas decisões no mesmo arquivo
    // continuam sendo duas decisões.
    let mut decisores: Vec<String> = Vec::new();
    for arquivo in &arquivos {
        let texto = fs::read_to_string(arquivo).expect("ler fonte Rust");
        let relativo = arquivo
            .strip_prefix(raiz_do_repositorio())
            .unwrap_or(arquivo)
            .to_string_lossy()
            .replace('\\', "/");
        for corpo in corpos_de_funcao(&texto) {
            if decide_identidade_qualificada(&corpo) {
                decisores.push(format!("{relativo}::{}", assinatura(&corpo)));
            }
        }
    }

    assert_eq!(
        decisores.len(),
        1,
        "a identidade qualificada tem de ser decidida em um único lugar; \
         decisores encontrados: {decisores:?}"
    );
    assert!(
        decisores[0].starts_with(&format!("{AUTORIDADE}::")),
        "o único decisor tem de ser a autoridade: {decisores:?}"
    );
}

#[test]
fn nenhuma_fase_consulta_o_indice_de_metodos_por_chave_completa() {
    let src = raiz_do_repositorio().join("src");
    let mut arquivos = Vec::new();
    fontes_rust(&src, &mut arquivos);

    let mut infratores: Vec<String> = Vec::new();
    for arquivo in &arquivos {
        let texto = fs::read_to_string(arquivo).expect("ler fonte Rust");
        // Consultar `impl_methods` por chave é decidir a correspondência sem
        // passar pela autoridade — a forma que a IR tinha antes da #647.
        if texto.contains("impl_methods.get(") || texto.contains("impl_methods\n            .get(")
        {
            infratores.push(arquivo.to_string_lossy().into_owned());
        }
    }

    assert!(
        infratores.is_empty(),
        "a consulta por chave completa pertence a {AUTORIDADE}; encontrada em {infratores:?}"
    );
}

#[test]
fn semantica_e_ir_consomem_a_autoridade_canonica() {
    for consumidor in ["src/semantic/calls.rs", "src/ir/lowering.rs"] {
        let texto =
            fs::read_to_string(raiz_do_repositorio().join(consumidor)).expect("ler consumidor");
        assert!(
            texto.contains("method_identity::resolve_qualified_impl_method("),
            "{consumidor} tem de derivar a decisão de {AUTORIDADE}"
        );
    }
}

// ---------------------------------------------------------------------------
// A regra da autoridade — nenhuma componente é dispensável
// ---------------------------------------------------------------------------

fn indice() -> Vec<(MethodIdentity<&'static str>, &'static str)> {
    vec![
        (
            MethodIdentity::new("tr.Medida".into(), "bombom", "medir".into()),
            "f_medida_bombom_medir",
        ),
        (
            MethodIdentity::new("tr.Medida".into(), "verso", "medir".into()),
            "f_medida_verso_medir",
        ),
        (
            MethodIdentity::new("tp.Peso".into(), "bombom", "medir".into()),
            "f_peso_bombom_medir",
        ),
    ]
}

fn resolver(trato: &str, alvo: &str, metodo: &str) -> QualifiedMethodResolution {
    let entradas = indice();
    resolve_qualified_impl_method(
        entradas
            .iter()
            .map(|(identidade, funcao)| (identidade, *funcao)),
        trato,
        &alvo,
        metodo,
    )
}

#[test]
fn a_autoridade_corresponde_as_tres_componentes() {
    assert_eq!(
        resolver("tr.Medida", "bombom", "medir"),
        QualifiedMethodResolution::Resolved("f_medida_bombom_medir".into())
    );
    // O alvo participa: mesmo trato e mesmo método, alvo diferente.
    assert_eq!(
        resolver("tr.Medida", "verso", "medir"),
        QualifiedMethodResolution::Resolved("f_medida_verso_medir".into())
    );
    // O trato participa: mesmo alvo e mesmo método, trato diferente.
    assert_eq!(
        resolver("tp.Peso", "bombom", "medir"),
        QualifiedMethodResolution::Resolved("f_peso_bombom_medir".into())
    );
    // O método participa.
    assert_eq!(
        resolver("tr.Medida", "bombom", "inexistente"),
        QualifiedMethodResolution::NoMatch
    );
    // Nenhuma correspondência parcial vira aproximação.
    assert_eq!(
        resolver("tp.Peso", "verso", "medir"),
        QualifiedMethodResolution::NoMatch
    );
    // A grafia crua do trato não é identidade: o homônimo não corresponde.
    assert_eq!(
        resolver("Medida", "bombom", "medir"),
        QualifiedMethodResolution::NoMatch
    );
}

// ---------------------------------------------------------------------------
// Metade comportamental — matriz 3 formas x 4 ambientes
// ---------------------------------------------------------------------------

struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte)
        .unwrap_or_else(|erro| panic!("gravar {}: {erro}", caminho.display()));
    caminho
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #647");
    for (modulo, fonte) in modulos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
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

const TR: &str = "pacote tr;\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
const OUTRO: &str = "pacote outro;\ntrazer tr.Medida;\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n";
const RAIZ: &str = "pacote main;\ntrazer tr.Medida;\ntrazer outro;\ntrazer chamador.usar;\ncarinho principal() -> bombom {\n    mimo usar(5);\n}\n";

/// As quatro configurações de import do chamador.
const AMBIENTES: [(&str, &str); 4] = [
    ("trato-nomeado", "trazer tr.Medida;\n"),
    ("nenhum", ""),
    ("unidade-de-impl", "trazer outro;\n"),
    ("ambos", "trazer tr.Medida;\ntrazer outro;\n"),
];

/// As três formas de chamada.
const FORMAS: [(&str, &str); 3] = [
    ("nao-qualificada", "    mimo x.medir();\n"),
    ("qualificada", "    mimo Medida.medir(x);\n"),
    (
        "objeto-de-trato",
        "    nova obj: trato<Medida> = x virar trato<Medida>;\n    mimo obj.medir();\n",
    ),
];

fn caso_da_matriz(forma: &str, corpo: &str, ambiente: &str, imports: &str) -> Caso {
    let chamador =
        format!("pacote chamador;\n{imports}carinho usar(x: bombom) -> bombom {{\n{corpo}}}\n");
    caso(
        &format!("raiz_{forma}_{ambiente}"),
        RAIZ,
        &[
            ("tr", TR.to_string()),
            ("outro", OUTRO.to_string()),
            ("chamador", chamador),
        ],
    )
}

/// A célula é aceita quando a RELAÇÃO alcança o chamador — e, nas formas que
/// nomeiam o trato, também quando o trato é nomeável. As três formas concordam
/// entre si e `--check` concorda com a execução.
///
/// #649/`POLICY_B_RELATION_REACHABILITY_ALWAYS_MATTERS` migrou três das doze
/// células desta matriz, todas no ambiente `trato-nomeado`: nomear `tr.Medida`
/// deixou de autorizar a relação declarada por `outro`, unidade que o chamador
/// nunca importou. As outras nove são as mesmas de antes.
#[test]
fn a_matriz_de_tres_formas_por_quatro_ambientes_e_estavel() {
    for (forma, corpo) in FORMAS {
        for (ambiente, imports) in AMBIENTES {
            let c = caso_da_matriz(forma, corpo, ambiente, imports);
            let logico = format!("647-{forma}-{ambiente}");
            let checagem = pink(&logico, &["--check"], &c.raiz);
            let execucao = pink(&logico, &["--run"], &c.raiz);

            // O chamador nomeia o trato em dois dos quatro ambientes. Sem isso,
            // a forma qualificada e o objeto de trato não chegam à identidade:
            // a resolução nominal recusa antes.
            let nomeia_o_trato = imports.contains("tr.Medida");
            // A única relação do caso é declarada por `outro`; alcançá-la exige
            // importar essa unidade (#649/POLICY_B).
            let alcanca_a_relacao = imports.contains("outro");
            let aceito = if forma == "nao-qualificada" {
                // A forma não qualificada não nomeia trato nenhum; ela depende
                // só do alcance da relação.
                alcanca_a_relacao
            } else {
                // As formas que nomeiam o trato precisam das DUAS coisas: a
                // resolução nominal recusa antes quando o trato não é nomeável,
                // e o alcance recusa depois quando a relação não chega aqui.
                nomeia_o_trato && alcanca_a_relacao
            };

            if aceito {
                assert_eq!(
                    codigo(&checagem),
                    0,
                    "{forma}/{ambiente} devia ser aceito: {}",
                    stderr(&checagem)
                );
                assert_eq!(
                    codigo(&execucao),
                    77,
                    "{forma}/{ambiente} devia executar o método da relação: {}",
                    stderr(&execucao)
                );
            } else {
                assert_eq!(
                    codigo(&checagem),
                    1,
                    "{forma}/{ambiente} devia ser recusado: {}",
                    stderr(&checagem)
                );
                assert_eq!(
                    codigo(&execucao),
                    1,
                    "o lowering tem de recusar o que `--check` recusou em {forma}/{ambiente}: {}",
                    stderr(&execucao)
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Negativos que provam alcance da autoridade qualificada
// ---------------------------------------------------------------------------

/// O chamador nomeia o trato e o programa é bem formado até a identidade: a
/// recusa vem da autoridade qualificada, com o trato canônico na mensagem.
#[test]
fn metodo_ausente_e_recusado_pela_autoridade_e_nao_pela_resolucao_nominal() {
    let chamador =
        "pacote chamador;\ntrazer tr.Medida;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.inexistente(x);\n}\n";
    let c = caso(
        "raiz_metodo_ausente",
        RAIZ,
        &[
            ("tr", TR.to_string()),
            ("outro", OUTRO.to_string()),
            ("chamador", chamador.to_string()),
        ],
    );
    let saida = pink("647-metodo-ausente", &["--check"], &c.raiz);
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("método 'tr.Medida.inexistente' não implementado para tipo 'bombom'"),
        "a recusa tem de vir da identidade qualificada, não da resolução nominal: {erro}"
    );
}

/// Mesmo trato e mesmo método, alvo diferente: o alvo resolvido participa da
/// identidade, e o programa executa os dois métodos distintos.
#[test]
fn o_alvo_resolvido_desempata_a_identidade_qualificada() {
    let outro = "pacote outro;\ntrazer tr.Medida;\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 20; }\n}\nimpl Medida para verso {\n    carinho medir(valor: verso) -> bombom { mimo 3; }\n}\n";
    let chamador = "pacote chamador;\ntrazer tr.Medida;\ntrazer outro;\ncarinho usar(x: bombom) -> bombom {\n    nova v: verso = \"oi\";\n    nova a: bombom = Medida.medir(x);\n    nova b: bombom = Medida.medir(v);\n    mimo a + b;\n}\n";
    let c = caso(
        "raiz_alvo",
        RAIZ,
        &[
            ("tr", TR.to_string()),
            ("outro", outro.to_string()),
            ("chamador", chamador.to_string()),
        ],
    );
    assert_eq!(codigo(&pink("647-alvo", &["--check"], &c.raiz)), 0);
    let execucao = pink("647-alvo", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        23,
        "cada alvo tem de resolver para o seu próprio método (20 + 3): {}",
        stderr(&execucao)
    );
}

/// Mesmo alvo e mesmo método, tratos diferentes: o trato participa da
/// identidade.
#[test]
fn o_trato_desempata_a_identidade_qualificada() {
    let tp = "pacote tp;\ntrato Peso {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let outro = "pacote outro;\ntrazer tr.Medida;\ntrazer tp.Peso;\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 20; }\n}\nimpl Peso para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 3; }\n}\n";
    let raiz = "pacote main;\ntrazer tr.Medida;\ntrazer tp.Peso;\ntrazer outro;\ntrazer chamador.usar;\ncarinho principal() -> bombom {\n    mimo usar(5);\n}\n";
    let corpo = "carinho usar(x: bombom) -> bombom {\n    nova a: bombom = Medida.medir(x);\n    nova b: bombom = Peso.medir(x);\n    mimo a + b;\n}\n";

    for (nome, imports) in [
        (
            "direta",
            "trazer tr.Medida;\ntrazer tp.Peso;\ntrazer outro;\n",
        ),
        (
            "invertida",
            "trazer tp.Peso;\ntrazer tr.Medida;\ntrazer outro;\n",
        ),
    ] {
        let chamador = format!("pacote chamador;\n{imports}{corpo}");
        let c = caso(
            &format!("raiz_trato_{nome}"),
            raiz,
            &[
                ("tr", TR.to_string()),
                ("tp", tp.to_string()),
                ("outro", outro.to_string()),
                ("chamador", chamador),
            ],
        );
        let logico = format!("647-trato-{nome}");
        assert_eq!(codigo(&pink(&logico, &["--check"], &c.raiz)), 0);
        let execucao = pink(&logico, &["--run"], &c.raiz);
        assert_eq!(
            codigo(&execucao),
            23,
            "cada trato tem de resolver para o seu próprio método (20 + 3), \
             e a ordem dos imports não muda isso ({nome}): {}",
            stderr(&execucao)
        );
    }
}

/// Tratos homônimos de unidades distintas: a identidade canônica do trato, não
/// a grafia crua, decide. `ta.Medida` vale para `bombom`; `tb.Medida`, para
/// `verso`; e nomear `tb.Medida` sobre `bombom` é recusado pela autoridade.
#[test]
fn tratos_homonimos_nao_compartilham_identidade_qualificada() {
    let ta = "pacote ta;\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let tb = "pacote tb;\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let impla = "pacote impla;\ntrazer ta.Medida;\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 11; }\n}\n";
    let implb = "pacote implb;\ntrazer tb.Medida;\nimpl Medida para verso {\n    carinho medir(valor: verso) -> bombom { mimo 22; }\n}\n";
    let raiz = "pacote main;\ntrazer impla;\ntrazer implb;\ntrazer chamador.usar;\ncarinho principal() -> bombom {\n    mimo usar(5);\n}\n";

    let modulos = |chamador: String| {
        vec![
            ("ta", ta.to_string()),
            ("tb", tb.to_string()),
            ("impla", impla.to_string()),
            ("implb", implb.to_string()),
            ("chamador", chamador),
        ]
    };

    let c = caso(
        "raiz_hom_a",
        raiz,
        &modulos("pacote chamador;\ntrazer ta.Medida;\ntrazer impla;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n".to_string()),
    );
    assert_eq!(codigo(&pink("647-hom-a", &["--check"], &c.raiz)), 0);
    assert_eq!(
        codigo(&pink("647-hom-a", &["--run"], &c.raiz)),
        11,
        "o trato nomeado é o de `ta`"
    );

    let c = caso(
        "raiz_hom_b",
        raiz,
        &modulos("pacote chamador;\ntrazer tb.Medida;\ntrazer implb;\ncarinho usar(x: bombom) -> bombom {\n    nova v: verso = \"oi\";\n    mimo Medida.medir(v);\n}\n".to_string()),
    );
    assert_eq!(codigo(&pink("647-hom-b", &["--check"], &c.raiz)), 0);
    assert_eq!(
        codigo(&pink("647-hom-b", &["--run"], &c.raiz)),
        22,
        "o trato nomeado é o de `tb`"
    );

    let c = caso(
        "raiz_hom_cruzado",
        raiz,
        &modulos("pacote chamador;\ntrazer tb.Medida;\ntrazer impla;\ntrazer implb;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n".to_string()),
    );
    let saida = pink("647-hom-cruzado", &["--check"], &c.raiz);
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("método 'tb.Medida.medir' não implementado para tipo 'bombom'"),
        "a mensagem tem de acusar o trato canônico nomeado, não o homônimo: {erro}"
    );
}

// ---------------------------------------------------------------------------
// LAW-03, segunda metade do espaço: sítio da chamada em corpo default de trato
// materializado cross-unit
// ---------------------------------------------------------------------------

const TR2: &str = "pacote tr2;\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
const IMPL_M: &str = "pacote impl_m;\ntrazer tr2.Medida;\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n";
const USER: &str = "pacote user;\ntrazer tr.Base;\nimpl Base para bombom { }\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n";
const RAIZ_DEF: &str = "pacote main;\ntrazer tr.Base;\ntrazer tr2.Medida;\ntrazer impl_m;\ntrazer user.usar;\ncarinho principal() -> bombom { mimo usar(5); }\n";

/// Os corpos do método default do trato `Base`, nas três formas de chamada.
const FORMAS_DEFAULT: [(&str, &str); 3] = [
    ("nao-qualificada", "        mimo valor.medir();\n"),
    ("qualificada", "        mimo Medida.medir(valor);\n"),
    (
        "objeto-de-trato",
        "        nova obj: trato<Medida> = valor virar trato<Medida>;\n        mimo obj.medir();\n",
    ),
];

/// O que a unidade DECLARANTE do trato importa. É o ambiente que decide, por
/// contrato #517 e pela correção de alcance da U-04 — não o do importador.
const AMBIENTES_DECLARANTE: [(&str, &str); 4] = [
    ("trato-nomeado", "trazer tr2.Medida;\n"),
    ("nenhum", ""),
    ("unidade-de-impl", "trazer impl_m;\n"),
    ("ambos", "trazer tr2.Medida;\ntrazer impl_m;\n"),
];

fn caso_materializado(forma: &str, corpo: &str, ambiente: &str, imports: &str) -> Caso {
    let declarante = format!(
        "pacote tr;\n{imports}trato Base {{\n    carinho rodar(valor: si) -> bombom {{\n{corpo}    }}\n}}\n"
    );
    caso(
        &format!("raizdef_{forma}_{ambiente}"),
        RAIZ_DEF,
        &[
            ("tr2", TR2.to_string()),
            ("impl_m", IMPL_M.to_string()),
            ("tr", declarante),
            ("user", USER.to_string()),
        ],
    )
}

/// O corpo default materializado atravessa para uma unidade que não podia
/// nomear o trato, e o veredito segue o ambiente da DECLARANTE nas três formas.
///
/// A forma não qualificada exige que a declarante alcance a relação; as formas
/// que NOMEIAM o trato exigem, além disso, que a declarante o tenha importado.
/// O AMBIENTE continua sendo o da declarante — é o contrato #517/U-04 que esta
/// unidade preserva —; o que a #649 mudou foi o predicado de alcance, nas mesmas
/// três células `trato-nomeado` da outra metade do espaço.
#[test]
fn a_matriz_do_corpo_default_materializado_segue_a_unidade_declarante() {
    for (forma, corpo) in FORMAS_DEFAULT {
        for (ambiente, imports) in AMBIENTES_DECLARANTE {
            let c = caso_materializado(forma, corpo, ambiente, imports);
            let logico = format!("647-def-{forma}-{ambiente}");
            let checagem = pink(&logico, &["--check"], &c.raiz);
            let execucao = pink(&logico, &["--run"], &c.raiz);

            let nomeia_o_trato = imports.contains("tr2.Medida");
            let alcanca_a_relacao = imports.contains("impl_m");
            let aceito = if forma == "nao-qualificada" {
                alcanca_a_relacao
            } else {
                nomeia_o_trato && alcanca_a_relacao
            };

            if aceito {
                assert_eq!(
                    codigo(&checagem),
                    0,
                    "def {forma}/{ambiente} devia ser aceito: {}",
                    stderr(&checagem)
                );
                assert_eq!(
                    codigo(&execucao),
                    77,
                    "def {forma}/{ambiente} devia executar o método da relação: {}",
                    stderr(&execucao)
                );
            } else {
                assert_eq!(
                    codigo(&checagem),
                    1,
                    "def {forma}/{ambiente} devia ser recusado: {}",
                    stderr(&checagem)
                );
                assert_eq!(
                    codigo(&execucao),
                    1,
                    "o lowering tem de recusar o que `--check` recusou em def {forma}/{ambiente}: {}",
                    stderr(&execucao)
                );
            }
        }
    }
}

/// Terceiro estado do trato no gerador de LAW-03: declarado na PRÓPRIA unidade
/// que escreve a chamada, em vez de importado ou não possuído.
///
/// Vale nas duas metades do espaço — corpo próprio e corpo default de trato
/// materializado cross-unit — e nas três formas de chamada. Aqui a resolução
/// nominal nunca recusa, então toda célula chega à identidade.
#[test]
fn trato_declarado_na_propria_unidade_resolve_nas_tres_formas() {
    let raiz_propria = "pacote main;\ntrazer chamador.usar;\ncarinho principal() -> bombom {\n    mimo usar(5);\n}\n";
    let declaracao = "trato Medida {\n    carinho medir(valor: si) -> bombom;\n}\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n";

    for (forma, corpo) in FORMAS {
        let chamador = format!(
            "pacote chamador;\n{declaracao}carinho usar(x: bombom) -> bombom {{\n{corpo}}}\n"
        );
        let c = caso(
            &format!("raizprop_{forma}"),
            raiz_propria,
            &[("chamador", chamador)],
        );
        let logico = format!("647-proprio-{forma}");
        let checagem = pink(&logico, &["--check"], &c.raiz);
        assert_eq!(
            codigo(&checagem),
            0,
            "trato próprio/{forma} devia ser aceito: {}",
            stderr(&checagem)
        );
        let execucao = pink(&logico, &["--run"], &c.raiz);
        assert_eq!(
            codigo(&execucao),
            77,
            "trato próprio/{forma} devia executar o método da relação: {}",
            stderr(&execucao)
        );
    }

    let raiz_def = "pacote main;\ntrazer tr.Base;\ntrazer user.usar;\ncarinho principal() -> bombom {{ mimo usar(5); }}\n"
        .replace("{{", "{")
        .replace("}}", "}");
    for (forma, corpo) in FORMAS_DEFAULT {
        let declarante = format!(
            "pacote tr;\n{declaracao}trato Base {{\n    carinho rodar(valor: si) -> bombom {{\n{corpo}    }}\n}}\n"
        );
        let c = caso(
            &format!("raizpropdef_{forma}"),
            &raiz_def,
            &[("tr", declarante), ("user", USER.to_string())],
        );
        let logico = format!("647-propriodef-{forma}");
        let checagem = pink(&logico, &["--check"], &c.raiz);
        assert_eq!(
            codigo(&checagem),
            0,
            "default com trato próprio/{forma} devia ser aceito: {}",
            stderr(&checagem)
        );
        let execucao = pink(&logico, &["--run"], &c.raiz);
        assert_eq!(
            codigo(&execucao),
            77,
            "default com trato próprio/{forma} devia executar o método: {}",
            stderr(&execucao)
        );
    }
}

/// Alvo nominal apelidado, inclusive em cadeia: a identidade é a resolvida, não
/// a grafia escrita na assinatura.
#[test]
fn apelido_encadeado_do_alvo_resolve_para_a_mesma_identidade_qualificada() {
    let chamador = "pacote chamador;\ntrazer tr.Medida;\ntrazer outro;\napelido Doce = bombom;\napelido DoceEncadeado = Doce;\ncarinho usar(x: DoceEncadeado) -> bombom {\n    mimo Medida.medir(x);\n}\n";
    let c = caso(
        "raiz_apelido",
        RAIZ,
        &[
            ("tr", TR.to_string()),
            ("outro", OUTRO.to_string()),
            ("chamador", chamador.to_string()),
        ],
    );
    let checagem = pink("647-apelido", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("647-apelido", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        77,
        "o apelido tem de resolver para a mesma relação de `bombom`: {}",
        stderr(&execucao)
    );
}

/// Interpretador e binário nativo consomem a mesma identidade qualificada: se a
/// autoridade compartilhada e o caminho nativo divergissem, o valor executado
/// mudaria de um para o outro.
#[test]
fn paridade_interpretador_e_nativo_da_chamada_qualificada() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-647-paridade", true)
    else {
        return;
    };
    let chamador =
        "pacote chamador;\ntrazer tr.Medida;\ntrazer outro;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n";
    let c = caso(
        "paridade_647",
        RAIZ,
        &[
            ("tr", TR.to_string()),
            ("outro", OUTRO.to_string()),
            ("chamador", chamador.to_string()),
        ],
    );
    let interpretado = pink("647-paridade-interpretador", &["--run"], &c.raiz);
    assert_eq!(codigo(&interpretado), 77, "{}", stderr(&interpretado));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("647-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #647");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_647"))
        .logical_case("647-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #647");
    assert_eq!(
        nativo.status.code(),
        interpretado.status.code(),
        "interpretador e nativo têm de resolver a mesma identidade qualificada"
    );
}
