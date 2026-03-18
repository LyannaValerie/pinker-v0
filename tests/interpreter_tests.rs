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
    assert!(err.contains("[runtime::funcao_inexistente]"));
    assert!(err.contains("função chamada inexistente"));
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

// ── Fase 16: testes negativos de runtime ──────────────────────────────────

#[test]
fn run_falha_divisao_por_zero() {
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
                code: vec![
                    MachineInstr::PushInt(10),
                    MachineInstr::PushInt(0),
                    MachineInstr::Div,
                ],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("[runtime::divisao_por_zero]"),
        "mensagem: {}",
        err
    );
    assert!(err.contains("divisão por zero"), "mensagem: {}", err);
    assert!(
        err.contains("dica: verifique se o divisor é diferente de 0"),
        "mensagem: {}",
        err
    );
    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("at principal [bloco: entry] [instr: div]"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_runtime_em_chamada_tem_stack_trace() {
    let err = run_code(
        "pacote main; carinho quebra(x: bombom) -> bombom { mimo x / 0; } carinho principal() -> bombom { mimo quebra(10); }",
    )
    .unwrap_err();

    assert!(err.contains("divisão por zero"), "mensagem: {}", err);
    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("at principal [bloco: entry] [instr: call]"),
        "mensagem: {}",
        err
    );
    assert!(
        err.contains("at quebra [bloco: entry] [instr: div]"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_runtime_em_recursao_tem_stack_trace() {
    let err = run_code(
        "pacote main; carinho queda(n: bombom) -> bombom { talvez n == 0 { mimo 10 / 0; } senao { mimo queda(n - 1); } } carinho principal() -> bombom { mimo queda(2); }",
    )
    .unwrap_err();

    assert!(err.contains("divisão por zero"), "mensagem: {}", err);
    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("at principal [bloco: entry] [instr: call]"),
        "mensagem: {}",
        err
    );
    assert!(err.contains("at queda"), "mensagem: {}", err);
    assert!(err.contains("[instr: div]"), "mensagem: {}", err);
    assert!(
        err.matches("[instr: call]").count() >= 2,
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_limite_recursao_excedido_tem_categoria_e_trace() {
    let err = run_code(
        "pacote main; carinho loop() -> bombom { mimo loop(); } carinho principal() -> bombom { mimo loop(); }",
    )
    .unwrap_err();

    assert!(
        err.contains("[runtime::limite_recursao_excedido]"),
        "mensagem: {}",
        err
    );
    assert!(
        err.contains("limite preventivo de recursão excedido"),
        "mensagem: {}",
        err
    );
    assert!(
        err.contains("profundidade máxima de chamadas (128)"),
        "mensagem: {}",
        err
    );
    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("at principal [bloco: entry] [instr: call]"),
        "mensagem: {}",
        err
    );
    assert!(err.contains("at loop"), "mensagem: {}", err);
}

#[test]
fn run_falha_slot_nao_inicializado() {
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
                code: vec![MachineInstr::LoadSlot("slot_fantasma".to_string())],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("[runtime::slot_nao_inicializado]"),
        "mensagem: {}",
        err
    );
    assert!(
        err.contains("load_slot em slot não inicializado"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_call_retorna_void() {
    // Call para função que faz RetVoid: deve falhar com "call exige função com retorno"
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "principal".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::Call {
                        callee: "aux".to_string(),
                        argc: 0,
                    }],
                    terminator: MachineTerminator::Ret,
                }],
            },
            MachineFunction {
                name: "aux".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Nulo,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![],
                    terminator: MachineTerminator::RetVoid,
                }],
            },
        ],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("call exige função com retorno"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_call_void_retorna_valor() {
    // CallVoid para função que empilha valor e faz Ret: deve falhar com "call_void exige função sem retorno"
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "principal".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Nulo,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::CallVoid {
                        callee: "aux".to_string(),
                        argc: 0,
                    }],
                    terminator: MachineTerminator::RetVoid,
                }],
            },
            MachineFunction {
                name: "aux".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::PushInt(42)],
                    terminator: MachineTerminator::Ret,
                }],
            },
        ],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("call_void exige função sem retorno"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_aridade_invalida() {
    // principal chama aux com 1 argumento mas aux tem 0 parâmetros
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![
            MachineFunction {
                name: "principal".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![
                        MachineInstr::PushInt(1),
                        MachineInstr::Call {
                            callee: "aux".to_string(),
                            argc: 1,
                        },
                    ],
                    terminator: MachineTerminator::Ret,
                }],
            },
            MachineFunction {
                name: "aux".to_string(),
                ret_type: pinker_v0::ir::TypeIR::Bombom,
                params: vec![],
                locals: vec![],
                slot_types: HashMap::new(),
                blocks: vec![MachineBlock {
                    label: "entry".to_string(),
                    code: vec![MachineInstr::PushInt(99)],
                    terminator: MachineTerminator::Ret,
                }],
            },
        ],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("chamada com aridade inválida"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_falha_valor_global_nao_suportado() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![MachineGlobal {
            name: "G".to_string(),
            value: pinker_v0::cfg_ir::OperandIR::Local("x".to_string()),
        }],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::LoadGlobal("G".to_string())],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };
    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("valor global não suportado em runtime"),
        "mensagem: {}",
        err
    );
}

// ── Fase 16: testes end-to-end via run_code ───────────────────────────────

#[test]
fn run_not_unario() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez !falso { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_divisao() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova a = 10; nova b = 2; mimo a / b; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(5)));
}

