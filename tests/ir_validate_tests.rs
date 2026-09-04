use pinker_v0::error::PinkerError;
use pinker_v0::ir::{
    BindingIR, BlockIR, FunctionIR, InstructionIR, LocalIR, ProgramIR, ResolvedTypeIR,
    ResolvedTypeId, TypeIR, ValueIR,
};
use pinker_v0::ir_validate;
use pinker_v0::token::{Position, Span};

fn sp() -> Span {
    Span::new(Position::new(1, 1), Position::new(1, 1))
}

/// Identidade resolvida de `bombom` na tabela mínima usada por estes testes.
fn identidade_bombom() -> ResolvedTypeId {
    ResolvedTypeId(0)
}

/// Tabela mínima de identidades: apenas `bombom`, na posição 0.
fn tabela_identidades_bombom() -> Vec<ResolvedTypeIR> {
    vec![ResolvedTypeIR {
        id: identidade_bombom(),
        canonical_key: "bombom".to_string(),
        representation: TypeIR::Bombom,
        nominal_kind: None,
        nominal_name: None,
        pointee: None,
        element: None,
        signature: None,
        union_members: None,
    }]
}

fn base_function(ret_type: TypeIR, instructions: Vec<InstructionIR>) -> FunctionIR {
    FunctionIR {
        name: "principal".to_string(),
        params: vec![],
        locals: vec![LocalIR {
            source_name: "x".to_string(),
            slot: "%x#0".to_string(),
            ty: TypeIR::Bombom,
            resolved: None,
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

// @pinker-nav:start evidencia.ir.validacao-aceitacao-basica
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói IR manualmente e aceita o caso simples presente pelo validador direto.
#[test]
fn valida_ir_simples_valida() {
    let program = ProgramIR {
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
// @pinker-nav:end evidencia.ir.validacao-aceitacao-basica

// @pinker-nav:start evidencia.ir.validacao-retorno-e-condicao
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói IR manualmente e rejeita nos casos presentes retorno e condição incompatíveis.
#[test]
fn falha_retorno_invalido() {
    let program = ProgramIR {
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
// @pinker-nav:end evidencia.ir.validacao-retorno-e-condicao

// @pinker-nav:start evidencia.ir.validacao-chamadas-e-nulo
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida diretamente IR manual e rejeita chamadas com argumento incompatível ou valor nulo usado como retorno.
#[test]
fn falha_chamada_invalida() {
    let callee = FunctionIR {
        name: "f".to_string(),
        params: vec![BindingIR {
            source_name: "a".to_string(),
            slot: "%a#0".to_string(),
            ty: TypeIR::Bombom,
            resolved: Some(identidade_bombom()),
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
                identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
            }),
            span: sp(),
        }],
    );

    let program = ProgramIR {
        resolved_types: tabela_identidades_bombom(),
        union_types: vec![],
        enum_variants: vec![],
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
                identidade: pinker_v0::intrinsics::identity::CalleeIdentity::User,
            }),
            span: sp(),
        }],
    );

    let program = ProgramIR {
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
// @pinker-nav:end evidencia.ir.validacao-chamadas-e-nulo

// @pinker-nav:start evidencia.ir.validacao-estrutura-e-diagnostico
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Rejeita bloco malformado e inspeciona parcialmente o contexto textual do diagnóstico de tipos.
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
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
        resolved_types: vec![],
        union_types: vec![],
        enum_variants: vec![],
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
// @pinker-nav:end evidencia.ir.validacao-estrutura-e-diagnostico

// @pinker-nav:start evidencia.ir.validacao-objetos-trato-fase244
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida manualmente a representação estrutural inicial da Fase 244 na IR: materialização explícita, snapshot concreto, vtable ordenada, chamada dinâmica com retorno ou nulo e diagnósticos para receiver e tipo concreto incompatíveis.

/// Identidade resolvida de `trato<Medivel>` usada pelos programas de Fase 244
/// construídos à mão. Um slot de `trato` tem representação ambígua e precisa de
/// identidade explícita, exatamente como os demais tipos nominais.
fn identidade_trato() -> ResolvedTypeId {
    ResolvedTypeId(0)
}

fn tabela_identidades_trato() -> Vec<ResolvedTypeIR> {
    vec![ResolvedTypeIR {
        id: identidade_trato(),
        canonical_key: "aplicado:5:trato[13:nome:7:Medivel]".to_string(),
        representation: TypeIR::TraitObject,
        nominal_kind: None,
        nominal_name: None,
        pointee: None,
        element: None,
        signature: None,
        union_members: None,
    }]
}

fn fase244_programa_com_funcao(function: FunctionIR) -> ProgramIR {
    ProgramIR {
        resolved_types: tabela_identidades_trato(),
        union_types: vec![],
        enum_variants: vec![],
        is_freestanding: false,
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![function],
    }
}

#[test]
fn fase244_ir_valida_materializacao_e_chamada_dinamica_com_retorno() {
    let function = FunctionIR {
        name: "principal".to_string(),
        params: vec![],
        locals: vec![LocalIR {
            source_name: "objeto".to_string(),
            slot: "%objeto#0".to_string(),
            ty: TypeIR::TraitObject,
            resolved: Some(identidade_trato()),
            is_mut: false,
        }],
        ret_type: TypeIR::Bombom,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions: vec![
                InstructionIR::Let {
                    slot: "%objeto#0".to_string(),
                    value: ValueIR::MakeTraitObject {
                        value: Box::new(ValueIR::Int(21)),
                        trait_name: "Medivel".to_string(),
                        concrete_type: TypeIR::Bombom,
                        concrete_type_name: "bombom".to_string(),
                        concrete_size: 8,
                        vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
                    },
                    span: sp(),
                },
                InstructionIR::Return {
                    value: Some(ValueIR::TraitCall {
                        object: Box::new(ValueIR::Local("%objeto#0".to_string())),
                        trait_name: "Medivel".to_string(),
                        method_name: "medir".to_string(),
                        method_slot: 0,
                        method_count: 1,
                        args: vec![ValueIR::Int(2)],
                        param_types: vec![TypeIR::Bombom],
                        ret_type: TypeIR::Bombom,
                    }),
                    span: sp(),
                },
            ],
            span: sp(),
        },
        span: sp(),
    };

    assert!(ir_validate::validate_program(&fase244_programa_com_funcao(function)).is_ok());
}

#[test]
fn fase244_ir_valida_chamada_dinamica_nula_como_comando() {
    let function = FunctionIR {
        name: "principal".to_string(),
        params: vec![],
        locals: vec![LocalIR {
            source_name: "objeto".to_string(),
            slot: "%objeto#0".to_string(),
            ty: TypeIR::TraitObject,
            resolved: Some(identidade_trato()),
            is_mut: false,
        }],
        ret_type: TypeIR::Bombom,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions: vec![
                InstructionIR::Let {
                    slot: "%objeto#0".to_string(),
                    value: ValueIR::MakeTraitObject {
                        value: Box::new(ValueIR::Int(35)),
                        trait_name: "Observavel".to_string(),
                        concrete_type: TypeIR::Bombom,
                        concrete_type_name: "bombom".to_string(),
                        concrete_size: 8,
                        vtable_methods: vec!["__impl_10_Observavel_6_bombom_observar".to_string()],
                    },
                    span: sp(),
                },
                InstructionIR::Expr {
                    value: ValueIR::TraitCall {
                        object: Box::new(ValueIR::Local("%objeto#0".to_string())),
                        trait_name: "Observavel".to_string(),
                        method_name: "observar".to_string(),
                        method_slot: 0,
                        method_count: 1,
                        args: vec![ValueIR::Int(7)],
                        param_types: vec![TypeIR::Bombom],
                        ret_type: TypeIR::Nulo,
                    },
                    span: sp(),
                },
                InstructionIR::Return {
                    value: Some(ValueIR::Int(0)),
                    span: sp(),
                },
            ],
            span: sp(),
        },
        span: sp(),
    };

    assert!(ir_validate::validate_program(&fase244_programa_com_funcao(function)).is_ok());
}

#[test]
fn fase244_followup_ir_rejeita_identidade_nominal_obrigatoria_ausente() {
    let function = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::TraitCall {
                object: Box::new(ValueIR::MakeTraitObject {
                    value: Box::new(ValueIR::Int(42)),
                    trait_name: "Medivel".to_string(),
                    concrete_type: TypeIR::Bombom,
                    concrete_type_name: "bombom".to_string(),
                    concrete_size: 8,
                    vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
                }),
                trait_name: String::new(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![],
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            }),
            span: sp(),
        }],
    );

    let err = ir_validate::validate_program(&fase244_programa_com_funcao(function))
        .expect_err("identidade nominal vazia deve ser recusada");
    match err {
        PinkerError::IrValidation { msg, span } => {
            assert!(
                msg.contains("chamada dinâmica sem identidade nominal completa"),
                "diagnóstico inesperado: {msg}"
            );
            assert_eq!(span, sp(), "diagnóstico deve preservar o span da instrução");
        }
        other => panic!("esperado erro estruturado de IR, obtido {other:?}"),
    }
}

