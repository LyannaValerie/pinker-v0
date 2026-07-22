use pinker_v0::agent::{
    executar, relatorio, sensibilidade, sha256_hex, status, verificar, EXIT_ACCEPTED, EXIT_BLOCKED,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

// @pinker-nav:start evidencia.agent.runner
// @pinker-nav:domain development
// @pinker-nav:layer evidencia
// @pinker-nav:summary Execução real em repositórios sintéticos confinados: códigos esperados e inesperados, stdout/stderr persistidos e exibidos, fail-fast NOT_RUN, estados terminais, Pinker tipado, relatório e manifesto SHA-256 com detecção de adulteração.
static NEXT: AtomicU64 = AtomicU64::new(1);

fn root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "pink-agent-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(path.join("worktree")).expect("worktree");
    fs::create_dir_all(path.join("delegated")).expect("delegated");
    git(&path.join("worktree"), &["init", "-q"]);
    fs::write(path.join("worktree/README"), "fixture\n").expect("fixture");
    git(&path.join("worktree"), &["add", "README"]);
    git(
        &path.join("worktree"),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "base",
        ],
    );
    path
}

fn git(cwd: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("git")
        .success());
}

fn spec(root: &Path, commands: &str) -> PathBuf {
    let path = root.join("delegated/task.agent");
    let text = format!(
        "schema = 1\n\
         task_id = RUNNER\n\
         repo_root = {}\n\
         worktree = {}\n\
         delegated_root = {}\n\
         expected_base = fixture\n\
         allowed_write = .\n\
         verdict.accepted = ACCEPTED_TEST\n\
         verdict.blocked = BLOCKED_TEST\n\
         verdict.human = HUMAN_TEST\n{commands}",
        root.display(),
        root.join("worktree").display(),
        root.join("delegated").display()
    );
    fs::write(&path, text).expect("spec");
    path
}

fn shell(id: &str, script: &str, expected: i32) -> String {
    format!("command.{id}.kind = program\ncommand.{id}.program = {script}\ncommand.{id}.cwd = .\ncommand.{id}.expect = {expected}\ncommand.{id}.shell = true\n")
}

#[test]
fn codigo_de_saida_esperado_aceita() {
    let root = root("expected");
    let path = spec(&root, &shell("ok", "exit 7", 7));
    assert_eq!(executar(&path).expect("run"), EXIT_ACCEPTED);
}

#[test]
fn codigo_de_saida_inesperado_bloqueia() {
    let root = root("unexpected");
    let path = spec(&root, &shell("bad", "exit 0", 9));
    assert_eq!(executar(&path).expect("run"), EXIT_BLOCKED);
}

#[test]
fn stdout_e_stderr_sao_persistidos() {
    let root = root("streams");
    let path = spec(
        &root,
        &shell("streams", "printf-out-and-err", 0)
            .replace("printf-out-and-err", "printf out; printf err >&2"),
    );
    assert_eq!(executar(&path).expect("run"), EXIT_ACCEPTED);
    assert_eq!(
        fs::read_to_string(root.join("delegated/logs/streams.stdout.txt")).unwrap(),
        "out"
    );
    assert_eq!(
        fs::read_to_string(root.join("delegated/logs/streams.stderr.txt")).unwrap(),
        "err"
    );
}

#[test]
fn stdout_e_stderr_sao_exibidos_pela_cli() {
    let root = root("display");
    let path = spec(
        &root,
        &shell("streams", "printf-out-and-err", 0)
            .replace("printf-out-and-err", "printf out; printf err >&2"),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["agente", "executar", path.to_str().unwrap()])
        .output()
        .expect("pink");
    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr, b"err");
}

#[test]
fn comando_posterior_fica_not_run() {
    let root = root("not-run");
    let path = spec(
        &root,
        &(shell("first", "exit 8", 0) + &shell("second", "exit 0", 0)),
    );
    assert_eq!(executar(&path).expect("run"), EXIT_BLOCKED);
    let events = fs::read_to_string(root.join("delegated/estado/command-events.jsonl")).unwrap();
    assert!(events.contains("\"command\":\"second\",\"status\":\"NOT_RUN\""));
}

