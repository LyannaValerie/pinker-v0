mod common;

use common::{render_cli_machine_output, render_machine};
use pinker_v0::abstract_machine::{
    MachineBlock, MachineFunction, MachineInstr, MachineProgram, MachineTerminator,
};
use pinker_v0::abstract_machine_validate;
use pinker_v0::ir::TypeIR;
use std::collections::HashMap;

fn validate(function: MachineFunction) -> Result<(), String> {
    let program = MachineProgram {
        union_types: vec![],
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
        slot_types: HashMap::new(),
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

// @pinker-nav:start evidence.machine.rendering-valid-program
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Renders a simple program accepted by the path present.
#[test]
fn stack_valida_programa_simples() {
    let out = render_machine("pacote main; carinho principal() -> bombom { mimo 1 + 2; }").unwrap();
    assert!(out.contains("vm add"));
}

// @pinker-nav:end evidence.machine.rendering-valid-program
// @pinker-nav:start evidence.machine.validation-operator-underflow
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Builds unary and binary operations manually and expects refusal due to underflow.
#[test]
fn stack_underflow_unaria() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::Neg { ty: TypeIR::Bombom }],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("underflow em operação unária"));
}

#[test]
fn stack_underflow_binaria() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![
            MachineInstr::PushInt(1),
            MachineInstr::Add { ty: TypeIR::Bombom },
        ],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("underflow em operação binária"));
}

// @pinker-nav:end evidence.machine.validation-operator-underflow
// @pinker-nav:start evidence.machine.validation-calls-arity-and-underflow
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Builds calls manually and expects refusal of underflow or invalid arity in the cases present.
#[test]
fn stack_underflow_call() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "soma".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec!["%x#0".to_string(), "%y#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([
                    ("%x#0".to_string(), TypeIR::Bombom),
                    ("%y#0".to_string(), TypeIR::Bombom),
                ]),
                blocks: vec![block(
                    "entry",
                    vec![
                        MachineInstr::LoadSlot("%x#0".to_string()),
                        MachineInstr::LoadSlot("%y#0".to_string()),
                        MachineInstr::Add { ty: TypeIR::Bombom },
                    ],
                    MachineTerminator::Ret,
                )],
            },
            fn_bombom(vec![block(
                "entry",
                vec![
                    MachineInstr::PushInt(1),
                    MachineInstr::Call {
                        callee: "soma".to_string(),
                        argc: 2,
                        identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                    },
                ],
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
fn stack_call_aridade_invalida() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "soma".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec!["%x#0".to_string(), "%y#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([
                    ("%x#0".to_string(), TypeIR::Bombom),
                    ("%y#0".to_string(), TypeIR::Bombom),
                ]),
                blocks: vec![block(
                    "entry",
                    vec![
                        MachineInstr::LoadSlot("%x#0".to_string()),
                        MachineInstr::LoadSlot("%y#0".to_string()),
                        MachineInstr::Add { ty: TypeIR::Bombom },
                    ],
                    MachineTerminator::Ret,
                )],
            },
            fn_bombom(vec![block(
                "entry",
                vec![
                    MachineInstr::PushInt(1),
                    MachineInstr::PushInt(2),
                    MachineInstr::Call {
                        callee: "soma".to_string(),
                        argc: 1,
                        identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                    },
                ],
                MachineTerminator::Ret,
            )]),
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("call com aridade inválida"));
}

#[test]
fn stack_call_void_aridade_invalida() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "log".to_string(),
                ret_type: TypeIR::Nulo,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Bombom)]),
                blocks: vec![block("entry", vec![], MachineTerminator::RetVoid)],
            },
            fn_bombom(vec![block(
                "entry",
                vec![
                    MachineInstr::PushInt(1),
                    MachineInstr::CallVoid {
                        callee: "log".to_string(),
                        argc: 0,
                        identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                    },
                    MachineInstr::PushInt(0),
                ],
                MachineTerminator::Ret,
            )]),
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("call_void com aridade inválida"));
}

