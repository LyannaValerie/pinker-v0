//! Núcleo comum determinístico e observacional (#385) — contrato puro.
//!
//! Cobre plano imutável e determinístico, serialização canônica com payload
//! hexadecimal coberto pelo digest, limites explícitos, classificação por
//! comparação de bytes, relatórios JSON e Markdown derivados do mesmo modelo, e
//! as fronteiras negativas do recorte: nenhum filesystem, nenhuma escrita,
//! nenhuma rede, nenhum Git e nenhuma dependência de root absoluto.

// A #605 moveu implementação de `src/main.rs` para `src/pink_cli/`. Os
// oráculos abaixo leem o binário inteiro, não um arquivo só (#601, OG-1).
#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;

use pinker_v0::automation::{
    check, json_failure, json_report, markdown_report, Allowlist, ChangeKind, Decision, Failure,
    HarnessCause, Observation, ObservedState, Outcome, PlanBuilder, PolicyCause, RelativePath,
    AUTOMATION_SCHEMA, MAX_PATH_LEN, MAX_PLAN_BYTES, MAX_TARGET_BYTES,
};

// ---------------------------------------------------------------------------
// Fixtures sintéticas
// ---------------------------------------------------------------------------

fn allowlist() -> Allowlist {
    Allowlist::new(&["docs/a.md", "docs/b.md", "docs/c.md"]).expect("allowlist válida")
}

/// Plano de referência: um target com conteúdo e um target a remover.
fn plano() -> pinker_v0::automation::Plan {
    PlanBuilder::new("adaptador-de-teste", allowlist())
        .desire("docs/a.md", b"conteudo-desejado-de-a\n".to_vec())
        .unwrap()
        .desire("docs/b.md", b"conteudo-desejado-de-b\n".to_vec())
        .unwrap()
        .remove("docs/c.md")
        .unwrap()
        .build()
        .expect("plano válido")
}

/// Observação que faz o plano coincidir por inteiro.
fn observado_convergente() -> ObservedState {
    ObservedState::new()
        .with(Observation::present("docs/a.md", b"conteudo-desejado-de-a\n".to_vec()).unwrap())
        .unwrap()
        .with(Observation::present("docs/b.md", b"conteudo-desejado-de-b\n".to_vec()).unwrap())
        .unwrap()
        .with(Observation::absent("docs/c.md").unwrap())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Plano: determinismo, ordem e forma canônica
// ---------------------------------------------------------------------------

#[test]
fn plano_e_deterministico() {
    assert_eq!(plano(), plano());
    assert_eq!(plano().to_canonical_json(), plano().to_canonical_json());
    assert_eq!(plano().digest(), plano().digest());
}

#[test]
fn ordem_de_declaracao_nao_altera_o_plano() {
    let direto = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"um".to_vec())
        .unwrap()
        .desire("docs/b.md", b"dois".to_vec())
        .unwrap()
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();
    let invertido = PlanBuilder::new("p", allowlist())
        .remove("docs/c.md")
        .unwrap()
        .desire("docs/b.md", b"dois".to_vec())
        .unwrap()
        .desire("docs/a.md", b"um".to_vec())
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(direto, invertido);
    assert_eq!(direto.to_canonical_json(), invertido.to_canonical_json());
    assert_eq!(direto.digest(), invertido.digest());
}

#[test]
fn targets_ficam_em_ordem_canonica() {
    let plano = plano();
    let caminhos: Vec<&str> = plano.targets().iter().map(|t| t.path().as_str()).collect();
    let mut esperado = caminhos.clone();
    esperado.sort_unstable();
    assert_eq!(caminhos, esperado);
}

#[test]
fn payload_e_hexadecimal_minusculo_na_forma_canonica() {
    let plano = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", vec![0x00, 0x0f, 0xab, 0xff])
        .unwrap()
        .build()
        .unwrap();
    let json = plano.to_canonical_json();
    assert!(json.contains("\"desired\":\"000fabff\""), "{json}");
    assert!(!json.contains("AB"), "hexadecimal precisa ser minúsculo");
}

