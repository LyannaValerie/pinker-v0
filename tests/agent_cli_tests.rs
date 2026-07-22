use pinker_v0::agent::{parse_spec_text, CommandKind};
use std::process::Command;

// @pinker-nav:start evidencia.agent.cli-spec
// @pinker-nav:domain development
// @pinker-nav:layer evidencia
// @pinker-nav:summary Contrato público e parsing estrito do agent-spec-v1: superfície CLI, spec válida, schema, campos e comandos duplicados/desconhecidos, shell explícito e comando Pinker tipado.
fn valid_spec(extra: &str) -> String {
    format!(
        "schema = 1\n\
         task_id = TEST\n\
         repo_root = /repo\n\
         worktree = /repo/worktree\n\
         delegated_root = /repo/delegated\n\
         expected_base = abc\n\
         allowed_write = artefatos\n\
         allowed_change = src/agent.rs\n\
         verdict.accepted = ACCEPTED_TEST\n\
         verdict.blocked = BLOCKED_TEST\n\
         verdict.human = HUMAN_TEST\n\
         command.one.kind = program\n\
         command.one.program = printf\n\
         command.one.arg = ok\n\
         command.one.cwd = .\n\
         command.one.expect = 0\n\
         command.one.shell = false\n{extra}"
    )
}

#[test]
fn spec_valida_e_ordenada() {
    let spec = parse_spec_text(&valid_spec("")).expect("spec válida");
    assert_eq!(spec.schema, 1);
    assert_eq!(spec.commands.len(), 1);
    assert_eq!(spec.commands[0].argv, ["ok"]);
}

#[test]
fn schema_invalido_e_rejeitado() {
    let text = valid_spec("").replacen("schema = 1", "schema = 2", 1);
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("schema não suportado"));
}

#[test]
fn campo_desconhecido_e_rejeitado() {
    assert!(parse_spec_text(&valid_spec("misterio = x\n"))
        .unwrap_err()
        .contains("campo desconhecido"));
}

#[test]
fn campo_escalar_duplicado_e_rejeitado() {
    assert!(parse_spec_text(&valid_spec("task_id = OUTRO\n"))
        .unwrap_err()
        .contains("duplicado"));
}

#[test]
fn propriedade_de_comando_duplicada_e_rejeitada() {
    assert!(parse_spec_text(&valid_spec("command.one.program = echo\n"))
        .unwrap_err()
        .contains("duplicado"));
}

#[test]
fn shell_implicito_e_rejeitado() {
    let text = valid_spec("").replace(
        "command.one.program = printf",
        "command.one.program = /bin/sh",
    );
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("shell implícito"));
}

#[test]
fn shell_explicito_e_registrado() {
    let text = valid_spec("")
        .replace(
            "command.one.program = printf",
            "command.one.program = printf ok",
        )
        .replace("command.one.shell = false", "command.one.shell = true");
    assert!(parse_spec_text(&text).expect("shell explícito").commands[0].shell);
}

#[test]
fn pinker_tipado_exige_programa_pink() {
    let text = valid_spec("").replace("command.one.kind = program", "command.one.kind = pinker");
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("exige programa pink"));
}

#[test]
fn pinker_tipado_valido_e_modelado() {
    let text = valid_spec("")
        .replace("command.one.kind = program", "command.one.kind = pinker")
        .replace("command.one.program = printf", "command.one.program = pink");
    assert_eq!(
        parse_spec_text(&text).expect("pinker").commands[0].kind,
        CommandKind::Pinker
    );
}

#[test]
fn retomada_e_explicitamente_rejeitada_pelo_schema_a() {
    assert!(parse_spec_text(&valid_spec("resume = true\n"))
        .unwrap_err()
        .contains("campo desconhecido"));
}

#[test]
fn cli_publica_cinco_subcomandos() {
    let pink = env!("CARGO_BIN_EXE_pink");
    let output = Command::new(pink)
        .args(["agente", "--help"])
        .output()
        .expect("pink");
    let help = String::from_utf8_lossy(&output.stderr);
    for name in ["iniciar", "executar", "verificar", "status", "relatorio"] {
        assert!(help.contains(name), "{name}: {help}");
    }
}
// @pinker-nav:end evidencia.agent.cli-spec