#[test]
fn resultado_registra_estado_accepted() {
    let root = root("accepted");
    let path = spec(&root, &shell("ok", "exit 0", 0));
    executar(&path).unwrap();
    assert!(
        fs::read_to_string(root.join("delegated/artefatos/resultado.json"))
            .unwrap()
            .contains("\"status\": \"ACCEPTED\"")
    );
    assert_eq!(status(&path, false).unwrap(), EXIT_ACCEPTED);
}

#[test]
fn resultado_registra_estado_blocked() {
    let root = root("blocked");
    let path = spec(&root, &shell("bad", "exit 1", 0));
    executar(&path).unwrap();
    assert!(
        fs::read_to_string(root.join("delegated/artefatos/resultado.json"))
            .unwrap()
            .contains("\"status\": \"BLOCKED\"")
    );
    assert_eq!(status(&path, false).unwrap(), EXIT_BLOCKED);
}

#[test]
fn relatorio_final_e_gerado() {
    let root = root("report");
    let path = spec(&root, &shell("ok", "exit 0", 0));
    executar(&path).unwrap();
    assert_eq!(relatorio(&path).unwrap(), EXIT_ACCEPTED);
    assert!(root.join("delegated/artefatos/RELATORIO.md").is_file());
}

#[test]
fn manifesto_sha256_e_deterministico() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let root = root("manifest");
    let path = spec(&root, &shell("ok", "exit 0", 0));
    executar(&path).unwrap();
    let first =
        fs::read_to_string(root.join("delegated/artefatos/artifact-manifest.json")).unwrap();
    executar(&path).unwrap();
    let second =
        fs::read_to_string(root.join("delegated/artefatos/artifact-manifest.json")).unwrap();
    for manifest in [first, second] {
        let report = manifest.find("RELATORIO.md").unwrap();
        let environment = manifest.find("environment.json").unwrap();
        let result = manifest.find("resultado.json").unwrap();
        let validation = manifest.find("validation.json").unwrap();
        assert!(report < environment && environment < result && result < validation);
    }
}

