//! U3 / F-07 — autoridade explícita da superfície pública de intrínsecas.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::abstract_machine_validate;
use pinker_v0::ast::Item;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::error::PinkerError;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use pinker_v0::semantic::SemanticChecker;
use std::fs;

fn run_code(code: &str) -> Result<Option<RuntimeValue>, String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())?;
    let program_ir = ir::lower_program(&program).map_err(|error| error.to_string())?;
    ir_validate::validate_program(&program_ir).map_err(|error| error.to_string())?;
    let cfg = cfg_ir::lower_program(&program_ir).map_err(|error| error.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|error| error.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|error| error.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|error| error.to_string())?;
    let machine =
        pinker_v0::abstract_machine::lower_program(&selected).map_err(|error| error.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|error| error.to_string())?;
    interpreter::run_program(&machine).map_err(|error| error.to_string())
}

fn declaration(name: &str, parameter_type: &str, body: &str, called: bool) -> String {
    let call = if called {
        format!("mimo {name}(7);")
    } else {
        "mimo 0;".to_string()
    };
    format!(
        "pacote main;\ncarinho {name}(valor: {parameter_type}) -> bombom {{ {body} }}\ncarinho principal() -> bombom {{ {call} }}\n"
    )
}

fn rejection_at_declaration(code: &str, name: &str) -> String {
    let program = common::parse(code).expect("programa sintaticamente válido");
    let declaration_span = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == name => Some(function.span),
            _ => None,
        })
        .expect("declaração sob teste");
    match semantic::check_program(&program).expect_err("homônimo deveria ser rejeitado") {
        PinkerError::Semantic { msg, span } => {
            assert_eq!(
                span, declaration_span,
                "diagnóstico precisa apontar a declaração"
            );
            assert!(msg.contains(name), "{msg}");
            // Depois da #505 há duas causas de recusa, e elas NÃO são
            // intercambiáveis: um `||` aqui aceitaria a mensagem errada para o
            // caso errado. A causa é escolhida pelo próprio arquivo — se ele
            // traz o membro homônimo, a recusa é a do import; senão, é a da
            // grafia canônica reservada.
            let traz_o_membro = code.contains(&format!("trazer ambiente.{name};"))
                || code.contains(&format!("trazer arquivo.{name};"))
                || code.contains(&format!("trazer texto.{name};"));
            if traz_o_membro {
                assert!(msg.contains("colide com o membro"), "{msg}");
            } else {
                assert!(
                    msg.contains("é a grafia canônica da superfície intrínseca Pinker"),
                    "{msg}"
                );
                assert!(msg.contains("não pode ser redeclarada"), "{msg}");
            }
            assert!(!msg.contains("pinker_"), "{msg}");
            assert!(!msg.contains("runtime"), "{msg}");
            msg
        }
        error => panic!("classe diagnóstica inesperada: {error}"),
    }
}

fn direct_rejection_at_declaration(code: &str, name: &str) -> String {
    let program = common::parse(code).expect("programa sintaticamente válido");
    let declaration_span = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == name => Some(function.span),
            _ => None,
        })
        .expect("declaração sob teste");
    match SemanticChecker::new()
        .check_program(&program)
        .expect_err("entrypoint público direto deve rejeitar o homônimo")
    {
        PinkerError::Semantic { msg, span } => {
            assert_eq!(span, declaration_span);
            assert!(msg.contains(name), "{msg}");
            assert!(
                msg.contains("é a grafia canônica da superfície intrínseca Pinker"),
                "{msg}"
            );
            assert!(msg.contains("não pode ser redeclarada"), "{msg}");
            msg
        }
        error => panic!("classe diagnóstica inesperada: {error}"),
    }
}

fn verdict_via_free_entrypoint(code: &str) -> Result<(), String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())
}

fn verdict_via_direct_entrypoint(code: &str) -> Result<(), String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    SemanticChecker::new()
        .check_program(&program)
        .map_err(|error| error.to_string())
}

#[test]
fn t_a1_direct_checker_rejects_historical_builtin_homonym() {
    let source = declaration("tamanho_verso", "verso", "mimo 777;", false);
    direct_rejection_at_declaration(&source, "tamanho_verso");
}

#[test]
fn t_a2_direct_checker_rejects_modern_intrinsic_homonym() {
    let source = declaration("ler_arquivo_resultado", "verso", "mimo 777;", false);
    direct_rejection_at_declaration(&source, "ler_arquivo_resultado");
}

