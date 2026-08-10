mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::{ExprKind, InlineAsmDirection, Item, Stmt};
use pinker_v0::inline_asm;
use pinker_v0::{
    abstract_machine, abstract_machine_validate, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, interpreter, ir, ir_validate, semantic,
};
use std::fs;
use std::time::Duration;

const OPERAND_PROGRAM: &str = r#"
pacote main;
carinho principal() -> bombom {
    nova a: bombom = 20;
    nova b: bombom = 22;
    nova muda resultado: bombom = 0;
    nova vivo: bombom = 7;
    sussurro(
        "mov {resultado}, {a}\nadd {resultado}, {b}";
        entrada a: r8 = a;
        entrada b: r9 = b;
        saida resultado: r11 = resultado;
        destroi(flags)
    );
    falar(resultado + vivo);
    mimo 0;
}
"#;

fn error(source: &str) -> String {
    common::parse_and_check(source)
        .expect_err("fonte D4 deveria ser recusada")
        .to_string()
}

fn wrapped(body: &str) -> String {
    format!("pacote main; carinho principal() -> bombom {{ {body} mimo 0; }}")
}

#[test]
fn parser_representa_direcoes_constraints_valores_e_clobbers() {
    let program = common::parse(OPERAND_PROGRAM).expect("parser D4");
    let Item::Function(function) = &program.items[0] else {
        panic!("função esperada");
    };
    let Stmt::InlineAsm(stmt) = &function.body.stmts[4] else {
        panic!("sussurro esperado");
    };
    assert_eq!(stmt.operands.len(), 3);
    assert_eq!(stmt.clobbers.len(), 1);
    assert_eq!(stmt.operands[0].name, "a");
    assert_eq!(stmt.operands[0].direction, InlineAsmDirection::Input);
    assert_eq!(stmt.operands[2].direction, InlineAsmDirection::Output);
    assert!(
        matches!(stmt.operands[2].value.kind, ExprKind::Ident(ref name) if name == "resultado")
    );
}

#[test]
fn input_output_multiplos_e_valor_vivo_aparecem_no_assembly_real() {
    let asm =
        common::render_backend_s_external_subset_nativo(OPERAND_PROGRAM).expect("assembly D4");
    for expected in [
        "# pinker:sussurro input a -> r8",
        "movq -8(%rbp), %r8",
        "# pinker:sussurro input b -> r9",
        "movq -16(%rbp), %r9",
        "mov r11, r8",
        "add r11, r9",
        "movq %r11, -24(%rbp)",
        "movq -32(%rbp), %r10",
    ] {
        assert!(asm.contains(expected), "ausente: {expected}\n{asm}");
    }
}

#[test]
fn clobber_declarado_e_aceito_e_conflito_e_diagnostico_pinker() {
    common::parse_and_check(&wrapped(
        r#"sussurro("xor r10, r10"; destroi(r10, flags));"#,
    ))
    .expect("clobber caller-saved declarado");

    let conflict = error(&wrapped(
        r#"nova x: bombom = 1; sussurro("nop {x}"; entrada x: r8 = x; destroi(r8));"#,
    ));
    assert!(
        conflict.contains("E-SEMANTIC-ASM-REGISTER-CONFLICT"),
        "{conflict}"
    );
}

#[test]
fn tipos_escalares_e_seta_tem_representacao_nativa_explicita() {
    for (ty, value) in [
        ("bombom", "1"),
        ("u8", "1"),
        ("u16", "1"),
        ("u32", "1"),
        ("u64", "1"),
        ("i8", "1"),
        ("i16", "1"),
        ("i32", "1"),
        ("i64", "1"),
        ("logica", "verdade"),
        ("seta<bombom>", "1 virar seta<bombom>"),
    ] {
        let source = wrapped(&format!(
            "nova entrada: {ty} = {value}; nova muda saida: {ty} = {value}; \
             sussurro(\"mov {{saida}}, {{entrada}}\"; entrada entrada: r8 = entrada; \
             saida saida: r9 = saida);"
        ));
        common::render_backend_s_external_subset_nativo(&source)
            .unwrap_or_else(|failure| panic!("tipo {ty}: {failure}"));
    }
}