#[test]
fn stack_underflow_call_void() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "log".to_string(),
                ret_type: TypeIR::Nulo,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Bombom)]),
                blocks: vec![block("entry", vec![], MachineTerminator::RetVoid)],
            },
            fn_bombom(vec![block(
                "entry",
                vec![MachineInstr::CallVoid {
                    callee: "log".to_string(),
                    argc: 1,
                    identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                }],
                MachineTerminator::Ret,
            )]),
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("underflow em call_void"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
    assert!(err.contains("instr='call_void log, 1'"));
}

// @pinker-nav:end evidence.machine.validation-calls-arity-and-underflow
// @pinker-nav:start evidence.machine.validation-diagnostic-format
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Inspects the contextual format of the validation diagnostic present.
#[test]
fn erro_machine_mantem_formato_padrao_de_contexto() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::PushBool(true)],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();

    assert!(err.contains("ret com tipo incompatível"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
    assert!(err.contains("term='ret'"));
    assert!(err.contains("esperado=bombom, recebido=lógica"));
}

// @pinker-nav:end evidence.machine.validation-diagnostic-format
// @pinker-nav:start evidence.machine.validation-branch
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Builds branches manually and expects refusal due to a missing or incompatible condition.
#[test]
fn stack_branch_sem_condicao() {
    let err = validate(fn_bombom(vec![
        block(
            "entry",
            vec![],
            MachineTerminator::BrTrue {
                then_label: "then_0".to_string(),
                else_label: "else_1".to_string(),
            },
        ),
        block(
            "then_0",
            vec![MachineInstr::PushInt(1)],
            MachineTerminator::Ret,
        ),
        block(
            "else_1",
            vec![MachineInstr::PushInt(0)],
            MachineTerminator::Ret,
        ),
    ]))
    .unwrap_err();
    assert!(err.contains("underflow em br_true"));
    assert!(err.contains("term='br_true then_0, else_1'"));
}

#[test]
fn stack_branch_tipo_incompativel() {
    let err = validate(fn_bombom(vec![
        block(
            "entry",
            vec![MachineInstr::PushInt(1)],
            MachineTerminator::BrTrue {
                then_label: "then_0".to_string(),
                else_label: "else_1".to_string(),
            },
        ),
        block(
            "then_0",
            vec![MachineInstr::PushInt(1)],
            MachineTerminator::Ret,
        ),
        block(
            "else_1",
            vec![MachineInstr::PushInt(0)],
            MachineTerminator::Ret,
        ),
    ]))
    .unwrap_err();
    assert!(err.contains("br_true requer condição lógica"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
    assert!(err.contains("term='br_true then_0, else_1'"));
}

// @pinker-nav:end evidence.machine.validation-branch
// @pinker-nav:start evidence.machine.rendering-valid-branch
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Renders the type-compatible branch present.
#[test]
fn stack_branch_tipo_compativel() {
    let out = render_machine(
        "pacote main; carinho principal() -> bombom { talvez verdade { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert!(out.contains("term br_true"));
}

// @pinker-nav:end evidence.machine.rendering-valid-branch
// @pinker-nav:start evidence.machine.validation-return
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Builds returns manually and expects refusal due to a missing value or an incompatible type.
#[test]
fn stack_ret_sem_valor() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("ret requer exatamente um valor na pilha"));
}

#[test]
fn stack_ret_tipo_incompativel() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::PushBool(true)],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("ret com tipo incompatível"));
    assert!(err.contains("term='ret'"));
    assert!(err.contains("esperado=bombom, recebido=lógica"));
}

// @pinker-nav:end evidence.machine.validation-return
// @pinker-nav:start evidence.machine.rendering-valid-return
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Renders the type-compatible return present.
#[test]
fn stack_ret_tipo_compativel() {
    let out = render_machine("pacote main; carinho principal() -> bombom { mimo 7; }").unwrap();
    assert!(out.contains("term ret"));
}

