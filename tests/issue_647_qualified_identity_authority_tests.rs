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

/// Uma decisão de identidade qualificada compara as TRÊS componentes contra as
/// entradas de um índice.
///
/// Duas componentes não bastam e não são a mesma pergunta: `(alvo, método)` é a
/// construção de candidatos da chamada NÃO qualificada, que pertence a
/// `method_dispatch` (#590/#591); `(trato, método)` é a cobrança de cobertura de
/// contrato do trato. Nenhuma das duas resolve identidade.
/// Construir a chave e consultar um índice com ela é a mesma decisão por outra
/// forma; era assim que a IR resolvia antes da #647.
fn decide_identidade_qualificada(corpo: &str) -> bool {
    let compara_as_tres = corpo.contains(".trait_name ==")
        && corpo.contains(".target ==")
        && corpo.contains(".method_name ==");
    let consulta_pela_chave = corpo.contains("MethodIdentity::new(")
        && (corpo.contains(".get(")
            || corpo.contains(".find(")
            || corpo.contains(".any(")
            || corpo.contains(".position("));
    compara_as_tres || consulta_pela_chave
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

    let mut decisores: Vec<String> = Vec::new();
    for arquivo in &arquivos {
        let texto = fs::read_to_string(arquivo).expect("ler fonte Rust");
        for corpo in corpos_de_funcao(&texto) {
            if decide_identidade_qualificada(&corpo) {
                let relativo = arquivo
                    .strip_prefix(raiz_do_repositorio())
                    .unwrap_or(arquivo)
                    .to_string_lossy()
                    .replace('\\', "/");
                decisores.push(relativo);
            }
        }
    }
    decisores.sort();
    decisores.dedup();

    assert_eq!(
        decisores,
        vec![AUTORIDADE.to_string()],
        "a identidade qualificada tem de ser decidida em um único lugar; \
         decisores encontrados: {decisores:?}"
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

/// A célula é aceita quando o trato é nomeável pelo chamador; as três formas
/// concordam entre si e `--check` concorda com a execução.
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
            let aceito = if forma == "nao-qualificada" {
                // A forma não qualificada não nomeia trato nenhum; ela depende
                // do alcance da relação, decidido por `method_dispatch`.
                ambiente != "nenhum"
            } else {
                nomeia_o_trato
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
    let chamador = "pacote chamador;\ntrazer tr.Medida;\ncarinho usar(x: bombom) -> bombom {\n    nova v: verso = \"oi\";\n    nova a: bombom = Medida.medir(x);\n    nova b: bombom = Medida.medir(v);\n    mimo a + b;\n}\n";
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
        ("direta", "trazer tr.Medida;\ntrazer tp.Peso;\n"),
        ("invertida", "trazer tp.Peso;\ntrazer tr.Medida;\n"),
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
        &modulos("pacote chamador;\ntrazer ta.Medida;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n".to_string()),
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
        &modulos("pacote chamador;\ntrazer tb.Medida;\ncarinho usar(x: bombom) -> bombom {\n    nova v: verso = \"oi\";\n    mimo Medida.medir(v);\n}\n".to_string()),
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
        &modulos("pacote chamador;\ntrazer tb.Medida;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n".to_string()),
    );
    let saida = pink("647-hom-cruzado", &["--check"], &c.raiz);
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("método 'tb.Medida.medir' não implementado para tipo 'bombom'"),
        "a mensagem tem de acusar o trato canônico nomeado, não o homônimo: {erro}"
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
        "pacote chamador;\ntrazer tr.Medida;\ncarinho usar(x: bombom) -> bombom {\n    mimo Medida.medir(x);\n}\n";
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
