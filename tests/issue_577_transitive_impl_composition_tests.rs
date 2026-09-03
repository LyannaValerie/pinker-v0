mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.modulos.impl-transitivo-pela-unidade-importada
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental da #577: quem importa a unidade que declara um `impl` passa a poder despachar por essa relação sem reimportar o trato de origem, e sem ganhar nada mais que isso. A matriz positiva cobre a cadeia A -> B -> RAIZ com bloco vazio, com default simples, com default que fecha closure (#567), com override explícito (#566), com objeto de trato e com import explícito adicional do próprio trato; a adversarial fixa que a raiz continua sem poder nomear o trato, sem poder escrever `impl` sobre ele, que o homônimo da raiz resolve a relação DELA sem capturar nem ser capturado, que duas origens homônimas permanecem entidades distintas em vez de colidirem, que a duplicata de relação continua governada pela #572 e que a unidade não importada continua fora do despacho. O oráculo é o valor observado — cada origem devolve um número próprio — mais a identidade canônica renderizada na IR, que nomeia sempre a unidade declarante do trato e nunca a que hospeda o `impl`.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #577");
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

fn executar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--run"], &caso.raiz)
}

fn checar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--check"], &caso.raiz)
}

fn ir(caso: &Caso, caso_logico: &str) -> String {
    let saida = pink(caso_logico, &["--ir"], &caso.raiz);
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    stdout(&saida)
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

/// Símbolo do método de `impl`, sob o codec prefixado por comprimento: a
/// identidade do trato aparece inteira, então uma resolução que trocasse a
/// unidade declarante pela hospedeira produziria outro símbolo.
fn simbolo_de_impl(trato_canonico: &str, alvo: &str, metodo: &str) -> String {
    format!(
        "__impl_{}_{}_{}_{}_{}",
        trato_canonico.len(),
        trato_canonico,
        alvo.len(),
        alvo,
        metodo
    )
}

// ---------------------------------------------------------------------------
// A topologia da Issue: A declara o trato, B o importa e implementa, a RAIZ
// importa só a superfície de B.
// ---------------------------------------------------------------------------

/// Módulo A — declara o trato e nada mais.
fn origem(pacote: &str, soma: i32) -> String {
    format!(
        "pacote {pacote};\n\n\
         trato Marca {{\n    \
             carinho padrao(valor: bombom) -> bombom {{ mimo valor + {soma}; }}\n\
         }}\n"
    )
}

/// Módulo B — importa o trato de A, implementa e expõe superfície própria.
fn implementador(pacote: &str, origem_do_trato: &str, bloco: &str) -> String {
    format!(
        "pacote {pacote};\n\
         trazer {origem_do_trato}.Marca;\n\n\
         {bloco}\n\
         carinho usar() -> bombom {{\n    \
             nova x: bombom = 10;\n    \
             mimo x.padrao();\n\
         }}\n"
    )
}

const BLOCO_VAZIO: &str = "impl Marca para bombom {}\n";

/// A cadeia canônica desta Issue: `+1` na origem, `usar()` devolve `11`.
fn cadeia() -> Vec<(&'static str, String)> {
    vec![
        ("m577o", origem("m577o", 1)),
        ("m577i", implementador("m577i", "m577o", BLOCO_VAZIO)),
    ]
}

/// A raiz importa só a superfície de B e consome as duas metades dela: a função
/// exposta e o `impl` que a torna válida. `20 + 1` mais `10 + 1` é `32`.
const RAIZ_CONSOME_B: &str = "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + usar();\n}\n";

// ---------------------------------------------------------------------------
// P1 — a composição A -> B -> RAIZ
// ---------------------------------------------------------------------------

/// P1 — a raiz importa apenas a superfície de B e compõe, sem jamais nomear A.
#[test]
fn p1_raiz_que_importa_so_o_implementador_alcanca_o_impl() {
    let c = caso("p1_577", RAIZ_CONSOME_B, &cadeia());
    let saida = executar(&c, "577-p1");
    assert_eq!(codigo(&saida), 32, "{}", stderr(&saida));
    assert!(
        ir(&c, "577-p1-ir").contains(&simbolo_de_impl("m577o.Marca", "bombom", "padrao")),
        "a relação continua indexada pela unidade que DECLAROU o trato"
    );
}

/// P1, controle pareado: a superfície de B sozinha já compunha antes desta
/// Issue. Sem este controle o caso acima poderia estar verde por acidente de
/// carregamento, e não pela relação de `impl` alcançar a raiz.
#[test]
fn p1_controle_a_funcao_exposta_por_b_sempre_compos() {
    let c = caso(
        "p1c_577",
        "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom { mimo usar(); }\n",
        &cadeia(),
    );
    let saida = executar(&c, "577-p1-controle");
    assert_eq!(codigo(&saida), 11, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// P2/P3 — dependência semântica transportada não é reexport
// ---------------------------------------------------------------------------

/// P2 — a raiz não pode NOMEAR o trato de A. A dependência que ela alcança é a
/// relação, não a ligação de nome: `ModuleEnvironment` não recebeu nada.
#[test]
fn p2_raiz_nao_nomeia_o_trato_da_origem_sem_import_proprio() {
    let c = caso(
        "p2_577",
        "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo Marca.padrao(y) + usar();\n}\n",
        &cadeia(),
    );
    let saida = checar(&c, "577-p2");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("identificador 'Marca' não declarado"),
        "a grafia do trato continua fora do ambiente da raiz; veio: {}",
        stderr(&saida)
    );
}

/// P2, forma de tipo — `trato<Marca>` também exige o nome, e o nome não veio.
#[test]
fn p2_raiz_nao_nomeia_o_trato_como_tipo_de_objeto() {
    let c = caso(
        "p2t_577",
        "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    nova o: trato<Marca> = y virar trato<Marca>;\n    mimo o.padrao() + usar();\n}\n",
        &cadeia(),
    );
    let saida = checar(&c, "577-p2-tipo");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("trato 'Marca' não declarado"),
        "a recusa precisa ser a do nome ausente, não outra falha qualquer; veio: {}",
        stderr(&saida)
    );
}

/// P3 — a raiz não ganha autoridade de `impl` sobre o trato que não importou.
/// A recusa continua vindo da autoridade de import do parser, antes da
/// semântica, exatamente como a #517 a deixou.
#[test]
fn p3_raiz_nao_escreve_impl_do_trato_da_origem_sem_import_proprio() {
    let c = caso(
        "p3_577",
        "pacote main;\ntrazer m577i.usar;\n\nimpl Marca para si {}\n\ncarinho principal() -> bombom { mimo usar(); }\n",
        &cadeia(),
    );
    let saida = checar(&c, "577-p3");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains(
            "impl usa trato 'Marca' não declarado antes deste ponto nem trazido por import"
        ),
        "a permissão de `impl` continua exigindo import próprio; veio: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// P4/P5 — homônimos e origens distintas
// ---------------------------------------------------------------------------

/// P4 — um trato homônimo declarado na raiz não captura o `impl` de B, e
/// também não é capturado por ele: a chamada da raiz resolve a relação DA RAIZ
/// (`20 + 500`) enquanto `usar()` continua devolvendo a de B (`10 + 1`).
///
/// `520 + 11` é `531`, e o código de saída observa `531 % 256 = 19`. Captura em
/// qualquer das duas direções daria outro número.
#[test]
fn p4_homonimo_da_raiz_nao_captura_nem_e_capturado() {
    let c = caso(
        "p4_577",
        "pacote main;\ntrazer m577i.usar;\n\ntrato Marca {\n    carinho padrao(valor: bombom) -> bombom { mimo valor + 500; }\n}\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + usar();\n}\n",
        &cadeia(),
    );
    let saida = executar(&c, "577-p4");
    assert_eq!(codigo(&saida), 19, "{}", stderr(&saida));
    let texto = ir(&c, "577-p4-ir");
    assert!(
        texto.contains(&simbolo_de_impl("Marca", "bombom", "padrao")),
        "a relação da raiz preserva a grafia dela"
    );
    assert!(
        texto.contains(&simbolo_de_impl("m577o.Marca", "bombom", "padrao")),
        "a relação de B continua indexada pela origem, sem colidir com a da raiz"
    );
}

/// P5 — dois tratos homônimos de origens distintas permanecem entidades
/// distintas. Cada implementador devolve o número da SUA origem (`11` e `22`),
/// e a chamada não qualificada da raiz, que alcança as duas pela mesma força,
/// falha fechada em ambiguidade em vez de eleger uma em silêncio.
#[test]
fn p5_origens_homonimas_distintas_nao_colidem() {
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        ("m577o2", origem("m577o2", 12)),
        ("m577i", implementador("m577i", "m577o", BLOCO_VAZIO)),
        (
            "m577i2",
            implementador("m577i2", "m577o2", BLOCO_VAZIO).replace("usar()", "usar2()"),
        ),
    ];
    let c = caso(
        "p5_577",
        "pacote main;\ntrazer m577i.usar;\ntrazer m577i2.usar2;\n\ncarinho principal() -> bombom {\n    falar(usar());\n    falar(usar2());\n    mimo 0;\n}\n",
        &modulos,
    );
    let saida = executar(&c, "577-p5");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    assert_eq!(stdout(&saida), "11\n22\n");

    let ambigua = caso(
        "p5amb_577",
        "pacote main;\ntrazer m577i.usar;\ntrazer m577i2.usar2;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao();\n}\n",
        &modulos,
    );
    let recusa = checar(&ambigua, "577-p5-ambigua");
    assert_eq!(codigo(&recusa), 1, "{}", stdout(&recusa));
    assert!(
        stderr(&recusa).contains("é ambíguo"),
        "duas origens distintas não elegem uma vencedora; veio: {}",
        stderr(&recusa)
    );
}

// ---------------------------------------------------------------------------
// P6 — import explícito do trato continua correto e não duplica
// ---------------------------------------------------------------------------

/// P6 — a raiz que TAMBÉM importa `A.Marca` compõe pelo mesmo caminho, com a
/// mesma identidade canônica e uma só relação: o alcance por importação não
/// cria uma segunda autoridade ao lado do import explícito.
#[test]
fn p6_import_explicito_do_trato_nao_cria_segunda_autoridade() {
    let c = caso(
        "p6_577",
        "pacote main;\ntrazer m577o.Marca;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + usar();\n}\n",
        &cadeia(),
    );
    let saida = executar(&c, "577-p6");
    assert_eq!(codigo(&saida), 32, "{}", stderr(&saida));
    let simbolo = simbolo_de_impl("m577o.Marca", "bombom", "padrao");
    let texto = ir(&c, "577-p6-ir");
    assert_eq!(
        texto.matches(&format!("func {simbolo} ")).count(),
        1,
        "uma relação, uma materialização"
    );
}

// ---------------------------------------------------------------------------
// P7/P8/P9 — as três formas de corpo do método alcançado
// ---------------------------------------------------------------------------

/// P7 — bloco de `impl` vazio, satisfeito só por default, compõe pela cadeia.
/// É a forma de `cadeia()`, aqui isolada contra o par explícito abaixo.
#[test]
fn p7_bloco_vazio_composto_apenas_por_default_alcanca_a_raiz() {
    let c = caso("p7_577", RAIZ_CONSOME_B, &cadeia());
    assert_eq!(codigo(&executar(&c, "577-p7")), 32);
}

/// P8 — default importado simples: o corpo veio de A, foi materializado em B e
/// continua significando `+1` quando a raiz o alcança.
#[test]
fn p8_default_importado_simples_continua_correto() {
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        (
            "m577i",
            implementador("m577i", "m577o", BLOCO_VAZIO)
                .replace("nova x: bombom = 10;", "nova x: bombom = 40;"),
        ),
    ];
    let c = caso(
        "p8_577",
        "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    falar(usar());\n    mimo 0;\n}\n",
        &modulos,
    );
    let saida = executar(&c, "577-p8");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    assert_eq!(stdout(&saida), "41\n");
}