// @pinker-nav:end evidence.machine.rendering-valid-return
// @pinker-nav:start evidence.machine.validation-stack-retvoid-and-merges
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Expects refusal of residual stack on an empty return and of inconsistent heights at a flow merge.
#[test]
fn stack_ret_void_pilha_suja() {
    let function = MachineFunction {
        name: "log".to_string(),
        ret_type: TypeIR::Nulo,
        params: vec![],
        locals: vec![],
        slot_types: HashMap::new(),
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
        block(
            "a",
            vec![MachineInstr::PushInt(1)],
            MachineTerminator::Jmp("join".to_string()),
        ),
        block("b", vec![], MachineTerminator::Jmp("join".to_string())),
        block(
            "join",
            vec![MachineInstr::PushInt(7)],
            MachineTerminator::Ret,
        ),
    ]))
    .unwrap_err();
    assert!(err.contains("altura de pilha inconsistente entre predecessores"));
}

// @pinker-nav:end evidence.machine.validation-stack-retvoid-and-merges
// @pinker-nav:start evidence.machine.validation-slot-existence
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Expects refusal of load and store for nonexistent slots in the constructed programs.
#[test]
fn stack_load_slot_invalido() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![MachineInstr::LoadSlot("%x#0".to_string())],
        MachineTerminator::Ret,
    )]))
    .unwrap_err();
    assert!(err.contains("load_slot para slot inexistente"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
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
    assert!(err.contains("função 'principal', bloco 'entry'"));
}

// @pinker-nav:end evidence.machine.validation-slot-existence
// @pinker-nav:start evidence.machine.validation-typed-slots
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Validates the cases present of flow through typed parameters and locals and rejects an incompatible store.
#[test]
fn stack_load_slot_param_tipado_fluxo_valido() {
    let function = MachineFunction {
        name: "f".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec!["%p#0".to_string()],
        locals: vec![],
        slot_types: HashMap::from([("%p#0".to_string(), TypeIR::Logica)]),
        blocks: vec![
            block(
                "entry",
                vec![MachineInstr::LoadSlot("%p#0".to_string())],
                MachineTerminator::BrTrue {
                    then_label: "then_0".to_string(),
                    else_label: "else_1".to_string(),
                },
            ),
            block(
                "then_0",
                vec![MachineInstr::PushInt(1)],
                MachineTerminator::Ret,
            ),
            block(
                "else_1",
                vec![MachineInstr::PushInt(0)],
                MachineTerminator::Ret,
            ),
        ],
    };

    assert!(validate(function).is_ok());
}

#[test]
fn stack_load_slot_local_tipado_fluxo_valido() {
    let function = MachineFunction {
        name: "f".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec![],
        locals: vec!["%x#0".to_string()],
        slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Bombom)]),
        blocks: vec![block(
            "entry",
            vec![
                MachineInstr::PushInt(10),
                MachineInstr::StoreSlot("%x#0".to_string()),
                MachineInstr::LoadSlot("%x#0".to_string()),
            ],
            MachineTerminator::Ret,
        )],
    };

    assert!(validate(function).is_ok());
}

#[test]
fn stack_store_slot_tipado_incompativel() {
    let function = MachineFunction {
        name: "f".to_string(),
        ret_type: TypeIR::Nulo,
        params: vec![],
        locals: vec!["%x#0".to_string()],
        slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Bombom)]),
        blocks: vec![block(
            "entry",
            vec![
                MachineInstr::PushBool(true),
                MachineInstr::StoreSlot("%x#0".to_string()),
            ],
            MachineTerminator::RetVoid,
        )],
    };

    let err = validate(function).unwrap_err();
    assert!(err.contains("store_slot com tipo incompatível"));
    assert!(err.contains("slot='%x#0'"));
    assert!(err.contains("esperado=bombom, recebido=lógica"));
}

