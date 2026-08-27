mod common;

// @pinker-nav:start evidencia.genericos.inferencia-local-d8
// @pinker-nav:domain genericos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência D8 da inferência genérica local: cobre id e funções nominalmente distintas, dois parâmetros de tipo, conflito e ausência de fonte, compatibilidade explícita, nesting lista<T>, chamadas não genéricas, sensitivity contra special-case nominal e paridade interpretador/nativo com diagnóstico pré-backend.

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::Type;
use pinker_v0::generic_identity::{specialization_name, GenericKind, GenericOrigin};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

const CORE_SOURCE: &str = r#"
pacote main; trazer texto.igual;

carinho id<T>(valor: T) -> T {
    mimo valor;
}

carinho par<A, B>(primeiro: A, segundo: B) -> B {
    mimo segundo;
}

carinho mesmo<T>(esquerda: T, direita: T) -> T {
    mimo esquerda;
}

carinho principal() -> bombom {
    nova a: bombom = id(42);
    nova b: verso = par(1, "ok");
    nova c: bombom = mesmo(20, 22);
    talvez a == 42 && igual(b, "ok") && c == 20 {
        mimo 0;
    }
    mimo 1;
}
"#;

const NESTED_SOURCE: &str = r#"
pacote main; trazer lista.anexar; trazer lista.criar; trazer lista.obter; trazer texto.igual;

carinho primeiro<T>(itens: lista<T>) -> T {
    mimo obter(itens, 0);
}

carinho principal() -> bombom {
    nova numeros: lista<bombom> = criar();
    anexar(numeros, 42);
    nova textos: lista<verso> = criar();
    anexar(textos, "ok");
    nova numero: bombom = primeiro(numeros);
    nova texto: verso = primeiro(textos);
    talvez numero == 42 && igual(texto, "ok") {
        mimo 0;
    }
    mimo 1;
}
"#;

const PARITY_SOURCE: &str = r#"
pacote main;

carinho identidade<T>(valor: T) -> T {
    mimo valor;
}

carinho principal() -> bombom {
    nova resultado: bombom = identidade(42);
    falar(resultado);
    mimo 0;
}
"#;

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar fonte D8 temporária");
    path
}

fn run_interpreter(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador D8")
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
        .timeout(Duration::from_secs(30))
        .output()
        .expect("compilar D8 nativo")
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D8")
}

fn generic_function_name(name: &str, type_args: Vec<Type>) -> String {
    specialization_name(
        GenericKind::Function,
        &GenericOrigin::Root,
        name,
        &type_args,
    )
}

#[test]
fn infere_id_segunda_funcao_dois_parametros_e_mesmo_tipo() {
    common::parse_and_check(CORE_SOURCE).expect("inferência D8 central");
    let ir = common::render_ir(CORE_SOURCE).expect("IR D8 central");
    let span = pinker_v0::falha_operacional::span_sintetico();
    for symbol in [
        generic_function_name("id", vec![Type::Bombom(span)]),
        generic_function_name("par", vec![Type::Bombom(span), Type::Verso(span)]),
        generic_function_name("mesmo", vec![Type::Bombom(span)]),
    ] {
        assert!(
            ir.contains(&symbol),
            "especialização ausente: {symbol}\n{ir}"
        );
    }
}

#[test]
fn infere_estrutura_aninhada_lista_sem_tabela_nominal() {
    common::parse_and_check(NESTED_SOURCE).expect("inferência estrutural lista<T>");
    let ir = common::render_ir(NESTED_SOURCE).expect("IR lista<T>");
    let span = pinker_v0::falha_operacional::span_sintetico();
    assert!(
        ir.contains(&generic_function_name("primeiro", vec![Type::Bombom(span)])),
        "{ir}"
    );
    assert!(
        ir.contains(&generic_function_name("primeiro", vec![Type::Verso(span)])),
        "{ir}"
    );

    let parser = include_str!("../src/parser.rs");
    for nominal in ["\"id\"", "\"par\"", "\"mesmo\"", "\"primeiro\""] {
        assert!(
            !parser.contains(nominal),
            "inferência contém special-case nominal {nominal}"
        );
    }
}

