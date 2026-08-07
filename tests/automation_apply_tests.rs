//! Núcleo de automação (#385) — root canônico, confinamento e apply local.
//!
//! Cobre descoberta canônica da raiz, confinamento no filesystem, check
//! estritamente sem escrita, autorização por digest exato, detecção de plano
//! obsoleto, atomicidade por arquivo, progresso parcial explícito e a ausência
//! declarada de rollback global.
//!
//! Cada teste monta um repositório sintético sob `TMPDIR` e o remove ao fim.

use pinker_v0::automation::{
    apply, check, confine, json_apply_report, markdown_apply_report, observe, observe_target,
    verify_written, Allowlist, Authorization, Decision, Failure, FinalDrift, HarnessCause, Outcome,
    Plan, PlanBuilder, PolicyCause, RelativePath, RepoRoot, ROOT_MARKER,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Repositório sintético
// ---------------------------------------------------------------------------

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

/// Cria um repositório sintético com o marcador canônico e `docs/`.
fn repo(nome: &str) -> PathBuf {
    let agora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = SEQUENCIA.fetch_add(1, Ordering::SeqCst);
    let base = std::env::temp_dir().join(format!("pinker_apply_{nome}_{agora}_{seq}"));
    fs::create_dir_all(base.join(".pinker")).unwrap();
    fs::write(base.join(ROOT_MARKER), "schema = 1\n").unwrap();
    fs::create_dir_all(base.join("docs")).unwrap();
    base
}

fn limpar(base: &Path) {
    // Restaura permissões antes de remover: alguns testes tornam diretórios
    // somente leitura de propósito.
    if let Ok(entradas) = fs::read_dir(base.join("docs")) {
        for entrada in entradas.flatten() {
            if entrada.path().is_dir() {
                let _ = fs::set_permissions(entrada.path(), fs::Permissions::from_mode(0o755));
            }
        }
    }
    let _ = fs::set_permissions(base.join("docs"), fs::Permissions::from_mode(0o755));
    let _ = fs::remove_dir_all(base);
}

fn allowlist() -> Allowlist {
    Allowlist::new(&[
        "docs/a.md",
        "docs/b.md",
        "docs/c.md",
        "docs/sub/inner.md",
        "docs/b/inner.md",
    ])
    .expect("allowlist")
}

fn plano_criar(paths: &[(&str, &[u8])]) -> Plan {
    let mut builder = PlanBuilder::new("adaptador-de-teste", allowlist());
    for (path, bytes) in paths {
        builder = builder.desire(path, bytes.to_vec()).unwrap();
    }
    builder.build().unwrap()
}

fn rel(path: &str) -> RelativePath {
    RelativePath::new(path).unwrap()
}

/// Autoriza e aplica um plano com precondição observada agora.
fn aplicar(root: &RepoRoot, plan: &Plan) -> pinker_v0::automation::ApplyReport {
    let observado = observe(root, plan).expect("observação");
    let precondicao = check(plan, &observado).expect("check");
    let autorizacao = Authorization::for_digest(&plan.digest());
    apply(root, plan, &autorizacao, &precondicao)
}

/// Instantâneo (path relativo, bytes) de toda a árvore.
fn instantaneo(base: &Path) -> Vec<(String, Vec<u8>)> {
    fn caminhar(base: &Path, atual: &Path, saida: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entradas) = fs::read_dir(atual) else {
            return;
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            if caminho.is_dir() {
                caminhar(base, &caminho, saida);
            } else {
                let relativo = caminho
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                saida.push((relativo, fs::read(&caminho).unwrap_or_default()));
            }
        }
    }
    let mut saida = Vec::new();
    caminhar(base, base, &mut saida);
    saida.sort();
    saida
}

fn temporarios(base: &Path) -> Vec<String> {
    instantaneo(base)
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| p.contains(".pinker-tmp-"))
        .collect()
}

// ---------------------------------------------------------------------------
// Raiz canônica
// ---------------------------------------------------------------------------

