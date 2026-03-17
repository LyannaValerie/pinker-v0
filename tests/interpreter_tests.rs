mod common;

use pinker_v0::abstract_machine::{
    MachineBlock, MachineFunction, MachineGlobal, MachineInstr, MachineProgram, MachineTerminator,
};
use pinker_v0::abstract_machine_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::collections::HashMap;
use std::fs;
use std::process::Command;

fn run_code(code: &str) -> Result<Option<RuntimeValue>, String> {
    let program = common::parse(code).map_err(|e| e.to_string())?;
    semantic::check_program(&program).map_err(|e| e.to_string())?;
    let program_ir = ir::lower_program(&program).map_err(|e| e.to_string())?;
    ir_validate::validate_program(&program_ir).map_err(|e| e.to_string())?;
    let cfg = cfg_ir::lower_program(&program_ir).map_err(|e| e.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|e| e.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|e| e.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|e| e.to_string())?;
    let machine =
        pinker_v0::abstract_machine::lower_program(&selected).map_err(|e| e.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|e| e.to_string())?;
    interpreter::run_program(&machine).map_err(|e| e.to_string())
}

#[test]
fn run_retorno_constante() {
    let out = run_code("pacote main; carinho principal() -> bombom { mimo 42; }").unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_retorno_global_inteira() {
    let out = run_code(
        "pacote main; eterno LIMITE: bombom = 100; carinho principal() -> bombom { mimo LIMITE; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(100)));
}

#[test]
fn run_global_em_expressao_aritmetica() {
    let out = run_code(
        "pacote main; eterno BASE: bombom = 20; carinho principal() -> bombom { mimo (BASE + 2) * 2; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(44)));
}

#[test]
fn run_global_booleana_em_fluxo_condicional() {
    let out = run_code(
        "pacote main; eterno FLAG: logica = verdade; carinho principal() -> bombom { talvez FLAG { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_soma_de_locais() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova a = 40; nova b = 2; mimo a + b; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_if_else_com_retorno() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez verdade { mimo 7; } senao { mimo 9; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(7)));
}

#[test]
fn run_negacao_unaria() {
    let out = run_code("pacote main; carinho principal() -> bombom { mimo -5; }").unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(u64::MAX - 4)));
}

#[test]
fn run_comparacao_em_fluxo_de_controle() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 1 < 2 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_chamada_simples_um_argumento() {
    let out = run_code(
        "pacote main; carinho dobro(x: bombom) -> bombom { mimo x + x; } carinho principal() -> bombom { mimo dobro(21); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_chamada_com_multiplos_argumentos() {
    let out = run_code(
        "pacote main; carinho calc(a: bombom, b: bombom, c: bombom) -> bombom { mimo a + b * c; } carinho principal() -> bombom { mimo calc(2, 10, 4); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_chamada_respeita_ordem_argumentos() {
    let out = run_code(
        "pacote main; carinho sub(a: bombom, b: bombom) -> bombom { mimo a - b; } carinho principal() -> bombom { mimo sub(10, 3); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(7)));
}

#[test]
fn run_chamada_encadeada() {
    let out = run_code(
        "pacote main; carinho inc(x: bombom) -> bombom { mimo x + 1; } carinho dobro(x: bombom) -> bombom { mimo x + x; } carinho principal() -> bombom { mimo dobro(inc(20)); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_chamada_void_como_statement() {
    let out = run_code(
        "pacote main; carinho marca() { mimo; } carinho principal() -> bombom { marca(); mimo 42; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_falha_divisao_por_zero() {
    let err = run_code("pacote main; carinho principal() -> bombom { mimo 10 / 0; }").unwrap_err();
    assert!(err.contains("divisão por zero"));
}

#[test]
fn run_falha_funcao_inexistente() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::Call {
                    callee: "nao_existe".to_string(),
                    argc: 0,
                }],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("função chamada inexistente"));
}

#[test]
fn run_falha_call_void_politica_retorno() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "retorna".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::PushInt(1)],
                    terminator: MachineTerminator::Ret,
                }],
            },
            MachineFunction {
                name: "principal".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![
                        MachineInstr::CallVoid {
                            callee: "retorna".to_string(),
                            argc: 0,
                        },
                        MachineInstr::PushInt(0),
                    ],
                    terminator: MachineTerminator::Ret,
                }],
            },
        ],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("call_void exige função sem retorno"));
}

#[test]
fn run_falha_aridade_em_chamada() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "id".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec!["%x#0".to_string()],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::LoadSlot("%x#0".to_string())],
                    terminator: MachineTerminator::Ret,
                }],
            },
            MachineFunction {
                name: "principal".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::Call {
                        callee: "id".to_string(),
                        argc: 0,
                    }],
                    terminator: MachineTerminator::Ret,
                }],
            },
        ],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("chamada com aridade inválida"));
}

#[test]
fn run_falha_global_inexistente() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::LoadGlobal("NAO_EXISTE".to_string())],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("global inexistente em runtime"));
}

#[test]
fn run_falha_global_duplicada() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![
            MachineGlobal {
                name: "LIM".to_string(),
                value: pinker_v0::cfg_ir::OperandIR::Int(1),
            },
            MachineGlobal {
                name: "LIM".to_string(),
                value: pinker_v0::cfg_ir::OperandIR::Int(2),
            },
        ],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::LoadGlobal("LIM".to_string())],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("global duplicada em runtime"));
}

#[test]
fn run_falha_valor_global_nao_suportado() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![MachineGlobal {
            name: "RUIM".to_string(),
            value: pinker_v0::cfg_ir::OperandIR::Local("%x#0".to_string()),
        }],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::PushInt(0)],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("valor global não suportado em runtime"));
}

#[test]
fn cli_run_funciona_em_caso_valido() {
    let source =
        "pacote main; carinho dobro(x: bombom) -> bombom { mimo x + x; } carinho principal() -> bombom { mimo dobro(21); }";
    let file = std::env::temp_dir().join("pinker_run_call_ok.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_global_funciona() {
    let source =
        "pacote main; eterno LIMITE: bombom = 100; carinho principal() -> bombom { mimo LIMITE; }";
    let file = std::env::temp_dir().join("pinker_run_global_ok.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "100\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_global_expr_funciona() {
    let source = "pacote main; eterno BASE: bombom = 20; carinho principal() -> bombom { mimo (BASE + 2) * 2; }";
    let file = std::env::temp_dir().join("pinker_run_global_expr_ok.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "44\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_chamada_void_funciona() {
    let source =
        "pacote main; carinho log() { mimo; } carinho principal() -> bombom { log(); mimo 42; }";
    let file = std::env::temp_dir().join("pinker_run_call_void_ok.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}
