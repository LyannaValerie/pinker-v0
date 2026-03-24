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
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

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

fn run_code_with_args(code: &str, args: &[&str]) -> Result<interpreter::RunOutcome, String> {
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
    let runtime_args: Vec<String> = args.iter().map(|v| (*v).to_string()).collect();
    interpreter::run_program_with_args(&machine, &runtime_args).map_err(|e| e.to_string())
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
fn run_bitwise_basico() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova a = 6; nova b = 3; mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1); }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(22)));
}

#[test]
fn run_logicos_basicos() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { talvez (verdade && falso) || !falso { mimo 1; } senao { mimo 0; } }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_verso_operacional_minimo_em_local_parametro_retorno() {
    let out = run_code(
        "pacote main;
         carinho eco(msg: verso) -> verso { mimo msg; }
         carinho principal() -> bombom {
             nova texto: verso = \"oi\";
             nova copia: verso = eco(texto);
             falar(copia);
             mimo 1;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_verso_concat_minimo_e_comprimento_minimo_funcionam() {
    let out = run_code(
        r#"
        pacote main;
        carinho junta(a: verso, b: verso) -> verso {
            mimo juntar_verso(a, b);
        }
        carinho principal() -> bombom {
            nova base: verso = "la";
            nova fim: verso = "li";
            nova texto: verso = junta(base, fim);
            falar(texto);
            nova n: bombom = tamanho_verso(texto);
            mimo n;
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(4)));
}

#[test]
fn run_indice_verso_minimo_funciona_e_pode_ir_para_falar() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = "lua";
            nova letra: verso = indice_verso(texto, 1);
            falar(letra);
            mimo tamanho_verso(letra);
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_indice_verso_falha_com_indice_fora_da_faixa() {
    let err = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = "oi";
            nova letra: verso = indice_verso(texto, 2);
            falar(letra);
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("índice fora da faixa em 'indice_verso'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_contem_verso_intrinseca_true_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = contem_verso("pinker v0", "ker");
            falar(ok);
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_contem_verso_intrinseca_false_em_caso_negativo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = contem_verso("pinker v0", "zzz");
            falar(ok);
            talvez ok {
                mimo 0;
            } senao {
                mimo 1;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_comeca_com_intrinseca_true_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = comeca_com("pinker", "pin");
            falar(ok);
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_comeca_com_intrinseca_false_em_caso_negativo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = comeca_com("pinker", "ker");
            falar(ok);
            talvez ok {
                mimo 0;
            } senao {
                mimo 1;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_termina_com_intrinseca_true_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = termina_com("pinker", "ker");
            falar(ok);
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_termina_com_intrinseca_false_em_caso_negativo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = termina_com("pinker", "pin");
            falar(ok);
            talvez ok {
                mimo 0;
            } senao {
                mimo 1;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_igual_verso_intrinseca_true_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = igual_verso("pinker", "pinker");
            falar(ok);
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_igual_verso_intrinseca_false_em_caso_negativo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = igual_verso("pinker", "Pinker");
            falar(ok);
            talvez ok {
                mimo 0;
            } senao {
                mimo 1;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_vazio_verso_intrinseca_true_em_string_vazia() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = vazio_verso("");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_vazio_verso_intrinseca_false_em_conteudo_real() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = vazio_verso("x");
            talvez ok { mimo 0; } senao { mimo 1; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_aparar_verso_intrinseca_remove_bordas() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova limpo: verso = aparar_verso("  pinker  ");
            nova ok: logica = igual_verso(limpo, "pinker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_aparar_verso_pode_resultar_em_vazio() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova limpo: verso = aparar_verso("   ");
            nova ok: logica = vazio_verso(limpo);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_minusculo_verso_intrinseca_funciona_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = minusculo_verso("PiNkEr V0");
            nova ok: logica = igual_verso(texto, "pinker v0");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_maiusculo_verso_intrinseca_funciona_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = maiusculo_verso("PiNkEr v0");
            nova ok: logica = igual_verso(texto, "PINKER V0");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_indice_verso_em_intrinseca_retorna_primeira_posicao_em_caso_positivo() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova pos: bombom = indice_verso_em("ola pinker", "pin");
            talvez pos == 4 { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_indice_verso_em_intrinseca_retorna_u64_max_quando_trecho_ausente() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova pos: bombom = indice_verso_em("ola pinker", "zzz");
            talvez pos == 18446744073709551615 {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_nao_vazio_verso_intrinseca_true_em_conteudo_real() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio_verso("x");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_nao_vazio_verso_intrinseca_false_em_string_vazia() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio_verso("");
            talvez ok { mimo 0; } senao { mimo 1; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_argumento_intrinseca_ler_posicional_minimo() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova nome: verso = argumento(0);
            falar("oi", nome);
            mimo tamanho_verso(nome);
        }"#,
        &["Pinker"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(6)));
    assert_eq!(out.exit_status, None);
}

#[test]
fn run_argumento_intrinseca_falha_sem_arg_disponivel() {
    let err = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            falar(argumento(0));
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("índice fora da faixa em 'argumento'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_quantos_argumentos_intrinseca_conta_argv_posicional() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova total: bombom = quantos_argumentos();
            falar(total);
            mimo total;
        }"#,
        &["um", "dois", "tres"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(3)));
    assert_eq!(out.exit_status, None);
}

#[test]
fn run_tem_argumento_intrinseca_integra_com_argumento() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez tem_argumento(1) {
                falar(argumento(1));
                mimo tamanho_verso(argumento(1));
            } senao {
                falar("faltou");
                sair(9);
                mimo 0;
            }
        }"#,
        &["A", "beta"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(4)));
    assert_eq!(out.exit_status, None);
}

#[test]
fn run_tem_argumento_intrinseca_false_sem_falha_de_argumento() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez tem_argumento(2) {
                falar(argumento(2));
                mimo 0;
            } senao {
                sair(5);
                mimo 1;
            }
        }"#,
        &["A", "B"],
    )
    .unwrap();
    assert_eq!(out.return_value, None);
    assert_eq!(out.exit_status, Some(5));
}

#[test]
fn run_argumento_ou_intrinseca_usa_fallback_sem_arg() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova nome: verso = argumento_ou(0, "visitante");
            falar("oi", nome);
            mimo tamanho_verso(nome);
        }"#,
        &[],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(9)));
    assert_eq!(out.exit_status, None);
}

#[test]
fn run_argumento_ou_intrinseca_prioriza_arg_existente() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova nome: verso = argumento_ou(0, "visitante");
            falar("oi", nome);
            mimo tamanho_verso(nome);
        }"#,
        &["Pinker"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(6)));
    assert_eq!(out.exit_status, None);
}

#[test]
fn run_falar_multiplos_argumentos_bombom_funciona() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            falar(10, 20, 30);
            mimo 0;
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_falar_mistura_verso_e_bombom_funciona() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            falar("idade", 7, "anos");
            mimo 0;
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_falar_multiplos_argumentos_com_locals_e_chamada_funciona() {
    let out = run_code(
        r#"
        pacote main;
        carinho eco(v: verso) -> verso { mimo v; }
        carinho principal() -> bombom {
            nova nome: verso = "Pinker";
            nova n: bombom = 2;
            falar("oi", eco(nome), n);
            mimo n;
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(2)));
}

