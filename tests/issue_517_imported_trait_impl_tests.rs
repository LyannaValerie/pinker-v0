mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.modulos.impl-sobre-trato-importado
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental da #517: um trato explicitamente importado é alvo legítimo de `impl` sem que a grafia passe a decidir identidade. A matriz positiva cobre import seletivo e inteiro, `impl` dentro de módulo, default herdado, override explícito, objeto de trato e paridade interpretador/nativo; a negativa fixa ausência de import, captura por homônimo local e irmão, estado ambíguo de import, reexport implícito nas duas grafias, `impl` duplicado e as recusas de contrato que já existiam. O oráculo de identidade é o símbolo canônico `__impl_<n>_<módulo>.<trato>_...` observado na IR, não a ausência de mensagem de erro: ele distingue `a.Marca` de `b.Marca` e de uma resolução por texto puro.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #517");
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

/// Símbolo do método de `impl` como a IR o escreve depois da resolução.
///
/// O codec é prefixado por comprimento, então a identidade do trato aparece
/// inteira: `a.Marca` e `b.Marca` produzem símbolos diferentes, e uma resolução
/// que perdesse a origem produziria `__impl_5_Marca_...`. É este símbolo, e não
/// a ausência de erro, que separa "o `impl` foi aceito" de "o `impl` foi aceito
/// pelo trato certo".
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

/// `a` e `b` declaram o MESMO nome de trato com defaults observavelmente
/// distintos: +1 contra +2. Um `impl` que resolvesse por texto não teria como
/// escolher, e escolher errado muda o valor de saída.
fn modulos_homonimos() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "m517_a",
            "pacote m517_a;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n",
        ),
        (
            "m517_b",
            "pacote m517_b;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 2; }\n}\n\ncarinho enfeite_b() -> bombom { mimo 0; }\n",
        ),
    ]
}

// ---------------------------------------------------------------------------
// P1/P2 — import explícito torna o trato alvo legítimo, e a chamada chega lá
// ---------------------------------------------------------------------------

