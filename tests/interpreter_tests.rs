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
fn run_falha_call_nao_suportado() {
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
                    callee: "x".to_string(),
                    argc: 0,
                }],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("runtime mínimo não suporta call"));
}

#[test]
fn run_falha_call_void_nao_suportado() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Nulo,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::CallVoid {
                    callee: "x".to_string(),
                    argc: 0,
                }],
                terminator: MachineTerminator::RetVoid,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("runtime mínimo não suporta call_void"));
}

#[test]
fn run_falha_globals_nao_suportadas() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![MachineGlobal {
            name: "g".to_string(),
            value: pinker_v0::cfg_ir::OperandIR::Int(1),
        }],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::PushInt(1)],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(err.contains("runtime mínimo não suporta globals"));
}

#[test]
fn cli_run_funciona_em_caso_valido() {
    let source = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let file = std::env::temp_dir().join("pinker_run_ok.pink");
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