#[test]
fn raiz_e_descoberta_subindo_ate_o_marcador() {
    let base = repo("descoberta");
    let fundo = base.join("docs/sub/mais/fundo");
    fs::create_dir_all(&fundo).unwrap();
    let root = RepoRoot::discover(&fundo).expect("raiz descoberta");
    assert_eq!(root.path(), base.canonicalize().unwrap());
    limpar(&base);
}

#[test]
fn raiz_e_canonica_atraves_de_link_simbolico() {
    let base = repo("canonica");
    let atalho = base.parent().unwrap().join(format!(
        "atalho_{}",
        base.file_name().unwrap().to_string_lossy()
    ));
    std::os::unix::fs::symlink(&base, &atalho).unwrap();
    let por_atalho = RepoRoot::discover(&atalho).expect("raiz pelo atalho");
    let direto = RepoRoot::discover(&base).expect("raiz direta");
    assert_eq!(
        por_atalho, direto,
        "dois caminhos para o mesmo repositório precisam convergir"
    );
    assert_eq!(por_atalho.path(), base.canonicalize().unwrap());
    let _ = fs::remove_file(&atalho);
    limpar(&base);
}

#[test]
fn raiz_sem_marcador_e_falha_de_harness() {
    let base = repo("sem-marcador");
    fs::remove_file(base.join(ROOT_MARKER)).unwrap();
    match RepoRoot::at(&base) {
        Err(Failure::HarnessFailure(HarnessCause::RootNotFound { .. })) => {}
        outro => panic!("esperada raiz não encontrada, veio {outro:?}"),
    }
    limpar(&base);
}

#[test]
fn raiz_declarada_nao_sobe() {
    let base = repo("declarada");
    let sub = base.join("docs");
    // `at` exige o marcador exatamente ali; não sobe como `discover`.
    assert!(RepoRoot::at(&sub).is_err());
    assert!(RepoRoot::discover(&sub).is_ok());
    limpar(&base);
}

#[test]
fn a_mesma_operacao_em_roots_diferentes_produz_o_mesmo_relatorio() {
    let um = repo("root-um");
    let outro = repo("root-outro-bem-mais-longo-para-diferir");
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n"), ("docs/b.md", b"outro\n")]);

    let relatorio_um = json_apply_report(&aplicar(&RepoRoot::at(&um).unwrap(), &plano));
    let relatorio_outro = json_apply_report(&aplicar(&RepoRoot::at(&outro).unwrap(), &plano));

    assert_eq!(relatorio_um, relatorio_outro);
    for proibido in [
        um.to_string_lossy().to_string(),
        outro.to_string_lossy().to_string(),
    ] {
        assert!(
            !relatorio_um.contains(&proibido),
            "root absoluto vazou no relatório"
        );
    }
    limpar(&um);
    limpar(&outro);
}

// ---------------------------------------------------------------------------
// Check estritamente sem escrita
// ---------------------------------------------------------------------------