/// P1 + P2: `trazer m.T;` habilita `impl T`, e o método chamado é o do `impl`.
#[test]
fn p1_import_seletivo_habilita_impl_e_a_chamada_alcanca_o_metodo() {
    let c = caso(
        "p1_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 20; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "p1-517-seletivo");
    assert_eq!(codigo(&saida), 30, "{}", stderr(&saida));
    assert!(
        ir(&c, "p1-517-ir").contains(&simbolo_de_impl("m517_a.Marca", "bombom", "marcar")),
        "o método precisa ser indexado sob a identidade canônica do trato de origem"
    );
}

/// P1 na forma inteira: `trazer m;` autoriza a mesma superfície.
#[test]
fn p1_import_inteiro_habilita_impl_sobre_trato_do_modulo() {
    let c = caso(
        "p1i_517",
        "pacote main;\ntrazer m517_a;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 20; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "p1-517-inteiro");
    assert_eq!(codigo(&saida), 30, "{}", stderr(&saida));
    assert!(
        ir(&c, "p1i-517-ir").contains(&simbolo_de_impl("m517_a.Marca", "bombom", "marcar")),
        "import inteiro resolve para a mesma identidade canônica do seletivo"
    );
}

/// P1 dentro de um módulo: a autoridade é a mesma fora da raiz.
#[test]
fn p1_modulo_tambem_pode_implementar_trato_que_importou() {
    let c = caso(
        "p1m_517",
        "pacote main;\ntrazer m517_impl.usar;\n\ncarinho principal() -> bombom { mimo usar(); }\n",
        &[
            (
                "m517_a",
                "pacote m517_a;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n",
            ),
            (
                "m517_impl",
                "pacote m517_impl;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 20; }\n}\n\ncarinho usar() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
            ),
        ],
    );
    let saida = executar(&c, "p1-517-modulo");
    assert_eq!(codigo(&saida), 30, "{}", stderr(&saida));
    assert!(
        ir(&c, "p1m-517-ir").contains(&simbolo_de_impl("m517_a.Marca", "bombom", "marcar")),
        "o `impl` de um módulo sobre trato importado usa a identidade da origem"
    );
}

// ---------------------------------------------------------------------------
// P3 — identidade canônica sobrevive a homônimo em outra unidade
// ---------------------------------------------------------------------------

/// P3 — `m517_b` está carregado e declara `Marca`, mas o import explícito é o de
/// `m517_a`. O default herdado é o oráculo: +1 é de `a`, +2 seria de `b`.
#[test]
fn p3_homonimo_irmao_carregado_nao_sequestra_o_impl() {
    let c = caso(
        "p3_517",
        "pacote main;\ntrazer m517_a.Marca;\ntrazer m517_b.enfeite_b;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar() + enfeite_b();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "p3-517-irmao");
    assert_eq!(
        codigo(&saida),
        11,
        "o default executado tem de ser o de m517_a (+1), não o de m517_b (+2): {}",
        stderr(&saida)
    );
    let texto = ir(&c, "p3-517-ir");
    assert!(
        texto.contains(&simbolo_de_impl("m517_a.Marca", "bombom", "marcar")),
        "{texto}"
    );
    assert!(
        !texto.contains(&simbolo_de_impl("m517_b.Marca", "bombom", "marcar")),
        "o homônimo irmão não pode aparecer como alvo do impl: {texto}"
    );
}

// ---------------------------------------------------------------------------
// D12 sob import — default herdado e override explícito
// ---------------------------------------------------------------------------

/// Default do trato importado continua sendo herdado pelo `impl` vazio.
#[test]
fn default_do_trato_importado_e_herdado() {
    let c = caso(
        "d1_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "d12-517-herdado");
    assert_eq!(codigo(&saida), 11, "{}", stderr(&saida));
}

/// Override explícito vence o default do trato importado.
#[test]
fn override_explicito_vence_o_default_do_trato_importado() {
    let c = caso(
        "d2_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 100; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "d12-517-override");
    assert_eq!(codigo(&saida), 110, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// Corpo default do trato importado — pertence à unidade que o declarou
// ---------------------------------------------------------------------------

/// `m517_aux` declara o auxiliar `apoio_517` e o usa no corpo default do trato.
/// O importador NÃO importou `apoio_517`, e não precisa: o corpo é do módulo.
fn modulos_com_auxiliar() -> Vec<(&'static str, &'static str)> {
    vec![(
        "m517_aux",
        "pacote m517_aux;\n\ncarinho apoio_517() -> bombom { mimo 7; }\n\ntrato ComApoio {\n    carinho apoiar(valor: bombom) -> bombom { mimo valor + apoio_517(); }\n}\n",
    )]
}

/// O default importado alcança o auxiliar do próprio módulo, sem que o
/// importador tenha de importá-lo — e sem que ele vaze para o importador.
#[test]
fn corpo_default_importado_resolve_no_modulo_de_origem() {
    let c = caso(
        "cd1_517",
        "pacote main;\ntrazer m517_aux.ComApoio;\n\nimpl ComApoio para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.apoiar();\n}\n",
        &modulos_com_auxiliar(),
    );
    let saida = executar(&c, "cd1-517-origem");
    assert_eq!(codigo(&saida), 17, "{}", stderr(&saida));
}

/// O caso que a #517 tornou alcançável e que precisa continuar fechado: a raiz
/// declara um homônimo do auxiliar do módulo. A raiz preserva grafia, então
/// resolver o corpo default contra o ambiente do importador o capturaria em
/// silêncio — 510 em vez de 17, sem erro nenhum.
#[test]
fn homonimo_da_raiz_nao_captura_o_corpo_default_do_trato_importado() {
    let c = caso(
        "cd2_517",
        "pacote main;\ntrazer m517_aux.ComApoio;\n\ncarinho apoio_517() -> bombom { mimo 500; }\n\nimpl ComApoio para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.apoiar();\n}\n",
        &modulos_com_auxiliar(),
    );
    let saida = executar(&c, "cd2-517-homonimo-de-auxiliar");
    assert_eq!(
        codigo(&saida),
        17,
        "o corpo default tem de alcançar m517_aux.apoio_517 (7), nunca o homônimo da raiz (500): {}",
        stderr(&saida)
    );
}

/// Com override explícito o corpo default continua sendo checado, e continua
/// sendo checado contra a unidade que o escreveu — o override é que executa.
#[test]
fn override_explicito_nao_reabre_a_captura_do_corpo_default() {
    let c = caso(
        "cd3_517",
        "pacote main;\ntrazer m517_aux.ComApoio;\n\ncarinho apoio_517() -> bombom { mimo 500; }\n\nimpl ComApoio para bombom {\n    carinho apoiar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.apoiar();\n}\n",
        &modulos_com_auxiliar(),
    );
    let saida = executar(&c, "cd3-517-override");
    assert_eq!(codigo(&saida), 11, "{}", stderr(&saida));
}

/// Limitação corrente registrada: o parser materializa o default na unidade do
/// `impl`, mas os templates de closure sintética vivem na unidade que fez o
/// parse. Um default importado que declare closure falha FECHADO, com
/// diagnóstico próprio — nunca em silêncio e nunca ligado à unidade errada.
#[test]
fn default_importado_com_closure_sintetica_falha_fechado() {
    let c = caso(
        "cd4_517",
        "pacote main;\ntrazer m517_cl.ComClosure;\n\nimpl ComClosure para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.calcular();\n}\n",
        &[(
            "m517_cl",
            "pacote m517_cl;\n\ncarinho apoio_cl() -> bombom { mimo 7; }\n\ntrato ComClosure {\n    carinho calcular(valor: bombom) -> bombom {\n        nova f: carinho() -> bombom = carinho () -> bombom { mimo apoio_cl(); };\n        mimo valor + f();\n    }\n}\n",
        )],
    );
    let saida = checar(&c, "cd4-517-closure");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("closure sintética") && erro.contains("do default não foi encontrada"),
        "{erro}"
    );
}

// ---------------------------------------------------------------------------
// Objeto de trato — a maquinaria existente enxerga a identidade importada
// ---------------------------------------------------------------------------

/// `trato<T>` sobre trato importado: dois tipos concretos, um tipo nominal de
/// objeto, despacho dinâmico pelo `impl` de cada um.
#[test]
fn objeto_de_trato_importado_despacha_pela_identidade_canonica() {
    let c = caso(
        "to_517",
        "pacote main;\ntrazer m517_med.Medivel;\n\nimpl Medivel para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\nimpl Medivel para u64 {\n    carinho medir(valor: u64) -> bombom { mimo 64; }\n}\n\ncarinho principal() -> bombom {\n    nova a: bombom = 20;\n    nova b: u64 = 5;\n    nova oa: trato<Medivel> = a virar trato<Medivel>;\n    nova ob: trato<Medivel> = b virar trato<Medivel>;\n    mimo oa.medir() + ob.medir();\n}\n",
        &[(
            "m517_med",
            "pacote m517_med;\n\ntrato Medivel {\n    carinho medir(valor: si) -> bombom;\n}\n",
        )],
    );
    let saida = executar(&c, "to-517-objeto");
    assert_eq!(codigo(&saida), 85, "{}", stderr(&saida));
    let texto = ir(&c, "to-517-ir");
    assert!(
        texto.contains(&simbolo_de_impl("m517_med.Medivel", "bombom", "medir")),
        "{texto}"
    );
    assert!(
        texto.contains(&simbolo_de_impl("m517_med.Medivel", "u64", "medir")),
        "{texto}"
    );
}

// ---------------------------------------------------------------------------
// P4/P5 — paridade interpretador/nativo
// ---------------------------------------------------------------------------

/// P5 — o mesmo programa com `impl` sobre trato importado observa o mesmo
/// resultado no interpretador e no ELF nativo.
#[test]
fn paridade_interpretador_e_nativo_de_impl_sobre_trato_importado() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-517-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_517",
        "pacote main;\ntrazer m517_a.Marca;\ntrazer m517_b.enfeite_b;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar() + enfeite_b());\n    mimo 0;\n}\n",
        &modulos_homonimos(),
    );
    let interpretado = executar(&c, "issue-517-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));
    assert_eq!(stdout(&interpretado), "11\n");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("issue-517-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #517");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_517"))
        .logical_case("issue-517-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #517");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
    assert_eq!(String::from_utf8_lossy(&nativo.stdout), "11\n");
}

// ---------------------------------------------------------------------------
// N1 — sem import não há alvo
// ---------------------------------------------------------------------------

/// N1 — o módulo existe ao lado e declara o trato, mas ninguém o importou.
#[test]
fn n1_trato_externo_sem_import_continua_recusado() {
    let c = caso(
        "n1_517",
        "pacote main;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n1-517-sem-import");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("impl usa trato 'Marca'"), "{erro}");
}

/// N1 — importar OUTRO símbolo do módulo não autoriza o trato dele.
#[test]
fn n1_import_de_outro_simbolo_do_modulo_nao_autoriza_o_trato() {
    let c = caso(
        "n1b_517",
        "pacote main;\ntrazer m517_b.enfeite_b;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor; }\n}\n\ncarinho principal() -> bombom { mimo enfeite_b(); }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n1-517-outro-simbolo");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("impl usa trato 'Marca'"), "{erro}");
}

// ---------------------------------------------------------------------------
// N2 — homônimo local não captura o trato importado
// ---------------------------------------------------------------------------

/// N2 — declarar `Marca` localmente e importar `m517_a.Marca` é o estado que a
/// política de import já recusa; a #517 não abre exceção para ele.
#[test]
fn n2_homonimo_local_com_import_permanece_colisao() {
    let c = caso(
        "n2_517",
        "pacote main;\ntrazer m517_a.Marca;\n\ntrato Marca { carinho marcar(valor: bombom) -> bombom { mimo valor + 5; } }\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n2-517-homonimo-local");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("colisão de nome no import: 'Marca' já existe no arquivo principal"),
        "{erro}"
    );
}

/// N2 — sem import, o trato local homônimo continua sendo o alvo, e a
/// identidade permanece a da raiz.
#[test]
fn n2_trato_local_sem_import_continua_sendo_o_alvo() {
    let c = caso(
        "n2b_517",
        "pacote main;\ntrazer m517_b.enfeite_b;\n\ntrato Marca { carinho marcar(valor: bombom) -> bombom { mimo valor + 5; } }\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar() + enfeite_b();\n}\n",
        &modulos_homonimos(),
    );
    let saida = executar(&c, "n2b-517-local-vence");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
    assert!(
        ir(&c, "n2b-517-ir").contains(&simbolo_de_impl("Marca", "bombom", "marcar")),
        "a raiz preserva a grafia da própria declaração"
    );
}

// ---------------------------------------------------------------------------
// N3 — estado ambíguo de import
// ---------------------------------------------------------------------------

/// N3 — dois tratos homônimos importados de módulos distintos continuam sendo
/// um diagnóstico explícito, nunca uma escolha arbitrária.
#[test]
fn n3_import_ambiguo_de_tratos_homonimos_e_recusado() {
    let c = caso(
        "n3_517",
        "pacote main;\ntrazer m517_a.Marca;\ntrazer m517_b.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n3-517-ambiguo");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("colisão de nome no import: 'Marca' trazido por múltiplos módulos"),
        "{erro}"
    );
}

/// N3 — a recusa não depende da ordem em que os dois imports foram escritos.
#[test]
fn n3_import_ambiguo_e_recusado_na_ordem_inversa() {
    let c = caso(
        "n3b_517",
        "pacote main;\ntrazer m517_b.Marca;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n3b-517-ambiguo-inverso");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("colisão de nome no import: 'Marca' trazido por múltiplos módulos"),
        "{erro}"
    );
}

