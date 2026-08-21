mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::anonymous_identity::{
    anonymous_callable_identity_bytes, anonymous_callable_name, ANONYMOUS_CALLABLE_PREFIX,
};
use pinker_v0::source_origin::SourceOrigin;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

static U1_TEST_LOCK: Mutex<()> = Mutex::new(());

// @pinker-nav:start evidencia.identidades.anonima-proveniencia
// @pinker-nav:domain identidade
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental e matricial de F-03: closures de origens distintas não colidem, a ordem de import não participa da identidade, import seletivo transporta somente dependências anônimas alcançáveis e interpreter/native permanecem equivalentes.
fn serial() -> MutexGuard<'static, ()> {
    U1_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

const ALFA: &str = r#"
pacote alfa;

carinho alfa_aplica(x: bombom) -> bombom {
    nova valor: bombom = carinho(v: bombom) -> bombom {
        mimo v + 1;
    }(x);
    mimo valor;
}

carinho alfa_publica_nao_solicitada() -> bombom {
    mimo 99;
}

carinho alfa_nao_relacionada(x: bombom) -> bombom {
    nova valor: bombom = carinho(v: bombom) -> bombom {
        mimo v * 3;
    }(x);
    mimo valor;
}

carinho alfa_aninhada(base: bombom) -> bombom {
    nova fabrica: carinho() -> carinho() -> bombom = carinho() -> carinho() -> bombom {
        mimo carinho() -> bombom {
            mimo base + 2;
        };
    };
    nova interna: carinho() -> bombom = fabrica();
    mimo interna();
}
"#;

const BETA: &str = r#"
pacote beta;

carinho beta_aplica(x: bombom) -> bombom {
    nova valor: bombom = carinho(v: bombom) -> bombom {
        mimo v * 2;
    }(x);
    mimo valor;
}
"#;

const SIMPLES: &str = r#"
pacote simples;

carinho soma2(x: bombom) -> bombom {
    mimo x + 2;
}
"#;

fn write_source(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(format!("{name}.pink"));
    fs::write(&path, source).unwrap_or_else(|error| panic!("gravar {}: {error}", path.display()));
    path
}

fn module_fixture() -> NativeArtifactDir {
    let dir = NativeArtifactDir::create().expect("diretório marcado U1");
    write_source(dir.path(), "alfa", ALFA);
    write_source(dir.path(), "beta", BETA);
    write_source(dir.path(), "simples", SIMPLES);
    dir
}

fn run_cli(path: &Path, logical_case: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar interpretador U1")
}

fn check_cli(path: &Path, logical_case: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar check U1")
}

fn ir_cli(path: &Path, logical_case: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--ir")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("emitir IR U1")
}

#[test]
fn t1_t2_t5_t10_t12_modulos_distintos_executam_independentes_da_ordem() {
    let _guard = serial();
    let dir = module_fixture();
    let normal = write_source(
        dir.path(),
        "normal",
        r#"
pacote main;
trazer alfa;
trazer beta;
carinho principal() -> bombom {
    falar(alfa_aplica(20));
    falar(beta_aplica(20));
    mimo 0;
}
"#,
    );
    let reversed = write_source(
        dir.path(),
        "reversed",
        r#"
pacote main;
trazer beta;
trazer alfa;
carinho principal() -> bombom {
    falar(alfa_aplica(20));
    falar(beta_aplica(20));
    mimo 0;
}
"#,
    );

    let normal_output = run_cli(&normal, "u1-t1-normal");
    let reversed_output = run_cli(&reversed, "u1-t2-reversed");
    assert!(normal_output.status.success(), "{:?}", normal_output);
    assert!(reversed_output.status.success(), "{:?}", reversed_output);
    assert_eq!(String::from_utf8_lossy(&normal_output.stdout), "21\n40\n");
    assert_eq!(normal_output.stdout, reversed_output.stdout);
    assert!(normal_output.stderr.is_empty());
    assert!(reversed_output.stderr.is_empty());
}

