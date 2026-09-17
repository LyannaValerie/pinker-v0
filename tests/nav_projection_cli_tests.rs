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
                let atual = text_field(linha, "domain");
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

fn text_field(linha: &str, campo: &str) -> String {
    let marker = format!("\"{campo}\":\"");
    let start = linha.find(&marker).unwrap() + marker.len();
    let end = linha[start..].find('"').unwrap() + start;
    linha[start..end].to_string()
}

fn catalog_field(repo: &TempRepo, key: &str, campo: &str) -> String {
    let texto = fs::read_to_string(repo.path().join("src/navigation.jsonl")).unwrap();
    let linha = texto
        .lines()
        .find(|linha| linha.contains(&format!("\"key\":\"{key}\"")))
        .expect("região presente");
    text_field(linha, campo)
}

/// Reescreve só a metadata de uma região, preservando a chave corrente.
///
/// É a renomeação que o blocker desta unidade descreve: a chave continua
/// resolvendo diretamente, e só `domain` ou `layer` mudou.
fn retag_in_catalog(repo: &TempRepo, key: &str, campo: &str, valor: &str) {
    let path = repo.path().join("src/navigation.jsonl");
    let texto = fs::read_to_string(&path).unwrap();
    let mut saida = String::with_capacity(texto.len());
    let mut encontrada = false;
    for linha in texto.lines() {
        if linha.contains(&format!("\"key\":\"{key}\"")) {
            let atual = text_field(linha, campo);
            saida.push_str(&linha.replace(
                &format!("\"{campo}\":\"{atual}\""),
                &format!("\"{campo}\":\"{valor}\""),
            ));
            encontrada = true;
        } else {
            saida.push_str(linha);
        }
        saida.push('\n');
    }
    assert!(encontrada, "região {key} ausente do catálogo da fixture");
    fs::write(path, saida).unwrap();
}

/// Bloco `[[rules]]` que nomeia a chave indicada, para inspeção pontual.
fn rule_block(recipe: &str, key: &str) -> String {
    recipe
        .split("[[rules]]")
        .find(|bloco| bloco.contains(&format!("key = \"{key}\"\n")))
        .unwrap_or_else(|| panic!("nenhuma regra nomeia {key}"))
        .to_string()
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
    let dominio_antigo = catalog_field(&repo, RENOMEADA_COM_REGRA, "domain");
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
    let dominio_antigo = catalog_field(&repo, RENOMEADA_COM_REGRA, "domain");
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

// ---------------------------------------------------------------------------
// #685 — renomeação só de metadata sobre regra existente
//
// A chave continua resolvendo diretamente no catálogo corrente, e mesmo assim a
// identidade histórica de `domain`/`layer` precisa ser reconstruída. O
// reconciliador tem de reconhecer o mapa que nomeia essa mesma chave corrente,
// sem por isso deixar o mapa competir com o catálogo.
// ---------------------------------------------------------------------------

/// Região com regra `override-region` na receita, participante da história.
const REGIAO_COM_OVERRIDE_REGION: &str = "ast.closures.identificadores-livres";

fn recipe_path(repo: &TempRepo) -> PathBuf {
    repo.path()
        .join(".pinker/projections/recipes/normalizacao-corrente-para-historico.toml")
}

#[test]
fn renome_so_de_dominio_reconcilia_a_override_hash_existente() {
    let repo = TempRepo::full("rename-domain-only-hash");
    repo.trust_main();
    let dominio_antigo = catalog_field(&repo, RENOMEADA_COM_REGRA, "domain");
    let recipe = recipe_path(&repo);
    let antes = fs::read_to_string(&recipe).unwrap();
    let regra_antes = rule_block(&antes, RENOMEADA_COM_REGRA);
    assert!(regra_antes.contains("op = \"override-hash\""));
    let to_hash = regra_antes
        .lines()
        .find(|linha| linha.starts_with("to = "))
        .unwrap()
        .replace("to = ", "");

    retag_in_catalog(&repo, RENOMEADA_COM_REGRA, "domain", "model");
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{RENOMEADA_COM_REGRA}\"\ncurrent_domain = \"model\"\nhistorical_domain = \"{dominio_antigo}\"\n"
        ),
    );

    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let json = stdout(&plano);
    assert!(json.contains("EXPLICIT_RENAME_MAPPED"), "{json}");
    assert!(json.contains("planned_allowed_mutation=recipe.override_region"));
    // A regra que já nomeia a região carrega a restauração: fabricar uma
    // segunda regra para a mesma chave deixaria a `override-hash` correndo
    // primeiro contra um domínio que ela não reconhece mais.
    assert!(!json.contains("planned_allowed_mutation=recipe.new_override_region"));
    assert_eq!(fs::read_to_string(&recipe).unwrap(), antes);

    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));
    let depois = fs::read_to_string(&recipe).unwrap();
    assert!(depois.starts_with("schema = 4\n"), "{}", &depois[..40]);
    let regra = rule_block(&depois, RENOMEADA_COM_REGRA);
    assert!(regra.contains("op = \"override-region\""), "{regra}");
    assert!(regra.contains(&format!("to_hash = {to_hash}")), "{regra}");
    assert!(regra.contains("expect_domain = \"model\""), "{regra}");
    assert!(
        regra.contains(&format!("to_domain = \"{dominio_antigo}\"")),
        "{regra}"
    );
    assert!(!regra.contains("to_key"), "{regra}");

    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
}

