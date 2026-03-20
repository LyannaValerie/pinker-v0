use pinker_v0::backend_text::{
    BackendTextBlock, BackendTextFunction, BackendTextGlobal, BackendTextInstruction,
    BackendTextProgram, BackendTextTerminator,
};
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir::{OperandIR, TempIR};
use pinker_v0::error::PinkerError;
use pinker_v0::ir::{BinaryOpIR, TypeIR};

fn valid_program() -> BackendTextProgram {
    BackendTextProgram {
        is_freestanding: false,
        module_name: "main".to_string(),
        globals: vec![BackendTextGlobal {
            name: "LIMITE".to_string(),
            value: OperandIR::Int(10),
        }],
        functions: vec![BackendTextFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["%x#0".to_string()],
            blocks: vec![BackendTextBlock {
                label: "entry".to_string(),
                instructions: vec![BackendTextInstruction::Mov {
                    dest: "%x#0".to_string(),
                    src: OperandIR::GlobalConst("LIMITE".to_string()),
                }],
                terminator: BackendTextTerminator::Return(Some(OperandIR::Local(
                    "%x#0".to_string(),
                ))),
            }],
        }],
    }
}

#[test]
fn backend_text_valido_simples() {
    assert!(backend_text_validate::validate_program(&valid_program()).is_ok());
}

#[test]
fn falha_sem_entry() {
    let mut p = valid_program();
    p.functions[0].blocks[0].label = "b0".to_string();
    assert!(matches!(
        backend_text_validate::validate_program(&p),
        Err(PinkerError::BackendTextValidation { .. })
    ));
}

#[test]
fn falha_label_duplicado() {
    let mut p = valid_program();
    p.functions[0].blocks.push(BackendTextBlock {
        label: "entry".to_string(),
        instructions: vec![],
        terminator: BackendTextTerminator::Return(Some(OperandIR::Int(0))),
    });
    assert!(matches!(
        backend_text_validate::validate_program(&p),
        Err(PinkerError::BackendTextValidation { .. })
    ));
}

#[test]
fn falha_jump_inexistente() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator = BackendTextTerminator::Jump("fim".to_string());
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_branch_inexistente() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator = BackendTextTerminator::Branch {
        cond: OperandIR::Bool(true),
        then_label: "ok".to_string(),
        else_label: "no".to_string(),
    };
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_ret_valor_em_nulo() {
    let mut p = valid_program();
    p.functions[0].ret_type = TypeIR::Nulo;
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_ret_vazio_em_nao_nulo() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator = BackendTextTerminator::Return(None);
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_call_nulo_com_destino() {
    let mut p = valid_program();
    p.functions.push(BackendTextFunction {
        name: "log".to_string(),
        ret_type: TypeIR::Nulo,
        params: vec![],
        locals: vec![],
        blocks: vec![BackendTextBlock {
            label: "entry".to_string(),
            instructions: vec![],
            terminator: BackendTextTerminator::Return(None),
        }],
    });
    p.functions[0].blocks[0].instructions = vec![BackendTextInstruction::Call {
        dest: Some(TempIR(0)),
        callee: "log".to_string(),
        args: vec![],
        ret_type: TypeIR::Nulo,
    }];
    p.functions[0].blocks[0].terminator = BackendTextTerminator::Return(Some(OperandIR::Int(0)));
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_temp_invalido() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator =
        BackendTextTerminator::Return(Some(OperandIR::Temp(TempIR(99))));
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_slot_invalido() {
    let mut p = valid_program();
    p.functions[0].blocks[0].instructions = vec![BackendTextInstruction::Mov {
        dest: "%y#0".to_string(),
        src: OperandIR::Int(1),
    }];
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn falha_global_invalida() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator =
        BackendTextTerminator::Return(Some(OperandIR::GlobalConst("NAO".to_string())));
    assert!(backend_text_validate::validate_program(&p).is_err());
}

#[test]
fn caso_call_binaria_temporario_if_else_valido() {
    let p = BackendTextProgram {
        is_freestanding: false,
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            BackendTextFunction {
                name: "soma".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec!["%x#0".to_string(), "%y#0".to_string()],
                locals: vec![],
                blocks: vec![BackendTextBlock {
                    label: "entry".to_string(),
                    instructions: vec![BackendTextInstruction::Binary {
                        dest: TempIR(0),
                        op: BinaryOpIR::Add,
                        lhs: OperandIR::Local("%x#0".to_string()),
                        rhs: OperandIR::Local("%y#0".to_string()),
                    }],
                    terminator: BackendTextTerminator::Return(Some(OperandIR::Temp(TempIR(0)))),
                }],
            },
            BackendTextFunction {
                name: "principal".to_string(),
                ret_type: TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                blocks: vec![
                    BackendTextBlock {
                        label: "entry".to_string(),
                        instructions: vec![],
                        terminator: BackendTextTerminator::Branch {
                            cond: OperandIR::Bool(true),
                            then_label: "then_0".to_string(),
                            else_label: "else_1".to_string(),
                        },
                    },
                    BackendTextBlock {
                        label: "then_0".to_string(),
                        instructions: vec![BackendTextInstruction::Call {
                            dest: Some(TempIR(0)),
                            callee: "soma".to_string(),
                            args: vec![OperandIR::Int(1), OperandIR::Int(2)],
                            ret_type: TypeIR::Bombom,
                        }],
                        terminator: BackendTextTerminator::Return(Some(OperandIR::Temp(TempIR(0)))),
                    },
                    BackendTextBlock {
                        label: "else_1".to_string(),
                        instructions: vec![],
                        terminator: BackendTextTerminator::Return(Some(OperandIR::Int(0))),
                    },
                ],
            },
        ],
    };

    assert!(backend_text_validate::validate_program(&p).is_ok());
}

#[test]
fn backend_text_invalido_nao_e_impresso() {
    let mut p = valid_program();
    p.functions[0].blocks[0].terminator = BackendTextTerminator::Return(None);

    let output = match backend_text_validate::validate_program(&p) {
        Ok(()) => pinker_v0::backend_text::render_program(&p),
        Err(_) => String::new(),
    };

    assert_eq!(output, "");
}

#[test]
fn formato_estrutura_estavel() {
    let out = pinker_v0::backend_text::render_program(&valid_program());
    assert!(out.starts_with("module main\nmode hospedado\nglobals:\n"));
    assert!(out.contains("text:\n  func principal:\n"));
    assert!(out.contains("entry:\n      ins mov %x#0, @LIMITE\n      term ret %x#0\n"));
}
