mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.tratos.relacao-de-impl-duplicada
// @pinker-nav:domain tratos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental da #572: a relação nominal de `impl` existe pela declaração, não pelos métodos que o bloco materializa. A matriz fixa a cardinalidade `(trato canônico, alvo canônico) <= 1` em bloco vazio somado a bloco explícito nas duas ordens, dois blocos vazios, dois blocos com métodos explícitos distintos, grafias de alvo canonicamente equivalentes, trato importado da #517 e colocação raiz/não-raiz; e fixa o que NÃO é duplicata: um único bloco vazio com defaults suficientes, ausência de método requerido — que continua sendo erro de cobertura —, tratos homônimos de origens canônicas distintas e alvos canonicamente distintos. O oráculo é o diagnóstico da relação, que cita a identidade canônica do trato, aponta o segundo bloco, preserva o span do primeiro e nunca nomeia `__impl_*` nem `__trait_default_check_*`: nome sintético é transporte, não autoridade de coerência.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #572");
    for (modulo, fonte) in modulos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
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

fn checar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--check"], &caso.raiz)
}

fn executar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--run"], &caso.raiz)
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

/// O que uma recusa de relação duplicada precisa provar, sempre.
///
/// Nome sintético é transporte: se ele aparecesse aqui, a coerência teria sido
/// decidida por colisão de símbolo gerado, e não pela identidade nominal.
fn diagnostico_de_relacao_duplicada(saida: &std::process::Output, trato_canonico: &str) {
    let texto = stderr(saida);
    assert_eq!(
        codigo(saida),
        1,
        "relação duplicada precisa ser recusada; stdout: {}",
        stdout(saida)
    );
    assert!(
        texto.contains(&format!("impl do trato '{trato_canonico}'")),
        "a recusa precisa citar a identidade canônica do trato; veio: {texto}"
    );
    assert!(
        texto.contains("outra declaração em"),
        "a recusa precisa preservar a origem da primeira declaração; veio: {texto}"
    );
    assert!(
        !texto.contains("__impl_"),
        "nome sintético não pode ser a autoridade da recusa; veio: {texto}"
    );
    assert!(
        !texto.contains("__trait_default_check_"),
        "nome sintético não pode ser a autoridade da recusa; veio: {texto}"
    );
}

/// Trato de um método só, inteiramente coberto por default.
///
/// Um bloco vazio para ele é semanticamente válido — e continua sendo uma
/// declaração da relação.
fn trato_com_default(pacote: &str) -> String {
    format!(
        "pacote {pacote};\n\n\
         trato Marca {{\n    \
             carinho marcar(valor: bombom) -> bombom {{ mimo valor + 1; }}\n\
         }}\n"
    )
}

const BLOCO_VAZIO: &str = "impl Marca para bombom {}\n";
const BLOCO_EXPLICITO: &str =
    "impl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n";

fn raiz_de_um_arquivo(corpo: &str) -> String {
    format!(
        "pacote main;\n\n\
         trato Marca {{\n    \
             carinho marcar(valor: bombom) -> bombom {{ mimo valor + 1; }}\n\
         }}\n\n\
         {corpo}\n\
         carinho principal() -> bombom {{ mimo 0; }}\n"
    )
}

// ---------------------------------------------------------------------------
// D1..D5 — cardinalidade da relação, independente de método explícito
// ---------------------------------------------------------------------------

/// D1 — bloco vazio somado a bloco explícito, nas duas ordens.
///
/// Este é o buraco que a #572 fecha: o bloco sem método explícito não
/// materializa nada e, sob a política anterior, sumia da coerência.
#[test]
fn d1_vazio_mais_explicito_e_duplicata_nas_duas_ordens() {
    for (nome, corpo) in [
        ("d1a_572", format!("{BLOCO_VAZIO}{BLOCO_EXPLICITO}")),
        ("d1b_572", format!("{BLOCO_EXPLICITO}{BLOCO_VAZIO}")),
    ] {
        let c = caso(nome, &raiz_de_um_arquivo(&corpo), &[]);
        let saida = checar(&c, nome);
        diagnostico_de_relacao_duplicada(&saida, "Marca");
    }
}

