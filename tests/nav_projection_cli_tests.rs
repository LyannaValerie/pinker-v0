use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl TempRepo {
    fn full(label: &str) -> TempRepo {
        TempRepo::full_in(std::env::temp_dir(), label)
    }

    fn full_without_git_context(label: &str) -> TempRepo {
        TempRepo::full_in(PathBuf::from("/tmp"), label)
    }

    fn full_in(base: PathBuf, label: &str) -> TempRepo {
        let repo = TempRepo::empty_in(base, label);
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        fs::create_dir_all(repo.0.join(".pinker/projections/recipes")).unwrap();
        fs::create_dir_all(repo.0.join("src")).unwrap();
        fs::copy(
            source.join(".pinker/doc.toml"),
            repo.0.join(".pinker/doc.toml"),
        )
        .unwrap();
        for entry in fs::read_dir(source.join(".pinker/projections")).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                fs::copy(
                    &path,
                    repo.0
                        .join(".pinker/projections")
                        .join(path.file_name().unwrap()),
                )
                .unwrap();
            }
        }
        for entry in fs::read_dir(source.join(".pinker/projections/recipes")).unwrap() {
            let path = entry.unwrap().path();
            fs::copy(
                &path,
                repo.0
                    .join(".pinker/projections/recipes")
                    .join(path.file_name().unwrap()),
            )
            .unwrap();
        }
        fs::copy(
            source.join("src/navigation.jsonl"),
            repo.0.join("src/navigation.jsonl"),
        )
        .unwrap();
        repo
    }

    fn empty(label: &str) -> TempRepo {
        TempRepo::empty_in(std::env::temp_dir(), label)
    }

    fn empty_in(base: PathBuf, label: &str) -> TempRepo {
        let path = base.join(format!(
            "pinker_projection_cli_{}_{}_{}",
            label,
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempRepo(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn trust_main(&self) {
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=projection-test",
                "-c",
                "user.email=projection-test@example.invalid",
                "commit",
                "-qm",
                "trusted baseline",
            ],
            vec!["update-ref", "refs/remotes/origin/main", "HEAD"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&self.0)
                .status()
                .unwrap()
                .success());
        }
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("TZ", "UTC")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn digest(json: &str) -> String {
    field(json, "digest")
}

/// Valor textual de um campo do relatório JSON.
fn field(json: &str, name: &str) -> String {
    let marker = format!("\"{name}\":\"");
    let Some(start) = json.find(&marker).map(|pos| pos + marker.len()) else {
        return String::new();
    };
    let end = json[start..].find('"').unwrap() + start;
    json[start..end].to_string()
}

fn projection(repo: &TempRepo, rest: &[&str]) -> Output {
    let mut args = vec!["nav", "projecao"];
    args.extend_from_slice(rest);
    args.push("--repo");
    args.push(repo.path().to_str().unwrap());
    run(&args)
}

fn prepare_plan(repo: &TempRepo, id: &str) -> Output {
    projection(
        repo,
        &[
            "preparar",
            id,
            "--predecessor",
            "onda-pink-agente-d",
            "--justificativa",
            "fixture de processo",
            "--json",
        ],
    )
}

fn prepare_apply(repo: &TempRepo, id: &str) -> Output {
    let plan = prepare_plan(repo, id);
    assert_eq!(plan.status.code(), Some(0), "{}", stderr(&plan));
    let digest = digest(&stdout(&plan));
    projection(
        repo,
        &[
            "preparar",
            id,
            "--predecessor",
            "onda-pink-agente-d",
            "--justificativa",
            "fixture de processo",
            "--autorizar",
            &digest,
            "--json",
        ],
    )
}

#[test]
fn help_publica_namespace_e_seis_subcomandos() {
    let help = run(&["nav", "projecao", "--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());
    let text = stdout(&help);
    for command in [
        "listar",
        "mostrar",
        "verificar",
        "preparar",
        "aceitar",
        "reconciliar",
    ] {
        assert!(text.contains(command), "{command}: {text}");
        let sub = run(&["nav", "projecao", command, "--help"]);
        assert_eq!(sub.status.code(), Some(0), "{command}");
        assert!(stdout(&sub).contains(&format!("nav projecao {command}")));
    }
    let nav = run(&["nav", "--help"]);
    assert!(stdout(&nav).contains("projecao"));
    let main = run(&["--help"]);
    assert!(stdout(&main).contains("nav"));
}

#[test]
fn listar_mostrar_e_verificar_sao_deterministicos_e_repo_relativos() {
    let repo = TempRepo::full("readonly");
    repo.trust_main();
    let list_a = projection(&repo, &["listar", "--json"]);
    let list_b = projection(&repo, &["listar", "--json"]);
    assert_eq!(list_a.status.code(), Some(0), "{}", stderr(&list_a));
    assert_eq!(list_a.stdout, list_b.stdout);
    let json = stdout(&list_a);
    assert!(json.starts_with("{\"schema\":1,\"command\":\"listar\""));
    assert!(!json.contains(repo.path().to_str().unwrap()));
    assert!(!json.contains("\u{1b}["));
    assert_eq!(json.lines().count(), 1);

    let show = projection(
        &repo,
        &["mostrar", "onda-pink-agente-d", "--observado", "--json"],
    );
    assert_eq!(show.status.code(), Some(0), "{}", stderr(&show));
    let shown = stdout(&show);
    assert!(shown.contains("\"definicao\""));
    assert!(shown.contains("\"observado\""));
    assert!(shown.contains("\"artifact_schema\":4"));

    for args in [
        vec!["verificar", "onda-pink-agente-d", "--json"],
        vec!["verificar", "--json"],
    ] {
        let verified = projection(&repo, &args);
        assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
        assert!(stdout(&verified).contains("\"outcome\":\"MATCH\""));
    }
}

#[test]
fn verificar_exige_autoridade_frozen_historica_confiavel() {
    let non_git = TempRepo::full_without_git_context("historical-authority-non-git");
    let git = Command::new("git")
        .args([
            "-C",
            non_git.path().to_str().unwrap(),
            "rev-parse",
            "--is-inside-work-tree",
        ])
        .output()
        .unwrap();
    assert!(!git.status.success(), "unexpected Git authority");
    let unavailable_without_git = projection(&non_git, &["verificar", "--json"]);
    assert_eq!(
        unavailable_without_git.status.code(),
        Some(6),
        "stdout={} stderr={}",
        stdout(&unavailable_without_git),
        stderr(&unavailable_without_git)
    );
    assert!(stdout(&unavailable_without_git).contains("HISTORICAL_AUTHORITY_UNVERIFIABLE"));

    let repo = TempRepo::full("historical-authority");
    repo.trust_main();
    let path = repo
        .path()
        .join(".pinker/projections/onda-pink-agente-d.toml");
    let original = fs::read_to_string(&path).unwrap();
    let mutated = original.replacen(
        "key = \"layout.tipos.memoria\"\nfrom = \"fnv1a64:a99dad8c28300e92\"",
        "key = \"layout.tipos.memoria\"\nfrom = \"fnv1a64:0000000000000000\"",
        1,
    );
    assert_ne!(original, mutated);
    fs::write(&path, mutated).unwrap();

    let protected = projection(&repo, &["verificar", "onda-pink-agente-d", "--json"]);
    assert_eq!(protected.status.code(), Some(6));
    assert!(stdout(&protected).contains("HISTORICAL_AUTHORITY_MUTATED"));

    fs::write(&path, &original).unwrap();
    fs::remove_file(&path).unwrap();
    let removed = projection(&repo, &["verificar", "onda-pink-agente-d", "--json"]);
    assert_eq!(removed.status.code(), Some(6));
    assert!(stdout(&removed).contains("HISTORICAL_AUTHORITY_MUTATED"));
    fs::write(&path, original).unwrap();
    assert!(Command::new("git")
        .args(["update-ref", "-d", "refs/remotes/origin/main"])
        .current_dir(repo.path())
        .status()
        .unwrap()
        .success());
    let unavailable = projection(&repo, &["verificar", "onda-pink-agente-d", "--json"]);
    assert_eq!(unavailable.status.code(), Some(6));
    assert!(stdout(&unavailable).contains("HISTORICAL_AUTHORITY_UNVERIFIABLE"));
}

#[test]
fn inventario_preserva_validos_e_sai_6_com_artefato_invalido() {
    let repo = TempRepo::full("invalid-list");
    fs::write(
        repo.path().join(".pinker/projections/invalido.toml"),
        "schema = 3\nid = \"invalido\"\nstate = \"MEIO\"\n",
    )
    .unwrap();
    let output = projection(&repo, &["listar", "--json"]);
    assert_eq!(output.status.code(), Some(6));
    let json = stdout(&output);
    assert!(json.contains("onda-pink-agente-d"));
    assert!(json.contains("invalido.toml"));
    assert!(json.contains("HARNESS_FAILURE"));
}

#[test]
fn exits_de_uso_autoridade_ausencia_e_politica_sao_distintos() {
    let repo = TempRepo::full("exits");
    let usage = projection(&repo, &["mostrar"]);
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert!(stderr(&usage).contains("requer exatamente um ID"));

    let missing = projection(&repo, &["mostrar", "nao-existe", "--json"]);
    assert_eq!(missing.status.code(), Some(4));
    assert!(stdout(&missing).contains("NOT_FOUND"));

    let policy = projection(
        &repo,
        &["preparar", "marco-c", "--justificativa", "x", "--json"],
    );
    assert_eq!(policy.status.code(), Some(7));
    assert!(stdout(&policy).contains("POLICY_VIOLATION"));

    let empty = TempRepo::empty("authority");
    fs::create_dir_all(empty.path().join(".pinker")).unwrap();
    fs::write(empty.path().join(".pinker/doc.toml"), "# root\n").unwrap();
    let authority = projection(&empty, &["listar", "--json"]);
    assert_eq!(authority.status.code(), Some(3));
    assert!(stdout(&authority).contains("IO_FAILURE"));
}

#[test]
fn preparar_plan_apply_digest_errado_e_reprepare() {
    let repo = TempRepo::full("prepare");
    let plan = prepare_plan(&repo, "marco-c");
    assert_eq!(plan.status.code(), Some(0), "{}", stderr(&plan));
    let json = stdout(&plan);
    assert!(json.contains("CANDIDATE_PLANNED"));
    assert!(json.contains("normalizacao-corrente-para-marco-c.toml"));
    assert!(!repo
        .path()
        .join(".pinker/projections/marco-c.toml")
        .exists());

    let wrong = projection(
        &repo,
        &[
            "preparar",
            "marco-c",
            "--predecessor",
            "onda-pink-agente-d",
            "--justificativa",
            "fixture de processo",
            "--autorizar",
            "deadbeef",
            "--json",
        ],
    );
    assert_eq!(wrong.status.code(), Some(7));
    assert!(stdout(&wrong).contains("POLICY_VIOLATION"));

    let applied = prepare_apply(&repo, "marco-c");
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert!(stdout(&applied).contains("CANDIDATE_PREPARED"));
    let rerun = prepare_plan(&repo, "marco-c");
    assert_eq!(rerun.status.code(), Some(0));
    assert!(stdout(&rerun).contains("NO_CHANGE"));
}

#[test]
fn aceitar_plan_apply_e_segunda_aceitacao_policy() {
    let repo = TempRepo::full("accept");
    let prepared = prepare_apply(&repo, "marco-c");
    assert_eq!(prepared.status.code(), Some(0), "{}", stderr(&prepared));
    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-marco-c.toml");
    let recipe_before = fs::read(&recipe).unwrap();

    let plan = projection(&repo, &["aceitar", "marco-c", "--json"]);
    assert_eq!(plan.status.code(), Some(0), "{}", stderr(&plan));
    assert!(stdout(&plan).contains("FROZEN_PLANNED"));
    let authorization = digest(&stdout(&plan));
    let applied = projection(
        &repo,
        &[
            "aceitar",
            "marco-c",
            "--autorizar",
            &authorization,
            "--json",
        ],
    );
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert!(stdout(&applied).contains("FROZEN_ACCEPTED"));
    assert_eq!(fs::read(recipe).unwrap(), recipe_before);
    let frozen = fs::read_to_string(repo.path().join(".pinker/projections/marco-c.toml")).unwrap();
    assert!(frozen.contains("state = \"FROZEN\""));

    let second = projection(&repo, &["aceitar", "marco-c", "--json"]);
    assert_eq!(second.status.code(), Some(7));
    assert!(stdout(&second).contains("POLICY_VIOLATION"));
}

#[test]
fn aceitar_candidate_que_nao_representa_mais_catalogo_sai_drift_5() {
    let repo = TempRepo::full("drift");
    assert_eq!(prepare_apply(&repo, "marco-c").status.code(), Some(0));
    let catalog_path = repo.path().join("src/navigation.jsonl");
    let catalog = fs::read_to_string(&catalog_path).unwrap();
    let changed = catalog.replacen("\"summary\":\"", "\"summary\":\"mudou legitimamente ", 1);
    assert_ne!(catalog, changed);
    fs::write(catalog_path, changed).unwrap();
    let output = projection(&repo, &["aceitar", "marco-c", "--json"]);
    assert_eq!(output.status.code(), Some(5), "{}", stderr(&output));
    assert!(stdout(&output).contains("\"outcome\":\"DRIFT\""));
    let candidate =
        fs::read_to_string(repo.path().join(".pinker/projections/marco-c.toml")).unwrap();
    assert!(candidate.contains("state = \"CANDIDATE\""));
}

#[test]
fn artifact_target_invalido_sai_harness_6_sem_vazar_root() {
    let repo = TempRepo::full("harness");
    fs::write(
        repo.path().join(".pinker/projections/quebrado.toml"),
        "schema = 3\nid = \"quebrado\"\nstate = \"CANDIDATE\"\n",
    )
    .unwrap();
    let output = projection(&repo, &["mostrar", "quebrado", "--json"]);
    assert_eq!(output.status.code(), Some(6));
    let json = stdout(&output);
    assert!(json.contains("HARNESS_FAILURE"));
    assert!(!json.contains(repo.path().to_str().unwrap()));
}

#[test]
fn reconciliar_planeja_sem_escrever_rejeita_stale_e_aplica_rota_legitima() {
    let repo = TempRepo::full("reconcile");
    repo.trust_main();
    let catalog = repo.path().join("src/navigation.jsonl");
    let changed = fs::read_to_string(&catalog).unwrap().replacen(
        "fnv1a64:22548e053f0d0e30",
        "fnv1a64:e9419517124b1183",
        1,
    );
    fs::write(&catalog, changed).unwrap();

    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let before = fs::read(&recipe).unwrap();
    let first = projection(&repo, &["reconciliar", "--json"]);
    let second = projection(&repo, &["reconciliar", "--json"]);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(fs::read(&recipe).unwrap(), before);
    let first_json = stdout(&first);
    assert!(first_json.contains("MECHANICALLY_RECONCILABLE"));
    assert!(first_json.contains("owner_file="));
    assert!(first_json.contains("planned_allowed_mutation=recipe.from"));
    let stale_digest = digest(&first_json);

    let changed_again = fs::read_to_string(&catalog).unwrap().replacen(
        "fnv1a64:e9419517124b1183",
        "fnv1a64:10d26d0524f20a0c",
        1,
    );
    fs::write(&catalog, changed_again).unwrap();
    let stale = projection(
        &repo,
        &["reconciliar", "--autorizar", &stale_digest, "--json"],
    );
    assert_eq!(stale.status.code(), Some(8), "{}", stderr(&stale));
    assert!(stdout(&stale).contains("STALE_PLAN"));
    assert_eq!(fs::read(&recipe).unwrap(), before);

    let plan = projection(&repo, &["reconciliar", "--json"]);
    let authorization = digest(&stdout(&plan));
    let applied = projection(
        &repo,
        &["reconciliar", "--autorizar", &authorization, "--json"],
    );
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert!(stdout(&applied).contains("\"outcome\":\"APPLIED\""));
    let verified = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verified.status.code(), Some(0), "{}", stderr(&verified));
    assert!(stdout(&verified).contains("\"outcome\":\"MATCH\""));

    let altered_recipe = fs::read_to_string(&recipe).unwrap().replacen(
        "expect_layer = \"layout\"",
        "expect_layer = \"other\"",
        1,
    );
    fs::write(&recipe, altered_recipe).unwrap();
    let source_again = fs::read_to_string(&catalog).unwrap().replacen(
        "fnv1a64:10d26d0524f20a0c",
        "fnv1a64:0000000000000000",
        1,
    );
    fs::write(&catalog, source_again).unwrap();
    let ambiguous = projection(&repo, &["reconciliar", "--json"]);
    assert_eq!(ambiguous.status.code(), Some(7), "{}", stderr(&ambiguous));
    assert!(stdout(&ambiguous).contains("SEMANTIC_AMBIGUITY_BLOCK"));
}

#[test]
fn recipe_change_that_breaks_reconstruction_exits_as_drift() {
    let repo = TempRepo::full("reconstruction-drift");
    repo.trust_main();
    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let original = fs::read_to_string(&recipe).unwrap();
    let mutated = original.replacen(
        "to = \"fnv1a64:a99dad8c28300e92\"",
        "to = \"fnv1a64:0000000000000000\"",
        1,
    );
    assert_ne!(original, mutated);
    fs::write(recipe, mutated).unwrap();
    let output = projection(&repo, &["verificar", "--json"]);
    assert_eq!(output.status.code(), Some(6), "{}", stderr(&output));
    assert!(stdout(&output).contains("HARNESS_FAILURE"));
}

// ---------------------------------------------------------------------------
// #685 — renomeação corrente reconstruída para a identidade histórica
// ---------------------------------------------------------------------------

/// Região com regra `override-hash` na receita, participante da história.
const RENOMEADA_COM_REGRA: &str = "backend-text.modelo.representacao";
/// Região participante da história que nenhuma regra nomeia.
const RENOMEADA_SEM_REGRA: &str = "ast.tipos.representacao";
/// Região que a receita exclui: nenhuma projeção histórica a contém.
const RENOMEADA_EXCLUIDA: &str = "automation.contrato.autorizacao";

/// Reescreve a identidade corrente de uma região no catálogo derivado.
fn rename_in_catalog(repo: &TempRepo, key: &str, novo_key: &str, novo_domain: Option<&str>) {
    let path = repo.path().join("src/navigation.jsonl");
    let texto = fs::read_to_string(&path).unwrap();
    let mut saida = String::with_capacity(texto.len());
    let mut encontrada = false;
    for linha in texto.lines() {
        if linha.contains(&format!("\"key\":\"{key}\"")) {
            let mut nova = linha.replace(
                &format!("\"key\":\"{key}\""),
                &format!("\"key\":\"{novo_key}\""),
            );
            if let Some(dominio) = novo_domain {
                let atual = domain_of(linha);
                nova = nova.replace(
                    &format!("\"domain\":\"{atual}\""),
                    &format!("\"domain\":\"{dominio}\""),
                );
            }
            saida.push_str(&nova);
            encontrada = true;
        } else {
            saida.push_str(linha);
        }
        saida.push('\n');
    }
    assert!(encontrada, "região {key} ausente do catálogo da fixture");
    fs::write(path, saida).unwrap();
}

fn domain_of(linha: &str) -> String {
    let marker = "\"domain\":\"";
    let start = linha.find(marker).unwrap() + marker.len();
    let end = linha[start..].find('"').unwrap() + start;
    linha[start..end].to_string()
}

fn catalog_domain(repo: &TempRepo, key: &str) -> String {
    let texto = fs::read_to_string(repo.path().join("src/navigation.jsonl")).unwrap();
    let linha = texto
        .lines()
        .find(|linha| linha.contains(&format!("\"key\":\"{key}\"")))
        .expect("região presente");
    domain_of(linha)
}

fn write_rename_map(repo: &TempRepo, corpo: &str) -> PathBuf {
    let path = repo.path().join("renames.toml");
    fs::write(&path, format!("schema = 1\n{corpo}")).unwrap();
    path
}

fn reconcile(repo: &TempRepo, map: Option<&Path>, autorizar: Option<&str>) -> Output {
    let mut args = vec!["reconciliar".to_string(), "--json".to_string()];
    if let Some(map) = map {
        args.push("--renomeacoes".to_string());
        args.push(map.to_str().unwrap().to_string());
    }
    if let Some(digest) = autorizar {
        args.push("--autorizar".to_string());
        args.push(digest.to_string());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    projection(repo, &refs)
}

#[test]
fn renomeacao_sem_mapa_permanece_ambiguidade_semantica() {
    let repo = TempRepo::full("rename-unmapped");
    repo.trust_main();
    rename_in_catalog(&repo, RENOMEADA_COM_REGRA, "backend-text.model.form", None);
    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let before = fs::read(&recipe).unwrap();

    let sem_mapa = reconcile(&repo, None, None);
    assert_eq!(sem_mapa.status.code(), Some(7), "{}", stderr(&sem_mapa));
    assert!(stdout(&sem_mapa).contains("SEMANTIC_AMBIGUITY_BLOCK"));
    assert_eq!(fs::read(&recipe).unwrap(), before);
}

#[test]
fn renomeacao_mapeada_reconstroi_identidade_e_todos_os_frozen_batem() {
    let repo = TempRepo::full("rename-mapped");
    repo.trust_main();
    let dominio_antigo = catalog_domain(&repo, RENOMEADA_COM_REGRA);
    rename_in_catalog(
        &repo,
        RENOMEADA_COM_REGRA,
        "backend-text.model.form",
        Some("model"),
    );
    rename_in_catalog(&repo, RENOMEADA_SEM_REGRA, "ast.types.form", None);
    rename_in_catalog(
        &repo,
        RENOMEADA_EXCLUIDA,
        "automation.contract.authorization",
        None,
    );

    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let before = fs::read(&recipe).unwrap();
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"backend-text.model.form\"\nhistorical_key = \"{RENOMEADA_COM_REGRA}\"\ncurrent_domain = \"model\"\nhistorical_domain = \"{dominio_antigo}\"\n\
             \n[[rename]]\ncurrent_key = \"ast.types.form\"\nhistorical_key = \"{RENOMEADA_SEM_REGRA}\"\n\
             \n[[rename]]\ncurrent_key = \"automation.contract.authorization\"\nhistorical_key = \"{RENOMEADA_EXCLUIDA}\"\n"
        ),
    );

    // C10: planejar não escreve byte nenhum, e o plano é determinístico.
    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let repetido = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.stdout, repetido.stdout);
    assert_eq!(fs::read(&recipe).unwrap(), before);
    let json = stdout(&plano);
    assert!(json.contains("EXPLICIT_RENAME_MAPPED"));
    assert!(json.contains("planned_allowed_mutation=recipe.override_region"));
    assert!(json.contains("planned_allowed_mutation=recipe.new_override_region"));
    assert!(json.contains("planned_allowed_mutation=recipe.exclude_key"));

    // C11: apply explícito muta só a receita autorizada.
    let autorizacao = digest(&json);
    let aplicado = reconcile(&repo, Some(&map), Some(&autorizacao));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));
    assert!(stdout(&aplicado).contains("\"outcome\":\"APPLIED\""));
    let depois = fs::read_to_string(&recipe).unwrap();
    assert!(depois.contains("to_key = \"backend-text.modelo.representacao\""));
    assert!(depois.contains(&format!("to_domain = \"{dominio_antigo}\"")));
    assert!(depois.contains("key = \"automation.contract.authorization\""));

    // C17: os treze snapshots congelados voltam a bater.
    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(stdout(&verificado).contains("\"outcome\":\"MATCH\""));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
}

