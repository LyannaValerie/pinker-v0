mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::abstract_machine_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

const VALID_SCALING: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova raw: seta<u8> = alocar(16);
    nova base: seta<u32> = raw virar seta<u32>;
    *base = 11;
    nova zero: seta<u32> = base + 0;
    nova segundo: seta<u32> = base + 1;
    *segundo = 22;
    nova terceiro: seta<u32> = base + 2;
    *terceiro = 33;
    nova fim: seta<u32> = base + 4;
    falar(*zero);
    falar(*segundo);
    falar(*terceiro);
    liberar(raw);
    mimo 0;
}
"#;

fn compile_machine(source: &str) -> Result<pinker_v0::abstract_machine::MachineProgram, String> {
    let program = common::parse(source).map_err(|error| error.to_string())?;
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
    Ok(machine)
}

fn run_interpreter(source: &str) -> Result<interpreter::RunOutcome, String> {
    let machine = compile_machine(source)?;
    interpreter::run_program_with_args(&machine, &[]).map_err(|error| error.to_string())
}

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar fonte D5 temporária");
    path
}

fn run_interpreter_cli(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run"])
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador sob envelope")
}

fn build_native(
    dir: &NativeArtifactDir,
    path: &Path,
    runtime_lib: &Path,
    logical_case: &str,
) -> PathBuf {
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(path)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("compilar ELF D5 sob envelope");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    dir.path().join(path.file_stem().expect("stem da fonte D5"))
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D5 sob envelope")
}

