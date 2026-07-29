mod common;

use pinker_v0::{
    abstract_machine, abstract_machine_validate, backend_s, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, interpreter, ir, ir_validate, semantic,
};

fn lower(
    source: &str,
) -> (
    ir::ProgramIR,
    cfg_ir::ProgramCfgIR,
    instr_select::SelectedProgram,
    abstract_machine::MachineProgram,
) {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let ir = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    (ir, cfg, selected, machine)
}

#[test]
fn fase247_sussurro_atravessa_pipeline_e_emite_wrappers_balanceados() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jmp 1b");
            mimo 0;
        }
    "#;
    let (ir, cfg, selected, machine) = lower(source);
    assert!(ir::render_program(&ir).contains("inline_asm"));
    assert!(cfg_ir::render_program(&cfg).contains("inline_asm"));
    assert!(instr_select::render_program(&selected).contains("inline_asm"));
    assert!(abstract_machine::render_program(&machine).contains("inline_asm"));

    let asm = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    assert_eq!(asm.matches(".intel_syntax noprefix").count(), 1);
    assert_eq!(asm.matches(".att_syntax prefix").count(), 1);
    assert!(asm.contains("1: nop"));
    assert!(asm.contains("jmp 1b"));
}

#[test]
fn fase247_interpretador_rejeita_execucao_sem_noop() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop");
            mimo 0;
        }
    "#;
    let (_, _, _, machine) = lower(source);
    let error = interpreter::run_program(&machine)
        .expect_err("sussurro não executa no interpretador")
        .to_string();
    assert!(error.contains("E-RUNTIME-SUSSURRO-NATIVO"), "{error}");
}

#[test]
fn fase247_rejeita_diretiva_que_altera_secao() {
    let ast = common::parse(include_str!(
        "../examples/fase247_sussurro_diretiva_invalido.pink"
    ))
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("diretiva deve falhar")
        .to_string();
    assert!(error.contains(".section"), "{error}");
}

#[test]
fn fase248_rejeita_encaixe_inexaustivo() {
    let ast = common::parse(include_str!(
        "../examples/fase248_uniao_inexaustiva_invalido.pink"
    ))
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("encaixe inexaustivo deve falhar")
        .to_string();
    assert!(error.contains("exaustivo"), "{error}");
}

#[test]
fn fase248_registry_estrutural_e_preservado_em_todas_as_camadas() {
    let source = include_str!("../examples/fase248_unioes_estruturais_valido.pink");
    let (ir, cfg, selected, machine) = lower(source);
    assert_eq!(ir.union_types.len(), 1);
    let union = &ir.union_types[0];
    assert_eq!(union.id.0, 0);
    assert_eq!(union.members.len(), 2);
    assert_eq!(union.members[0].tag, 0);
    assert_eq!(union.members[1].tag, 1);
    assert_eq!(cfg.union_types, ir.union_types);
    assert_eq!(selected.union_types, ir.union_types);
    assert_eq!(machine.union_types, ir.union_types);

    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar união");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn fase248_ordem_textual_produz_mesma_identidade() {
    let source = r#"
        pacote main;
        carinho aceitar(a: uniao<u8, verso>) -> uniao<verso, u8> { mimo a; }
        carinho principal() -> bombom {
            nova x: uniao<verso, u8> = (7 virar u8) virar uniao<u8, verso>;
            nova y: uniao<u8, verso> = aceitar(x);
            encaixe y {
                caso verso(t) { falar(t); }
                caso u8(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let (ir, _, _, machine) = lower(source);
    assert_eq!(ir.union_types.len(), 1);
    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn fase248_rejeita_menos_de_dois_membros_canonicos() {
    let ast = common::parse(
        r#"pacote main; carinho principal() -> bombom {
            nova x: uniao<u8, u8> = 1;
            mimo 0;
        }"#,
    )
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("duplicata canonical deve colapsar")
        .to_string();
    assert!(error.contains("dois membros"), "{error}");
}

#[test]
fn fase248_rejeita_operacoes_sem_contrato_observavel() {
    let cases = [
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                nova y: logica = x == x;
                mimo 0;
            }"#,
            "igualdade e desigualdade de união",
        ),
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                falar(x);
                mimo 0;
            }"#,
            "'falar' não suporta tipo 'uniao'",
        ),
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                nova bruto: u64 = x virar u64;
                mimo bruto;
            }"#,
            "downcast de união fora de 'encaixe'",
        ),
    ];
    for (source, expected) in cases {
        let ast = common::parse(source).expect("parse");
        let error = semantic::check_program(&ast)
            .expect_err("operação sobre união deve falhar")
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}