#[test]
fn fase244_ir_rejeita_tipo_concreto_incompatível_na_materializacao() {
    let function = FunctionIR {
        name: "principal".to_string(),
        params: vec![],
        locals: vec![LocalIR {
            source_name: "objeto".to_string(),
            slot: "%objeto#0".to_string(),
            ty: TypeIR::TraitObject,
            resolved: Some(identidade_trato()),
            is_mut: false,
        }],
        ret_type: TypeIR::Bombom,
        entry: BlockIR {
            label: "entry".to_string(),
            instructions: vec![
                InstructionIR::Let {
                    slot: "%objeto#0".to_string(),
                    value: ValueIR::MakeTraitObject {
                        value: Box::new(ValueIR::Bool(true)),
                        trait_name: "Medivel".to_string(),
                        concrete_type: TypeIR::Bombom,
                        concrete_type_name: "bombom".to_string(),
                        concrete_size: 8,
                        vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
                    },
                    span: sp(),
                },
                InstructionIR::Return {
                    value: Some(ValueIR::Int(0)),
                    span: sp(),
                },
            ],
            span: sp(),
        },
        span: sp(),
    };

    let err = ir_validate::validate_program(&fase244_programa_com_funcao(function))
        .expect_err("tipo concreto divergente deve ser recusado")
        .to_string();

    assert!(
        err.contains("valor concreto incompatível na materialização de objeto de trato"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_ir_rejeita_chamada_dinamica_sobre_valor_comum() {
    let function = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::TraitCall {
                object: Box::new(ValueIR::Int(42)),
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 0,
                method_count: 1,
                args: vec![],
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            }),
            span: sp(),
        }],
    );

    let err = ir_validate::validate_program(&fase244_programa_com_funcao(function))
        .expect_err("receiver comum deve ser recusado")
        .to_string();

    assert!(
        err.contains("chamada dinâmica exige objeto de trato"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_ir_rejeita_slot_fora_da_vtable() {
    let function = base_function(
        TypeIR::Bombom,
        vec![InstructionIR::Return {
            value: Some(ValueIR::TraitCall {
                object: Box::new(ValueIR::MakeTraitObject {
                    value: Box::new(ValueIR::Int(42)),
                    trait_name: "Medivel".to_string(),
                    concrete_type: TypeIR::Bombom,
                    concrete_type_name: "bombom".to_string(),
                    concrete_size: 8,
                    vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
                }),
                trait_name: "Medivel".to_string(),
                method_name: "medir".to_string(),
                method_slot: 1,
                method_count: 1,
                args: vec![],
                param_types: vec![],
                ret_type: TypeIR::Bombom,
            }),
            span: sp(),
        }],
    );

    let err = ir_validate::validate_program(&fase244_programa_com_funcao(function))
        .expect_err("slot fora da vtable deve ser recusado")
        .to_string();
    assert!(
        err.contains("slot fora da vtable"),
        "diagnóstico inesperado: {err}"
    );
}

// @pinker-nav:end evidencia.ir.validacao-objetos-trato-fase244