#[test]
fn t_a3_t_a4_direct_checker_rejects_uncalled_and_called_before_dispatch() {
    let uncalled = declaration("tamanho_verso", "bombom", "mimo valor;", false);
    let called = declaration("tamanho_verso", "bombom", "mimo valor;", true);
    assert_eq!(
        direct_rejection_at_declaration(&uncalled, "tamanho_verso"),
        direct_rejection_at_declaration(&called, "tamanho_verso")
    );
}

#[test]
fn t_a5_direct_checker_accepts_ordinary_callable_and_valid_builtin_call() {
    let ordinary = r#"
pacote main;
carinho minha_funcao_normal(valor: bombom) -> bombom { mimo valor + 1; }
carinho principal() -> bombom { mimo minha_funcao_normal(41); }
"#;
    verdict_via_direct_entrypoint(ordinary).expect("callable ordinária deve permanecer válida");

    let builtin = "pacote main; trazer texto.tamanho; carinho principal() -> bombom { mimo tamanho(\"rosa\"); }";
    verdict_via_direct_entrypoint(builtin).expect("builtin válido deve permanecer válido");
}

#[test]
fn t_a6_free_and_direct_entrypoints_have_equivalent_f07_verdicts() {
    let cases = [
        declaration("tamanho_verso", "verso", "mimo 777;", false),
        declaration("ler_arquivo_resultado", "verso", "mimo 777;", true),
        "pacote main; carinho comum(valor: bombom) -> bombom { mimo valor; } carinho principal() -> bombom { mimo comum(7); }".to_string(),
        "pacote main; trazer texto.tamanho; carinho principal() -> bombom { mimo tamanho(\"rosa\"); }".to_string(),
    ];
    for source in cases {
        assert_eq!(
            verdict_via_free_entrypoint(&source),
            verdict_via_direct_entrypoint(&source),
            "entrypoints divergiram para:\n{source}"
        );
    }
}

#[test]
fn t1_t2_historical_matching_and_incompatible_reject_the_declaration() {
    let matching = declaration("tamanho_verso", "verso", "mimo 777;", true);
    let incompatible = declaration("tamanho_verso", "bombom", "mimo valor;", true);
    let matching_error = rejection_at_declaration(&matching, "tamanho_verso");
    let incompatible_error = rejection_at_declaration(&incompatible, "tamanho_verso");
    assert_eq!(matching_error, incompatible_error);
    assert!(!incompatible_error.contains("tipo inválido no argumento"));
}

#[test]
fn t3_t6_modern_json_sha_and_table_driven_classes_are_rejected() {
    for name in [
        "ler_arquivo_resultado",
        "ler_json_resultado",
        "sha256_arquivo",
        "tipo_de_entrada",
    ] {
        let source = declaration(name, "verso", "mimo 777;", false);
        rejection_at_declaration(&source, name);
    }
}