#[test]
fn remocao_e_representada_por_null() {
    let plano = PlanBuilder::new("p", allowlist())
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();
    assert!(plano.to_canonical_json().contains("\"desired\":null"));
    assert_eq!(plano.targets()[0].desired_bytes(), None);
}

#[test]
fn forma_canonica_nao_contem_root_absoluto() {
    let json = plano().to_canonical_json();
    for proibido in ["/home/", "/tmp/", "/pinker/", "/var/", "C:\\"] {
        assert!(!json.contains(proibido), "root absoluto na forma canônica");
    }
    assert!(!json.contains('\n'), "a forma canônica é de uma linha");
}

#[test]
fn planos_de_roots_absolutos_diferentes_tem_a_mesma_forma_canonica() {
    // O núcleo só conhece paths repo-relativos. Simula o mesmo repositório
    // materializado em dois roots absolutos distintos: o adaptador relativiza
    // antes de declarar, e as duas formas canônicas precisam coincidir.
    fn construir(root: &str, absolutos: &[(&str, &[u8])]) -> String {
        let mut builder = PlanBuilder::new("adaptador", allowlist());
        for (absoluto, bytes) in absolutos {
            let relativo = absoluto
                .strip_prefix(root)
                .expect("path pertence ao root")
                .trim_start_matches('/');
            builder = builder.desire(relativo, bytes.to_vec()).unwrap();
        }
        builder.build().unwrap().to_canonical_json()
    }

    let um = construir(
        "/var/tmp/clone-a",
        &[
            ("/var/tmp/clone-a/docs/a.md", b"x"),
            ("/var/tmp/clone-a/docs/b.md", b"y"),
        ],
    );
    let outro = construir(
        "/var/tmp/outro-root-bem-mais-longo",
        &[
            ("/var/tmp/outro-root-bem-mais-longo/docs/a.md", b"x"),
            ("/var/tmp/outro-root-bem-mais-longo/docs/b.md", b"y"),
        ],
    );
    assert_eq!(um, outro);
}

// ---------------------------------------------------------------------------
// Digest
// ---------------------------------------------------------------------------

#[test]
fn digest_cobre_o_payload() {
    let base = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"conteudo".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let um_byte_diferente = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"conteudp".to_vec())
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(base.digest(), um_byte_diferente.digest());
}