#[test]
fn mapa_participa_do_digest_e_plano_obsoleto_e_recusado() {
    let repo = TempRepo::full("rename-digest");
    repo.trust_main();
    rename_in_catalog(&repo, RENOMEADA_SEM_REGRA, "ast.types.form", None);
    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let before = fs::read(&recipe).unwrap();

    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"ast.types.form\"\nhistorical_key = \"{RENOMEADA_SEM_REGRA}\"\n"
        ),
    );
    let json_primeiro = stdout(&reconcile(&repo, Some(&map), None));
    let primeiro = digest(&json_primeiro);

    // O mesmo plano de bytes, autorizado com o digest de um mapa diferente,
    // precisa ser recusado: a relação declarada faz parte da autorização.
    let outro = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"ast.types.form\"\nhistorical_key = \"{RENOMEADA_SEM_REGRA}\"\ncurrent_domain = \"dominio-que-a-regiao-nao-tem\"\nhistorical_domain = \"outro\"\n"
        ),
    );
    let com_outro = reconcile(&repo, Some(&outro), None);
    assert_eq!(com_outro.status.code(), Some(7), "{}", stdout(&com_outro));
    assert!(stdout(&com_outro).contains("MAPPING_GUARD_STALE"));

    // O plano publica a impressão digital do mapa que o produziu, e o produtor
    // do plano — que entra na forma canônica assinada pelo digest — carrega
    // essa mesma impressão. É o elo que torna a relação declarada inseparável
    // da autorização.
    let fingerprint = field(&json_primeiro, "rename_map");
    assert!(!fingerprint.is_empty());
    let producer = field(&json_primeiro, "producer");
    assert!(
        producer.contains(&fingerprint),
        "o produtor do plano não carrega o mapa: {producer}"
    );
    let sem_mapa = stdout(&reconcile(&repo, None, None));
    assert!(sem_mapa.contains("\"rename_map\":null"));
    assert!(!field(&sem_mapa, "producer").contains("renames:"));

    let stale = reconcile(&repo, None, Some(&primeiro));
    assert_eq!(stale.status.code(), Some(8), "{}", stderr(&stale));
    assert!(stdout(&stale).contains("STALE_PLAN"));
    assert_eq!(fs::read(&recipe).unwrap(), before);
}