#[test]
fn t3_root_e_modulo_possuem_identidades_distintas() {
    let _guard = serial();
    let dir = module_fixture();
    let root = write_source(
        dir.path(),
        "root_module",
        r#"
pacote main;
trazer alfa;
carinho principal() -> bombom {
    nova raiz: bombom = carinho(v: bombom) -> bombom { mimo v + 2; }(20);
    falar(raiz);
    falar(alfa_aplica(20));
    mimo 0;
}
"#,
    );
    let output = run_cli(&root, "u1-t3-root-module");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "22\n21\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn t4_multiplas_closures_no_mesmo_arquivo_continuam_distintas() {
    let _guard = serial();
    let dir = NativeArtifactDir::create().expect("diretório marcado U1 T4");
    let root = write_source(
        dir.path(),
        "same_file",
        r#"
pacote main;
carinho principal() -> bombom {
    nova a: bombom = carinho(v: bombom) -> bombom { mimo v + 1; }(20);
    nova b: bombom = carinho(v: bombom) -> bombom { mimo v * 2; }(20);
    falar(a);
    falar(b);
    mimo 0;
}
"#,
    );
    let output = run_cli(&root, "u1-t4-same-file");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "21\n40\n");
}

#[test]
fn t6_t7_import_seletivo_simples_e_com_closure_executam() {
    let _guard = serial();
    let dir = module_fixture();
    let simple = write_source(
        dir.path(),
        "select_simple",
        r#"
pacote main;
trazer simples.soma2;
carinho principal() -> bombom { falar(soma2(40)); mimo 0; }
"#,
    );
    let closure = write_source(
        dir.path(),
        "select_closure",
        r#"
pacote main;
trazer alfa.alfa_aplica;
carinho principal() -> bombom { falar(alfa_aplica(20)); mimo 0; }
"#,
    );

    let simple_output = run_cli(&simple, "u1-t6-select-simple");
    let closure_output = run_cli(&closure, "u1-t7-select-closure");
    assert!(simple_output.status.success(), "{:?}", simple_output);
    assert!(closure_output.status.success(), "{:?}", closure_output);
    assert_eq!(String::from_utf8_lossy(&simple_output.stdout), "42\n");
    assert_eq!(String::from_utf8_lossy(&closure_output.stdout), "21\n");
}

#[test]
fn t7_import_seletivo_transporta_dependencias_anonimas_transitivas() {
    let _guard = serial();
    let dir = module_fixture();
    let selected = write_source(
        dir.path(),
        "select_nested_closure",
        r#"
pacote main;
trazer alfa.alfa_aninhada;
carinho principal() -> bombom { falar(alfa_aninhada(40)); mimo 0; }
"#,
    );

    let output = run_cli(&selected, "u1-t7-select-nested-closure");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");

    let ir = ir_cli(&selected, "u1-t7-select-nested-closure-ir");
    assert!(ir.status.success(), "{:?}", ir);
    let rendered = String::from_utf8_lossy(&ir.stdout);
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.trim_start().starts_with("func __anon_carinho_"))
            .count(),
        2,
        "IR:\n{rendered}"
    );
    assert!(
        !rendered.contains("func alfa_nao_relacionada"),
        "IR:\n{rendered}"
    );
}

#[test]
fn t8_t9_import_seletivo_nao_traz_publico_nem_closure_nao_relacionados() {
    let _guard = serial();
    let dir = module_fixture();
    let selected = write_source(
        dir.path(),
        "selected_only",
        r#"
pacote main;
trazer alfa.alfa_aplica;
carinho principal() -> bombom { falar(alfa_aplica(20)); mimo 0; }
"#,
    );
    let illegal = write_source(
        dir.path(),
        "unrequested_public",
        r#"
pacote main;
trazer alfa.alfa_aplica;
carinho principal() -> bombom { mimo alfa_publica_nao_solicitada(); }
"#,
    );

    let ir = ir_cli(&selected, "u1-t9-reachable-generated-only");
    assert!(ir.status.success(), "{:?}", ir);
    let rendered = String::from_utf8_lossy(&ir.stdout);
    let anonymous_definitions = rendered
        .lines()
        .filter(|line| line.trim_start().starts_with("func __anon_carinho_"))
        .count();
    assert_eq!(anonymous_definitions, 1, "IR:\n{rendered}");
    assert!(
        !rendered.contains("func alfa_nao_relacionada"),
        "IR:\n{rendered}"
    );

    let rejected = check_cli(&illegal, "u1-t8-unrequested-public");
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        stderr.contains("função 'alfa_publica_nao_solicitada' não declarada"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains(ANONYMOUS_CALLABLE_PREFIX),
        "stderr: {stderr}"
    );
}