#[test]
fn renome_so_de_camada_reconcilia_a_override_region_existente() {
    let repo = TempRepo::full("rename-layer-only-region");
    repo.trust_main();
    let camada_antiga = catalog_field(&repo, REGIAO_COM_OVERRIDE_REGION, "layer");
    let recipe = recipe_path(&repo);
    let antes = fs::read_to_string(&recipe).unwrap();
    let regra_antes = rule_block(&antes, REGIAO_COM_OVERRIDE_REGION);
    assert!(regra_antes.contains("op = \"override-region\""));
    let to_hash = regra_antes
        .lines()
        .find(|linha| linha.starts_with("to_hash = "))
        .unwrap()
        .to_string();
    let expect_file = regra_antes
        .lines()
        .find(|linha| linha.starts_with("expect_file = "))
        .unwrap()
        .to_string();

    retag_in_catalog(&repo, REGIAO_COM_OVERRIDE_REGION, "layer", "syntax");
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{REGIAO_COM_OVERRIDE_REGION}\"\ncurrent_layer = \"syntax\"\nhistorical_layer = \"{camada_antiga}\"\n"
        ),
    );

    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let json = stdout(&plano);
    assert!(json.contains("EXPLICIT_RENAME_MAPPED"), "{json}");
    assert!(!json.contains("planned_allowed_mutation=recipe.new_override_region"));
    assert_eq!(fs::read_to_string(&recipe).unwrap(), antes);

    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));
    let depois = fs::read_to_string(&recipe).unwrap();
    let regra = rule_block(&depois, REGIAO_COM_OVERRIDE_REGION);
    // A capacidade antiga sobrevive inteira: hash, caminho e o resto da regra.
    assert!(regra.contains(&to_hash), "{regra}");
    assert!(regra.contains(&expect_file), "{regra}");
    assert!(regra.contains("expect_layer = \"syntax\""), "{regra}");
    assert!(
        regra.contains(&format!("to_layer = \"{camada_antiga}\"")),
        "{regra}"
    );

    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
}

