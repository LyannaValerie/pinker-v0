use pinker_v0::cfg_ir::{
    BasicBlockIR, FunctionCfgIR, GlobalConstCfgIR, InstructionCfgIR, OperandIR, ProgramCfgIR,
    TempIR, TerminatorIR,
};
use pinker_v0::cfg_ir_validate;
use pinker_v0::error::PinkerError;
use pinker_v0::ir::{BindingIR, LocalIR, ResolvedTypeId, TypeIR};
use pinker_v0::token::{Position, Span};

fn sp() -> Span {
    Span::new(Position::new(1, 1), Position::new(1, 1))
}

fn base_program(function: FunctionCfgIR) -> ProgramCfgIR {
    ProgramCfgIR {
        union_types: vec![],
        is_freestanding: false,
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
            resolved: Some(ResolvedTypeId(0)),
        }],
        locals: vec![LocalIR {
            source_name: "x".to_string(),
            slot: "%x#0".to_string(),
            ty: TypeIR::Bombom,
            resolved: None,
            is_mut: true,
        }],
        ret_type,
        entry: "entry".to_string(),
        blocks,
        span: sp(),
    }
}

// @pinker-nav:start evidencia.cfg.validacao-aceitacao-basica
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói CFG manualmente e aceita o caso simples presente pelo validador direto.
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
// @pinker-nav:end evidencia.cfg.validacao-aceitacao-basica

// @pinker-nav:start evidencia.cfg.validacao-blocos-e-alvos
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Rejeita nos casos presentes entrada ausente, label duplicado e alvos inexistentes de jump ou branch.
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
// @pinker-nav:end evidencia.cfg.validacao-blocos-e-alvos

// @pinker-nav:start evidencia.cfg.validacao-condicao-e-retorno
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Rejeita condição de branch incompatível e formas de retorno divergentes da assinatura.
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
// @pinker-nav:end evidencia.cfg.validacao-condicao-e-retorno

// @pinker-nav:start evidencia.cfg.validacao-chamada-e-referencias
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói CFG manualmente e rejeita destino inválido de chamada, slot, global ou temporário.
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
        union_types: vec![],
        is_freestanding: false,
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
// @pinker-nav:end evidencia.cfg.validacao-chamada-e-referencias

// @pinker-nav:start evidencia.cfg.validacao-alcancabilidade-e-renderizacao
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Rejeita bloco inalcançável e compara que uma CFG inválida não é renderizada no fluxo montado pelo teste.
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
// @pinker-nav:end evidencia.cfg.validacao-alcancabilidade-e-renderizacao

// @pinker-nav:start evidencia.cfg.validacao-diagnostico
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeciona parcialmente o contexto textual do diagnóstico de incompatibilidade em slot.
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
// @pinker-nav:end evidencia.cfg.validacao-diagnostico

// @pinker-nav:start evidencia.cfg.validacao-objetos-trato-fase244
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida manualmente materialização e despacho dinâmico na CFG e rejeita receiver comum, valor concreto divergente e destino em método nulo.

fn fase244_cfg_com_local_objeto(
    instructions: Vec<InstructionCfgIR>,
    terminator: TerminatorIR,
) -> FunctionCfgIR {
    let mut function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions,
            terminator,
        }],
    );

    function.locals.push(LocalIR {
        source_name: "objeto".to_string(),
        slot: "%objeto#0".to_string(),
        ty: TypeIR::TraitObject,
        resolved: None,
        is_mut: false,
    });

    function
}

#[test]
fn fase244_cfg_validation_aceita_materializacao_e_despacho() {
    let function = fase244_cfg_com_local_objeto(
        vec![
            InstructionCfgIR::MakeTraitObject {
                dest: TempIR(0),
                value: OperandIR::Int(21),
                trait_name: "Medivel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
            },
            InstructionCfgIR::Let {
                slot: "%objeto#0".to_string(),
                value: OperandIR::Temp(TempIR(0)),
            },
            InstructionCfgIR::TraitCall {
                dest: Some(TempIR(1)),
                object: OperandIR::Local("%objeto#0".to_string()),
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![OperandIR::Int(2)],
                param_types: vec![TypeIR::Bombom],
                ret_type: TypeIR::Bombom,
            },
        ],
        TerminatorIR::Return(Some(OperandIR::Temp(TempIR(1)))),
    );

    assert!(cfg_ir_validate::validate_program(&base_program(function)).is_ok());
}

