mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::falha_operacional;
use pinker_v0::source_map::SourceId;
use pinker_v0::token::{Position, Span};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

// Evidência da U-06 (#658): quando um diagnóstico tem mais de uma candidata, o sujeito é a primeira
// declaração em ordem de leitura da fonte — unidade na ordem de descoberta do carregador, depois
// posição —, e não a primeira que a iteração de uma tabela hash alcançar. Cobre as quatro famílias
// comprovadas: o conflito de taxonomia de `Resultado<T,E>` reivindicado por template de usuário
// (sujeito estável entre execuções, com a grafia humana do leque no lugar do nome sintético de
// transporte), a validação das declarações coletadas na passagem 1 da semântica (apelido, ninho e
// carga de variante), e os operandos de `sussurro`. Cada família é medida no consumidor real
// (stderr da CLI) e em duas ordens de texto, com a ordem de fonte contradizendo a ordem
// alfabética, de modo que ordenar por grafia não satisfaz o teste.

/// Quantas execuções do MESMO programa cada caso de estabilidade observa.
///
/// A iteração de `HashMap`/`HashSet` varia por processo, então a instabilidade
/// aparece entre execuções e não dentro de uma. Com duas candidatas alcançáveis,
/// dez execuções deixam a chance de um falso verde abaixo de 1 em 500.
const EXECUCOES: usize = 10;

const MODULO_IO: &str = r#"pacote io; trazer arquivo.ler_caminho_resultado;

apelido ResVV = Resultado<verso, verso>;

carinho ler(c: verso) -> ResVV { mimo ler_caminho_resultado(c); }
"#;

/// Raiz que produz o valor pelo runtime através de `io` e redeclara a
/// identidade na própria unidade, na linha 6.
const RAIZ_QUE_REDECLARA: &str = r#"pacote main; trazer ambiente.argumento_ou;

trazer io.ler;

leque Resultado<T, E> { Erro(E), Ok(T) }

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

/// A declaração do usuário está na linha 5 desta raiz.
const LINHA_DA_REDECLARACAO_NA_RAIZ: &str = "5:1..5:41";

/// Módulo que declara o próprio `Resultado<T,E>` e o especializa, sem produzir
/// valor algum pelo runtime. A declaração está na linha 3.
fn modulo_que_declara(pacote: &str, alias: &str, argumentos: &str, carga: &str) -> String {
    format!(
        r#"pacote {pacote};

leque Resultado<T, E> {{ Erro(E), Ok(T) }}

apelido {alias} = Resultado<{argumentos}>;

carinho f_{pacote}() -> {alias} {{ mimo {alias}.Ok({carga}); }}
"#
    )
}

fn escrever(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> std::path::PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte U-06");
    caminho
}

/// Recusa do interpretador, como um humano a vê: stderr inteiro.
fn recusa(caminho: &Path, caso: &str) -> String {
    let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(caminho)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar interpretador U-06 sob envelope");
    assert_eq!(
        saida.status.code(),
        Some(1),
        "{caso}: o programa deveria ser recusado (stdout: {})",
        String::from_utf8_lossy(&saida.stdout)
    );
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

/// Conjunto dos diagnósticos distintos observados em `EXECUCOES` execuções do
/// mesmo programa. Um único elemento é a prova de estabilidade.
fn diagnosticos_distintos(caminho: &Path, caso: &str) -> BTreeSet<String> {
    (0..EXECUCOES)
        .map(|tentativa| recusa(caminho, &format!("{caso}-{tentativa}")))
        .collect()
}

fn unico_diagnostico(caminho: &Path, caso: &str) -> String {
    let observados = diagnosticos_distintos(caminho, caso);
    assert_eq!(
        observados.len(),
        1,
        "{caso}: o mesmo programa produziu diagnósticos diferentes entre execuções:\n{}",
        observados.into_iter().collect::<Vec<_>>().join("\n---\n")
    );
    observados.into_iter().next().expect("um diagnóstico")
}

/// A superfície de referência desta evidência e a grafia que a autoridade
/// declara para o leque que ela produz. O teste não escreve a grafia à mão: se
/// a autoridade mudar, esta evidência acompanha.
fn superficie_de_referencia() -> &'static falha_operacional::SuperficieFalivel {
    falha_operacional::superficie("ler_arquivo_resultado").expect("superfície de filesystem")
}