// @pinker-nav:end evidence.machine.validation-typed-slots
// @pinker-nav:start evidence.machine.validation-types-operations-and-return
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Expects refusal of a logical parameter in arithmetic and of an incompatible return.
#[test]
fn stack_aritmetica_invalida_com_parametro_logico() {
    let function = MachineFunction {
        name: "f".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec!["%p#0".to_string()],
        locals: vec![],
        slot_types: HashMap::from([("%p#0".to_string(), TypeIR::Logica)]),
        blocks: vec![block(
            "entry",
            vec![
                MachineInstr::LoadSlot("%p#0".to_string()),
                MachineInstr::PushInt(1),
                MachineInstr::Add { ty: TypeIR::Bombom },
            ],
            MachineTerminator::Ret,
        )],
    };

    let err = validate(function).unwrap_err();
    assert!(err.contains("tipo inválido em operação binária"));
}

#[test]
fn stack_ret_invalido_com_parametro_logico() {
    let function = MachineFunction {
        name: "f".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec!["%p#0".to_string()],
        locals: vec![],
        slot_types: HashMap::from([("%p#0".to_string(), TypeIR::Logica)]),
        blocks: vec![block(
            "entry",
            vec![MachineInstr::LoadSlot("%p#0".to_string())],
            MachineTerminator::Ret,
        )],
    };

    let err = validate(function).unwrap_err();
    assert!(err.contains("ret com tipo incompatível"));
}

// @pinker-nav:end evidence.machine.validation-types-operations-and-return
// @pinker-nav:start evidence.machine.validation-call-types
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Expects refusal of incompatible arguments in calls with and without a return.
#[test]
fn stack_call_tipo_argumento_incompativel() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "usa_int".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Bombom)]),
                blocks: vec![block(
                    "entry",
                    vec![MachineInstr::LoadSlot("%x#0".to_string())],
                    MachineTerminator::Ret,
                )],
            },
            MachineFunction {
                name: "f".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![block(
                    "entry",
                    vec![
                        MachineInstr::PushBool(true),
                        MachineInstr::Call {
                            callee: "usa_int".to_string(),
                            argc: 1,
                            identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                        },
                    ],
                    MachineTerminator::Ret,
                )],
            },
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("call com tipo de argumento incompatível"));
    assert!(err.contains("callee='usa_int'"));
    assert!(err.contains("esperado=bombom, recebido=lógica"));
}

#[test]
fn stack_call_void_tipo_argumento_incompativel() {
    let program = MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "usa_logica".to_string(),
                ret_type: TypeIR::Nulo,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                slot_types: HashMap::from([("%x#0".to_string(), TypeIR::Logica)]),
                blocks: vec![block("entry", vec![], MachineTerminator::RetVoid)],
            },
            MachineFunction {
                name: "f".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![block(
                    "entry",
                    vec![
                        MachineInstr::PushInt(1),
                        MachineInstr::CallVoid {
                            callee: "usa_logica".to_string(),
                            argc: 1,
                            identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
                        },
                        MachineInstr::PushInt(0),
                    ],
                    MachineTerminator::Ret,
                )],
            },
        ],
    };

    let err = abstract_machine_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("call_void com tipo de argumento incompatível"));
    assert!(err.contains("callee='usa_logica'"));
    assert!(err.contains("esperado=lógica, recebido=bombom"));
}

// @pinker-nav:end evidence.machine.validation-call-types
// @pinker-nav:start evidence.machine.rendering-valid-cases
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Renders the valid cases present of a temporary in if/else and a call with a return.
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

// @pinker-nav:end evidence.machine.rendering-valid-cases
// @pinker-nav:start evidence.machine.validation-invalid-program
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Validates directly an invalid machine built by hand and observes the refusal present.
#[test]
fn machine_invalida_nao_e_impressa() {
    let function = MachineFunction {
        name: "principal".to_string(),
        ret_type: TypeIR::Bombom,
        params: vec![],
        locals: vec![],
        slot_types: HashMap::new(),
        blocks: vec![block(
            "entry",
            vec![MachineInstr::Neg { ty: TypeIR::Bombom }],
            MachineTerminator::Ret,
        )],
    };
    let validation = validate(function);
    assert!(validation.is_err());
}

// @pinker-nav:end evidence.machine.validation-invalid-program
// @pinker-nav:start evidence.machine.rendering-cli-golden
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Inspects fragments of the CLI renderer for a valid non-trivial program.
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
// @pinker-nav:end evidence.machine.rendering-cli-golden