#[test]
fn renome_so_de_metadata_sem_mapa_nao_e_inferido_nem_escrito() {
    for (key, campo, valor) in [
        (RENOMEADA_COM_REGRA, "domain", "model"),
        (REGIAO_COM_OVERRIDE_REGION, "layer", "syntax"),
    ] {
        let repo = TempRepo::full("rename-metadata-unmapped");
        repo.trust_main();
        retag_in_catalog(&repo, key, campo, valor);
        let recipe = recipe_path(&repo);
        let antes = fs::read(&recipe).unwrap();

        let saida = reconcile(&repo, None, None);
        let json = stdout(&saida);
        // Sem mapa não há relação declarada: semelhança de valores não autoriza
        // reconstrução, e nada é escrito.
        assert!(
            !json.contains("MECHANICALLY_RECONCILABLE"),
            "{campo}: {json}"
        );
        assert!(!json.contains("to_domain"), "{campo}: {json}");
        assert!(!json.contains("to_layer"), "{campo}: {json}");
        assert_eq!(fs::read(&recipe).unwrap(), antes, "{campo}");

        // E a projeção congelada continua acusando a divergência em vez de
        // passar a bater por conta própria.
        let verificado = projection(&repo, &["verificar", "--json"]);
        assert_ne!(verificado.status.code(), Some(0), "{campo}: deveria acusar");
    }
}

#[test]
fn mapa_que_nomeia_chave_corrente_sem_efeito_material_e_recusado() {
    let repo = TempRepo::full("rename-metadata-irrelevant");
    repo.trust_main();
    // A região é excluída por toda projeção histórica: restaurar domínio para
    // ela não participa de reconciliação nenhuma.
    let dominio_antigo = catalog_field(&repo, RENOMEADA_EXCLUIDA, "domain");
    retag_in_catalog(&repo, RENOMEADA_EXCLUIDA, "domain", "contract");
    let recipe = recipe_path(&repo);
    let antes = fs::read(&recipe).unwrap();
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{RENOMEADA_EXCLUIDA}\"\ncurrent_domain = \"contract\"\nhistorical_domain = \"{dominio_antigo}\"\n"
        ),
    );

    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    let json = stdout(&saida);
    assert!(
        json.contains("MAPPING_ENTRY_UNUSED") || json.contains("MAPPING_RESTORES_EXCLUDED_REGION"),
        "{json}"
    );
    assert_eq!(fs::read(&recipe).unwrap(), antes);
}

#[test]
fn mapa_que_contradiz_guarda_de_metadata_da_regra_existente_e_recusado() {
    let repo = TempRepo::full("rename-metadata-contradiction");
    repo.trust_main();
    retag_in_catalog(&repo, RENOMEADA_COM_REGRA, "domain", "model");
    let recipe = recipe_path(&repo);
    let antes = fs::read(&recipe).unwrap();
    // A regra existente já afirma `expect_domain = "modelo"`. O mapa afirma
    // outra grafia histórica: escolher entre as duas seria o reconciliador
    // decidindo história.
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{RENOMEADA_COM_REGRA}\"\ncurrent_domain = \"model\"\nhistorical_domain = \"modelo-antigo\"\n"
        ),
    );

    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(
        stdout(&saida).contains("MAPPING_CONTRADICTS_AUTHORITY"),
        "{}",
        stdout(&saida)
    );
    assert_eq!(fs::read(&recipe).unwrap(), antes);
}

/// Região real cuja receita tem **duas** regras de override com o mesmo seletor
/// corrente: `override-hash` restaura o hash e `override-region` restaura o
/// arquivo. Enquanto nenhuma delas restaura identidade, as duas casam; quando
/// uma passa a devolver a chave histórica, a outra deixa de encontrar a região.
const DUPLICADA: &str = "cli.doc.consulta";
const DUPLICADA_CORRENTE: &str = "cli.doc.query";

/// Quantos blocos `[[rules]]` têm a chave indicada como **seletor**.
///
/// A comparação é por linha inteira: `to_key = "x"` contém `key = "x"` como
/// substring, e contar o destino como seletor leria uma regra fundida como duas.
fn rule_blocks(recipe: &str, key: &str) -> usize {
    recipe
        .split("[[rules]]")
        .filter(|bloco| seleciona(bloco, key))
        .count()
}