#[test]
fn sensibilidade_do_digest_a_cada_campo() {
    let referencia = plano().digest();

    let outro_produtor = PlanBuilder::new("outro-adaptador", allowlist())
        .desire("docs/a.md", b"conteudo-desejado-de-a\n".to_vec())
        .unwrap()
        .desire("docs/b.md", b"conteudo-desejado-de-b\n".to_vec())
        .unwrap()
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(
        referencia,
        outro_produtor.digest(),
        "produtor não entra no digest"
    );

    let outro_path = PlanBuilder::new("adaptador-de-teste", allowlist())
        .desire("docs/a.md", b"conteudo-desejado-de-a\n".to_vec())
        .unwrap()
        .desire("docs/c.md", b"conteudo-desejado-de-b\n".to_vec())
        .unwrap()
        .remove("docs/b.md")
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(referencia, outro_path.digest(), "path não entra no digest");

    let remocao_virou_conteudo = PlanBuilder::new("adaptador-de-teste", allowlist())
        .desire("docs/a.md", b"conteudo-desejado-de-a\n".to_vec())
        .unwrap()
        .desire("docs/b.md", b"conteudo-desejado-de-b\n".to_vec())
        .unwrap()
        .desire("docs/c.md", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(
        referencia,
        remocao_virou_conteudo.digest(),
        "remoção e conteúdo vazio precisam ser distinguíveis"
    );
}

#[test]
fn digest_e_sha256_da_forma_canonica() {
    let plano = plano();
    assert_eq!(
        plano.digest(),
        pinker_sha256_contract::sha256_hex(plano.to_canonical_json().as_bytes())
    );
    assert_eq!(plano.digest().len(), 64);
}

// ---------------------------------------------------------------------------
// Limites explícitos
// ---------------------------------------------------------------------------

#[test]
fn limites_sao_constantes_explicitas() {
    assert_eq!(MAX_TARGET_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_PLAN_BYTES, 32 * 1024 * 1024);
    assert_eq!(MAX_PATH_LEN, 512);
    assert_eq!(AUTOMATION_SCHEMA, 1);
}

#[test]
fn limite_por_target_aceita_o_limite_e_rejeita_um_byte_a_mais() {
    let no_limite = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", vec![b'x'; MAX_TARGET_BYTES])
        .expect("exatamente no limite é aceito")
        .build();
    assert!(no_limite.is_ok());

    let excedido =
        PlanBuilder::new("p", allowlist()).desire("docs/a.md", vec![b'x'; MAX_TARGET_BYTES + 1]);
    match excedido {
        Err(Failure::PolicyViolation(PolicyCause::TargetLimitExceeded {
            bytes, limit, ..
        })) => {
            assert_eq!(bytes, MAX_TARGET_BYTES + 1);
            assert_eq!(limit, MAX_TARGET_BYTES);
        }
        other => panic!("esperado limite por target, veio {other:?}"),
    }
}

#[test]
fn limite_por_plano_aceita_o_limite_e_rejeita_um_byte_a_mais() {
    let allow = Allowlist::new(&["t0", "t1", "t2", "t3", "t4"]).unwrap();

    let mut builder = PlanBuilder::new("p", allow.clone());
    for i in 0..4 {
        builder = builder
            .desire(&format!("t{i}"), vec![b'x'; MAX_TARGET_BYTES])
            .unwrap();
    }
    let no_limite = builder.build();
    assert!(
        no_limite.is_ok(),
        "quatro targets somam exatamente o limite"
    );
    assert_eq!(no_limite.unwrap().decoded_bytes(), MAX_PLAN_BYTES);

    let mut builder = PlanBuilder::new("p", allow);
    for i in 0..4 {
        builder = builder
            .desire(&format!("t{i}"), vec![b'x'; MAX_TARGET_BYTES])
            .unwrap();
    }
    builder = builder.desire("t4", vec![b'x']).unwrap();
    match builder.build() {
        Err(Failure::PolicyViolation(PolicyCause::PlanLimitExceeded { bytes, limit })) => {
            assert_eq!(bytes, MAX_PLAN_BYTES + 1);
            assert_eq!(limit, MAX_PLAN_BYTES);
        }
        other => panic!("esperado limite por plano, veio {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Política lexical de paths e allowlist
// ---------------------------------------------------------------------------

#[test]
fn path_lexicalmente_invalido_e_rejeitado() {
    assert_eq!(RelativePath::new(""), Err(PolicyCause::PathEmpty));
    assert!(matches!(
        RelativePath::new("/docs/a.md"),
        Err(PolicyCause::PathAbsolute { .. })
    ));
    assert!(matches!(
        RelativePath::new("../docs/a.md"),
        Err(PolicyCause::PathTraversal { .. })
    ));
    assert!(matches!(
        RelativePath::new("docs/../a.md"),
        Err(PolicyCause::PathTraversal { .. })
    ));
    assert!(matches!(
        RelativePath::new("docs//a.md"),
        Err(PolicyCause::PathDegenerateComponent { .. })
    ));
    assert!(matches!(
        RelativePath::new("docs\\a.md"),
        Err(PolicyCause::PathBackslash { .. })
    ));
    assert!(matches!(
        RelativePath::new("docs/\u{1}.md"),
        Err(PolicyCause::PathControlChar { .. })
    ));
}

#[test]
fn path_com_ponto_isolado_e_rejeitado() {
    assert!(matches!(
        RelativePath::new("docs/./a.md"),
        Err(PolicyCause::PathDegenerateComponent { .. })
    ));
}

#[test]
fn path_longo_demais_e_rejeitado() {
    let longo = format!("docs/{}", "a".repeat(MAX_PATH_LEN));
    assert!(matches!(
        RelativePath::new(&longo),
        Err(PolicyCause::PathTooLong { .. })
    ));
}

#[test]
fn target_fora_da_allowlist_e_rejeitado() {
    match PlanBuilder::new("p", allowlist()).desire("docs/z.md", b"x".to_vec()) {
        Err(Failure::PolicyViolation(PolicyCause::TargetNotAllowed { path })) => {
            assert_eq!(path, "docs/z.md");
        }
        other => panic!("esperado target fora da allowlist, veio {other:?}"),
    }
}

#[test]
fn allowlist_e_canonica_e_independe_da_ordem() {
    let a = Allowlist::new(&["b.md", "a.md", "a.md"]).unwrap();
    let b = Allowlist::new(&["a.md", "b.md"]).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.entries().len(), 2);
}

// ---------------------------------------------------------------------------
// Falhas de harness, separadas de drift
// ---------------------------------------------------------------------------

#[test]
fn schema_desconhecido_e_falha_de_harness() {
    match PlanBuilder::new("p", allowlist()).with_schema(2).build() {
        Err(Failure::HarnessFailure(HarnessCause::SchemaUnknown { found })) => {
            assert_eq!(found, 2);
        }
        other => panic!("esperado schema desconhecido, veio {other:?}"),
    }
}

#[test]
fn produtor_ausente_e_falha_de_harness() {
    assert!(matches!(
        PlanBuilder::new("   ", allowlist()).build(),
        Err(Failure::HarnessFailure(HarnessCause::ProducerMissing))
    ));
}

#[test]
fn target_duplicado_e_falha_de_harness() {
    let builder = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"um".to_vec())
        .unwrap();
    assert!(matches!(
        builder.desire("docs/a.md", b"dois".to_vec()),
        Err(Failure::HarnessFailure(
            HarnessCause::DuplicateTarget { .. }
        ))
    ));
}

#[test]
fn observacao_duplicada_e_falha_de_harness() {
    let estado = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap();
    assert!(matches!(
        estado.with(Observation::present("docs/a.md", b"x".to_vec()).unwrap()),
        Err(Failure::HarnessFailure(
            HarnessCause::DuplicateObservation { .. }
        ))
    ));
}

#[test]
fn observacao_ausente_e_falha_de_harness_e_nunca_create() {
    // Sem observação não há classificação. Inventar ausência transformaria um
    // erro do chamador em CREATE — exatamente a confusão que o contrato proíbe.
    let parcial = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap()
        .with(Observation::absent("docs/b.md").unwrap())
        .unwrap();
    match check(&plano(), &parcial) {
        Err(Failure::HarnessFailure(HarnessCause::MissingObservation { path })) => {
            assert_eq!(path, "docs/c.md");
        }
        other => panic!("esperada observação ausente, veio {other:?}"),
    }
}

#[test]
fn observacao_sem_target_e_falha_de_harness() {
    let plano = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"x".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let estado = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap()
        .with(Observation::absent("docs/b.md").unwrap())
        .unwrap();
    match check(&plano, &estado) {
        Err(Failure::HarnessFailure(HarnessCause::ObservationWithoutTarget { path })) => {
            assert_eq!(path, "docs/b.md");
        }
        other => panic!("esperada observação órfã, veio {other:?}"),
    }
}

#[test]
fn falha_de_harness_nunca_vira_drift() {
    // O tipo de retorno já separa: `check` devolve Result, então uma falha não
    // pode ocupar o lugar de um outcome. Este teste fixa a garantia.
    let parcial = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap();
    let resultado = check(&plano(), &parcial);
    assert!(resultado.is_err());
    let Err(falha) = resultado else {
        unreachable!("verificado acima")
    };
    assert_eq!(falha.code(), "HARNESS_FAILURE");
    assert!(falha.reachable_by_pure_core());
}

#[test]
fn violacao_de_politica_e_distinta_de_falha_de_harness() {
    let politica = PlanBuilder::new("p", allowlist())
        .desire("docs/z.md", b"x".to_vec())
        .expect_err("target não permitido");
    let harness = PlanBuilder::new("", allowlist())
        .build()
        .expect_err("produtor ausente");
    assert_eq!(politica.code(), "POLICY_VIOLATION");
    assert_eq!(harness.code(), "HARNESS_FAILURE");
    assert_ne!(politica.code(), harness.code());
}

// ---------------------------------------------------------------------------
// Classificação e outcomes
// ---------------------------------------------------------------------------

#[test]
fn classifica_create_replace_remove_e_no_change() {
    let observado = ObservedState::new()
        // desejado presente, observado ausente -> CREATE
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap()
        // desejado presente, observado diferente -> REPLACE
        .with(Observation::present("docs/b.md", b"outro".to_vec()).unwrap())
        .unwrap()
        // desejado ausente, observado presente -> REMOVE
        .with(Observation::present("docs/c.md", b"sobra".to_vec()).unwrap())
        .unwrap();
    let report = check(&plano(), &observado).expect("check válido");
    let por_path = |path: &str| {
        report
            .targets
            .iter()
            .find(|t| t.path == path)
            .expect("target presente")
            .change
    };
    assert_eq!(por_path("docs/a.md"), ChangeKind::Create);
    assert_eq!(por_path("docs/b.md"), ChangeKind::Replace);
    assert_eq!(por_path("docs/c.md"), ChangeKind::Remove);
    assert_eq!(report.outcome, Outcome::Drift);

    let convergente = check(&plano(), &observado_convergente()).expect("check válido");
    assert!(convergente
        .targets
        .iter()
        .all(|t| t.change == ChangeKind::NoChange));
    assert_eq!(convergente.outcome, Outcome::Match);
}

#[test]
fn remocao_de_arquivo_ja_ausente_e_no_change() {
    let plano = PlanBuilder::new("p", allowlist())
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();
    let observado = ObservedState::new()
        .with(Observation::absent("docs/c.md").unwrap())
        .unwrap();
    let report = check(&plano, &observado).unwrap();
    assert_eq!(report.targets[0].change, ChangeKind::NoChange);
    assert_eq!(report.outcome, Outcome::Match);
}

#[test]
fn sensibilidade_da_classificacao_a_um_unico_byte() {
    let plano = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", b"conteudo".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let igual = ObservedState::new()
        .with(Observation::present("docs/a.md", b"conteudo".to_vec()).unwrap())
        .unwrap();
    assert_eq!(check(&plano, &igual).unwrap().outcome, Outcome::Match);

    let um_byte = ObservedState::new()
        .with(Observation::present("docs/a.md", b"conteudp".to_vec()).unwrap())
        .unwrap();
    assert_eq!(check(&plano, &um_byte).unwrap().outcome, Outcome::Drift);

    let um_byte_a_mais = ObservedState::new()
        .with(Observation::present("docs/a.md", b"conteudo\n".to_vec()).unwrap())
        .unwrap();
    assert_eq!(
        check(&plano, &um_byte_a_mais).unwrap().outcome,
        Outcome::Drift
    );
}

#[test]
fn conteudo_vazio_e_distinto_de_ausencia() {
    let plano = PlanBuilder::new("p", allowlist())
        .desire("docs/a.md", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    let ausente = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap();
    let vazio = ObservedState::new()
        .with(Observation::present("docs/a.md", Vec::new()).unwrap())
        .unwrap();
    assert_eq!(
        check(&plano, &ausente).unwrap().targets[0].change,
        ChangeKind::Create
    );
    assert_eq!(
        check(&plano, &vazio).unwrap().targets[0].change,
        ChangeKind::NoChange
    );
}

#[test]
fn o_nucleo_puro_so_produz_match_e_drift() {
    let convergente = check(&plano(), &observado_convergente()).unwrap();
    assert!(convergente.outcome.reachable_by_pure_core());
    let divergente = check(
        &plano(),
        &ObservedState::new()
            .with(Observation::absent("docs/a.md").unwrap())
            .unwrap()
            .with(Observation::absent("docs/b.md").unwrap())
            .unwrap()
            .with(Observation::absent("docs/c.md").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(divergente.outcome.reachable_by_pure_core());
    assert!(!Outcome::Applied.reachable_by_pure_core());
    assert!(!Outcome::NoChange.reachable_by_pure_core());
}

#[test]
fn o_nucleo_puro_nao_produz_decisao_humana() {
    let report = check(&plano(), &observado_convergente()).unwrap();
    assert_eq!(report.decision, None);
}

// ---------------------------------------------------------------------------
// Relatórios
// ---------------------------------------------------------------------------

#[test]
fn json_e_deterministico_e_de_uma_linha() {
    let report = check(&plano(), &observado_convergente()).unwrap();
    let a = json_report(&report);
    let b = json_report(&check(&plano(), &observado_convergente()).unwrap());
    assert_eq!(a, b);
    assert!(!a.contains('\n'));
    assert!(!a.contains('\u{1b}'), "JSON não pode conter ANSI");
}

#[test]
fn json_tem_ordem_de_chaves_fixa() {
    let json = json_report(&check(&plano(), &observado_convergente()).unwrap());
    let ordem = [
        "\"schema\":",
        "\"producer\":",
        "\"plan_digest\":",
        "\"outcome\":",
        "\"targets\":",
        "\"summary\":",
        "\"failure\":",
        "\"decision\":",
    ];
    let mut anterior = 0usize;
    for campo in ordem {
        let posicao = json
            .find(campo)
            .unwrap_or_else(|| panic!("campo ausente: {campo}"));
        assert!(posicao >= anterior, "ordem instável em {campo}");
        anterior = posicao;
    }
}

#[test]
fn relatorios_nao_carregam_o_payload_completo() {
    let report = check(&plano(), &observado_convergente()).unwrap();
    let json = json_report(&report);
    let markdown = markdown_report(&report);
    for proibido in ["conteudo-desejado-de-a", "conteudo-desejado-de-b"] {
        assert!(!json.contains(proibido), "payload vazou no JSON");
        assert!(!markdown.contains(proibido), "payload vazou no Markdown");
    }
    // Nem em hexadecimal.
    let hex = plano().targets()[0].desired().unwrap().to_hex();
    assert!(!json.contains(&hex));
    assert!(!markdown.contains(&hex));
}

#[test]
fn relatorios_nao_carregam_root_absoluto() {
    let report = check(&plano(), &observado_convergente()).unwrap();
    for texto in [json_report(&report), markdown_report(&report)] {
        for proibido in ["/home/", "/tmp/", "/pinker/", "/var/"] {
            assert!(!texto.contains(proibido), "root absoluto no relatório");
        }
    }
}

#[test]
fn markdown_deriva_do_mesmo_modelo_do_json() {
    let report = check(&plano(), &observado_convergente()).unwrap();
    let json = json_report(&report);
    let markdown = markdown_report(&report);
    // Mesmos fatos nos dois: outcome, produtor, digest e todos os paths.
    assert!(json.contains("\"outcome\":\"MATCH\""));
    assert!(markdown.starts_with("# Automação — MATCH\n"));
    assert!(markdown.contains(&report.plan_digest));
    assert!(json.contains(&report.plan_digest));
    for target in &report.targets {
        assert!(json.contains(&format!("\"path\":\"{}\"", target.path)));
        assert!(markdown.contains(&format!("`{}`", target.path)));
    }
    assert_eq!(
        markdown,
        markdown_report(&report),
        "Markdown determinístico"
    );
}

#[test]
fn markdown_acompanha_o_json_quando_ha_drift() {
    let observado = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap()
        .with(Observation::present("docs/b.md", b"outro".to_vec()).unwrap())
        .unwrap()
        .with(Observation::present("docs/c.md", b"sobra".to_vec()).unwrap())
        .unwrap();
    let report = check(&plano(), &observado).unwrap();
    let json = json_report(&report);
    let markdown = markdown_report(&report);
    assert!(json.contains("\"outcome\":\"DRIFT\""));
    assert!(markdown.starts_with("# Automação — DRIFT\n"));
    for kind in ["CREATE", "REPLACE", "REMOVE"] {
        assert!(json.contains(kind), "{kind} ausente do JSON");
        assert!(markdown.contains(kind), "{kind} ausente do Markdown");
    }
    assert!(json.contains("\"create\":1"));
    assert!(markdown.contains("create 1"));
}

#[test]
fn relatorio_de_falha_carrega_causa_e_decisao_lado_a_lado() {
    let falha = Failure::HarnessFailure(HarnessCause::ProducerMissing);
    let json = json_failure("adaptador", &falha, Some(Decision::NeedsHumanDecision));
    assert!(json.contains("\"code\":\"HARNESS_FAILURE\""));
    assert!(json.contains("\"decision\":\"NEEDS_HUMAN_DECISION\""));
    assert!(json.contains("\"outcome\":null"), "falha não vira outcome");
    assert!(!json.contains('\n'));

    let sem_decisao = json_failure("adaptador", &falha, None);
    assert!(sem_decisao.contains("\"decision\":null"));
    assert!(sem_decisao.contains("\"code\":\"HARNESS_FAILURE\""));
}

#[test]
fn json_escapa_caracteres_de_controle() {
    let allow = Allowlist::new(&["docs/a.md"]).unwrap();
    let plano = PlanBuilder::new("produtor \"com\" \\ aspas\te tab", allow)
        .desire("docs/a.md", b"x".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let observado = ObservedState::new()
        .with(Observation::present("docs/a.md", b"x".to_vec()).unwrap())
        .unwrap();
    let json = json_report(&check(&plano, &observado).unwrap());
    assert!(json.contains("\\\"") && json.contains("\\\\") && json.contains("\\t"));
    assert!(!json.contains('\t'), "tab literal não pode vazar");
}

#[test]
fn resumo_conta_cada_classificacao() {
    let observado = ObservedState::new()
        .with(Observation::absent("docs/a.md").unwrap())
        .unwrap()
        .with(Observation::present("docs/b.md", b"conteudo-desejado-de-b\n".to_vec()).unwrap())
        .unwrap()
        .with(Observation::present("docs/c.md", b"sobra".to_vec()).unwrap())
        .unwrap();
    let report = check(&plano(), &observado).unwrap();
    assert_eq!(
        report.summary(),
        [
            ("create", 1),
            ("replace", 0),
            ("remove", 1),
            ("no_change", 1)
        ]
    );
}

// ---------------------------------------------------------------------------
// Fronteiras negativas do recorte
// ---------------------------------------------------------------------------

const FONTES: [(&str, &str); 5] = [
    ("mod.rs", include_str!("../src/automation/mod.rs")),
    ("path.rs", include_str!("../src/automation/path.rs")),
    ("plan.rs", include_str!("../src/automation/plan.rs")),
    ("compare.rs", include_str!("../src/automation/compare.rs")),
    ("report.rs", include_str!("../src/automation/report.rs")),
];

#[test]
fn o_nucleo_nao_toca_o_filesystem() {
    for (nome, fonte) in FONTES {
        for proibido in [
            "std::fs",
            "fs::write",
            "fs::read",
            "File::create",
            "File::open",
            "OpenOptions",
            "create_new",
            "remove_file",
            "std::path::Path",
            "PathBuf",
            "temp_dir",
        ] {
            assert!(
                !fonte.contains(proibido),
                "{nome} tocou filesystem: {proibido}"
            );
        }
    }
}

#[test]
fn o_nucleo_nao_usa_rede_processos_nem_git() {
    for (nome, fonte) in FONTES {
        for proibido in [
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "std::net",
            "Command::new",
            "std::process",
            "gh pr",
            "git rev-parse",
            "https://",
        ] {
            assert!(
                !fonte.contains(proibido),
                "{nome} alcançou o mundo externo: {proibido}"
            );
        }
    }
}

#[test]
fn o_nucleo_nao_depende_de_estado_nao_deterministico() {
    for (nome, fonte) in FONTES {
        for proibido in [
            "SystemTime",
            "Instant::",
            "std::time",
            "HashMap<",
            "HashSet<",
            "std::env",
            "random",
            "as *const",
        ] {
            assert!(
                !fonte.contains(proibido),
                "{nome} admitiu estado instável: {proibido}"
            );
        }
    }
}

#[test]
fn o_nucleo_puro_nao_expoe_apply_root_nem_cli() {
    for (nome, fonte) in FONTES {
        for proibido in [
            "pub fn apply",
            "pub fn aplicar",
            "pub fn discover_root",
            "pub fn repo_root",
            "pub fn write",
            "pub fn rename",
            "rename(",
        ] {
            assert!(
                !fonte.contains(proibido),
                "{nome} expôs superfície de estágio posterior: {proibido}"
            );
        }
    }
    let cli = fonte_de_modulo::pink_cli();
    // O Stage E é o primeiro consumidor real, mas a CLI continua sem montar,
    // observar, autorizar ou aplicar Plan diretamente.
    for proibido in [
        "automation::Plan",
        "automation::observe(",
        "automation::check(",
        "automation::Authorization",
        "automation::apply(",
    ] {
        assert!(
            !cli.contains(proibido),
            "a CLI assumiu responsabilidade do núcleo puro: {proibido}"
        );
    }
}

#[test]
fn o_nucleo_nao_conhece_estados_do_pink_agente() {
    // O núcleo reutiliza contrato SHA-256 compartilhado, sem importar estados
    // organizacionais do agente.
    for (nome, fonte) in FONTES {
        for proibido in [
            "CONTRACT_",
            "EXIT_ACCEPTED",
            "EXIT_BLOCKED",
            "EXIT_NEEDS_HUMAN",
            "agent::verificar",
            "agent::status",
            "CheckState",
        ] {
            assert!(
                !fonte.contains(proibido),
                "{nome} importou estado operacional: {proibido}"
            );
        }
    }
    let usos: usize = FONTES
        .iter()
        .map(|(_, fonte)| fonte.matches("pinker_sha256_contract::sha256_hex").count())
        .sum();
    assert!(
        usos > 0,
        "o digest precisa reutilizar a autoridade de SHA-256"
    );
    assert!(FONTES
        .iter()
        .all(|(_, fonte)| !fonte.contains("crate::pinker_sha256_contract::")));
}

#[test]
fn o_plano_nao_e_versionado_no_repositorio() {
    // O plano é efêmero: não existe diretório de planos e nenhuma fonte do
    // núcleo sabe escrever um.
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidato in [
        ".pinker/plans",
        ".pinker/planos",
        ".pinker/automation",
        ".pinker/automacao",
    ] {
        assert!(
            !raiz.join(candidato).exists(),
            "plano versionado encontrado em {candidato}"
        );
    }
}

#[test]
fn nao_existe_parser_de_plano() {
    // Decisão registrada: o plano nunca é lido de volta, então não há caminho de
    // desserialização — a autorização futura compara digests de planos
    // recalculados.
    for (nome, fonte) in FONTES {
        for proibido in ["pub fn from_json", "pub fn parse", "fn deserialize"] {
            assert!(!fonte.contains(proibido), "{nome} expôs parser: {proibido}");
        }
    }
}
