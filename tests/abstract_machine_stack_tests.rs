mod common;

use common::{render_cli_machine_output, render_machine};
use pinker_v0::abstract_machine::{
    MachineBlock, MachineFunction, MachineInstr, MachineProgram, MachineTerminator,
};
use pinker_v0::abstract_machine_validate;
use pinker_v0::ir::TypeIR;

fn validate(function: MachineFunction) -> Result<(), String> {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![function],
    };

    abstract_machine_validate::validate_program(&program).map_err(|e| e.to_string())
}

fn fn_bombom(blocks: Vec<MachineBlock>) -> MachineFunction {
    MachineFunction {
        name: "principal".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec![],
        locals: vec![],
        blocks,
    }
}

fn block(label: &str, code: Vec<MachineInstr>, term: MachineTerminator) -> MachineBlock {
    MachineBlock {
        label: label.to_string(),
        code,
        terminator: term,
    }
}

#[test]
fn stack_valida_programa_simples() {
    let out = render_machine("pacote main; carinho principal() -> bombom { mimo 1 + 2; }").unwrap();
    assert!(out.contains("vm add"));
}

#[test]
fn stack_underflow_unaria() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::Neg],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("underflow em operação unária"));
}

#[test]
fn stack_underflow_binaria() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::PushInt(1), MachineInstr::Add],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("underflow em operação binária"));
}

#[test]
fn stack_underflow_call() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "soma".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec!["%x#0".to_string(), "%y#0".to_string()],
                locals: vec![],
                blocks: vec![block(
                    "entry",
                    vec![
                        MachineInstr::LoadSlot("%x#0".to_string()),
                        MachineInstr::LoadSlot("%y#0".to_string()),
                        MachineInstr::Add,
                    ],
                    MachineTerminator::Ret,
                )],
            },
            fn_bombom(vec![block(
                "entry",
                vec![MachineInstr::PushInt(1), MachineInstr::Call {
                    callee: "soma".to_string(),
                    argc: 2,
                }],
                MachineTerminator::Ret,
            )]),
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("underflow em call"));
}

#[test]
fn stack_underflow_call_void() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "log".to_string(),
                ret_type: TypeIR::Nulo,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                blocks: vec![block("entry", vec![], MachineTerminator::RetVoid)],
            },
            fn_bombom(vec![block(
                "entry",
                vec![MachineInstr::CallVoid {
                    callee: "log".to_string(),
                    argc: 1,
                }],
                MachineTerminator::Ret,
            )]),
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("underflow em call_void"));
}

#[test]
fn stack_branch_sem_condicao() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![],
        MachineTerminator::BrTrue {
            then_label: "then_0".to_string(),
            else_label: "else_1".to_string(),
        },
    ),
    block("then_0", vec![MachineInstr::PushInt(1)], MachineTerminator::Ret),
    block("else_1", vec![MachineInstr::PushInt(0)], MachineTerminator::Ret),
    ]))
    .unwrap_err();
    assert!(err.contains("underflow em br_true"));
}

#[test]
fn stack_ret_sem_valor() {
    let err = validate(fn_bombom(vec![block("entry", vec![], MachineTerminator::Ret)]))
        .unwrap_err();
    assert!(err.contains("ret requer exatamente um valor na pilha"));
}

#[test]
fn stack_ret_void_pilha_suja() {
    let function = MachineFunction {
        name: "log".to_string(),
        ret_type: TypeIR::Nulo,
        params: vec![],
        locals: vec![],
        blocks: vec![block(
            "entry",
            vec![MachineInstr::PushInt(1)],
            MachineTerminator::RetVoid,
        )],
    };
    let err = validate(function).unwrap_err();
    assert!(err.contains("ret_void requer pilha vazia"));
}

#[test]
fn stack_altura_inconsistente_entre_predecessores() {
    let err = validate(fn_bombom(vec![
        block(
            "entry",
            vec![MachineInstr::PushBool(true)],
            MachineTerminator::BrTrue {
                then_label: "a".to_string(),
                else_label: "b".to_string(),
            },
        ),
        block("a", vec![MachineInstr::PushInt(1)], MachineTerminator::Jmp("join".to_string())),
        block("b", vec![], MachineTerminator::Jmp("join".to_string())),
        block("join", vec![MachineInstr::PushInt(7)], MachineTerminator::Ret),
    ]))
    .unwrap_err();
    assert!(err.contains("altura de pilha inconsistente entre predecessores"));
}

#[test]
fn stack_load_slot_invalido() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::LoadSlot("%x#0".to_string())],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("load_slot para slot inexistente"));
}

#[test]
fn stack_store_slot_invalido() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![
            MachineInstr::PushInt(1),
            MachineInstr::StoreSlot("x".to_string()),
            MachineInstr::PushInt(0),
        ],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("store_slot para slot inválido"));
}

#[test]
fn stack_valido_temporario_if_else() {
    let out = render_machine(
        "pacote main; carinho principal() -> bombom { talvez verdade { mimo 1 + 2; } senao { mimo 3 + 4; } }",
    )
    .unwrap();
    assert!(out.contains("term br_true"));
    assert!(out.contains("vm store_slot %t0") || out.contains("vm store_slot %t1"));
}

#[test]
fn stack_valido_call_retorno() {
    let out = render_machine(
        "pacote main; carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; } carinho principal() -> bombom { mimo soma(1, 2); }",
    )
    .unwrap();
    assert!(out.contains("vm call soma, 2"));
    assert!(out.contains("term ret"));
}

#[test]
fn machine_invalida_nao_e_impressa() {
    let function = MachineFunction {
        name: "principal".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec![],
        locals: vec![],
        blocks: vec![block("entry", vec![MachineInstr::Neg], MachineTerminator::Ret)],
    };
    let validation = validate(function);
    assert!(validation.is_err());
}

#[test]
fn golden_machine_nao_trivial_valido() {
    let code = "
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom {
  nova a = 2;
  nova b = 3;
  talvez verdade {
    mimo soma(a, b);
  } senao {
    mimo a;
  }
}";

    let out = render_cli_machine_output(code).unwrap();
    assert!(out.contains("=== MACHINE ==="));
    assert!(out.contains("func soma:"));
    assert!(out.contains("term br_true then_0, else_1"));
    assert!(out.contains("Análise semântica concluída sem erros."));
}