fn seleciona(bloco: &str, key: &str) -> bool {
    bloco
        .lines()
        .any(|linha| linha == format!("key = \"{key}\""))
}

fn expected_overrides(recipe: &str) -> u64 {
    let marker = "expected_overrides = ";
    let start = recipe.find(marker).unwrap() + marker.len();
    let end = recipe[start..].find('\n').unwrap() + start;
    recipe[start..end].trim().parse().unwrap()
}

/// Acrescenta uma regra de override à receita, ajustando o orçamento declarado.
fn add_override_rule(repo: &TempRepo, bloco: &str) {
    let path = recipe_path(repo);
    let texto = fs::read_to_string(&path).unwrap();
    let orcamento = expected_overrides(&texto);
    let texto = texto.replace(
        &format!("expected_overrides = {orcamento}"),
        &format!("expected_overrides = {}", orcamento + 1),
    );
    fs::write(&path, format!("{texto}\n[[rules]]\n{bloco}")).unwrap();
}

/// Bytes de todo artefato congelado, para provar que a reconciliação não os
/// toca.
fn frozen_bytes(repo: &TempRepo) -> Vec<(String, Vec<u8>)> {
    let mut saida = Vec::new();
    for entry in fs::read_dir(repo.path().join(".pinker/projections")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            saida.push((
                path.file_name().unwrap().to_string_lossy().to_string(),
                fs::read(&path).unwrap(),
            ));
        }
    }
    saida.sort();
    saida
}

#[test]
fn overrides_duplicados_da_mesma_regiao_fundem_numa_regra_atomica() {
    let repo = TempRepo::full("dup-coalesce");
    repo.trust_main();
    rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
    let recipe = recipe_path(&repo);
    let antes = fs::read(&recipe).unwrap();
    let orcamento_antes = expected_overrides(&String::from_utf8(antes.clone()).unwrap());
    let congelados_antes = frozen_bytes(&repo);
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
        ),
    );

    // C7 + C8: planejar é determinístico e não escreve byte nenhum.
    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let repetido = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.stdout, repetido.stdout);
    assert_eq!(fs::read(&recipe).unwrap(), antes);
    let json = stdout(&plano);
    assert!(json.contains("DUPLICATE_OVERRIDE_COALESCED"), "{json}");

    // C1 + C9: o apply autorizado muta só a receita e alcança MATCH.
    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));
    assert!(stdout(&aplicado).contains("\"outcome\":\"APPLIED\""));

    let depois = fs::read_to_string(&recipe).unwrap();
    assert_eq!(rule_blocks(&depois, DUPLICADA_CORRENTE), 1, "{depois}");
    assert_eq!(rule_blocks(&depois, DUPLICADA), 0, "{depois}");

    // C2: a regra única preserva cada efeito das duas originais — identidade,
    // hash e arquivo históricos — e cada guarda que elas declaravam.
    let bloco = rule_block(&depois, DUPLICADA_CORRENTE);
    for esperado in [
        "op = \"override-region\"",
        "to_key = \"cli.doc.consulta\"",
        "from_hash = \"fnv1a64:7072d67afe89778d\"",
        "to_hash = \"fnv1a64:b538942c8ee175c8\"",
        "expect_file = \"src/pink_cli/doc_cli.rs\"",
        "to_file = \"src/main.rs\"",
        "expect_domain = \"doc\"",
        "expect_layer = \"cli\"",
    ] {
        assert!(bloco.contains(esperado), "{esperado} ausente de {bloco}");
    }

    // C11: duas regras viraram uma, e o orçamento desce exatamente um.
    assert_eq!(expected_overrides(&depois), orcamento_antes - 1);

    // C10: nenhum byte congelado mudou.
    assert_eq!(frozen_bytes(&repo), congelados_antes);

    // C12: a reconstrução histórica volta a bater nos treze congelados.
    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(stdout(&verificado).contains("\"outcome\":\"MATCH\""));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
    assert!(!stdout(&verificado).contains("E-SNAP-REGIAO-REMOVIDA"));
}

