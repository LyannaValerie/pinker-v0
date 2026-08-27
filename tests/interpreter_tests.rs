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
use pinker_v0::ir::{self, TypeIR};
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn assert_cli_completed(output: &Output) {
    assert!(
        output.status.code().is_some(),
        "processo terminou sem status: {output:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "programa válido escreveu diagnóstico: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

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

fn fase162_helper_bin(name: &str) -> &'static str {
    match name {
        "exit0" => env!("CARGO_BIN_EXE_pinker_fase162_exit0"),
        "exit1" => env!("CARGO_BIN_EXE_pinker_fase162_exit1"),
        _ => panic!("helper de processo externo desconhecido: {name}"),
    }
}

fn fase163_helper_bin(name: &str) -> &'static str {
    match name {
        "stdout_ok" => env!("CARGO_BIN_EXE_pinker_fase163_stdout_ok"),
        "stdout_invalido_utf8" => env!("CARGO_BIN_EXE_pinker_fase163_stdout_invalido_utf8"),
        _ => panic!("helper de captura stdout desconhecido: {name}"),
    }
}

fn fase164_helper_bin(name: &str) -> &'static str {
    match name {
        "stderr_ok" => env!("CARGO_BIN_EXE_pinker_fase164_stderr_ok"),
        "stderr_invalido_utf8" => env!("CARGO_BIN_EXE_pinker_fase164_stderr_invalido_utf8"),
        _ => panic!("helper de captura stderr desconhecido: {name}"),
    }
}

fn fase165_helper_bin(name: &str) -> &'static str {
    match name {
        "stdin_ok" => env!("CARGO_BIN_EXE_pinker_fase165_stdin_ok"),
        _ => panic!("helper de stdin mínimo desconhecido: {name}"),
    }
}

fn fase166_helper_bin(name: &str) -> &'static str {
    match name {
        "produtor_pipe_ok" => env!("CARGO_BIN_EXE_pinker_fase166_pipe_produtor"),
        "consumidor_stdin_ok" => fase165_helper_bin("stdin_ok"),
        _ => panic!("helper de pipe mínimo desconhecido: {name}"),
    }
}

fn fase168_helper_bin() -> &'static str {
    env!("CARGO_BIN_EXE_pinker_fase168_argv_um")
}

fn pink_string_literal(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

// @pinker-nav:start evidencia.interpreter.execucao-nucleo-estado-aritmetica-fluxo
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita a execução interpretada mínima — retorno constante, leitura de globais/locais, expressão aritmética, fluxo condicional, negação unária, comparação, operadores bitwise e lógicos — comparando o valor de runtime por igualdade exata.
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

// @pinker-nav:end evidencia.interpreter.execucao-nucleo-estado-aritmetica-fluxo
// @pinker-nav:start evidencia.interpreter.texto-verso-intrinsecas-consulta-transformacao
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre intrínsecas de verso executadas no interpretador (operacional, concat, comprimento, índice, contém, começa/termina com, igual, vazio, não vazio, aparar, minúsculo/maiúsculo, buscar), verificando resultados presentes por igualdade e rejeições por erro; não prova a intrínseca inteira, apenas os casos exercidos.
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
        pacote main; trazer texto.juntar; trazer texto.tamanho;
        carinho junta(a: verso, b: verso) -> verso {
            mimo juntar(a, b);
        }
        carinho principal() -> bombom {
            nova base: verso = "la";
            nova fim: verso = "li";
            nova texto: verso = junta(base, fim);
            falar(texto);
            nova n: bombom = tamanho(texto);
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
        pacote main; trazer texto.indice; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = "lua";
            nova letra: verso = indice(texto, 1);
            falar(letra);
            mimo tamanho(letra);
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_indice_verso_falha_com_indice_fora_da_faixa() {
    let err = run_code(
        r#"
        pacote main; trazer texto.indice;
        carinho principal() -> bombom {
            nova texto: verso = "oi";
            nova letra: verso = indice(texto, 2);
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
        pacote main; trazer texto.contem;
        carinho principal() -> bombom {
            nova ok: logica = contem("pinker v0", "ker");
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
        pacote main; trazer texto.contem;
        carinho principal() -> bombom {
            nova ok: logica = contem("pinker v0", "zzz");
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
        pacote main; trazer texto.comeca_com;
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
        pacote main; trazer texto.comeca_com;
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
        pacote main; trazer texto.termina_com;
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
        pacote main; trazer texto.termina_com;
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
        pacote main; trazer texto.igual;
        carinho principal() -> bombom {
            nova ok: logica = igual("pinker", "pinker");
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
        pacote main; trazer texto.igual;
        carinho principal() -> bombom {
            nova ok: logica = igual("pinker", "Pinker");
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
        pacote main; trazer texto.vazio;
        carinho principal() -> bombom {
            nova ok: logica = vazio("");
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
        pacote main; trazer texto.vazio;
        carinho principal() -> bombom {
            nova ok: logica = vazio("x");
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
        pacote main; trazer texto.aparar; trazer texto.igual;
        carinho principal() -> bombom {
            nova limpo: verso = aparar("  pinker  ");
            nova ok: logica = igual(limpo, "pinker");
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
        pacote main; trazer texto.aparar; trazer texto.vazio;
        carinho principal() -> bombom {
            nova limpo: verso = aparar("   ");
            nova ok: logica = vazio(limpo);
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
        pacote main; trazer texto.igual; trazer texto.minusculo;
        carinho principal() -> bombom {
            nova texto: verso = minusculo("PiNkEr V0");
            nova ok: logica = igual(texto, "pinker v0");
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
        pacote main; trazer texto.igual; trazer texto.maiusculo;
        carinho principal() -> bombom {
            nova texto: verso = maiusculo("PiNkEr v0");
            nova ok: logica = igual(texto, "PINKER V0");
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
        pacote main; trazer texto.indice_em;
        carinho principal() -> bombom {
            nova pos: bombom = indice_em("ola pinker", "pin");
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
        pacote main; trazer texto.indice_em;
        carinho principal() -> bombom {
            nova pos: bombom = indice_em("ola pinker", "zzz");
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
fn run_fase140_buscar_verso_retorna_primeira_ocorrencia() {
    let source = r#"pacote main; trazer texto.buscar;
        carinho principal() -> bombom {
            nova pos: bombom = buscar("id=42;id=99", "id=");
            talvez pos == 0 {
                mimo 140;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(140)));
}

#[test]
fn run_fase140_buscar_verso_retorna_u64_max_quando_ausente() {
    let source = r#"pacote main; trazer texto.buscar;
        carinho principal() -> bombom {
            nova pos: bombom = buscar("nome:pinker", "zzz");
            talvez pos == 18446744073709551615 {
                mimo 140;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(140)));
}

#[test]
fn run_fase140_buscar_verso_padrao_vazio_falha() {
    let source = r#"pacote main; trazer texto.buscar;
        carinho principal() -> bombom {
            nova pos: bombom = buscar("abc", "");
            falar(pos);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err().to_string();
    assert!(err.contains("intrínseca 'buscar_verso' não aceita padrão vazio"));
}

#[test]
fn run_nao_vazio_verso_intrinseca_true_em_conteudo_real() {
    let out = run_code(
        r#"
        pacote main; trazer texto.nao_vazio;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio("x");
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
        pacote main; trazer texto.nao_vazio;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio("");
            talvez ok { mimo 0; } senao { mimo 1; }
        }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

// @pinker-nav:end evidencia.interpreter.texto-verso-intrinsecas-consulta-transformacao
// @pinker-nav:start evidencia.interpreter.entrada-argumentos-nomeados-e-flags
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita argumentos posicionais e nomeados no interpretador — argumento, quantos_argumentos, tem_argumento, argumento_ou, tem_chave, pedir_argumento e tem_flag — cobrindo positivos, fallback e rejeições por contains.
#[test]
fn run_argumento_intrinseca_ler_posicional_minimo() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova nome: verso = argumento(0);
            falar("oi", nome);
            mimo tamanho(nome);
        }"#,
        &["Pinker"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(6)));
    assert_eq!(out.exit_status, Some(6));
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
        pacote main; trazer ambiente.quantos_argumentos;
        carinho principal() -> bombom {
            nova total: bombom = quantos_argumentos();
            falar(total);
            mimo total;
        }"#,
        &["um", "dois", "tres"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(3)));
    assert_eq!(out.exit_status, Some(3));
}

#[test]
fn run_tem_argumento_intrinseca_integra_com_argumento() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.argumento; trazer ambiente.tem_argumento; trazer processo.sair; trazer texto.tamanho;
        carinho principal() -> bombom {
            talvez tem_argumento(1) {
                falar(argumento(1));
                mimo tamanho(argumento(1));
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
    assert_eq!(out.exit_status, Some(4));
}

#[test]
fn run_tem_argumento_intrinseca_false_sem_falha_de_argumento() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.argumento; trazer ambiente.tem_argumento; trazer processo.sair;
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
        pacote main; trazer ambiente.argumento_ou; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova nome: verso = argumento_ou(0, "visitante");
            falar("oi", nome);
            mimo tamanho(nome);
        }"#,
        &[],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(9)));
    assert_eq!(out.exit_status, Some(9));
}

#[test]
fn run_argumento_ou_intrinseca_prioriza_arg_existente() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.argumento_ou; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova nome: verso = argumento_ou(0, "visitante");
            falar("oi", nome);
            mimo tamanho(nome);
        }"#,
        &["Pinker"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(6)));
    assert_eq!(out.exit_status, Some(6));
}

#[test]
fn run_tem_chave_intrinseca_true_para_forma_separada() {
    let out = run_code_with_args(
        r#"
        pacote main;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida", "resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tem_chave_intrinseca_true_para_forma_com_igual() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tem_chave_intrinseca_false_quando_ausente() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("--inexistente") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida", "resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_pedir_argumento_retorna_valor_na_forma_separada() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova valor: verso = pedir_argumento("--saida", "padrao");
            falar(valor);
            mimo tamanho(valor);
        }"#,
        &["--saida", "resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(13)));
}

#[test]
fn run_pedir_argumento_retorna_valor_na_forma_com_igual() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova valor: verso = pedir_argumento("--saida", "padrao");
            falar(valor);
            mimo tamanho(valor);
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(13)));
}

#[test]
fn run_pedir_argumento_retorna_padrao_quando_ausente() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova valor: verso = pedir_argumento("--inexistente", "padrao");
            falar(valor);
            mimo tamanho(valor);
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(6)));
}

#[test]
fn run_pedir_argumento_falha_quando_chave_aparece_sem_valor() {
    let err = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento;
        carinho principal() -> bombom {
            nova valor: verso = pedir_argumento("--saida", "padrao");
            falar(valor);
            mimo 0;
        }"#,
        &["--saida"],
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'pedir_argumento' encontrou chave '--saida' sem valor"),
        "erro: {}",
        err
    );
}

#[test]
fn run_tem_chave_rejeita_chave_vazia() {
    let err = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'tem_chave' exige chave não vazia"),
        "erro: {}",
        err
    );
}

#[test]
fn run_pedir_argumento_rejeita_chave_vazia() {
    let err = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento;
        carinho principal() -> bombom {
            nova valor: verso = pedir_argumento("", "padrao");
            falar(valor);
            mimo 0;
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'pedir_argumento' exige chave não vazia"),
        "erro: {}",
        err
    );
}

#[test]
fn run_tem_flag_verdade_quando_flag_presente() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            talvez tem_flag("--quiet") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--quiet"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tem_flag_verdade_para_verbose() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            talvez tem_flag("--verbose") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--verbose"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tem_flag_falso_quando_ausente() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            talvez tem_flag("--inexistente") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--quiet"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_tem_flag_nao_infere_presenca_de_chave_com_valor_separado() {
    // --saida resultado.txt não deve ser detectado como flag booleana --saida
    // tem_flag verifica presença literal apenas: --saida é seguido de valor, mas
    // neste argv o elemento literal "--saida" ainda aparece — então retorna verdade.
    // O teste relevante é: --saida=valor (forma com =) não deve vazar como flag.
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            talvez tem_flag("--saida") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida=resultado.txt"],
    )
    .unwrap();
    // "--saida=resultado.txt" não é igual literal a "--saida", portanto falso
    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_tem_flag_coexiste_com_argumento_nomeado() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.pedir_argumento; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            nova tem_quiet: logica = tem_flag("--quiet");
            nova saida: verso = pedir_argumento("--saida", "padrao.txt");
            talvez tem_quiet {
                falar(saida);
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--quiet", "--saida", "out.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_tem_flag_rejeita_chave_vazia() {
    let err = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom {
            talvez tem_flag("") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--quiet"],
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'tem_flag' exige chave não vazia"),
        "erro: {}",
        err
    );
}

// @pinker-nav:end evidencia.interpreter.entrada-argumentos-nomeados-e-flags
// @pinker-nav:start evidencia.interpreter.entrada-contexto-ambiente-e-saida
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita buscar_contexto com prioridade entre argumento nomeado, ambiente e fallback, suas rejeições de chave, e a saída falar com múltiplos argumentos no interpretador.
#[test]
fn run_buscar_contexto_prioriza_argumento_nomeado() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .arg("--")
        .arg("--saida")
        .arg("out.txt")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "out.txt\nfalso\n2\n"
    );
}

#[test]
fn run_buscar_contexto_usa_ambiente_quando_argumento_ausente() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "env.txt\nfalso\n0\n"
    );
}

#[test]
fn run_buscar_contexto_usa_fallback_quando_ambos_ausentes() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink",
        &[],
        &["PINKER_FASE143_SAIDA"],
        None,
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "padrao.txt\nverdade\n0\n"
    );
}

