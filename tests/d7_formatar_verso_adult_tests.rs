mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.formatar-verso.d7-pack
// @pinker-nav:domain texto
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova adulta D7 de pack geral para formatar_verso: aridades 9/13 pelo mesmo ABI, tipos normalizados, placeholders/Unicode/lifetime, sensitivity contra helpers numéricos e paridade interpretador-nativo sob envelope.
const POSITIVE_SOURCE: &str = r#"
pacote main;

carinho resultado_local() -> verso {
    nova local: verso = "local";
    mimo formatar_verso("[{}:{}]", local, 99);
}

carinho principal() -> bombom {
    nova pequeno: verso = formatar_verso("{}={}", "idade", 7);
    nova oito: verso = formatar_verso("{}{}{}{}{}{}{}{}", 1, 2, 3, 4, 5, 6, 7, 8);
    nova nove: verso = formatar_verso("{}|{}|{}|{}|{}|{}|{}|{}|{}", 1, 2, 3, 4, 5, 6, 7, 8, 9);
    nova treze: verso = formatar_verso(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        "ação", 2, "três", 4, "cinco", 6, "sete", 8, "nove", 10, "onze", 12, "treze"
    );
    nova unicode: verso = formatar_verso("Olá, {} — {}", "Pínker", "🌸");
    nova eco: verso = "eco";
    nova repetido: verso = formatar_verso("{}{}", eco, eco);
    nova um: verso = formatar_verso("{}", 1);
    falar(pequeno);
    falar(oito);
    falar(nove);
    falar(treze);
    falar(unicode);
    falar(repetido);
    falar(resultado_local());
    falar(um);
    mimo 0;
}
"#;

const EXPECTED_STDOUT: &str = concat!(
    "idade=7\n",
    "12345678\n",
    "1|2|3|4|5|6|7|8|9\n",
    "ação|2|três|4|cinco|6|sete|8|nove|10|onze|12|treze\n",
    "Olá, Pínker — 🌸\n",
    "ecoeco\n",
    "[local:99]\n",
    "1\n",
);

fn run_code(code: &str) -> Result<Option<RuntimeValue>, String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())?;
    let ir = ir::lower_program(&program).map_err(|error| error.to_string())?;
    ir_validate::validate_program(&ir).map_err(|error| error.to_string())?;
    let cfg = cfg_ir::lower_program(&ir).map_err(|error| error.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|error| error.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|error| error.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|error| error.to_string())?;
    let machine =
        pinker_v0::abstract_machine::lower_program(&selected).map_err(|error| error.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|error| error.to_string())?;
    interpreter::run_program(&machine).map_err(|error| error.to_string())
}

fn emit_asm(code: &str) -> Result<String, String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())?;
    let ir = ir::lower_program(&program).map_err(|error| error.to_string())?;
    ir_validate::validate_program(&ir).map_err(|error| error.to_string())?;
    let cfg = cfg_ir::lower_program(&ir).map_err(|error| error.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|error| error.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|error| error.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|error| error.to_string())?;
    backend_s::emit_external_toolchain_subset_nativo(&selected).map_err(|error| error.to_string())
}

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar fonte D7 temporária");
    path
}

fn run_interpreter_cli(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador D7 sob envelope")
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
        .expect("compilar D7 sob envelope")
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D7 sob envelope")
}

#[test]
fn aridades_9_e_13_atravessam_o_mesmo_pack_geral() {
    assert_eq!(
        run_code(POSITIVE_SOURCE).unwrap(),
        Some(RuntimeValue::Int(0))
    );
    let asm = emit_asm(POSITIVE_SOURCE).expect("emitir assembly D7");
    assert!(asm.contains("call pinker_formatar_verso_pack"), "{asm}");
    assert!(asm.contains("movq $9, %rsi"), "{asm}");
    assert!(asm.contains("movq $13, %rsi"), "{asm}");
    assert!(asm.contains("addq $80, %rsp"), "{asm}");
    assert!(asm.contains("addq $112, %rsp"), "{asm}");
    for arity in 0..=32 {
        assert!(
            !asm.contains(&format!("call pinker_formatar_verso_{arity}")),
            "wrapper por aridade reapareceu para {arity}: {asm}"
        );
    }
}

#[test]
fn contrato_publico_preserva_aridade_minima_e_tipos_suportados() {
    let zero = common::parse_and_check(
        "pacote main; carinho principal() -> bombom { nova x: verso = formatar_verso(\"sem pack\"); mimo 0; }",
    )
    .unwrap_err()
    .to_string();
    assert!(zero.contains("esperado pelo menos 2"), "{zero}");

    for (tipo, expr) in [("logica", "falso"), ("lista", "lista_bombom_criar()")] {
        let code = format!(
            "pacote main; carinho principal() -> bombom {{ nova x: verso = formatar_verso(\"{{}}\", {expr}); mimo 0; }}"
        );
        let error = common::parse_and_check(&code).unwrap_err().to_string();
        assert!(
            error.contains("tipo inválido no argumento 2"),
            "{tipo}: {error}"
        );
    }
}

