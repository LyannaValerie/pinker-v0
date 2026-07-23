use pinker_v0::agent::{
    contract_digest, contract_v1_json, parse_spec_text, CommandKind, CONTRACT_CHECK_KINDS,
    CONTRACT_ID, CONTRACT_MIN_HUMAN_CHARS, CONTRACT_MIN_SECTION_CHARS, CONTRACT_PUBLICATION_STATES,
    CONTRACT_REQUIRED_BODY_SECTIONS, CONTRACT_SPEC_SCHEMA, CONTRACT_SUBCOMMANDS,
    CONTRACT_TERMINAL_STATES, CONTRACT_VERSION,
};
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
fn cli_publica_subcomandos_v1c() {
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
        "publicar",
        "retomar",
        "status",
        "relatorio",
    ] {
        assert!(help.contains(name), "{name}: {help}");
    }
}

fn publication() -> &'static str {
    "publication.repository = LyannaValerie/pinker-v0\n\
     publication.remote = origin\n\
     publication.base_branch = main\n\
     publication.expected_base = abc\n\
     publication.head_branch = agents/test\n\
     publication.commit_message = feat: test\n\
     publication.change = src/agent.rs\n\
     publication.pr_title = feat: test\n\
     publication.pr_body = entradas/pr-body.md\n\
     publication.draft = false\n\
     publication.required_check = rust\n\
     publication.defer_checks = true\n\
     publication.poll_seconds = 1\n\
     publication.timeout_seconds = 10\n"
}

#[test]
fn spec_v1c_modela_pr_body_e_publicacao() {
    let extra = format!(
        "check.body.kind = pr-body\n\
         check.body.path = entradas/pr-body.md\n\
         check.body.validation_pr_number = 999\n\
         check.body.expected_kind = parallel-phase\n\
         check.body.expected_title = T\n\
         check.body.expected_area = development.agent\n\
         check.body.expected_validation = make ci\n\
         check.body.forbid_sentinel = true\n{}",
        publication()
    );
    let spec = parse_spec_text(&valid_spec(&extra)).expect("V1-C");
    assert!(spec.publication.is_some());
    assert_eq!(spec.checks.len(), 1);
}

#[test]
fn publication_rejeita_repository_e_draft() {
    let bad_repo = publication().replace("LyannaValerie/pinker-v0", "outra/repo");
    assert!(parse_spec_text(&valid_spec(&bad_repo)).is_err());
    let draft = publication().replace("publication.draft = false", "publication.draft = true");
    assert!(parse_spec_text(&valid_spec(&draft)).is_err());
}

#[test]
fn publication_rejeita_campo_desconhecido_e_duplicado() {
    assert!(parse_spec_text(&valid_spec(&format!(
        "{}publication.banana = x\n",
        publication()
    )))
    .is_err());
    assert!(parse_spec_text(&valid_spec(&format!(
        "{}publication.remote = upstream\n",
        publication()
    )))
    .unwrap_err()
    .contains("duplicado"));
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
#[test]
fn contrato_v1_serializacao_e_byte_estavel() {
    let a = contract_v1_json();
    let b = contract_v1_json();
    assert_eq!(a, b, "contrato deve ser byte a byte estável");
    assert_eq!(contract_digest(), contract_digest());
    assert_eq!(contract_digest().len(), 64);
    assert!(a.starts_with("{\n  \"schema\": 1,\n"));
    assert!(a.ends_with("}\n"));
}

#[test]
fn contrato_identidade_e_schema_exatos() {
    assert_eq!(CONTRACT_ID, "pink-agent-v1");
    assert_eq!(CONTRACT_VERSION, 1);
    assert_eq!(CONTRACT_SPEC_SCHEMA, 1);
    let json = contract_v1_json();
    assert!(json.contains("\"contract_id\": \"pink-agent-v1\""));
    assert!(json.contains("\"contract_version\": 1,"));
    assert!(json.contains("\"spec_schema\": 1,"));
}

#[test]
fn contrato_subcomandos_exatos_e_ordenados() {
    assert_eq!(
        CONTRACT_SUBCOMMANDS,
        [
            "iniciar",
            "executar",
            "verificar",
            "sensibilidade",
            "publicar",
            "retomar",
            "status",
            "relatorio"
        ]
    );
}

#[test]
fn contrato_check_kinds_exatos_e_ordenados() {
    assert_eq!(
        CONTRACT_CHECK_KINDS,
        ["git", "marker-only", "projection", "pr-body"]
    );
}

#[test]
fn contrato_estados_terminais_exatos() {
    assert_eq!(
        CONTRACT_TERMINAL_STATES,
        ["ACCEPTED", "BLOCKED", "NEEDS_HUMAN_DECISION"]
    );
}

#[test]
fn contrato_estados_publicacao_exatos_e_ordenados() {
    assert_eq!(
        CONTRACT_PUBLICATION_STATES,
        [
            "LOCAL_ACCEPTED",
            "COMMIT_INTENT",
            "COMMITTED",
            "PUSH_INTENT",
            "PUSHED",
            "PR_INTENT",
            "PR_CREATED",
            "BODY_VERIFIED",
            "CHECKS_PENDING",
            "ACCEPTED",
            "BLOCKED",
            "NEEDS_HUMAN_DECISION"
        ]
    );
}

#[test]
fn contrato_codigos_de_saida_exatos() {
    let json = contract_v1_json();
    assert!(json.contains(
        "\"exit_codes\": {\"ACCEPTED\": 0, \"BLOCKED\": 1, \"NEEDS_HUMAN_DECISION\": 2}"
    ));
}

#[test]
fn contrato_proibicoes_sao_todas_declaracoes_false() {
    let json = contract_v1_json();
    for prohibition in [
        "\"merge\": false",
        "\"auto_merge\": false",
        "\"force_push\": false",
        "\"workflow_rerun\": false",
        "\"remote_body_edit\": false",
    ] {
        assert!(
            json.contains(prohibition),
            "proibição ausente: {prohibition}"
        );
    }
    assert!(!json.contains("true"), "nenhuma capacidade pode ser true");
}

#[test]
fn contrato_secoes_humanas_e_limites_exatos() {
    assert_eq!(
        CONTRACT_REQUIRED_BODY_SECTIONS,
        [
            "Resumo",
            "Problema",
            "Implementação",
            "Validação",
            "Limitações",
            "Próximo passo"
        ]
    );
    assert_eq!(CONTRACT_MIN_SECTION_CHARS, 40);
    assert_eq!(CONTRACT_MIN_HUMAN_CHARS, 400);
    let json = contract_v1_json();
    assert!(json.contains("\"minimum_section_characters\": 40,"));
    assert!(json.contains("\"minimum_human_characters\": 400,"));
}

#[test]
fn contrato_schema_zero_e_dois_sao_rejeitados() {
    let zero = valid_spec("").replacen("schema = 1", "schema = 0", 1);
    assert!(parse_spec_text(&zero).is_err());
    let dois = valid_spec("").replacen("schema = 1", "schema = 2", 1);
    assert!(parse_spec_text(&dois)
        .unwrap_err()
        .contains("schema não suportado"));
}

#[test]
fn contrato_required_check_duplicado_e_rejeitado() {
    let extra = format!("{}publication.required_check = rust\n", publication());
    assert!(parse_spec_text(&valid_spec(&extra))
        .unwrap_err()
        .contains("duplicado"));
}
// @pinker-nav:end evidencia.agent.cli-spec