#[test]
fn run_buscar_contexto_falha_sem_mascarar_valor_ausente_por_ambiente() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .arg("--")
        .arg("--saida")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("intrínseca 'buscar_contexto' encontrou chave '--saida' sem valor"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn run_buscar_contexto_rejeita_chave_de_argumento_vazia() {
    let err = run_code(
        r#"
        pacote main; trazer ambiente.buscar_contexto;
        carinho principal() -> bombom {
            nova valor: verso = buscar_contexto("", "PINKER_FASE143", "padrao");
            falar(valor);
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'buscar_contexto' exige chave não vazia"),
        "erro: {}",
        err
    );
}

#[test]
fn run_buscar_contexto_rejeita_chave_de_ambiente_vazia() {
    let err = run_code(
        r#"
        pacote main; trazer ambiente.buscar_contexto;
        carinho principal() -> bombom {
            nova valor: verso = buscar_contexto("--saida", "", "padrao");
            falar(valor);
            mimo 0;
        }"#,
    )
    .unwrap_err();
    assert!(
        err.contains("intrínseca 'buscar_contexto' exige chave de ambiente não vazia"),
        "erro: {}",
        err
    );
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

// @pinker-nav:end evidencia.interpreter.entrada-contexto-ambiente-e-saida
// @pinker-nav:start evidencia.interpreter.execucao-chamadas-e-curto-circuito
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica chamadas de função (simples, com argumentos na ordem, encadeada, void como statement) e curto-circuito de e/ou que não avalia o lado direito, por igualdade de valor de runtime.
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

// @pinker-nav:end evidencia.interpreter.execucao-chamadas-e-curto-circuito
// @pinker-nav:start evidencia.interpreter.diagnostico-simbolo-inexistente
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de runtime para função e global inexistentes, verificando a mensagem por contains (não igualdade exata).
#[test]
fn run_falha_funcao_inexistente() {
    let program = MachineProgram {
        union_types: vec![],
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
        union_types: vec![],
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

// @pinker-nav:end evidencia.interpreter.diagnostico-simbolo-inexistente
// @pinker-nav:start evidencia.interpreter.ponteiros-seta-operacional
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre a semântica operacional de ponteiros (seta) no interpretador: representação mínima em slot, dereferência de leitura, escrita indireta, efeito frágil, cast de memória e acesso a campo, com rejeição de operação não suportada; mistura casos positivos por igualdade e negativos por erro.
#[test]
fn run_seta_tem_repr_minima_no_runtime_em_slot() {
    let mut slot_types = HashMap::new();
    slot_types.insert(
        "p".to_string(),
        pinker_v0::ir::TypeIR::Pointer { is_volatile: false },
    );

    let program = MachineProgram {
        union_types: vec![],
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
        union_types: vec![],
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
fn run_indexacao_operacional_em_array_por_valor_minima_funciona() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         eterno C: bombom = 30;
         carinho pega(a: [bombom; 3]) -> bombom {
             mimo a[1];
         }
         carinho principal() -> bombom {
             nova base: seta<[bombom; 3]> = 1;
             mimo pega(*base);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(20)));
}

#[test]
fn run_falha_quando_usa_ponteiro_em_operacao_nao_suportada() {
    let mut slot_types = HashMap::new();
    slot_types.insert(
        "p".to_string(),
        pinker_v0::ir::TypeIR::Pointer { is_volatile: false },
    );

    let program = MachineProgram {
        union_types: vec![],
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
                    MachineInstr::Add { ty: TypeIR::Bombom },
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

// @pinker-nav:end evidencia.interpreter.ponteiros-seta-operacional
// @pinker-nav:start evidencia.interpreter.execucao-cli-exemplos-basicos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos .pink básicos versionados através do binário CLI (caso válido e global), comparando a saída renderizada; exercita a superfície de execução via processo, não apenas o interpretador em processo.
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

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
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

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

// ── Fase 16: testes negativos de runtime ──────────────────────────────────

// @pinker-nav:end evidencia.interpreter.execucao-cli-exemplos-basicos
// @pinker-nav:start evidencia.interpreter.diagnostico-runtime-avaliacao-e-chamadas
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera falhas de avaliação e chamadas no runtime — divisão/módulo por zero, stack trace em chamada e recursão finita profunda, slot não inicializado e call sem valor — verificando categoria e trechos por contains.
#[test]
fn run_falha_divisao_por_zero() {
    let program = MachineProgram {
        union_types: vec![],
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
                    MachineInstr::Div { ty: TypeIR::Bombom },
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
        union_types: vec![],
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
                    MachineInstr::Mod { ty: TypeIR::Bombom },
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
fn run_recursao_terminante_atravessa_teto_historico() {
    let result = run_code(
        "pacote main; carinho descer(n: bombom) -> bombom { talvez n == 0 { mimo 42; } senao { mimo descer(n - 1); } } carinho principal() -> bombom { mimo descer(80); }",
    )
    .unwrap();

    assert_eq!(result, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_falha_slot_nao_inicializado() {
    let program = MachineProgram {
        union_types: vec![],
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
        union_types: vec![],
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

// @pinker-nav:end evidencia.interpreter.diagnostico-runtime-avaliacao-e-chamadas
// @pinker-nav:start evidencia.interpreter.diagnostico-runtime-execucao-invalida
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera rejeição de execução quando call void retorna valor, a aridade da chamada é inválida ou o valor global não é suportado, verificando a mensagem por contains.
#[test]
fn run_falha_call_void_retorna_valor() {
    // CallVoid para função que empilha valor e faz Ret: deve falhar com "call_void exige função sem retorno"
    let program = MachineProgram {
        union_types: vec![],
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
        union_types: vec![],
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
        union_types: vec![],
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

// @pinker-nav:end evidencia.interpreter.diagnostico-runtime-execucao-invalida
// @pinker-nav:start evidencia.interpreter.execucao-operadores-aritmeticos-relacionais-e-sinais
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita operadores unários/binários, divisão, módulo, igualdade, diferença, comparações, inteiros com/sem sinal e variável mutável no interpretador, comparando por igualdade exata.
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

// @pinker-nav:end evidencia.interpreter.execucao-operadores-aritmeticos-relacionais-e-sinais
// @pinker-nav:start evidencia.interpreter.execucao-recursao-e-fluxo-interpretador-e-cli
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre chamadas, recursão e fluxo/estado no interpretador e via CLI, incluindo a falha processual com diagnóstico e exit não-zero; compara valores, saída renderizada e trechos do erro.
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

fn run_cli_source(name: &str, source: &str) -> std::process::Output {
    let path = std::env::temp_dir().join(format!("{name}-{}.pink", std::process::id()));
    fs::write(&path, source).expect("gravar fonte temporário");
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&path)
        .output()
        .expect("executar fonte temporário");
    fs::remove_file(path).expect("remover fonte temporário");
    output
}

fn deep_runtime_failure_source(depth: u64) -> String {
    format!(
        "pacote main; carinho queda(n: bombom) -> bombom {{ talvez n == 0 {{ mimo 10 / 0; }} senao {{ mimo queda(n - 1); }} }} carinho principal() -> bombom {{ mimo queda({depth}); }}"
    )
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

fn run_cli_repl_session(stdin_data: &str, args: &[&str]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("repl")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("falha ao executar CLI repl");
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

#[test]
fn cli_run_mantem_exemplos_base() {
    let casos = [
        ("examples/run_soma.pink", 42),
        ("examples/run_chamada.pink", 42),
        ("examples/run_global.pink", 100),
        ("examples/run_global_expr.pink", 44),
    ];

    for (path, expected_exit) in casos {
        let out = run_cli_example(path);
        assert_cli_completed(&out);
        assert!(out.stdout.is_empty(), "path={path}");
        assert_eq!(out.status.code(), Some(expected_exit), "path={path}");
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
        assert_cli_completed(&out);
    }
}

#[test]
fn cli_run_global_com_chamada_exemplo_novo() {
    let out = run_cli_example("examples/run_global_call_combo.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_mutacao_com_if_else_exemplo_novo() {
    let out = run_cli_example("examples/run_mut_if_else.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_recursao_com_global_exemplo_novo() {
    let out = run_cli_example("examples/run_recursao_global.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn cli_run_algoritmo_complexo_fallthrough_if_else() {
    let out = run_cli_example("examples/algoritmo_complexo.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty());
}

// @pinker-nav:end evidencia.interpreter.execucao-recursao-e-fluxo-interpretador-e-cli
// @pinker-nav:start evidencia.backend-s.build-cli-artefato-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita dois builds híbridos via processo `pink build`: exige sucesso, saída esperada, criação do artefato .s no diretório padrão ou em --out-dir e conteúdo textual mínimo, inclusive com import; não monta, linka nem executa o artefato.
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
    assert_cli_completed(&output);
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
    assert_cli_completed(&output);
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    let artifact = out_dir.join("main.s");
    assert!(
        artifact.exists(),
        "artefato não gerado: {}",
        artifact.display()
    );
    let artifact_content = fs::read_to_string(&artifact).unwrap();
    // A superfície textual `pinker.text.v0` preserva a grafia Pinker do
    // entrypoint, que continua global por ser o entrypoint.
    assert!(artifact_content.contains(".globl principal"));
    let _ = fs::remove_dir_all(&temp);
}
// @pinker-nav:end evidencia.backend-s.build-cli-artefato-textual

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

// @pinker-nav:start evidencia.interpreter.execucao-repl-e-render-erro-fonte
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita a sessão REPL (abrir/sair, fluxo mínimo e composto, entrada inválida preservando a sessão) e a renderização de erro com contexto de fonte, verificando saída por contains.
#[test]
fn cli_repl_sem_argumentos_abre_e_sai_com_quit() {
    let output = run_cli_repl_session(":quit\n", &[]);
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== Pinker REPL ==="), "stdout={stdout}");
    assert!(stdout.contains("pinker> "), "stdout={stdout}");
    assert!(
        stdout.contains("Encerrando REPL Pinker."),
        "stdout={stdout}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_repl_fluxo_minimo_com_mimo_exibe_resultado() {
    let output = run_cli_repl_session("mimo 42;\n:quit\n", &[]);
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=> 42"), "stdout={stdout}");
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_repl_fluxo_composto_em_linha_unica_funciona() {
    let output = run_cli_repl_session("nova a: bombom = 40; falar(a + 2);\n:quit\n", &[]);
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pinker> 42\nok\n"), "stdout={stdout}");
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_repl_erro_de_entrada_invalida_preserva_sessao() {
    let output = run_cli_repl_session("nova = ;\nmimo 7;\n:quit\n", &[]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Sintático:"), "stderr={stderr}");
    assert!(stdout.contains("=> 7"), "stdout={stdout}");
}

#[test]
fn cli_repl_sem_arquivo_rejeita_argumento_posicional() {
    let output = run_cli_repl_session("", &["extra"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("repl"), "stderr={stderr}");
    assert!(
        stderr.contains("não aceita argumentos posicionais"),
        "stderr={stderr}"
    );
}

#[test]
fn cli_run_erro_runtime_profundo_tem_saida_previsivel() {
    let source = deep_runtime_failure_source(128);
    let out = run_cli_source("pinker_run_erro_profundo", &source);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Erro Runtime:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("[runtime::divisao_por_zero]"),
        "stderr: {}",
        stderr
    );
    assert!(stderr.contains("stack trace:"), "stderr: {}", stderr);
    assert!(stderr.contains("at principal"), "stderr: {}", stderr);
    assert!(stderr.contains("at queda"), "stderr: {}", stderr);
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

// @pinker-nav:end evidencia.interpreter.execucao-repl-e-render-erro-fonte
// @pinker-nav:start evidencia.interpreter.fluxo-controle-lacos-basicos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica laços sempre_que básicos no interpretador e via exemplo CLI, comparando o resultado e a saída por igualdade.
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

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

// ── Fase 27b: truncamento de stack trace longo ────────────────────────────

// @pinker-nav:end evidencia.interpreter.fluxo-controle-lacos-basicos
// @pinker-nav:start evidencia.interpreter.diagnostico-stack-trace-truncamento
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre a renderização do stack trace — trace curto sem truncamento, trace longo truncado preservando frames iniciais e finais, e truncamento na saída CLI — verificando por contains.
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
    let err = run_code(&deep_runtime_failure_source(128)).unwrap_err();

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
    assert!(err.contains("at queda"), "queda deve aparecer: {}", err);
}

#[test]
fn run_trace_longo_preserva_frames_iniciais_e_finais() {
    let err = run_code(&deep_runtime_failure_source(96)).unwrap_err();

    // Frames iniciais: principal (frame 0) e loop (frame 1+) devem aparecer
    assert!(
        err.contains("at principal [bloco: entry] [instr: call]"),
        "frame inicial principal deve aparecer: {}",
        err
    );
    // Frames finais: queda deve aparecer (nos últimos 5)
    assert!(
        err.contains("at queda"),
        "frames finais de queda devem aparecer: {}",
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
fn cli_run_erro_profundo_trace_truncado_na_saida() {
    let source = deep_runtime_failure_source(128);
    let out = run_cli_source("pinker_run_trace_profundo", &source);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[runtime::divisao_por_zero]"),
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
    assert!(
        stderr.contains("at queda"),
        "queda deve aparecer: {}",
        stderr
    );
}

// @pinker-nav:end evidencia.interpreter.diagnostico-stack-trace-truncamento
// @pinker-nav:start evidencia.interpreter.execucao-operadores-e-fluxo-cli-exemplos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI versionados de operadores (bitwise, curto-circuito lógico, inteiros fixos, alias de tipo) e fluxo, comparando a saída renderizada.
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

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
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

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_bitwise_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_bitwise_basico.pink")
        .output()
        .unwrap();

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_modulo_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_modulo_basico.pink")
        .output()
        .unwrap();

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn cli_run_logica_curto_circuito_and_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_logica_curto_circuito_and.pink")
        .output()
        .expect("falha ao executar CLI --run");

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn cli_run_logica_curto_circuito_or_funciona() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/run_logica_curto_circuito_or.pink")
        .output()
        .expect("falha ao executar CLI --run");

    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn cli_run_unsigned_fixos_funciona() {
    let out = run_cli_example("examples/run_unsigned_basico.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
}

#[test]
fn cli_run_signed_fixos_funciona() {
    let out = run_cli_example("examples/run_signed_basico.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
}

#[test]
fn cli_run_alias_tipo_funciona() {
    let out = run_cli_example("examples/run_alias_tipo_basico.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
}

#[test]
fn cli_run_falar_signed_funciona() {
    let out = run_cli_example("examples/fase64_falar_signed.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "-3\nverdade\n");
}

// @pinker-nav:end evidencia.interpreter.execucao-operadores-e-fluxo-cli-exemplos
// @pinker-nav:start evidencia.interpreter.texto-io-por-handle-e-arquivos-releitura
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita I/O textual por handle de arquivo (criar/escrever/truncar/anexar verso, ler_verso, ouvir_verso) no interpretador e via exemplos CLI, cobrindo releitura, EOF e rejeições por handle inválido; observado, não exaustivo.
#[test]
fn cli_run_ouvir_bombom_funciona_com_exemplo_versionado() {
    let out = run_cli_example_with_stdin("examples/fase85_ouvir_bombom_valido.pink", "41\n");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
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
fn cli_run_entrada_textual_minima_fase110_funciona_com_exemplo_versionado() {
    let out = run_cli_example_with_stdin(
        "examples/fase110_entrada_textual_minima_valida.pink",
        "  pinker v0  \n",
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "  pinker v0  \nverdade\npadrão-fase110\n"
    );
}

#[test]
fn cli_run_arquivo_leitura_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase86_arquivo_leitura_minima_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
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
        pacote main; trazer arquivo.abrir; trazer arquivo.escrever_bombom; trazer arquivo.fechar; trazer arquivo.ler_bombom;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{file_path_literal}");
            escrever_bombom(h, 42);
            nova v: bombom = ler_bombom(h);
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
    let err = run_code("pacote main; trazer arquivo.escrever_bombom; carinho principal() -> bombom { escrever_bombom(999, 1); mimo 0; }")
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
        pacote main; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = criar("{}");
            escrever_verso(h, "olá pinker");
            nova lido: verso = ler_verso(h);
            fechar(h);
            falar(lido);
            mimo tamanho(lido);
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
        r#"pacote main; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer arquivo.truncar; trazer caminho.arquivo_vazio; trazer caminho.tamanho_arquivo; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = criar("{}");
            escrever_verso(h, "conteudo fase 102");
            truncar(h);
            nova texto: verso = ler_verso(h);
            fechar(h);
            nova t: bombom = tamanho_arquivo("{}");
            nova v: logica = arquivo_vazio("{}");
            falar(t, v, tamanho(texto));
            talvez t == 0 {{
                talvez v {{
                    talvez tamanho(texto) == 0 {{
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
        r#"pacote main; trazer arquivo.truncar;
        carinho principal() -> bombom {
            truncar(999);
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.truncar;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            truncar(h);
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
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
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
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.anexar_verso; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer caminho.tamanho_arquivo; trazer texto.igual;
        carinho principal() -> bombom {{
            nova alvo: verso = "{}";
            nova criado: bombom = criar(alvo);
            escrever_verso(criado, "base");
            fechar(criado);
            nova h: bombom = abrir_anexo(alvo);
            anexar_verso(h, "-A");
            anexar_verso(h, "-B");
            nova texto: verso = ler_verso(h);
            fechar(h);
            nova tam: bombom = tamanho_arquivo(alvo);
            falar(texto, tam);
            talvez igual(texto, "base-A-B") {{
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
        pacote main; trazer arquivo.anexar_verso;
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
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.anexar_verso; trazer arquivo.fechar;
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
        pacote main; trazer arquivo.abrir; trazer arquivo.anexar_verso; trazer arquivo.fechar;
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
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.fechar;
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
fn run_ler_arquivo_verso_minimo_por_caminho_funciona() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase109_ler_arquivo_verso_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);

    let source = format!(
        r#"
        pacote main; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_caminho_verso; trazer texto.contem; trazer texto.igual;
        carinho principal() -> bombom {{
            nova alvo: verso = "{}";
            nova h: bombom = criar(alvo);
            escrever_verso(h, "fase109-ok");
            fechar(h);
            nova texto: verso = ler_caminho_verso(alvo);
            falar(texto, contem(texto, "109"));
            talvez igual(texto, "fase109-ok") {{
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
fn run_arquivo_ou_retorna_padrao_para_caminho_ausente() {
    let source = r#"
        pacote main; trazer arquivo.ler_caminho_ou; trazer texto.igual; trazer texto.nao_vazio;
        carinho principal() -> bombom {
            nova texto: verso = ler_caminho_ou("__pinker_fase109_nao_existe__.txt", "padrao109");
            falar(texto, nao_vazio(texto));
            talvez igual(texto, "padrao109") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_ler_arquivo_verso_falha_com_caminho_invalido() {
    let source = r#"
        pacote main; trazer arquivo.ler_caminho_verso;
        carinho principal() -> bombom {
            nova texto: verso = ler_caminho_verso("/pinker/fase109/caminho/invalido.txt");
            falar(texto);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao ler arquivo em 'ler_arquivo_verso'"),
        "erro: {}",
        err
    );
}

// @pinker-nav:end evidencia.interpreter.texto-io-por-handle-e-arquivos-releitura
// @pinker-nav:start evidencia.interpreter.texto-verso-e-io-textual-por-caminho
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre leitura/escrita textual por caminho, intrínsecas de verso associadas e saída via falar com argumentos mistos, no interpretador e em exemplos CLI, verificando resultados e rejeições.
#[test]
fn run_ouvir_verso_ler_texto_minimo_remove_newline_final() {
    let out = run_cli_example_with_stdin(
        "examples/fase110_entrada_textual_minima_valida.pink",
        "linha110\n",
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "linha110\nverdade\npadrão-fase110\n"
    );
}

#[test]
fn run_ouvir_verso_ou_retorna_padrao_em_eof_imediato() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase110_ouvir_verso_ou_{}_{}.pink",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    fs::write(
        &file_path,
        r#"
pacote main; trazer entrada.ouvir_verso_ou; trazer texto.igual;
carinho principal() -> bombom {
    nova texto: verso = ouvir_verso_ou("padrao110");
    falar(texto, igual(texto, "padrao110"));
    mimo 0;
}"#,
    )
    .expect("falha ao gravar programa temporário");

    let out = run_cli_example_with_stdin(&file_path.to_string_lossy(), "");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "padrao110 verdade\n");
    let _ = fs::remove_file(&file_path);
}

#[test]
fn run_ouvir_verso_falha_com_eof_imediato() {
    let out = run_cli_example_with_stdin("examples/fase110_entrada_textual_minima_valida.pink", "");
    assert!(!out.status.success(), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("falha ao ler stdin em 'ouvir_verso'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn run_ouvir_verso_falha_com_aridade_invalida_no_runtime() {
    let err =
        run_code("pacote main; trazer entrada.ouvir_verso; carinho principal() -> bombom { nova t: verso = ouvir_verso(\"x\"); falar(t); mimo 0; }")
            .unwrap_err();
    assert!(
        err.contains("chamada de 'ouvir_verso' com aridade inválida"),
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
        pacote main; trazer ambiente.argumento_ou; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer caminho.e_arquivo; trazer caminho.existe; trazer caminho.juntar; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome: verso = argumento_ou(0, "saida.txt");
            nova alvo: verso = juntar(base, nome);
            nova h: bombom = criar(alvo);
            escrever_verso(h, "ok");
            nova texto: verso = ler_verso(h);
            fechar(h);
            falar(existe(alvo), e_arquivo(alvo), texto);
            talvez existe(alvo) {{
                talvez e_arquivo(alvo) {{
                    talvez tamanho(texto) == 2 {{
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
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase102_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase102");
    let out = run_cli_example_with_args(
        "examples/fase102_truncamento_minimo_arquivo_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "pinker_fase102_saida.txt",
        ],
    );
    let _ = std::fs::remove_file(base_dir.join("pinker_fase102_saida.txt"));
    let _ = std::fs::remove_dir(&base_dir);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "0 verdade 0\n");
}

#[test]
fn cli_run_observacao_textual_minima_fase103_funciona_com_exemplo_versionado() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase103_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase103");
    let out = run_cli_example_with_args(
        "examples/fase103_observacao_textual_minima_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "pinker_fase103_entrada.txt",
        ],
    );
    let _ = std::fs::remove_file(base_dir.join("pinker_fase103_entrada.txt"));
    let _ = std::fs::remove_dir(&base_dir);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n");
}

#[test]
fn cli_run_observacao_textual_complementar_minima_fase104_funciona_com_exemplo_versionado() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase104_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase104");
    let out = run_cli_example_with_args(
        "examples/fase104_observacao_textual_complementar_minima_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "pinker_fase104_entrada.txt",
        ],
    );
    let _ = std::fs::remove_file(base_dir.join("pinker_fase104_entrada.txt"));
    let _ = std::fs::remove_dir(&base_dir);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n");
}

#[test]
fn cli_run_saneamento_textual_minimo_fase105_funciona_com_exemplo_versionado() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase105_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase105");
    let out = run_cli_example_with_args(
        "examples/fase105_saneamento_textual_minimo_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "pinker_fase105_entrada.txt",
        ],
    );
    let _ = std::fs::remove_file(base_dir.join("pinker_fase105_entrada.txt"));
    let _ = std::fs::remove_dir(&base_dir);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade\n");
}

#[test]
fn cli_run_normalizacao_minima_caixa_fase106_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase106_normalizacao_minima_caixa_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "verdade verdade\n");
}

#[test]
fn cli_run_observacao_textual_posicional_minima_fase107_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase107_observacao_textual_posicional_minima_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7 verdade\n");
}

#[test]
fn cli_run_append_textual_minimo_fase108_funciona_com_exemplo_versionado() {
    let mut base_dir = std::env::temp_dir();
    let unique = format!(
        "pinker_fase108_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    base_dir.push(unique);
    std::fs::create_dir(&base_dir).expect("falha ao criar diretório-base da fase108");
    let out = run_cli_example_with_args(
        "examples/fase108_append_textual_minimo_valido.pink",
        &[
            base_dir.to_string_lossy().as_ref(),
            "fase108_append_textual_minimo.txt",
        ],
    );
    let _ = std::fs::remove_file(base_dir.join("fase108_append_textual_minimo.txt"));
    let _ = std::fs::remove_dir(&base_dir);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "base+A+B 8\n");
}

#[test]
fn cli_run_leitura_textual_direta_por_caminho_fase109_funciona_com_exemplo_versionado() {
    let mut file_path = std::env::temp_dir();
    let unique = format!(
        "pinker_fase109_cli_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock monotônico")
            .as_nanos()
    );
    file_path.push(unique);
    let out = run_cli_example_with_args(
        "examples/fase109_leitura_textual_direta_por_caminho_valido.pink",
        &[file_path.to_string_lossy().as_ref()],
    );
    let _ = std::fs::remove_file(&file_path);
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "fase109\npadrao109\n");
}

#[test]
fn cli_run_verso_operacional_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase88_verso_operacional_minimo_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "olá verso\n");
}

#[test]
fn cli_run_verso_operacoes_minimas_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase89_verso_operacoes_minimas_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi Pinker\n9\n");
}

#[test]
fn cli_run_indice_verso_minimo_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase90_verso_indexacao_minima_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "n\n1\n");
}

#[test]
fn cli_run_falar_multiplos_argumentos_mistos_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase91_falar_multiplos_argumentos_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "oi Pinker 2\nstatus verdade\n"
    );
}

// @pinker-nav:end evidencia.interpreter.texto-verso-e-io-textual-por-caminho
// @pinker-nav:start evidencia.interpreter.entrada-argumentos-e-ambiente-cli-exemplos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI versionados da superfície de entrada — argumentos posicionais/nomeados, quantos, fallback, flags booleanas e buscar_contexto com prioridade sobre o ambiente — comparando saída, erro e código de saída.
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
    let source = r#"pacote main; trazer ambiente.argumento; trazer ambiente.quantos_argumentos; trazer ambiente.tem_argumento; trazer processo.sair;
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
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi visitante\n");
}

#[test]
fn cli_run_argumento_ou_prioriza_arg_existente_com_exemplo_versionado() {
    let out = run_cli_example_with_args(
        "examples/fase94_argumento_ou_fallback_minimo_valido.pink",
        &["Pinker"],
    );
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi Pinker\n");
}

#[test]
fn cli_run_argumentos_nomeados_minimos_funcionam_na_forma_separada() {
    let out = run_cli_example_with_args(
        "examples/fase141_argumentos_nomeados_minimos_valido.pink",
        &["--saida", "out.txt"],
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nout.txt\nsimples\n"
    );
}

#[test]
fn cli_run_argumentos_nomeados_minimos_funcionam_na_forma_com_igual() {
    let out = run_cli_example_with_args(
        "examples/fase141_argumentos_nomeados_minimos_valido.pink",
        &["--saida=out.txt", "--modo=rapido"],
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nout.txt\nrapido\n"
    );
}

#[test]
fn cli_run_argumento_nomeado_sem_valor_falha_com_erro_claro() {
    let out = run_cli_example_with_args(
        "examples/fase141_argumentos_nomeados_minimos_valido.pink",
        &["--saida"],
    );
    assert!(!out.status.success(), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("intrínseca 'pedir_argumento' encontrou chave '--saida' sem valor"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_run_flags_booleanas_minimas_funcionam_com_quiet() {
    let out = run_cli_example_with_args(
        "examples/fase142_flags_booleanas_minimas_valido.pink",
        &["--quiet"],
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nfalso\npadrao.txt\n"
    );
}

#[test]
fn cli_run_flags_booleanas_minimas_funcionam_com_mistura_de_flag_e_nomeado() {
    let out = run_cli_example_with_args(
        "examples/fase142_flags_booleanas_minimas_valido.pink",
        &["--quiet", "--saida", "out.txt"],
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nfalso\nout.txt\n"
    );
}

#[test]
fn cli_run_buscar_contexto_prioriza_saida_do_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .arg("--")
        .arg("--saida")
        .arg("out.txt")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "out.txt\nfalso\n2\n"
    );
}

#[test]
fn cli_run_buscar_contexto_usa_env_sem_saida_no_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "env.txt\nfalso\n0\n"
    );
}

#[test]
fn cli_run_buscar_contexto_usa_fallback_quando_tudo_ausente() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink",
        &[],
        &["PINKER_FASE143_SAIDA"],
        None,
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "padrao.txt\nverdade\n0\n"
    );
}

#[test]
fn cli_run_buscar_contexto_falha_sem_valor_mesmo_com_env() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase143_argumento_nomeado_ou_ambiente_ou_valido.pink")
        .arg("--")
        .arg("--saida")
        .env("PINKER_FASE143_SAIDA", "env.txt")
        .output()
        .expect("falha ao executar CLI --run");
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("intrínseca 'buscar_contexto' encontrou chave '--saida' sem valor"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn run_legado_tem_argumento_nomeado_permanece_operacional() {
    let out = run_code_with_args(
        r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#,
        &["--saida", "resultado.txt"],
    )
    .unwrap();
    assert_eq!(out.return_value, Some(RuntimeValue::Int(1)));
}

// @pinker-nav:end evidencia.interpreter.entrada-argumentos-e-ambiente-cli-exemplos
// @pinker-nav:start evidencia.interpreter.arquivos-introspeccao-caminho-e-diretorios
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita intrínsecas de caminho e diretório no interpretador (existe, é arquivo/diretório, juntar caminho, tamanho, é vazio, criar/remover diretório), cobrindo positivos e rejeições; juntar_caminho não promete canonicalização.
#[test]
fn run_caminho_existe_intrinseca_true_para_arquivo_existente() {
    let out = run_code(
        r#"
        pacote main; trazer caminho.existe;
        carinho principal() -> bombom {
            talvez existe("README.md") {
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
        pacote main; trazer caminho.existe;
        carinho principal() -> bombom {
            talvez existe("__pinker_fase96_nao_existe__.pink") {
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
        pacote main; trazer caminho.diretorio_atual; trazer caminho.e_arquivo; trazer caminho.existe;
        carinho principal() -> bombom {
            nova cwd: verso = diretorio_atual();
            falar(cwd, existe(cwd), e_arquivo(cwd));
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
        pacote main; trazer caminho.e_diretorio;
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
        pacote main; trazer caminho.e_diretorio;
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
        pacote main; trazer ambiente.argumento_ou; trazer caminho.diretorio_atual; trazer caminho.e_diretorio; trazer caminho.existe; trazer caminho.juntar;
        carinho principal() -> bombom {
            nova cwd: verso = diretorio_atual();
            nova alvo: verso = juntar(cwd, argumento_ou(0, "README.md"));
            falar(alvo, existe(alvo), e_diretorio(alvo));
            talvez existe(alvo) {
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
        pacote main; trazer caminho.tamanho_arquivo;
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
        pacote main; trazer caminho.arquivo_vazio;
        carinho principal() -> bombom {{
            talvez arquivo_vazio("{}") {{
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
        pacote main; trazer caminho.tamanho_arquivo;
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
        pacote main; trazer ambiente.argumento_ou; trazer caminho.arquivo_vazio; trazer caminho.diretorio_atual; trazer caminho.e_arquivo; trazer caminho.existe; trazer caminho.juntar; trazer caminho.tamanho_arquivo;
        carinho principal() -> bombom {
            nova base: verso = diretorio_atual();
            nova nome: verso = argumento_ou(0, "README.md");
            nova alvo: verso = juntar(base, nome);
            nova t: bombom = tamanho_arquivo(alvo);
            nova v: logica = arquivo_vazio(alvo);
            falar(alvo, existe(alvo), e_arquivo(alvo), t, v);
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
        pacote main; trazer caminho.criar_diretorio; trazer caminho.e_diretorio;
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
        pacote main; trazer caminho.existe; trazer caminho.remover_arquivo;
        carinho principal() -> bombom {{
            remover_arquivo("{}");
            talvez existe("{}") {{
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
        pacote main; trazer caminho.remover_arquivo;
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
        pacote main; trazer caminho.existe; trazer caminho.remover_diretorio;
        carinho principal() -> bombom {{
            remover_diretorio("{}");
            talvez existe("{}") {{
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
        pacote main; trazer caminho.remover_diretorio;
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
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova t: verso = ler_verso(h);
            fechar(h);
            falar(t);
            mimo tamanho(t);
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
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.comeca_com; trazer texto.contem;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso(h);
            fechar(h);
            nova tem: logica = contem(texto, "conteudo");
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
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.igual; trazer texto.termina_com;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso(h);
            fechar(h);
            nova sufixo_ok: logica = termina_com(texto, "ok");
            nova igual_ok: logica = igual(texto, "status: ok");
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
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.contem; trazer texto.igual; trazer texto.maiusculo; trazer texto.minusculo;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova texto: verso = ler_verso(h);
            fechar(h);
            nova baixo: verso = minusculo(texto);
            nova alto: verso = maiusculo(texto);
            nova ok_baixo: logica = contem(baixo, "pinker");
            nova ok_alto: logica = igual(alto, "PINKER V0");
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
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.aparar; trazer texto.indice_em; trazer texto.nao_vazio;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova bruto: verso = ler_verso(h);
            fechar(h);
            nova texto: verso = aparar(bruto);
            nova pos: bombom = indice_em(texto, "v0");
            nova ok: logica = nao_vazio(texto);
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
        pacote main; trazer ambiente.argumento_ou; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer caminho.criar_diretorio; trazer caminho.e_diretorio; trazer caminho.existe; trazer caminho.juntar; trazer caminho.remover_diretorio; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome_dir: verso = argumento_ou(0, "saida");
            nova nome_arquivo: verso = argumento_ou(1, "entrada.txt");
            nova alvo_dir: verso = juntar(base, nome_dir);
            nova alvo_arquivo: verso = juntar(base, nome_arquivo);
            criar_diretorio(alvo_dir);
            nova h: bombom = abrir(alvo_arquivo);
            nova t: verso = ler_verso(h);
            fechar(h);
            remover_diretorio(alvo_dir);
            falar(tamanho(t), existe(alvo_dir), e_diretorio(alvo_dir));
            talvez tamanho(t) > 0 {{
                talvez existe(alvo_dir) {{
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
        pacote main; trazer ambiente.argumento_ou; trazer caminho.criar_diretorio; trazer caminho.e_arquivo; trazer caminho.e_diretorio; trazer caminho.existe; trazer caminho.juntar; trazer caminho.remover_arquivo;
        carinho principal() -> bombom {{
            nova base: verso = "{}";
            nova nome_dir: verso = argumento_ou(0, "saida");
            nova alvo_dir: verso = juntar(base, nome_dir);
            criar_diretorio(alvo_dir);
            nova arquivo: verso = juntar(base, "temp.txt");
            remover_arquivo(arquivo);
            falar(existe(alvo_dir), e_diretorio(alvo_dir), existe(arquivo), e_arquivo(arquivo));
            talvez e_diretorio(alvo_dir) {{
                talvez existe(arquivo) {{
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

// @pinker-nav:end evidencia.interpreter.arquivos-introspeccao-caminho-e-diretorios
// @pinker-nav:start evidencia.interpreter.arquivos-e-ambiente-fallback-cli-exemplos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI de introspecção de caminho, diretório atual, refinamento de caminho e combinação argumento_ou/ambiente_ou, verificando a saída.
#[test]
fn run_ambiente_ou_intrinseca_usa_fallback_sem_env() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase95_ambiente_processo_minimo_valido.pink",
        &[],
        &["PINKER_TEST_ENV_PHASE95"],
        None,
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "visitante\n");
}

#[test]
fn run_ambiente_ou_intrinseca_ler_valor_real_do_ambiente() {
    let output = run_cli_example_with_env_and_cwd(
        "examples/fase95_ambiente_processo_minimo_valido.pink",
        &[("PINKER_TEST_ENV_PHASE95", "PinkerLab")],
        &[],
        None,
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "PinkerLab\n");
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
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", tmp.display())
    );
}

#[test]
fn cli_run_introspeccao_caminho_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase96_introspeccao_caminho_minima_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade
verdade
"
    );
}

#[test]
fn cli_run_refinamento_caminho_fase97_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase97_refinamento_caminho_minimo_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n"
    );
}

#[test]
fn cli_run_refinamento_arquivo_fase98_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase98_refinamento_arquivo_minimo_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n"
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
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "verdade\nverdade\nfalso\n"
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
    assert_cli_completed(&output);
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
    assert_cli_completed(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "texto fase101\n");
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
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "cli\n");
}

#[test]
fn cli_run_corpus_tooling_verso_minimo_funciona_com_exemplo_dedicado() {
    let out = run_cli_example_with_args(
        "examples/run_corpus_tooling_verso_minimo.pink",
        &["Pinker", "beta"],
    );
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "oi Pinker 9 o\nextra beta\n"
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
        r#"pacote t; trazer arquivo.abrir; trazer arquivo.fechar;
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

// @pinker-nav:end evidencia.interpreter.arquivos-e-ambiente-fallback-cli-exemplos
// @pinker-nav:start evidencia.interpreter.checagem-cli-modulos-e-recortes-linguagem
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Checa e executa via CLI de exemplos versionados recortes da linguagem — quebrar fora de laço, símbolo ausente, módulos exportados/apelidados/qualificados e suas rejeições, verso constante global — verificando validade e mensagens por contains.
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
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
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
fn cli_check_modulo_ninho_exportado_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase144_modulo_ninho_exportado_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_modulo_ninho_exportado_valido_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase144_modulo_ninho_exportado_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "16\n");
}

#[test]
fn cli_check_modulo_ninho_nao_importado_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase144_modulo_ninho_nao_importado_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("tipo 'PessoaCompartilhada' não existe"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_ninho_inexistente_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase144_modulo_ninho_inexistente_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains(
            "símbolo 'NaoExiste' não encontrado no módulo 'fase144_modulo_ninho_exportado_tipos'"
        ),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_apelido_exportado_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase145_modulo_apelido_exportado_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_modulo_apelido_exportado_valido_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase145_modulo_apelido_exportado_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
}

#[test]
fn cli_check_modulo_apelido_nao_importado_falha_com_exemplo_versionado() {
    let output =
        run_cli_check_example("examples/fase145_modulo_apelido_nao_importado_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("tipo 'Idade' não existe"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_apelido_inexistente_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase145_modulo_apelido_inexistente_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains(
            "símbolo 'IdadeNaoExiste' não encontrado no módulo 'fase145_modulo_apelido_exportado_tipos'"
        ),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_tipo_qualificado_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase146_modulo_tipo_qualificado_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_modulo_tipo_qualificado_valido_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase146_modulo_tipo_qualificado_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "42
16
"
    );
}

#[test]
fn cli_check_modulo_tipo_qualificado_inexistente_falha_com_exemplo_versionado() {
    let output =
        run_cli_check_example("examples/fase146_modulo_tipo_qualificado_inexistente_invalido.pink");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("tipo 'fase146_modulo_tipo_qualificado_tipos.NaoExiste' não existe"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_modulo_tipo_qualificado_modulo_nao_importado_falha_com_exemplo_versionado() {
    let output = run_cli_check_example(
        "examples/fase146_modulo_tipo_qualificado_modulo_nao_importado_invalido.pink",
    );
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Erro Semântico:"), "stderr: {}", stderr);
    assert!(
        stderr.contains("tipo 'fase146_modulo_tipo_qualificado_tipos.Idade' não existe"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_verso_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase61_verso_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_cfg_ir_verso_constante_global_com_exemplo_versionado() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--cfg-ir")
        .arg("examples/fase61_verso_cfg_ir_invalido.pink")
        .output()
        .expect("falha ao executar CLI --cfg-ir");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("const @MSG: verso"), "stdout: {}", stdout);
}

// @pinker-nav:end evidencia.interpreter.checagem-cli-modulos-e-recortes-linguagem
// @pinker-nav:start evidencia.interpreter.ponteiros-boot-freestanding-e-subset-nativo
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Checa/executa via CLI exemplos de ponteiros frágeis e voláteis, inline asm, freestanding/boot entry, kernel mínimo e cast de memória, verificando aceitação dentro do subset e rejeição fora dele por contains.
#[test]
fn cli_check_volatile_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_volatile_valido.pink");
    assert_cli_completed(&output);
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
    assert_cli_completed(&output);
    assert_eq!(output.status.code(), Some(89));
    assert!(output.stdout.is_empty());
}

#[test]
fn cli_check_fragil_operacional_fora_subset_falha_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/fase72_fragil_operacional_minimo_invalido.pink");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("retorno incompatível em 'ler': esperado 'bombom', encontrado 'u8'"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn cli_check_inline_asm_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_inline_asm_valido.pink");
    assert_cli_completed(&output);
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
    assert_cli_completed(&output);
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
    assert_cli_completed(&output);
}

#[test]
fn cli_check_kernel_minimo_fase59_valido_com_exemplo_versionado() {
    let output = run_cli_check_example("examples/check_kernel_minimo_fase59_valido.pink");
    assert_cli_completed(&output);
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
    assert_cli_completed(&output);
    assert_eq!(output.status.code(), Some(77));
    assert!(output.stdout.is_empty());
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
        stderr.contains("retorno incompatível em 'principal': esperado 'bombom', encontrado 'u8'"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_escrita_indireta_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase67_escrita_indireta_valida.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("123"), "stdout={}", stdout);
}

#[test]
fn cli_check_escrita_indireta_seta_u8_falha_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase67_escrita_indireta_seta_u8_invalida.pink");
    assert!(
        !output.status.success(),
        "esperava falha de runtime para escrita em ponteiro estrangeiro"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("deref_store em endereço inválido ou não inicializado"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_aritmetica_ponteiro_valida_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase68_ptr_aritmetica_valida.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("20"), "stdout={}", stdout);
}

#[test]
fn cli_run_aritmetica_ponteiro_leitura_valida_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase68_ptr_aritmetica_leitura_valida.pink");
    assert_cli_completed(&output);
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
        stderr.contains("apenas 'seta<T> + bombom'"),
        "mensagem inesperada: {}",
        stderr
    );
}

#[test]
fn cli_run_acesso_campo_ninho_operacional_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase69_ninho_campo_operacional_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(output.status.code(), Some(22));
    assert!(output.stdout.is_empty());
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
    assert_cli_completed(&output);
    assert_eq!(output.status.code(), Some(30));
    assert!(output.stdout.is_empty());
}

// @pinker-nav:end evidencia.interpreter.ponteiros-boot-freestanding-e-subset-nativo
// @pinker-nav:start evidencia.interpreter.ponteiros-array-fixo-e-cast-memoria-cli
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa via CLI o recorte de baixo nível de array fixo por valor e cast de memória, cobrindo casos operacionais mínimos e as respectivas rejeições fora do subset.
#[test]
fn cli_run_fase147_array_fixo_operacional_minimo_por_valor_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase147_array_fixo_operacional_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("20"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase147_array_fixo_operacional_minimo_invalido_falha_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase147_array_fixo_operacional_minimo_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha operacional para array fora do recorte bombom"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ponteiro para escalar público, array suportado ou ninho"),
        "stderr={}",
        stderr
    );
}

#[test]
fn cli_run_cast_memoria_operacional_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase71_cast_memoria_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(output.status.code(), Some(55));
    assert!(output.stdout.is_empty());
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

// @pinker-nav:end evidencia.interpreter.ponteiros-array-fixo-e-cast-memoria-cli
// @pinker-nav:start evidencia.interpreter.diagnostico-render-fonte-e-operador-bitnot
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre renderização de erro com contexto de fonte (parse, semântico, runtime sem span real) e o operador bitnot (til/nope, equivalência, inversão de bits conhecidos, dupla inversão, rejeição por tipo), misturando contains e igualdade.
#[test]
fn runtime_erro_sem_span_real_mostra_localizacao_indisponivel() {
    // Erro de runtime deve exibir "localização: indisponível" em vez de "span: 1:1..1:1"
    // porque a instrução de máquina não carrega span real.
    let program = MachineProgram {
        union_types: vec![],
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
                    MachineInstr::Div { ty: TypeIR::Bombom },
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

// @pinker-nav:end evidencia.interpreter.diagnostico-render-fonte-e-operador-bitnot
// @pinker-nav:start evidencia.interpreter.arquivos-handle-fechado-e-fluxo-completo
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita falhas após fechar o handle (ler, ler_verso, tamanho, é_vazio em diretório/ausente) e o fluxo completo criar→escrever→ler→fechar no interpretador.
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_bombom;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            nova v: bombom = ler_bombom(h);
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            nova v: verso = ler_verso(h);
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.escrever_bombom; trazer arquivo.fechar;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            fechar(h);
            escrever_bombom(h, 99);
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.escrever_verso; trazer arquivo.fechar;
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.fechar;
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            nova v: verso = ler_verso(h);
            fechar(h);
            mimo tamanho(v);
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
        r#"pacote main; trazer arquivo.abrir; trazer arquivo.escrever_bombom; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = abrir("{}");
            escrever_bombom(h, 42);
            nova v: verso = ler_verso(h);
            fechar(h);
            mimo tamanho(v);
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
    let source = r#"pacote main; trazer caminho.tamanho_arquivo;
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
    let source = r#"pacote main; trazer caminho.arquivo_vazio;
        carinho principal() -> bombom {
            nova v: logica = arquivo_vazio("/tmp");
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
    let source = r#"pacote main; trazer caminho.arquivo_vazio;
        carinho principal() -> bombom {
            nova v: logica = arquivo_vazio("/caminho/que/nao/existe/hf3_xyzzy.txt");
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
        r#"pacote main; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {{
            nova h: bombom = criar("{}");
            escrever_verso(h, "pinker hf3");
            nova lido: verso = ler_verso(h);
            fechar(h);
            mimo tamanho(lido);
        }}"#,
        file_path.to_string_lossy().replace('\\', "\\\\")
    );
    let out = run_code(&source).unwrap();
    let _ = std::fs::remove_file(&file_path);
    // "pinker hf3" has 10 characters
    assert_eq!(out, Some(RuntimeValue::Int(10)));
}

// ─── Fase 137 — split camada 1 conservadora ───────────────────────────────────

// @pinker-nav:end evidencia.interpreter.arquivos-handle-fechado-e-fluxo-completo
// @pinker-nav:start evidencia.interpreter.texto-dividir-substituir-juntar-e-buscar
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre dividir_verso, substituir_verso e juntar_verso — contagem, pedaços vazios, encadeamento, combinações e rejeições — além de busca textual via exemplo CLI.
#[test]
fn run_fase137_dividir_verso_contar_dois_pedacos() {
    let source = r#"pacote main; trazer texto.dividir_contar;
        carinho principal() -> bombom {
            nova n: bombom = dividir_contar("a:b", ":");
            mimo n;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(2)));
}

#[test]
fn run_fase137_dividir_verso_contar_tres_pedacos() {
    let source = r#"pacote main; trazer texto.dividir_contar;
        carinho principal() -> bombom {
            nova n: bombom = dividir_contar("nome:idade:cidade", ":");
            mimo n;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(3)));
}

#[test]
fn run_fase137_dividir_verso_contar_sem_separador_retorna_um() {
    let source = r#"pacote main; trazer texto.dividir_contar;
        carinho principal() -> bombom {
            nova n: bombom = dividir_contar("pinker", ":");
            mimo n;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_fase137_dividir_verso_em_primeiro_pedaco() {
    let source = r#"pacote main; trazer texto.dividir_em;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("nome:idade:cidade", ":", 0);
            falar(p);
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_fase137_dividir_verso_em_segundo_pedaco() {
    let source = r#"pacote main; trazer texto.dividir_em; trazer texto.igual;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("nome:idade:cidade", ":", 1);
            nova ok: logica = igual(p, "idade");
            talvez ok {
                mimo 137;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(137)));
}

#[test]
fn run_fase137_dividir_verso_em_terceiro_pedaco() {
    let source = r#"pacote main; trazer texto.dividir_em; trazer texto.igual;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("nome:idade:cidade", ":", 2);
            nova ok: logica = igual(p, "cidade");
            talvez ok {
                mimo 137;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(137)));
}

#[test]
fn run_fase137_dividir_verso_em_separador_espaco() {
    let source = r#"pacote main; trazer texto.dividir_em; trazer texto.igual;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("hello world pinker", " ", 2);
            nova ok: logica = igual(p, "pinker");
            talvez ok {
                mimo 137;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(137)));
}

#[test]
fn run_fase137_dividir_verso_em_indice_fora_de_faixa_falha() {
    let source = r#"pacote main; trazer texto.dividir_em;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("a:b", ":", 5);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("índice fora da faixa"),
        "esperava erro de índice, obteve: {}",
        err
    );
}

#[test]
fn run_fase137_dividir_verso_contar_separador_vazio_falha() {
    let source = r#"pacote main; trazer texto.dividir_contar;
        carinho principal() -> bombom {
            nova n: bombom = dividir_contar("pinker", "");
            mimo n;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("separador vazio"),
        "esperava erro de separador vazio, obteve: {}",
        err
    );
}

#[test]
fn run_fase137_dividir_verso_em_separador_vazio_falha() {
    let source = r#"pacote main; trazer texto.dividir_em;
        carinho principal() -> bombom {
            nova p: verso = dividir_em("pinker", "", 0);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("separador vazio"),
        "esperava erro de separador vazio, obteve: {}",
        err
    );
}

#[test]
fn run_fase137_dividir_verso_em_combina_com_contar() {
    let source = r#"pacote main; trazer texto.dividir_contar; trazer texto.dividir_em; trazer texto.igual;
        carinho principal() -> bombom {
            nova texto: verso = "a,b,c,d";
            nova sep: verso = ",";
            nova n: bombom = dividir_contar(texto, sep);
            nova ultimo: verso = dividir_em(texto, sep, 3);
            nova ok: logica = igual(ultimo, "d");
            talvez ok {
                mimo n;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(4)));
}

// ─── Fase 138 — replace camada 1 conservadora ──────────────────────────────────

#[test]
fn run_fase138_substituir_verso_basico() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova t: verso = substituir("hello world", "world", "pinker");
            nova ok: logica = igual(t, "hello pinker");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

#[test]
fn run_fase138_substituir_verso_multiplas_ocorrencias() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova t: verso = substituir("a,b,a,c,a", ",", ";");
            nova ok: logica = igual(t, "a;b;a;c;a");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

#[test]
fn run_fase138_substituir_verso_sem_ocorrencia_retorna_original() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova t: verso = substituir("pinker", "x", "y");
            nova ok: logica = igual(t, "pinker");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

#[test]
fn run_fase138_substituir_verso_por_vazio_remove() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova t: verso = substituir("hello world", "world", "");
            nova ok: logica = igual(t, "hello ");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

#[test]
fn run_fase138_substituir_verso_padrao_vazio_falha() {
    let source = r#"pacote main; trazer texto.substituir;
        carinho principal() -> bombom {
            nova t: verso = substituir("pinker", "", "x");
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("padrão vazio"),
        "esperava erro de padrão vazio, obteve: {}",
        err
    );
}

#[test]
fn run_fase138_substituir_verso_normaliza_separadores() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova linha: verso = "nome:idade:cidade";
            nova normalizado: verso = substituir(linha, ":", "-");
            nova ok: logica = igual(normalizado, "nome-idade-cidade");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

#[test]
fn run_fase138_substituir_verso_combina_com_split() {
    let source = r#"pacote main; trazer texto.dividir_em; trazer texto.igual; trazer texto.substituir;
        carinho principal() -> bombom {
            nova base: verso = "a.b.c";
            nova trocado: verso = substituir(base, ".", ",");
            nova campo1: verso = dividir_em(trocado, ",", 1);
            nova ok: logica = igual(campo1, "b");
            talvez ok {
                mimo 138;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(138)));
}

// =====================================================================
// Fase 139 — juntar_verso_com(a, sep, b) -> verso
// =====================================================================

#[test]
fn run_fase139_juntar_verso_com_basico() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("nome", "-", "idade");
            nova ok: logica = igual(r, "nome-idade");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_separador_vazio() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("abc", "", "def");
            nova ok: logica = igual(r, "abcdef");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_separador_longo() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("X", " :: ", "Y");
            nova ok: logica = igual(r, "X :: Y");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_pedaco_vazio_esquerda() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("", ",", "fim");
            nova ok: logica = igual(r, ",fim");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_pedaco_vazio_direita() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("inicio", ",", "");
            nova ok: logica = igual(r, "inicio,");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_encadeado() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova ab: verso = juntar_com("a", ":", "b");
            nova abc: verso = juntar_com(ab, ":", "c");
            nova ok: logica = igual(abc, "a:b:c");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_combina_com_split() {
    let source = r#"pacote main; trazer texto.dividir_em; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova campo0: verso = dividir_em("nome:idade:cidade", ":", 0);
            nova campo2: verso = dividir_em("nome:idade:cidade", ":", 2);
            nova r: verso = juntar_com(campo0, "-", campo2);
            nova ok: logica = igual(r, "nome-cidade");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn run_fase139_juntar_verso_com_tudo_vazio() {
    let source = r#"pacote main; trazer texto.igual; trazer texto.juntar_com;
        carinho principal() -> bombom {
            nova r: verso = juntar_com("", "", "");
            nova ok: logica = igual(r, "");
            talvez ok {
                mimo 139;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(139)));
}

#[test]
fn cli_run_fase140_busca_textual_minima_funciona_com_exemplo_versionado() {
    let out = run_cli_example("examples/fase140_busca_textual_camada1_valido.pink");
    assert_cli_completed(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "0\n18446744073709551615\n"
    );
}

// ── Fase 157: formatação simples de saída com placeholders mínimos ───────────

// @pinker-nav:end evidencia.interpreter.texto-dividir-substituir-juntar-e-buscar
// @pinker-nav:start evidencia.interpreter.texto-formatar-verso
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita formatar_verso (com bombom, verso e bombom, fluxo composto) e suas rejeições (placeholders a menos, modelo inválido).
#[test]
fn run_fase157_formatar_verso_com_bombom() {
    let source = r#"pacote main; trazer texto.formatar; trazer texto.igual;
        carinho principal() -> bombom {
            nova linha: verso = formatar("saldo={}", 42);
            nova ok: logica = igual(linha, "saldo=42");
            talvez ok {
                mimo 157;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(157)));
}

#[test]
fn run_fase157_formatar_verso_com_verso_e_bombom() {
    let source = r#"pacote main; trazer texto.formatar; trazer texto.igual;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{}={}", "idade", 7);
            nova ok: logica = igual(linha, "idade=7");
            talvez ok {
                mimo 157;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(157)));
}

#[test]
fn run_fase157_formatar_verso_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_obter; trazer lista.bombom_tamanho; trazer texto.formatar; trazer texto.igual;
        carinho montar_item(chave: verso, valor: bombom) -> verso {
            mimo formatar("{}={}", chave, valor);
        }
        carinho principal() -> bombom {
            nova itens: lista<bombom> = bombom_criar();
            bombom_anexar(itens, 7);
            bombom_anexar(itens, 9);
            nova cabecalho: verso = formatar("relatorio {}", "rodada");
            nova linha1: verso = montar_item("total", bombom_tamanho(itens));
            nova linha2: verso = montar_item("primeiro", bombom_obter(itens, 0));
            talvez igual(cabecalho, "relatorio rodada") && igual(linha1, "total=2") && igual(linha2, "primeiro=7") {
                mimo 157;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(157)));
}

#[test]
fn run_fase157_formatar_verso_falha_com_placeholders_a_menos() {
    let source = r#"pacote main; trazer texto.formatar;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{} {}", 1);
            falar(linha);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("quantidade de placeholders"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase157_formatar_verso_falha_com_modelo_invalido() {
    let source = r#"pacote main; trazer texto.formatar;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{nome}", "ana");
            falar(linha);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("modelo inválido em 'formatar_verso'"),
        "{}",
        err
    );
}

// ── Fase 158: CSV mínimo (camada 1 conservadora) ────────────────────────────

// @pinker-nav:end evidencia.interpreter.texto-formatar-verso
// @pinker-nav:start evidencia.interpreter.arquivos-csv-serializacao
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre ler/emitir linha CSV mínima e fluxo composto, com rejeições de quoting, multiline e separador longo; recorte mínimo, não CSV completo.
#[test]
fn run_fase158_ler_linha_csv_bombom_minima_funciona() {
    let source = r#"pacote main; trazer csv.ler_linha_bombom; trazer lista.bombom_obter; trazer lista.bombom_tamanho;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_bombom("7,11,13", ",");
            talvez bombom_tamanho(itens) == 3 && bombom_obter(itens, 1) == 11 {
                mimo 158;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(158)));
}

#[test]
fn run_fase158_emitir_linha_csv_bombom_minima_funciona() {
    let source = r#"pacote main; trazer csv.emitir_linha_bombom; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer texto.igual;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = bombom_criar();
            bombom_anexar(itens, 3);
            bombom_anexar(itens, 5);
            bombom_anexar(itens, 8);
            nova linha: verso = emitir_linha_bombom(itens, ";");
            talvez igual(linha, "3;5;8") {
                mimo 158;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(158)));
}

#[test]
fn run_fase158_csv_minimo_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer csv.emitir_linha_bombom; trazer csv.ler_linha_bombom; trazer lista.bombom_anexar; trazer texto.igual;
        carinho somar_linha_csv(linha: verso, sep: verso) -> bombom {
            nova itens: lista<bombom> = ler_linha_bombom(linha, sep);
            nova muda total: bombom = 0;
            para cada item em itens {
                total = total + item;
            }
            mimo total;
        }

        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_bombom("10;20;30", ";");
            nova total: bombom = somar_linha_csv("10;20;30", ";");
            bombom_anexar(itens, total);
            nova resumo: verso = emitir_linha_bombom(itens, ";");
            talvez igual(resumo, "10;20;30;60") {
                mimo 158;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(158)));
}

#[test]
fn run_fase158_ler_linha_csv_bombom_rejeita_quoting() {
    let dir = std::env::temp_dir();
    let path = dir.join("pinker_fase158_csv_quoting.txt");
    fs::write(&path, "\"7\",11").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer csv.ler_linha_bombom; trazer lista.bombom_tamanho;
        carinho principal() -> bombom {{
            nova linha: verso = ler_caminho_verso("{}");
            nova itens: lista<bombom> = ler_linha_bombom(linha, ",");
            mimo bombom_tamanho(itens);
        }}"#,
        path.display()
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("quoting fora do recorte"),
        "erro inesperado: {}",
        err
    );
    let _ = fs::remove_file(path);
}

#[test]
fn run_fase158_ler_linha_csv_bombom_rejeita_multiline() {
    let dir = std::env::temp_dir();
    let path = dir.join("pinker_fase158_csv_multiline.txt");
    fs::write(&path, "7,11\n13").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer csv.ler_linha_bombom; trazer lista.bombom_tamanho;
        carinho principal() -> bombom {{
            nova linha: verso = ler_caminho_verso("{}");
            nova itens: lista<bombom> = ler_linha_bombom(linha, ",");
            mimo bombom_tamanho(itens);
        }}"#,
        path.display()
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("multiline fora do recorte"),
        "erro inesperado: {}",
        err
    );
    let _ = fs::remove_file(path);
}

#[test]
fn run_fase158_emitir_linha_csv_bombom_rejeita_separador_longo() {
    let source = r#"pacote main; trazer csv.emitir_linha_bombom; trazer lista.bombom_anexar; trazer lista.bombom_criar;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = bombom_criar();
            bombom_anexar(itens, 1);
            nova linha: verso = emitir_linha_bombom(itens, "::");
            falar(linha);
            mimo 0;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("exige separador de 1 caractere"),
        "erro inesperado: {}",
        err
    );
}

// @pinker-nav:end evidencia.interpreter.arquivos-csv-serializacao
// @pinker-nav:start evidencia.interpreter.arquivos-json-serializacao
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre ler/emitir JSON plano mínimo e fluxo composto, com rejeições de array, escape rico e nesting; recorte plano, não JSON completo.
#[test]
fn run_fase159_ler_json_plano_bombom_minimo_funciona() {
    let path = std::env::temp_dir().join("pinker_fase159_json_minimo.json");
    fs::write(&path, "{\"idade\":7,\"pontos\":9}").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_obter; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {{
            nova json: verso = ler_caminho_verso("{}");
            nova dados: mapa<verso,bombom> = ler_plano_bombom(json);
            talvez verso_bombom_tamanho(dados) == 2 && verso_bombom_obter(dados, "idade") == 7 {{
                mimo 159;
            }}
            mimo 0;
        }}"#,
        path.display()
    );
    let out = run_code(&source).unwrap();
    let _ = fs::remove_file(path);
    assert_eq!(out, Some(RuntimeValue::Int(159)));
}

#[test]
fn run_fase159_emitir_json_plano_bombom_minimo_funciona() {
    let source = r#"pacote main; trazer json.emitir_plano_bombom; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {
            nova dados: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(dados, "b", 2);
            verso_bombom_definir(dados, "a", 1);
            nova json: verso = emitir_plano_bombom(dados);
            nova lido: mapa<verso,bombom> = ler_plano_bombom(json);
            talvez verso_bombom_tamanho(lido) == 2
                && verso_bombom_obter(lido, "a") == 1
                && verso_bombom_obter(lido, "b") == 2 {
                mimo 159;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(159)));
}

#[test]
fn run_fase159_json_plano_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer json.emitir_plano_bombom; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter;
        carinho carregar_relatorio() -> verso {
            nova dados: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(dados, "falhas", 2);
            verso_bombom_definir(dados, "ok", 5);
            mimo emitir_plano_bombom(dados);
        }

        carinho principal() -> bombom {
            nova dados: mapa<verso,bombom> = ler_plano_bombom(carregar_relatorio());
            nova muda total: bombom = 0;
            para cada chave em dados {
                total = total + verso_bombom_obter(dados, chave);
            }
            verso_bombom_definir(dados, "total", total);
            nova json: verso = emitir_plano_bombom(dados);
            falar(json);
            nova validado: mapa<verso,bombom> = ler_plano_bombom(json);
            talvez verso_bombom_obter(validado, "falhas") == 2
                && verso_bombom_obter(validado, "ok") == 5
                && verso_bombom_obter(validado, "total") == 7 {
                mimo 159;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(159)));
}

#[test]
fn run_fase159_ler_json_plano_bombom_rejeita_array() {
    let path = std::env::temp_dir().join("pinker_fase159_json_array.json");
    fs::write(&path, "[1,2,3]").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {{
            nova json: verso = ler_caminho_verso("{}");
            nova dados: mapa<verso,bombom> = ler_plano_bombom(json);
            mimo verso_bombom_tamanho(dados);
        }}"#,
        path.display()
    );
    let err = run_code(&source).unwrap_err();
    let _ = fs::remove_file(path);
    assert!(err.contains("esperado '{'"), "erro inesperado: {}", err);
}

#[test]
fn run_fase159_ler_json_plano_bombom_rejeita_escape_rico() {
    let path = std::env::temp_dir().join("pinker_fase159_json_escape.json");
    fs::write(&path, "{\"li\\nha\":1}").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {{
            nova json: verso = ler_caminho_verso("{}");
            nova dados: mapa<verso,bombom> = ler_plano_bombom(json);
            mimo verso_bombom_tamanho(dados);
        }}"#,
        path.display()
    );
    let err = run_code(&source).unwrap_err();
    let _ = fs::remove_file(path);
    assert!(
        err.contains("escapes em chave fora do recorte"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase159_ler_json_plano_bombom_rejeita_nesting() {
    let path = std::env::temp_dir().join("pinker_fase159_json_nesting.json");
    fs::write(&path, "{\"meta\":{\"x\":1}}").unwrap();
    let source = format!(
        r#"pacote main; trazer arquivo.ler_caminho_verso; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {{
            nova json: verso = ler_caminho_verso("{}");
            nova dados: mapa<verso,bombom> = ler_plano_bombom(json);
            mimo verso_bombom_tamanho(dados);
        }}"#,
        path.display()
    );
    let err = run_code(&source).unwrap_err();
    let _ = fs::remove_file(path);
    assert!(
        err.contains("valor deve ser bombom sem sinal"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase159_json_basico_valido() {
    let output = run_cli_check_example("examples/fase159_json_basico_valido.pink");
    assert_cli_completed(&output);
}

// @pinker-nav:end evidencia.interpreter.arquivos-json-serializacao
// @pinker-nav:start evidencia.interpreter.tempo-unix-e-formatacao
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita tempo unix e formatação de tempo (época, timestamp mínimo positivo, fluxo composto) no interpretador e via CLI; não fixa valor absoluto de relógio.
#[test]
fn run_fase160_formatar_tempo_unix_epoca_funciona() {
    let source = r#"pacote main; trazer tempo.formatar_unix; trazer texto.igual;
        carinho principal() -> bombom {
            nova texto: verso = formatar_unix(0);
            talvez igual(texto, "1970-01-01T00:00:00Z") {
                mimo 160;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(160)));
}

#[test]
fn run_fase160_tempo_unix_retorna_timestamp_minimo_positivo() {
    let source = r#"pacote main; trazer tempo.formatar_unix; trazer tempo.unix; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova ts: bombom = unix();
            nova iso: verso = formatar_unix(ts);
            talvez ts > 0 && tamanho(iso) == 20 {
                mimo 160;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(160)));
}

#[test]
fn run_fase160_tempo_basico_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer tempo.formatar_unix; trazer tempo.unix; trazer texto.contem; trazer texto.formatar; trazer texto.tamanho;
        carinho carimbar_evento(nome: verso, instante: bombom) -> verso {
            nova prefixo: verso = formatar("evento={};ts={}", nome, instante);
            nova iso: verso = formatar_unix(instante);
            mimo formatar("{};iso={}", prefixo, iso);
        }

        carinho principal() -> bombom {
            nova ts_inicio: bombom = unix();
            nova iso_inicio: verso = formatar_unix(ts_inicio);
            nova evento: verso = carimbar_evento("coleta", ts_inicio);

            nova relatorio: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(relatorio, "inicio_unix", ts_inicio);
            verso_bombom_definir(relatorio, "camada", 1);

            talvez tamanho(iso_inicio) == 20
                && contem(evento, "evento=coleta")
                && contem(evento, ";iso=")
                && verso_bombom_obter(relatorio, "inicio_unix") == ts_inicio {
                mimo 160;
            }
            mimo 0;
        }"#;
    let out = run_code(source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(160)));
}

#[test]
fn cli_check_fase160_tempo_basico_timestamp_valido() {
    let output = run_cli_check_example("examples/fase160_tempo_basico_timestamp_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase160_tempo_basico_timestamp_valido() {
    let output = run_cli_example("examples/fase160_tempo_basico_timestamp_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let linhas = stdout.lines().collect::<Vec<_>>();
    assert_eq!(linhas.len(), 2, "stdout inesperado: {}", stdout);
    assert_eq!(linhas[0], "1970-01-01T00:00:00Z");
    let ts = linhas[1]
        .parse::<u64>()
        .expect("timestamp inválido em stdout");
    assert!(ts > 0, "timestamp inesperado: {}", ts);
    assert_eq!(output.status.code(), Some(160));
}

#[test]
fn cli_check_fase160_tempo_basico_fluxo_composto_valido() {
    let output = run_cli_check_example("examples/fase160_tempo_basico_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase160_tempo_basico_fluxo_composto_valido() {
    let output = run_cli_example("examples/fase160_tempo_basico_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let linhas = stdout.lines().collect::<Vec<_>>();
    assert_eq!(linhas.len(), 3, "stdout inesperado: {}", stdout);
    assert!(
        linhas[0].starts_with("inicio=20"),
        "stdout inesperado: {}",
        stdout
    );
    assert!(
        linhas[1].contains("evento=coleta;ts="),
        "stdout inesperado: {}",
        stdout
    );
    assert!(
        linhas[1].contains(";iso=20"),
        "stdout inesperado: {}",
        stdout
    );
    assert!(
        linhas[2].contains("\"camada\":1"),
        "stdout inesperado: {}",
        stdout
    );
    assert!(
        linhas[2].contains("\"inicio_unix\":"),
        "stdout inesperado: {}",
        stdout
    );
    assert_eq!(output.status.code(), Some(160));
}

// @pinker-nav:end evidencia.interpreter.tempo-unix-e-formatacao
// @pinker-nav:start evidencia.interpreter.processos-externo-executar
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre executar processo externo mínimo (código zero e não-zero, rejeição de comando vazio) no interpretador e via exemplos CLI.
#[test]
fn run_fase161_executar_processo_minimo_retorna_codigo_zero() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("__CMD__");
            talvez codigo == 0 {
                mimo 161;
            }
            mimo 0;
        }"#
    .replace("__CMD__", &pink_string_literal(fase162_helper_bin("exit0")));
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(161)));
}

#[test]
fn run_fase161_executar_processo_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer processo.executar; trazer texto.formatar;
        carinho verificar(nome: verso, comando: verso) -> bombom {
            nova codigo: bombom = executar(comando);
            falar(formatar("{}={}", nome, codigo));
            mimo codigo;
        }

        carinho principal() -> bombom {
            nova codigo_ok: bombom = verificar("ok", "__CMD_OK__");
            nova codigo_falha: bombom = verificar("falha", "__CMD_FAIL__");
            nova resumo: verso = formatar("ok_zero={};falha_zero={}", codigo_ok, codigo_falha);
            falar(resumo);
            talvez codigo_ok == 0 && codigo_falha == 1 {
                mimo 161;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD_OK__",
        &pink_string_literal(fase162_helper_bin("exit0")),
    )
    .replace(
        "__CMD_FAIL__",
        &pink_string_literal(fase162_helper_bin("exit1")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(161)));
}

#[test]
fn run_fase161_executar_processo_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("__CMD__ --flag");
            mimo codigo;
        }"#
    .replace("__CMD__", &pink_string_literal(fase162_helper_bin("exit0")));
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'executar_processo'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase161_executar_processo_falha_com_spawn_invalido() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("/__pinker_fase161_comando_inexistente__");
            mimo codigo;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'executar_processo'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase161_executar_processo_rejeita_comando_vazio() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("");
            mimo codigo;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("intrínseca 'executar_processo' exige comando não vazio"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase161_processo_externo_minimo_valido() {
    let output = run_cli_check_example("examples/fase161_processo_externo_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase161_processo_externo_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase161_processo_externo_minimo_valido.pink",
        &[fase162_helper_bin("exit0")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_check_fase161_processo_externo_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase161_processo_externo_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase161_processo_externo_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase161_processo_externo_fluxo_composto_valido.pink",
        &[fase162_helper_bin("exit0"), fase162_helper_bin("exit1")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ok=0\nfalha=1\nok_zero=0;falha_zero=1\n"
    );
}

// @pinker-nav:end evidencia.interpreter.processos-externo-executar
// @pinker-nav:start evidencia.interpreter.processos-argv-explicito
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita executar processo com argv explícito mínimo e rejeição de argv fora do recorte, no interpretador e via CLI.
#[test]
fn run_fase168_executar_processo_aceita_argv_explicito_minimo() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("__CMD__", "--modo=ok");
            talvez codigo == 0 {
                mimo 168;
            }
            mimo 0;
        }"#
    .replace("__CMD__", &pink_string_literal(fase168_helper_bin()));
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(168)));
}

#[test]
fn run_fase168_executar_processo_fluxo_composto_com_argv_explicito() {
    let source = r#"pacote main; trazer processo.executar; trazer texto.formatar;
        carinho verificar(nome: verso, comando: verso, arg: verso) -> bombom {
            nova codigo: bombom = executar(comando, arg);
            falar(formatar("{}={}", nome, codigo));
            mimo codigo;
        }

        carinho principal() -> bombom {
            nova codigo_ok: bombom = verificar("ok", "__CMD__", "--modo=ok");
            nova codigo_falha: bombom = verificar("falha", "__CMD__", "--modo=falha");
            talvez codigo_ok == 0 && codigo_falha == 1 {
                mimo 168;
            }
            mimo 0;
        }"#
    .replace("__CMD__", &pink_string_literal(fase168_helper_bin()));
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(168)));
}

#[test]
fn run_fase168_executar_processo_rejeita_argv_fora_do_recorte_minimo() {
    let source = r#"pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("__CMD__", "--modo=ok", "--extra");
            mimo codigo;
        }"#
    .replace("__CMD__", &pink_string_literal(fase168_helper_bin()));
    let err = semantic::check_program(&common::parse(&source).unwrap())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("chamada de 'executar_processo' com aridade inválida"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase168_argv_explicito_minimo_valido() {
    let output = run_cli_check_example("examples/fase168_argv_explicito_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase168_argv_explicito_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase168_argv_explicito_minimo_valido.pink",
        &[fase168_helper_bin()],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_check_fase168_argv_explicito_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase168_argv_explicito_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase168_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase168_argv_explicito_fluxo_composto_valido.pink",
        &[fase168_helper_bin()],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ok=0\nfalha=1\nok_zero=0;falha_zero=1\n"
    );
}

// @pinker-nav:end evidencia.interpreter.processos-argv-explicito
// @pinker-nav:start evidencia.interpreter.processos-captura-stdout
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre captura de stdout de processo (retorna verso, argv explícito, rejeição de stdout não-UTF8) no interpretador e via CLI.
#[test]
fn run_fase163_capturar_stdout_minimo_retorna_verso() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.contem; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__");
            nova tem_status: logica = contem(texto, "status=ok");
            nova tem_valor: logica = contem(texto, "valor=7");
            talvez tem_status && tem_valor && tamanho(texto) == 18 {
                mimo 163;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(163)));
}

#[test]
fn run_fase169_capturar_stdout_aceita_argv_explicito_minimo() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.contem; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__", "--alvo=rosa");
            nova tem_status: logica = contem(texto, "status=ok");
            nova tem_alvo: logica = contem(texto, "alvo=rosa");
            talvez tem_status && tem_alvo && tamanho(texto) == 20 {
                mimo 169;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(169)));
}

#[test]
fn run_fase169_capturar_stdout_fluxo_composto_com_argv_explicito() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.contem; trazer texto.formatar; trazer texto.igual; trazer texto.tamanho;
        carinho resumir(texto: verso) -> verso {
            nova tem_status: logica = contem(texto, "status=ok");
            nova tem_alvo: logica = contem(texto, "alvo=rosa");
            talvez tem_status && tem_alvo {
                mimo formatar("captura={} bytes", tamanho(texto));
            }
            mimo "captura=invalida";
        }

        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__", "--alvo=rosa");
            nova resumo: verso = resumir(texto);
            falar(resumo);
            talvez igual(resumo, "captura=20 bytes") {
                mimo 169;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(169)));
}

#[test]
fn run_fase169_capturar_stdout_rejeita_argv_fora_do_recorte_minimo() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__", "--alvo=rosa", "--extra");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let err = semantic::check_program(&common::parse(&source).unwrap())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("chamada de 'capturar_stdout' com aridade inválida"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase163_capturar_stdout_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.contem; trazer texto.formatar; trazer texto.tamanho;
        carinho resumir(texto: verso) -> verso {
            nova tem_status: logica = contem(texto, "status=ok");
            nova tem_valor: logica = contem(texto, "valor=7");
            talvez tem_status && tem_valor {
                mimo formatar("captura={} bytes", tamanho(texto));
            }
            mimo "captura=invalida";
        }

        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__");
            nova resumo: verso = resumir(texto);
            falar(resumo);
            talvez contem(resumo, "captura=") {
                mimo 163;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(163)));
}

#[test]
fn run_fase163_capturar_stdout_falha_com_spawn_invalido() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("/__pinker_fase163_comando_inexistente__");
            mimo tamanho(texto);
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stdout'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase163_capturar_stdout_rejeita_comando_vazio() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("");
            mimo tamanho(texto);
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("intrínseca 'capturar_stdout' exige comando não vazio"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase163_capturar_stdout_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__ --flag");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stdout'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase169_capturar_stdout_com_argv_explicito_ainda_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__ --flag", "--alvo=rosa");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stdout'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase163_capturar_stdout_rejeita_stdout_nao_utf8() {
    let source = r#"pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("__CMD__");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase163_helper_bin("stdout_invalido_utf8")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("stdout inválido em 'capturar_stdout'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase163_captura_stdout_minima_valido() {
    let output = run_cli_check_example("examples/fase163_captura_stdout_minima_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase163_captura_stdout_minima_valido() {
    let output = run_cli_example_with_args(
        "examples/fase163_captura_stdout_minima_valido.pink",
        &[fase163_helper_bin("stdout_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "status=ok\nvalor=7\n\n"
    );
}

#[test]
fn cli_check_fase163_captura_stdout_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase163_captura_stdout_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase163_captura_stdout_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase163_captura_stdout_fluxo_composto_valido.pink",
        &[fase163_helper_bin("stdout_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "captura=18 bytes\n"
    );
}

#[test]
fn cli_check_fase169_captura_stdout_argv_explicito_minimo_valido() {
    let output =
        run_cli_check_example("examples/fase169_captura_stdout_argv_explicito_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase169_captura_stdout_argv_explicito_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase169_captura_stdout_argv_explicito_minimo_valido.pink",
        &[fase163_helper_bin("stdout_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "status=ok\nalvo=rosa\n\n"
    );
}

#[test]
fn cli_check_fase169_captura_stdout_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_check_example(
        "examples/fase169_captura_stdout_argv_explicito_fluxo_composto_valido.pink",
    );
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase169_captura_stdout_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase169_captura_stdout_argv_explicito_fluxo_composto_valido.pink",
        &[fase163_helper_bin("stdout_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "captura=20 bytes\n"
    );
}

// @pinker-nav:end evidencia.interpreter.processos-captura-stdout
// @pinker-nav:start evidencia.interpreter.processos-captura-stderr
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre captura de stderr (mínimo, argv explícito, preservação UTF-8 estrita) no interpretador e via CLI.
#[test]
fn run_fase164_capturar_stderr_minimo_retorna_verso() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.contem; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__");
            nova tem_erro: logica = contem(texto, "erro=sim");
            nova tem_codigo: logica = contem(texto, "codigo=9");
            talvez tem_erro && tem_codigo && tamanho(texto) == 18 {
                mimo 164;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(164)));
}

#[test]
fn run_fase170_capturar_stderr_aceita_argv_explicito_minimo() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.contem; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__", "--alvo=rosa");
            nova tem_erro: logica = contem(texto, "erro=sim");
            nova tem_alvo: logica = contem(texto, "alvo=rosa");
            talvez tem_erro && tem_alvo && tamanho(texto) == 19 {
                mimo 170;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(170)));
}

#[test]
fn run_fase170_capturar_stderr_fluxo_composto_com_argv_explicito() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.contem; trazer texto.formatar; trazer texto.igual; trazer texto.tamanho;
        carinho resumir(stderr_texto: verso) -> verso {
            nova tem_erro: logica = contem(stderr_texto, "erro=sim");
            nova tem_alvo: logica = contem(stderr_texto, "alvo=rosa");
            talvez tem_erro && tem_alvo {
                mimo formatar("stderr={} bytes", tamanho(stderr_texto));
            }
            mimo "stderr=invalido";
        }

        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__", "--alvo=rosa");
            nova resumo: verso = resumir(texto);
            falar(resumo);
            talvez igual(resumo, "stderr=19 bytes") {
                mimo 170;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(170)));
}

#[test]
fn run_fase164_capturar_stderr_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.contem; trazer texto.formatar; trazer texto.tamanho;
        carinho resumir(stderr_texto: verso) -> verso {
            nova tem_erro: logica = contem(stderr_texto, "erro=sim");
            nova tem_codigo: logica = contem(stderr_texto, "codigo=9");
            talvez tem_erro && tem_codigo {
                mimo formatar("stderr={} bytes", tamanho(stderr_texto));
            }
            mimo "stderr=invalido";
        }

        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__");
            nova resumo: verso = resumir(texto);
            falar(resumo);
            talvez contem(resumo, "stderr=") {
                mimo 164;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(164)));
}

#[test]
fn run_fase170_capturar_stderr_rejeita_argv_fora_do_recorte_minimo() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__", "--alvo=rosa", "--extra");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let err = semantic::check_program(&common::parse(&source).unwrap())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("chamada de 'capturar_stderr' com aridade inválida"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase164_capturar_stderr_falha_com_spawn_invalido() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("/__pinker_fase164_comando_inexistente__");
            mimo tamanho(texto);
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stderr'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase164_capturar_stderr_rejeita_comando_vazio() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("");
            mimo tamanho(texto);
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("intrínseca 'capturar_stderr' exige comando não vazio"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase164_capturar_stderr_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__ --flag");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stderr'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase170_capturar_stderr_com_argv_explicito_ainda_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__ --flag", "--alvo=rosa");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'capturar_stderr'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase164_capturar_stderr_rejeita_stderr_nao_utf8() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_invalido_utf8")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("stderr inválido em 'capturar_stderr'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase170_capturar_stderr_com_argv_explicito_preserva_utf8_estrito() {
    let source = r#"pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("__CMD__", "--alvo=rosa");
            mimo tamanho(texto);
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase164_helper_bin("stderr_invalido_utf8")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("stderr inválido em 'capturar_stderr'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase164_captura_stderr_minima_valido() {
    let output = run_cli_check_example("examples/fase164_captura_stderr_minima_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_check_fase170_captura_stderr_argv_explicito_minimo_valido() {
    let output =
        run_cli_check_example("examples/fase170_captura_stderr_argv_explicito_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase170_captura_stderr_argv_explicito_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase170_captura_stderr_argv_explicito_minimo_valido.pink",
        &[fase164_helper_bin("stderr_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "erro=sim\nalvo=rosa\n\n"
    );
}

#[test]
fn cli_check_fase170_captura_stderr_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_check_example(
        "examples/fase170_captura_stderr_argv_explicito_fluxo_composto_valido.pink",
    );
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase170_captura_stderr_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase170_captura_stderr_argv_explicito_fluxo_composto_valido.pink",
        &[fase164_helper_bin("stderr_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "stderr=19 bytes\n");
}

#[test]
fn cli_run_fase164_captura_stderr_minima_valido() {
    let output = run_cli_example_with_args(
        "examples/fase164_captura_stderr_minima_valido.pink",
        &[fase164_helper_bin("stderr_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "erro=sim\ncodigo=9\n\n"
    );
}

#[test]
fn cli_check_fase164_captura_stderr_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase164_captura_stderr_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase164_captura_stderr_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase164_captura_stderr_fluxo_composto_valido.pink",
        &[fase164_helper_bin("stderr_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "stderr=18 bytes\n");
}

// @pinker-nav:end evidencia.interpreter.processos-captura-stderr
// @pinker-nav:start evidencia.interpreter.processos-entrada-stdin
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre executar com entrada stdin (código zero, fluxo composto, argv explícito, rejeição de spawn inválido, sem shell implícito) no interpretador e via CLI.
#[test]
fn run_fase165_executar_com_entrada_minimo_retorna_codigo_zero() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("__CMD__", "rosa\n");
            talvez codigo == 0 {
                mimo 165;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(165)));
}

#[test]
fn run_fase165_executar_com_entrada_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer processo.executar_com_entrada; trazer texto.formatar; trazer texto.juntar; trazer texto.tamanho;
        carinho montar() -> verso {
            nova prefixo: verso = "linha=ok\n";
            nova sufixo: verso = formatar("valor={}\n", 7);
            mimo juntar(prefixo, sufixo);
        }

        carinho principal() -> bombom {
            nova entrada: verso = montar();
            nova codigo: bombom = executar_com_entrada("__CMD__", entrada);
            talvez codigo == 0 && tamanho(entrada) == 17 {
                mimo 165;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(165)));
}

#[test]
fn run_fase177_executar_com_entrada_aceita_argv_explicito_minimo() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("__CMD__", "argv=ok\n", "--modo=ok");
            talvez codigo == 0 {
                mimo 177;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(177)));
}

#[test]
fn run_fase177_executar_com_entrada_fluxo_composto_com_argv_explicito() {
    let source = r#"pacote main; trazer processo.executar_com_entrada; trazer texto.formatar; trazer texto.igual; trazer texto.juntar;
        carinho montar_entrada() -> verso {
            nova prefixo: verso = "linha=argv\n";
            nova sufixo: verso = formatar("valor={}\n", 177);
            mimo juntar(prefixo, sufixo);
        }

        carinho verificar(comando: verso, entrada: verso, arg: verso) -> bombom {
            nova codigo: bombom = executar_com_entrada(comando, entrada, arg);
            falar(formatar("stdin_argv_status={}", codigo));
            mimo codigo;
        }

        carinho principal() -> bombom {
            nova entrada: verso = montar_entrada();
            nova codigo: bombom = verificar("__CMD__", entrada, "--modo=ok");
            talvez codigo == 0 && igual(entrada, "linha=argv\nvalor=177\n") {
                mimo 177;
            }
            mimo 0;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(177)));
}

#[test]
fn run_fase165_executar_com_entrada_falha_com_spawn_invalido() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("/__pinker_fase165_comando_inexistente__", "rosa\n");
            mimo codigo;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'executar_com_entrada'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase165_executar_com_entrada_rejeita_comando_vazio() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("", "rosa\n");
            mimo codigo;
        }"#;
    let err = run_code(source).unwrap_err();
    assert!(
        err.contains("intrínseca 'executar_com_entrada' exige comando não vazio"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase165_executar_com_entrada_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("__CMD__ --flag", "rosa\n");
            mimo codigo;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'executar_com_entrada'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase177_executar_com_entrada_com_argv_explicito_ainda_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("__CMD__ --flag", "argv=ok\n", "--modo=ok");
            mimo codigo;
        }"#
    .replace(
        "__CMD__",
        &pink_string_literal(fase165_helper_bin("stdin_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo em 'executar_com_entrada'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase165_stdin_textual_minimo_valido() {
    let output = run_cli_check_example("examples/fase165_stdin_textual_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_check_fase177_stdin_textual_argv_explicito_minimo_valido() {
    let output =
        run_cli_check_example("examples/fase177_stdin_textual_argv_explicito_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase165_stdin_textual_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase165_stdin_textual_minimo_valido.pink",
        &[fase165_helper_bin("stdin_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_run_fase177_stdin_textual_argv_explicito_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase177_stdin_textual_argv_explicito_minimo_valido.pink",
        &[fase165_helper_bin("stdin_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_check_fase165_stdin_textual_fluxo_composto_valido() {
    let output = run_cli_check_example("examples/fase165_stdin_textual_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_check_fase177_stdin_textual_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_check_example(
        "examples/fase177_stdin_textual_argv_explicito_fluxo_composto_valido.pink",
    );
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase165_stdin_textual_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase165_stdin_textual_fluxo_composto_valido.pink",
        &[fase165_helper_bin("stdin_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "stdin_status=0\n");
}

#[test]
fn cli_run_fase177_stdin_textual_argv_explicito_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase177_stdin_textual_argv_explicito_fluxo_composto_valido.pink",
        &[fase165_helper_bin("stdin_ok")],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "stdin_argv_status=0\n"
    );
}

// @pinker-nav:end evidencia.interpreter.processos-entrada-stdin
// @pinker-nav:start evidencia.interpreter.processos-pipeline
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre pipeline mínimo (código do consumidor, sem shell implícito) no interpretador e via CLI.
#[test]
fn run_fase166_pipeline_minimo_retorna_codigo_do_consumidor() {
    let source = r#"pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("__PRODUTOR__", "__CONSUMIDOR__");
            talvez codigo == 0 {
                mimo 166;
            }
            mimo 0;
        }"#
    .replace(
        "__PRODUTOR__",
        &pink_string_literal(fase166_helper_bin("produtor_pipe_ok")),
    )
    .replace(
        "__CONSUMIDOR__",
        &pink_string_literal(fase166_helper_bin("consumidor_stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(166)));
}

#[test]
fn run_fase166_pipeline_minimo_fluxo_composto_funciona() {
    let source = r#"pacote main; trazer processo.pipeline_minimo; trazer texto.formatar; trazer texto.igual;
        carinho verificar(nome: verso, produtor: verso, consumidor: verso) -> bombom {
            nova codigo: bombom = pipeline_minimo(produtor, consumidor);
            falar(formatar("{}={}", nome, codigo));
            mimo codigo;
        }

        carinho principal() -> bombom {
            nova codigo: bombom = verificar("pipe", "__PRODUTOR__", "__CONSUMIDOR__");
            nova resumo: verso = formatar("pipe_zero={}", codigo);
            falar(resumo);
            talvez codigo == 0 && igual(resumo, "pipe_zero=0") {
                mimo 166;
            }
            mimo 0;
        }"#
    .replace(
        "__PRODUTOR__",
        &pink_string_literal(fase166_helper_bin("produtor_pipe_ok")),
    )
    .replace(
        "__CONSUMIDOR__",
        &pink_string_literal(fase166_helper_bin("consumidor_stdin_ok")),
    );
    let out = run_code(&source).unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(166)));
}

#[test]
fn run_fase166_pipeline_minimo_falha_com_spawn_produtor_invalido() {
    let source = r#"pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("/__pinker_fase166_produtor_inexistente__", "__CONSUMIDOR__");
            mimo codigo;
        }"#
    .replace(
        "__CONSUMIDOR__",
        &pink_string_literal(fase166_helper_bin("consumidor_stdin_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo produtor em 'pipeline_minimo'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase166_pipeline_minimo_falha_com_spawn_consumidor_invalido() {
    let source = r#"pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("__PRODUTOR__", "/__pinker_fase166_consumidor_inexistente__");
            mimo codigo;
        }"#
    .replace(
        "__PRODUTOR__",
        &pink_string_literal(fase166_helper_bin("produtor_pipe_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo consumidor em 'pipeline_minimo'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase166_pipeline_minimo_rejeita_comando_vazio() {
    let source = r#"pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("", "__CONSUMIDOR__");
            mimo codigo;
        }"#
    .replace(
        "__CONSUMIDOR__",
        &pink_string_literal(fase166_helper_bin("consumidor_stdin_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("intrínseca 'pipeline_minimo' exige comando não vazio"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn run_fase166_pipeline_minimo_nao_abre_shell_implicito() {
    let source = r#"pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("__PRODUTOR__ --flag", "__CONSUMIDOR__");
            mimo codigo;
        }"#
    .replace(
        "__PRODUTOR__",
        &pink_string_literal(fase166_helper_bin("produtor_pipe_ok")),
    )
    .replace(
        "__CONSUMIDOR__",
        &pink_string_literal(fase166_helper_bin("consumidor_stdin_ok")),
    );
    let err = run_code(&source).unwrap_err();
    assert!(
        err.contains("falha ao executar processo produtor em 'pipeline_minimo'"),
        "erro inesperado: {}",
        err
    );
}

#[test]
fn cli_check_fase166_pipe_minimo_valido() {
    let output = run_cli_check_example("examples/fase166_pipe_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase166_pipe_minimo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase166_pipe_minimo_valido.pink",
        &[
            fase166_helper_bin("produtor_pipe_ok"),
            fase166_helper_bin("consumidor_stdin_ok"),
        ],
    );
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn cli_check_fase166_pipe_minimo_fluxo_composto_valido() {
    let output = run_cli_check_example("examples/fase166_pipe_minimo_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase166_pipe_minimo_fluxo_composto_valido() {
    let output = run_cli_example_with_args(
        "examples/fase166_pipe_minimo_fluxo_composto_valido.pink",
        &[
            fase166_helper_bin("produtor_pipe_ok"),
            fase166_helper_bin("consumidor_stdin_ok"),
        ],
    );
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "etapa=0\npipeline_status=0\npipeline=ok\n"
    );
}

// @pinker-nav:end evidencia.interpreter.processos-pipeline
// @pinker-nav:start evidencia.interpreter.arquivos-csv-json-cli-exemplos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI versionados básicos de JSON e CSV, verificando validade e saída.
#[test]
fn cli_run_fase159_json_basico_valido() {
    let output = run_cli_example("examples/fase159_json_basico_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("{\"idade\":7,\"pontos\":9}"),
        "stdout={}",
        stdout
    );
}

#[test]
fn cli_check_fase159_json_basico_fluxo_composto_valido() {
    let output = run_cli_check_example("examples/fase159_json_basico_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase159_json_basico_fluxo_composto_valido() {
    let output = run_cli_example("examples/fase159_json_basico_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("{\"falhas\":2,\"ok\":5,\"total\":7}"),
        "stdout={}",
        stdout
    );
}

#[test]
fn cli_check_fase158_csv_minimo_valido() {
    let output = run_cli_check_example("examples/fase158_csv_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase158_csv_minimo_valido() {
    let output = run_cli_example("examples/fase158_csv_minimo_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n11\n7,11,13\n");
}

#[test]
fn cli_check_fase158_csv_minimo_fluxo_composto_valido() {
    let output = run_cli_check_example("examples/fase158_csv_minimo_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase158_csv_minimo_fluxo_composto_valido() {
    let output = run_cli_example("examples/fase158_csv_minimo_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "10;20;30\n10;20;30;60\n"
    );
}

// @pinker-nav:end evidencia.interpreter.arquivos-csv-json-cli-exemplos
// @pinker-nav:start evidencia.interpreter.texto-formatar-cli-exemplos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI de formatação simples, verificando saída e fluxo composto.
#[test]
fn cli_check_fase157_formatacao_simples_saida_valido() {
    let output = run_cli_check_example("examples/fase157_formatacao_simples_saida_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase157_formatacao_simples_saida_valido() {
    let output = run_cli_example("examples/fase157_formatacao_simples_saida_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "saldo=42\n8\n");
}

#[test]
fn cli_check_fase157_formatacao_simples_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase157_formatacao_simples_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase157_formatacao_simples_fluxo_composto_valido() {
    let output = run_cli_example("examples/fase157_formatacao_simples_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "relatorio rodada\ntotal=2\nprimeiro=7\n"
    );
}

// ── Fase 148: escrita por índice em array fixo [bombom; N] ───────────────────

// @pinker-nav:end evidencia.interpreter.texto-formatar-cli-exemplos
// @pinker-nav:start evidencia.interpreter.ponteiros-escrita-indice-e-array-fixo
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita escrita por índice em array por valor (com releitura comprovando o efeito) e array fixo, no interpretador e via CLI.
#[test]
fn run_escrita_por_indice_em_array_por_valor_minima_funciona() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         eterno C: bombom = 30;
         carinho escreve_e_le(a: [bombom; 3]) -> bombom {
             a[1] = 99;
             mimo a[1];
         }
         carinho principal() -> bombom {
             nova base: seta<[bombom; 3]> = 1;
             mimo escreve_e_le(*base);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(99)));
}

#[test]
fn run_escrita_por_indice_leitura_apos_escrita_comprova_efeito() {
    let out = run_code(
        "pacote main;
         eterno A: bombom = 10;
         eterno B: bombom = 20;
         eterno C: bombom = 30;
         carinho preenche(a: [bombom; 3], i: bombom, v: bombom) {
             a[i] = v;
         }
         carinho le(a: [bombom; 3], i: bombom) -> bombom {
             mimo a[i];
         }
         carinho principal() -> bombom {
             nova base: seta<[bombom; 3]> = 1;
             preenche(*base, 2, 77);
             mimo le(*base, 2);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(77)));
}

#[test]
fn cli_run_fase148_array_fixo_escrita_indice_minima_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase148_array_fixo_escrita_indice_minima_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("99"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase148_array_fixo_escrita_indice_elemento_nao_bombom_invalido_falha() {
    let output = run_cli_check_example(
        "examples/fase148_array_fixo_escrita_indice_elemento_nao_bombom_invalido.pink",
    );
    assert!(
        !output.status.success(),
        "esperava falha para escrita fora do recorte bombom"
    );
}

// ── Fase 149: lista mínima homogênea de bombom ──────────────────────────────

// @pinker-nav:end evidencia.interpreter.ponteiros-escrita-indice-e-array-fixo
// @pinker-nav:start evidencia.interpreter.colecoes-lista-bombom
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre lista de bombom (criar/anexar/obter/definir/tirar último) no interpretador e via exemplos CLI, com rejeições fora da faixa e lista vazia; recorte homogêneo bombom.
#[test]
fn run_lista_bombom_minima_criar_anexar_obter_funciona() {
    let out = run_code(
        "pacote main;
         carinho principal() -> bombom {
             nova l: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(l, 7);
             lista_bombom_anexar(l, 11);
             mimo lista_bombom_obter(l, 1);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(11)));
}

#[test]
fn run_lista_bombom_fluxo_composto_funciona() {
    let out = run_code(
        "pacote main;
         carinho carregar_base() -> lista<bombom> {
             nova l: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(l, 40);
             lista_bombom_anexar(l, 2);
             mimo l;
         }
         carinho soma_primeiros(l: lista<bombom>) -> bombom {
             nova a: bombom = lista_bombom_obter(l, 0);
             nova b: bombom = lista_bombom_obter(l, 1);
             mimo a + b;
         }
         carinho principal() -> bombom {
             nova itens: lista<bombom> = carregar_base();
             lista_bombom_anexar(itens, 99);
             falar(lista_bombom_tamanho(itens));
             mimo soma_primeiros(itens);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn cli_run_fase149_lista_minima_bombom_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase149_lista_minima_bombom_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('5'), "stdout={}", stdout);
    assert!(stdout.contains('8'), "stdout={}", stdout);
}

#[test]
fn cli_run_fase149_lista_fluxo_composto_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase149_lista_minima_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('3'), "stdout={}", stdout);
    assert!(stdout.contains("42"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase149_lista_homogenea_invalida_falha() {
    let output =
        run_cli_check_example("examples/fase149_lista_minima_bombom_homogenea_invalido.pink");
    assert!(
        !output.status.success(),
        "esperava falha para lista fora do recorte homogêneo de bombom"
    );
}

// ── Fase 150: escrita mínima por índice em lista<bombom> ───────────────────

#[test]
fn run_lista_bombom_definir_minimo_funciona() {
    let out = run_code(
        "pacote main;
         carinho principal() -> bombom {
             nova l: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(l, 7);
             lista_bombom_definir(l, 0, 33);
             mimo lista_bombom_obter(l, 0);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(33)));
}

#[test]
fn run_lista_bombom_definir_fluxo_composto_funciona() {
    let out = run_code(
        "pacote main;
         carinho carregar() -> lista<bombom> {
             nova itens: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(itens, 40);
             lista_bombom_anexar(itens, 2);
             mimo itens;
         }
         carinho ajustar(itens: lista<bombom>) {
             nova atual: bombom = lista_bombom_obter(itens, 1);
             lista_bombom_definir(itens, 1, atual + 8);
         }
         carinho principal() -> bombom {
             nova itens: lista<bombom> = carregar();
             ajustar(itens);
             falar(lista_bombom_tamanho(itens));
             mimo lista_bombom_obter(itens, 1);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(10)));
}

#[test]
fn run_lista_bombom_definir_fora_da_faixa_falha_claro() {
    let err = run_code(
        "pacote main;
         carinho principal() -> bombom {
             nova l: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(l, 1);
             lista_bombom_definir(l, 2, 9);
             mimo 0;
         }",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("índice fora do intervalo em 'lista_bombom_definir'"),
        "{}",
        err
    );
}

#[test]
fn cli_run_fase150_lista_bombom_definir_minimo_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase150_lista_bombom_definir_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("33"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase150_lista_bombom_definir_fluxo_composto_funciona_com_exemplo_versionado() {
    let output =
        run_cli_example("examples/fase150_lista_bombom_definir_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('2'), "stdout={}", stdout);
    assert!(stdout.contains("10"), "stdout={}", stdout);
}

// ── Fase 151: remoção mínima do fim em lista<bombom> ───────────────────────

#[test]
fn run_lista_bombom_tirar_ultimo_minimo_funciona() {
    let out = run_code(
        "pacote main;
         carinho principal() -> bombom {
             nova l: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(l, 7);
             lista_bombom_anexar(l, 11);
             mimo lista_bombom_tirar_ultimo(l);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(11)));
}

#[test]
fn run_lista_bombom_tirar_ultimo_fluxo_composto_funciona() {
    let out = run_code(
        "pacote main;
         carinho carregar() -> lista<bombom> {
             nova itens: lista<bombom> = lista_bombom_criar();
             lista_bombom_anexar(itens, 4);
             lista_bombom_anexar(itens, 8);
             lista_bombom_anexar(itens, 15);
             mimo itens;
         }
         carinho fechar_lote(itens: lista<bombom>) -> bombom {
             mimo lista_bombom_tirar_ultimo(itens);
         }
         carinho principal() -> bombom {
             nova itens: lista<bombom> = carregar();
             nova retirado: bombom = fechar_lote(itens);
             falar(retirado);
             falar(lista_bombom_tamanho(itens));
             mimo lista_bombom_obter(itens, 1);
         }",
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(8)));
}

#[test]
fn run_lista_bombom_tirar_ultimo_em_lista_vazia_falha_claro() {
    let err = run_code(
        "pacote main;
         carinho principal() -> bombom {
             nova l: lista<bombom> = lista_bombom_criar();
             mimo lista_bombom_tirar_ultimo(l);
         }",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("lista vazia em 'lista_bombom_tirar_ultimo'"),
        "{}",
        err
    );
}

#[test]
fn cli_check_fase151_lista_bombom_tirar_ultimo_minimo_valido() {
    let output =
        run_cli_check_example("examples/fase151_lista_bombom_tirar_ultimo_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase151_lista_bombom_tirar_ultimo_minimo_funciona_com_exemplo_versionado() {
    let output = run_cli_example("examples/fase151_lista_bombom_tirar_ultimo_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("11"), "stdout={}", stdout);
    assert!(stdout.contains('1'), "stdout={}", stdout);
}

#[test]
fn cli_check_fase151_lista_bombom_tirar_ultimo_fluxo_composto_valido() {
    let output = run_cli_check_example(
        "examples/fase151_lista_bombom_tirar_ultimo_fluxo_composto_valido.pink",
    );
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase151_lista_bombom_tirar_ultimo_fluxo_composto_funciona_com_exemplo_versionado() {
    let output =
        run_cli_example("examples/fase151_lista_bombom_tirar_ultimo_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("15"), "stdout={}", stdout);
    assert!(stdout.contains('2'), "stdout={}", stdout);
    assert!(stdout.contains('8'), "stdout={}", stdout);
}

// ── Fase 152: mapa mínimo homogêneo verso -> bombom ────────────────────────

// @pinker-nav:end evidencia.interpreter.colecoes-lista-bombom
// @pinker-nav:start evidencia.interpreter.colecoes-mapa-verso-bombom
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre mapa verso→bombom (criar/definir/obter/tem) no interpretador e via CLI, com rejeição de chave ausente.
#[test]
fn run_mapa_verso_bombom_minimo_criar_definir_obter_tem_funciona() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer mapa.verso_bombom_tem;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "idade", 7);
             talvez verso_bombom_tem(m, "idade") {
                 mimo verso_bombom_obter(m, "idade");
             }
             mimo 0;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(7)));
}

#[test]
fn run_mapa_verso_bombom_fluxo_composto_funciona() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer mapa.verso_bombom_tem;
         carinho carregar() -> mapa<verso,bombom> {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "ana", 10);
             verso_bombom_definir(m, "bia", 20);
             mimo m;
         }
         carinho principal() -> bombom {
             nova placar: mapa<verso,bombom> = carregar();
             talvez verso_bombom_tem(placar, "ana") {
                 nova atual: bombom = verso_bombom_obter(placar, "ana");
                 verso_bombom_definir(placar, "ana", atual + 5);
             }
             mimo verso_bombom_obter(placar, "ana");
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(15)));
}

#[test]
fn run_mapa_verso_bombom_obter_chave_ausente_falha_claro() {
    let err = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_obter;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             mimo verso_bombom_obter(m, "faltando");
         }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("chave ausente em 'mapa_verso_bombom_obter'"),
        "{}",
        err
    );
}

#[test]
fn cli_check_fase152_mapa_verso_bombom_minimo_valido() {
    let output = run_cli_check_example("examples/fase152_mapa_verso_bombom_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase152_mapa_verso_bombom_minimo_valido() {
    let output = run_cli_example("examples/fase152_mapa_verso_bombom_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("42"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase152_mapa_verso_bombom_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase152_mapa_verso_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase152_mapa_verso_bombom_fluxo_composto_valido() {
    let output = run_cli_example("examples/fase152_mapa_verso_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("15"), "stdout={}", stdout);
    assert!(stdout.contains("20"), "stdout={}", stdout);
}

// ── Fase 153: iteração confortável mínima sobre lista<bombom> ──────────────

// @pinker-nav:end evidencia.interpreter.colecoes-mapa-verso-bombom
// @pinker-nav:start evidencia.interpreter.colecoes-iteracao-lista-e-mapa
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cobre iteração sobre lista e mapa de bombom no interpretador e via CLI, com rejeição de iteração em tipo fora do recorte.
#[test]
fn run_fase153_iteracao_lista_bombom_minima_funciona() {
    let out = run_code(
        r#"pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar;
         carinho principal() -> bombom {
             nova itens: lista<bombom> = bombom_criar();
             bombom_anexar(itens, 5);
             bombom_anexar(itens, 7);
             nova muda soma: bombom = 0;
             para cada item em itens {
                 soma = soma + item;
             }
             mimo soma;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(12)));
}

#[test]
fn run_fase153_iteracao_lista_bombom_fluxo_composto_funciona() {
    let out = run_code(
        r#"pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar;
         carinho carregar() -> lista<bombom> {
             nova itens: lista<bombom> = bombom_criar();
             bombom_anexar(itens, 10);
             bombom_anexar(itens, 20);
             bombom_anexar(itens, 30);
             mimo itens;
         }
         carinho principal() -> bombom {
             nova dados: lista<bombom> = carregar();
             nova muda soma: bombom = 0;
             nova muda pares: bombom = 0;
             para cada valor em dados {
                 soma = soma + valor;
                 talvez valor % 2 == 0 {
                     pares = pares + 1;
                 }
             }
             falar(soma, pares);
             mimo soma;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(60)));
}

#[test]
fn run_fase153_iteracao_em_tipo_fora_do_recorte_falha_claro() {
    // Após Fase 154, `mapa<verso,bombom>` está no recorte.
    // O teste cobre tipo fora do recorte: bombom não é coleção iterável.
    let err = run_code(
        r#"pacote main;
         carinho principal() -> bombom {
             nova x: bombom = 5;
             para cada item em x {
                 falar(item);
             }
             mimo 0;
         }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("tipo inválido no argumento 1 da chamada 'lista_bombom_tamanho'"),
        "{}",
        err
    );
}

#[test]
fn cli_check_fase153_iteracao_lista_bombom_minima_valido() {
    let output = run_cli_check_example("examples/fase153_iteracao_lista_bombom_minima_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase153_iteracao_lista_bombom_minima_valido() {
    let output = run_cli_example("examples/fase153_iteracao_lista_bombom_minima_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("12"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase153_iteracao_lista_bombom_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase153_iteracao_lista_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase153_iteracao_lista_bombom_fluxo_composto_valido() {
    let output =
        run_cli_example("examples/fase153_iteracao_lista_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("60"), "stdout={}", stdout);
    assert!(stdout.contains('3'), "stdout={}", stdout);
}

// ── Fase 154: iteração confortável mínima sobre mapa<verso,bombom> ──────────

#[test]
fn run_fase154_iteracao_mapa_verso_bombom_minima_funciona() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "pontos", 42);
             nova muda total: bombom = 0;
             para cada chave em m {
                 nova v: bombom = verso_bombom_obter(m, chave);
                 total = total + v;
             }
             mimo total;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(42)));
}

#[test]
fn run_fase154_iteracao_mapa_verso_bombom_chave_verso_no_corpo() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_tem;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "chave_unica", 7);
             nova muda encontrou: bombom = 0;
             para cada k em m {
                 talvez verso_bombom_tem(m, k) {
                     encontrou = encontrou + 1;
                 }
             }
             mimo encontrou;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_fase154_iteracao_mapa_verso_bombom_valor_obter_no_corpo() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "a", 10);
             verso_bombom_definir(m, "b", 20);
             verso_bombom_definir(m, "c", 30);
             nova muda soma: bombom = 0;
             para cada k em m {
                 nova v: bombom = verso_bombom_obter(m, k);
                 soma = soma + v;
             }
             mimo soma;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(60)));
}

#[test]
fn run_fase154_iteracao_mapa_verso_bombom_vazio_nao_executa_corpo() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar;
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             nova muda execucoes: bombom = 0;
             para cada k em m {
                 execucoes = execucoes + 1;
             }
             mimo execucoes;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(0)));
}

#[test]
fn run_fase154_iteracao_lista_bombom_continua_funcional() {
    // Regressão: Phase 153 ainda funciona após Phase 154.
    let out = run_code(
        r#"pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar;
         carinho principal() -> bombom {
             nova itens: lista<bombom> = bombom_criar();
             bombom_anexar(itens, 1);
             bombom_anexar(itens, 2);
             bombom_anexar(itens, 3);
             nova muda soma: bombom = 0;
             para cada item em itens {
                 soma = soma + item;
             }
             mimo soma;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(6)));
}

#[test]
fn run_fase154_iteracao_mapa_em_parametro_funciona() {
    let out = run_code(
        r#"pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter;
         carinho somar(m: mapa<verso,bombom>) -> bombom {
             nova muda s: bombom = 0;
             para cada k em m {
                 nova v: bombom = verso_bombom_obter(m, k);
                 s = s + v;
             }
             mimo s;
         }
         carinho principal() -> bombom {
             nova m: mapa<verso,bombom> = verso_bombom_criar();
             verso_bombom_definir(m, "x", 5);
             verso_bombom_definir(m, "y", 15);
             mimo somar(m);
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(20)));
}

#[test]
fn run_fase154_iteracao_mapa_tipo_fora_do_recorte_bombom_falha_claro() {
    // Iteração sobre tipo primitivo (bombom) continua falhando com erro claro.
    let err = run_code(
        r#"pacote main;
         carinho principal() -> bombom {
             nova x: bombom = 5;
             para cada item em x {
                 falar(item);
             }
             mimo 0;
         }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("tipo inválido no argumento 1 da chamada 'lista_bombom_tamanho'"),
        "{}",
        err
    );
}

#[test]
fn cli_check_fase154_iteracao_mapa_verso_bombom_minima_valido() {
    let output =
        run_cli_check_example("examples/fase154_iteracao_mapa_verso_bombom_minima_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase154_iteracao_mapa_verso_bombom_minima_valido() {
    let output = run_cli_example("examples/fase154_iteracao_mapa_verso_bombom_minima_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("42"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido() {
    let output = run_cli_check_example(
        "examples/fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido.pink",
    );
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido() {
    let output =
        run_cli_example("examples/fase154_iteracao_mapa_verso_bombom_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("60"), "stdout={}", stdout);
    assert!(stdout.contains('2'), "stdout={}", stdout);
}

// ── Fase 156: aleatoriedade básica com semente explícita ───────────────────

// @pinker-nav:end evidencia.interpreter.colecoes-iteracao-lista-e-mapa
// @pinker-nav:start evidencia.interpreter.aleatoriedade-semente
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita aleatoriedade com semente (mesma semente produz mesma sequência, sementes diferentes, handle inválido) no interpretador e via CLI; determinismo por semente, não qualidade estatística.
#[test]
fn run_fase156_mesma_semente_produz_mesma_sequencia() {
    let out = run_code(
        r#"pacote main; trazer acaso.criar; trazer acaso.proximo; trazer arquivo.criar;
         carinho principal() -> bombom {
             nova a: bombom = criar(42);
             nova b: bombom = criar(42);
             nova a1: bombom = proximo(a);
             nova b1: bombom = proximo(b);
             nova a2: bombom = proximo(a);
             nova b2: bombom = proximo(b);
             talvez a1 == b1 {
                 talvez a2 == b2 {
                     mimo 1;
                 }
             }
             mimo 0;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_fase156_sementes_diferentes_sao_distinguiveis() {
    let out = run_code(
        r#"pacote main; trazer acaso.criar; trazer acaso.proximo; trazer arquivo.criar;
         carinho principal() -> bombom {
             nova a: bombom = criar(1);
             nova b: bombom = criar(2);
             nova va: bombom = proximo(a);
             nova vb: bombom = proximo(b);
             talvez va == vb {
                 mimo 0;
             }
             mimo 1;
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(1)));
}

#[test]
fn run_fase156_fluxo_composto_com_lista_funciona() {
    let out = run_code(
        r#"pacote main; trazer acaso.criar; trazer acaso.proximo; trazer arquivo.criar; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_obter; trazer lista.bombom_tamanho;
         carinho rolar_face(gerador: bombom) -> bombom {
             mimo (proximo(gerador) % 6) + 1;
         }
         carinho jogar_rodada(gerador: bombom, historico: lista<bombom>) -> bombom {
             nova dado_a: bombom = rolar_face(gerador);
             nova dado_b: bombom = rolar_face(gerador);
             bombom_anexar(historico, dado_a);
             bombom_anexar(historico, dado_b);
             mimo dado_a + dado_b;
         }
         carinho principal() -> bombom {
             nova gerador: bombom = criar(2024);
             nova historico: lista<bombom> = bombom_criar();
             nova primeira: bombom = jogar_rodada(gerador, historico);
             nova segunda: bombom = jogar_rodada(gerador, historico);
             falar(bombom_tamanho(historico));
             falar(primeira, segunda);
             mimo bombom_obter(historico, 0) + bombom_obter(historico, 3);
         }"#,
    )
    .unwrap();
    assert_eq!(out, Some(RuntimeValue::Int(9)));
}

#[test]
fn run_fase156_handle_invalido_falha_claro() {
    let err = run_code(
        r#"pacote main; trazer acaso.proximo;
         carinho principal() -> bombom {
             mimo proximo(999);
         }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("handle de aleatoriedade inválido em 'aleatorio_proximo'"),
        "{}",
        err
    );
}

#[test]
fn cli_check_fase156_aleatoriedade_basica_semente_valido() {
    let output = run_cli_check_example("examples/fase156_aleatoriedade_basica_semente_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase156_aleatoriedade_basica_semente_valido() {
    let output = run_cli_example("examples/fase156_aleatoriedade_basica_semente_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.matches("verdade").count(), 2, "stdout={}", stdout);
}

#[test]
fn cli_check_fase156_aleatoriedade_basica_fluxo_composto_valido() {
    let output =
        run_cli_check_example("examples/fase156_aleatoriedade_basica_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase156_aleatoriedade_basica_fluxo_composto_valido() {
    let output =
        run_cli_example("examples/fase156_aleatoriedade_basica_fluxo_composto_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('4'), "stdout={}", stdout);
    assert!(stdout.contains("7 7"), "stdout={}", stdout);
    assert_eq!(output.status.code(), Some(9));
}

// ── Fases 186–188 — importação por família: `tempo`, `ambiente` e `acaso` ──

// @pinker-nav:end evidencia.interpreter.aleatoriedade-semente
// @pinker-nav:start evidencia.interpreter.leques-trazer-recursos-e-programas-brinquedo
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa exemplos CLI de leques, intrínsecas 'trazer' (tempo, texto, arquivo, caminho, processo) e programas brinquedo (lexer/compilador), verificando validade e saída.
#[test]
fn cli_check_fase186_trazer_tempo_minimo_valido() {
    let output = run_cli_check_example("examples/fase186_trazer_tempo_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase186_trazer_tempo_minimo_valido() {
    let output = run_cli_example("examples/fase186_trazer_tempo_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1970-01-01T00:00:00Z"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase187_trazer_ambiente_minimo_valido() {
    let output = run_cli_check_example("examples/fase187_trazer_ambiente_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase187_trazer_ambiente_minimo_valido() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase187_trazer_ambiente_minimo_valido.pink")
        .arg("--")
        .arg("--saida")
        .arg("cli.txt")
        .arg("--quiet")
        .env("PINKER_FASE187_AMBIENTE", "env.txt")
        .output()
        .expect("falha ao executar pink --run no exemplo da fase 187");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cli.txt"), "stdout={}", stdout);
    assert!(stdout.contains("env.txt"), "stdout={}", stdout);
    assert!(stdout.contains("quiet"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase188_trazer_acaso_minimo_valido() {
    let output = run_cli_check_example("examples/fase188_trazer_acaso_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase188_trazer_acaso_minimo_valido() {
    let output = run_cli_example("examples/fase188_trazer_acaso_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("10481999410520546993"), "stdout={}", stdout);
    assert!(stdout.contains("4159066171780167020"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase189_trazer_texto_minimo_valido() {
    let output = run_cli_check_example("examples/fase189_trazer_texto_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase189_trazer_texto_minimo_valido() {
    let output = run_cli_example("examples/fase189_trazer_texto_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rosa pinker"), "stdout={}", stdout);
    assert!(stdout.contains("texto"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase207_trazer_arquivo_caminho_processo_valido() {
    let output =
        run_cli_check_example("examples/fase207_trazer_arquivo_caminho_processo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_check_fase208_leque_minimo_valido() {
    let output = run_cli_check_example("examples/fase208_leque_minimo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase208_leque_minimo_valido() {
    let output = run_cli_example("examples/fase208_leque_minimo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("verde"), "stdout={}", stdout);
    assert!(stdout.contains("azul"), "stdout={}", stdout);
    assert!(stdout.contains('0'), "stdout={}", stdout);
}

#[test]
fn cli_check_fase209_leque_carga_encaixe_valido() {
    let output = run_cli_check_example("examples/fase209_leque_carga_encaixe_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase209_leque_carga_encaixe_valido() {
    let output = run_cli_example("examples/fase209_leque_carga_encaixe_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("numero 42"), "stdout={}", stdout);
    assert!(stdout.contains("palavra rosa"), "stdout={}", stdout);
    assert!(stdout.contains("fim"), "stdout={}", stdout);
    assert!(stdout.contains("fria"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase209_lexer_brinquedo_valido() {
    let output = run_cli_example("examples/fase209_lexer_brinquedo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PALAVRA soma"), "stdout={}", stdout);
    assert!(stdout.contains("NUMERO 12"), "stdout={}", stdout);
    assert!(stdout.contains("NUMERO 30"), "stdout={}", stdout);
    assert!(stdout.contains("PALAVRA total"), "stdout={}", stdout);
    assert!(stdout.contains("FIM"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase210_leque_recursivo_avaliador_valido() {
    let output = run_cli_check_example("examples/fase210_leque_recursivo_avaliador_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase210_leque_recursivo_avaliador_valido() {
    let output = run_cli_example("examples/fase210_leque_recursivo_avaliador_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("avaliando resposta"), "stdout={}", stdout);
    assert!(stdout.contains("42"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase211_lista_generica_valido() {
    let output = run_cli_example("examples/fase211_lista_generica_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('3'), "stdout={}", stdout);
    assert!(stdout.contains("quente"), "stdout={}", stdout);
    assert!(stdout.contains("fria"), "stdout={}", stdout);
    assert!(stdout.contains("verde saiu"), "stdout={}", stdout);
}

#[test]
fn cli_check_fase211_compilador_brinquedo_valido() {
    let output = run_cli_check_example("examples/fase211_compilador_brinquedo_valido.pink");
    assert_cli_completed(&output);
}

#[test]
fn cli_run_fase211_compilador_brinquedo_valido() {
    let output = run_cli_example("examples/fase211_compilador_brinquedo_valido.pink");
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tokens: 6"), "stdout={}", stdout);
    assert!(stdout.contains("42"), "stdout={}", stdout);
}

#[test]
fn cli_run_fase207_trazer_arquivo_caminho_processo_valido() {
    let output = run_cli_example_with_args(
        "examples/fase207_trazer_arquivo_caminho_processo_valido.pink",
        &[fase162_helper_bin("exit0")],
    );
    assert_cli_completed(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rosa pinker"), "stdout={}", stdout);
    assert!(stdout.contains('0'), "stdout={}", stdout);
}

// @pinker-nav:end evidencia.interpreter.leques-trazer-recursos-e-programas-brinquedo
// @pinker-nav:start evidencia.interpreter.execucao-funcoes-usuario-tratos-e-genericos
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa no interpretador funções de usuário, funções anônimas, tratos/impl (resolução nominal, ninho, cobertura, homônimos, múltiplos contratos), propagação e genéricos, comparando o valor por igualdade.
#[test]
fn fase223_tentar_error_handling_executa_no_interpretador() {
    let code = include_str!("../examples/fase223_error_handling_tentar_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase224_propagar_error_handling_executa_no_interpretador() {
    let code = include_str!("../examples/fase224_error_handling_propagar_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase225_carinho_anonimo_executa_no_interpretador() {
    let code = include_str!("../examples/fase225_carinho_anonimo_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase226_trato_metodo_executa_no_interpretador() {
    let code = include_str!("../examples/fase226_trato_metodo_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase227_impl_trato_executa_no_interpretador() {
    let code = include_str!("../examples/fase227_impl_trato_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase228_impl_resolucao_nominal_executa_no_interpretador() {
    let code = include_str!("../examples/fase228_impl_resolucao_nominal_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase229_impl_ninho_executa_no_interpretador() {
    let code = include_str!("../examples/fase229_impl_ninho_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase230_impl_cobertura_executa_no_interpretador() {
    let code = include_str!("../examples/fase230_impl_cobertura_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase231_propagar_valor_nomeado_executa_no_interpretador() {
    let code = include_str!("../examples/fase231_propagar_valor_nomeado_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase237_propagar_curto_executa_no_interpretador() {
    let code = include_str!("../examples/fase237_propagar_curto_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase238_funcao_local_valor_executa_no_interpretador() {
    let code = include_str!("../examples/fase238_funcao_local_valor_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase239_funcao_parametro_estatica_executa_no_interpretador() {
    let code = include_str!("../examples/fase239_funcao_parametro_estatica_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase240_leque_generico_resultado_executa_no_interpretador() {
    let code = include_str!("../examples/fase240_leque_generico_resultado_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase232_impl_multiplos_contratos_executa_no_interpretador() {
    let code = include_str!("../examples/fase232_impl_multiplos_contratos_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase233_mapa_generico_executa_no_interpretador() {
    let code = include_str!("../examples/fase233_mapa_generico_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase234_impl_homonimos_executa_no_interpretador() {
    let code = include_str!("../examples/fase234_impl_homonimos_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase235_mapa_generico_expressoes_executa_no_interpretador() {
    let code = include_str!("../examples/fase235_mapa_generico_expressoes_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase236_funcao_generica_usuario_executa_no_interpretador() {
    let code = include_str!("../examples/fase236_funcao_generica_usuario_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase241_resultado_predeclarado_executa_no_interpretador() {
    let code = include_str!("../examples/fase241_resultado_predeclarado_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase242_funcao_indireta_executa_no_interpretador() {
    let code = include_str!("../examples/fase242_funcao_indireta_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase242_funcao_indireta_stdout_via_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase242_funcao_indireta_valido.pink")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n63\n");
}

#[test]
fn fase242_variavel_local_callable_precedencia_executa_no_interpretador() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho triplicar(x: bombom) -> bombom { mimo x * 3; }
        carinho principal() -> bombom {
            nova dobrar: carinho(bombom) -> bombom = triplicar;
            mimo dobrar(10);
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(30)));
}

#[test]
fn fase242_callable_zero_argumentos_executa_no_interpretador() {
    let code = r#"
        pacote main;
        carinho constante() -> bombom { mimo 7; }
        carinho aplicar_zero(f: carinho() -> bombom) -> bombom { mimo f(); }
        carinho principal() -> bombom {
            mimo aplicar_zero(constante);
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(7)));
}

#[test]
fn fase242_callable_recebendo_callable_executa_no_interpretador() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho aplicar_duas_vezes(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
            mimo f(f(x));
        }
        carinho principal() -> bombom {
            mimo aplicar_duas_vezes(dobrar, 5);
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(20)));
}

#[test]
fn fase242_callable_com_oito_argumentos_cruza_pilha_no_interpretador() {
    let code = include_str!("../examples/fase242_funcao_indireta_pilha_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase242_funcao_anonima_nao_capturante_como_valor_executa() {
    let code = r#"
        pacote main;
        carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom { mimo f(x); }
        carinho principal() -> bombom {
            nova quadruplicar: carinho(bombom) -> bombom = carinho(x: bombom) -> bombom {
                mimo x * 4;
            };
            mimo aplicar(quadruplicar, 5);
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(20)));
}
// @pinker-nav:end evidencia.interpreter.execucao-funcoes-usuario-tratos-e-genericos

// @pinker-nav:start evidencia.interpreter.closures-captura-imutavel
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fase 243: executa closures com captura imutável por valor no interpretador — exemplo canônico com duas instâncias distintas (ambientes independentes, execução após o retorno do escopo criador), captura múltipla de tipos distintos e os dois exemplos de fronteira de ABI (pilha par/ímpar) com env cruzando para a pilha —, nos casos presentes.
#[test]
fn fase243_closure_captura_imutavel_executa_no_interpretador() {
    let code = include_str!("../examples/fase243_closure_captura_imutavel_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(84)));
}

#[test]
fn fase243_closure_captura_imutavel_stdout_via_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg("examples/fase243_closure_captura_imutavel_valido.pink")
        .output()
        .expect("falha ao executar CLI --run");
    assert_cli_completed(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n42\n");
}

#[test]
fn fase243_closure_ambientes_distintos_nao_interferem_no_interpretador() {
    // Duas instâncias de `fabricar_somador` (2 e 10) devem manter ambientes
    // heap independentes: se compartilhassem endereço, o resultado divergiria
    // de 84 (42 + 42).
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova a: carinho() -> bombom = fabricar(3);
            nova b: carinho() -> bombom = fabricar(9);
            nova c: carinho() -> bombom = fabricar(27);
            mimo a() + b() + c();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(39)));
}

#[test]
fn fase243_closure_executa_apos_retorno_do_escopo_criador_no_interpretador() {
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            nova resultado: carinho() -> bombom = carinho() -> bombom {
                mimo base * 2;
            };
            mimo resultado;
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(21);
            mimo f();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(42)));
}

#[test]
fn fase243_closure_captura_multipla_de_tipos_distintos_executa_no_interpretador() {
    let code = r#"
        pacote main; trazer texto.tamanho;
        carinho fabricar(base: bombom, ligado: logica, rotulo: verso) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                talvez ligado {
                    mimo base + tamanho(rotulo);
                } senao {
                    mimo base;
                }
            };
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(10, verdade, "abc");
            mimo f();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(13)));
}

#[test]
fn fase243_closure_aninhada_captura_transitiva_executa_no_interpretador() {
    // A closure intermediária (0 params, sem uso textual de `base`) precisa
    // capturar `base` só para repassá-la à closure mais interna — sem
    // propagação transitiva do free-var scan pelos níveis intermediários,
    // isso falha na resolução da IR (`identificador não resolvido`).
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> carinho() -> bombom {
            mimo carinho() -> carinho() -> bombom {
                mimo carinho() -> bombom {
                    mimo base;
                };
            };
        }
        carinho principal() -> bombom {
            nova externa: carinho() -> carinho() -> bombom = fabricar(55);
            nova interna: carinho() -> bombom = externa();
            mimo interna();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(55)));
}

#[test]
fn fase243_closure_captura_e_snapshot_por_valor_nao_por_referencia() {
    // A closure guarda o VALOR de `x` no instante da criação (1); a
    // reatribuição de `x` no escopo criador logo em seguida (99, permitida
    // porque `x` é `muda` — só a CAPTURA dentro da closure é imutável) não
    // pode ser observada pela closure já criada.
    let code = r#"
        pacote main;
        carinho fabricar() -> carinho() -> bombom {
            nova muda x: bombom = 1;
            nova f: carinho() -> bombom = carinho() -> bombom {
                mimo x;
            };
            x = 99;
            mimo f;
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar();
            mimo f();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(1)));
}

#[test]
fn fase243_closure_parametro_sombreia_captura_usa_valor_do_parametro() {
    // Companheiro comportamental de fase243_closure_parametro_sombreia_
    // captura_aceita (semantic_tests.rs, só type-check): aqui o resultado
    // real prova que o parâmetro (5) vence a captura homônima (18), não o
    // contrário.
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho(bombom) -> bombom {
            mimo carinho(base: bombom) -> bombom {
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova f: carinho(bombom) -> bombom = fabricar(18);
            mimo f(5);
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(5)));
}

#[test]
fn fase243_closure_local_sombreia_captura_usa_valor_do_local() {
    // Companheiro comportamental de fase243_closure_local_sombreia_captura_
    // aceita (semantic_tests.rs, só type-check).
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                nova base: bombom = base + 1000;
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(1);
            mimo f();
        }
    "#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(1001)));
}

#[test]
fn fase243_closure_pilha_par_cruza_registrador_e_pilha_no_interpretador() {
    let code = include_str!("../examples/fase243_closure_pilha_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase243_closure_pilha_impar_aplica_padding_no_interpretador() {
    let code = include_str!("../examples/fase243_closure_pilha_impar_valido.pink");
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}
// @pinker-nav:end evidencia.interpreter.closures-captura-imutavel

// @pinker-nav:start evidencia.interpreter.objetos-trato-fase244
// @pinker-nav:domain interpreter
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa objetos de trato no interpretador hospedado: snapshot, aliases, despacho dinâmico, callables e closures, diagnósticos de handles e reatribuições condicionais verdadeiras, falsas, aninhadas e repetidas com bindings inferidos e cópias.

#[test]
fn fase244_interpreter_materializa_despacha_e_preserva_snapshot() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom {
        mimo valor;
    }
}

carinho consultar(objeto: trato<Medivel>) -> bombom {
    mimo objeto.medir();
}

carinho principal() -> bombom {
    nova muda fonte: bombom = 21;
    nova objeto: trato<Medivel> =
        fonte virar trato<Medivel>;

    fonte = 99;

    mimo consultar(objeto);
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(21)));
}

#[test]
fn fase244_interpreter_copia_handle_e_mantem_objetos_independentes() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom {
        mimo valor;
    }
}

carinho principal() -> bombom {
    nova primeiro: trato<Medivel> =
        10 virar trato<Medivel>;
    nova copia: trato<Medivel> = primeiro;
    nova segundo: trato<Medivel> =
        20 virar trato<Medivel>;

    mimo copia.medir() + segundo.medir();
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(30)));
}

#[test]
fn fase244_interpreter_despacha_slot_ordenado_com_argumento_qualificado() {
    let code = r#"
pacote main;

trato Operavel {
    carinho base(valor: si) -> bombom;
    carinho somar(valor: si, adicional: bombom) -> bombom;
}

impl Operavel para bombom {
    carinho base(valor: bombom) -> bombom {
        mimo valor;
    }

    carinho somar(
        valor: bombom,
        adicional: bombom
    ) -> bombom {
        mimo valor + adicional;
    }
}

carinho principal() -> bombom {
    nova objeto: trato<Operavel> =
        10 virar trato<Operavel>;

    mimo Operavel.somar(objeto, 7);
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(17)));
}

#[test]
fn fase244_interpreter_trait_call_nula_nao_deixa_valor_na_pilha() {
    let code = r#"
pacote main;

trato Observavel {
    carinho observar(valor: si, codigo: bombom);
}

impl Observavel para bombom {
    carinho observar(valor: bombom, codigo: bombom) {
        nova total: bombom = valor + codigo;
        mimo;
    }
}

carinho principal() -> bombom {
    nova objeto: trato<Observavel> =
        35 virar trato<Observavel>;

    objeto.observar(7);

    mimo 0;
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(0)));
}

#[test]
fn fase244_interpreter_objeto_de_trato_cruza_retorno_de_funcao() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom {
        mimo valor;
    }
}

carinho empacotar(valor: bombom) -> trato<Medivel> {
    mimo valor virar trato<Medivel>;
}

carinho principal() -> bombom {
    nova objeto: trato<Medivel> = empacotar(13);
    mimo objeto.medir();
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(13)));
}

#[test]
fn fase244_interpreter_encadeia_retorno_dinamico_de_objeto_de_trato() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}

trato Fabrica {
    carinho criar(valor: si) -> trato<Medivel>;
}

impl Fabrica para bombom {
    carinho criar(valor: bombom) -> trato<Medivel> {
        mimo valor virar trato<Medivel>;
    }
}

carinho principal() -> bombom {
    nova fabrica: trato<Fabrica> = 42 virar trato<Fabrica>;
    mimo fabrica.criar().medir();
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(42)));
}

#[test]
fn fase244_interpreter_preserva_aliases_de_objeto_em_parametros_retornos_metodos_e_copias() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}

apelido ObjetoBase = trato<Medivel>;
apelido ObjetoPublico = ObjetoBase;
apelido Numero = bombom;

carinho usar_base(objeto: ObjetoBase) -> bombom { mimo objeto.medir(); }
carinho usar_publico(objeto: ObjetoPublico) -> bombom { mimo objeto.medir(); }
carinho criar_base(valor: bombom) -> ObjetoBase {
    mimo valor virar trato<Medivel>;
}
carinho criar_publico(valor: bombom) -> ObjetoPublico {
    mimo valor virar trato<Medivel>;
}

trato Fabrica {
    carinho criar(valor: si) -> ObjetoPublico;
}

impl Fabrica para bombom {
    carinho criar(valor: bombom) -> ObjetoPublico {
        mimo valor virar trato<Medivel>;
    }
}

carinho principal() -> bombom {
    nova direto: trato<Medivel> = 7 virar trato<Medivel>;
    nova base: ObjetoBase = 11 virar trato<Medivel>;
    nova publico: ObjetoPublico = 13 virar trato<Medivel>;
    nova copia = publico;
    nova numero: Numero = 5;
    nova fabrica: trato<Fabrica> = 41 virar trato<Fabrica>;
    mimo direto.medir()
        + usar_base(base)
        + usar_publico(copia)
        + copia.medir()
        + criar_base(17).medir()
        + criar_publico(19).medir()
        + fabrica.criar().medir()
        + numero;
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(126)));
}

fn fase244_manual_trait_program(code: Vec<MachineInstr>) -> MachineProgram {
    MachineProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![MachineFunction {
            name: "principal".to_string(),
            ret_type: ir::TypeIR::Bombom,
            params: vec![],
            locals: vec!["%objeto#0".to_string()],
            slot_types: HashMap::from([("%objeto#0".to_string(), ir::TypeIR::TraitObject)]),
            blocks: vec![MachineBlock {
                label: "entry".to_string(),
                code,
                terminator: MachineTerminator::Ret,
            }],
        }],
    }
}

#[test]
fn fase244_interpreter_rejeita_handle_de_objeto_invalido() {
    let program = fase244_manual_trait_program(vec![
        MachineInstr::PushInt(999),
        MachineInstr::StoreSlot("%objeto#0".to_string()),
        MachineInstr::LoadSlot("%objeto#0".to_string()),
        MachineInstr::TraitCall {
            trait_name: "Medivel".to_string(),
            method_name: "medir".to_string(),
            method_slot: 0,
            method_count: 2,
            argc: 0,
            param_types: vec![],
            ret_type: ir::TypeIR::Bombom,
        },
    ]);

    let err = interpreter::run_program(&program)
        .expect_err("handle inválido deve ser recusado")
        .to_string();

    assert!(
        err.contains("trait_call com handle de objeto de trato inválido"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_interpreter_rejeita_slot_de_vtable_invalido() {
    let program = fase244_manual_trait_program(vec![
        MachineInstr::PushInt(10),
        MachineInstr::MakeTraitObject {
            trait_name: "Medivel".to_string(),
            concrete_type: ir::TypeIR::Bombom,
            concrete_type_name: "bombom".to_string(),
            concrete_size: 8,
            vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
        },
        MachineInstr::StoreSlot("%objeto#0".to_string()),
        MachineInstr::LoadSlot("%objeto#0".to_string()),
        MachineInstr::TraitCall {
            trait_name: "Medivel".to_string(),
            method_name: "medir".to_string(),
            method_slot: 1,
            method_count: 2,
            argc: 0,
            param_types: vec![],
            ret_type: ir::TypeIR::Bombom,
        },
    ]);

    let err = interpreter::run_program(&program)
        .expect_err("slot inválido deve ser recusado")
        .to_string();

    assert!(
        err.contains("trait_call com slot de vtable inválido"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_interpreter_rejeita_trato_nominal_divergente() {
    let program = fase244_manual_trait_program(vec![
        MachineInstr::PushInt(10),
        MachineInstr::MakeTraitObject {
            trait_name: "Medivel".to_string(),
            concrete_type: ir::TypeIR::Bombom,
            concrete_type_name: "bombom".to_string(),
            concrete_size: 8,
            vtable_methods: vec!["__impl_7_Medivel_6_bombom_medir".to_string()],
        },
        MachineInstr::StoreSlot("%objeto#0".to_string()),
        MachineInstr::LoadSlot("%objeto#0".to_string()),
        MachineInstr::TraitCall {
            trait_name: "Outro".to_string(),
            method_name: "medir".to_string(),
            method_slot: 0,
            method_count: 2,
            argc: 0,
            param_types: vec![],
            ret_type: ir::TypeIR::Bombom,
        },
    ]);

    let err = interpreter::run_program(&program)
        .expect_err("trato divergente deve ser recusado")
        .to_string();

    assert!(
        err.contains("trait_call de trato incompatível"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_followup_callable_retorna_objeto_de_trato_no_interpretador() {
    let code = r#"
pacote main;
trato Medivel { carinho medir(valor: si) -> bombom; }
impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}
carinho criar(valor: bombom) -> trato<Medivel> {
    mimo valor virar trato<Medivel>;
}
carinho aplicar(
    fabrica: carinho(bombom) -> trato<Medivel>,
    valor: bombom
) -> bombom {
    mimo fabrica(valor).medir();
}
carinho escolher() -> carinho(bombom) -> trato<Medivel> { mimo criar; }
carinho principal() -> bombom {
    nova direto: carinho(bombom) -> trato<Medivel> = criar;
    nova alias = direto;
    nova alias2 = alias;
    nova copia = alias2;
    nova retornado = escolher();
    nova muda mutavel: carinho(bombom) -> trato<Medivel> = criar;
    mutavel = copia;
    nova muda anonimo: carinho(bombom) -> trato<Medivel> =
        carinho(valor: bombom) -> trato<Medivel> {
            mimo valor virar trato<Medivel>;
        };
    mimo direto(1).medir()
        + alias2(2).medir()
        + copia(3).medir()
        + aplicar(copia, 4)
        + retornado(5).medir()
        + mutavel(6).medir()
        + anonimo(7).medir();
}
"#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(28)));
}

#[test]
fn fase244_followup_closure_restaura_trato_e_callable_no_interpretador() {
    let code = r#"
pacote main;
trato Medivel { carinho medir(valor: si) -> bombom; }
impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}
carinho criar(valor: bombom) -> trato<Medivel> {
    mimo valor virar trato<Medivel>;
}
carinho combinar(
    objeto: trato<Medivel>,
    delta: bombom,
    fabrica: carinho(bombom) -> trato<Medivel>
) -> carinho() -> bombom {
    nova alias = objeto;
    nova alias2 = alias;
    nova copia = alias2;
    mimo carinho() -> bombom {
        mimo copia.medir() + delta + fabrica(3).medir();
    };
}
carinho aninhar(objeto: trato<Medivel>)
    -> carinho() -> carinho() -> bombom {
    mimo carinho() -> carinho() -> bombom {
        mimo carinho() -> bombom { mimo objeto.medir(); };
    };
}
carinho devolver(objeto: trato<Medivel>)
    -> carinho() -> trato<Medivel> {
    mimo carinho() -> trato<Medivel> { mimo objeto; };
}
carinho sombrear(objeto: trato<Medivel>)
    -> carinho(trato<Medivel>) -> bombom {
    mimo carinho(objeto: trato<Medivel>) -> bombom {
        mimo objeto.medir();
    };
}
carinho principal() -> bombom {
    nova muda origem: bombom = 5;
    nova objeto: trato<Medivel> = origem virar trato<Medivel>;
    nova copia = objeto;
    nova executar: carinho() -> bombom = combinar(copia, 2, criar);
    origem = 99;
    nova externa: carinho() -> carinho() -> bombom = aninhar(copia);
    nova interna: carinho() -> bombom = externa();
    nova retorno: carinho() -> trato<Medivel> = devolver(copia);
    nova sombra: carinho(trato<Medivel>) -> bombom = sombrear(copia);
    nova outro: trato<Medivel> = 11 virar trato<Medivel>;
    mimo executar() + interna() + retorno().medir() + sombra(outro);
}
"#;
    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(31)));
}

#[test]
fn fase244_reatribuicao_condicional_de_callable_executa_todos_os_casos_validos() {
    let code = r#"
pacote main;
trato Medivel { carinho medir(valor: si) -> bombom; }
impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}
carinho um() -> trato<Medivel> { mimo 1 virar trato<Medivel>; }
carinho dois() -> trato<Medivel> { mimo 2 virar trato<Medivel>; }
carinho numero_um() -> bombom { mimo 10; }
carinho numero_dois() -> bombom { mimo 20; }
carinho principal() -> bombom {
    nova inferido_um = um;
    nova inferido_dois = dois;
    nova copia_um = inferido_um;
    nova copia_dois = inferido_dois;
    nova muda f = um;
    f = verdade ? inferido_um : inferido_dois;
    nova r1 = f().medir();
    f = falso ? copia_um : copia_dois;
    nova r2 = f().medir();
    f = verdade ? (falso ? um : dois) : um;
    nova r3 = f().medir();
    f = falso ? um : dois;
    f = verdade ? um : dois;
    nova r4 = f().medir();
    nova muda comum: carinho() -> bombom = numero_um;
    comum = falso ? numero_um : numero_dois;
    mimo r1 + r2 + r3 + r4 + comum();
}
"#;

    let result = run_code(code).unwrap();
    assert_eq!(result, Some(RuntimeValue::Int(26)));
}

// @pinker-nav:end evidencia.interpreter.objetos-trato-fase244