#[test]
fn check_nao_escreve_nada() {
    let base = repo("zero-write");
    fs::write(base.join("docs/a.md"), b"antigo\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"novo\n"), ("docs/b.md", b"criar\n")]);

    let antes = instantaneo(&base);
    let observado = observe(&root, &plano).expect("observação");
    let relatorio = check(&plano, &observado).expect("check");
    let depois = instantaneo(&base);

    assert_eq!(antes, depois, "o check alterou a árvore");
    assert_eq!(relatorio.outcome, Outcome::Drift);
    assert!(temporarios(&base).is_empty());
    limpar(&base);
}

#[test]
fn observacao_distingue_alvo_presente_de_ausente() {
    let base = repo("observacao");
    fs::write(base.join("docs/a.md"), b"presente\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();

    let presente = observe_target(&root, &rel("docs/a.md")).expect("observação presente");
    assert_eq!(presente.bytes(), Some(&b"presente\n"[..]));
    let ausente = observe_target(&root, &rel("docs/b.md")).expect("observação ausente");
    assert_eq!(ausente.bytes(), None, "ausência é observação válida");
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Autorização
// ---------------------------------------------------------------------------

#[test]
fn apply_exige_autorizacao_no_tipo() {
    // Não existe caminho de escrita sem autorização: a assinatura recusa.
    let fonte = include_str!("../src/automation/fsio.rs");
    assert!(fonte.contains("authorization: &Authorization"));
    assert!(!fonte.contains("fn apply_unchecked"));
}

#[test]
fn digest_errado_e_rejeitado_sem_tocar_o_disco() {
    let base = repo("digest-errado");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();

    let antes = instantaneo(&base);
    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&"0".repeat(64)),
        &precondicao,
    );
    let depois = instantaneo(&base);

    assert_eq!(antes, depois, "autorização inválida escreveu");
    match &relatorio.failure {
        Some(Failure::PolicyViolation(PolicyCause::AuthorizationMismatch { expected, .. })) => {
            assert_eq!(expected, &plano.digest());
        }
        outro => panic!("esperada autorização divergente, veio {outro:?}"),
    }
    assert!(relatorio.applied.is_empty());
    assert_eq!(relatorio.not_attempted, vec!["docs/a.md".to_string()]);
    assert!(!relatorio.rollback_performed);
    limpar(&base);
}

#[test]
fn sensibilidade_da_autorizacao_a_um_unico_caractere() {
    let base = repo("autorizacao-sensivel");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();

    let mut quase = plano.digest();
    let ultimo = quase.pop().unwrap();
    quase.push(if ultimo == 'a' { 'b' } else { 'a' });

    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&quase),
        &precondicao,
    );
    assert!(matches!(
        relatorio.failure,
        Some(Failure::PolicyViolation(
            PolicyCause::AuthorizationMismatch { .. }
        ))
    ));
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Plano obsoleto
// ---------------------------------------------------------------------------

#[test]
fn plano_obsoleto_e_detectado_e_nao_escreve() {
    let base = repo("stale");
    fs::write(base.join("docs/a.md"), b"antes\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"desejado\n")]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();

    // Alguém mexeu no alvo entre o check e a aplicação.
    fs::write(base.join("docs/a.md"), b"mudou por fora\n").unwrap();

    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&plano.digest()),
        &precondicao,
    );
    match &relatorio.failure {
        Some(Failure::StalePlan { msg, .. }) => assert!(msg.contains("docs/a.md"), "{msg}"),
        outro => panic!("esperado plano obsoleto, veio {outro:?}"),
    }
    assert_eq!(
        fs::read(base.join("docs/a.md")).unwrap(),
        b"mudou por fora\n",
        "plano obsoleto não pode escrever"
    );
    assert!(relatorio.applied.is_empty());
    assert_eq!(relatorio.decision, Some(Decision::NeedsHumanDecision));
    limpar(&base);
}

#[test]
fn precondicao_de_outro_plano_e_rejeitada() {
    let base = repo("stale-outro-plano");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"um\n")]);
    let outro = plano_criar(&[("docs/b.md", b"dois\n")]);
    let observado = observe(&root, &outro).unwrap();
    let precondicao = check(&outro, &observado).unwrap();

    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&plano.digest()),
        &precondicao,
    );
    assert!(matches!(relatorio.failure, Some(Failure::StalePlan { .. })));
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Apply: create, replace, remove, idempotência
// ---------------------------------------------------------------------------

