mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.modulos.validacao-de-corpo-sintetico-de-trato
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental da #566: a cobertura de validação semântica do corpo de um `impl` não depende da unidade física que o hospeda, e a checagem do corpo default vencido por override é endereçada pela identidade canônica do trato. A matriz cobre override e default, raiz e módulo, no par de equivalência que só troca a localização; tratos homônimos em unidades distintas validados de forma independente nas duas ordens; recusa de `impl` duplicado pela autoridade de contratos de trato, e não por choque de nome sintético; regressões da #517; e paridade interpretador/nativo. O oráculo de identidade é o símbolo `__trait_default_check_<n>_<módulo>.<trato>_...` observado na IR, não a ausência de mensagem.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #566");
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

fn ir(caso: &Caso, caso_logico: &str) -> String {
    let saida = pink(caso_logico, &["--ir"], &caso.raiz);
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    stdout(&saida)
}

/// Símbolo da checagem do corpo default, como a IR o escreve depois da
/// resolução. O codec é prefixado por comprimento, então a identidade do trato
/// aparece inteira: `ma.Marca` e `mb.Marca` produzem símbolos diferentes, e uma
/// identidade que perdesse a origem produziria `__trait_default_check_5_Marca_`.
fn simbolo_de_checagem(trato_canonico: &str, alvo: &str, metodo: &str) -> String {
    format!(
        "__trait_default_check_{}_{}_{}_{}_{}",
        trato_canonico.len(),
        trato_canonico,
        alvo.len(),
        alvo,
        metodo
    )
}

/// Símbolo do método de `impl`, sob o mesmo codec prefixado por comprimento.
fn simbolo_de_metodo(trato_canonico: &str, alvo: &str, metodo: &str) -> String {
    format!(
        "__impl_{}_{}_{}_{}_{}",
        trato_canonico.len(),
        trato_canonico,
        alvo.len(),
        alvo,
        metodo
    )
}

/// O `impl` de um trato local, com override explícito, escrito por extenso.
///
/// `default` é o corpo do contrato; `override` é o corpo que vence a seleção.
/// A checagem do default continua devida nos dois lugares em que este texto
/// pode ser colocado — raiz ou módulo —, e é essa igualdade que a #566 fixa.
fn trato_com_override(pacote: &str, default: &str, corpo_override: &str) -> String {
    format!(
        "pacote {pacote};\n\n\
         trato Marca {{\n    \
             carinho marcar(valor: bombom) -> bombom {{ {default} }}\n\
         }}\n\n\
         impl Marca para bombom {{\n    \
             carinho marcar(valor: bombom) -> bombom {{ {corpo_override} }}\n\
         }}\n"
    )
}

/// Erro que a validação do corpo default produz quando ele contradiz o
/// contrato declarado.
fn diagnostico_de_default(saida: &std::process::Output, trato_canonico: &str) {
    let texto = stderr(saida);
    assert!(
        texto.contains(&simbolo_de_checagem(trato_canonico, "bombom", "marcar")),
        "o diagnóstico precisa nomear a checagem do trato canônico; veio: {texto}"
    );
    assert!(
        texto.contains("retorno incompatível"),
        "o diagnóstico precisa ser o do corpo, não o da assinatura; veio: {texto}"
    );
}

// ---------------------------------------------------------------------------
// Equivalência raiz/não-raiz — o teste central
// ---------------------------------------------------------------------------

/// O par muda UMA coisa: onde o `impl` mora. A obrigação semântica é idêntica.
///
/// Este é o detector do contrato adulto da #566: enquanto o corpo default
/// vencido por override era materializado só quando o `impl` ficava na raiz, a
/// unidade física decidia se o programa era validado.
#[test]
fn equivalencia_raiz_e_modulo_para_default_invalido() {
    let corpo = trato_com_override("main", "mimo \"ruim\";", "mimo valor + 5;");
    let raiz = caso(
        "eq_raiz_566",
        &format!("{corpo}\ncarinho principal() -> bombom {{\n    nova x: bombom = 10;\n    mimo x.marcar();\n}}\n"),
        &[],
    );
    let na_raiz = checar(&raiz, "566-equivalencia-raiz");
    assert_eq!(codigo(&na_raiz), 1, "{}", stdout(&na_raiz));
    diagnostico_de_default(&na_raiz, "Marca");

    let modulo = caso(
        "eq_modulo_566",
        "pacote main;\ntrazer m566eq.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[("m566eq", trato_com_override("m566eq", "mimo \"ruim\";", "mimo valor + 5;"))],
    );
    let no_modulo = checar(&modulo, "566-equivalencia-modulo");
    assert_eq!(
        codigo(&no_modulo),
        1,
        "o mesmo programa hospedado em módulo precisa da mesma recusa; saiu: {}",
        stdout(&no_modulo)
    );
    diagnostico_de_default(&no_modulo, "m566eq.Marca");
}

