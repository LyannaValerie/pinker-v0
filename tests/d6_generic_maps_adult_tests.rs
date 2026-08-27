mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::abstract_machine_validate;
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

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar fonte D6 temporária");
    path
}

fn run_interpreter_cli(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador D6 sob envelope")
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
        .expect("compilar D6 sob envelope")
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D6 sob envelope")
}

#[derive(Debug, PartialEq, Eq)]
struct TupleMatchCandidate {
    function: String,
    arms: usize,
    fingerprint: u64,
}

fn matching_delimiter(source: &str, opening: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, byte) in source.as_bytes()[opening..].iter().enumerate() {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(opening + offset);
            }
        }
    }
    None
}

fn structural_fingerprint(source: &str) -> u64 {
    source
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

fn tuple_match_candidates(source: &str) -> Vec<TupleMatchCandidate> {
    let mut candidates = Vec::new();
    for (match_offset, _) in source.match_indices("match") {
        let mut cursor = match_offset + "match".len();
        while source
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if source.as_bytes().get(cursor) != Some(&b'(') {
            continue;
        }
        let mut comma = false;
        let mut depth = 0_u32;
        for byte in &source.as_bytes()[cursor..] {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b',' if depth == 1 => comma = true,
                _ => {}
            }
        }
        let Some(closing) = matching_delimiter(source, cursor, b'(', b')') else {
            continue;
        };
        if !comma {
            continue;
        }
        let mut block_opening = closing + 1;
        while source
            .as_bytes()
            .get(block_opening)
            .is_some_and(u8::is_ascii_whitespace)
        {
            block_opening += 1;
        }
        if source.as_bytes().get(block_opening) != Some(&b'{') {
            continue;
        }
        let Some(block_closing) = matching_delimiter(source, block_opening, b'{', b'}') else {
            continue;
        };
        let scrutinee = source[cursor + 1..closing].to_ascii_lowercase();
        let prefix = &source[..match_offset];
        let function = prefix
            .rfind("fn ")
            .map(|start| {
                prefix[start + 3..]
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                    .collect::<String>()
            })
            .unwrap_or_default();
        let function_lower = function.to_ascii_lowercase();
        let names_key_and_value = (scrutinee.contains("key") && scrutinee.contains("value"))
            || (scrutinee.contains("chave") && scrutinee.contains("valor"));
        if function_lower.contains("map") || function_lower.contains("mapa") || names_key_and_value
        {
            let expression = &source[match_offset..=block_closing];
            candidates.push(TupleMatchCandidate {
                function,
                arms: expression.matches("=>").count(),
                fingerprint: structural_fingerprint(expression),
            });
        }
    }
    candidates
}

const BOMBOM_LEQUE: &str = r#"
pacote main; trazer mapa.definir; trazer mapa.obter; trazer mapa.remover; trazer mapa.tamanho; trazer mapa.tem; trazer texto;

leque Escolha {
    Vazio,
    Numero(bombom),
    Texto(verso),
}

carinho principal() -> bombom {
    nova m: mapa<bombom, Escolha> = mapa_criar();
    talvez tamanho(m) != 0 { mimo 1; }

    definir(m, 3, Escolha.Vazio);
    definir(m, 1, Escolha.Numero(41));
    definir(m, 2, Escolha.Texto("carga"));
    definir(m, 1, Escolha.Numero(42));
    talvez tamanho(m) != 3 { mimo 2; }
    talvez !tem(m, 2) { mimo 3; }

    remover(m, 3);
    talvez tem(m, 3) { mimo 4; }
    talvez tamanho(m) != 2 { mimo 5; }

    nova muda ordem: bombom = 0;
    para cada chave em m {
        ordem = ordem * 10 + chave;
    }
    talvez ordem != 12 { mimo 6; }

    nova numero: Escolha = obter(m, 1);
    encaixe numero {
        caso Escolha.Numero(n) { talvez n != 42 { mimo 7; } }
        caso Escolha.Texto(t) { mimo 8; }
        caso Escolha.Vazio { mimo 9; }
    }
    nova escolhido: Escolha = obter(m, 2);
    encaixe escolhido {
        caso Escolha.Numero(n) { mimo 10; }
        caso Escolha.Texto(t) { talvez texto.tamanho(t) != 5 { mimo 11; } }
        caso Escolha.Vazio { mimo 12; }
    }

    nova muda i: bombom = 4;
    sempre que i < 13 {
        definir(m, i, Escolha.Numero(i * 10));
        i = i + 1;
    }
    talvez tamanho(m) != 11 { mimo 13; }
    remover(m, 7);
    talvez tamanho(m) != 10 { mimo 14; }
    mimo 0;
}
"#;