#[test]
fn tres_overrides_da_mesma_regiao_com_campos_disjuntos_fundem_deterministicamente() {
    let repo = TempRepo::full("dup-coalesce-disjoint");
    repo.trust_main();
    let resumo = catalog_field(&repo, DUPLICADA, "summary");
    // Uma terceira regra sobre a mesma região, restaurando um campo que
    // nenhuma das outras duas toca.
    add_override_rule(
        &repo,
        &format!(
            "op = \"override-region\"\nkey = \"{DUPLICADA}\"\nfrom_summary = \"{resumo}\"\nto_summary = \"{resumo}\"\n"
        ),
    );
    rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
    let recipe = recipe_path(&repo);
    let orcamento_antes = expected_overrides(&fs::read_to_string(&recipe).unwrap());
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
        ),
    );

    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    assert_eq!(reconcile(&repo, Some(&map), None).stdout, plano.stdout);

    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&stdout(&plano))));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));

    // C3: os três campos disjuntos sobrevivem numa regra só.
    let depois = fs::read_to_string(&recipe).unwrap();
    assert_eq!(rule_blocks(&depois, DUPLICADA_CORRENTE), 1, "{depois}");
    let bloco = rule_block(&depois, DUPLICADA_CORRENTE);
    for esperado in ["to_hash = ", "to_file = ", "to_summary = ", "to_key = "] {
        assert!(bloco.contains(esperado), "{esperado} ausente de {bloco}");
    }
    // C11: três regras viraram uma, e o orçamento desce exatamente dois.
    assert_eq!(expected_overrides(&depois), orcamento_antes - 2);

    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
}

#[test]
fn destinos_contraditorios_na_mesma_regiao_recusam_antes_de_escrever() {
    let repo = TempRepo::full("dup-coalesce-destino");
    repo.trust_main();
    // A receita já restaura `src/main.rs` para esta região. Uma segunda regra
    // que restaura outro arquivo é uma contradição: preferir a primeira seria
    // escolher vencedor por ordem textual.
    add_override_rule(
        &repo,
        &format!(
            "op = \"override-region\"\nkey = \"{DUPLICADA}\"\nexpect_file = \"src/pink_cli/doc_cli.rs\"\nto_file = \"src/outro.rs\"\n"
        ),
    );
    rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
    let recipe = recipe_path(&repo);
    let antes = fs::read(&recipe).unwrap();
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
        ),
    );

    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(
        stdout(&saida).contains("DUPLICATE_OVERRIDE_CONFLICTING_DESTINATION"),
        "{}",
        stdout(&saida)
    );
    assert_eq!(fs::read(&recipe).unwrap(), antes);
}

#[test]
fn guardas_contraditorias_na_mesma_regiao_recusam_antes_de_escrever() {
    let repo = TempRepo::full("dup-coalesce-guarda");
    repo.trust_main();
    // As duas regras existentes declaram o mesmo `expect_file`. Uma terceira
    // que declara outro caminho corrente para a mesma região afirma um presente
    // diferente, e fundir teria de descartar uma das duas guardas.
    add_override_rule(
        &repo,
        &format!(
            "op = \"override-region\"\nkey = \"{DUPLICADA}\"\nexpect_file = \"src/outro.rs\"\nto_file = \"src/main.rs\"\n"
        ),
    );
    rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
    let recipe = recipe_path(&repo);
    let antes = fs::read(&recipe).unwrap();
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
        ),
    );

    let saida = reconcile(&repo, Some(&map), None);
    assert_eq!(saida.status.code(), Some(7), "{}", stdout(&saida));
    assert!(
        stdout(&saida).contains("DUPLICATE_OVERRIDE_CONFLICTING_GUARD"),
        "{}",
        stdout(&saida)
    );
    assert_eq!(fs::read(&recipe).unwrap(), antes);
}