#[test]
fn run_short_circuit_and_nao_avalia_rhs() {
    let out = run_code(
        "pacote main;
         carinho falha() -> logica {
             talvez 1 / 0 == 0 { mimo verdade; } senao { mimo falso; }
         }
         carinho principal() -> bombom {
             talvez falso && falha() { mimo 1; } senao { mimo 0; }
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_short_circuit_or_nao_avalia_rhs() {
    let out = run_code(
        "pacote main;
         carinho falha() -> logica {
             talvez 1 / 0 == 0 { mimo verdade; } senao { mimo falso; }
         }
         carinho principal() -> bombom {
             talvez verdade || falha() { mimo 1; } senao { mimo 0; }
         }",
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
fn run_seta_tem_repr_minima_no_runtime_em_slot() {
    let mut slot_types = HashMap::new();
    slot_types.insert(
        "p".to_string(),
        pinker_v0::ir::TypeIR::Pointer { is_volatile: false },
    );

    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Pointer { is_volatile: false },
            params: vec![],
            locals: vec!["p".to_string()],
            slot_types,
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![
                    MachineInstr::PushInt(4096),
                    MachineInstr::StoreSlot("p".to_string()),
                    MachineInstr::LoadSlot("p".to_string()),
                ],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let out = interpreter::run_program(&program).unwrap();
    assert_eq!(out, Some(RuntimeValue::Ptr(4096)));
}

#[test]
fn run_seta_tem_repr_minima_no_runtime_em_global() {
    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![MachineGlobal {
            name: "PORTA".to_string(),
            ty: pinker_v0::ir::TypeIR::Pointer { is_volatile: true },
            value: pinker_v0::cfg_ir::OperandIR::Int(8192),
        }],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Pointer { is_volatile: true },
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![MachineInstr::LoadGlobal("PORTA".to_string())],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let out = interpreter::run_program(&program).unwrap();
    assert_eq!(out, Some(RuntimeValue::Ptr(8192)));
}

#[test]
fn run_dereferencia_de_leitura_via_seta_bombom() {
    let out = run_code(
        "pacote main;
         eterno BASE: bombom = 77;
         carinho ler(p: seta<bombom>) -> bombom { mimo *p; }
         carinho principal() -> bombom {
             nova p: seta<bombom> = 1;
             mimo ler(p);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(77)));
}

#[test]
fn run_dereferencia_falha_com_endereco_invalido() {
    let err = run_code(
        "pacote main;
         eterno BASE: bombom = 77;
         carinho principal() -> bombom {
             nova p: seta<bombom> = 99;
             mimo *p;
         }",
    )
    .unwrap_err();
    assert!(
        err.contains("deref_load em endereço inválido ou não inicializado"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_escrita_indireta_via_seta_bombom() {
    let out = run_code(
        "pacote main;
         eterno BASE: bombom = 10;
         carinho principal() -> bombom {
             nova p: seta<bombom> = 1;
             *p = 123;
             mimo *p;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(123)));
}

#[test]
fn run_fragil_seta_bombom_tem_efeito_operacional_minimo() {
    let out = run_code(
        "pacote main;
         eterno BASE: bombom = 10;
         carinho principal() -> bombom {
             nova p: fragil seta<bombom> = 1 virar fragil seta<bombom>;
             *p = 88;
             mimo *p;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(88)));
}

#[test]
fn run_escrita_indireta_falha_com_endereco_invalido() {
    let err = run_code(
        "pacote main;
         eterno BASE: bombom = 10;
         carinho principal() -> bombom {
             nova p: seta<bombom> = 99;
             *p = 1;
             mimo 0;
         }",
    )
    .unwrap_err();
    assert!(
        err.contains("deref_store em endereço inválido ou não inicializado"),
        "mensagem: {}",
        err
    );
}

#[test]
fn run_aritmetica_ponteiro_offset_suporta_leitura_indireta() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         carinho principal() -> bombom {
             nova p: seta<bombom> = 1;
             nova q: seta<bombom> = p + 1;
             mimo *q;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(20)));
}

#[test]
fn run_aritmetica_ponteiro_offset_suporta_escrita_indireta() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         carinho principal() -> bombom {
             nova p: seta<bombom> = 2;
             nova q: seta<bombom> = p - 1;
             *q = 99;
             mimo *q;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(99)));
}

#[test]
fn run_cast_memoria_bombom_para_seta_bombom_e_volta_funciona() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 33;
         carinho principal() -> bombom {
             nova endereco: bombom = 1;
             nova p: seta<bombom> = endereco virar seta<bombom>;
             nova raw: bombom = p virar bombom;
             nova q: seta<bombom> = raw virar seta<bombom>;
             mimo *q;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(33)));
}

#[test]
fn run_acesso_campo_operacional_em_ninho_via_ponteiro() {
    let out = run_code(
        "pacote main;
         ninho Par { a: bombom; b: bombom; }
         eterno A: bombom = 11;
         eterno F1: bombom = 0;
         eterno F2: bombom = 0;
         eterno F3: bombom = 0;
         eterno F4: bombom = 0;
         eterno F5: bombom = 0;
         eterno F6: bombom = 0;
         eterno F7: bombom = 0;
         eterno B: bombom = 22;
         carinho principal() -> bombom {
             nova p: seta<Par> = 1;
             mimo (*p).b;
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(22)));
}

#[test]
fn run_indexacao_operacional_em_array_via_seta_funciona() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         eterno C: bombom = 30;
         carinho principal() -> bombom {
             nova base: seta<[bombom; 3]> = 1;
             mimo (*base)[2];
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(30)));
}

#[test]
fn run_indexacao_operacional_falha_para_base_array_por_valor() {
    let err = run_code(
        "pacote main;
         carinho pega(a: [bombom; 3]) -> bombom {
             mimo a[1];
         }
         carinho principal() -> bombom { mimo 0; }",
    )
    .unwrap_err();
    assert!(err.contains("(*ptr)[i]"), "mensagem: {}", err);
}