// ---------------------------------------------------------------------------
// N4/N5 — as recusas de contrato que já existiam continuam valendo
// ---------------------------------------------------------------------------

/// N4 — método faltante do trato importado continua recusado, com o span
/// apontando a assinatura na fonte que a declarou.
#[test]
fn n4_impl_incompleto_de_trato_importado_continua_recusado() {
    let c = caso(
        "n4_517",
        "pacote main;\ntrazer m517_par.Par;\n\nimpl Par para bombom {\n    carinho um(valor: bombom) -> bombom { mimo valor; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &[(
            "m517_par",
            "pacote m517_par;\n\ntrato Par {\n    carinho um(valor: bombom) -> bombom;\n    carinho dois(valor: bombom) -> bombom;\n}\n",
        )],
    );
    let saida = checar(&c, "n4-517-incompleto");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("impl 'm517_par.Par' para 'bombom' não implementa método 'dois'"),
        "{erro}"
    );
    assert!(erro.contains("m517_par.pink"), "{erro}");
}

/// N4 — método extra continua recusado, e a mensagem nomeia a identidade
/// canônica do trato, não a grafia importada.
#[test]
fn n4_metodo_extra_em_trato_importado_continua_recusado() {
    let c = caso(
        "n4b_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor; }\n    carinho sobra(valor: bombom) -> bombom { mimo valor; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n4b-517-metodo-extra");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains(
            "impl 'm517_a.Marca' para 'bombom' declara método 'sobra' que não existe no trato"
        ),
        "{erro}"
    );
}