#[test]
fn fase244_cfg_validation_aceita_trait_call_nula_sem_destino() {
    let function = fase244_cfg_com_local_objeto(
        vec![
            InstructionCfgIR::MakeTraitObject {
                dest: TempIR(0),
                value: OperandIR::Int(35),
                trait_name: "Observavel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec!["__impl_10_Observavel_6_bombom_observar".to_string()],
            },
            InstructionCfgIR::Let {
                slot: "%objeto#0".to_string(),
                value: OperandIR::Temp(TempIR(0)),
            },
            InstructionCfgIR::TraitCall {
                dest: None,
                object: OperandIR::Local("%objeto#0".to_string()),
                trait_name: "Observavel".to_string(),
                method_name: "observar".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![OperandIR::Int(7)],
                param_types: vec![TypeIR::Bombom],
                ret_type: TypeIR::Nulo,
            },
        ],
        TerminatorIR::Return(Some(OperandIR::Int(0))),
    );

    assert!(cfg_ir_validate::validate_program(&base_program(function)).is_ok());
}

#[test]
fn fase244_cfg_validation_rejeita_receiver_comum() {
    let function = base_function(
        TypeIR::Bombom,
        vec![BasicBlockIR {
            label: "entry".to_string(),
            instructions: vec![InstructionCfgIR::TraitCall {
                dest: Some(TempIR(0)),
                object: OperandIR::Int(42),
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![],
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            }],
            terminator: TerminatorIR::Return(Some(OperandIR::Temp(TempIR(0)))),
        }],
    );

    let err = cfg_ir_validate::validate_program(&base_program(function))
        .expect_err("receiver comum deve ser recusado")
        .to_string();

    assert!(
        err.contains("trait_call exige operando de objeto de trato"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_cfg_validation_rejeita_concreto_incompativel() {
    let function = fase244_cfg_com_local_objeto(
        vec![InstructionCfgIR::MakeTraitObject {
            dest: TempIR(0),
            value: OperandIR::Bool(true),
            trait_name: "Medivel".to_string(),
            concrete_type: TypeIR::Bombom,
            concrete_type_name: "bombom".to_string(),
            concrete_size: 8,
            vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
        }],
        TerminatorIR::Return(Some(OperandIR::Int(0))),
    );

    let err = cfg_ir_validate::validate_program(&base_program(function))
        .expect_err("concreto divergente deve ser recusado")
        .to_string();

    assert!(
        err.contains("make_trait_object com valor concreto incompatível"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_cfg_validation_rejeita_destino_em_metodo_nulo() {
    let function = fase244_cfg_com_local_objeto(
        vec![
            InstructionCfgIR::MakeTraitObject {
                dest: TempIR(0),
                value: OperandIR::Int(35),
                trait_name: "Observavel".to_string(),
                concrete_type: TypeIR::Bombom,
                concrete_type_name: "bombom".to_string(),
                concrete_size: 8,
                vtable_methods: vec!["__impl_10_Observavel_6_bombom_observar".to_string()],
            },
            InstructionCfgIR::Let {
                slot: "%objeto#0".to_string(),
                value: OperandIR::Temp(TempIR(0)),
            },
            InstructionCfgIR::TraitCall {
                dest: Some(TempIR(1)),
                object: OperandIR::Local("%objeto#0".to_string()),
                trait_name: "Observavel".to_string(),
                method_name: "observar".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![],
                param_types: vec![],
                ret_type: TypeIR::Nulo,
            },
        ],
        TerminatorIR::Return(Some(OperandIR::Int(0))),
    );

    let err = cfg_ir_validate::validate_program(&base_program(function))
        .expect_err("método nulo com destino deve ser recusado")
        .to_string();

    assert!(
        err.contains("trait_call nulo não pode definir temporário"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_cfg_validation_rejeita_slot_fora_da_vtable() {
    let function = fase244_cfg_com_local_objeto(
        vec![InstructionCfgIR::TraitCall {
            dest: Some(TempIR(0)),
            object: OperandIR::Local("%objeto#0".to_string()),
            trait_name: "Medivel".to_string(),
            method_name: "medir".to_string(),
            method_slot: 2,
            method_count: 1,
            args: vec![],
            param_types: vec![],
            ret_type: TypeIR::Bombom,
        }],
        TerminatorIR::Return(Some(OperandIR::Temp(TempIR(0)))),
    );

    let err = cfg_ir_validate::validate_program(&base_program(function))
        .expect_err("slot fora da vtable deve ser recusado")
        .to_string();
    assert!(
        err.contains("slot fora da vtable"),
        "diagnóstico inesperado: {err}"
    );
}

// @pinker-nav:end evidencia.cfg.validacao-objetos-trato-fase244