const VERSO_LEQUE: &str = r#"
pacote main; trazer mapa.definir; trazer mapa.obter; trazer mapa.tem; trazer texto.formatar; trazer texto.tamanho;

leque Escolha { Vazio, Numero(bombom), Texto(verso) }

carinho principal() -> bombom {
    nova m: mapa<verso, Escolha> = mapa_criar();
    nova chave: verso = formatar("{}{}", "cha", "ve");
    definir(m, chave, Escolha.Numero(77));
    definir(m, "outra", Escolha.Texto("payload"));
    talvez !tem(m, "chave") { mimo 1; }
    nova valor: Escolha = obter(m, "chave");
    encaixe valor {
        caso Escolha.Numero(n) { talvez n != 77 { mimo 2; } }
        caso Escolha.Texto(t) { mimo 3; }
        caso Escolha.Vazio { mimo 4; }
    }
    nova muda ordem: bombom = 0;
    para cada k em m { ordem = ordem + tamanho(k); }
    talvez ordem != 10 { mimo 5; }
    mimo 0;
}
"#;

const PARITY_SOURCE: &str = r#"
pacote main; trazer mapa.definir; trazer mapa.obter; trazer mapa.remover; trazer mapa.tamanho; trazer texto.formatar;

leque Escolha { Vazio, Numero(bombom), Texto(verso) }

carinho principal() -> bombom {
    nova a: mapa<bombom, Escolha> = mapa_criar();
    definir(a, 1, Escolha.Vazio);
    definir(a, 2, Escolha.Numero(41));
    definir(a, 3, Escolha.Texto("carga"));
    definir(a, 2, Escolha.Numero(42));
    encaixe obter(a, 1) { caso Escolha.Vazio { falar(10); } caso Escolha.Numero(n) { falar(n); } caso Escolha.Texto(t) { falar(t); } }
    encaixe obter(a, 2) { caso Escolha.Numero(n) { falar(n); } caso Escolha.Vazio { falar(11); } caso Escolha.Texto(t) { falar(t); } }
    encaixe obter(a, 3) { caso Escolha.Texto(t) { falar(t); } caso Escolha.Vazio { falar(12); } caso Escolha.Numero(n) { falar(n); } }
    remover(a, 1);
    falar(tamanho(a));

    nova b: mapa<verso, Escolha> = mapa_criar();
    nova chave: verso = formatar("{}{}", "cha", "ve");
    definir(b, chave, Escolha.Numero(77));
    definir(b, "outra", Escolha.Texto("payload"));
    encaixe obter(b, "chave") { caso Escolha.Numero(n) { falar(n); } caso Escolha.Vazio { falar(13); } caso Escolha.Texto(t) { falar(t); } }
    para cada k em b { falar(k); }
    mimo 0;
}
"#;

#[test]
fn mapa_bombom_leque_preserva_variantes_cargas_overwrite_remove_ordem_e_crescimento() {
    assert_eq!(run_code(BOMBOM_LEQUE).unwrap(), Some(RuntimeValue::Int(0)));
}

#[test]
fn segunda_instanciacao_verso_leque_usa_igualdade_por_conteudo() {
    assert_eq!(run_code(VERSO_LEQUE).unwrap(), Some(RuntimeValue::Int(0)));
}

#[test]
fn quatro_familias_historicas_permanecem_compativeis() {
    let code = r#"
pacote main; trazer mapa.bombom_bombom_criar; trazer mapa.bombom_bombom_definir; trazer mapa.bombom_bombom_obter; trazer mapa.bombom_verso_criar; trazer mapa.bombom_verso_definir; trazer mapa.bombom_verso_obter; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer mapa.verso_verso_criar; trazer mapa.verso_verso_definir; trazer mapa.verso_verso_obter; trazer texto.tamanho;
carinho principal() -> bombom {
    nova a: mapa<verso,bombom> = verso_bombom_criar();
    nova b: mapa<verso,verso> = verso_verso_criar();
    nova c: mapa<bombom,bombom> = bombom_bombom_criar();
    nova d: mapa<bombom,verso> = bombom_verso_criar();
    verso_bombom_definir(a, "a", 1);
    verso_verso_definir(b, "b", "dois");
    bombom_bombom_definir(c, 3, 3);
    bombom_bombom_definir(c, 1, 1);
    bombom_bombom_definir(c, 2, 2);
    bombom_verso_definir(d, 4, "quatro");
    talvez verso_bombom_obter(a, "a") != 1 { mimo 1; }
    talvez tamanho(verso_verso_obter(b, "b")) != 4 { mimo 2; }
    talvez bombom_bombom_obter(c, 3) != 3 { mimo 3; }
    talvez tamanho(bombom_verso_obter(d, 4)) != 6 { mimo 4; }
    nova muda ordem: bombom = 0;
    para cada chave em c { ordem = ordem * 10 + chave; }
    talvez ordem != 312 { mimo 5; }
    mimo 0;
}
"#;
    assert_eq!(run_code(code).unwrap(), Some(RuntimeValue::Int(0)));
}