/// P9 — regressão obrigatória da #567: o default cujo corpo fecha closure
/// compõe também na cadeia A -> B -> RAIZ, e a closure continua significando o
/// que a unidade DECLARANTE escreveu.
///
/// `apoio_577` é o discriminante: a origem devolve `3`, e o homônimo que a raiz
/// declara devolve `900`. O valor observado é `(3 + 5 + 2)` mais `(3 + 5 + 2)`,
/// isto é `20`; captura do homônimo daria `1814`.
#[test]
fn p9_default_importado_com_closure_compoe_na_cadeia() {
    let modulos = vec![
        (
            "m577c",
            "pacote m577c;\n\ncarinho apoio_577() -> bombom { mimo 3; }\n\ntrato Marca {\n    carinho padrao(valor: si) -> bombom {\n        nova base: bombom = 5;\n        nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom { mimo apoio_577() + base + v; };\n        mimo f(2);\n    }\n}\n".to_string(),
        ),
        (
            "m577ci",
            "pacote m577ci;\ntrazer m577c.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usar() -> bombom {\n    nova x: bombom = 10;\n    mimo x.padrao();\n}\n".to_string(),
        ),
    ];
    let c = caso(
        "p9_577",
        "pacote main;\ntrazer m577ci.usar;\n\ncarinho apoio_577() -> bombom { mimo 900; }\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + usar();\n}\n",
        &modulos,
    );
    let saida = executar(&c, "577-p9");
    assert_eq!(codigo(&saida), 20, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// P10 — override explícito, e a validação dele
// ---------------------------------------------------------------------------

/// P10 — o override explícito escrito em B vence o default e alcança a raiz
/// pela mesma cadeia. `20 + 700` mais `10 + 700` é `1430`, observado como
/// `1430 % 256 = 150`.
#[test]
fn p10_override_explicito_em_b_vence_e_alcanca_a_raiz() {
    let bloco = "impl Marca para bombom {\n    carinho padrao(valor: bombom) -> bombom { mimo valor + 700; }\n}\n";
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        ("m577i", implementador("m577i", "m577o", bloco)),
    ];
    let c = caso("p10_577", RAIZ_CONSOME_B, &modulos);
    let saida = executar(&c, "577-p10");
    assert_eq!(codigo(&saida), 150, "{}", stderr(&saida));
}

/// P10, metade adversarial e regressão da #566: a validação do override NÃO
/// depende da unidade física que o hospeda. O mesmo corpo inválido escrito em
/// B, e não na raiz, continua diagnosticado.
#[test]
fn p10_override_invalido_em_b_continua_diagnosticado() {
    let bloco = "impl Marca para bombom {\n    carinho padrao(valor: bombom) -> bombom { mimo \"texto\"; }\n}\n";
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        ("m577i", implementador("m577i", "m577o", bloco)),
    ];
    let c = caso("p10n_577", RAIZ_CONSOME_B, &modulos);
    let saida = checar(&c, "577-p10-invalido");
    assert_eq!(
        codigo(&saida),
        1,
        "corpo que contradiz o contrato não pode passar por morar fora da raiz: {}",
        stdout(&saida)
    );
    assert!(
        stderr(&saida).contains("retorno incompatível")
            && stderr(&saida).contains(&simbolo_de_impl("m577o.Marca", "bombom", "padrao")),
        "a recusa precisa ser a do corpo, sob a identidade canônica da origem; veio: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// P11 — a duplicata de relação continua da #572
// ---------------------------------------------------------------------------

/// P11 — duas unidades que declaram a MESMA relação canônica continuam
/// recusadas pela autoridade da #572, e o alcance novo não é o que decide.
#[test]
fn p11_duplicata_de_relacao_continua_governada_pela_572() {
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        ("m577i", implementador("m577i", "m577o", BLOCO_VAZIO)),
        (
            "m577i2",
            implementador("m577i2", "m577o", BLOCO_VAZIO).replace("usar()", "usar2()"),
        ),
    ];
    let c = caso(
        "p11_577",
        "pacote main;\ntrazer m577i.usar;\ntrazer m577i2.usar2;\n\ncarinho principal() -> bombom { mimo usar() + usar2(); }\n",
        &modulos,
    );
    let saida = checar(&c, "577-p11");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("já declarado"),
        "a mesma relação canônica duas vezes continua duplicata; veio: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// P12 — objeto de trato
// ---------------------------------------------------------------------------

/// P12 — o despacho dinâmico dentro de B continua correto quando a raiz importa
/// só a superfície dele. `21` pelo default mais `64` pelo override é `85`, e a
/// raiz ainda alcança a relação sobre `bombom` por conta própria: `85 + 21` é
/// `106`.
#[test]
fn p12_objeto_de_trato_em_b_continua_correto_pela_cadeia() {
    let modulos = vec![
        (
            "m577o",
            "pacote m577o;\n\ntrato Marca {\n    carinho padrao(valor: si) -> bombom { mimo 21; }\n}\n"
                .to_string(),
        ),
        (
            "m577ob",
            "pacote m577ob;\ntrazer m577o.Marca;\n\nimpl Marca para bombom {}\n\nimpl Marca para u64 {\n    carinho padrao(valor: u64) -> bombom { mimo 64; }\n}\n\ncarinho despachar() -> bombom {\n    nova a: bombom = 20;\n    nova b: u64 = 5;\n    nova oa: trato<Marca> = a virar trato<Marca>;\n    nova ob: trato<Marca> = b virar trato<Marca>;\n    mimo oa.padrao() + ob.padrao();\n}\n".to_string(),
        ),
    ];
    let c = caso(
        "p12_577",
        "pacote main;\ntrazer m577ob.despachar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 30;\n    mimo despachar() + y.padrao();\n}\n",
        &modulos,
    );
    let saida = executar(&c, "577-p12");
    assert_eq!(codigo(&saida), 106, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// P13 — o que continua falhando fechado
// ---------------------------------------------------------------------------

/// P13 — o alcance é da unidade IMPORTADA, não do grafo. A raiz que importa uma
/// ponte, e não o implementador, continua sem despachar pela relação dele:
/// transportar a dependência uma indireção adiante seria reexport.
#[test]
fn p13_unidade_nao_importada_continua_fora_do_despacho() {
    let modulos = vec![
        ("m577o", origem("m577o", 1)),
        ("m577i", implementador("m577i", "m577o", BLOCO_VAZIO)),
        (
            "m577p",
            "pacote m577p;\ntrazer m577i.usar;\n\ncarinho ponte() -> bombom { mimo usar(); }\n"
                .to_string(),
        ),
    ];
    let c = caso(
        "p13_577",
        "pacote main;\ntrazer m577p.ponte;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + ponte();\n}\n",
        &modulos,
    );
    let saida = checar(&c, "577-p13");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("método 'padrao' não implementado para tipo 'bombom'"),
        "a raiz não importou quem implementa; veio: {}",
        stderr(&saida)
    );
}

/// P13, e a fronteira exata do alcance: o que a unidade importada transporta são
/// as relações QUE ELA DECLAROU, não todas as relações do trato dela.
///
/// `ma577` declara o trato; `md577` o importa e declara `impl Marca para u64`;
/// `mi577` o importa, declara `impl Marca para bombom` e importa um símbolo
/// qualquer de `md577`. A raiz importa só `mi577.usar`: ela alcança a relação
/// sobre `bombom`, que `mi577` declarou, e continua sem alcançar a relação sobre
/// `u64`, que quem declarou foi uma unidade que a raiz nunca pediu.
#[test]
fn p13_alcance_e_da_relacao_da_unidade_importada_nao_do_trato() {
    let modulos = vec![
        (
            "ma577",
            "pacote ma577;\n\ntrato Marca {\n    carinho padrao(valor: si) -> bombom { mimo 21; }\n}\n".to_string(),
        ),
        (
            "md577",
            "pacote md577;\ntrazer ma577.Marca;\n\nimpl Marca para u64 {\n    carinho padrao(valor: u64) -> bombom { mimo 64; }\n}\n\ncarinho dd() -> bombom { mimo 1; }\n".to_string(),
        ),
        (
            "mi577",
            "pacote mi577;\ntrazer ma577.Marca;\ntrazer md577.dd;\n\nimpl Marca para bombom {}\n\ncarinho usar() -> bombom {\n    nova x: bombom = 10;\n    mimo x.padrao() + dd();\n}\n".to_string(),
        ),
    ];
    let vaza = caso(
        "p13d_577",
        "pacote main;\ntrazer mi577.usar;\n\ncarinho principal() -> bombom {\n    nova z: u64 = 5;\n    mimo z.padrao() + usar();\n}\n",
        &modulos,
    );
    let recusa = checar(&vaza, "577-p13-relacao-alheia");
    assert_eq!(codigo(&recusa), 1, "{}", stdout(&recusa));
    assert!(
        stderr(&recusa).contains("método 'padrao' não implementado para tipo 'u64'"),
        "a relação de md577 não é da superfície que a raiz importou; veio: {}",
        stderr(&recusa)
    );

    // Controle pareado, na mesma composição: a relação que `mi577` DECLAROU
    // continua alcançando a raiz. Sem ele a recusa acima poderia ser apenas o
    // alcance inteiro tendo desaparecido.
    let compoe = caso(
        "p13dc_577",
        "pacote main;\ntrazer mi577.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 30;\n    mimo y.padrao() + usar();\n}\n",
        &modulos,
    );
    let saida = executar(&compoe, "577-p13-relacao-propria");
    assert_eq!(codigo(&saida), 43, "{}", stderr(&saida));
}

/// A mesma fronteira quando quem declara a outra relação é a PRÓPRIA unidade
/// que declarou o trato: a raiz importa só o implementador e continua sem
/// alcançar o que a unidade do trato implementou por conta própria.
#[test]
fn p13_relacao_da_unidade_do_trato_nao_alcanca_quem_so_importou_o_implementador() {
    let modulos = vec![
        (
            "ma6577",
            "pacote ma6577;\n\ntrato Marca {\n    carinho padrao(valor: si) -> bombom { mimo 21; }\n}\n\nimpl Marca para u64 {\n    carinho padrao(valor: u64) -> bombom { mimo 64; }\n}\n".to_string(),
        ),
        (
            "mb6577",
            "pacote mb6577;\ntrazer ma6577.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usar() -> bombom {\n    nova x: bombom = 10;\n    mimo x.padrao();\n}\n".to_string(),
        ),
    ];
    let c = caso(
        "p13e_577",
        "pacote main;\ntrazer mb6577.usar;\n\ncarinho principal() -> bombom {\n    nova z: u64 = 5;\n    mimo z.padrao() + usar();\n}\n",
        &modulos,
    );
    let saida = checar(&c, "577-p13-relacao-da-origem");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("método 'padrao' não implementado para tipo 'u64'"),
        "importar o implementador não é importar o declarante; veio: {}",
        stderr(&saida)
    );
}

/// P13, segunda metade: sem relação nenhuma no programa, a cobrança continua
/// sendo a de cobertura de contrato — o alcance novo não inventa um `impl`.
#[test]
fn p13_ausencia_de_impl_continua_cobrada_pela_cobertura() {
    let c = caso(
        "p13b_577",
        "pacote main;\ntrazer m577o.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao();\n}\n",
        &[(
            "m577o",
            "pacote m577o;\n\ntrato Marca {\n    carinho padrao(valor: bombom) -> bombom;\n}\n"
                .to_string(),
        )],
    );
    let saida = checar(&c, "577-p13-cobertura");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("não implementa método 'padrao'"),
        "ausência de método é cobertura, não despacho; veio: {}",
        stderr(&saida)
    );
}

/// P13, terceira metade: um módulo continua sem receber método default de um
/// trato declarado na RAIZ que ele nunca importou. É a não-interferência que o
/// índice de despacho existia para provar, e ela não podia afrouxar.
#[test]
fn p13_modulo_nao_recebe_trato_declarado_na_raiz() {
    let c = caso(
        "p13c_577",
        "pacote main;\ntrazer m577n.usa;\n\ntrato Marca {\n    carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom { mimo usa(); }\n",
        &[(
            "m577n",
            "pacote m577n;\n\ncarinho usa() -> bombom {\n    nova x: bombom = 10;\n    mimo x.padrao();\n}\n".to_string(),
        )],
    );
    let saida = checar(&c, "577-p13-interferencia");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains("método 'padrao' não implementado para tipo 'bombom'"),
        "MODULE_IMPORTER_NON_INTERFERENCE continua valendo; veio: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// P14 — paridade interpretador/nativo
// ---------------------------------------------------------------------------

/// P14 — o mesmo programa da cadeia observa o mesmo resultado no interpretador
/// e no ELF nativo.
#[test]
fn p14_paridade_interpretador_e_nativo_da_cadeia() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-577-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_577",
        "pacote main;\ntrazer m577i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    falar(y.padrao());\n    falar(usar());\n    mimo 0;\n}\n",
        &cadeia(),
    );
    let interpretado = executar(&c, "577-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));
    assert_eq!(stdout(&interpretado), "21\n11\n");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("577-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #577");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_577"))
        .logical_case("577-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #577");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
}
// @pinker-nav:end evidencia.modulos.impl-transitivo-pela-unidade-importada
