mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::EnumPattern;
use pinker_v0::cfg_ir::{self, InstructionCfgIR, ProgramCfgIR, TerminatorIR};
use pinker_v0::ir::{self, EnumPatternIR, InstructionIR, ProgramIR, TypeIR};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, cfg_ir_validate, instr_select,
    instr_select_validate, ir_validate, semantic,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.encaixe.d10-patterns-aninhados
// @pinker-nav:domain encaixe
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova adulta D10 de patterns recursivos de leque: AST/IR recursivas, identidade nominal, aridade, escopo e materialização tardia de bindings, exaustividade/senao, duas famílias independentes, profundidade pequena, scrutinee único e paridade interpretador-nativo sob envelope.
const POSITIVE_SOURCE: &str = r#"
pacote main;

leque Interno { Numero(bombom), Texto(verso) }
leque Externo { Valor(Interno), Fim }
leque Camada { Caixa(Externo), Vazio }

leque OutraInterna { Sim(bombom), Nao }
leque OutraExterna { Item(OutraInterna), Fim }

carinho criar() -> Externo {
    falar("avaliou");
    mimo Externo.Valor(Interno.Texto("rosa"));
}

carinho principal() -> bombom {
    encaixe criar() {
        caso Externo.Valor(Interno.Numero(n)) { falar(n); }
        caso Externo.Valor(Interno.Texto(t)) { falar(t); }
        caso Externo.Fim { falar("fim"); }
    }

    nova profundo: Camada = Camada.Caixa(Externo.Valor(Interno.Numero(9)));
    encaixe profundo {
        caso Camada.Caixa(Externo.Valor(Interno.Numero(n))) { falar(n); }
        caso Camada.Caixa(Externo.Valor(Interno.Texto(t))) { falar(t); }
        caso Camada.Caixa(Externo.Fim) { falar("caixa-fim"); }
        caso Camada.Vazio { falar("vazio"); }
    }

    nova outra: OutraExterna = OutraExterna.Item(OutraInterna.Sim(4));
    encaixe outra {
        caso OutraExterna.Item(OutraInterna.Sim(n)) { falar(n); }
        caso OutraExterna.Item(OutraInterna.Nao) { falar("nao"); }
        caso OutraExterna.Fim { falar("outra-fim"); }
    }

    nova fim: Externo = Externo.Fim;
    encaixe fim {
        caso Externo.Valor(Interno.Numero(n)) { falar(n); }
        caso Externo.Valor(Interno.Texto(t)) { falar(t); }
        caso Externo.Fim { falar("outer-fim"); }
    }

    nova resto: Externo = Externo.Valor(Interno.Texto("resto"));
    encaixe resto {
        caso Externo.Valor(Interno.Numero(n)) { falar(n); }
        senao { falar("senao"); }
    }

    nova mapa: mapa<bombom, Externo> = mapa_criar();
    mapa_definir(mapa, 1, Externo.Valor(Interno.Numero(6)));
    encaixe mapa_obter(mapa, 1) {
        caso Externo.Valor(Interno.Numero(n)) { falar(n); }
        caso Externo.Valor(Interno.Texto(t)) { falar(t); }
        caso Externo.Fim { falar("mapa-fim"); }
    }
    mimo 0;
}
"#;

const EXPECTED_STDOUT: &str = "avaliou\nrosa\n9\n4\nouter-fim\nsenao\n6\n";

fn pipeline(code: &str) -> (ProgramIR, ProgramCfgIR) {
    let ast = common::parse(code).expect("parse D10");
    semantic::check_program(&ast).expect("semântica D10");
    let ir = ir::lower_program(&ast).expect("IR D10");
    ir_validate::validate_program(&ir).expect("validação IR D10");
    let cfg = cfg_ir::lower_program(&ir).expect("CFG D10");
    cfg_ir_validate::validate_program(&cfg).expect("validação CFG D10");
    let selected = instr_select::lower_program(&cfg).expect("seleção D10");
    instr_select_validate::validate_program(&selected).expect("validação seleção D10");
    let machine = abstract_machine::lower_program(&selected).expect("máquina D10");
    abstract_machine_validate::validate_program(&machine).expect("validação máquina D10");
    (ir, cfg)
}