#[test]
fn lookup_ausente_falha_com_diagnostico_de_runtime() {
    let code = r#"
pacote main; trazer mapa.obter;
leque Escolha { Vazio }
carinho principal() -> bombom {
    nova m: mapa<bombom, Escolha> = mapa_criar();
    nova x: Escolha = obter(m, 99);
    mimo 0;
}
"#;
    let error = run_code(code).unwrap_err();
    assert!(
        error.contains("chave ausente em leitura de mapa"),
        "{error}"
    );
}

#[test]
fn chave_sem_capacidade_falha_na_semantica() {
    let code = r#"
pacote main;
leque Escolha { Vazio }
carinho principal() -> bombom {
    nova m: mapa<logica, Escolha> = mapa_criar();
    mimo 0;
}
"#;
    let error = common::parse_and_check(code).unwrap_err().to_string();
    assert!(
        error.contains("tipo de chave de mapa incompatível"),
        "{error}"
    );
    assert!(error.contains("logica"), "{error}");
}

#[test]
fn valor_sem_lifetime_aprovado_falha_na_semantica() {
    let code = r#"
pacote main;
carinho principal() -> bombom {
    nova m: mapa<bombom, lista<bombom>> = mapa_criar();
    mimo 0;
}
"#;
    let error = common::parse_and_check(code).unwrap_err().to_string();
    assert!(
        error.contains("representação de valor de mapa não suportada"),
        "{error}"
    );
    assert!(error.contains("lista<bombom>"), "{error}");
}

#[test]
fn lowering_de_duas_instanciacoes_compartilha_as_mesmas_operacoes() {
    let bombom_ir = common::render_ir(BOMBOM_LEQUE).unwrap();
    let verso_ir = common::render_ir(VERSO_LEQUE).unwrap();
    for symbol in [
        "__pinker_internal_mapa_definir",
        "__pinker_internal_mapa_obter",
        "__pinker_internal_mapa_tem",
        "__pinker_internal_mapa_tamanho",
        "__pinker_internal_mapa_iterador_criar",
    ] {
        assert!(
            bombom_ir.contains(symbol),
            "{symbol} ausente em {bombom_ir}"
        );
        assert!(verso_ir.contains(symbol), "{symbol} ausente em {verso_ir}");
    }
}