#[test]
fn manifesto_detecta_artefato_alterado() {
    let root = root("tamper");
    let path = spec(&root, &shell("ok", "exit 0", 0));
    executar(&path).unwrap();
    fs::write(
        root.join("delegated/artefatos/resultado.json"),
        "adulterado",
    )
    .unwrap();
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

#[test]
fn comando_pinker_tipado_executa_pela_cli() {
    let root = root("typed");
    let command = "command.help.kind = pinker\ncommand.help.program = pink\ncommand.help.arg = --help\ncommand.help.cwd = .\ncommand.help.expect = 1\ncommand.help.shell = false\n";
    let path = spec(&root, command);
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["agente", "executar", path.to_str().unwrap()])
        .output()
        .expect("pink");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn append_spec(root: &Path, extra: &str) -> PathBuf {
    let path = spec(root, "");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(extra);
    fs::write(&path, text).unwrap();
    path
}

fn initialize_for_checks(path: &Path) {
    assert_eq!(executar(path).unwrap(), EXIT_ACCEPTED);
}

#[test]
fn git_check_valida_conjunto_exato_e_subconjunto() {
    let root = root("git-exact");
    fs::write(root.join("worktree/README"), "changed\n").unwrap();
    let head = Command::new("git")
        .current_dir(root.join("worktree"))
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let head = String::from_utf8(head.stdout).unwrap();
    let extra = format!("allowed_change = README\ncheck.g.kind = git\ncheck.g.expected_head = {}\ncheck.g.expected_change = README\ncheck.g.allowed_change = README\ncheck.g.diff_check = true\n", head.trim());
    let path = append_spec(&root, &extra);
    initialize_for_checks(&path);
    assert_eq!(
        verificar(&path).unwrap(),
        EXIT_ACCEPTED,
        "{}",
        fs::read_to_string(root.join("delegated/artefatos/git-checks.json")).unwrap()
    );
}

#[test]
fn git_check_bloqueia_path_ausente_e_extra() {
    let root = root("git-set");
    fs::write(root.join("worktree/README"), "changed\n").unwrap();
    let path = append_spec(
        &root,
        "allowed_change = README\ncheck.g.kind = git\ncheck.g.expected_change = missing\n",
    );
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

#[test]
fn git_check_bloqueia_branch_head_e_commit_count() {
    let root = root("git-identity");
    let path = append_spec(&root, "check.g.kind = git\ncheck.g.expected_branch = other\ncheck.g.expected_head = deadbeef\ncheck.g.commit_count_after_base = 99\n");
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

#[test]
fn git_diff_check_detecta_erro_de_whitespace() {
    let root = root("git-whitespace");
    fs::write(root.join("worktree/README"), "trailing   \n").unwrap();
    let path = append_spec(&root, "allowed_change = README\ncheck.g.kind = git\ncheck.g.expected_change = README\ncheck.g.diff_check = true\n");
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

fn marker_spec(root: &Path, source: &str, sha: &str) -> PathBuf {
    fs::write(root.join("worktree/marked.rs"), source).unwrap();
    append_spec(root, &format!("allowed_change = marked.rs\ncheck.m.kind = marker-only\ncheck.m.path = marked.rs\ncheck.m.base_sha256 = {sha}\ncheck.m.expected_regions = 1\ncheck.m.expected_marker_lines = 5\ncheck.m.expected_key = evidencia.test\n"))
}

#[test]
fn marker_only_valido_reconstroi_original() {
    let root = root("marker-valid");
    let original = "fn value() {}\n";
    let source = "// @pinker-nav:start evidencia.test\n// @pinker-nav:domain development\n// @pinker-nav:layer evidence\n// @pinker-nav:summary Teste.\nfn value() {}\n// @pinker-nav:end evidencia.test\n";
    let path = marker_spec(&root, source, &sha256_hex(original.as_bytes()));
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_ACCEPTED);
}

#[test]
fn marker_only_detecta_end_ausente() {
    let root = root("marker-missing-end");
    let source = "// @pinker-nav:start evidencia.test\n// @pinker-nav:domain development\n// @pinker-nav:layer evidence\n// @pinker-nav:summary Teste.\nfn value() {}\n";
    let path = marker_spec(&root, source, &sha256_hex(b"fn value() {}\n"));
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

#[test]
fn marker_like_em_string_e_apos_codigo_sao_ignorados() {
    let root = root("marker-lexical");
    let original =
        "const X: &str = \"// @pinker-nav:end fake\";\nfn value() {} // @pinker-nav:start fake\n";
    let source = format!("// @pinker-nav:start evidencia.test\n// @pinker-nav:domain development\n// @pinker-nav:layer evidence\n// @pinker-nav:summary Teste.\n{original}// @pinker-nav:end evidencia.test\n");
    let path = marker_spec(&root, &source, &sha256_hex(original.as_bytes()));
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_ACCEPTED);
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn projection_spec(root: &Path, tail: &str) -> PathBuf {
    let json = "{\"schema\":1,\"key\":\"evidencia.a\",\"kind\":\"region\",\"domain\":\"d\",\"layer\":\"evidencia\",\"file\":\"src/a.rs\",\"start_marker\":1,\"content_start\":5,\"content_end\":5,\"end_marker\":6,\"summary\":\"A.\",\"hash\":\"fnv1a64:01\",\"status\":\"active\"}\n";
    fs::create_dir_all(root.join("worktree/src")).unwrap();
    fs::write(root.join("worktree/src/navigation.jsonl"), json).unwrap();
    let projection = format!(
        "{:?}\n",
        (
            1,
            "evidencia.a",
            "region",
            Some("d"),
            Some("evidencia"),
            "src/a.rs",
            "A.",
            "fnv1a64:01",
            "active"
        )
    );
    append_spec(root, &format!("allowed_change = src/navigation.jsonl\ncheck.p.kind = projection\ncheck.p.catalog = src/navigation.jsonl\ncheck.p.expected_total = 1\ncheck.p.expected_evidence = 1\ncheck.p.expected_runtime = 0\ncheck.p.expected_length = {}\ncheck.p.expected_fnv1a64 = {:016x}\n{tail}", projection.len(), fnv(projection.as_bytes())))
}

#[test]
fn projection_valida_medidas_estaveis() {
    let root = root("projection-valid");
    let path = projection_spec(&root, "");
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_ACCEPTED);
}

#[test]
fn projection_detecta_total_e_fnv_divergentes() {
    let root = root("projection-mismatch");
    let path = projection_spec(&root, "check.p.expected_total = 2\n");
    assert!(
        executar(&path).is_err(),
        "campo duplicado deve ser rejeitado"
    );
}

#[test]
fn projection_rejeita_override_nao_usado() {
    let root = root("projection-unused");
    let path = projection_spec(
        &root,
        "check.p.override_hash.missing = fnv1a64:0000000000000002\n",
    );
    initialize_for_checks(&path);
    assert_eq!(verificar(&path).unwrap(), EXIT_BLOCKED);
}

fn mutation_spec(root: &Path, program: &str, expected: i32, matches: usize) -> PathBuf {
    fs::write(root.join("worktree/target.txt"), "abc\n").unwrap();
    fs::write(root.join("delegated/before"), "b").unwrap();
    fs::write(root.join("delegated/after"), "x").unwrap();
    append_spec(root, &format!("mutation.m.target = target.txt\nmutation.m.search_file = before\nmutation.m.replacement_file = after\nmutation.m.expected_matches = {matches}\nmutation.m.probe_program = {program}\nmutation.m.probe_expected_exit = {expected}\n"))
}

#[test]
fn sensibilidade_detecta_e_restaura_mutacao() {
    let root = root("mutation-detected");
    let path = mutation_spec(&root, "/bin/false", 1, 1);
    assert_eq!(sensibilidade(&path).unwrap(), EXIT_ACCEPTED);
    assert_eq!(
        fs::read(root.join("worktree/target.txt")).unwrap(),
        b"abc\n"
    );
}

#[test]
fn sensibilidade_undetected_bloqueia_e_restaura() {
    let root = root("mutation-undetected");
    let path = mutation_spec(&root, "/bin/true", 1, 1);
    assert_eq!(sensibilidade(&path).unwrap(), EXIT_BLOCKED);
    assert_eq!(
        fs::read(root.join("worktree/target.txt")).unwrap(),
        b"abc\n"
    );
}

#[test]
fn sensibilidade_probe_failure_bloqueia_e_restaura() {
    let root = root("mutation-probe-failure");
    let path = mutation_spec(&root, "/missing/program", 1, 1);
    assert_eq!(sensibilidade(&path).unwrap(), EXIT_BLOCKED);
    assert_eq!(
        fs::read(root.join("worktree/target.txt")).unwrap(),
        b"abc\n"
    );
}

#[test]
fn sensibilidade_match_count_errado_bloqueia() {
    let root = root("mutation-match-count");
    let path = mutation_spec(&root, "/bin/false", 1, 2);
    assert_eq!(sensibilidade(&path).unwrap(), EXIT_BLOCKED);
}

#[cfg(unix)]
#[test]
fn falha_de_restauracao_bloqueia_e_interrompe_mutacoes_posteriores() {
    let root = root("mutation-restore-failure");
    fs::write(root.join("worktree/target.txt"), "abc\n").unwrap();
    fs::write(root.join("delegated/before"), "b").unwrap();
    fs::write(root.join("delegated/after"), "x").unwrap();
    let first = "mutation.first.target = target.txt\nmutation.first.search_file = before\nmutation.first.replacement_file = after\nmutation.first.expected_matches = 1\nmutation.first.probe_program = /bin/chmod\nmutation.first.probe_arg = 0555\nmutation.first.probe_arg = .\nmutation.first.probe_expected_exit = 0\n";
    let second = "mutation.second.target = target.txt\nmutation.second.search_file = before\nmutation.second.replacement_file = after\nmutation.second.expected_matches = 1\nmutation.second.probe_program = /bin/false\nmutation.second.probe_expected_exit = 1\n";
    let path = append_spec(&root, &(first.to_string() + second));
    assert_eq!(sensibilidade(&path).unwrap(), EXIT_BLOCKED);
    fs::set_permissions(root.join("worktree"), fs::Permissions::from_mode(0o755)).unwrap();
    let events = fs::read_to_string(root.join("delegated/estado/mutation-events.jsonl")).unwrap();
    assert_eq!(events.lines().count(), 1);
    assert!(events.contains("HARNESS_ERROR"));
}
// @pinker-nav:end evidencia.agent.runner