/// A mesma equivalência no sentido positivo: default válido passa nos dois
/// lugares, e nos dois a checagem é materializada sob a identidade canônica.
#[test]
fn equivalencia_raiz_e_modulo_para_default_valido() {
    let modulo = caso(
        "eqp_modulo_566",
        "pacote main;\ntrazer m566eqp.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m566eqp",
            trato_com_override("m566eqp", "mimo valor + 1;", "mimo valor + 5;"),
        )],
    );
    let saida = executar(&modulo, "566-equivalencia-positiva");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
    assert!(
        ir(&modulo, "566-equivalencia-positiva-ir").contains(&simbolo_de_checagem(
            "m566eqp.Marca",
            "bombom",
            "marcar"
        )),
        "a checagem do default do módulo precisa sobreviver à materialização"
    );
}

// ---------------------------------------------------------------------------
// Matriz positiva P1..P6
// ---------------------------------------------------------------------------

/// P1 — `impl` válido na raiz.
#[test]
fn p1_impl_valido_na_raiz() {
    let c = caso(
        "p1_566",
        &format!(
            "{}\ncarinho principal() -> bombom {{\n    nova x: bombom = 10;\n    mimo x.marcar();\n}}\n",
            trato_com_override("main", "mimo valor + 1;", "mimo valor + 5;")
        ),
        &[],
    );
    let saida = executar(&c, "566-p1");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
}

/// P2 — `impl` válido em módulo não-raiz.
#[test]
fn p2_impl_valido_em_modulo() {
    let c = caso(
        "p2_566",
        "pacote main;\ntrazer m566p2.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m566p2",
            trato_com_override("m566p2", "mimo valor + 1;", "mimo valor + 5;"),
        )],
    );
    let saida = executar(&c, "566-p2");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
}

/// P3 — `impl` sobre trato importado (#517) continua válido, e a checagem do
/// default do trato importado é endereçada pela origem canônica dele.
#[test]
fn p3_impl_sobre_trato_importado_preserva_a_origem_canonica() {
    let c = caso(
        "p3_566",
        "pacote main;\ntrazer m566p3.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 5; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m566p3",
            "pacote m566p3;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n".to_string(),
        )],
    );
    let saida = executar(&c, "566-p3");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
    let corpo = ir(&c, "566-p3-ir");
    assert!(
        corpo.contains(&simbolo_de_checagem("m566p3.Marca", "bombom", "marcar")),
        "a checagem precisa herdar a identidade canônica do trato importado"
    );
    assert!(
        !corpo.contains(&simbolo_de_checagem("Marca", "bombom", "marcar")),
        "nenhuma identidade textual pode sobreviver ao lado da canônica"
    );
}

/// P4 — dois tratos homônimos com defaults válidos distintos, em módulos
/// distintos, recebem checagem independente sob identidades distintas.
#[test]
fn p4_tratos_homonimos_validos_recebem_checagens_distintas() {
    let c = caso(
        "p4_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos_homonimos("mimo valor + 1;", "mimo valor + 2;"),
    );
    let saida = executar(&c, "566-p4");
    assert_eq!(codigo(&saida), 33, "{}", stderr(&saida));
    let corpo = ir(&c, "566-p4-ir");
    for trato in ["m566a.Marca", "m566b.Marca"] {
        assert!(
            corpo.contains(&simbolo_de_checagem(trato, "bombom", "marcar")),
            "{trato} precisa ter checagem própria; IR: {corpo}"
        );
    }
    assert!(
        !corpo.contains(&simbolo_de_checagem("Marca", "bombom", "marcar")),
        "nenhuma checagem pode ficar endereçada pela grafia textual comum"
    );
}