#[test]
fn apply_cria_substitui_e_remove() {
    let base = repo("crud");
    fs::write(base.join("docs/b.md"), b"antigo\n").unwrap();
    fs::write(base.join("docs/c.md"), b"sobra\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();

    let plano = PlanBuilder::new("adaptador", allowlist())
        .desire("docs/a.md", b"criado\n".to_vec())
        .unwrap()
        .desire("docs/b.md", b"substituido\n".to_vec())
        .unwrap()
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();

    let relatorio = aplicar(&root, &plano);
    assert_eq!(relatorio.outcome, Some(Outcome::Applied));
    assert_eq!(relatorio.failure, None);
    assert_eq!(relatorio.decision, None);
    assert_eq!(
        relatorio.applied,
        vec![
            "docs/a.md".to_string(),
            "docs/b.md".to_string(),
            "docs/c.md".to_string()
        ]
    );
    assert_eq!(relatorio.failed, None);
    assert!(relatorio.not_attempted.is_empty());
    assert!(!relatorio.rollback_performed);
    assert_eq!(relatorio.final_drift, FinalDrift::Measured(Outcome::Match));

    assert_eq!(fs::read(base.join("docs/a.md")).unwrap(), b"criado\n");
    assert_eq!(fs::read(base.join("docs/b.md")).unwrap(), b"substituido\n");
    assert!(!base.join("docs/c.md").exists());
    assert!(temporarios(&base).is_empty(), "temporário residual");
    limpar(&base);
}

#[test]
fn apply_e_idempotente() {
    let base = repo("idempotente");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);

    let primeiro = aplicar(&root, &plano);
    assert_eq!(primeiro.outcome, Some(Outcome::Applied));
    assert_eq!(primeiro.applied, vec!["docs/a.md".to_string()]);

    let arvore_apos_primeiro = instantaneo(&base);
    let segundo = aplicar(&root, &plano);
    assert_eq!(segundo.outcome, Some(Outcome::NoChange));
    assert!(segundo.applied.is_empty());
    assert_eq!(segundo.final_drift, FinalDrift::Measured(Outcome::Match));
    assert_eq!(
        arvore_apos_primeiro,
        instantaneo(&base),
        "a segunda execução alterou a árvore"
    );
    limpar(&base);
}