#[test]
fn sensibilidade_recusa_familias_manuais_para_as_novas_combinacoes() {
    let interpreter = include_str!("../src/interpreter.rs");
    let surfaces = [
        include_str!("../src/ast.rs"),
        include_str!("../src/parser.rs"),
        include_str!("../src/semantic.rs"),
        include_str!("../src/ir.rs"),
        include_str!("../src/interpreter.rs"),
        include_str!("../src/backend_s.rs"),
    ]
    .join("\n");
    for forbidden in [
        "MapBombomLeque",
        "MapVersoLeque",
        "mapa_bombom_leque",
        "mapa_verso_leque",
    ] {
        assert!(
            !surfaces.contains(forbidden),
            "matriz K×V reapareceu: {forbidden}"
        );
    }
    // Autoriza o conteúdo estrutural exato das três matrizes públicas históricas.
    // Uma combinação, arm ou helper novo altera contagem e fingerprint.
    let allowed_legacy_tuple_matches = [
        ("parser.rs", "generic_map_callee", 21, 0x27c0_ab8f_221a_2856),
        (
            "semantic.rs",
            "generic_map_monomorphic_callee",
            21,
            0xc67c_85c1_adfd_da2c,
        ),
        (
            "ir.rs",
            "generic_map_monomorphic_callee",
            21,
            0x801d_52f7_919f_3d40,
        ),
    ];
    for (file, source) in [
        ("ast.rs", include_str!("../src/ast.rs")),
        ("parser.rs", include_str!("../src/parser.rs")),
        ("semantic.rs", include_str!("../src/semantic.rs")),
        ("ir.rs", include_str!("../src/ir.rs")),
        ("interpreter.rs", include_str!("../src/interpreter.rs")),
        ("backend_s.rs", include_str!("../src/backend_s.rs")),
    ] {
        for candidate in tuple_match_candidates(source) {
            assert!(
                allowed_legacy_tuple_matches.contains(&(
                    file,
                    candidate.function.as_str(),
                    candidate.arms,
                    candidate.fingerprint,
                )),
                "matriz manual K×V detectada ou adapter legado alterado em \
                 {file}::{} (arms={}, fingerprint={:#018x})",
                candidate.function,
                candidate.arms,
                candidate.fingerprint,
            );
        }
    }
    let mutation = r#"
        fn lower_mapa(chave_tipo: Tipo, valor_tipo: Tipo) {
            match (chave_tipo, valor_tipo) {
                (Bombom, Enum) => emitir_a(),
                (Verso, Enum) => emitir_b(),
                _ => rejeitar(),
            }
        }
    "#;
    let mutation_candidates = tuple_match_candidates(mutation);
    assert_eq!(mutation_candidates.len(), 1);
    assert_eq!(mutation_candidates[0].function, "lower_mapa");
    assert_eq!(mutation_candidates[0].arms, 3);
    assert!(
        !allowed_legacy_tuple_matches.iter().any(|allowed| {
            allowed.1 == mutation_candidates[0].function
                && allowed.2 == mutation_candidates[0].arms
                && allowed.3 == mutation_candidates[0].fingerprint
        }),
        "o detector deixou sobreviver uma mutação manual K×V com nomes novos"
    );
    assert!(surfaces.contains("RuntimeGenericMap"));
    assert!(surfaces.contains("Map {") && surfaces.contains("key: MapKeyIR"));
    let dispatch = interpreter
        .find("try_call_map_intrinsic_authority(callee, args, map_state)")
        .expect("dispatch genérico hospedado");
    let legacy_match = interpreter[dispatch..]
        .find("match callee {")
        .map(|offset| dispatch + offset)
        .expect("match de compatibilidade");
    assert!(
        dispatch < legacy_match,
        "aliases históricos deixaram de encaminhar à autoridade genérica"
    );
}

#[test]
fn paridade_interpretador_nativo_positiva_e_negativa_e_bounded() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let positive_dir = NativeArtifactDir::create().expect("diretório nativo D6 positivo");
    let positive_source = write_case(&positive_dir, "d6_generic_maps_positive", PARITY_SOURCE);
    let interpreted = run_interpreter_cli(&positive_source, "d6-positive-interpreter");
    let build = build_native(
        &positive_dir,
        &positive_source,
        &runtime_lib,
        "d6-positive-build",
    );
    assert!(
        build.status.success(),
        "build nativo D6 falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &positive_dir.path().join("d6_generic_maps_positive"),
        "d6-positive-native",
    );
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(
        String::from_utf8_lossy(&native.stdout),
        "10\n42\ncarga\n2\n77\nchave\noutra\n"
    );

    let negative_cases = [
        (
            "key",
            "pacote main; leque K { A } carinho principal() -> bombom { nova m: mapa<K,bombom> = mapa_criar(); mimo 0; }",
            "tipo de chave de mapa incompatível",
        ),
        (
            "value",
            "pacote main; carinho principal() -> bombom { nova m: mapa<bombom,lista<bombom>> = mapa_criar(); mimo 0; }",
            "representação de valor de mapa não suportada",
        ),
    ];
    for (name, source, diagnostic) in negative_cases {
        let dir = NativeArtifactDir::create().expect("diretório nativo D6 negativo");
        let path = write_case(&dir, &format!("d6_negative_{name}"), source);
        let interpreted = run_interpreter_cli(&path, &format!("d6-{name}-interpreter"));
        let native_build = build_native(&dir, &path, &runtime_lib, &format!("d6-{name}-build"));
        assert_eq!(interpreted.status.code(), Some(1), "{name}: interpretador");
        assert_eq!(native_build.status.code(), Some(1), "{name}: build nativo");
        assert!(
            String::from_utf8_lossy(&interpreted.stderr).contains(diagnostic),
            "{name}: {}",
            String::from_utf8_lossy(&interpreted.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native_build.stderr).contains(diagnostic),
            "{name}: {}",
            String::from_utf8_lossy(&native_build.stderr)
        );
    }
}