#[test]
fn frontend_aceita_apenas_offset_bombom_e_elementos_com_acesso_coerente() {
    let supported = r#"
        pacote main;
        ninho Par { a: u8; b: u32; }
        carinho principal() -> bombom {
            nova a: seta<u8> = 1; nova a1: seta<u8> = a + 1;
            nova b: seta<u16> = 1; nova b1: seta<u16> = b + 1;
            nova c: seta<u32> = 1; nova c1: seta<u32> = c + 1;
            nova d: seta<u64> = 1; nova d1: seta<u64> = d + 1;
            nova e: seta<i8> = 1; nova e1: seta<i8> = e + 1;
            nova f: seta<i16> = 1; nova f1: seta<i16> = f + 1;
            nova g: seta<i32> = 1; nova g1: seta<i32> = g + 1;
            nova h: seta<i64> = 1; nova h1: seta<i64> = h + 1;
            nova i: seta<bombom> = 1; nova i1: seta<bombom> = i + 1;
            nova j: seta<logica> = 1; nova j1: seta<logica> = j + 1;
            nova k: seta<[bombom; 3]> = 1; nova k1: seta<[bombom; 3]> = k + 1;
            nova l: seta<Par> = 1; nova l1: seta<Par> = l + 1;
            mimo 0;
        }
    "#;
    common::parse_and_check(supported).expect("matriz D5 suportada");

    let unsupported = [
        (
            "tipo sem layout/acesso",
            "pacote main; carinho principal() -> bombom { nova p: seta<verso> = 1; nova q: seta<verso> = p + 1; mimo 0; }",
            "não participa de D5",
        ),
        (
            "offset u8",
            "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 1; nova n: u8 = 1; nova q: seta<u32> = p + n; mimo 0; }",
            "seta<T> + bombom",
        ),
        (
            "offset negativo",
            "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 1; nova q: seta<u32> = p + -1; mimo 0; }",
            "não negativo",
        ),
        (
            "inteiro mais ponteiro",
            "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 1; nova q: seta<u32> = 1 + p; mimo 0; }",
            "seta<T> + bombom",
        ),
    ];
    for (name, source, expected) in unsupported {
        let error = common::parse_and_check(source).expect_err(name).to_string();
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[test]
fn ir_cfg_selecao_e_maquina_preservam_layout_e_nao_soma_crua() {
    let ir = common::render_ir(VALID_SCALING).expect("IR D5");
    let cfg = common::render_cfg_ir(VALID_SCALING).expect("CFG D5");
    let selected = common::render_selected(VALID_SCALING).expect("seleção D5");
    let machine = common::render_machine(VALID_SCALING).expect("máquina D5");
    for (layer, rendered) in [
        ("IR", ir),
        ("CFG", cfg),
        ("seleção", selected),
        ("máquina", machine),
    ] {
        assert!(
            rendered.contains("pointer_offset")
                && rendered.contains("size=4")
                && rendered.contains("align=4"),
            "{layer} perdeu layout tipado:\n{rendered}"
        );
    }

    let assembly =
        common::render_backend_s_external_subset_nativo(VALID_SCALING).expect("assembly nativo D5");
    assert!(assembly.contains("call pinker_ponteiro_derivar_tipado"));
    assert!(assembly.contains("movq $4, %rdx"), "scaling u32 ausente");
    assert!(assembly.contains("movq $4, %rcx"), "align u32 ausente");
    assert!(assembly.contains("call pinker_publico_validar_derivacao"));
}

#[test]
fn interpretador_cobre_scaling_zero_multiplos_load_store_e_proveniencia() {
    let outcome = run_interpreter(VALID_SCALING).expect("execução válida D5");
    assert_eq!(outcome.exit_status, Some(0));
    assert_eq!(
        outcome.return_value,
        Some(interpreter::RuntimeValue::Int(0))
    );
}

#[test]
fn derivacao_one_past_existe_mas_acesso_one_past_e_invalido() {
    let derive = r#"
        pacote main;
        carinho principal() -> bombom {
            nova raw: seta<u8> = alocar(8);
            nova p: seta<u32> = raw virar seta<u32>;
            nova fim: seta<u32> = p + 2;
            liberar(raw);
            mimo 0;
        }
    "#;
    run_interpreter(derive).expect("one-past deve poder existir");

    let access = r#"
        pacote main;
        carinho principal() -> bombom {
            nova raw: seta<u8> = alocar(8);
            nova p: seta<u32> = raw virar seta<u32>;
            nova fim: seta<u32> = p + 2;
            mimo (*fim) virar bombom;
        }
    "#;
    let error = run_interpreter(access).expect_err("deref one-past deve falhar");
    assert!(
        error.contains("E-RUNTIME-MEM-CROSS-BOUNDARY")
            || error.contains("E-RUNTIME-MEM-OUT-OF-BOUNDS"),
        "{error}"
    );
}

#[test]
fn negativos_interpretados_cobrem_bounds_uaf_null_overflows_e_alinhamento() {
    let cases = [
        (
            "bounds",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u32> = raw virar seta<u32>; nova q: seta<u32> = p + 3; mimo 0; }",
            "E-RUNTIME-MEM-OUT-OF-BOUNDS",
        ),
        (
            "uaf",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u32> = raw virar seta<u32>; liberar(raw); nova q: seta<u32> = p + 1; mimo 0; }",
            "E-RUNTIME-MEM-USE-AFTER-FREE",
        ),
        (
            "null",
            "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 0; nova q: seta<u32> = p + 0; mimo 0; }",
            "E-RUNTIME-POINTER-NULL-ARITHMETIC",
        ),
        (
            "scaling overflow",
            "pacote main; carinho deslocar(p: seta<u32>, n: bombom) -> seta<u32> { mimo p + n; } carinho principal() -> bombom { nova p: seta<u32> = 1; nova q: seta<u32> = deslocar(p, 4611686018427387904); mimo 0; }",
            "E-RUNTIME-POINTER-OFFSET-OVERFLOW",
        ),
        (
            "address overflow",
            "pacote main; carinho principal() -> bombom { nova p: seta<u8> = 18446744073709551614; nova q: seta<u8> = p + 2; mimo 0; }",
            "E-RUNTIME-POINTER-ADDRESS-OVERFLOW",
        ),
        (
            "alignment",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u8> = raw + 1; nova q: seta<u32> = p virar seta<u32>; mimo (*q) virar bombom; }",
            "E-RUNTIME-MEM-MISALIGNED",
        ),
    ];
    for (name, source, diagnostic) in cases {
        let error = run_interpreter(source).expect_err(name);
        assert!(error.contains(diagnostic), "{name}: {error}");
    }
}