/// P5 — overrides explícitos válidos em módulos separados convivem.
#[test]
fn p5_overrides_explicitos_validos_em_modulos_separados() {
    let c = caso(
        "p5_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos_homonimos("mimo valor + 1;", "mimo valor + 2;"),
    );
    let corpo = ir(&c, "566-p5-ir");
    for trato in ["m566a.Marca", "m566b.Marca"] {
        assert!(
            corpo.contains(&simbolo_de_metodo(trato, "bombom", "marcar")),
            "o override de {trato} precisa continuar indexado sob a identidade canônica"
        );
    }
}

/// P6 — interpretador e nativo observam o mesmo resultado quando o `impl` e a
/// checagem do default vivem em módulo não-raiz.
#[test]
fn p6_paridade_interpretador_e_nativo_com_impl_em_modulo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-566-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_566",
        "pacote main;\ntrazer m566p6.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar());\n    mimo 0;\n}\n",
        &[(
            "m566p6",
            trato_com_override("m566p6", "mimo valor + 1;", "mimo valor + 5;"),
        )],
    );
    let interpretado = executar(&c, "566-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));
    assert_eq!(stdout(&interpretado), "15\n");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("566-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #566");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_566"))
        .logical_case("566-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #566");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
}

// ---------------------------------------------------------------------------
// Matriz negativa N1..N8
// ---------------------------------------------------------------------------

/// N1 — override explícito inválido na raiz.
#[test]
fn n1_override_invalido_na_raiz() {
    let c = caso(
        "n1_566",
        &format!(
            "{}\ncarinho principal() -> bombom {{\n    nova x: bombom = 10;\n    mimo x.marcar();\n}}\n",
            trato_com_override("main", "mimo valor + 1;", "mimo \"ruim\";")
        ),
        &[],
    );
    let saida = checar(&c, "566-n1");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(stderr(&saida).contains(&simbolo_de_metodo("Marca", "bombom", "marcar")));
}

/// N2 — o MESMO override inválido, hospedado em módulo não-raiz.
#[test]
fn n2_override_invalido_em_modulo() {
    let c = caso(
        "n2_566",
        "pacote main;\ntrazer m566n2.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m566n2",
            trato_com_override("m566n2", "mimo valor + 1;", "mimo \"ruim\";"),
        )],
    );
    let saida = checar(&c, "566-n2");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    assert!(
        stderr(&saida).contains(&simbolo_de_metodo("m566n2.Marca", "bombom", "marcar")),
        "{}",
        stderr(&saida)
    );
}

/// N3 — corpo default inválido na raiz, com override vencendo a seleção.
#[test]
fn n3_default_invalido_na_raiz() {
    let c = caso(
        "n3_566",
        &format!(
            "{}\ncarinho principal() -> bombom {{\n    nova x: bombom = 10;\n    mimo x.marcar();\n}}\n",
            trato_com_override("main", "mimo \"ruim\";", "mimo valor + 5;")
        ),
        &[],
    );
    let saida = checar(&c, "566-n3");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    diagnostico_de_default(&saida, "Marca");
}

/// N4 — o MESMO corpo default inválido, hospedado em módulo não-raiz.
///
/// Reprodutor terminal da auditoria #570: antes da #566 este programa passava
/// em `--check` e executava.
#[test]
fn n4_default_invalido_em_modulo() {
    let c = caso(
        "n4_566",
        "pacote main;\ntrazer m566n4.Marca;\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m566n4",
            trato_com_override("m566n4", "mimo \"ruim\";", "mimo valor + 5;"),
        )],
    );
    let saida = checar(&c, "566-n4");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    diagnostico_de_default(&saida, "m566n4.Marca");

    let execucao = executar(&c, "566-n4-run");
    assert_eq!(
        codigo(&execucao),
        1,
        "programa inválido não pode executar; saiu: {}",
        stdout(&execucao)
    );
}