#[test]
fn remocao_de_arquivo_ausente_nao_e_tentada() {
    let base = repo("remocao-ausente");
    let root = RepoRoot::at(&base).unwrap();
    let plano = PlanBuilder::new("adaptador", allowlist())
        .remove("docs/c.md")
        .unwrap()
        .build()
        .unwrap();
    let relatorio = aplicar(&root, &plano);
    assert_eq!(relatorio.outcome, Some(Outcome::NoChange));
    assert!(relatorio.applied.is_empty());
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Temporários
// ---------------------------------------------------------------------------

#[test]
fn colisao_de_temporario_e_contornada_sem_tocar_o_ocupante() {
    let base = repo("colisao");
    let ocupante = base.join("docs/.a.md.pinker-tmp-0");
    fs::write(&ocupante, b"nao me toque\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);

    let relatorio = aplicar(&root, &plano);
    assert_eq!(relatorio.outcome, Some(Outcome::Applied));
    assert_eq!(fs::read(base.join("docs/a.md")).unwrap(), b"conteudo\n");
    assert_eq!(
        fs::read(&ocupante).unwrap(),
        b"nao me toque\n",
        "o temporário ocupado foi sobrescrito"
    );
    limpar(&base);
}

#[test]
fn exaustao_de_temporarios_falha_explicitamente() {
    let base = repo("exaustao");
    for i in 0..pinker_v0::automation::fsio::MAX_TEMP_ATTEMPTS {
        fs::write(base.join(format!("docs/.a.md.pinker-tmp-{i}")), b"x").unwrap();
    }
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);
    let relatorio = aplicar(&root, &plano);
    match &relatorio.failure {
        Some(Failure::IoFailure { msg, .. }) => {
            assert!(msg.contains("exclusivo"), "{msg}")
        }
        outro => panic!("esperada exaustão de temporários, veio {outro:?}"),
    }
    assert!(!base.join("docs/a.md").exists());
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Falha antes e depois da substituição
// ---------------------------------------------------------------------------

#[test]
fn falha_antes_do_rename_preserva_o_alvo_e_nao_deixa_temporario() {
    let base = repo("pre-rename");
    fs::write(base.join("docs/a.md"), b"original\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"novo\n")]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();

    fs::set_permissions(base.join("docs"), fs::Permissions::from_mode(0o555)).unwrap();
    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&plano.digest()),
        &precondicao,
    );
    fs::set_permissions(base.join("docs"), fs::Permissions::from_mode(0o755)).unwrap();

    assert!(matches!(relatorio.failure, Some(Failure::IoFailure { .. })));
    assert_eq!(
        fs::read(base.join("docs/a.md")).unwrap(),
        b"original\n",
        "o alvo foi tocado apesar da falha antes do rename"
    );
    assert!(temporarios(&base).is_empty(), "temporário residual");
    assert!(!relatorio.rollback_performed);
    limpar(&base);
}

#[test]
fn verificacao_posterior_a_substituicao_detecta_conteudo_divergente() {
    // A verificação final é a garantia real por arquivo. Aqui ela é exercitada
    // diretamente: o alvo existe com um conteúdo e a expectativa é outra.
    let base = repo("pos-rename");
    fs::write(base.join("docs/a.md"), b"o que esta no disco\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();

    match verify_written(&root, &rel("docs/a.md"), Some(b"o que se esperava\n")) {
        Err(Failure::VerifyAfterApplyFailure { path, msg }) => {
            assert_eq!(path, "docs/a.md");
            assert!(msg.contains("digest") || msg.contains("tamanho"), "{msg}");
        }
        outro => panic!("esperada falha de verificação, veio {outro:?}"),
    }
    limpar(&base);
}

#[test]
fn verificacao_posterior_detecta_tamanho_divergente() {
    let base = repo("pos-rename-tamanho");
    fs::write(base.join("docs/a.md"), b"curto\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    match verify_written(&root, &rel("docs/a.md"), Some(b"bem mais longo\n")) {
        Err(Failure::VerifyAfterApplyFailure { msg, .. }) => {
            assert!(msg.contains("tamanho"), "{msg}")
        }
        outro => panic!("esperada divergência de tamanho, veio {outro:?}"),
    }
    limpar(&base);
}

#[test]
fn verificacao_posterior_exige_ausencia_apos_remocao() {
    let base = repo("pos-remocao");
    fs::write(base.join("docs/a.md"), b"ainda aqui\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    match verify_written(&root, &rel("docs/a.md"), None) {
        Err(Failure::VerifyAfterApplyFailure { msg, .. }) => {
            assert!(msg.contains("removido"), "{msg}")
        }
        outro => panic!("esperada exigência de ausência, veio {outro:?}"),
    }
    assert!(verify_written(&root, &rel("docs/b.md"), None).is_ok());
    limpar(&base);
}

#[test]
fn verificacao_posterior_aprova_o_que_o_apply_escreveu() {
    let base = repo("pos-ok");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"exato\n")]);
    assert_eq!(aplicar(&root, &plano).outcome, Some(Outcome::Applied));
    assert!(verify_written(&root, &rel("docs/a.md"), Some(b"exato\n")).is_ok());
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Confinamento
// ---------------------------------------------------------------------------

#[test]
fn target_symlink_e_rejeitado() {
    let base = repo("symlink-alvo");
    let fora = base.join("fora.txt");
    fs::write(&fora, b"externo\n").unwrap();
    std::os::unix::fs::symlink(&fora, base.join("docs/a.md")).unwrap();
    let root = RepoRoot::at(&base).unwrap();

    assert!(matches!(
        confine(&root, &rel("docs/a.md")),
        Err(Failure::PolicyViolation(PolicyCause::SymlinkTarget { .. }))
    ));
    // O pipeline para já na observação: o confinamento é atravessado antes de
    // qualquer escrita, então nem chega a existir plano autorizado a aplicar.
    let plano = plano_criar(&[("docs/a.md", b"tentativa\n")]);
    assert!(matches!(
        observe(&root, &plano),
        Err(Failure::PolicyViolation(PolicyCause::SymlinkTarget { .. }))
    ));
    assert!(matches!(
        observe_target(&root, &rel("docs/a.md")),
        Err(Failure::PolicyViolation(PolicyCause::SymlinkTarget { .. }))
    ));
    assert_eq!(
        fs::read(&fora).unwrap(),
        b"externo\n",
        "escreveu através do link simbólico"
    );
    limpar(&base);
}

