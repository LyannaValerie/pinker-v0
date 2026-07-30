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

// @pinker-nav:start evidencia.machine.renderizacao-programa-valido
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Renderiza um programa simples aceito pelo caminho presente.
#[test]
fn stack_valida_programa_simples() {
    let out = render_machine("pacote main; carinho principal() -> bombom { mimo 1 + 2; }").unwrap();
    assert!(out.contains("vm add"));
}

// @pinker-nav:end evidencia.machine.renderizacao-programa-valido
// @pinker-nav:start evidencia.machine.validacao-underflow-operadores
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói manualmente operações unária e binária e espera rejeição por underflow.
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

// @pinker-nav:end evidencia.machine.validacao-underflow-operadores
// @pinker-nav:start evidencia.machine.validacao-chamadas-aridade-e-underflow
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói chamadas manualmente e espera rejeição de underflow ou aridade inválida nos casos presentes.
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

// @pinker-nav:end evidencia.machine.validacao-chamadas-aridade-e-underflow
// @pinker-nav:start evidencia.machine.validacao-formato-diagnostico
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeciona o formato contextual do diagnóstico de validação presente.
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

// @pinker-nav:end evidencia.machine.validacao-formato-diagnostico
// @pinker-nav:start evidencia.machine.validacao-branch
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói branches manualmente e espera rejeição por condição ausente ou incompatível.
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

// @pinker-nav:end evidencia.machine.validacao-branch
// @pinker-nav:start evidencia.machine.renderizacao-branch-valido
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Renderiza o branch de tipo compatível presente.
#[test]
fn stack_branch_tipo_compativel() {
    let out = render_machine(
        "pacote main; carinho principal() -> bombom { talvez verdade { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert!(out.contains("term br_true"));
}

// @pinker-nav:end evidencia.machine.renderizacao-branch-valido
// @pinker-nav:start evidencia.machine.validacao-retorno
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Constrói retornos manualmente e espera rejeição por valor ausente ou tipo incompatível.
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

// @pinker-nav:end evidencia.machine.validacao-retorno
// @pinker-nav:start evidencia.machine.renderizacao-retorno-valido
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Renderiza o retorno de tipo compatível presente.
#[test]
fn stack_ret_tipo_compativel() {
    let out = render_machine("pacote main; carinho principal() -> bombom { mimo 7; }").unwrap();
    assert!(out.contains("term ret"));
}

// @pinker-nav:end evidencia.machine.renderizacao-retorno-valido
// @pinker-nav:start evidencia.machine.validacao-pilha-retvoid-e-merges
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de pilha residual em retorno vazio e de alturas inconsistentes em merge de fluxo.
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

// @pinker-nav:end evidencia.machine.validacao-pilha-retvoid-e-merges
// @pinker-nav:start evidencia.machine.validacao-slots-existencia
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de load e store para slots inexistentes nos programas construídos.
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

// @pinker-nav:end evidencia.machine.validacao-slots-existencia
// @pinker-nav:start evidencia.machine.validacao-slots-tipados
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida os casos presentes de fluxo por parâmetros e locais tipados e rejeita store incompatível.
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

// @pinker-nav:end evidencia.machine.validacao-slots-tipados
// @pinker-nav:start evidencia.machine.validacao-tipos-operacoes-e-retorno
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de parâmetro lógico em aritmética e retorno incompatível.
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

// @pinker-nav:end evidencia.machine.validacao-tipos-operacoes-e-retorno
// @pinker-nav:start evidencia.machine.validacao-tipos-chamadas
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de argumentos incompatíveis em chamadas com e sem retorno.
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

// @pinker-nav:end evidencia.machine.validacao-tipos-chamadas
// @pinker-nav:start evidencia.machine.renderizacao-casos-validos
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Renderiza os casos válidos presentes de temporário em if/else e chamada com retorno.
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

// @pinker-nav:end evidencia.machine.renderizacao-casos-validos
// @pinker-nav:start evidencia.machine.validacao-programa-invalido
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida diretamente uma máquina inválida construída manualmente e observa a rejeição presente.
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

// @pinker-nav:end evidencia.machine.validacao-programa-invalido
// @pinker-nav:start evidencia.machine.renderizacao-cli-golden
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeciona fragmentos do renderer CLI para um programa não trivial válido.
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
// @pinker-nav:end evidencia.machine.renderizacao-cli-golden

// @pinker-nav:start evidencia.machine.validacao-objetos-trato-fase244
// @pinker-nav:domain machine
// @pinker-nav:layer evidencia
// @pinker-nav:summary Valida manualmente os efeitos de pilha da materialização e do despacho dinâmico da Fase 244, incluindo receiver nominal, argumentos, retorno nulo e invariantes da vtable.

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

// @pinker-nav:end evidencia.machine.validacao-objetos-trato-fase244