#[test]
fn run_falha_quando_usa_ponteiro_em_operacao_nao_suportada() {
    let mut slot_types = HashMap::new();
    slot_types.insert(
        "p".to_string(),
        pinker_v0::ir::TypeIR::Pointer { is_volatile: false },
    );

    let program = MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec!["p".to_string()],
            slot_types,
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code: vec![
                    MachineInstr::PushInt(1024),
                    MachineInstr::StoreSlot("p".to_string()),
                    MachineInstr::LoadSlot("p".to_string()),
                    MachineInstr::LoadSlot("p".to_string()),
                    MachineInstr::Add,
                ],
                terminator: MachineTerminator::Ret,
            }],
        }],
    };

    let err = interpreter::run_program(&program).unwrap_err().to_string();
    assert!(
        err.contains("add exige inteiros ou 'seta<bombom> + bombom'"),
        "mensagem: {}",
        err
    );
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
fn run_falha_modulo_por_zero() {
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
                    MachineInstr::Mod,
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
        err.contains("at principal [bloco: entry] [instr: mod]"),
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
        err.contains("profundidade máxima de chamadas (64)"),
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
            ty: pinker_v0::ir::TypeIR::Bombom,
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
fn run_signed_i32_em_runtime_funciona() {
    let out = run_code(
        "pacote main;
         carinho soma(a: i32, b: i32) -> i32 { mimo a + b; }
         carinho principal() -> bombom {
             nova base: i32 = 5;
             nova x: i32 = -base;
             nova y: i32 = soma(x, 2);
             talvez y < 0 { mimo 1; } senao { mimo 0; }
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_signed_retorno_e_chamada_funcionam() {
    let out = run_code(
        "pacote main;
         carinho delta(a: i64, b: i64) -> i64 { mimo a - b; }
         carinho principal() -> bombom {
             nova a: i64 = 10;
             nova b: i64 = 3;
             nova d: i64 = delta(-a, -b);
             nova sete: i64 = 7;
             talvez d == -sete { mimo 1; } senao { mimo 0; }
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

// Regressão HF-2: literal no lado esquerdo de op não-comutativa com signed no RHS.
// normalize_numeric_pair invertia a ordem dos operandos nesse caso.
#[test]
fn run_signed_literal_lhs_operacoes_nao_comutativas() {
    // sub: 10 - v (v=3) deve ser 7, não -7
    let out = run_code(
        "pacote main;
         carinho sub_lhs(v: i32) -> i32 { mimo 10 - v; }
         carinho principal() -> bombom {
             nova r: i32 = sub_lhs(3);
             talvez r == 7 { mimo 1; } senao { mimo 0; }
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)), "10 - 3 deve ser 7");

    // cmp_lt: 5 < v (v=3) deve ser falso, não verdade
    let out2 = run_code(
        "pacote main;
         carinho cmp_lhs(v: i32) -> logica { mimo 5 < v; }
         carinho principal() -> bombom {
             nova r: logica = cmp_lhs(3);
             talvez r { mimo 0; } senao { mimo 1; }
         }",
    )
    .unwrap();
    assert_eq!(out2, Some(RuntimeValue::Int(1)), "5 < 3 deve ser falso");
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
fn run_modulo() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova a = 10; nova b = 4; mimo a % b; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(2)));
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
        run_code("pacote main; carinho principal() -> bombom { nova muda x = 1; x = 99; mimo x; }")
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
    assert!(
        stderr.contains("  localização: indisponível"),
        "stderr: {}",
        stderr
    );
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

fn run_cli_example_with_args(path: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .arg("--")
        .args(args)
        .output()
        .unwrap()
}

fn run_cli_example_with_env_and_cwd(
    path: &str,
    set_env: &[(&str, &str)],
    unset_env: &[&str],
    cwd: Option<&std::path::Path>,
) -> std::process::Output {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pink"));
    cmd.arg("--run").arg(path);
    for (key, value) in set_env {
        cmd.env(key, value);
    }
    for key in unset_env {
        cmd.env_remove(key);
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output().unwrap()
}

fn run_cli_check_example(path: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(path)
        .output()
        .unwrap()
}

fn run_cli_example_with_stdin(path: &str, stdin_data: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("falha ao executar CLI --run com stdin");
    child
        .stdin
        .as_mut()
        .expect("stdin do processo filho indisponível")
        .write_all(stdin_data.as_bytes())
        .expect("falha ao escrever stdin do teste");
    child
        .wait_with_output()
        .expect("falha ao aguardar saída do processo filho")
}

fn run_cli_build_args(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .args(args)
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
fn cli_run_algoritmo_complexo_fallthrough_if_else() {
    let out = run_cli_example("examples/algoritmo_complexo.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "26\n");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_build_gera_artefato_s_no_diretorio_padrao() {
    let temp = std::env::temp_dir().join("pinker_build_fase63_ok");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    let source_path = temp.join("app.pink");
    fs::write(
        &source_path,
        "pacote main; carinho principal() -> bombom { mimo 42; }",
    )
    .unwrap();

    let output = run_cli_build_args(&[source_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Build concluído:"));

    let artifact = std::path::Path::new("build").join("app.s");
    assert!(
        artifact.exists(),
        "artefato não gerado: {}",
        artifact.display()
    );
    let artifact_content = fs::read_to_string(&artifact).unwrap();
    assert!(artifact_content.contains("pinker.text.v0"));
    let _ = fs::remove_file(&artifact);
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn cli_build_com_imports_gera_artefato_no_out_dir() {
    let temp = std::env::temp_dir().join("pinker_build_fase63_imports");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    let source_path = temp.join("main.pink");
    let module_path = temp.join("util.pink");
    let out_dir = temp.join("saida_build");

    fs::write(
        &source_path,
        "pacote main; trazer util.soma2; carinho principal() -> bombom { mimo soma2(40); }",
    )
    .unwrap();
    fs::write(
        module_path,
        "pacote util; carinho soma2(x: bombom) -> bombom { mimo x + 2; }",
    )
    .unwrap();

    let output = run_cli_build_args(&[
        "--out-dir",
        out_dir.to_str().unwrap(),
        source_path.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    let artifact = out_dir.join("main.s");
    assert!(
        artifact.exists(),
        "artefato não gerado: {}",
        artifact.display()
    );
    let artifact_content = fs::read_to_string(&artifact).unwrap();
    assert!(artifact_content.contains(".globl principal"));
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn cli_build_sem_arquivo_falha_com_uso() {
    let output = run_cli_build_args(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Uso:"));
    assert!(stderr.contains("build"));
}

#[test]
fn cli_build_falha_semantica_retorna_erro() {
    let temp = std::env::temp_dir().join("pinker_build_fase63_fail");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    let source_path = temp.join("quebrado.pink");
    fs::write(
        &source_path,
        "pacote main; carinho principal() -> bombom { falar(verdade + 1); mimo 0; }",
    )
    .unwrap();

    let output = run_cli_build_args(&[source_path.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"));
    let _ = fs::remove_dir_all(&temp);
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
    assert!(
        stderr.contains("  localização: indisponível"),
        "stderr: {}",
        stderr
    );
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
    assert!(
        stderr.contains("  localização: indisponível"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn run_sempre_que_simples() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova muda x = 0; sempre que x < 5 { x = x + 1; } mimo x; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(5)));
}

#[test]
fn cli_run_sempre_que_funciona() {
    let output = run_cli_example("examples/run_sempre_que.pink");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

// ── Fase 27b: truncamento de stack trace longo ────────────────────────────

#[test]
fn run_trace_curto_sem_truncamento() {
    // Trace com 2 frames (principal + quebra): não deve ser truncado.
    let err = run_code(
        "pacote main; carinho quebra(x: bombom) -> bombom { mimo x / 0; } carinho principal() -> bombom { mimo quebra(10); }",
    )
    .unwrap_err();

    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("at principal"),
        "principal deve aparecer: {}",
        err
    );
    assert!(err.contains("at quebra"), "quebra deve aparecer: {}", err);
    assert!(
        !err.contains("frames omitidos"),
        "trace curto não deve ter omissão: {}",
        err
    );
}

#[test]
fn run_trace_longo_e_truncado() {
    // Recursão infinita atinge MAX_CALL_DEPTH e produz trace com dezenas de frames.
    // O trace deve ser resumido com linha de omissão.
    let err = run_code(
        "pacote main; carinho loop() -> bombom { mimo loop(); } carinho principal() -> bombom { mimo loop(); }",
    )
    .unwrap_err();

    assert!(err.contains("stack trace:"), "mensagem: {}", err);
    assert!(
        err.contains("frames omitidos"),
        "trace longo deve indicar omissão: {}",
        err
    );
    // Frames iniciais devem estar presentes
    assert!(
        err.contains("at principal"),
        "principal deve aparecer: {}",
        err
    );
    assert!(err.contains("at loop"), "loop deve aparecer: {}", err);
}

#[test]
fn run_trace_longo_preserva_frames_iniciais_e_finais() {
    // Verifica que o trace resumido contém frames do início e do final.
    let err = run_code(
        "pacote main; carinho loop() -> bombom { mimo loop(); } carinho principal() -> bombom { mimo loop(); }",
    )
    .unwrap_err();

    // Frames iniciais: principal (frame 0) e loop (frame 1+) devem aparecer
    assert!(
        err.contains("at principal [bloco: entry] [instr: call]"),
        "frame inicial principal deve aparecer: {}",
        err
    );
    // Frames finais: loop deve aparecer (nos últimos 5)
    assert!(
        err.contains("at loop"),
        "frames finais de loop devem aparecer: {}",
        err
    );
    // Linha de omissão com contagem explícita
    assert!(
        err.contains("frames omitidos"),
        "deve indicar frames omitidos: {}",
        err
    );
}

#[test]
fn cli_run_limite_recursao_trace_truncado_na_saida() {
    // CLI: trace longo de recursão deve aparecer truncado no stderr.
    let out = run_cli_example("examples/run_recursao_limite_cli.pink");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[runtime::limite_recursao_excedido]"),
        "stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("frames omitidos"),
        "trace longo deve ser truncado no CLI: {}",
        stderr
    );
    assert!(
        stderr.contains("at principal"),
        "principal deve aparecer: {}",
        stderr
    );
    assert!(stderr.contains("at loop"), "loop deve aparecer: {}", stderr);
}

#[test]
fn run_sempre_que_com_quebrar_interrompe_loop() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova muda x = 0; sempre que x < 5 { x = x + 1; quebrar; } mimo x; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn cli_run_quebrar_funciona() {
    let output = run_cli_example("examples/run_quebrar.pink");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn run_sempre_que_com_continuar_pula_para_proxima_iteracao() {
    let out = run_code(
        "pacote main; carinho principal() -> bombom { nova muda x = 0; nova muda s = 0; sempre que x < 5 { x = x + 1; talvez x == 3 { continuar; } s = s + x; } mimo s; }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(12)));
}

#[test]
fn cli_run_continuar_funciona() {
    let output = run_cli_example("examples/run_continuar.pink");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "12\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_bitwise_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_bitwise_basico.pink")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "22\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_modulo_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_modulo_basico.pink")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_logica_curto_circuito_and_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_logica_curto_circuito_and.pink")
        .output()
        .expect("falha ao executar CLI --run");

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_run_logica_curto_circuito_or_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_logica_curto_circuito_or.pink")
        .output()
        .expect("falha ao executar CLI --run");

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

#[test]
fn cli_run_unsigned_fixos_funciona() {
    let out = run_cli_example("examples/run_unsigned_basico.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn cli_run_signed_fixos_funciona() {
    let out = run_cli_example("examples/run_signed_basico.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn cli_run_alias_tipo_funciona() {
    let out = run_cli_example("examples/run_alias_tipo_basico.pink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn cli_run_falar_signed_funciona() {
    let out = run_cli_example("examples/fase64_falar_signed.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "-3\nverdade\n0\n");
}

#[test]
fn cli_run_ouvir_bombom_funciona_com_exemplo_versionado() {
    let out = run_cli_example_with_stdin("examples/fase85_ouvir_bombom_valido.pink", "41\n");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn cli_run_ouvir_bombom_invalido_falha_com_erro_claro() {
    let out = run_cli_example_with_stdin("examples/fase85_ouvir_bombom_valido.pink", "abc\n");
    assert!(!out.status.success(), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("entrada inválida para 'ouvir'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_arquivo_leitura_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase86_arquivo_leitura_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n0\n");
}

#[test]
fn run_arquivo_escrita_minima_com_leitura_no_mesmo_handle() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase87_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    fs::write(&file_path, "1\n").expect("falha ao criar arquivo temporário");

    let file_path_literal = file_path.to_string_lossy().replace('\\', "\\\\");
    let code = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{file_path_literal}");
            escrever(h, 42);
            nova v: bombom = ler_arquivo(h);
            fechar(h);
            mimo v;
        }}
    "#
    );

    let out = run_code(&code).expect("execução em --run deve funcionar");
    assert_eq!(out, Some(RuntimeValue::Int(42)));

    let persisted = fs::read_to_string(&file_path).expect("falha ao reler arquivo temporário");
    let _ = fs::remove_file(&file_path);
    assert_eq!(persisted, "42");
}

#[test]
fn run_escrever_falha_com_handle_invalido() {
    let err = run_code("pacote main; carinho principal() -> bombom { escrever(999, 1); mimo 0; }")
        .unwrap_err();
    assert!(
        err.contains("handle inválido em 'escrever'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_criar_arquivo_e_escrever_verso_minimos_funcionam_com_releitura() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase101_escrever_verso_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = criar_arquivo("{}");
            escrever_verso(h, "olá pinker");
            nova lido: verso = ler_verso_arquivo(h);
            fechar(h);
            falar(lido);
            mimo tamanho_verso(lido);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(10)));
    let persisted = std::fs::read_to_string(&file_path).expect("falha ao reler arquivo");
    assert_eq!(persisted, "olá pinker");
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_escrever_verso_falha_com_handle_invalido() {
    let err = run_code(
        r#"pacote main;
        carinho principal() -> bombom {
            escrever_verso(999, "x");
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("handle inválido em 'escrever_verso'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_truncar_arquivo_minimo_funciona_e_reflete_em_tamanho_e_vazio() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_fase102_truncar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = criar_arquivo("{}");
            escrever_verso(h, "conteudo fase 102");
            truncar_arquivo(h);
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova t: bombom = tamanho_arquivo("{}");
            nova v: logica = e_vazio("{}");
            falar(t, v, tamanho_verso(texto));
            talvez t == 0 {{
                talvez v {{
                    talvez tamanho_verso(texto) == 0 {{
                        mimo 1;
                    }} senao {{
                        mimo 0;
                    }}
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\"),
        file_path.to_string_lossy().replace('\\', "\\\\"),
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let persisted = std::fs::read_to_string(&file_path).expect("falha ao reler arquivo");
    assert_eq!(persisted, "");
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_truncar_arquivo_falha_com_handle_invalido() {
    let err = run_code(
        r#"pacote main;
        carinho principal() -> bombom {
            truncar_arquivo(999);
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("handle inválido em 'truncar_arquivo'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_truncar_arquivo_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_fase102_truncar_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "x").expect("falha ao preparar arquivo");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            truncar_arquivo(h);
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'truncar_arquivo'"),
        "erro: {}",
        err
    );
}

#[test]
fn cli_run_arquivo_escrita_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase87_arquivo_escrita_minima_valido.pink");
    fs::write("examples/fase87_output_numero.txt", "1\n")
        .expect("falha ao restaurar fixture da fase 87");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n0\n");
}

#[test]
fn run_abrir_anexo_e_anexar_verso_minimos_funcionam_com_releitura() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase108_append_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);

    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova alvo: verso = "{}";
            nova criado: bombom = criar_arquivo(alvo);
            escrever_verso(criado, "base");
            fechar(criado);
            nova h: bombom = abrir_anexo(alvo);
            anexar_verso(h, "-A");
            anexar_verso(h, "-B");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova tam: bombom = tamanho_arquivo(alvo);
            falar(texto, tam);
            talvez igual_verso(texto, "base-A-B") {{
                talvez tam == 8 {{
                    mimo 1;
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_anexar_verso_falha_com_handle_invalido() {
    let err = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            anexar_verso(999, "x");
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("handle inválido em 'anexar_verso'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_anexar_verso_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase108_append_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "base").expect("falha ao criar arquivo temporário");

    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir_anexo("{}");
            fechar(h);
            anexar_verso(h, "x");
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("handle já fechado em 'anexar_verso'"),
        "erro: {}",
        err
    );
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_anexar_verso_falha_em_handle_aberto_sem_append() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase108_append_modo_errado_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "base").expect("falha ao criar arquivo temporário");

    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            anexar_verso(h, "x");
            fechar(h);
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("handle não foi aberto com 'abrir_anexo' em 'anexar_verso'"),
        "erro: {}",
        err
    );
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_abrir_anexo_falha_com_caminho_invalido() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir_anexo("/pinker/fase108/caminho/invalido/arquivo.txt");
            fechar(h);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao abrir arquivo em 'abrir_anexo'"),
        "erro: {}",
        err
    );
}

#[test]
fn run_criar_arquivo_e_escrever_verso_integram_com_argumento_ou_e_juntar_caminho() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase101_integrado_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome: verso = argumento_ou(0, "saida.txt");
            nova alvo: verso = juntar_caminho(base, nome);
            nova h: bombom = criar_arquivo(alvo);
            escrever_verso(h, "ok");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            falar(caminho_existe(alvo), e_arquivo(alvo), texto);
            talvez caminho_existe(alvo) {{
                talvez e_arquivo(alvo) {{
                    talvez tamanho_verso(texto) == 2 {{
                        mimo 1;
                    }} senao {{
                        mimo 0;
                    }}
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        base_dir.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(base_dir.join("saida.txt"));
    let _ = std::fs::remove_dir(&base_dir);
}

#[test]
fn cli_run_truncamento_minimo_fase102_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase102_truncamento_minimo_arquivo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "0 verdade 0\n0\n");
}

#[test]
fn cli_run_observacao_textual_minima_fase103_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase103_observacao_textual_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n1\n");
}

#[test]
fn cli_run_observacao_textual_complementar_minima_fase104_funciona_com_exemplo_versionado() {
    let out =
        run_cli_example("examples/fase104_observacao_textual_complementar_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n1\n");
}

#[test]
fn cli_run_saneamento_textual_minimo_fase105_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase105_saneamento_textual_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade\n1\n");
}

#[test]
fn cli_run_normalizacao_minima_caixa_fase106_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase106_normalizacao_minima_caixa_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n1\n");
}

#[test]
fn cli_run_observacao_textual_posicional_minima_fase107_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase107_observacao_textual_posicional_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7 verdade\n1\n");
}

#[test]
fn cli_run_append_textual_minimo_fase108_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase108_append_textual_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "base+A+B 8\n1\n");
}

#[test]
fn cli_run_verso_operacional_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase88_verso_operacional_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "olá verso\n0\n");
}

#[test]
fn cli_run_verso_operacoes_minimas_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase89_verso_operacoes_minimas_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi Pinker\n9\n0\n");
}

#[test]
fn cli_run_indice_verso_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase90_verso_indexacao_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "n\n1\n0\n");
}

#[test]
fn cli_run_falar_multiplos_argumentos_mistos_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase91_falar_multiplos_argumentos_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "oi Pinker 2\nstatus verdade\n0\n"
    );
}

#[test]
fn cli_run_argumento_posicional_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example_with_args(
        "examples/fase92_tooling_base_argumento_status_valido.pink",
        &["Pinker"],
    );
    assert!(!out.status.success(), "{:?}", out);
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi Pinker\n");
}

#[test]
fn cli_run_argumento_faltando_falha_com_erro_claro() {
    let out = run_cli_example("examples/fase92_tooling_base_argumento_status_valido.pink");
    assert!(!out.status.success(), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("índice fora da faixa em 'argumento'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_quantos_e_tem_argumento_minimos_funcionam() {
    let source = r#"pacote main;
carinho principal() -> bombom {
    falar(quantos_argumentos());
    talvez tem_argumento(1) {
        falar(argumento(1));
        sair(9);
        mimo 0;
    } senao {
        falar("faltou");
        mimo 1;
    }
}"#;
    let file = std::env::temp_dir().join("pinker_fase93_argv_minimo_ok.pink");
    fs::write(&file, source).expect("falha ao gravar fonte temporária");
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .arg("--")
        .arg("a")
        .arg("beta")
        .output()
        .expect("falha ao executar CLI --run");

    assert!(!output.status.success(), "{:?}", output);
    assert_eq!(output.status.code(), Some(9));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2\nbeta\n");
}

#[test]
fn cli_run_argumento_ou_fallback_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase94_argumento_ou_fallback_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi visitante\n9\n");
}

#[test]
fn cli_run_argumento_ou_prioriza_arg_existente_com_exemplo_versionado() {
    let out = run_cli_example_with_args(
        "examples/fase94_argumento_ou_fallback_minimo_valido.pink",
        &["Pinker"],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi Pinker\n6\n");
}

#[test]
fn run_caminho_existe_intrinseca_true_para_arquivo_existente() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez caminho_existe("README.md") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_caminho_existe_intrinseca_false_para_caminho_ausente() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez caminho_existe("__pinker_fase96_nao_existe__.pink") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_e_arquivo_intrinseca_distingue_arquivo_de_diretorio() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova cwd: verso = diretorio_atual();
            falar(cwd, caminho_existe(cwd), e_arquivo(cwd));
            talvez e_arquivo("README.md") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_e_diretorio_intrinseca_true_para_diretorio_existente() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez e_diretorio(".") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_e_diretorio_intrinseca_false_para_arquivo_e_caminho_ausente() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova arquivo: logica = e_diretorio("README.md");
            nova ausente: logica = e_diretorio("__pinker_fase97_nao_existe__.pink");
            talvez arquivo {
                mimo 7;
            } senao {
                talvez ausente {
                    mimo 8;
                } senao {
                    mimo 1;
                }
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_juntar_caminho_intrinseca_compoe_sem_prometer_canonicalizacao() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova cwd: verso = diretorio_atual();
            nova alvo: verso = juntar_caminho(cwd, argumento_ou(0, "README.md"));
            falar(alvo, caminho_existe(alvo), e_diretorio(alvo));
            talvez caminho_existe(alvo) {
                talvez e_diretorio(alvo) {
                    mimo 2;
                } senao {
                    mimo 1;
                }
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tamanho_arquivo_intrinseca_retorna_tamanho_de_arquivo_existente() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase98_tamanho_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "12345").expect("falha ao gravar arquivo temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            mimo tamanho_arquivo("{}");
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(5)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_e_vazio_intrinseca_true_para_arquivo_vazio() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase98_vazio_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "").expect("falha ao gravar arquivo vazio temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            talvez e_vazio("{}") {{
                mimo 1;
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_tamanho_arquivo_intrinseca_falha_para_caminho_ausente() {
    let err = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_arquivo("__pinker_fase98_nao_existe__.txt");
        }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("falha ao obter metadados em 'tamanho_arquivo'"));
}

#[test]
fn run_tamanho_arquivo_e_e_vazio_integram_com_argumento_ou_e_juntar_caminho() {
    let out = run_code(
        r#"
        pacote main;
        carinho principal() -> bombom {
            nova base: verso = diretorio_atual();
            nova nome: verso = argumento_ou(0, "README.md");
            nova alvo: verso = juntar_caminho(base, nome);
            nova t: bombom = tamanho_arquivo(alvo);
            nova v: logica = e_vazio(alvo);
            falar(alvo, caminho_existe(alvo), e_arquivo(alvo), t, v);
            talvez t > 0 {
                talvez v {
                    mimo 0;
                } senao {
                    mimo 1;
                }
            } senao {
                mimo 0;
            }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_criar_diretorio_intrinseca_cria_diretorio_simples() {
    let mut dir_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase99_dir_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    dir_path.push(unique);
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            criar_diretorio("{}");
            talvez e_diretorio("{}") {{
                mimo 1;
            }} senao {{
                mimo 0;
            }}
        }}"#,
        dir_path.to_string_lossy().replace('\\', "\\\\"),
        dir_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_dir(&dir_path);
}

#[test]
fn run_remover_arquivo_intrinseca_remove_arquivo_simples() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase99_rm_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "42").expect("falha ao criar arquivo temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            remover_arquivo("{}");
            talvez caminho_existe("{}") {{
                mimo 0;
            }} senao {{
                mimo 1;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\"),
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_remover_arquivo_intrinseca_falha_para_diretorio() {
    let mut dir_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase99_rm_dir_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    dir_path.push(unique);
    std::fs::create_dir(&dir_path).expect("falha ao criar diretório temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            remover_arquivo("{}");
            mimo 0;
        }}"#,
        dir_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err().to_string();
    assert!(err.contains("falha ao remover arquivo em 'remover_arquivo'"));
    let _ = std::fs::remove_dir(&dir_path);
}

#[test]
fn run_remover_diretorio_intrinseca_remove_diretorio_vazio() {
    let mut dir_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase100_rm_dir_ok_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    dir_path.push(unique);
    std::fs::create_dir(&dir_path).expect("falha ao criar diretório temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            remover_diretorio("{}");
            talvez caminho_existe("{}") {{
                mimo 0;
            }} senao {{
                mimo 1;
            }}
        }}"#,
        dir_path.to_string_lossy().replace('\\', "\\\\"),
        dir_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_remover_diretorio_intrinseca_falha_para_diretorio_nao_vazio() {
    let mut dir_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase100_rm_dir_fail_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    dir_path.push(unique);
    std::fs::create_dir(&dir_path).expect("falha ao criar diretório temporário");
    let child_path = dir_path.join("conteudo.txt");
    std::fs::write(&child_path, "conteudo").expect("falha ao criar arquivo no diretório");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            remover_diretorio("{}");
            mimo 0;
        }}"#,
        dir_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err().to_string();
    assert!(err.contains("falha ao remover diretório em 'remover_diretorio'"));
    let _ = std::fs::remove_file(&child_path);
    let _ = std::fs::remove_dir(&dir_path);
}

#[test]
fn run_ler_verso_arquivo_intrinseca_retorna_texto_completo() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase100_ler_verso_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "linha 1\nlinha 2\n").expect("falha ao criar arquivo temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova t: verso = ler_verso_arquivo(h);
            fechar(h);
            falar(t);
            mimo tamanho_verso(t);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(16)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_contem_e_comeca_com_integram_com_ler_verso_arquivo() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase103_verso_observacao_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "prefixo: conteudo útil")
        .expect("falha ao criar arquivo da fase 103");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova tem: logica = contem_verso(texto, "conteudo");
            nova prefixo_ok: logica = comeca_com(texto, "prefixo:");
            falar(tem, prefixo_ok);
            talvez tem {{
                talvez prefixo_ok {{
                    mimo 1;
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_termina_com_e_igual_verso_integram_com_ler_verso_arquivo() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase104_verso_observacao_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "status: ok").expect("falha ao criar arquivo da fase 104");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova sufixo_ok: logica = termina_com(texto, "ok");
            nova igual_ok: logica = igual_verso(texto, "status: ok");
            falar(sufixo_ok, igual_ok);
            talvez sufixo_ok {{
                talvez igual_ok {{
                    mimo 1;
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_aparar_e_vazio_verso_integram_com_ler_verso_arquivo() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase105_verso_saneamento_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "   \n\t  ").expect("falha ao criar arquivo da fase 105");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova limpo: verso = aparar_verso(texto);
            nova vazio: logica = vazio_verso(limpo);
            falar(vazio, tamanho_verso(limpo));
            talvez vazio {{
                mimo 1;
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_minusculo_e_maiusculo_verso_integram_com_ler_verso_arquivo_e_contem() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase106_verso_caixa_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "PiNkEr v0").expect("falha ao criar arquivo da fase 106");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova baixo: verso = minusculo_verso(texto);
            nova alto: verso = maiusculo_verso(texto);
            nova ok_baixo: logica = contem_verso(baixo, "pinker");
            nova ok_alto: logica = igual_verso(alto, "PINKER V0");
            falar(ok_baixo, ok_alto);
            talvez ok_baixo {{
                talvez ok_alto {{
                    mimo 1;
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_indice_verso_em_e_nao_vazio_verso_integram_com_ler_verso_arquivo_e_aparar() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase107_verso_posicao_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    std::fs::write(&file_path, "   pinker v0   ").expect("falha ao criar arquivo da fase 107");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova bruto: verso = ler_verso_arquivo(h);
            fechar(h);
            nova texto: verso = aparar_verso(bruto);
            nova pos: bombom = indice_verso_em(texto, "v0");
            nova ok: logica = nao_vazio_verso(texto);
            falar(pos, ok);
            talvez ok {{
                talvez pos == 7 {{
                    mimo 1;
                }} senao {{
                    mimo 0;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn run_remover_diretorio_e_ler_verso_arquivo_integram_com_argumento_ou_e_juntar_caminho() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase100_integrado_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar base temporária");
    let file_path = base_dir.join("entrada.txt");
    std::fs::write(&file_path, "pinker").expect("falha ao criar arquivo temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome_dir: verso = argumento_ou(0, "saida");
            nova nome_arquivo: verso = argumento_ou(1, "entrada.txt");
            nova alvo_dir: verso = juntar_caminho(base, nome_dir);
            nova alvo_arquivo: verso = juntar_caminho(base, nome_arquivo);
            criar_diretorio(alvo_dir);
            nova h: bombom = abrir(alvo_arquivo);
            nova t: verso = ler_verso_arquivo(h);
            fechar(h);
            remover_diretorio(alvo_dir);
            falar(tamanho_verso(t), caminho_existe(alvo_dir), e_diretorio(alvo_dir));
            talvez tamanho_verso(t) > 0 {{
                talvez caminho_existe(alvo_dir) {{
                    mimo 0;
                }} senao {{
                    mimo 1;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        base_dir.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(base_dir.join("saida"));
    let _ = std::fs::remove_dir(&base_dir);
}

#[test]
fn run_criar_diretorio_e_remover_arquivo_integram_com_argumento_ou_e_juntar_caminho() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase99_integrado_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar base temporária");
    let file_path = base_dir.join("temp.txt");
    std::fs::write(&file_path, "99").expect("falha ao criar arquivo temporário");
    let source = format!(
        r#"
        pacote main;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome_dir: verso = argumento_ou(0, "saida");
            nova alvo_dir: verso = juntar_caminho(base, nome_dir);
            criar_diretorio(alvo_dir);
            nova arquivo: verso = juntar_caminho(base, "temp.txt");
            remover_arquivo(arquivo);
            falar(caminho_existe(alvo_dir), e_diretorio(alvo_dir), caminho_existe(arquivo), e_arquivo(arquivo));
            talvez e_diretorio(alvo_dir) {{
                talvez caminho_existe(arquivo) {{
                    mimo 0;
                }} senao {{
                    mimo 1;
                }}
            }} senao {{
                mimo 0;
            }}
        }}"#,
        base_dir.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
    let _ = std::fs::remove_dir(base_dir.join("saida"));
    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(&base_dir);
}

#[test]
fn run_ambiente_ou_intrinseca_usa_fallback_sem_env() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase95_ambiente_processo_minimo_valido.pink",
        &[],
        &["PINKER_TEST_ENV_PHASE95"],
        None,
    );
    assert!(output.status.success(), "status={:?}", output.status);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "visitante\n9\n");
}

#[test]
fn run_ambiente_ou_intrinseca_ler_valor_real_do_ambiente() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase95_ambiente_processo_minimo_valido.pink",
        &[("PINKER_TEST_ENV_PHASE95", "PinkerLab")],
        &[],
        None,
    );
    assert!(output.status.success(), "status={:?}", output.status);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "PinkerLab\n9\n");
}

#[test]
fn cli_run_diretorio_atual_funciona_com_exemplo_versionado() {
    let tmp = std::env::temp_dir().join("pinker_fase95_diretorio_atual");
    fs::create_dir_all(&tmp).unwrap();
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase95_diretorio_atual_minimo_valido.pink",
        &[],
        &[],
        Some(&tmp),
    );
    assert!(output.status.success(), "status={:?}", output.status);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n0\n", tmp.display())
    );
}

#[test]
fn cli_run_introspeccao_caminho_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase96_introspeccao_caminho_minima_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade
verdade
1
"
    );
}

#[test]
fn cli_run_refinamento_caminho_fase97_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase97_refinamento_caminho_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n1\n"
    );
}

#[test]
fn cli_run_refinamento_arquivo_fase98_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase98_refinamento_arquivo_minimo_valido.pink");
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n1\n"
    );
}

#[test]
fn cli_run_refinamento_diretorio_arquivo_fase99_funciona_com_exemplo_versionado() {
    let unique_dir = format!(
        "fase99_saida_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_fase99_cli_rm_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    ));
    std::fs::write(&file_path, "7").expect("falha ao criar arquivo para o exemplo Fase 99");
    let file_arg = file_path.to_string_lossy().to_string();
    let out = run_cli_example_with_args(
        "examples/fase99_refinamento_diretorio_arquivo_minimo_valido.pink",
        &[&unique_dir, &file_arg],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n1\n"
    );
    let _ = std::fs::remove_dir(std::env::current_dir().unwrap().join(unique_dir));
    let _ = std::fs::remove_file(file_path);
}

#[test]
fn cli_run_refinamento_diretorio_texto_fase100_funciona_com_exemplo_versionado() {
    let output = run_cli_example_with_args(
        "examples/fase100_refinamento_diretorio_texto_minimo_valido.pink",
        &["fase100_saida_teste", "README.md"],
    );
    assert!(output.status.success(), "status={:?}", output.status);
    let cwd = std::env::current_dir().expect("cwd indisponível");
    let dir_path = cwd.join("fase100_saida_teste");
    assert!(
        !dir_path.exists(),
        "diretório temporário deveria ter sido removido"
    );
    if dir_path.exists() {
        let _ = std::fs::remove_dir(&dir_path);
    }
}

#[test]
fn cli_run_escrita_textual_minima_fase101_funciona_com_exemplo_versionado() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase101_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase101");
    let out = run_cli_example_with_args(
        "examples/fase101_escrita_textual_minima_arquivo_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "fase101_saida.txt",
            "texto fase101",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "texto fase101\n1\n");
    let persisted = std::fs::read_to_string(base_dir.join("fase101_saida.txt"))
        .expect("falha ao reler saída da fase101");
    assert_eq!(persisted, "texto fase101");
    let _ = std::fs::remove_file(base_dir.join("fase101_saida.txt"));
    let _ = std::fs::remove_dir(&base_dir);
}

#[test]
fn cli_run_argumento_ou_e_ambiente_ou_combinados_funcionam() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase95_argumento_ou_ambiente_ou_valido.pink")
        .arg("--")
        .arg("cli")
        .env("PINKER_TEST_ENV_PHASE95", "env")
        .output()
        .unwrap();
    assert!(output.status.success(), "status={:?}", output.status);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "cli\n3\n");
}

#[test]
fn cli_run_corpus_tooling_verso_minimo_funciona_com_exemplo_dedicado() {
    let out = run_cli_example_with_args(
        "examples/run_corpus_tooling_verso_minimo.pink",
        &["Pinker", "beta"],
    );
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "oi Pinker 9 o\nextra beta\n2\n"
    );
}

#[test]
fn cli_run_abrir_arquivo_inexistente_falha_com_erro_claro() {
    let mut script_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase86_{}_{}.pink",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    script_path.push(unique);
    std::fs::write(
        &script_path,
        r#"pacote t;
carinho principal() -> bombom {
    nova h: bombom = abrir("arquivo_que_nao_existe_12345.txt");
    fechar(h);
    mimo 0;
}"#,
    )
    .expect("falha ao gravar script temporário");

    let out = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&script_path)
        .output()
        .expect("falha ao executar CLI --run");
    let _ = std::fs::remove_file(&script_path);

    assert!(!out.status.success(), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("falha ao abrir arquivo em 'abrir'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_quebrar_fora_de_loop_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_quebrar_fora_loop.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("'quebrar' só pode ser usado dentro de 'sempre que'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_continuar_fora_de_loop_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_continuar_fora_loop.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("'continuar' só pode ser usado dentro de 'sempre que'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_modulos_imports_valido_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase60_modulos_valido.pink");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
}

#[test]
fn cli_check_alias_tipo_inexistente_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_alias_tipo_inexistente.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("tipo 'Fantasma' não existe"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_ausente_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase60_modulo_ausente.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("módulo 'nao_existe' não encontrado"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_simbolo_ausente_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase60_simbolo_ausente.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("símbolo 'nao_existe' não encontrado no módulo 'fase60_modulo_util'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_verso_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase61_verso_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_cfg_ir_verso_falha_claro_com_exemplo_versionado() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--cfg-ir")
        .arg("examples/fase61_verso_cfg_ir_invalido.pink")
        .output()
        .expect("falha ao executar CLI --cfg-ir");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("constante global 'verso' ainda não é lowerada"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_volatile_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_volatile_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_check_volatile_invalido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_volatile_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Sintático:") || stderr.contains("Erro Semântico:"));
    assert!(
        stderr.contains("'fragil' só pode qualificar tipo seta"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_fragil_operacional_minimo_funciona_com_exemplo_versionado() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase72_fragil_operacional_minimo_valido.pink")
        .output()
        .expect("falha ao executar CLI --run");
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("89"), "stdout: {}", stdout);
}

#[test]
fn cli_check_fragil_operacional_fora_subset_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase72_fragil_operacional_minimo_invalido.pink");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dereferência nesta fase aceita apenas"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_inline_asm_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_inline_asm_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_check_inline_asm_invalido_vazio_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_inline_asm_invalido_vazio.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("não pode conter string vazia"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_freestanding_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_freestanding_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_check_freestanding_invalido_fora_topo_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_freestanding_invalido_fora_topo.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Sintático:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("marcador `livre;` apenas uma vez no topo do programa"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_boot_entry_livre_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_boot_entry_livre_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_check_kernel_minimo_fase59_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_kernel_minimo_fase59_valido.pink");
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn cli_check_boot_entry_livre_sem_principal_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_boot_entry_livre_sem_principal.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("boot entry desta fase em modo `livre`"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_dereferencia_leitura_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase66_deref_leitura_valido.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("77"), "stdout={}", stdout);
}

#[test]
fn cli_check_dereferencia_seta_u8_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase66_deref_seta_u8_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha semântica para deref fora do subset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("apenas 'seta<bombom>'"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_escrita_indireta_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase67_escrita_indireta_valida.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("123"), "stdout={}", stdout);
}

#[test]
fn cli_check_escrita_indireta_seta_u8_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase67_escrita_indireta_seta_u8_invalida.pink");
    assert!(
        !output.status.success(),
        "esperava falha semântica para escrita indireta fora do subset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("apenas 'seta<bombom>'"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_aritmetica_ponteiro_valida_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase68_ptr_aritmetica_valida.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("20"), "stdout={}", stdout);
}

#[test]
fn cli_run_aritmetica_ponteiro_leitura_valida_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase68_ptr_aritmetica_leitura_valida.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("99"), "stdout={}", stdout);
}

#[test]
fn cli_check_aritmetica_ponteiro_invalida_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase68_ptr_aritmetica_invalida.pink");
    assert!(
        !output.status.success(),
        "esperava falha semântica para aritmética de ponteiro fora do subset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("apenas 'ptr + bombom'"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_acesso_campo_ninho_operacional_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase69_ninho_campo_operacional_valido.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("22"), "stdout={}", stdout);
}

#[test]
fn cli_run_acesso_campo_ninho_fora_subset_operacional_falha() {
    let output = run_cli_example("examples/fase69_ninho_campo_operacional_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha operacional para acesso em base não ponteiro"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("(*ptr).campo"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_indexacao_operacional_em_array_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase70_indexacao_array_operacional_valido.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("30"), "stdout={}", stdout);
}

#[test]
fn cli_run_indexacao_operacional_em_array_fora_subset_falha_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase70_indexacao_array_operacional_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha operacional para indexação com array por valor"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("(*ptr)[i]"), "stderr={}", stderr);
}

#[test]
fn cli_run_cast_memoria_operacional_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase71_cast_memoria_valido.pink");
    assert!(
        output.status.success(),
        "esperava sucesso, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("55"), "stdout={}", stdout);
}

#[test]
fn cli_check_cast_memoria_fora_subset_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase71_cast_memoria_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha semântica para cast fora do subset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cast explícito inválido nesta fase"),
        "stderr={}",
        stderr
    );
}

// ── Fase 28c: spans/source context em erros de runtime e parser ───────────

#[test]
fn runtime_erro_sem_span_real_mostra_localizacao_indisponivel() {
    // Erro de runtime deve exibir "localização: indisponível" em vez de "span: 1:1..1:1"
    // porque a instrução de máquina não carrega span real.
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
    let err = interpreter::run_program(&program).unwrap_err();
    let rendered = err.render_for_cli();
    assert!(
        rendered.contains("localização: indisponível"),
        "deve indicar localização indisponível: {}",
        rendered
    );
    assert!(
        !rendered.contains("span: 1:1..1:1"),
        "não deve mostrar span dummy: {}",
        rendered
    );
}

#[test]
fn cli_parse_error_mostra_source_context() {
    // Erro de parser deve incluir a linha de origem com indicador de coluna (^)
    let source = "pacote main; carinho principal() -> bombom { mimo 1 + ; }";
    let file = std::env::temp_dir().join("pinker_28c_parse_ctx.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Sintático:"), "stderr: {}", stderr);
    // Source context: deve mostrar a linha e o caret
    assert!(
        stderr.contains("| "),
        "deve mostrar linha de origem: {}",
        stderr
    );
    assert!(
        stderr.contains('^'),
        "deve mostrar caret de coluna: {}",
        stderr
    );
}

#[test]
fn cli_semantic_error_mostra_source_context() {
    // Erro semântico deve incluir a linha de origem com indicador de coluna (^)
    let source = "pacote main; carinho principal() -> bombom { mimo verdade + 1; }";
    let file = std::env::temp_dir().join("pinker_28c_semantic_ctx.pink");
    fs::write(&file, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Pode ser erro semântico ou sintático dependendo da pipeline
    assert!(
        stderr.contains("Erro Semântico:") || stderr.contains("Erro Sintático:"),
        "stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("| "),
        "deve mostrar linha de origem: {}",
        stderr
    );
    assert!(
        stderr.contains('^'),
        "deve mostrar caret de coluna: {}",
        stderr
    );
}

// --- Rodada Paralela-1: negação bitwise dual (~ + nope) ---

#[test]
fn run_bitnot_til_bombom_simples() {
    // ~0 em u64 deve ser u64::MAX
    let out =
        run_code("pacote main; carinho principal() -> bombom { nova x: bombom = 0; mimo ~x; }")
            .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(u64::MAX)));
}

#[test]
fn run_bitnot_nope_bombom_simples() {
    // nope equivale a ~ — resultado idêntico
    let out =
        run_code("pacote main; carinho principal() -> bombom { nova x: bombom = 0; mimo nope x; }")
            .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(u64::MAX)));
}

#[test]
fn run_bitnot_til_e_nope_equivalentes() {
    // ~x e nope x produzem o mesmo resultado
    let out_til =
        run_code("pacote main; carinho principal() -> bombom { nova x: bombom = 12345; mimo ~x; }")
            .unwrap();
    let out_nope = run_code(
        "pacote main; carinho principal() -> bombom { nova x: bombom = 12345; mimo nope x; }",
    )
    .unwrap();
    assert_eq!(out_til, out_nope);
    assert_eq!(out_til, Some(RuntimeValue::Int(!12345u64)));
}

#[test]
fn run_bitnot_inverte_bits_conhecidos() {
    // ~10 deve ser !10u64
    let out =
        run_code("pacote main; carinho principal() -> bombom { nova x: bombom = 10; mimo ~x; }")
            .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(!10u64)));
}

#[test]
fn run_bitnot_duplo_retorna_original() {
    // ~~x == x
    let out =
        run_code("pacote main; carinho principal() -> bombom { nova x: bombom = 42; mimo ~~x; }")
            .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_bitnot_tipo_invalido_rejeita_logica() {
    let err = run_code(
        "pacote main; carinho principal() -> bombom { nova b: logica = verdade; mimo ~b; }",
    )
    .unwrap_err();
    assert!(
        err.contains("negação bitwise requer operando inteiro"),
        "erro inesperado: {}",
        err
    );
}

// ── HF-3: estabilização do Bloco 8 — testes de borda de handles/I/O ──────

#[test]
fn run_hf3_ler_arquivo_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_ler_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "42").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            nova v: bombom = ler_arquivo(h);
            mimo v;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'ler_arquivo'"),
        "esperava erro de handle já fechado, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_ler_verso_arquivo_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_ler_verso_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "texto").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            nova v: verso = ler_verso_arquivo(h);
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'ler_verso_arquivo'"),
        "esperava erro de handle já fechado, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_escrever_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_escrever_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "1").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            escrever(h, 99);
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'escrever'"),
        "esperava erro de handle já fechado, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_escrever_verso_falha_apos_fechar_handle() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_escrever_verso_apos_fechar_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            escrever_verso(h, "texto");
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'escrever_verso'"),
        "esperava erro de handle já fechado, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_fechar_duplo_falha_com_handle_ja_fechado() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_fechar_duplo_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "1").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            fechar(h);
            mimo 0;
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let err = run_code(&source).unwrap_err();
    let _ = std::fs::remove_file(&file_path);
    assert!(
        err.contains("handle já fechado em 'fechar'"),
        "esperava erro de handle já fechado, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_ler_verso_arquivo_retorna_vazio_em_arquivo_vazio() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_ler_verso_vazio_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova v: verso = ler_verso_arquivo(h);
            fechar(h);
            mimo tamanho_verso(v);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    let _ = std::fs::remove_file(&file_path);
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_hf3_escrever_bombom_depois_ler_verso_retorna_texto_numerico() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_cross_type_escrever_ler_verso_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&file_path, "0").expect("falha ao criar fixture");
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            escrever(h, 42);
            nova v: verso = ler_verso_arquivo(h);
            fechar(h);
            mimo tamanho_verso(v);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    let _ = std::fs::remove_file(&file_path);
    // "42" has 2 characters
    assert_eq!(out, Some(RuntimeValue::Int(2)));
}

#[test]
fn run_hf3_tamanho_arquivo_falha_em_diretorio() {
    let source = r#"pacote main;
        carinho principal() -> bombom {
            mimo tamanho_arquivo("/tmp");
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("arquivo regular"),
        "esperava erro de arquivo regular, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_e_vazio_falha_em_diretorio() {
    let source = r#"pacote main;
        carinho principal() -> bombom {
            nova v: logica = e_vazio("/tmp");
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("arquivo regular"),
        "esperava erro de arquivo regular, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_e_vazio_falha_em_caminho_ausente() {
    let source = r#"pacote main;
        carinho principal() -> bombom {
            nova v: logica = e_vazio("/caminho/que/nao/existe/hf3_xyzzy.txt");
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao obter metadados em 'e_vazio'"),
        "esperava erro de metadados, obteve: {}",
        err
    );
}

#[test]
fn run_hf3_criar_arquivo_escrever_verso_ler_verso_fechar_fluxo_completo() {
    let mut file_path = std::env::temp_dir();
    file_path.push(format!(
        "pinker_hf3_fluxo_completo_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source = format!(
        r#"pacote main;
        carinho principal() -> bombom {{
            nova h: bombom = criar_arquivo("{}");
            escrever_verso(h, "pinker hf3");
            nova lido: verso = ler_verso_arquivo(h);
            fechar(h);
            mimo tamanho_verso(lido);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    let _ = std::fs::remove_file(&file_path);
    // "pinker hf3" has 10 characters
    assert_eq!(out, Some(RuntimeValue::Int(10)));
}