#[test]
fn ancestral_symlink_e_rejeitado() {
    let base = repo("symlink-ancestral");
    let externo = base.join("externo");
    fs::create_dir_all(&externo).unwrap();
    std::os::unix::fs::symlink(&externo, base.join("docs/sub")).unwrap();
    let root = RepoRoot::at(&base).unwrap();

    match confine(&root, &rel("docs/sub/inner.md")) {
        Err(Failure::PolicyViolation(PolicyCause::SymlinkAncestor { component, .. })) => {
            assert_eq!(component, "sub");
        }
        outro => panic!("esperado ancestral symlink, veio {outro:?}"),
    }
    let plano = plano_criar(&[("docs/sub/inner.md", b"tentativa\n")]);
    assert!(matches!(
        observe(&root, &plano),
        Err(Failure::PolicyViolation(
            PolicyCause::SymlinkAncestor { .. }
        ))
    ));
    assert!(
        !externo.join("inner.md").exists(),
        "escreveu através do ancestral simbólico"
    );
    limpar(&base);
}

#[test]
fn ancestral_que_nao_e_diretorio_e_rejeitado() {
    let base = repo("ancestral-arquivo");
    fs::write(base.join("docs/sub"), b"sou arquivo\n").unwrap();
    let root = RepoRoot::at(&base).unwrap();
    assert!(matches!(
        confine(&root, &rel("docs/sub/inner.md")),
        Err(Failure::PolicyViolation(
            PolicyCause::AncestorNotDirectory { .. }
        ))
    ));
    limpar(&base);
}

#[test]
fn target_que_nao_e_arquivo_regular_e_rejeitado() {
    let base = repo("alvo-diretorio");
    fs::create_dir_all(base.join("docs/a.md")).unwrap();
    let root = RepoRoot::at(&base).unwrap();
    assert!(matches!(
        confine(&root, &rel("docs/a.md")),
        Err(Failure::PolicyViolation(
            PolicyCause::TargetNotRegularFile { .. }
        ))
    ));
    limpar(&base);
}

#[test]
fn path_absoluto_e_travessia_nao_chegam_ao_filesystem() {
    // A política lexical barra antes: não existe `RelativePath` inseguro para
    // entregar ao confinamento.
    assert!(matches!(
        RelativePath::new("/etc/passwd"),
        Err(PolicyCause::PathAbsolute { .. })
    ));
    assert!(matches!(
        RelativePath::new("../fora.md"),
        Err(PolicyCause::PathTraversal { .. })
    ));
    assert!(matches!(
        RelativePath::new("docs/../../fora.md"),
        Err(PolicyCause::PathTraversal { .. })
    ));
}

#[test]
fn target_fora_da_allowlist_nao_entra_no_plano() {
    let erro = PlanBuilder::new("adaptador", allowlist())
        .desire("docs/nao-declarado.md", b"x".to_vec())
        .expect_err("target não declarado");
    assert!(matches!(
        erro,
        Failure::PolicyViolation(PolicyCause::TargetNotAllowed { .. })
    ));
}