#[test]
fn placeholders_invalidos_excesso_vazio_e_unicode_preservam_contrato() {
    let cases = [
        ("insuficiente", "{} {}", "1"),
        ("excedente", "{}", "1, 2"),
        ("nomeado", "{nome}", "\"ana\""),
        ("fecha_solto", "}", "\"ana\""),
        ("template_vazio", "", "1"),
    ];
    for (name, template, args) in cases {
        let code = format!(
            "pacote main; carinho principal() -> bombom {{ nova x: verso = formatar_verso(\"{template}\", {args}); falar(x); mimo 0; }}"
        );
        let error = run_code(&code).unwrap_err();
        assert!(
            error.contains("quantidade de placeholders")
                || error.contains("modelo inválido em 'formatar_verso'"),
            "{name}: {error}"
        );
    }
    assert!(POSITIVE_SOURCE.contains("Olá, {} — {}"));
    assert!(POSITIVE_SOURCE.contains('🌸'));
}

#[test]
fn ir_preserva_tipo_normalizando_bombom_sem_reescrever_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova sete: verso = "sete";
            nova x: verso = formatar_verso("{} {}", 7, sete);
            mimo tamanho_verso(x);
        }
    "#;
    let rendered = common::render_ir(source).expect("renderizar IR D7");
    assert_eq!(
        rendered.matches("bombom_para_verso").count(),
        1,
        "{rendered}"
    );
    assert!(rendered.contains("formatar_verso"), "{rendered}");
}

fn wrappers_numericos(source: &str) -> Vec<u32> {
    let prefix = "pinker_formatar_verso_";
    source
        .match_indices(prefix)
        .filter_map(|(offset, _)| {
            let digits: String = source[offset + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            (!digits.is_empty()).then(|| digits.parse().expect("aridade decimal"))
        })
        .collect()
}

#[test]
fn sensitivity_recusa_helper_ou_dispatch_novo_por_aridade() {
    let backend = include_str!("../src/backend_s.rs");
    let runtime = include_str!("../runtime/pinker_rt/src/lib.rs");
    assert!(backend.contains("call pinker_formatar_verso_pack"));
    assert!(runtime.contains("pub unsafe extern \"C\" fn pinker_formatar_verso_pack"));
    assert!(runtime.contains("*const *const u8"));
    assert!(!backend.contains("(\"formatar_verso\", n)"));
    assert!(!backend.contains("pinker_formatar_verso_{}"));
    let wrappers = wrappers_numericos(runtime);
    assert!(!wrappers.is_empty(), "adapters ABI legados desapareceram");
    assert!(
        wrappers.iter().all(|arity| *arity <= 8),
        "novo helper numérico detectado: {wrappers:?}"
    );
    let mutation = "call pinker_formatar_verso_9; call pinker_formatar_verso_13";
    assert_eq!(wrappers_numericos(mutation), vec![9, 13]);
}

#[test]
fn paridade_interpretador_nativo_positiva_e_negativa_e_bounded() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let positive_dir = NativeArtifactDir::create().expect("diretório nativo D7 positivo");
    let positive_source = write_case(&positive_dir, "d7_format_pack_positive", POSITIVE_SOURCE);
    let interpreted = run_interpreter_cli(&positive_source, "d7-positive-interpreter");
    let build = build_native(
        &positive_dir,
        &positive_source,
        &runtime_lib,
        "d7-positive-build",
    );
    assert!(
        build.status.success(),
        "build nativo D7 falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &positive_dir.path().join("d7_format_pack_positive"),
        "d7-positive-native",
    );
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), EXPECTED_STDOUT);

    let runtime_negative = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: verso = formatar_verso("{} {}", 1);
            falar(x);
            mimo 0;
        }
    "#;
    let negative_dir = NativeArtifactDir::create().expect("diretório nativo D7 negativo");
    let negative_source = write_case(&negative_dir, "d7_format_pack_negative", runtime_negative);
    let interpreted = run_interpreter_cli(&negative_source, "d7-negative-interpreter");
    let build = build_native(
        &negative_dir,
        &negative_source,
        &runtime_lib,
        "d7-negative-build",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &negative_dir.path().join("d7_format_pack_negative"),
        "d7-negative-native",
    );
    assert_eq!(interpreted.status.code(), Some(1));
    assert_eq!(native.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&interpreted.stderr).contains("quantidade de placeholders"));
    assert!(String::from_utf8_lossy(&native.stderr).contains("quantidade de placeholders"));

    let semantic_negative =
        "pacote main; carinho principal() -> bombom { nova x: verso = formatar_verso(\"{}\", falso); mimo 0; }";
    let semantic_dir = NativeArtifactDir::create().expect("diretório semântico D7");
    let semantic_source = write_case(&semantic_dir, "d7_format_type_negative", semantic_negative);
    let interpreted = run_interpreter_cli(&semantic_source, "d7-type-interpreter");
    let native_build = build_native(
        &semantic_dir,
        &semantic_source,
        &runtime_lib,
        "d7-type-build",
    );
    assert_eq!(interpreted.status.code(), Some(1));
    assert_eq!(native_build.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&interpreted.stderr).contains("tipo inválido no argumento 2"));
    assert!(String::from_utf8_lossy(&native_build.stderr).contains("tipo inválido no argumento 2"));
}
// @pinker-nav:end evidencia.formatar-verso.d7-pack