#[test]
fn entrada_de_mapa_sem_uso_e_recusada_em_vez_de_ignorada() {
    let repo = TempRepo::full("rename-unused");
    repo.trust_main();
    let map = write_rename_map(
        &repo,
        "\n[[rename]]\ncurrent_key = \"chave.que.nao.existe\"\nhistorical_key = \"outra.que.nao.existe\"\n",
    );
    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(stdout(&saida).contains("MAPPING_CURRENT_ABSENT"));

    // Entrada cujo lado corrente existe, mas que não produz reconciliação
    // alguma: a região é excluída de toda projeção histórica e nenhuma regra
    // nomeia a identidade declarada. Ignorá-la deixaria passar um mapa que o
    // autor acredita ter efeito e não tem.
    let inerte = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{RENOMEADA_EXCLUIDA}\"\nhistorical_key = \"historico.que.regra.nenhuma.nomeia\"\n"
        ),
    );
    let saida = reconcile(&repo, Some(&inerte), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(stdout(&saida).contains("MAPPING_ENTRY_UNUSED"));
}

#[test]
fn mapa_invalido_recusa_antes_de_planejar() {
    let repo = TempRepo::full("rename-invalid");
    repo.trust_main();
    for corpo in [
        // Metade de par.
        "\n[[rename]]\ncurrent_key = \"a.b.c\"\nhistorical_key = \"d.e.f\"\ncurrent_domain = \"x\"\n",
        // Não declara restauração nenhuma.
        "\n[[rename]]\ncurrent_key = \"a.b.c\"\n",
        // Mesma identidade histórica reivindicada duas vezes.
        "\n[[rename]]\ncurrent_key = \"a.b.c\"\nhistorical_key = \"h.i.j\"\n\n[[rename]]\ncurrent_key = \"d.e.f\"\nhistorical_key = \"h.i.j\"\n",
        // Direção invertida: a mesma chave é corrente numa entrada e histórica
        // na outra.
        "\n[[rename]]\ncurrent_key = \"a.b.c\"\nhistorical_key = \"d.e.f\"\n\n[[rename]]\ncurrent_key = \"d.e.f\"\nhistorical_key = \"g.h.i\"\n",
    ] {
        let map = write_rename_map(&repo, corpo);
        let saida = reconcile(&repo, Some(&map), None);
        assert_eq!(saida.status.code(), Some(7), "{}", stderr(&saida));
        assert!(
            stdout(&saida).contains("RENAME_MAP_INVALID"),
            "{}",
            stdout(&saida)
        );
    }

    let ausente = repo.path().join("nao-existe.toml");
    let saida = reconcile(&repo, Some(&ausente), None);
    assert_eq!(saida.status.code(), Some(7));
    assert!(stdout(&saida).contains("RENAME_MAP_UNREADABLE"));
}