/// D2 — dois blocos vazios. Nenhum método explícito em lugar nenhum, e ainda
/// assim duas declarações da mesma relação.
#[test]
fn d2_dois_blocos_vazios_sao_duplicata() {
    let c = caso(
        "d2_572",
        &raiz_de_um_arquivo(&format!("{BLOCO_VAZIO}{BLOCO_VAZIO}")),
        &[],
    );
    let saida = checar(&c, "572-d2");
    diagnostico_de_relacao_duplicada(&saida, "Marca");
}

/// D3 — dois blocos com métodos explícitos DISTINTOS não completam uma
/// relação: são duas declarações dela.
#[test]
fn d3_metodos_explicitos_distintos_nao_completam_uma_relacao() {
    let fonte = "pacote main;\n\n\
                 trato Marca {\n    \
                     carinho primeiro(valor: bombom) -> bombom;\n    \
                     carinho segundo(valor: bombom) -> bombom;\n\
                 }\n\n\
                 impl Marca para bombom {\n    \
                     carinho primeiro(valor: bombom) -> bombom { mimo valor + 1; }\n\
                 }\n\
                 impl Marca para bombom {\n    \
                     carinho segundo(valor: bombom) -> bombom { mimo valor + 2; }\n\
                 }\n\n\
                 carinho principal() -> bombom { mimo 0; }\n";
    let c = caso("d3_572", fonte, &[]);
    let saida = checar(&c, "572-d3");
    diagnostico_de_relacao_duplicada(&saida, "Marca");
}

/// D4 — grafias de alvo canonicamente equivalentes são a MESMA relação.
///
/// A identidade vem de `union_canon`, a mesma autoridade que a identidade de
/// método usa; a grafia escrita continua aparecendo no diagnóstico, sem
/// decidir nada.
#[test]
fn d4_grafias_equivalentes_do_alvo_sao_a_mesma_relacao() {
    let fonte = "pacote main;\n\n\
                 apelido Numero = bombom;\n\n\
                 trato Marca {\n    \
                     carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n\
                 }\n\n\
                 impl Marca para bombom {}\n\
                 impl Marca para Numero {}\n\n\
                 carinho principal() -> bombom { mimo 0; }\n";
    let c = caso("d4_572", fonte, &[]);
    let saida = checar(&c, "572-d4");
    diagnostico_de_relacao_duplicada(&saida, "Marca");
    let texto = stderr(&saida);
    assert!(
        texto.contains("'Numero' e 'bombom' resolvem para 'bombom'"),
        "a equivalência precisa ser explicada pela grafia escrita; veio: {texto}"
    );
}

/// D5 — a duplicata é vista mesmo quando os dois blocos estão em unidades
/// físicas diferentes, nas duas colocações.
#[test]
fn d5_raiz_e_nao_raiz_seguem_a_mesma_regra() {
    let trato = ("m572t", trato_com_default("m572t"));

    let vazio_no_modulo = caso(
        "d5a_572",
        "pacote main;\ntrazer m572t.Marca;\ntrazer m572v.usa;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n\ncarinho principal() -> bombom { mimo usa(); }\n",
        &[
            trato.clone(),
            (
                "m572v",
                "pacote m572v;\ntrazer m572t.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usa() -> bombom { mimo 1; }\n".to_string(),
            ),
        ],
    );
    diagnostico_de_relacao_duplicada(&checar(&vazio_no_modulo, "572-d5a"), "m572t.Marca");

    let explicito_no_modulo = caso(
        "d5b_572",
        "pacote main;\ntrazer m572t.Marca;\ntrazer m572e.usa;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom { mimo usa(); }\n",
        &[
            trato,
            (
                "m572e",
                "pacote m572e;\ntrazer m572t.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n\ncarinho usa() -> bombom { mimo 1; }\n".to_string(),
            ),
        ],
    );
    diagnostico_de_relacao_duplicada(&checar(&explicito_no_modulo, "572-d5b"), "m572t.Marca");

    let dois_modulos = caso(
        "d5c_572",
        "pacote main;\ntrazer m572a.usa_a;\ntrazer m572b.usa_b;\n\ncarinho principal() -> bombom { mimo usa_a() + usa_b(); }\n",
        &[
            ("m572t", trato_com_default("m572t")),
            (
                "m572a",
                "pacote m572a;\ntrazer m572t.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usa_a() -> bombom { mimo 1; }\n".to_string(),
            ),
            (
                "m572b",
                "pacote m572b;\ntrazer m572t.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n\ncarinho usa_b() -> bombom { mimo 2; }\n".to_string(),
            ),
        ],
    );
    diagnostico_de_relacao_duplicada(&checar(&dois_modulos, "572-d5c"), "m572t.Marca");
}