/// N5 — primeiro homônimo válido, segundo inválido: o segundo é diagnosticado.
#[test]
fn n5_segundo_homonimo_invalido_e_diagnosticado() {
    let c = caso(
        "n5_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos_homonimos("mimo valor + 1;", "mimo \"ruim\";"),
    );
    let saida = checar(&c, "566-n5");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    diagnostico_de_default(&saida, "m566b.Marca");
    assert!(
        !stderr(&saida).contains(&simbolo_de_checagem("m566a.Marca", "bombom", "marcar")),
        "o trato válido não pode ser acusado pelo homônimo"
    );
    assert!(
        stderr(&saida).contains("m566b.pink"),
        "o span precisa apontar para a unidade que escreveu o corpo; veio: {}",
        stderr(&saida)
    );
}

/// N6 — primeiro homônimo inválido, segundo válido: o primeiro é diagnosticado.
#[test]
fn n6_primeiro_homonimo_invalido_e_diagnosticado() {
    let c = caso(
        "n6_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos_homonimos("mimo \"ruim\";", "mimo valor + 2;"),
    );
    let saida = checar(&c, "566-n6");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    diagnostico_de_default(&saida, "m566a.Marca");
    assert!(
        stderr(&saida).contains("m566a.pink"),
        "o span precisa apontar para a unidade que escreveu o corpo; veio: {}",
        stderr(&saida)
    );
}

/// N7 — `impl` duplicado da mesma relação canônica, em duas unidades, continua
/// recusado PELA autoridade de contratos de trato.
///
/// A checagem sintética não pode virar a autoridade que recusa: se ela
/// aparecesse primeiro, a mensagem passaria a falar de uma função que ninguém
/// escreveu, e a duplicata deixaria de ser nomeada como duplicata.
#[test]
fn n7_impl_duplicado_e_recusado_pela_autoridade_de_contratos() {
    let c = caso(
        "n7_566",
        "pacote main;\ntrazer m566da.a;\ntrazer m566db.b;\n\ncarinho principal() -> bombom { mimo a() + b(); }\n",
        &[
            (
                "m566t",
                "pacote m566t;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n".to_string(),
            ),
            (
                "m566da",
                "pacote m566da;\ntrazer m566t.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 10; }\n}\n\ncarinho a() -> bombom { mimo 1; }\n".to_string(),
            ),
            (
                "m566db",
                "pacote m566db;\ntrazer m566t.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 20; }\n}\n\ncarinho b() -> bombom { mimo 2; }\n".to_string(),
            ),
        ],
    );
    let saida = checar(&c, "566-n7");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    let texto = stderr(&saida);
    assert!(
        texto.contains("já implementado"),
        "a duplicata precisa ser recusada como duplicata de `impl`; veio: {texto}"
    );
    assert!(
        !texto.contains("__trait_default_check_"),
        "a checagem sintética não pode virar a autoridade nominal da recusa; veio: {texto}"
    );
}

/// N8 — sem o import da #517 o trato continua sem ser alvo legítimo de `impl`.
#[test]
fn n8_impl_sem_import_continua_recusado() {
    let c = caso(
        "n8_566",
        "pacote main;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 5; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[(
            "m566n8",
            "pacote m566n8;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n".to_string(),
        )],
    );
    let saida = checar(&c, "566-n8");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
}

/// N9 — trato e `impl` em unidades DIFERENTES, ambas não-raiz: o corpo default
/// inválido continua recusado, e o diagnóstico nomeia a unidade que DECLAROU o
/// trato, não a que hospeda o `impl`.
///
/// Na baseline este programa passava em `--check`. O caso é o que separa "o
/// corpo foi validado" de "o corpo foi validado contra o trato certo": quem
/// escreveu o `impl` é `m566h1`, quem deve o corpo é `m566h2`.
///
/// A asserção é sobre a IDENTIDADE no diagnóstico. O trecho de fonte renderizado
/// para um corpo default copiado de outra unidade ainda sai da unidade errada —
/// defeito preexistente de atribuição de `SourceId` na leitura best-effort de
/// import, observável de forma idêntica na baseline quando o `impl` mora na
/// raiz. Não é regressão desta Task e não é corrigido aqui; está reportado como
/// finding.
#[test]
fn n9_trato_e_impl_em_unidades_distintas_recusam_pelo_trato_declarante() {
    let c = caso(
        "n9_566",
        "pacote main;\ntrazer m566h1.usar;\n\ncarinho principal() -> bombom { mimo usar(); }\n",
        &[
            (
                "m566h2",
                "pacote m566h2;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo \"ruim\"; }\n}\n".to_string(),
            ),
            (
                "m566h1",
                "pacote m566h1;\ntrazer m566h2.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 5; }\n}\n\ncarinho usar() -> bombom { nova x: bombom = 1; mimo x.marcar(); }\n".to_string(),
            ),
        ],
    );
    let saida = checar(&c, "566-n9");
    assert_eq!(codigo(&saida), 1, "{}", stdout(&saida));
    diagnostico_de_default(&saida, "m566h2.Marca");

    let execucao = executar(&c, "566-n9-run");
    assert_eq!(
        codigo(&execucao),
        1,
        "programa inválido não pode executar; saiu: {}",
        stdout(&execucao)
    );
}