#[test]
fn t11_interpretador_e_nativo_concordam() {
    let _guard = serial();
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("u1-t11-native-parity", true)
    else {
        return;
    };
    let dir = module_fixture();
    let root = write_source(
        dir.path(),
        "native_parity",
        r#"
pacote main;
trazer alfa.alfa_aplica;
carinho principal() -> bombom { falar(alfa_aplica(20)); mimo 0; }
"#,
    );
    let interpreted = run_cli(&root, "u1-t11-interpreter");
    assert!(interpreted.status.success(), "{:?}", interpreted);

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(&root)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("u1-t11-native-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo U1");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(dir.path().join("native_parity"))
        .logical_case("u1-t11-native-run")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo U1");
    assert!(native.status.success(), "{:?}", native);
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), "21\n");
}

#[test]
fn t13_import_duplicado_preserva_contrato_atual() {
    let _guard = serial();
    let dir = module_fixture();
    let root = write_source(
        dir.path(),
        "duplicate",
        r#"
pacote main;
trazer alfa.alfa_aplica;
trazer alfa.alfa_aplica;
carinho principal() -> bombom { mimo 0; }
"#,
    );
    let output = check_cli(&root, "u1-t13-duplicate");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("import duplicado para 'alfa.alfa_aplica'"));
    assert!(!stderr.contains(ANONYMOUS_CALLABLE_PREFIX));
}

#[test]
fn t14_diagnostico_de_closure_nao_expoe_identidade_gerada() {
    let _guard = serial();
    let dir = module_fixture();
    fs::write(
        dir.path().join("diag.pink"),
        r#"
pacote diag;
carinho invalida() -> bombom {
    nova f: carinho() -> bombom = carinho() -> bombom {
        nova x: bombom = 1;
    };
    mimo f();
}
"#,
    )
    .unwrap();
    let root = write_source(
        dir.path(),
        "diagnostic_redaction",
        r#"
pacote main;
trazer diag.invalida;
carinho principal() -> bombom { mimo invalida(); }
"#,
    );

    let output = check_cli(&root, "u1-t14-diagnostic-redaction-check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("função '<anônima>'"), "stderr: {stderr}");
    assert!(
        !stderr.contains(ANONYMOUS_CALLABLE_PREFIX),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("70696e6b65722d616e6f6e796d6f75732d63616c6c61626c652d7631"),
        "stderr: {stderr}"
    );
}

#[test]
fn t5_t12_property_matrix_is_injective_and_order_free() {
    let _guard = serial();
    let origins = [
        SourceOrigin::Builtin,
        SourceOrigin::Root,
        SourceOrigin::module("a"),
        SourceOrigin::module("A"),
        SourceOrigin::module("a_b"),
        SourceOrigin::module("a1"),
        SourceOrigin::module("mod_builtin"),
        SourceOrigin::module("módulo"),
        SourceOrigin::module("á"),
    ];
    let mut names = BTreeSet::new();
    for origin in &origins {
        for local_index in 0..=32 {
            assert!(names.insert(anonymous_callable_name(origin, local_index)));
        }
    }
    assert_eq!(names.len(), origins.len() * 33);

    assert_ne!(
        anonymous_callable_name(&SourceOrigin::module("a_1"), 2),
        anonymous_callable_name(&SourceOrigin::module("a"), 12)
    );
    assert_ne!(
        anonymous_callable_name(&SourceOrigin::Root, 1),
        anonymous_callable_name(&SourceOrigin::module("Root"), 1)
    );
    assert_ne!(
        anonymous_callable_identity_bytes(&SourceOrigin::module("alfa"), 1),
        anonymous_callable_identity_bytes(&SourceOrigin::module("beta"), 1)
    );

    let mut import_order_a = vec![
        anonymous_callable_name(&SourceOrigin::module("alfa"), 1),
        anonymous_callable_name(&SourceOrigin::module("beta"), 1),
    ];
    let mut import_order_b = import_order_a.iter().cloned().rev().collect::<Vec<_>>();
    import_order_a.sort();
    import_order_b.sort();
    assert_eq!(import_order_a, import_order_b);
}
// @pinker-nav:end evidencia.identidades.anonima-proveniencia
