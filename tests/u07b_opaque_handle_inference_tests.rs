//! Evidência U-07B da decisão humana HD-02, eixo A0-1.
//!
//! Duas evidências genéricas do MESMO handle opaco nominal concordam em `I` e
//! atravessam parser, semântica, especialização monomórfica, IR e paridade
//! interpretador × ELF nativo. Handles nominalmente distintos continuam
//! recusados PELA consistência de evidências genéricas, com
//! `E-GENERIC-CONFLICTING-INFERENCE` emitido ainda na fase de parse — a
//! asserção observa a fase, não apenas o fracasso do programa, para que uma
//! regressão que empurre a recusa para a semântica ou o IR fique vermelha.
//!
//! Fecha também o representante por primeira evidência (a identidade da
//! especialização não muda ao inverter a ordem das evidências, embora a saída
//! do programa legitimamente mude), as consequências recursivas restritas a
//! compostos que contenham a MESMA folha nominal, e os controles de
//! preservação dos eixos não tocados: alias não normalizado em `I`,
//! `I(bombom,u64)` estrito e ordem de união não canonizada.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::Type;
use pinker_v0::generic_identity::{specialization_name, GenericKind, GenericOrigin};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

/// Duas evidências do mesmo handle opaco nominal, vindas de expressões e spans
/// diferentes, para o mesmo parâmetro genérico.
const MESMO_HANDLE_EXECUTAVEL: &str = r#"pacote main; trazer json.ler_resultado; trazer json.objeto_obter; trazer json.emitir;

apelido ResJson = Resultado<ValorJson, verso>;

carinho mesmo<T>(a: T, b: T) -> T { mimo a; }

carinho principal() -> bombom {
    tentar ler_resultado("{\"primeiro\": 10, \"segundo\": 20}") {
        sucesso ResJson.Ok(raiz) {
            nova alfa: ValorJson = objeto_obter(raiz, "primeiro");
            nova beta: ValorJson = objeto_obter(raiz, "segundo");
            nova escolhido: ValorJson = mesmo(alfa, beta);
            falar(emitir(escolhido));
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
        }
    }
    mimo 0;
}
"#;

/// A mesma fonte com as evidências de `T` invertidas. Nada mais muda: mesma
/// declaração genérica, mesmo template, mesma origem, mesmos dois handles.
const MESMO_HANDLE_ORDEM_INVERTIDA: &str = r#"pacote main; trazer json.ler_resultado; trazer json.objeto_obter; trazer json.emitir;

apelido ResJson = Resultado<ValorJson, verso>;

carinho mesmo<T>(a: T, b: T) -> T { mimo a; }

carinho principal() -> bombom {
    tentar ler_resultado("{\"primeiro\": 10, \"segundo\": 20}") {
        sucesso ResJson.Ok(raiz) {
            nova alfa: ValorJson = objeto_obter(raiz, "primeiro");
            nova beta: ValorJson = objeto_obter(raiz, "segundo");
            nova escolhido: ValorJson = mesmo(beta, alfa);
            falar(emitir(escolhido));
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
        }
    }
    mimo 0;
}
"#;

const HANDLES_DISTINTOS: &str = r#"
pacote main;
carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
carinho usar(x: ValorJson, y: SaidaProcesso) -> ValorJson { mimo mesmo(x, y); }
carinho principal() -> bombom { mimo 0; }
"#;

const DIAGNOSTICO_CONFLITO: &str = "E-GENERIC-CONFLICTING-INFERENCE";

fn nome_especializacao(handle: &str) -> String {
    specialization_name(
        GenericKind::Function,
        &GenericOrigin::Root,
        "mesmo",
        &[Type::OpaqueHandle {
            name: handle.to_string(),
            span: pinker_v0::falha_operacional::span_sintetico(),
        }],
    )
}

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar fonte U-07B temporária");
    path
}

fn run_interpreter(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador U-07B")
}

fn build_native(
    dir: &NativeArtifactDir,
    path: &Path,
    runtime_lib: &Path,
    logical_case: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(path)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("compilar U-07B nativo")
}