#[test]
fn literal_com_scaling_estourado_falha_na_semantica() {
    let source = "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 1; nova q: seta<u32> = p + 4611686018427387904; mimo 0; }";
    let error = common::parse_and_check(source)
        .expect_err("overflow constante deve falhar cedo")
        .to_string();
    assert!(error.contains("E-POINTER-OFFSET-OVERFLOW"), "{error}");
}

#[test]
fn paridade_interpretador_nativo_valida_e_negativa_e_bounded() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let valid_dir = NativeArtifactDir::create().expect("diretório nativo D5 válido");
    let valid_source = write_case(&valid_dir, "d5_valid_scaling", VALID_SCALING);
    let interpreted = run_interpreter_cli(&valid_source, "d5-valid-interpreter");
    let executable = build_native(&valid_dir, &valid_source, &runtime_lib, "d5-valid-build");
    let native = run_native(&executable, "d5-valid-native");
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), "11\n22\n33\n");

    let negative_cases = [
        (
            "null",
            "pacote main; carinho principal() -> bombom { nova p: seta<u32> = 0; nova q: seta<u32> = p + 0; mimo 0; }",
            "E-RUNTIME-POINTER-NULL-ARITHMETIC",
        ),
        (
            "bounds",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u32> = raw virar seta<u32>; nova q: seta<u32> = p + 3; mimo 0; }",
            "E-RUNTIME-MEM-OUT-OF-BOUNDS",
        ),
        (
            "alignment",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u8> = raw + 1; nova q: seta<u32> = p virar seta<u32>; mimo (*q) virar bombom; }",
            "E-RUNTIME-MEM-MISALIGNED",
        ),
        (
            "onepast-access",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u32> = raw virar seta<u32>; nova fim: seta<u32> = p + 2; mimo (*fim) virar bombom; }",
            "E-RUNTIME-MEM-CROSS-BOUNDARY",
        ),
        (
            "uaf",
            "pacote main; carinho principal() -> bombom { nova raw: seta<u8> = alocar(8); nova p: seta<u32> = raw virar seta<u32>; liberar(raw); nova q: seta<u32> = p + 1; mimo 0; }",
            "E-RUNTIME-MEM-USE-AFTER-FREE",
        ),
        (
            "scaling-overflow",
            "pacote main; carinho deslocar(p: seta<u32>, n: bombom) -> seta<u32> { mimo p + n; } carinho principal() -> bombom { nova p: seta<u32> = 1; nova q: seta<u32> = deslocar(p, 4611686018427387904); mimo 0; }",
            "E-RUNTIME-POINTER-OFFSET-OVERFLOW",
        ),
        (
            "address-overflow",
            "pacote main; carinho principal() -> bombom { nova p: seta<u8> = 18446744073709551614; nova q: seta<u8> = p + 2; mimo 0; }",
            "E-RUNTIME-POINTER-ADDRESS-OVERFLOW",
        ),
    ];
    for (name, source, diagnostic) in negative_cases {
        let dir = NativeArtifactDir::create().expect("diretório nativo D5 negativo");
        let source_path = write_case(&dir, &format!("d5_negative_{name}"), source);
        let interpreted = run_interpreter_cli(&source_path, &format!("d5-{name}-interpreter"));
        let executable = build_native(
            &dir,
            &source_path,
            &runtime_lib,
            &format!("d5-{name}-build"),
        );
        let native = run_native(&executable, &format!("d5-{name}-native"));
        assert_eq!(interpreted.status.code(), Some(1), "{name}: interpreter");
        assert_eq!(native.status.code(), Some(1), "{name}: native/signal");
        assert!(
            String::from_utf8_lossy(&interpreted.stderr).contains(diagnostic),
            "{name} interpreter: {}",
            String::from_utf8_lossy(&interpreted.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains(diagnostic),
            "{name} native: {}",
            String::from_utf8_lossy(&native.stderr)
        );
    }
}