// @pinker-nav:start evidence.machine.validation-trato-objects-phase244
// @pinker-nav:domain machine
// @pinker-nav:layer evidence
// @pinker-nav:summary Manually validates the stack effects of Phase 244 materialization and dynamic dispatch, including a nominal receiver, arguments, a null return and vtable invariants.

fn fase244_fn_com_objeto(code: Vec<MachineInstr>, ret_type: TypeIR) -> MachineFunction {
    MachineFunction {
        name: "principal".to_string(),
        ret_type,
        params: vec![],
        locals: vec!["%objeto#0".to_string()],
        slot_types: HashMap::from([("%objeto#0".to_string(), TypeIR::TraitObject)]),
        blocks: vec![block(
            "entry",
            code,
            if ret_type == TypeIR::Nulo {
                MachineTerminator::RetVoid
            } else {
                MachineTerminator::Ret
            },
        )],
    }
}

#[test]
fn fase244_machine_stack_aceita_materializacao_e_chamada_com_retorno() {
    let function = fase244_fn_com_objeto(
        vec![
            MachineInstr::PushInt(21),
            MachineInstr::MakeTraitObject {
                trait_name: "Medivel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
            },
            MachineInstr::StoreSlot("%objeto#0".to_string()),
            MachineInstr::PushInt(2),
            MachineInstr::LoadSlot("%objeto#0".to_string()),
            MachineInstr::TraitCall {
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                argc: 1,
                param_types: vec![TypeIR::Bombom],
                ret_type: TypeIR::Bombom,
            },
        ],
        TypeIR::Bombom,
    );

    assert!(validate(function).is_ok());
}

#[test]
fn fase244_machine_stack_aceita_chamada_nula_sem_valor_residual() {
    let function = fase244_fn_com_objeto(
        vec![
            MachineInstr::PushInt(35),
            MachineInstr::MakeTraitObject {
                trait_name: "Observavel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec!["__impl_10_Observavel_6_bombom_observar".to_string()],
            },
            MachineInstr::StoreSlot("%objeto#0".to_string()),
            MachineInstr::PushInt(7),
            MachineInstr::LoadSlot("%objeto#0".to_string()),
            MachineInstr::TraitCall {
                trait_name: "Observavel".to_string(),
                method_name: "observar".to_string(),
                method_slot: 0,
                method_count: 1,
                argc: 1,
                param_types: vec![TypeIR::Bombom],
                ret_type: TypeIR::Nulo,
            },
        ],
        TypeIR::Nulo,
    );

    assert!(validate(function).is_ok());
}

#[test]
fn fase244_machine_stack_rejeita_receiver_comum() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![
            MachineInstr::PushInt(42),
            MachineInstr::TraitCall {
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                argc: 0,
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            },
        ],
        MachineTerminator::Ret,
    )]))
    .expect_err("receiver comum deve ser recusado");

    assert!(
        err.contains("trait_call exige objeto de trato no topo"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_machine_stack_rejeita_vtable_vazia() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![
            MachineInstr::PushInt(1),
            MachineInstr::MakeTraitObject {
                trait_name: "Medivel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec![],
            },
        ],
        MachineTerminator::Ret,
    )]))
    .expect_err("vtable vazia deve ser recusada");

    assert!(
        err.contains("make_trait_object exige vtable não vazia"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_machine_stack_rejeita_slot_fora_da_vtable() {
    let err = validate(fn_bombom(vec![block(
        "entry",
        vec![
            MachineInstr::PushInt(42),
            MachineInstr::TraitCall {
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 1,
                method_count: 1,
                argc: 0,
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            },
        ],
        MachineTerminator::Ret,
    )]))
    .expect_err("slot fora da vtable deve ser recusado");

    assert!(
        err.contains("slot fora da vtable"),
        "diagnóstico inesperado: {err}"
    );
}

// @pinker-nav:end evidence.machine.validation-trato-objects-phase244
