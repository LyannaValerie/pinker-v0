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
fn cli_publica_subcomandos_v1b() {
    let pink = env!("CARGO_BIN_EXE_pink");
    let output = Command::new(pink)
        .args(["agente", "--help"])
        .output()
        .expect("pink");
    let help = String::from_utf8_lossy(&output.stderr);
    for name in [
        "iniciar",
        "executar",
        "verificar",
        "sensibilidade",
        "status",
        "relatorio",
    ] {
        assert!(help.contains(name), "{name}: {help}");
    }
}

#[test]
fn spec_v1b_modela_checks_ordenados_e_mutacao() {
    let extra = "check.g.kind = git\ncheck.g.expected_change = src/a.rs\ncheck.m.kind = marker-only\ncheck.m.path = tests/a.rs\ncheck.m.base_sha256 = abc\ncheck.m.expected_regions = 1\ncheck.m.expected_marker_lines = 5\ncheck.m.expected_key = evidencia.a\ncheck.p.kind = projection\ncheck.p.catalog = src/navigation.jsonl\ncheck.p.expected_total = 1\ncheck.p.expected_evidence = 0\ncheck.p.expected_runtime = 0\ncheck.p.expected_length = 1\ncheck.p.expected_fnv1a64 = 00\nmutation.x.target = src/a.rs\nmutation.x.search_file = entradas/mutations/x.before\nmutation.x.replacement_file = entradas/mutations/x.after\nmutation.x.expected_matches = 1\nmutation.x.probe_program = false\nmutation.x.probe_cwd = .\nmutation.x.probe_expected_exit = 1\n";
    let spec = parse_spec_text(&valid_spec(extra)).expect("V1-B");
    assert_eq!(spec.checks.len(), 3);
    assert_eq!(spec.mutations.len(), 1);
}

#[test]
fn check_com_id_invalido_e_rejeitado() {
    assert!(parse_spec_text(&valid_spec("check.bad!.kind = git\n"))
        .unwrap_err()
        .contains("inválido"));
}

#[test]
fn check_com_campo_duplicado_e_rejeitado() {
    assert!(
        parse_spec_text(&valid_spec("check.g.kind = git\ncheck.g.kind = git\n"))
            .unwrap_err()
            .contains("duplicado")
    );
}

#[test]
fn check_com_campo_desconhecido_e_rejeitado() {
    assert!(
        parse_spec_text(&valid_spec("check.g.kind = git\ncheck.g.banana = x\n"))
            .unwrap_err()
            .contains("desconhecido")
    );
}

#[test]
fn projection_sem_referencia_obrigatoria_e_rejeitada() {
    assert!(parse_spec_text(&valid_spec("check.p.kind = projection\n"))
        .unwrap_err()
        .contains("ausente"));
}

#[test]
fn override_duplicado_e_rejeitado() {
    let extra = "check.p.kind = projection\ncheck.p.catalog = src/navigation.jsonl\ncheck.p.expected_total = 1\ncheck.p.expected_evidence = 0\ncheck.p.expected_runtime = 0\ncheck.p.expected_length = 1\ncheck.p.expected_fnv1a64 = 00\ncheck.p.override_hash.a = fnv1a64:1\ncheck.p.override_hash.a = fnv1a64:2\n";
    assert!(parse_spec_text(&valid_spec(extra))
        .unwrap_err()
        .contains("duplicado"));
}
// @pinker-nav:end evidencia.agent.cli-spec