#[test]
fn diagnostica_conflito_e_ausencia_de_fonte_distintamente() {
    let conflict = r#"
        pacote main;
        carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
        carinho principal() -> bombom { mimo mesmo(1, "x"); }
    "#;
    let err = common::parse_and_check(conflict)
        .expect_err("conflito deveria falhar")
        .to_string();
    assert!(err.contains("E-GENERIC-CONFLICTING-INFERENCE"), "{err}");

    let no_source = r#"
        pacote main;
        carinho obter<T>() -> T { mimo 0 virar T; }
        carinho principal() -> bombom { mimo obter(); }
    "#;
    let err = common::parse(no_source)
        .expect_err("ausência de fonte deveria falhar")
        .to_string();
    assert!(err.contains("E-GENERIC-NO-INFERENCE-SOURCE"), "{err}");

    let unused_param = r#"
        pacote main;
        carinho fabricar<T>(valor: bombom) -> T { mimo valor virar T; }
        carinho principal() -> bombom { mimo fabricar(1); }
    "#;
    let err = common::parse(unused_param)
        .expect_err("parâmetro sem posição inferível deveria falhar")
        .to_string();
    assert!(err.contains("E-GENERIC-NO-INFERENCE-SOURCE"), "{err}");
}

#[test]
fn genericos_explicitos_continuam_escape_hatch_com_type_safety() {
    let explicit = r#"
        pacote main;
        carinho id<T>(valor: T) -> T { mimo valor; }
        carinho principal() -> bombom { mimo id<bombom>(42); }
    "#;
    common::parse_and_check(explicit).expect("genérico explícito compatível");

    let mismatch = r#"
        pacote main;
        carinho id<T>(valor: T) -> T { mimo valor; }
        carinho principal() -> bombom { mimo id<bombom>("x"); }
    "#;
    let err = common::parse_and_check(mismatch)
        .expect_err("genérico explícito incompatível deveria falhar")
        .to_string();
    assert!(err.contains("tipo inválido no argumento"), "{err}");
}

#[test]
fn chamadas_nao_genericas_permanecem_inalteradas() {
    let source = r#"
        pacote main;
        carinho soma(a: bombom, b: bombom) -> bombom { mimo a + b; }
        carinho principal() -> bombom { mimo soma(20, 22); }
    "#;
    common::parse_and_check(source).expect("chamada não genérica");
    let ir = common::render_ir(source).expect("IR não genérica");
    assert!(ir.contains("call soma("), "{ir}");
    assert!(!ir.contains("__gen_soma"), "{ir}");
}

#[test]
fn paridade_interpretador_nativo_e_diagnostico_pre_backend() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório nativo D8");
    let source = write_case(&dir, "d8_local_generic_inference", PARITY_SOURCE);
    let interpreted = run_interpreter(&source, "d8-positive-interpreter");
    let build = build_native(&dir, &source, &runtime_lib, "d8-positive-build");
    assert!(
        build.status.success(),
        "build nativo D8 falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &dir.path().join("d8_local_generic_inference"),
        "d8-positive-native",
    );
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), "42\n");

    let negative = r#"
        pacote main;
        carinho mesmo<T>(a: T, b: T) -> T { mimo a; }
        carinho principal() -> bombom { mimo mesmo(1, "x"); }
    "#;
    let negative_dir = NativeArtifactDir::create().expect("diretório negativo D8");
    let negative_source = write_case(&negative_dir, "d8_conflict", negative);
    let interpreted = run_interpreter(&negative_source, "d8-negative-interpreter");
    let native_build = build_native(
        &negative_dir,
        &negative_source,
        &runtime_lib,
        "d8-negative-build",
    );
    assert_eq!(interpreted.status.code(), Some(1));
    assert_eq!(native_build.status.code(), Some(1));
    for stderr in [&interpreted.stderr, &native_build.stderr] {
        assert!(
            String::from_utf8_lossy(stderr).contains("E-GENERIC-CONFLICTING-INFERENCE"),
            "{}",
            String::from_utf8_lossy(stderr)
        );
    }
}

// @pinker-nav:end evidencia.genericos.inferencia-local-d8