#[test]
fn mapa_nao_autoriza_mutacao_de_frozen() {
    let repo = TempRepo::full("rename-frozen");
    repo.trust_main();
    rename_in_catalog(&repo, RENOMEADA_SEM_REGRA, "ast.types.form", None);
    let frozen = repo
        .path()
        .join(".pinker/projections/onda-pink-agente-d.toml");
    let original = fs::read_to_string(&frozen).unwrap();
    let mutado = original.replacen("expect_domain = \"modelo\"", "expect_domain = \"model\"", 1);
    assert_ne!(original, mutado);
    fs::write(&frozen, mutado).unwrap();

    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"ast.types.form\"\nhistorical_key = \"{RENOMEADA_SEM_REGRA}\"\n"
        ),
    );
    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(6), "{}", stderr(&saida));
    assert!(stdout(&saida).contains("HISTORICAL_AUTHORITY_MUTATED"));
}

#[test]
fn renomeacao_sem_regra_e_sem_mapa_nao_produz_plano_nem_conserto_silencioso() {
    let repo = TempRepo::full("rename-noplan");
    repo.trust_main();
    rename_in_catalog(&repo, RENOMEADA_SEM_REGRA, "ast.types.form", None);
    let recipe = repo
        .path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml");
    let before = fs::read(&recipe).unwrap();

    let sem_mapa = reconcile(&repo, None, None);
    assert_eq!(sem_mapa.status.code(), Some(0), "{}", stdout(&sem_mapa));
    assert!(stdout(&sem_mapa).contains("NO_CHANGE"));
    assert_eq!(fs::read(&recipe).unwrap(), before);

    // E a renomeação continua visível como divergência: nada foi consertado.
    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_ne!(verificado.status.code(), Some(0));
    assert!(!stdout(&verificado).contains("\"outcome\":\"MATCH\"}"));
}

