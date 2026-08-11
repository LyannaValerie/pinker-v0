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

// @pinker-nav:start evidencia.fatiar-verso.d9-unicode
// @pinker-nav:domain texto
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova adulta D9 de fatiar_verso por Unicode scalar values no intervalo [start,end): cobre ASCII, Unicode multibyte, combining mark, emoji, vazios, bounds, overflow, sensitivity contra byte offset cru, lifetime e paridade interpretador-nativo sob envelope.
const POSITIVE_SOURCE: &str = r#"
pacote main;

carinho recorte_local() -> verso {
    nova local: verso = "prefixo🌸sufixo";
    mimo fatiar_verso(local, 7, 8);
}

carinho principal() -> bombom {
    falar(fatiar_verso("rosa", 0, 1));
    falar(fatiar_verso("rosa", 1, 3));
    falar(fatiar_verso("rosa", 3, 4));
    falar(fatiar_verso("rosa", 0, 4));
    falar(fatiar_verso("rosa", 2, 2));
    falar(fatiar_verso("", 0, 0));
    falar(fatiar_verso("rosa", 4, 4));
    falar(fatiar_verso("rosa", 1, 4));
    falar(fatiar_verso("aéz", 1, 2));
    falar(fatiar_verso("a日z", 1, 2));
    falar(fatiar_verso("a🌸z", 1, 2));
    falar(fatiar_verso("aéz", 1, 2));
    falar(fatiar_verso("aéz", 2, 3));
    falar(fatiar_verso("Aé日🌸Z", 1, 4));
    falar(recorte_local());
    falar(fatiar_verso("0123456789é日🌸abcdefghij", 5, 20));
    mimo 0;
}
"#;

const EXPECTED_STDOUT: &str = concat!(
    "r\n",
    "os\n",
    "a\n",
    "rosa\n",
    "\n",
    "\n",
    "\n",
    "osa\n",
    "é\n",
    "日\n",
    "🌸\n",
    "e\n",
    "\u{301}\n",
    "é日🌸\n",
    "🌸\n",
    "56789é日🌸abcdefg\n",
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
    fs::write(&path, source).expect("gravar fonte D9 temporária");
    path
}

fn run_interpreter_cli(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador D9 sob envelope")
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
        .expect("compilar D9 sob envelope")
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D9 sob envelope")
}

#[test]
fn contrato_semantico_exige_verso_e_dois_indices_bombom() {
    assert!(common::parse_and_check(
        "pacote main; carinho principal() -> bombom { nova x: verso = fatiar_verso(\"rosa\", 1, 3); mimo tamanho_verso(x); }"
    )
    .is_ok());

    let cases = [
        ("fatiar_verso(\"rosa\", 1)", "aridade inválida: esperado 3"),
        ("fatiar_verso(7, 1, 2)", "tipo inválido no argumento 1"),
        (
            "fatiar_verso(\"rosa\", falso, 2)",
            "tipo inválido no argumento 2",
        ),
        (
            "fatiar_verso(\"rosa\", 1, \"dois\")",
            "tipo inválido no argumento 3",
        ),
    ];
    for (call, expected) in cases {
        let code = format!(
            "pacote main; carinho principal() -> bombom {{ nova x: verso = {call}; mimo tamanho_verso(x); }}"
        );
        let error = common::parse_and_check(&code).unwrap_err().to_string();
        assert!(error.contains(expected), "{call}: {error}");
    }
}

#[test]
fn ascii_unicode_vazios_boundaries_e_lifetime_funcionam_no_interpretador() {
    assert_eq!(
        run_code(POSITIVE_SOURCE).unwrap(),
        Some(RuntimeValue::Int(0))
    );
    let asm = emit_asm(POSITIVE_SOURCE).expect("emitir assembly D9");
    assert!(asm.contains("call pinker_verso_fatiar"), "{asm}");
}

#[test]
fn bounds_invalidos_e_overflow_falham_pelo_erro_de_runtime() {
    let cases = [
        ("inicio", "fatiar_verso(\"abc\", 4, 4)", "índice inicial"),
        ("fim", "fatiar_verso(\"abc\", 0, 4)", "índice final"),
        (
            "ordem",
            "fatiar_verso(\"abc\", 2, 1)",
            "início maior que fim",
        ),
        (
            "overflow",
            "fatiar_verso(\"abc\", 18446744073709551615, 18446744073709551615)",
            "índice inicial",
        ),
    ];
    for (name, call, expected) in cases {
        let code = format!(
            "pacote main; carinho principal() -> bombom {{ nova x: verso = {call}; mimo tamanho_verso(x); }}"
        );
        let error = run_code(&code).unwrap_err();
        assert!(error.contains(expected), "{name}: {error}");
        assert!(!error.contains("panicked"), "{name}: {error}");
    }
}