/// F-04: o diagnóstico nomeia o leque pela grafia humana, não pelo nome
/// sintético de transporte que a AST usa como chave.
#[test]
fn conflito_de_taxonomia_nomeia_o_leque_em_grafia_humana() {
    let dir = NativeArtifactDir::create().expect("diretório U-06");
    fs::write(dir.path().join("io.pink"), MODULO_IO).expect("escrever módulo io");
    let raiz = escrever(&dir, "grafia", RAIZ_QUE_REDECLARA);

    let diagnostico = recusa(&raiz, "u06-grafia-humana");
    let superficie = superficie_de_referencia();

    // Oráculo independente: `Resultado<verso, verso>` é como um programa Pinker
    // escreve este tipo — é fato da linguagem, não da implementação. A asserção
    // derivada da autoridade vem depois, e as duas juntas impedem tanto que a
    // autoridade mude de grafia em silêncio quanto que o teste aceite qualquer
    // grafia consistente.
    assert!(
        diagnostico.contains("Resultado<verso, verso>"),
        "o diagnóstico não nomeia o leque como o usuário o escreve:\n{diagnostico}"
    );
    assert_eq!(
        superficie.grafia_humana(),
        "Resultado<verso, verso>",
        "a grafia humana da autoridade divergiu da grafia da linguagem"
    );
    assert!(
        diagnostico.contains(&superficie.grafia_humana()),
        "o diagnóstico não nomeia o leque como a autoridade o soletra ('{}'):\n{diagnostico}",
        superficie.grafia_humana()
    );
    assert!(
        !diagnostico.contains(&superficie.leque_monomorfico()),
        "o nome sintético de transporte vazou para o diagnóstico humano:\n{diagnostico}"
    );
    assert!(
        diagnostico.contains(&format!("template '{}'", superficie.identidade())),
        "o detalhe não nomeia o template em grafia humana:\n{diagnostico}"
    );
}

/// F-01: com mais de uma declaração alcançável reivindicando a identidade, o
/// sujeito é sempre o mesmo — a primeira em ordem de leitura.
#[test]
fn sujeito_do_conflito_e_estavel_e_vem_da_raiz_quando_ela_declara() {
    let dir = NativeArtifactDir::create().expect("diretório U-06");
    fs::write(dir.path().join("io.pink"), MODULO_IO).expect("escrever módulo io");
    fs::write(
        dir.path().join("outro.pink"),
        modulo_que_declara("outro", "OutroRes", "bombom, bombom", "1"),
    )
    .expect("escrever módulo outro");

    // A raiz importa `outro` e também redeclara: duas unidades-fonte distintas
    // reivindicam a identidade, e as duas materializam especialização.
    let raiz_fonte =
        RAIZ_QUE_REDECLARA.replace("trazer io.ler;", "trazer io.ler;\ntrazer outro.f_outro;");
    let raiz = escrever(&dir, "duas_origens", &raiz_fonte);

    let diagnostico = unico_diagnostico(&raiz, "u06-duas-origens");
    assert!(
        diagnostico.contains("6:1..6:41"),
        "o sujeito deveria ser a declaração da raiz, a primeira em ordem de leitura:\n{diagnostico}"
    );
    assert!(
        !diagnostico.contains("outro.pink"),
        "o diagnóstico aponta uma unidade descoberta depois da raiz:\n{diagnostico}"
    );
}

