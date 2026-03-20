use pinker_v0::error::PinkerError;
use pinker_v0::ir::{
    BindingIR, BlockIR, FunctionIR, InstructionIR, LocalIR, ProgramIR, TypeIR, ValueIR,
};
use pinker_v0::ir_validate;
use pinker_v0::token::{Position, Span};

fn sp() -> Span {
    Span::new(Position::new(1, 1), Position::new(1, 1))
}

fn base_function(ret_type: TypeIR, instructions: Vec<InstructionIR>) -> FunctionIR {
    FunctionIR {
        name: "principal".to_string(),
        params: vec![],
        locals: vec![LocalIR {
            source_name: "x".to_string(),
            slot: "%x#0".to_string(),
            ty: TypeIR::Bombom,
            is_mut: true,
        }],
        ret_type,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions,
            span: sp(),
        },
        span: sp(),
    }
}

#[test]
fn valida_ir_simples_valida() {
    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![base_function(
            TypeIR::Bombom,
            vec![InstructionIR::Return {
                value: Some(ValueIR::Int(0)),
                span: sp(),
            }],
        )],
    };

    assert!(ir_validate::validate_program(&program).is_ok());
}

#[test]
fn falha_retorno_invalido() {
    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![base_function(
            TypeIR::Nulo,
            vec![InstructionIR::Return {
                value: Some(ValueIR::Int(1)),
                span: sp(),
            }],
        )],
    };

    let err = ir_validate::validate_program(&program).unwrap_err();
    match err {
        PinkerError::IrValidation { msg, .. } => assert!(msg.contains("função nulo")),
        _ => panic!("esperado erro de validação IR"),
    }
}

#[test]
fn falha_condicao_if_invalida() {
    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![base_function(
            TypeIR::Bombom,
            vec![InstructionIR::If {
                condition: ValueIR::Int(1),
                then_block: BlockIR {
                    label: "then_0".to_string(),
                    instructions: vec![InstructionIR::Return {
                        value: Some(ValueIR::Int(1)),
                        span: sp(),
                    }],
                    span: sp(),
                },
                else_block: Some(BlockIR {
                    label: "else_1".to_string(),
                    instructions: vec![InstructionIR::Return {
                        value: Some(ValueIR::Int(0)),
                        span: sp(),
                    }],
                    span: sp(),
                }),
                span: sp(),
            }],
        )],
    };

    assert!(matches!(
        ir_validate::validate_program(&program),
        Err(PinkerError::IrValidation { .. })
    ));
}

#[test]
fn falha_chamada_invalida() {
    let callee = FunctionIR {
        name: "f".to_string(),
        params: vec![BindingIR {
            source_name: "a".to_string(),
            slot: "%a#0".to_string(),
            ty: TypeIR::Bombom,
        }],
        locals: vec![],
        ret_type: TypeIR::Bombom,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionIR::Return {
                value: Some(ValueIR::Local("%a#0".to_string())),
                span: sp(),
            }],
            span: sp(),
        },
        span: sp(),
    };

    let caller = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::Call {
                callee: "f".to_string(),
                args: vec![ValueIR::Bool(true)],
                ret_type: TypeIR::Bombom,
            }),
            span: sp(),
        }],
    );

    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![callee, caller],
    };

    assert!(matches!(
        ir_validate::validate_program(&program),
        Err(PinkerError::IrValidation { .. })
    ));
}

#[test]
fn falha_uso_incorreto_de_nulo() {
    let void_fn = FunctionIR {
        name: "log".to_string(),
        params: vec![],
        locals: vec![],
        ret_type: TypeIR::Nulo,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionIR::Return {
                value: None,
                span: sp(),
            }],
            span: sp(),
        },
        span: sp(),
    };

    let caller = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::Call {
                callee: "log".to_string(),
                args: vec![],
                ret_type: TypeIR::Nulo,
            }),
            span: sp(),
        }],
    );

    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![void_fn, caller],
    };

    assert!(matches!(
        ir_validate::validate_program(&program),
        Err(PinkerError::IrValidation { .. })
    ));
}

#[test]
fn falha_bloco_malformado() {
    let mut function = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::Int(0)),
            span: sp(),
        }],
    );
    function.entry.label = "".to_string();
    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![function],
    };

    assert!(matches!(
        ir_validate::validate_program(&program),
        Err(PinkerError::IrValidation { .. })
    ));
}

#[test]
fn erro_ir_tem_contexto_padronizado() {
    let program = ProgramIR {
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![base_function(
            TypeIR::Bombom,
            vec![InstructionIR::Assign {
                slot: "%x#0".to_string(),
                value: ValueIR::Bool(true),
                span: sp(),
            }],
        )],
    };

    let err = ir_validate::validate_program(&program)
        .unwrap_err()
        .to_string();
    assert!(err.contains("atribuição IR com tipo incompatível"));
    assert!(err.contains("função 'principal', bloco 'entry'"));
    assert!(err.contains("instr='let/assign'"));
    assert!(err.contains("esperado=Bombom, recebido=Logica"));
}