#[test]
fn run_igualdade() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 1 == 1 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_diferenca() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 1 != 2 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_comparacao_maior_igual() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 5 >= 3 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_comparacao_maior() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 5 > 3 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_comparacao_menor_igual() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez 3 <= 5 { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_variavel_mutavel() {
    let out =
        run_code("pacote main; carinho principal() -> bombom { nova mut x = 1; x = 99; mimo x; }")
            .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(99)));
}

// ── Fase 16: CLI — exit code não-zero em erro de runtime ─────────────────

#[test]
fn cli_run_erro_runtime_tem_exit_nonzero() {
    // Programa com divisão por zero via --run: deve retornar exit code != 0 e stderr não vazio
    let source =
        "pacote main; carinho div(a: bombom, b: bombom) -> bombom { mimo a / b; } carinho principal() -> bombom { mimo div(10, 0); }";
    let file = std::env::temp_dir().join("pinker_run_div_zero.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty());
    assert!(stderr.contains("Erro Runtime:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("  mensagem: [runtime::divisao_por_zero]"),
        "stderr: {}",
        stderr
    );
    assert!(stderr.contains("stack trace:"), "stderr: {}", stderr);
    assert!(stderr.contains("at principal"), "stderr: {}", stderr);
    assert!(stderr.contains("at div"), "stderr: {}", stderr);
    assert!(stderr.contains("[bloco:"), "stderr: {}", stderr);
    assert!(stderr.contains("  span: 1:1..1:1"), "stderr: {}", stderr);
}

// ── Fase 17: recursão no interpretador ─────────────────────────────────────

#[test]
fn run_recursao_fatorial() {
    let out = run_code(
        "pacote main; carinho fat(n: bombom) -> bombom { talvez n == 0 { mimo 1; } senao { mimo n * fat(n - 1); } } carinho principal() -> bombom { mimo fat(5); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(120)));
}

#[test]
fn run_recursao_fibonacci() {
    let out = run_code(
        "pacote main; carinho fib(n: bombom) -> bombom { talvez n == 0 { mimo 0; } senao { talvez n == 1 { mimo 1; } senao { mimo fib(n - 1) + fib(n - 2); } } } carinho principal() -> bombom { mimo fib(7); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(13)));
}

#[test]
fn run_recursao_com_acumulador() {
    let out = run_code(
        "pacote main; carinho soma(n: bombom) -> bombom { talvez n == 0 { mimo 0; } senao { mimo n + soma(n - 1); } } carinho principal() -> bombom { mimo soma(5); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(15)));
}

#[test]
fn run_recursao_mutua() {
    let out = run_code(
        "pacote main; carinho eh_par(n: bombom) -> bombom { talvez n == 0 { mimo 1; } senao { mimo eh_impar(n - 1); } } carinho eh_impar(n: bombom) -> bombom { talvez n == 0 { mimo 0; } senao { mimo eh_par(n - 1); } } carinho principal() -> bombom { mimo eh_par(4); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

// ── Fase 20: mais cenários end-to-end reais via CLI --run ─────────────────

fn run_cli_example(path: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn cli_run_mantem_exemplos_base() {
    let casos = [
        ("examples/run_soma.pink", "42\n"),
        ("examples/run_chamada.pink", "42\n"),
        ("examples/run_global.pink", "100\n"),
        ("examples/run_global_expr.pink", "44\n"),
    ];

    for (path, expected) in casos {
        let out = run_cli_example(path);
        assert!(out.status.success(), "falhou em {}", path);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            expected,
            "path={}",
            path
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).is_empty(),
            "stderr em {}: {}",
            path,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let maybe_fatorial = std::path::Path::new("examples/run_recursao_fatorial.pink");
    if maybe_fatorial.exists() {
        let out = run_cli_example("examples/run_recursao_fatorial.pink");
        assert!(out.status.success());
    }
}

#[test]
fn cli_run_global_com_chamada_exemplo_novo() {
    let out = run_cli_example("examples/run_global_call_combo.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_mutacao_com_if_else_exemplo_novo() {
    let out = run_cli_example("examples/run_mut_if_else.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_recursao_com_global_exemplo_novo() {
    let out = run_cli_example("examples/run_recursao_global.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5\n");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_erro_runtime_limite_recursao_tem_saida_previsivel() {
    let out = run_cli_example("examples/run_recursao_limite_cli.pink");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Erro Runtime:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("[runtime::limite_recursao_excedido]"),
        "stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("limite preventivo de recursão excedido"),
        "stderr: {}",
        stderr
    );
    assert!(stderr.contains("stack trace:"), "stderr: {}", stderr);
    assert!(stderr.contains("at principal"), "stderr: {}", stderr);
    assert!(stderr.contains("at loop"), "stderr: {}", stderr);
    assert!(stderr.contains("[instr: call]"), "stderr: {}", stderr);
    assert!(stderr.contains("  span: 1:1..1:1"), "stderr: {}", stderr);
}

#[test]
fn cli_run_erro_runtime_em_exemplo_novo() {
    let out = run_cli_example("examples/run_div_zero_cli.pink");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[runtime::divisao_por_zero]"),
        "stderr: {}",
        stderr
    );
    assert!(stderr.contains("Erro Runtime:"), "stderr: {}", stderr);
    assert!(stderr.contains("  mensagem:"), "stderr: {}", stderr);
    assert!(stderr.contains("divisão por zero"), "stderr: {}", stderr);
    assert!(stderr.contains("stack trace:"), "stderr: {}", stderr);
    assert!(stderr.contains("at principal"), "stderr: {}", stderr);
    assert!(stderr.contains("[instr: div]"), "stderr: {}", stderr);
    assert!(stderr.contains("  span: 1:1..1:1"), "stderr: {}", stderr);
}