#[test]
fn sensibilidade_do_confinamento_a_cada_posicao_do_caminho() {
    let base = repo("sensibilidade-confinamento");
    let externo = base.join("externo");
    fs::create_dir_all(&externo).unwrap();
    let root = RepoRoot::at(&base).unwrap();

    // Sem nenhum link: passa.
    fs::create_dir_all(base.join("docs/sub")).unwrap();
    assert!(confine(&root, &rel("docs/sub/inner.md")).is_ok());

    // Link no ancestral: recusa.
    fs::remove_dir_all(base.join("docs/sub")).unwrap();
    std::os::unix::fs::symlink(&externo, base.join("docs/sub")).unwrap();
    assert!(confine(&root, &rel("docs/sub/inner.md")).is_err());

    // Ancestral de volta ao normal e link no alvo: recusa.
    fs::remove_file(base.join("docs/sub")).unwrap();
    fs::create_dir_all(base.join("docs/sub")).unwrap();
    std::os::unix::fs::symlink(externo.join("x"), base.join("docs/sub/inner.md")).unwrap();
    assert!(confine(&root, &rel("docs/sub/inner.md")).is_err());
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Progresso parcial
// ---------------------------------------------------------------------------

#[test]
fn progresso_parcial_e_explicito_e_sem_rollback() {
    let base = repo("parcial");
    fs::create_dir_all(base.join("docs/b")).unwrap();
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[
        ("docs/a.md", b"primeiro\n"),
        ("docs/b/inner.md", b"segundo\n"),
        ("docs/c.md", b"terceiro\n"),
    ]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();

    fs::set_permissions(base.join("docs/b"), fs::Permissions::from_mode(0o555)).unwrap();
    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest(&plano.digest()),
        &precondicao,
    );
    fs::set_permissions(base.join("docs/b"), fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(relatorio.applied, vec!["docs/a.md".to_string()]);
    assert_eq!(relatorio.failed, Some("docs/b/inner.md".to_string()));
    assert_eq!(relatorio.not_attempted, vec!["docs/c.md".to_string()]);
    assert!(!relatorio.rollback_performed);
    assert_eq!(relatorio.outcome, None, "aplicação parcial não é APPLIED");
    assert!(
        relatorio.failure.is_some(),
        "a causa precisa estar presente"
    );
    assert_eq!(relatorio.decision, Some(Decision::NeedsHumanDecision));
    assert_eq!(
        relatorio.final_drift,
        FinalDrift::Measured(Outcome::Drift),
        "o drift final precisa ser medido"
    );

    // O que foi aplicado permanece aplicado: nada foi desfeito.
    assert_eq!(fs::read(base.join("docs/a.md")).unwrap(), b"primeiro\n");
    assert!(!base.join("docs/c.md").exists());
    limpar(&base);
}

#[test]
fn a_causa_nunca_e_substituida_pela_decisao() {
    let base = repo("causa-e-decisao");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"x\n")]);
    let observado = observe(&root, &plano).unwrap();
    let precondicao = check(&plano, &observado).unwrap();
    let relatorio = apply(
        &root,
        &plano,
        &Authorization::for_digest("nao-e-o-digest"),
        &precondicao,
    );
    assert!(relatorio.failure.is_some());
    assert!(relatorio.decision.is_some());
    let json = json_apply_report(&relatorio);
    assert!(json.contains("\"code\":\"POLICY_VIOLATION\""));
    assert!(json.contains("\"decision\":\"NEEDS_HUMAN_DECISION\""));
    limpar(&base);
}

#[test]
fn nenhum_rollback_global_e_alegado() {
    let fonte = include_str!("../src/automation/fsio.rs");
    for proibido in ["fn rollback", "fn undo", "fn desfazer", "restore_backup"] {
        assert!(!fonte.contains(proibido), "alegou rollback: {proibido}");
    }
    assert!(fonte.contains("rollback_performed: false"));
    assert!(!fonte.contains("rollback_performed: true"));
}

// ---------------------------------------------------------------------------
// Relatórios
// ---------------------------------------------------------------------------

