mod common;

use common::{render_backend_text, render_cli_machine_output, render_machine, render_selected};

#[test]
fn machine_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:  ; entrada da função
      vm push_int 0  ; empilha literal inteiro
      term ret  ; retorna o valor atual da pilha
"
    );
}

#[test]
fn machine_if_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:  ; entrada da função
      vm push_bool verdade  ; empilha literal lógico
      term br_true then_0, else_1  ; se a condição for verdadeira, entra no ramo 'talvez'; senão vai para o 'senão'
    then_0:  ; ramo 'verdadeiro' (talvez)
      vm push_int 1  ; empilha literal inteiro
      term ret  ; retorna o valor atual da pilha
    else_1:  ; ramo 'senão'
      vm push_int 0  ; empilha literal inteiro
      term ret  ; retorna o valor atual da pilha
"
    );
}

#[test]
fn machine_chamada_e_call_void() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom {
  log();
  mimo soma(1,2);
}";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func log:
    params []
    locals []
    entry:  ; entrada da função
      term ret_void  ; encerra a função sem retorno
  func soma:
    params x, y
    locals []
    temps  %t0  ; gerados pelo compilador
    entry:  ; entrada da função
      vm load_slot x  ; carrega valor do slot para a pilha
      vm load_slot y  ; carrega valor do slot para a pilha
      vm add  ; soma os dois topos da pilha
      vm store_slot %t0  ; guarda o resultado no temporário %t0
      vm load_slot %t0  ; carrega valor do slot para a pilha
      term ret  ; retorna o valor atual da pilha
  func principal:
    params []
    locals []
    temps  %t0  ; gerados pelo compilador
    entry:  ; entrada da função
      vm call_void log, 0  ; chama log com 0 argumento(s) sem retorno
      vm push_int 1  ; empilha literal inteiro
      vm push_int 2  ; empilha literal inteiro
      vm call soma, 2  ; chama soma com 2 argumento(s) e empilha o retorno
      vm store_slot %t0  ; guarda o resultado no temporário %t0
      vm load_slot %t0  ; carrega valor do slot para a pilha
      term ret  ; retorna o valor atual da pilha
"
    );
}

#[test]
fn machine_unaria_binaria_temporarios() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 1;
  nova y = 2;
  mimo -(x + y);
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("vm add  ; soma os dois topos da pilha\n      vm store_slot %t0"));
    assert!(out.contains("vm neg  ; negação aritmética do topo\n      vm store_slot %t1"));
}

#[test]
fn machine_cli_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_cli_machine_output(code).unwrap();
    assert_eq!(
        out,
        "\
=== MACHINE ===
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:  ; entrada da função
      vm push_int 0  ; empilha literal inteiro
      term ret  ; retorna o valor atual da pilha
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn machine_diferente_de_selected_e_pseudo_asm() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let machine = render_machine(code).unwrap();
    let selected = render_selected(code).unwrap();
    let backend = render_backend_text(code).unwrap();
    assert_ne!(machine, selected);
    assert_ne!(machine, backend);
}

#[test]
fn machine_falha_em_programa_invalido() {
    let program = pinker_v0::abstract_machine::MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![pinker_v0::abstract_machine::MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: std::collections::HashMap::new(),
            blocks: vec![pinker_v0::abstract_machine::MachineBlock {
                label: "entry".to_string(),
                code: vec![],
                terminator: pinker_v0::abstract_machine::MachineTerminator::RetVoid,
            }],
        }],
    };

    let err = pinker_v0::abstract_machine_validate::validate_program(&program).unwrap_err();
    assert!(err.to_string().contains("Erro Validação Máquina Abstrata"));
}

#[test]
fn machine_falha_load_slot_inexistente() {
    let program = pinker_v0::abstract_machine::MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![pinker_v0::abstract_machine::MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: std::collections::HashMap::new(),
            blocks: vec![pinker_v0::abstract_machine::MachineBlock {
                label: "entry".to_string(),
                code: vec![pinker_v0::abstract_machine::MachineInstr::LoadSlot(
                    "%x#0".to_string(),
                )],
                terminator: pinker_v0::abstract_machine::MachineTerminator::Ret,
            }],
        }],
    };

    let err = pinker_v0::abstract_machine_validate::validate_program(&program).unwrap_err();
    assert!(err.to_string().contains("load_slot para slot inexistente"));
}

#[test]
fn machine_bitwise_basico() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova a = 6;
  nova b = 3;
  mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1);
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("vm bitand"));
    assert!(out.contains("vm bitor"));
    assert!(out.contains("vm bitxor"));
    assert!(out.contains("vm shl"));
    assert!(out.contains("vm shr"));
}

// ── Fase 35: testes de legibilidade da saída --machine ────────────────────────

#[test]
fn machine_params_exibem_nomes_limpos_sem_prefixo_interno() {
    // Params do usuário devem aparecer como `x, y` e não como `%x#0, %y#0`
    let code = "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("params x, y"),
        "params devem mostrar nomes limpos"
    );
    assert!(
        !out.contains("params %x#0"),
        "prefixo interno %x#0 não deve aparecer em params"
    );
    assert!(
        !out.contains("params %y#0"),
        "prefixo interno %y#0 não deve aparecer em params"
    );
}

#[test]
fn machine_locals_exibem_nomes_limpos_sem_prefixo_interno() {
    // Locais do usuário devem aparecer como `x` e não como `%x#0`
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 1;
  nova y = 2;
  mimo x + y;
}";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("locals x, y"),
        "locals devem mostrar nomes limpos"
    );
    assert!(
        !out.contains("locals %x#0"),
        "prefixo interno %x#0 não deve aparecer em locals"
    );
}