/// N4 — receiver de tipo errado continua recusado sob import.
#[test]
fn n4_receiver_incompativel_em_trato_importado_continua_recusado() {
    let c = caso(
        "n4c_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para u32 {\n    carinho marcar(valor: bombom) -> bombom { mimo valor; }\n}\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n4c-517-receiver");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("receiver do método 'marcar'"), "{erro}");
}

/// N5 — `impl` duplicado do mesmo trato importado para o mesmo tipo continua
/// recusado, e a identidade citada é a canônica.
#[test]
fn n5_impl_duplicado_de_trato_importado_continua_recusado() {
    let c = caso(
        "n5_517",
        "pacote main;\ntrazer m517_a.Marca;\n\nimpl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo valor + 1; } }\nimpl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo valor + 2; } }\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "n5-517-duplicado");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("método 'marcar' do trato 'm517_a.Marca' para tipo 'bombom' já implementado"),
        "{erro}"
    );
}

// ---------------------------------------------------------------------------
// N6 — nenhum reexport implícito nasce da #517
// ---------------------------------------------------------------------------

/// N6 — `m517_ponte` importa `m517_a.Marca`; quem importa `m517_ponte` INTEIRO
/// não ganha o trato.
#[test]
fn n6_import_inteiro_de_modulo_intermediario_nao_reexporta_o_trato() {
    let c = caso(
        "n6_517",
        "pacote main;\ntrazer m517_ponte;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar() + ponte();\n}\n",
        &modulos_ponte(),
    );
    let saida = checar(&c, "n6-517-inteiro");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("impl usa trato 'Marca'"), "{erro}");
}