fn usa_fronteiras_logicas(block: &str, texto: &str, inicio: &str, fim: &str) -> bool {
    block
        .matches(&format!("verso_codepoint_byte_offset({texto}, {inicio})"))
        .count()
        == 1
        && block
            .matches(&format!("verso_codepoint_byte_offset({texto}, {fim})"))
            .count()
            == 1
        && !block.contains("as_bytes()[")
}

#[test]
fn sensitivity_recusa_offsets_de_byte_crus_e_fronteiras_utf8_invalidas() {
    assert!("Aé日🌸Z".len() > "Aé日🌸Z".chars().count());
    assert_ne!("Aé日🌸Z".char_indices().nth(2).unwrap().0, 2);

    let interpreter_source = include_str!("../src/interpreter.rs");
    let runtime_source = include_str!("../runtime/pinker_rt/src/lib.rs");
    assert!(interpreter_source.contains("for (byte_offset, _) in texto.char_indices()"));
    assert!(runtime_source.contains("for (byte_offset, _) in texto.char_indices()"));

    let interpreter = interpreter_source
        .split_once("\"fatiar_verso\" => {")
        .expect("braço fatiar_verso no interpretador")
        .1
        .split_once("\"contem_verso\" => {")
        .expect("fim do braço fatiar_verso no interpretador")
        .0;
    let runtime = runtime_source
        .split_once("pub unsafe extern \"C\" fn pinker_verso_fatiar(")
        .expect("função fatiar no runtime")
        .1
        .split_once("pub unsafe extern \"C\" fn pinker_verso_contem")
        .expect("fim da função fatiar no runtime")
        .0;

    assert!(usa_fronteiras_logicas(interpreter, "value", "start", "end"));
    assert!(usa_fronteiras_logicas(runtime, "texto", "inicio", "fim"));
    assert!(interpreter.contains("value[start_byte..end_byte]"));
    assert!(runtime.contains("texto[inicio_byte..fim_byte]"));

    let mutant_interpreter = interpreter
        .replace(
            "verso_codepoint_byte_offset(value, start)",
            "Some(start as usize)",
        )
        .replace(
            "verso_codepoint_byte_offset(value, end)",
            "Some(end as usize)",
        );
    let mutant_runtime = runtime
        .replace(
            "verso_codepoint_byte_offset(texto, inicio)",
            "Some(inicio as usize)",
        )
        .replace(
            "verso_codepoint_byte_offset(texto, fim)",
            "Some(fim as usize)",
        );
    assert!(!usa_fronteiras_logicas(
        &mutant_interpreter,
        "value",
        "start",
        "end"
    ));
    assert!(!usa_fronteiras_logicas(
        &mutant_runtime,
        "texto",
        "inicio",
        "fim"
    ));
}

#[test]
fn paridade_interpretador_nativo_positiva_negativa_e_bounded() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let positive_dir = NativeArtifactDir::create().expect("diretório nativo D9 positivo");
    let positive_source = write_case(&positive_dir, "d9_slice_positive", POSITIVE_SOURCE);
    let interpreted = run_interpreter_cli(&positive_source, "d9-positive-interpreter");
    let build = build_native(
        &positive_dir,
        &positive_source,
        &runtime_lib,
        "d9-positive-build",
    );
    assert!(
        build.status.success(),
        "build nativo D9 falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &positive_dir.path().join("d9_slice_positive"),
        "d9-positive-native",
    );
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), EXPECTED_STDOUT);

    let negative_cases = [
        ("start", "fatiar_verso(\"abc\", 4, 4)", "índice inicial"),
        ("end", "fatiar_verso(\"abc\", 0, 4)", "índice final"),
        (
            "order",
            "fatiar_verso(\"abc\", 2, 1)",
            "início maior que fim",
        ),
        (
            "overflow",
            "fatiar_verso(\"abc\", 18446744073709551615, 18446744073709551615)",
            "índice inicial",
        ),
    ];
    for (name, call, expected) in negative_cases {
        let source = format!(
            "pacote main; carinho principal() -> bombom {{ nova x: verso = {call}; falar(x); mimo 0; }}"
        );
        let dir = NativeArtifactDir::create().expect("diretório nativo D9 negativo");
        let path = write_case(&dir, &format!("d9_slice_negative_{name}"), &source);
        let interpreted = run_interpreter_cli(&path, &format!("d9-{name}-interpreter"));
        let build = build_native(&dir, &path, &runtime_lib, &format!("d9-{name}-build"));
        assert!(
            build.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = run_native(
            &dir.path().join(format!("d9_slice_negative_{name}")),
            &format!("d9-{name}-native"),
        );
        assert_eq!(interpreted.status.code(), Some(1), "{name}");
        assert_eq!(native.status.code(), Some(1), "{name}");
        assert!(
            String::from_utf8_lossy(&interpreted.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&interpreted.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&native.stderr)
        );
    }
}
// @pinker-nav:end evidencia.fatiar-verso.d9-unicode