/// F-01: quando a raiz não declara, o sujeito é a declaração do módulo
/// descoberto primeiro — e a ordem de descoberta é a dos imports, não a ordem
/// alfabética do nome do módulo.
#[test]
fn sujeito_do_conflito_segue_a_ordem_de_descoberta_dos_modulos() {
    const RAIZ: &str = r#"pacote main; trazer ambiente.argumento_ou;

trazer io.ler;
trazer PRIMEIRO.f_PRIMEIRO;
trazer SEGUNDO.f_SEGUNDO;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

    let dir = NativeArtifactDir::create().expect("diretório U-06");
    fs::write(dir.path().join("io.pink"), MODULO_IO).expect("escrever módulo io");
    fs::write(
        dir.path().join("zeta.pink"),
        modulo_que_declara("zeta", "ZetaRes", "bombom, bombom", "1"),
    )
    .expect("escrever módulo zeta");
    fs::write(
        dir.path().join("alfa.pink"),
        modulo_que_declara("alfa", "AlfaRes", "verso, bombom", "\"x\""),
    )
    .expect("escrever módulo alfa");

    for (primeiro, segundo) in [("zeta", "alfa"), ("alfa", "zeta")] {
        let raiz = escrever(
            &dir,
            &format!("ordem_de_import_{primeiro}"),
            &RAIZ
                .replace("PRIMEIRO", primeiro)
                .replace("SEGUNDO", segundo),
        );

        let diagnostico = unico_diagnostico(&raiz, &format!("u06-ordem-import-{primeiro}"));
        assert!(
            diagnostico.contains(&format!("{primeiro}.pink")),
            "o sujeito deveria ser a declaração de '{primeiro}', importado primeiro:\n{diagnostico}"
        );
        assert!(
            !diagnostico.contains(&format!("{segundo}.pink")),
            "o sujeito seguiu outra ordem que não a de descoberta:\n{diagnostico}"
        );
        // A declaração vive na linha 3 de qualquer um dos dois módulos.
        assert!(
            diagnostico.contains("3:1..3:41"),
            "o span não é o da declaração do módulo escolhido:\n{diagnostico}"
        );
    }
}

/// F-01: muitas especializações do MESMO template não multiplicam sujeitos.
#[test]
fn varias_especializacoes_do_mesmo_template_tem_um_sujeito_so() {
    let dir = NativeArtifactDir::create().expect("diretório U-06");
    fs::write(dir.path().join("io.pink"), MODULO_IO).expect("escrever módulo io");
    let raiz_fonte = RAIZ_QUE_REDECLARA.replace(
        "apelido ResVV = Resultado<verso, verso>;",
        "apelido ResVV = Resultado<verso, verso>;\napelido ResBB = Resultado<bombom, bombom>;",
    );
    let raiz = escrever(&dir, "duas_especializacoes", &raiz_fonte);

    let diagnostico = unico_diagnostico(&raiz, "u06-duas-especializacoes");
    assert!(
        diagnostico.contains(LINHA_DA_REDECLARACAO_NA_RAIZ),
        "o sujeito deveria ser a única declaração que existe:\n{diagnostico}"
    );
}

/// F-02: entre declarações coletadas independentemente inválidas, a reportada é
/// a primeira em ordem de leitura. Cada caso aparece nas duas ordens de texto, e
/// em uma delas a ordem de fonte contradiz a ordem alfabética do nome.
#[test]
fn declaracao_invalida_reportada_e_a_primeira_em_ordem_de_leitura() {
    struct Caso {
        nome: &'static str,
        /// Fonte com `PRIMEIRA`/`SEGUNDA` a serem substituídas pelas duas
        /// declarações inválidas, nessa ordem de texto.
        molde: &'static str,
        alfa: &'static str,
        zeta: &'static str,
        /// Trecho que só aparece no diagnóstico da declaração `alfa`.
        marca_alfa: &'static str,
        /// Trecho que só aparece no diagnóstico da declaração `zeta`.
        marca_zeta: &'static str,
    }

    const MOLDE: &str = r#"pacote main;

PRIMEIRA
SEGUNDA

carinho principal() -> bombom { mimo 0; }
"#;

    let casos = [
        Caso {
            nome: "carga_de_variante",
            molde: MOLDE,
            alfa: "leque Alfa { Um(u8) }",
            zeta: "leque Zeta { Dois(u16) }",
            marca_alfa: "variante 'Um'",
            marca_zeta: "variante 'Dois'",
        },
        Caso {
            nome: "ninho_recursivo",
            molde: MOLDE,
            alfa: "ninho Alfa { a: Alfa; }",
            zeta: "ninho Zeta { z: Zeta; }",
            marca_alfa: "struct 'Alfa'",
            marca_zeta: "struct 'Zeta'",
        },
        Caso {
            nome: "apelido_irresoluvel",
            molde: MOLDE,
            alfa: "apelido Alfa = AusenteAlfa;",
            zeta: "apelido Zeta = AusenteZeta;",
            marca_alfa: "'AusenteAlfa'",
            marca_zeta: "'AusenteZeta'",
        },
    ];

    let dir = NativeArtifactDir::create().expect("diretório U-06");
    for caso in &casos {
        for (ordem, (primeira, segunda, esperada, inesperada)) in [
            (caso.alfa, caso.zeta, caso.marca_alfa, caso.marca_zeta),
            (caso.zeta, caso.alfa, caso.marca_zeta, caso.marca_alfa),
        ]
        .into_iter()
        .enumerate()
        {
            let fonte = caso
                .molde
                .replace("PRIMEIRA", primeira)
                .replace("SEGUNDA", segunda);
            let caminho = escrever(&dir, &format!("{}_{ordem}", caso.nome), &fonte);
            let diagnostico = unico_diagnostico(
                &caminho,
                &format!("u06-ordem-declaracao-{}-{esperada}", caso.nome),
            );
            assert!(
                diagnostico.contains(esperada),
                "{}: a declaração reportada não é a primeira do texto:\n{diagnostico}",
                caso.nome
            );
            assert!(
                !diagnostico.contains(inesperada),
                "{}: a segunda declaração inválida venceu a primeira:\n{diagnostico}",
                caso.nome
            );
        }
    }
}