fn recusa(code: &str) -> String {
    common::parse_and_check(code)
        .expect_err("programa D10 deveria ser recusado antes do backend")
        .to_string()
}

fn pattern_depth(pattern: &EnumPattern) -> usize {
    match pattern {
        EnumPattern::Binding { .. } => 0,
        EnumPattern::Variant { payloads, .. } => {
            1 + payloads.iter().map(pattern_depth).max().unwrap_or(0)
        }
    }
}

fn ir_pattern_depth(pattern: &EnumPatternIR) -> usize {
    match pattern {
        EnumPatternIR::Binding { .. } => 0,
        EnumPatternIR::Variant { payloads, .. } => {
            1 + payloads
                .iter()
                .map(|payload| ir_pattern_depth(&payload.pattern))
                .max()
                .unwrap_or(0)
        }
    }
}

fn write_case(dir: &NativeArtifactDir, name: &str, source: &str) -> PathBuf {
    let path = dir.path().join(format!("{name}.pink"));
    fs::write(&path, source).expect("gravar caso D10");
    path
}

fn run_interpreter(path: &Path, logical_case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("interpretador D10 sob envelope")
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
        .expect("build nativo D10 sob envelope")
}

fn run_native(path: &Path, logical_case: &str) -> Output {
    Command::new(path)
        .logical_case(logical_case)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("ELF D10 sob envelope")
}

#[test]
fn ast_e_ir_preservam_arvore_recursiva_sem_special_case_nominal() {
    let ast = common::parse(POSITIVE_SOURCE).expect("parse D10");
    let json = ast.to_json_pretty();
    assert!(json.contains("EnumMatchStmt"));
    assert!(json.matches("EnumPatternVariant").count() >= 12);
    let max_ast_depth = ast
        .items
        .iter()
        .filter_map(|item| match item {
            pinker_v0::ast::Item::Function(function) if function.name == "principal" => {
                Some(&function.body)
            }
            _ => None,
        })
        .flat_map(|body| &body.stmts)
        .filter_map(|stmt| match stmt {
            pinker_v0::ast::Stmt::EnumMatch(enum_match) => Some(
                enum_match
                    .arms
                    .iter()
                    .map(|arm| pattern_depth(&arm.pattern))
                    .max()
                    .unwrap_or(0),
            ),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_ast_depth >= 3,
        "AST voltou a ser plana: {max_ast_depth}"
    );

    let (ir, _) = pipeline(POSITIVE_SOURCE);
    let max_ir_depth = ir
        .functions
        .iter()
        .find(|function| function.name == "principal")
        .expect("principal IR")
        .entry
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            InstructionIR::EnumMatch(enum_match) => Some(
                enum_match
                    .arms
                    .iter()
                    .map(|arm| ir_pattern_depth(&arm.pattern))
                    .max()
                    .unwrap_or(0),
            ),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(max_ir_depth >= 3, "IR perdeu recursão: {max_ir_depth}");
}

#[test]
fn lowering_recursivo_adia_binding_ate_sucesso_completo() {
    let (_, cfg) = pipeline(POSITIVE_SOURCE);
    let function = cfg
        .functions
        .iter()
        .find(|function| function.name == "principal")
        .expect("principal CFG");
    let tag_calls = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction,
                InstructionCfgIR::Call { callee, .. }
                    if callee == "__pinker_internal_leque_tag"
            )
        })
        .count();
    assert!(
        tag_calls >= 3,
        "lowering deixou de percorrer níveis: {tag_calls}"
    );

    for local in function
        .locals
        .iter()
        .filter(|local| local.source_name == "n")
    {
        assert_eq!(local.ty, TypeIR::Bombom, "binding numérico perdeu o tipo");
        let block = function
            .blocks
            .iter()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction, InstructionCfgIR::Let { slot, .. } if slot == &local.slot)
                })
            })
            .expect("bloco que materializa binding interno");
        assert!(
            !matches!(block.terminator, TerminatorIR::Branch { .. }),
            "binding interno nasceu antes de o pattern completo casar: {}",
            local.slot
        );
    }
    let text_bindings = function
        .locals
        .iter()
        .filter(|local| local.source_name == "t")
        .collect::<Vec<_>>();
    assert!(!text_bindings.is_empty(), "binding textual não foi criado");
    assert!(
        text_bindings
            .iter()
            .all(|binding| binding.ty == TypeIR::Verso),
        "binding textual perdeu o tipo"
    );
}