// ---------------------------------------------------------------------------
// Ordem
// ---------------------------------------------------------------------------

/// A ordem de import não decide qual corpo é validado.
#[test]
fn ordem_de_import_nao_muda_o_corpo_validado() {
    let modulos = modulos_homonimos("mimo valor + 1;", "mimo \"ruim\";");
    let direta = caso(
        "ord1_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos,
    );
    let invertida = caso(
        "ord2_566",
        "pacote main;\ntrazer m566b.usar_b;\ntrazer m566a.usar_a;\n\ncarinho principal() -> bombom { mimo usar_b() + usar_a(); }\n",
        &modulos,
    );
    let a = checar(&direta, "566-ordem-direta");
    let b = checar(&invertida, "566-ordem-invertida");
    assert_eq!(codigo(&a), 1, "{}", stdout(&a));
    assert_eq!(codigo(&b), 1, "{}", stdout(&b));
    diagnostico_de_default(&a, "m566b.Marca");
    diagnostico_de_default(&b, "m566b.Marca");
}

/// Dois módulos homônimos declarados em ordens opostas produzem o mesmo
/// conjunto de identidades de checagem.
#[test]
fn ordem_nao_muda_as_identidades_materializadas() {
    let modulos = modulos_homonimos("mimo valor + 1;", "mimo valor + 2;");
    let direta = caso(
        "ordi1_566",
        "pacote main;\ntrazer m566a.usar_a;\ntrazer m566b.usar_b;\n\ncarinho principal() -> bombom { mimo usar_a() + usar_b(); }\n",
        &modulos,
    );
    let invertida = caso(
        "ordi2_566",
        "pacote main;\ntrazer m566b.usar_b;\ntrazer m566a.usar_a;\n\ncarinho principal() -> bombom { mimo usar_b() + usar_a(); }\n",
        &modulos,
    );
    for corpo in [
        ir(&direta, "566-ordem-ir-direta"),
        ir(&invertida, "566-ordem-ir-invertida"),
    ] {
        for trato in ["m566a.Marca", "m566b.Marca"] {
            assert!(
                corpo.contains(&simbolo_de_checagem(trato, "bombom", "marcar")),
                "{trato} precisa ser materializado nas duas ordens"
            );
        }
    }
}

/// `ma` e `mb` declaram o MESMO nome de trato, com defaults observavelmente
/// distintos e overrides próprios. Uma identidade textual colapsaria as duas
/// checagens numa só.
fn modulos_homonimos(
    default_a: &'static str,
    default_b: &'static str,
) -> Vec<(&'static str, String)> {
    vec![
        (
            "m566a",
            format!(
                "{}\ncarinho usar_a() -> bombom {{ nova x: bombom = 1; mimo x.marcar(); }}\n",
                trato_com_override("m566a", default_a, "mimo valor + 10;")
            ),
        ),
        (
            "m566b",
            format!(
                "{}\ncarinho usar_b() -> bombom {{ nova y: bombom = 2; mimo y.marcar(); }}\n",
                trato_com_override("m566b", default_b, "mimo valor + 20;")
            ),
        ),
    ]
}

// @pinker-nav:end evidencia.modulos.validacao-de-corpo-sintetico-de-trato