#[test]
fn t7_ordinary_user_function_remains_callable() {
    let source = r#"
pacote main;
carinho minha_funcao_normal(valor: bombom) -> bombom { mimo valor + 1; }
carinho principal() -> bombom { mimo minha_funcao_normal(41); }
"#;
    assert_eq!(run_code(source), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t8_t9_uncalled_and_called_homonyms_fail_before_call_analysis() {
    let uncalled = declaration("tamanho_verso", "bombom", "mimo valor;", false);
    let called = declaration("tamanho_verso", "bombom", "mimo valor;", true);
    assert_eq!(
        rejection_at_declaration(&uncalled, "tamanho_verso"),
        rejection_at_declaration(&called, "tamanho_verso")
    );
}

#[test]
fn t10_t11_active_family_alias_and_canonical_spelling_share_the_policy() {
    let alias = r#"
pacote main;
trazer arquivo.ler_bombom;
carinho ler_bombom(valor: verso) -> bombom { mimo 7; }
carinho principal() -> bombom { mimo 0; }
"#;
    rejection_at_declaration(alias, "ler_bombom");
    let canonical = declaration("ler_arquivo", "verso", "mimo 7;", false);
    rejection_at_declaration(&canonical, "ler_arquivo");

    let inactive_alias = r#"
pacote main;
carinho criar(valor: bombom) -> bombom { mimo valor + 1; }
carinho principal() -> bombom { mimo criar(41); }
"#;
    assert_eq!(run_code(inactive_alias), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t12_local_value_and_callable_in_distinct_lexical_namespace_remain_allowed() {
    let local_value = r#"
pacote main;
carinho principal() -> bombom {
    nova tamanho_verso: bombom = 41;
    mimo tamanho_verso + 1;
}
"#;
    let local_callable = r#"
pacote main;
carinho principal() -> bombom {
    nova tamanho_verso = carinho(valor: bombom) -> bombom { mimo valor + 1; };
    mimo tamanho_verso(41);
}
"#;
    assert_eq!(run_code(local_value), Ok(Some(RuntimeValue::Int(42))));
    assert_eq!(run_code(local_callable), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t13_method_spelling_in_distinct_namespace_remains_allowed() {
    let source = r#"
pacote main;
trato Mede { carinho tamanho_verso(valor: si) -> bombom; }
impl Mede para bombom {
    carinho tamanho_verso(valor: bombom) -> bombom { mimo valor + 1; }
}
carinho principal() -> bombom { mimo 41.tamanho_verso(); }
"#;
    assert_eq!(run_code(source), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t14_t20_imported_callable_and_import_order_receive_the_same_policy() {
    let dir = NativeArtifactDir::create().expect("sandbox de import");
    fs::write(
        dir.path().join("util.pink"),
        "pacote util; carinho tamanho_verso(valor: bombom) -> bombom { mimo valor; }",
    )
    .expect("módulo homônimo");
    fs::write(
        dir.path().join("apoio.pink"),
        "pacote apoio; carinho marcador() -> bombom { mimo 0; }",
    )
    .expect("módulo neutro");

    for (label, imports) in [
        (
            "homonimo-antes",
            "trazer util.tamanho_verso;\ntrazer apoio.marcador;",
        ),
        (
            "homonimo-depois",
            "trazer apoio.marcador;\ntrazer util.tamanho_verso;",
        ),
    ] {
        let root = dir.path().join(format!("{label}.pink"));
        fs::write(
            &root,
            format!("pacote main;\n{imports}\ncarinho principal() -> bombom {{ mimo 0; }}"),
        )
        .expect("raiz");
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--check")
            .arg(&root)
            .logical_case(label)
            .output()
            .expect("checagem modular");
        assert!(!output.status.success(), "{label} foi aceito");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("tamanho_verso"), "{diagnostic}");
        assert!(
            diagnostic.contains("superfície intrínseca Pinker"),
            "{diagnostic}"
        );
    }
}

#[test]
fn t15_valid_builtin_call_keeps_its_meaning() {
    let source = r#"
pacote main;
trazer texto.tamanho;
carinho principal() -> bombom { mimo tamanho("oi"); }
"#;
    assert_eq!(run_code(source), Ok(Some(RuntimeValue::Int(2))));
}

#[test]
fn t16_valid_builtin_interpreter_and_native_remain_in_parity() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let dir = NativeArtifactDir::create().expect("sandbox nativo");
    let source = dir.path().join("builtin_valido.pink");
    fs::write(
        &source,
        "pacote main; trazer texto.tamanho; carinho principal() -> bombom { mimo tamanho(\"oi\"); }",
    )
    .expect("fonte");
    let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&source)
        .logical_case("u3-f07-t16-interpreter")
        .output()
        .expect("interpretador");
    assert_eq!(interpreted.status.code(), Some(2), "{interpreted:?}");

    let out_dir = dir.path().join("native");
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&source)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("u3-f07-t16-native-build")
        .output()
        .expect("build nativo");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(out_dir.join("builtin_valido"))
        .logical_case("u3-f07-t16-native-run")
        .output()
        .expect("execução nativa");
    assert_eq!(interpreted.status.code(), native.status.code());
}

#[test]
fn t17_t18_rejected_homonym_never_produces_native_artifact_and_diagnostic_is_public() {
    let dir = NativeArtifactDir::create().expect("sandbox de rejeição nativa");
    let source = dir.path().join("rejeitado.pink");
    fs::write(
        &source,
        declaration("tamanho_verso", "verso", "mimo 777;", true),
    )
    .expect("fonte");
    let out_dir = dir.path().join("native");
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&source)
        .logical_case("u3-f07-t17-rejected-native")
        .output()
        .expect("build rejeitado");
    assert!(!build.status.success());
    let diagnostic = String::from_utf8_lossy(&build.stderr);
    assert!(diagnostic.contains("tamanho_verso"), "{diagnostic}");
    assert!(
        diagnostic.contains("superfície intrínseca Pinker"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("pinker_verso_tamanho"), "{diagnostic}");
    assert!(!out_dir.join("rejeitado").exists());
    assert!(!out_dir.join("rejeitado.s").exists());
}

#[test]
fn t19_source_order_cannot_change_the_verdict() {
    let homonym = "carinho tamanho_verso(valor: bombom) -> bombom { mimo valor; }";
    let principal = "carinho principal() -> bombom { mimo 0; }";
    for source in [
        format!("pacote main;\n{homonym}\n{principal}"),
        format!("pacote main;\n{principal}\n{homonym}"),
    ] {
        rejection_at_declaration(&source, "tamanho_verso");
    }
}
