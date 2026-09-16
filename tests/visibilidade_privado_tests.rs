//! Marcador de visibilidade `privado` — sonda instrumentada da Trama Pinker.
//!
//! Esta suíte não fecha item de roadmap: ela existe para que a sonda tenha
//! oráculo próprio. O contrato exercido é o da superfície modular, e só ele:
//! `privado` retira o item da superfície que `trazer` enxerga sem retirá-lo da
//! unidade que o declarou.

use std::process::Command;

fn pink(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .output()
        .expect("executar pink")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).to_string()
}

fn stdout(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stdout).to_string()
}

// @pinker-nav:start evidencia.visibilidade.superficie-modular
// @pinker-nav:domain visibilidade
// @pinker-nav:layer evidencia
// @pinker-nav:summary Sonda da Trama: exercita o marcador `privado` de topo nas quatro arestas que o definem — o item privado continua chamável dentro do próprio módulo e o público que depende dele atravessa a fronteira; o import seletivo do privado é recusado com diagnóstico próprio, distinto de símbolo inexistente; `trazer <modulo>;` não liga o nome privado no importador; e o marcador sobre item que não é `carinho` de topo é recusado no parser com o construto na mensagem. A paridade interpretador x nativo do caso válido é verificada no mesmo corpo.
#[test]
fn privado_permanece_chamavel_dentro_do_proprio_modulo() {
    let saida = pink(&["--run", "examples/visibilidade_privado_valido.pink"]);
    assert!(saida.status.success(), "stderr: {}", stderr(&saida));
    // 20 dobrado pelo auxiliar privado, mais 1 somado pela função pública.
    assert_eq!(stdout(&saida).trim(), "41");
}

#[test]
fn import_seletivo_de_privado_recusa_com_diagnostico_proprio() {
    let saida = pink(&["--check", "examples/visibilidade_privado_invalido.pink"]);
    assert!(!saida.status.success());
    let msg = stderr(&saida);
    assert!(
        msg.contains("é privado no módulo"),
        "diagnóstico genérico demais: {msg}"
    );
    // A causa precisa ser distinguível de símbolo inexistente, senão o leitor
    // procura o erro no lugar errado.
    assert!(
        !msg.contains("não encontrado no módulo"),
        "privado foi reportado como ausente: {msg}"
    );
}

#[test]
fn simbolo_ausente_continua_com_o_diagnostico_historico() {
    let saida = pink(&[
        "--check",
        "examples/visibilidade_privado_ausente_invalido.pink",
    ]);
    assert!(!saida.status.success());
    assert!(
        stderr(&saida).contains("não encontrado no módulo"),
        "stderr: {}",
        stderr(&saida)
    );
}

#[test]
fn import_de_modulo_inteiro_nao_liga_o_nome_privado() {
    let saida = pink(&[
        "--check",
        "examples/visibilidade_privado_modulo_inteiro_invalido.pink",
    ]);
    assert!(!saida.status.success());
    assert!(
        stderr(&saida).contains("'dobrar' não declarada"),
        "stderr: {}",
        stderr(&saida)
    );
}

#[test]
fn marcador_fora_de_carinho_de_topo_e_recusado() {
    let saida = pink(&[
        "--check",
        "examples/visibilidade_privado_nao_funcao_invalido.pink",
    ]);
    assert!(!saida.status.success());
    let msg = stderr(&saida);
    assert!(msg.contains("`carinho` após `privado`"), "stderr: {msg}");
    assert!(
        msg.contains("ninho"),
        "o construto recusado não aparece: {msg}"
    );
}

#[test]
fn ausencia_do_marcador_preserva_a_superficie_historica() {
    // Compatibilidade histórica: o ausente é `Publica`. Um exemplo modular
    // anterior a este marcador continua exportando o que sempre exportou.
    // `--check`, e não `--run`: este exemplo devolve o próprio valor medido
    // como código de saída, então sucesso de processo não é o oráculo aqui.
    let saida = pink(&[
        "--check",
        "examples/fase144_modulo_ninho_exportado_valido.pink",
    ]);
    assert!(saida.status.success(), "stderr: {}", stderr(&saida));
}
// @pinker-nav:end evidencia.visibilidade.superficie-modular