/// A0-1: o mesmo handle nominal, repetido como evidência, concorda — e o
/// programa atravessa parser, semântica, especialização e IR.
#[test]
fn mesmo_handle_nominal_repetido_concorda_e_especializa() {
    common::parse_and_check(MESMO_HANDLE_EXECUTAVEL)
        .expect("mesmo handle nominal deveria concordar");
    let ir = common::render_ir(MESMO_HANDLE_EXECUTAVEL).expect("IR do mesmo handle nominal");
    let simbolo = nome_especializacao("ValorJson");
    assert!(
        ir.contains(&simbolo),
        "especialização ausente para o handle repetido: {simbolo}\n{ir}"
    );
}

/// A recusa de handles distintos tem de acontecer NA consistência de
/// evidências genéricas. `common::parse` para antes da semântica e do IR: se
/// a recusa migrar para uma fase posterior, o parse passa e o teste fica
/// vermelho — que é exatamente o mutante M2.
#[test]
fn handles_nominais_distintos_conflitam_na_inferencia_generica() {
    let erro = common::parse(HANDLES_DISTINTOS)
        .expect_err("handles nominalmente distintos deveriam conflitar já no parse")
        .to_string();
    assert!(
        erro.contains(DIAGNOSTICO_CONFLITO),
        "recusa sem o diagnóstico da consistência de evidências: {erro}"
    );
    assert!(
        erro.contains("ValorJson") && erro.contains("SaidaProcesso"),
        "diagnóstico não nomeia as duas identidades nominais em conflito: {erro}"
    );
}

/// AXIS_3: o representante continua sendo a primeira evidência e a identidade
/// da especialização não observa a ordem. A saída do programa observa, o que é
/// legítimo — por isso a prova compara identidade, não `stdout`.
#[test]
fn ordem_das_evidencias_nao_altera_identidade_da_especializacao() {
    let ir_ab = common::render_ir(MESMO_HANDLE_EXECUTAVEL).expect("IR ordem A,B");
    let ir_ba = common::render_ir(MESMO_HANDLE_ORDEM_INVERTIDA).expect("IR ordem B,A");
    let simbolo = nome_especializacao("ValorJson");
    assert!(ir_ab.contains(&simbolo), "{ir_ab}");
    assert!(ir_ba.contains(&simbolo), "{ir_ba}");

    let coletar = |ir: &str| {
        ir.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|token| token.starts_with("__gen_"))
            .map(str::to_string)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        coletar(&ir_ab),
        coletar(&ir_ba),
        "a ordem das evidências alterou o conjunto de identidades monomórficas"
    );
}

/// P5: a folha reflexiva propaga para compostos que contenham a MESMA
/// identidade nominal, e apenas para eles. Handles distintos dentro do mesmo
/// composto continuam conflitando na inferência.
#[test]
fn compostos_seguem_a_identidade_nominal_da_folha() {
    let composto = |tipo: &str, outro: &str| {
        format!(
            r#"
pacote main;
carinho mesmo<T>(a: T, b: T) -> T {{ mimo a; }}
carinho usar(x: {tipo}, y: {outro}) -> {tipo} {{ mimo mesmo(x, y); }}
carinho principal() -> bombom {{ mimo 0; }}
"#
        )
    };

    // As cinco formas recursivas de `inference_type_eq` que alcançam uma folha
    // `OpaqueHandle` pela fonte. `mapa<verso, ValorJson>` entra aqui: a
    // concordância das evidências é o que esta unidade decide, e a
    // representação de valor de mapa continua sendo recusada depois, pela
    // política própria dela — que a U-07B não toca. Por isso a asserção
    // positiva observa o parse, onde a relação `I` vive. `lista<T>` fica de
    // fora porque a gramática não admite `lista<ValorJson>` nesta fase.
    for tipo in [
        "[ValorJson; 2]",
        "seta<ValorJson>",
        "uniao<ValorJson, bombom>",
        "mapa<verso, ValorJson>",
        "seta<carinho(ValorJson) -> bombom>",
    ] {
        common::parse(&composto(tipo, tipo)).unwrap_or_else(|err| {
            panic!("composto de mesma folha nominal deveria concordar ({tipo}): {err}")
        });
    }

    for (tipo, outro) in [
        ("[ValorJson; 2]", "[SaidaProcesso; 2]"),
        ("seta<ValorJson>", "seta<SaidaProcesso>"),
        ("uniao<ValorJson, bombom>", "uniao<SaidaProcesso, bombom>"),
        ("mapa<verso, ValorJson>", "mapa<verso, SaidaProcesso>"),
        (
            "seta<carinho(ValorJson) -> bombom>",
            "seta<carinho(SaidaProcesso) -> bombom>",
        ),
    ] {
        let erro = common::parse(&composto(tipo, outro))
            .expect_err("composto com folhas nominais distintas deveria conflitar")
            .to_string();
        assert!(
            erro.contains(DIAGNOSTICO_CONFLITO),
            "composto {tipo} vs {outro} recusado fora da consistência de evidências: {erro}"
        );
    }
}