#[test]
fn sem_mapa_overrides_duplicados_permanecem_duas_regras() {
    let repo = TempRepo::full("dup-sem-mapa");
    repo.trust_main();
    let recipe = recipe_path(&repo);
    let orcamento_antes = expected_overrides(&fs::read_to_string(&recipe).unwrap());
    // Deriva do hash corrente: a rota legada reconcilia a base da regra sem
    // tocar em identidade nenhuma.
    retag_in_catalog(&repo, DUPLICADA, "hash", "fnv1a64:0123456789abcdef");

    let plano = reconcile(&repo, None, None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let json = stdout(&plano);
    assert!(
        json.contains("planned_allowed_mutation=recipe.from"),
        "{json}"
    );
    assert!(!json.contains("DUPLICATE_OVERRIDE_COALESCED"), "{json}");

    let aplicado = reconcile(&repo, None, Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));

    // C6: sem mapa a receita mantém as duas regras, a operação de cada uma e o
    // orçamento declarado.
    let depois = fs::read_to_string(&recipe).unwrap();
    assert_eq!(rule_blocks(&depois, DUPLICADA), 2, "{depois}");
    let blocos: Vec<&str> = depois
        .split("[[rules]]")
        .filter(|bloco| seleciona(bloco, DUPLICADA))
        .collect();
    assert!(blocos.iter().any(|b| b.contains("op = \"override-hash\"")));
    assert!(blocos
        .iter()
        .any(|b| b.contains("op = \"override-region\"")));
    assert_eq!(expected_overrides(&depois), orcamento_antes);
}

/// Reescreve o seletor de uma regra identificada por operação e chave.
fn retitle_rule(repo: &TempRepo, op: &str, key: &str, novo: &str) {
    let path = recipe_path(repo);
    let texto = fs::read_to_string(&path).unwrap();
    let mut saida = String::with_capacity(texto.len());
    let mut trocada = false;
    for (indice, bloco) in texto.split("[[rules]]").enumerate() {
        if indice > 0 {
            saida.push_str("[[rules]]");
        }
        if bloco.contains(&format!("op = \"{op}\"\n")) && seleciona(bloco, key) && !trocada {
            saida.push_str(&bloco.replace(
                &format!("key = \"{key}\"\n"),
                &format!("key = \"{novo}\"\n"),
            ));
            trocada = true;
        } else {
            saida.push_str(bloco);
        }
    }
    assert!(trocada, "nenhuma regra {op} nomeia {key}");
    fs::write(path, saida).unwrap();
}

#[test]
fn overrides_da_mesma_regiao_em_grafias_diferentes_tambem_fundem() {
    let repo = TempRepo::full("dup-coalesce-grafias");
    repo.trust_main();
    // Receita em transição: uma regra já foi escrita na grafia corrente e a
    // outra continua na histórica. As duas selecionam a mesma região, e agrupar
    // por texto deixaria justamente este par fora da fusão.
    retitle_rule(&repo, "override-region", DUPLICADA, DUPLICADA_CORRENTE);
    rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
    let recipe = recipe_path(&repo);
    let orcamento_antes = expected_overrides(&fs::read_to_string(&recipe).unwrap());
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
        ),
    );

    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let json = stdout(&plano);
    assert!(json.contains("DUPLICATE_OVERRIDE_COALESCED"), "{json}");

    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));

    let depois = fs::read_to_string(&recipe).unwrap();
    assert_eq!(rule_blocks(&depois, DUPLICADA_CORRENTE), 1, "{depois}");
    assert_eq!(rule_blocks(&depois, DUPLICADA), 0, "{depois}");
    let bloco = rule_block(&depois, DUPLICADA_CORRENTE);
    for esperado in ["to_key = ", "to_hash = ", "to_file = "] {
        assert!(bloco.contains(esperado), "{esperado} ausente de {bloco}");
    }
    assert_eq!(expected_overrides(&depois), orcamento_antes - 1);

    let verificado = projection(&repo, &["verificar", "--json"]);
    assert_eq!(verificado.status.code(), Some(0), "{}", stderr(&verificado));
    assert!(!stdout(&verificado).contains("E-SNAP-REGIAO-REMOVIDA"));
    assert!(!stdout(&verificado).contains("\"outcome\":\"DRIFT\""));
}