/// D6 — trato explicitamente importado (#517) mantém a identidade canônica na
/// recusa da duplicata: o que aparece é `m572t.Marca`, nunca a grafia local.
#[test]
fn d6_trato_importado_preserva_identidade_canonica_na_duplicata() {
    let c = caso(
        "d6_572",
        "pacote main;\ntrazer m572t.Marca;\n\nimpl Marca para bombom {}\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[("m572t", trato_com_default("m572t"))],
    );
    let saida = checar(&c, "572-d6");
    diagnostico_de_relacao_duplicada(&saida, "m572t.Marca");
    let texto = stderr(&saida);
    assert!(
        !texto.contains("impl do trato 'Marca'"),
        "a grafia local não pode substituir a identidade canônica; veio: {texto}"
    );
}

// ---------------------------------------------------------------------------
// V1..V4 — o que NÃO é duplicata
// ---------------------------------------------------------------------------

/// V1 — um único bloco vazio, com todos os métodos supridos por default,
/// continua válido e continua executando.
#[test]
fn v1_bloco_vazio_unico_com_defaults_suficientes_e_valido() {
    let c = caso(
        "v1_572",
        "pacote main;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar());\n    mimo 0;\n}\n",
        &[],
    );
    let saida = checar(&c, "572-v1");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "572-v1-run");
    assert_eq!(codigo(&execucao), 0, "{}", stderr(&execucao));
    assert_eq!(stdout(&execucao), "11\n");
}

/// V2 — bloco vazio único que omite método requerido é erro de COBERTURA.
///
/// Falta de método não pode virar falso conflito de relação.
#[test]
fn v2_metodo_requerido_ausente_e_erro_de_cobertura_e_nao_duplicata() {
    let c = caso(
        "v2_572",
        "pacote main;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom;\n}\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[],
    );
    let saida = checar(&c, "572-v2");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    let texto = stderr(&saida);
    assert!(
        texto.contains("não implementa método 'marcar'"),
        "a ausência precisa ser cobrada pela autoridade de cobertura; veio: {texto}"
    );
    assert!(
        !texto.contains("já declarado"),
        "ausência de método não é duplicata de relação; veio: {texto}"
    );
}

/// V3 — dois tratos textualmente homônimos, de origens canônicas distintas,
/// implementados para o MESMO alvo, continuam independentes.
#[test]
fn v3_tratos_homonimos_de_origens_distintas_sao_relacoes_independentes() {
    let c = caso(
        "v3_572",
        "pacote main;\ntrazer m572h1.Marca;\ntrazer m572h2.usa;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar());\n    falar(usa());\n    mimo 0;\n}\n",
        &[
            ("m572h1", trato_com_default("m572h1")),
            (
                "m572h2",
                "pacote m572h2;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 2; }\n}\n\nimpl Marca para bombom {}\n\ncarinho usa() -> bombom { nova y: bombom = 20; mimo y.marcar(); }\n".to_string(),
            ),
        ],
    );
    let saida = checar(&c, "572-v3");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "572-v3-run");
    assert_eq!(codigo(&execucao), 0, "{}", stderr(&execucao));
    assert_eq!(stdout(&execucao), "11\n22\n");
}