/// F-03: o operando de `sussurro` reportado segue a ordem do texto — a do
/// template para referências, a de declaração para bindings.
#[test]
fn operando_de_sussurro_reportado_segue_a_ordem_do_texto() {
    const TEMPLATE: &str = r#"pacote main;

carinho principal() -> bombom {
    sussurro("nop {PRIMEIRO} {SEGUNDO}");
    mimo 0;
}
"#;

    const BINDING: &str = r#"pacote main;

carinho principal() -> bombom {
    nova x: bombom = 1;
    sussurro("nop"; entrada PRIMEIRO: r8 = x; entrada SEGUNDO: r9 = x; destroi(r8, r9));
    mimo 0;
}
"#;

    let dir = NativeArtifactDir::create().expect("diretório U-06");
    for (forma, fonte) in [("template", TEMPLATE), ("binding", BINDING)] {
        for (primeiro, segundo) in [("zeta", "alfa"), ("alfa", "zeta")] {
            let caminho = escrever(
                &dir,
                &format!("sussurro_{forma}_{primeiro}"),
                &fonte
                    .replace("PRIMEIRO", primeiro)
                    .replace("SEGUNDO", segundo),
            );
            let diagnostico =
                unico_diagnostico(&caminho, &format!("u06-sussurro-{primeiro}-{segundo}"));
            // A asserção observa só a linha da mensagem: o eco da fonte abaixo
            // dela contém os dois operandos por definição.
            let mensagem = diagnostico
                .lines()
                .find(|linha| linha.contains("operando"))
                .unwrap_or_else(|| panic!("diagnóstico sem linha de operando:\n{diagnostico}"));
            assert!(
                mensagem.contains(primeiro),
                "o operando reportado não é o primeiro do texto:\n{diagnostico}"
            );
            assert!(
                !mensagem.contains(segundo),
                "o segundo operando venceu o primeiro:\n{diagnostico}"
            );
        }
    }
}

/// A regra de ordenação em si: unidade-fonte na ordem de descoberta primeiro,
/// posição depois, e span sintético por último.
#[test]
fn ordem_de_leitura_e_fonte_e_depois_posicao() {
    let raiz = SourceId::ROOT;
    let modulo = {
        let mut mapa = pinker_v0::source_map::SourceMap::new();
        mapa.register_root("raiz", "");
        mapa.register_module("m", "m", "")
    };

    let pos = Position::new;
    let na_raiz_linha_9 = Span::em(raiz, pos(9, 1), pos(9, 2));
    let no_modulo_linha_1 = Span::em(modulo, pos(1, 1), pos(1, 2));
    let na_raiz_linha_1_col_9 = Span::em(raiz, pos(1, 9), pos(1, 10));
    let sintetico = Span::single(pos(0, 0));

    assert!(
        na_raiz_linha_9.ordem_de_leitura() < no_modulo_linha_1.ordem_de_leitura(),
        "a unidade descoberta primeiro precede qualquer posição de uma posterior"
    );
    assert!(
        na_raiz_linha_1_col_9.ordem_de_leitura() < na_raiz_linha_9.ordem_de_leitura(),
        "dentro da mesma unidade, a posição decide"
    );
    assert!(
        no_modulo_linha_1.ordem_de_leitura() < sintetico.ordem_de_leitura(),
        "span sintético não reivindica fonte e ordena por último"
    );
}