#[test]
fn regiao_que_o_mapa_nao_renomeia_mantem_seus_overrides_separados() {
    let repo = TempRepo::full("dup-nao-renomeada");
    repo.trust_main();
    // O mapa renomeia outra região. A região de regras duplicadas não muda de
    // identidade, e fundir as regras dela seria reescrever a receita por conta
    // própria.
    rename_in_catalog(&repo, RENOMEADA_SEM_REGRA, "ast.types.form", None);
    let recipe = recipe_path(&repo);
    let orcamento_antes = expected_overrides(&fs::read_to_string(&recipe).unwrap());
    let map = write_rename_map(
        &repo,
        &format!(
            "\n[[rename]]\ncurrent_key = \"ast.types.form\"\nhistorical_key = \"{RENOMEADA_SEM_REGRA}\"\n"
        ),
    );

    let plano = reconcile(&repo, Some(&map), None);
    assert_eq!(plano.status.code(), Some(0), "{}", stderr(&plano));
    let json = stdout(&plano);
    assert!(!json.contains("DUPLICATE_OVERRIDE_COALESCED"), "{json}");

    let aplicado = reconcile(&repo, Some(&map), Some(&digest(&json)));
    assert_eq!(aplicado.status.code(), Some(0), "{}", stdout(&aplicado));

    // C6: as duas regras da região não renomeada continuam duas, com as
    // operações originais, e o orçamento só sobe pela regra nova da região que
    // o mapa de fato renomeia.
    let depois = fs::read_to_string(&recipe).unwrap();
    assert_eq!(rule_blocks(&depois, DUPLICADA), 2, "{depois}");
    let blocos: Vec<&str> = depois
        .split("[[rules]]")
        .filter(|bloco| seleciona(bloco, DUPLICADA))
        .collect();
    assert!(blocos.iter().any(|b| b.contains("op = \"override-hash\"")));
    assert!(blocos
        .iter()
        .any(|b| b.contains("op = \"override-region\"")));
    assert_eq!(expected_overrides(&depois), orcamento_antes + 1);
}

/// Sobe a versão declarada da receita, para as capacidades que exigem uma
/// versão maior do que a receita publicada declara hoje.
fn bump_recipe_schema(repo: &TempRepo, versao: u32) {
    let path = recipe_path(repo);
    let texto = fs::read_to_string(&path).unwrap();
    let novo = texto.replacen("schema = 3", &format!("schema = {versao}"), 1);
    assert_ne!(novo, texto, "a receita não declara schema = 3");
    fs::write(path, novo).unwrap();
}