/// V4 — o mesmo trato para alvos canonicamente distintos continua independente.
#[test]
fn v4_alvos_canonicamente_distintos_sao_relacoes_independentes() {
    let c = caso(
        "v4_572",
        "pacote main;\n\ntrato Marca {\n    carinho marcar(item: si) -> bombom { mimo 7; }\n}\n\nimpl Marca para bombom {}\nimpl Marca para u64 {}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[],
    );
    let saida = checar(&c, "572-v4");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// F1..F3 — fronteira de autoridade
// ---------------------------------------------------------------------------

/// F1 — a repetição do MESMO método dentro de um único bloco continua sendo
/// endereçada pela autoridade de método.
///
/// A cardinalidade da relação não absorve nem apaga a recusa de método
/// repetido: são duas perguntas diferentes.
#[test]
fn f1_metodo_repetido_no_mesmo_bloco_continua_com_a_autoridade_de_metodo() {
    let c = caso(
        "f1_572",
        "pacote main;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom;\n}\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 2; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[],
    );
    let saida = checar(&c, "572-f1");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    let texto = stderr(&saida);
    assert!(
        texto.contains("método 'marcar' do trato 'Marca'"),
        "método repetido é pergunta de método; veio: {texto}"
    );
    assert!(texto.contains("já implementado"), "{texto}");
    assert!(
        !texto.contains("já declarado"),
        "um bloco só não declara a relação duas vezes; veio: {texto}"
    );
}

/// F2 — blocos distintos chegam INTEIROS à autoridade de coerência.
///
/// A #571 fixou que corpos de `impl` distintos não são deduplicados por
/// conteúdo. Dois blocos byte a byte idênticos, em unidades diferentes,
/// continuam sendo duas declarações — e por isso podem ser recusados.
#[test]
fn f2_blocos_identicos_nao_sao_deduplicados_antes_da_coerencia() {
    let bloco = "impl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n";
    let c = caso(
        "f2_572",
        "pacote main;\ntrazer m572i1.usa_1;\ntrazer m572i2.usa_2;\n\ncarinho principal() -> bombom { mimo usa_1() + usa_2(); }\n",
        &[
            ("m572t", trato_com_default("m572t")),
            (
                "m572i1",
                format!("pacote m572i1;\ntrazer m572t.Marca;\n\n{bloco}\ncarinho usa_1() -> bombom {{ mimo 1; }}\n"),
            ),
            (
                "m572i2",
                format!("pacote m572i2;\ntrazer m572t.Marca;\n\n{bloco}\ncarinho usa_2() -> bombom {{ mimo 2; }}\n"),
            ),
        ],
    );
    diagnostico_de_relacao_duplicada(&checar(&c, "572-f2"), "m572t.Marca");
}

/// F3 — programa com relação duplicada não executa e não chega ao backend.
#[test]
fn f3_relacao_duplicada_nao_executa() {
    let c = caso(
        "f3_572",
        &raiz_de_um_arquivo(&format!("{BLOCO_VAZIO}{BLOCO_EXPLICITO}")),
        &[],
    );
    let execucao = executar(&c, "572-f3-run");
    assert_eq!(
        codigo(&execucao),
        1,
        "programa inválido não pode executar; saiu: {}",
        stdout(&execucao)
    );

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .logical_case("572-f3-build")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("build #572");
    assert!(
        !build.status.success(),
        "relação duplicada não pode chegar ao backend"
    );
}

// ---------------------------------------------------------------------------
// P1 — paridade interpretador/nativo do caso válido
// ---------------------------------------------------------------------------

/// P1 — o bloco vazio válido observa o mesmo resultado nos dois motores.
#[test]
fn p1_paridade_interpretador_e_nativo_do_bloco_vazio_valido() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-572-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_572",
        "pacote main;\ntrazer m572p1.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar());\n    mimo 0;\n}\n",
        &[("m572p1", trato_com_default("m572p1"))],
    );
    let interpretado = executar(&c, "572-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));
    assert_eq!(stdout(&interpretado), "11\n");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("572-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #572");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_572"))
        .logical_case("572-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #572");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
}

// @pinker-nav:end evidencia.tratos.relacao-de-impl-duplicada