/// HD-02 preservou AXIS_1, AXIS_2 e AXIS_4. Estes são controles de regressão
/// da decisão humana, não uma nova formalização.
#[test]
fn eixos_preservados_continuam_estritos_em_inferencia() {
    let alias = r#"
pacote main;
apelido Contador = bombom;
carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
carinho usar(x: Contador, y: bombom) -> Contador { mimo mesmo(x, y); }
carinho principal() -> bombom { mimo 0; }
"#;
    let erro = common::parse(alias)
        .expect_err("AXIS_1: alias não é normalizado para inferência")
        .to_string();
    assert!(erro.contains(DIAGNOSTICO_CONFLITO), "{erro}");

    let numerico = r#"
pacote main;
carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
carinho usar(x: bombom, y: u64) -> bombom { mimo mesmo(x, y); }
carinho principal() -> bombom { mimo 0; }
"#;
    let erro = common::parse(numerico)
        .expect_err("AXIS_2: I(bombom,u64) permanece falso")
        .to_string();
    assert!(erro.contains(DIAGNOSTICO_CONFLITO), "{erro}");

    // A mesma dupla continua compatível em M — I e M são relações diferentes.
    common::parse_and_check(
        r#"
pacote main;
carinho usar(x: bombom) -> u64 { mimo x; }
carinho principal() -> bombom { mimo 0; }
"#,
    )
    .expect("AXIS_2: M(bombom,u64) continua verdadeiro e independente de I");

    let uniao = r#"
pacote main;
carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
carinho usar(x: uniao<bombom, verso>, y: uniao<verso, bombom>) -> uniao<bombom, verso> { mimo mesmo(x, y); }
carinho principal() -> bombom { mimo 0; }
"#;
    let erro = common::parse(uniao)
        .expect_err("AXIS_4: ordem de união não é canonizada para inferência")
        .to_string();
    assert!(erro.contains(DIAGNOSTICO_CONFLITO), "{erro}");
}

/// O consumidor real, ponta a ponta: interpretador e ELF nativo produzem a
/// mesma saída para o programa que a U-07A provava recusado.
#[test]
fn paridade_interpretador_nativo_do_mesmo_handle_repetido() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório nativo U-07B");
    let fonte = write_case(&dir, "u07b_mesmo_handle", MESMO_HANDLE_EXECUTAVEL);
    let interpretado = run_interpreter(&fonte, "u07b-mesmo-handle-interpreter");
    let build = build_native(&dir, &fonte, &runtime_lib, "u07b-mesmo-handle-build");
    assert!(
        build.status.success(),
        "build nativo U-07B falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nativo = run_interpreter_free(
        &dir.path().join("u07b_mesmo_handle"),
        "u07b-mesmo-handle-native",
    );

    assert_eq!(interpretado.status.code(), Some(0));
    assert_eq!(nativo.status.code(), Some(0));
    assert_eq!(interpretado.stdout, nativo.stdout);
    assert_eq!(String::from_utf8_lossy(&nativo.stdout), "10\n");
}

fn run_interpreter_free(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF U-07B")
}