/// N6 — nem a forma seletiva `trazer m517_ponte.Marca;` inventa o reexport: o
/// símbolo simplesmente não pertence à superfície da ponte.
#[test]
fn n6_import_seletivo_atraves_de_modulo_intermediario_nao_reexporta_o_trato() {
    let c = caso(
        "n6b_517",
        "pacote main;\ntrazer m517_ponte.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &modulos_ponte(),
    );
    let saida = checar(&c, "n6b-517-seletivo");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("'Marca'"), "{erro}");
}

/// N6 — controle: sem o `impl`, o mesmo import seletivo pela ponte é recusado
/// pela autoridade de import, o que prova que a recusa acima não é um efeito
/// colateral do `impl`.
#[test]
fn n6_controle_import_seletivo_pela_ponte_ja_e_recusado_sem_impl() {
    let c = caso(
        "n6c_517",
        "pacote main;\ntrazer m517_ponte.Marca;\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_ponte(),
    );
    let saida = checar(&c, "n6c-517-controle");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("símbolo 'Marca' não encontrado no módulo 'm517_ponte'"),
        "{erro}"
    );
}

fn modulos_ponte() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "m517_a",
            "pacote m517_a;\n\ntrato Marca {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n",
        ),
        (
            "m517_ponte",
            "pacote m517_ponte;\ntrazer m517_a.Marca;\n\ncarinho ponte() -> bombom { mimo 7; }\n",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Ordem textual — comportamento corrente, registrado e não ampliado
// ---------------------------------------------------------------------------

/// A gramática da Pinker exige `trazer` no topo, antes dos itens. A #517 não
/// mexe nisso: o `impl` não passa a depender da ordem textual em relação ao
/// import porque a ordem já é fixa pela gramática, e o diagnóstico continua
/// sendo o da declaração fora de lugar — não o do trato ausente.
#[test]
fn ordem_textual_import_antes_dos_itens_e_regra_de_gramatica_preexistente() {
    let c = caso(
        "ord_517",
        "pacote main;\n\nimpl Marca para bombom {}\n\ntrazer m517_a.Marca;\n\ncarinho principal() -> bombom { mimo 0; }\n",
        &modulos_homonimos(),
    );
    let saida = checar(&c, "ordem-517");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("declaração `trazer` apenas no topo do programa"),
        "{erro}"
    );
}
// @pinker-nav:end evidencia.modulos.impl-sobre-trato-importado