/// Cada campo que a fusão carrega, provado por uma contradição própria.
///
/// `absorb_override` é uma tabela: todo campo passa pelo mesmo
/// `merge_override_field`, e a única forma de um deles deixar de ser fundido é
/// sumir dessa tabela. Sem um controle por campo isso é invisível de fora — foi
/// exatamente o que aconteceu com `to_hash`, porque em toda receita publicada
/// hoje quem carrega o hash histórico é também a regra que abre o grupo, então
/// tirá-lo da tabela não fazia nenhum teste falhar.
///
/// Cada caso declara duas regras sobre a mesma região que só divergem no campo
/// sob teste e espera a recusa nomeando esse campo, antes de qualquer escrita.
/// Perder o campo e eleger vencedor por ordem são o mesmo defeito visto de dois
/// lados: quem não funde o campo também não vê a contradição nele.
#[test]
fn cada_campo_fundido_recusa_sua_propria_contradicao_antes_de_escrever() {
    const FROM: &str = "fnv1a64:7072d67afe89778d";
    const TO: &str = "fnv1a64:b538942c8ee175c8";
    const ARQUIVO: &str = "src/pink_cli/doc_cli.rs";
    let par_hash = format!("from_hash = \"{FROM}\"\nto_hash = \"{TO}\"\n");

    // `identidade` marca os campos cuja capacidade a receita publicada ainda não
    // declara: restaurar `key`, `domain` ou `layer` exige a versão 4.
    let casos: Vec<(&str, bool, String, String, String)> = vec![
        (
            "from_hash",
            false,
            format!("to_hash = \"{TO}\"\n"),
            FROM.to_string(),
            "fnv1a64:0000000000000001".to_string(),
        ),
        (
            "to_hash",
            false,
            format!("from_hash = \"{FROM}\"\n"),
            TO.to_string(),
            "fnv1a64:0000000000000000".to_string(),
        ),
        (
            "from_summary",
            false,
            "to_summary = \"resumo historico\"\n".to_string(),
            "resumo corrente a".to_string(),
            "resumo corrente b".to_string(),
        ),
        (
            "to_summary",
            false,
            "from_summary = \"resumo corrente\"\n".to_string(),
            "resumo historico a".to_string(),
            "resumo historico b".to_string(),
        ),
        (
            "expect_file",
            false,
            par_hash.clone(),
            ARQUIVO.to_string(),
            "src/outro.rs".to_string(),
        ),
        (
            "to_file",
            false,
            format!("expect_file = \"{ARQUIVO}\"\n"),
            "src/main.rs".to_string(),
            "src/outro.rs".to_string(),
        ),
        (
            "expect_domain",
            false,
            par_hash.clone(),
            "doc".to_string(),
            "outro".to_string(),
        ),
        (
            "to_domain",
            true,
            "expect_domain = \"doc\"\n".to_string(),
            "doc-antigo-a".to_string(),
            "doc-antigo-b".to_string(),
        ),
        (
            "expect_layer",
            false,
            par_hash.clone(),
            "cli".to_string(),
            "outra".to_string(),
        ),
        (
            "to_layer",
            true,
            "expect_layer = \"cli\"\n".to_string(),
            "cli-antiga-a".to_string(),
            "cli-antiga-b".to_string(),
        ),
        (
            "to_key",
            true,
            String::new(),
            "cli.doc.antiga-a".to_string(),
            "cli.doc.antiga-b".to_string(),
        ),
    ];

    for (campo, identidade, comum, valor_a, valor_b) in casos {
        let repo = TempRepo::full(&format!("dup-merge-{campo}"));
        repo.trust_main();
        if identidade {
            bump_recipe_schema(&repo, 4);
        }
        for valor in [&valor_a, &valor_b] {
            add_override_rule(
                &repo,
                &format!(
                    "op = \"override-region\"\nkey = \"{DUPLICADA}\"\n{comum}{campo} = \"{valor}\"\n"
                ),
            );
        }
        rename_in_catalog(&repo, DUPLICADA, DUPLICADA_CORRENTE, None);
        let recipe = recipe_path(&repo);
        let antes = fs::read(&recipe).unwrap();
        let congelados_antes = frozen_bytes(&repo);
        let map = write_rename_map(
            &repo,
            &format!(
                "\n[[rename]]\ncurrent_key = \"{DUPLICADA_CORRENTE}\"\nhistorical_key = \"{DUPLICADA}\"\n"
            ),
        );

        let saida = reconcile(&repo, Some(&map), None);
        let texto = stdout(&saida);
        assert_eq!(saida.status.code(), Some(7), "{campo}: {texto}");
        assert!(
            texto.contains("DUPLICATE_OVERRIDE_CONFLICTING_"),
            "{campo}: {texto}"
        );
        assert!(
            texto.contains(&format!("field={campo}")),
            "{campo}: {texto}"
        );
        assert_eq!(fs::read(&recipe).unwrap(), antes, "{campo}");
        assert_eq!(frozen_bytes(&repo), congelados_antes, "{campo}");
    }
}