#[test]
fn tipos_sem_representacao_nao_entram_por_analogia() {
    for (ty, value) in [
        ("verso", r#""texto""#),
        ("lista<bombom>", "lista_criar()"),
        ("mapa<verso,bombom>", "mapa_criar()"),
    ] {
        let diagnostic = error(&wrapped(&format!(
            "nova x: {ty} = {value}; sussurro(\"nop {{x}}\"; entrada x: registrador = x);"
        )));
        assert!(
            diagnostic.contains("E-SEMANTIC-ASM-UNSUPPORTED-TYPE"),
            "{diagnostic}"
        );
    }
}

#[test]
fn constraints_direcoes_bindings_e_outputs_invalidos_tem_diagnostico() {
    let cases = [
        (
            r#"nova x: bombom = 1; sussurro("nop {x}"; entrada x: imediato = x);"#,
            "E-SEMANTIC-ASM-CONSTRAINT",
        ),
        (
            r#"nova x: bombom = 1; sussurro("nop {x}"; leitura x: registrador = x);"#,
            "E-SEMANTIC-ASM-DIRECTION",
        ),
        (
            r#"sussurro("nop {ausente}"; destroi(flags));"#,
            "E-SEMANTIC-ASM-UNKNOWN-OPERAND",
        ),
        (
            r#"nova x: bombom = 1; sussurro("nop {x}"; entrada x: r8 = x; entrada x: r9 = x);"#,
            "E-SEMANTIC-ASM-DUPLICATE-OPERAND",
        ),
        (
            r#"sussurro("mov {x}, 1"; saida x: r8 = 0);"#,
            "E-SEMANTIC-ASM-INVALID-OUTPUT",
        ),
    ];
    for (body, expected) in cases {
        let diagnostic = error(&wrapped(body));
        assert!(diagnostic.contains(expected), "{expected}: {diagnostic}");
    }
}

#[test]
fn operands_nao_contornam_politica_de_fonte() {
    for (template, expected) in [
        (".section .evil", "E-SEMANTIC-ASM-DIRECTIVE"),
        ("simbolo = 1", "E-SEMANTIC-ASM-SYMBOL-ASSIGN"),
        ("nome: nop", "E-SEMANTIC-ASM-NAMED-LABEL"),
    ] {
        let diagnostic = error(&wrapped(&format!(
            "sussurro(\"{template}\"; destroi(flags));"
        )));
        assert!(diagnostic.contains(expected), "{diagnostic}");
    }
}

#[test]
fn abi_recusa_stack_callee_saved_e_escape_de_envelope() {
    for (template, expected) in [
        ("mov rsp, rax", "E-SEMANTIC-ASM-ABI"),
        ("xor rbx, rbx", "E-SEMANTIC-ASM-ABI"),
        ("ret", "E-SEMANTIC-ASM-CONTROL-FLOW"),
        ("jmp destino", "E-SEMANTIC-ASM-CONTROL-FLOW"),
    ] {
        let diagnostic = error(&wrapped(&format!("sussurro(\"{template}\");")));
        assert!(diagnostic.contains(expected), "{diagnostic}");
    }
    common::parse_and_check(&wrapped(r#"sussurro("1: nop\njmp 1b");"#))
        .expect("controle local numérico permanece no envelope");
}

#[test]
fn interpretador_permanece_native_only_sem_emular_cpu() {
    let ast = common::parse(OPERAND_PROGRAM).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let program_ir = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&program_ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&program_ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    let diagnostic = interpreter::run_program(&machine)
        .expect_err("sussurro com operands deve falhar no interpretador")
        .to_string();
    assert!(
        diagnostic.contains("E-RUNTIME-SUSSURRO-NATIVO"),
        "{diagnostic}"
    );
}

#[test]
fn artifact_verification_cobre_assembly_com_bindings_reais() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };
    let asm =
        common::render_backend_s_external_subset_nativo(OPERAND_PROGRAM).expect("assembly D4");
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let checked = inline_asm::verify_native_artifact(&asm, &driver, artifacts.path())
        .expect("superfície ELF preservada");
    assert_eq!(checked.envelopes, 1);
}

#[test]
fn execucao_nativa_e_limitada_deterministica_e_sensivel_ao_binding() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let source = artifacts.path().join("d4_operands.pink");
    fs::write(&source, OPERAND_PROGRAM).expect("gravar fonte temporária");
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(artifacts.path())
        .arg(&source)
        .env("PINKER_RT_LIB", &runtime_lib)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("build nativo contido");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let executable = artifacts.path().join("d4_operands");
    let output = Command::new(executable)
        .logical_case("d4-sussurro-operands")
        .timeout(Duration::from_secs(5))
        .output()
        .expect("execução nativa contida");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "49\n");
}