#[test]
fn validador_recusa_identidade_interna_fabricada() {
    let (mut program_ir, _) = pipeline(POSITIVE_SOURCE);
    let enum_match = program_ir
        .functions
        .iter_mut()
        .find(|function| function.name == "principal")
        .expect("principal IR")
        .entry
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            InstructionIR::EnumMatch(enum_match) => Some(enum_match),
            _ => None,
        })
        .expect("enum match IR");
    let EnumPatternIR::Variant {
        expected_type_id,
        payloads,
        ..
    } = &mut enum_match.arms[0].pattern
    else {
        panic!("pattern raiz deveria ser variante");
    };
    *expected_type_id = payloads[0].resolved_type_id;
    let error = ir_validate::validate_program(&program_ir)
        .expect_err("identidade interna fabricada deveria ser recusada")
        .to_string();
    assert!(error.contains("E-IR-ENUM-PATTERN-IDENTITY"), "{error}");
}

#[test]
fn negativos_distinguem_tipo_aridade_aplicabilidade_cobertura_e_escopo() {
    let cases = [
        (
            "arity",
            r#"pacote main; leque I { N(bombom), T(verso) } leque O { V(I), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(I.N()) { falar(1); } caso O.V(I.T(t)) { falar(t); } caso O.F { falar(0); } } mimo 0; }"#,
            "INVALID_PATTERN_PAYLOAD_ARITY",
        ),
        (
            "nominal",
            r#"pacote main; leque A { N(bombom), F } leque B { N(bombom), F } leque O { V(A), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(B.N(n)) { falar(n); } caso O.V(B.F) { falar(0); } caso O.F { falar(0); } } mimo 0; }"#,
            "INVALID_NESTED_PATTERN_TYPE",
        ),
        (
            "scalar",
            r#"pacote main; leque I { N(bombom), F } leque O { V(bombom), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(I.N(n)) { falar(n); } caso O.F { falar(0); } } mimo 0; }"#,
            "PATTERN_NOT_APPLICABLE_TO_PAYLOAD",
        ),
        (
            "non_exhaustive",
            r#"pacote main; leque I { N(bombom), T(verso) } leque O { V(I), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(I.N(n)) { falar(n); } caso O.F { falar(0); } } mimo 0; }"#,
            "NON_EXHAUSTIVE_NESTED_MATCH",
        ),
        (
            "unreachable",
            r#"pacote main; leque I { N(bombom), T(verso) } leque O { V(I), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(v) { falar(1); } caso O.V(I.N(n)) { falar(n); } caso O.F { falar(0); } } mimo 0; }"#,
            "UNREACHABLE_PATTERN",
        ),
        (
            "duplicate",
            r#"pacote main; leque I { N(bombom), T(verso) } leque O { V(I), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(I.N(n)) { falar(n); } caso O.V(I.N(outro)) { falar(outro); } caso O.V(I.T(t)) { falar(t); } caso O.F { falar(0); } } mimo 0; }"#,
            "repetida",
        ),
        (
            "leak",
            r#"pacote main; leque I { N(bombom), T(verso) } leque O { V(I), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(I.N(n)) { falar(n); } caso O.V(I.T(t)) { falar(t); } caso O.F { falar(0); } } falar(n); mimo 0; }"#,
            "não declarado",
        ),
    ];
    for (name, source, expected) in cases {
        let error = recusa(source);
        assert!(error.contains(expected), "{name}: {error}");
        assert!(!error.contains("panic"), "{name}: {error}");
    }
}

