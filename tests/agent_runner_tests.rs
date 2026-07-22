use pinker_v0::agent::{
    executar, relatorio, sha256_hex, status, verificar, EXIT_ACCEPTED, EXIT_BLOCKED,
};
use std::fs;
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
// @pinker-nav:end evidencia.agent.runner
