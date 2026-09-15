use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl TempRepo {
    fn full(label: &str) -> TempRepo {
        let repo = TempRepo::empty(label);
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
        let path = std::env::temp_dir().join(format!(
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
    let marker = "\"digest\":\"";
    let start = json.find(marker).unwrap() + marker.len();
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
