use pinker_v0::cfg_ir::{
    BasicBlockIR, FunctionCfgIR, GlobalConstCfgIR, InstructionCfgIR, OperandIR, ProgramCfgIR,
    TempIR, TerminatorIR,
};
use pinker_v0::cfg_ir_validate;
use pinker_v0::error::PinkerError;
use pinker_v0::ir::{BindingIR, LocalIR, TypeIR};
use pinker_v0::token::{Position, Span};

fn sp() -> Span {
    Span::new(Position::new(1, 1), Position::new(1, 1))
}

fn base_program(function: FunctionCfgIR) -> ProgramCfgIR {
    ProgramCfgIR {
        module_name: "main".to_string(),
        consts: vec![GlobalConstCfgIR {
            name: "LIMITE".to_string(),
            ty: TypeIR::Bombom,
            value: OperandIR::Int(10),
        }],
        functions: vec![function],
    }
}

fn base_function(ret_type: TypeIR, blocks: Vec<BasicBlockIR>) -> FunctionCfgIR {
    FunctionCfgIR {
        name: "principal".to_string(),
        params: vec![BindingIR {
            source_name: "a".to_string(),
            slot: "%a#0".to_string(),
            ty: TypeIR::Bombom,
        }],
        locals: vec![LocalIR {
            source_name: "x".to_string(),
            slot: "%x#0".to_string(),
            ty: TypeIR::Bombom,
            is_mut: true,
        }],
        ret_type,
        entry: "entry".to_string(),
        blocks,
        span: sp(),
    }
}

#[test]
fn cfg_valida_simples() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionCfgIR::Let {
                slot: "%x#0".to_string(),
                value: OperandIR::GlobalConst("LIMITE".to_string()),
            }],
            terminator: TerminatorIR::Return(Some(OperandIR::Local("%x#0".to_string()))),
        }],
    );
    assert!(cfg_ir_validate::validate_program(&base_program(function)).is_ok());
}

#[test]
fn falha_entry_ausente() {
    let mut function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "bloco0".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
        }],
    );
    function.entry = "nao_entry".to_string();
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_label_duplicado() {
    let function = base_function(
        TypeIR::Bombom,
        vec![
            BasicBlockIR {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Jump("entry".to_string()),
            },
            BasicBlockIR {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
            },
        ],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_jump_label_inexistente() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Jump("fim".to_string()),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_branch_label_inexistente() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Branch {
                cond: OperandIR::Bool(true),
                then_label: "then".to_string(),
                else_label: "else".to_string(),
            },
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_branch_condicao_invalida() {
    let function = base_function(
        TypeIR::Bombom,
        vec![
            BasicBlockIR {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Branch {
                    cond: OperandIR::Int(1),
                    then_label: "then".to_string(),
                    else_label: "else".to_string(),
                },
            },
            BasicBlockIR {
                label: "then".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Return(Some(OperandIR::Int(1))),
            },
            BasicBlockIR {
                label: "else".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
            },
        ],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_return_com_valor_em_nulo() {
    let function = base_function(
        TypeIR::Nulo,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(Some(OperandIR::Int(1))),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_return_vazio_em_funcao_com_retorno() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(None),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_call_nulo_com_destino_temporario() {
    let mut function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionCfgIR::Call {
                dest: Some(TempIR(0)),
                callee: "log".to_string(),
                args: vec![],
                ret_type: TypeIR::Nulo,
            }],
            terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
        }],
    );

    let log_fn = FunctionCfgIR {
        name: "log".to_string(),
        params: vec![],
        locals: vec![],
        ret_type: TypeIR::Nulo,
        entry: "entry".to_string(),
        blocks: vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(None),
        }],
        span: sp(),
    };

    function.name = "principal".to_string();
    let program = ProgramCfgIR {
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![log_fn, function],
    };

    assert!(matches!(
        cfg_ir_validate::validate_program(&program),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_referencia_slot_invalido() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionCfgIR::Assign {
                slot: "%nao_existe".to_string(),
                value: OperandIR::Int(1),
            }],
            terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_constante_global_invalida() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(Some(OperandIR::GlobalConst("X".to_string()))),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn falha_temporario_nao_definido() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(Some(OperandIR::Temp(TempIR(99)))),
        }],
    );
    assert!(matches!(
        cfg_ir_validate::validate_program(&base_program(function)),
        Err(PinkerError::CfgIrValidation { .. })
    ));
}

#[test]
fn politica_inalcancavel_e_erro() {
    let function = base_function(
        TypeIR::Bombom,
        vec![
            BasicBlockIR {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
            },
            BasicBlockIR {
                label: "dead".to_string(),
                instructions: vec![],
                terminator: TerminatorIR::Return(Some(OperandIR::Int(1))),
            },
        ],
    );
    let err = cfg_ir_validate::validate_program(&base_program(function)).unwrap_err();
    match err {
        PinkerError::CfgIrValidation { msg, .. } => assert!(msg.contains("inalcançáveis")),
        _ => panic!("esperado erro de validação CFG IR"),
    }
}

#[test]
fn cfg_invalida_nao_deve_ser_impressa() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: TerminatorIR::Return(None),
        }],
    );
    let cfg = base_program(function);

    let output = match cfg_ir_validate::validate_program(&cfg) {
        Ok(()) => format!(
            "=== CFG IR ===\n{}",
            pinker_v0::cfg_ir::render_program(&cfg)
        ),
        Err(_) => String::new(),
    };

    assert_eq!(output, "");
}

#[test]
fn erro_cfg_tem_contexto_padronizado() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionCfgIR::Assign {
                slot: "%x#0".to_string(),
                value: OperandIR::Bool(true),
            }],
            terminator: TerminatorIR::Return(Some(OperandIR::Int(0))),
        }],
    );

    let err = cfg_ir_validate::validate_program(&base_program(function))
        .unwrap_err()
        .to_string();
    assert!(err.contains("tipo incompatível em slot '%x#0'"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
    assert!(err.contains("instr='let/assign'"));
    assert!(err.contains("esperado=Bombom, recebido=Logica"));
}