#[test]
fn senao_cobre_resto_aninhado_e_legacy_continua_valido() {
    let with_default = r#"
        pacote main;
        leque I { N(bombom), T(verso) }
        leque O { V(I), F }
        carinho principal() -> bombom {
            nova x: O = O.V(I.T("x"));
            encaixe x {
                caso O.V(I.N(n)) { falar(n); }
                senao { falar("resto"); }
            }
            mimo 0;
        }
    "#;
    assert!(common::parse_and_check(with_default).is_ok());

    let legacy = r#"
        pacote main;
        leque Token { Numero(bombom), Palavra(verso), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Numero(7);
            encaixe t {
                caso Token.Numero(n) { falar(n); }
                caso Token.Palavra(p) { falar(p); }
                caso Token.Fim { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    pipeline(legacy);

    let aliases = r#"
        pacote main;
        leque I { N(bombom), T(verso) }
        leque O { V(I), F }
        apelido IA = I;
        apelido OA = O;
        carinho principal() -> bombom {
            nova x: OA = OA.V(IA.N(8));
            encaixe x {
                caso OA.V(IA.N(n)) { falar(n); }
                caso OA.V(IA.T(t)) { falar(t); }
                caso OA.F { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    pipeline(aliases);
}

#[test]
fn interpretador_e_nativo_concordam_inclusive_scrutinee_unico() {
    let (_driver, Some(runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
            .expect("toolchain nativa D10")
    else {
        panic!("runtime nativo D10 ausente");
    };
    let dir = NativeArtifactDir::create().expect("diretório D10");
    let source = write_case(&dir, "d10_nested_positive", POSITIVE_SOURCE);
    let interpreted = run_interpreter(&source, "d10-positive-interpreter");
    assert!(interpreted.status.success());
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout),
        EXPECTED_STDOUT
    );
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout)
            .matches("avaliou")
            .count(),
        1,
        "scrutinee foi reavaliado"
    );

    let build = build_native(&dir, &source, &runtime_lib, "d10-positive-build");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = run_native(
        &dir.path().join("d10_nested_positive"),
        "d10-positive-native",
    );
    assert!(native.status.success());
    assert_eq!(native.stdout, interpreted.stdout);
    assert_eq!(native.status.code(), interpreted.status.code());
}

#[test]
fn diagnostico_negativo_e_identico_antes_do_backend() {
    let source = r#"pacote main; leque A { N(bombom), F } leque B { N(bombom), F } leque O { V(A), F } carinho principal() -> bombom { nova x: O = O.F; encaixe x { caso O.V(B.N(n)) { falar(n); } caso O.V(B.F) { falar(0); } caso O.F { falar(0); } } mimo 0; }"#;
    let dir = NativeArtifactDir::create().expect("diretório negativo D10");
    let path = write_case(&dir, "d10_nested_negative", source);
    let check = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(&path)
        .logical_case("d10-negative-check")
        .timeout(Duration::from_secs(20))
        .output()
        .expect("check negativo D10");
    assert!(!check.status.success());
    let check_error = String::from_utf8_lossy(&check.stderr);
    assert!(check_error.contains("INVALID_NESTED_PATTERN_TYPE"));

    let (_driver, Some(runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
            .expect("toolchain nativa negativa D10")
    else {
        panic!("runtime nativo D10 ausente");
    };
    let build = build_native(&dir, &path, &runtime_lib, "d10-negative-build");
    assert!(!build.status.success());
    assert!(String::from_utf8_lossy(&build.stderr).contains("INVALID_NESTED_PATTERN_TYPE"));
    assert!(!dir.path().join("d10_nested_negative").exists());
}
// @pinker-nav:end evidencia.encaixe.d10-patterns-aninhados