#[test]
fn machine_temps_listados_separadamente_no_cabecalho() {
    // Temporários internos devem aparecer na linha `temps %tN`
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 1;
  nova y = 2;
  mimo x + y;
}";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("temps  %t0  ; gerados pelo compilador"),
        "temporários internos devem ser listados na seção temps"
    );
}

#[test]
fn machine_instrucoes_de_slot_mostram_nome_limpo_para_variaveis_usuario() {
    // load_slot e store_slot de locais do usuário devem usar nome limpo
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 5;
  mimo x;
}";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("vm store_slot x  ; atualiza a variável local x"),
        "store_slot deve mostrar nome limpo"
    );
    assert!(
        out.contains("vm load_slot x  ; carrega valor do slot para a pilha"),
        "load_slot deve mostrar nome limpo"
    );
    assert!(
        !out.contains("load_slot %x#0"),
        "prefixo interno %x#0 não deve aparecer em load_slot"
    );
}

#[test]
fn machine_temps_mantidos_com_formato_percentual_nas_instrucoes() {
    // Temporários internos devem manter formato %tN nas instruções (visualmente distintos)
    let code = "\
pacote main;
carinho principal() -> bombom { mimo 1 + 2; }";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("vm store_slot %t0  ; guarda o resultado no temporário %t0"),
        "temporários internos devem manter formato %tN"
    );
    assert!(
        out.contains("vm load_slot %t0  ; carrega valor do slot para a pilha"),
        "temporários internos devem manter formato %tN em load"
    );
}

#[test]
fn machine_instrucao_call_tem_descricao_humana() {
    let code = "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }";
    let out = render_machine(code).unwrap();
    assert!(out.contains("vm call soma, 2  ; chama soma com 2 argumento(s) e empilha o retorno"));
}
#[test]
fn machine_instrucao_call_void_tem_descricao_humana() {
    let code = "pacote main;
carinho log() { mimo; }
carinho principal() -> bombom {
  log();
  mimo 0;
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("vm call_void log, 0  ; chama log com 0 argumento(s) sem retorno"));
}

#[test]
fn machine_terminadores_tem_descricoes_humanas() {
    let if_code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let if_out = render_machine(if_code).unwrap();
    assert!(if_out.contains(
        "term br_true then_0, else_1  ; se a condição for verdadeira, entra no ramo 'talvez'; senão vai para o 'senão'"
    ));
    assert!(if_out.contains("term ret  ; retorna o valor atual da pilha"));

    let loop_code = "\
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  sempre que x < 1 { x = x + 1; }
  mimo x;
}";
    let loop_out = render_machine(loop_code).unwrap();
    assert!(loop_out.contains(
        "term br_true loop_0, loop_join_1  ; se a condição do loop continuar verdadeira, entra no corpo; senão sai do loop"
    ));
    assert!(loop_out.contains("term jmp loop_cond_0  ; volta para a condição do loop"));
}

#[test]
fn machine_br_true_logico_tem_contexto_de_curto_circuito() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez (verdade || falso) { mimo 1; } senao { mimo 0; }
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("term br_true logic_short_"));
    assert!(out.contains(
        "; se o valor atual já decide o resultado, segue pelo atalho lógico; senão avalia o lado direito"
    ));
}

#[test]
fn machine_jmp_para_join_e_logic_join_tem_comentario_especifico() {
    let if_code = "\
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  talvez verdade { x = 1; } senao { x = 2; }
  mimo x;
}";
    let if_out = render_machine(if_code).unwrap();
    assert!(if_out.contains("term jmp join_"));
    assert!(if_out.contains("; segue para a convergência dos ramos"));

    let logic_code = "\
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  talvez (verdade && verdade) { x = 1; } senao { x = 2; }
  mimo x;
}";
    let logic_out = render_machine(logic_code).unwrap();
    assert!(logic_out.contains("term jmp logic_join_"));
    assert!(logic_out.contains("; continua após o atalho lógico"));
}

#[test]
fn machine_fluxos_auxiliares_break_continue_tem_comentario_especifico() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  sempre que x < 3 {
    x = x + 1;
    talvez x == 1 { continuar; }
    talvez x == 2 { quebrar; }
  }
  mimo x;
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("loop_continue_cont_"));
    assert!(out.contains("; caminho auxiliar após continuar"));
    assert!(out.contains("loop_break_cont_"));
    assert!(out.contains("; caminho auxiliar após quebrar"));
}

#[test]
fn machine_blocos_tem_anotacao_de_papel() {
    // Blocos conhecidos devem ter anotação de papel como comentário
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("entry:  ; entrada da função"),
        "bloco entry deve ter anotação"
    );
    assert!(
        out.contains("then_0:  ; ramo 'verdadeiro' (talvez)"),
        "bloco then deve ter anotação"
    );
    assert!(
        out.contains("else_1:  ; ramo 'senão'"),
        "bloco else deve ter anotação"
    );
}

#[test]
fn machine_loop_blocos_tem_anotacao_de_papel() {
    // Blocos de loop devem ter anotações explicativas
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  sempre que x < 3 { x = x + 1; }
  mimo x;
}";
    let out = render_machine(code).unwrap();
    assert!(
        out.contains("loop_cond_0:  ; condição do loop (sempre que)"),
        "bloco de condição do loop deve ter anotação"
    );
    assert!(
        out.contains("; corpo do loop"),
        "bloco do corpo do loop deve ter anotação"
    );
    assert!(
        out.contains("; saída do loop"),
        "bloco de saída do loop deve ter anotação"
    );
}