#[test]
fn mapa_que_contradiz_a_autoridade_existente_e_recusado() {
    let repo = TempRepo::full("rename-contradiction");
    repo.trust_main();
    let dominio_antigo = catalog_domain(&repo, RENOMEADA_COM_REGRA);
    rename_in_catalog(
        &repo,
        RENOMEADA_COM_REGRA,
        "backend-text.model.form",
        Some("model"),
    );
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"backend-text.model.form\"\nhistorical_key = \"{RENOMEADA_COM_REGRA}\"\ncurrent_domain = \"model\"\nhistorical_domain = \"dominio-que-a-receita-nao-espera\"\n"
        ),
    );
    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(stdout(&saida).contains("MAPPING_CONTRADICTS_AUTHORITY"));

    // O mesmo mapa, declarando a identidade histórica que a regra de fato
    // espera, é aceito: a recusa é de contradição, não de forma.
    let correto = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"backend-text.model.form\"\nhistorical_key = \"{RENOMEADA_COM_REGRA}\"\ncurrent_domain = \"model\"\nhistorical_domain = \"{dominio_antigo}\"\n"
        ),
    );
    let saida = reconcile(&repo, Some(&correto), None);
    assert_eq!(saida.status.code(), Some(0), "{}", stdout(&saida));
}

#[test]
fn identidade_historica_para_regiao_excluida_e_recusada() {
    let repo = TempRepo::full("rename-excluded");
    repo.trust_main();
    rename_in_catalog(
        &repo,
        RENOMEADA_EXCLUIDA,
        "automation.contract.authorization",
        None,
    );
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"automation.contract.authorization\"\nhistorical_key = \"{RENOMEADA_EXCLUIDA}\"\ncurrent_layer = \"automation\"\nhistorical_layer = \"automacao\"\n"
        ),
    );
    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(stdout(&saida).contains("MAPPING_RESTORES_EXCLUDED_REGION"));
}