#[test]
fn relatorio_de_aplicacao_e_deterministico_e_de_uma_linha() {
    let base = repo("relatorio");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"conteudo\n")]);
    let relatorio = aplicar(&root, &plano);

    let json = json_apply_report(&relatorio);
    assert_eq!(json, json_apply_report(&relatorio));
    assert!(!json.contains('\n'));
    assert!(!json.contains('\u{1b}'));
    let ordem = [
        "\"schema\":",
        "\"producer\":",
        "\"plan_digest\":",
        "\"outcome\":",
        "\"applied\":",
        "\"failed\":",
        "\"not_attempted\":",
        "\"rollback_performed\":",
        "\"final_drift\":",
        "\"failure\":",
        "\"decision\":",
        "\"recovery\":",
    ];
    let mut anterior = 0usize;
    for campo in ordem {
        let posicao = json
            .find(campo)
            .unwrap_or_else(|| panic!("campo ausente: {campo}"));
        assert!(posicao >= anterior, "ordem instável em {campo}");
        anterior = posicao;
    }
    assert!(json.contains("\"rollback_performed\":false"));

    let markdown = markdown_apply_report(&relatorio);
    assert_eq!(markdown, markdown_apply_report(&relatorio));
    assert!(markdown.contains("rollback executado: false"));
    assert!(markdown.contains("Recuperação: observar novamente"));
    assert!(!markdown.contains("conteudo\n"), "payload no Markdown");
    limpar(&base);
}

#[test]
fn drift_final_desconhecido_carrega_a_razao() {
    // Estado desconhecido é declarado, não omitido.
    let desconhecido = FinalDrift::Unknown("releitura indisponível".to_string());
    assert_eq!(desconhecido.as_str(), "UNKNOWN");
    assert_eq!(
        FinalDrift::Measured(Outcome::Match).as_str(),
        Outcome::Match.as_str()
    );
}

#[test]
fn relatorio_nao_carrega_root_absoluto() {
    let base = repo("sem-root-no-relatorio");
    let root = RepoRoot::at(&base).unwrap();
    let plano = plano_criar(&[("docs/a.md", b"x\n")]);
    let relatorio = aplicar(&root, &plano);
    let absoluto = base.to_string_lossy().to_string();
    for texto in [
        json_apply_report(&relatorio),
        markdown_apply_report(&relatorio),
    ] {
        assert!(!texto.contains(&absoluto), "root absoluto no relatório");
    }
    limpar(&base);
}

// ---------------------------------------------------------------------------
// Fronteiras negativas
// ---------------------------------------------------------------------------

const FONTES_FS: [(&str, &str); 2] = [
    ("root.rs", include_str!("../src/automation/root.rs")),
    ("fsio.rs", include_str!("../src/automation/fsio.rs")),
];

#[test]
fn o_estagio_de_apply_nao_usa_rede_processos_nem_git() {
    for (nome, fonte) in FONTES_FS {
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
fn o_estagio_de_apply_nao_expoe_cli() {
    let cli = include_str!("../src/main.rs");
    assert!(
        !cli.contains("automation"),
        "a CLI não consome o núcleo neste estágio"
    );
}

#[test]
fn a_escrita_passa_por_um_unico_caminho() {
    let (_, fsio) = FONTES_FS[1];
    // Toda criação de arquivo passa por `create_new`; não há `File::create`
    // nem `fs::write` soltos que sobrescrevam um alvo direto.
    assert!(fsio.contains("create_new(true)"));
    assert!(!fsio.contains("fs::write("));
    assert!(!fsio.contains("File::create("));
    // A substituição é sempre por rename no mesmo diretório.
    assert_eq!(fsio.matches("fs::rename(").count(), 1);
}

#[test]
fn observacao_nao_escreve() {
    let (_, fsio) = FONTES_FS[1];
    let inicio = fsio.find("pub fn observe_target").expect("observe_target");
    let fim = fsio
        .find("// @pinker-nav:end automation.filesystem.observacao")
        .expect("fim");
    let corpo = &fsio[inicio..fim];
    for proibido in ["create_new", "rename", "remove_file", "write_all"] {
        assert!(!corpo.contains(proibido), "observação escreveu: {proibido}");
    }
}
