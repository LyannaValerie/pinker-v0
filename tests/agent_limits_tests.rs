use pinker_v0::agent::{executar, parse_spec_text, EXIT_ACCEPTED, EXIT_BLOCKED};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

// @pinker-nav:start evidencia.agent.limits
// @pinker-nav:domain development
// @pinker-nav:layer evidencia
// @pinker-nav:summary Limites de path, cwd, variáveis e diff em repositórios sintéticos: rejeita fuga do repo/delegado/worktree, aceita somente mudanças enumeradas e bloqueia arquivo alterado fora do escopo.
static NEXT: AtomicU64 = AtomicU64::new(1);

fn base(repo: &str, worktree: &str, delegated: &str, extra: &str) -> String {
    format!(
        "schema = 1\n\
         task_id = LIMITS\n\
         repo_root = {repo}\n\
         worktree = {worktree}\n\
         delegated_root = {delegated}\n\
         expected_base = fixture\n\
         allowed_write = .\n\
         verdict.accepted = ACCEPTED_TEST\n\
         verdict.blocked = BLOCKED_TEST\n\
         verdict.human = HUMAN_TEST\n{extra}"
    )
}

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pink-agent-limits-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("worktree")).unwrap();
    fs::create_dir_all(root.join("delegated")).unwrap();
    git(&root.join("worktree"), &["init", "-q"]);
    fs::write(root.join("worktree/README"), "fixture\n").unwrap();
    git(&root.join("worktree"), &["add", "README"]);
    git(
        &root.join("worktree"),
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
    root
}

fn git(cwd: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .unwrap()
        .success());
}

fn command(script: &str) -> String {
    format!("command.write.kind = program\ncommand.write.program = {script}\ncommand.write.cwd = .\ncommand.write.expect = 0\ncommand.write.shell = true\n")
}

fn write_spec(root: &Path, extra: &str) -> PathBuf {
    let path = root.join("delegated/task.agent");
    fs::write(
        &path,
        base(
            &root.display().to_string(),
            &root.join("worktree").display().to_string(),
            &root.join("delegated").display().to_string(),
            extra,
        ),
    )
    .unwrap();
    path
}

#[test]
fn repo_relativo_e_rejeitado() {
    assert!(
        parse_spec_text(&base("repo", "/repo/worktree", "/repo/delegated", ""))
            .unwrap_err()
            .contains("devem ser absolutos")
    );
}

#[test]
fn worktree_fora_do_repositorio_e_rejeitado() {
    assert!(
        parse_spec_text(&base("/repo", "/outside", "/repo/delegated", ""))
            .unwrap_err()
            .contains("dentro de repo_root")
    );
}

#[test]
fn delegado_fora_do_repositorio_e_rejeitado() {
    assert!(
        parse_spec_text(&base("/repo", "/repo/worktree", "/outside", ""))
            .unwrap_err()
            .contains("dentro de repo_root")
    );
}

#[test]
fn escrita_escapando_do_delegado_e_rejeitada() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "allowed_write = ../worktree\n",
    );
    assert!(parse_spec_text(&text).unwrap_err().contains("fuga"));
}

#[test]
fn mudanca_escapando_do_worktree_e_rejeitada() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "allowed_change = ../delegated/x\n",
    );
    assert!(parse_spec_text(&text).unwrap_err().contains("fuga"));
}

#[test]
fn cwd_escapando_do_worktree_e_rejeitado() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        &command("true").replace("cwd = .", "cwd = ../delegated"),
    );
    assert!(parse_spec_text(&text).unwrap_err().contains("fuga"));
}

#[test]
fn variavel_com_nome_nao_autorizavel_e_rejeitada() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        &(command("true") + "command.write.env.bad = x\n"),
    );
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("não autorizável"));
}

#[test]
fn variavel_duplicada_e_rejeitada() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        &(command("true") + "command.write.env.OK = x\ncommand.write.env.OK = y\n"),
    );
    assert!(parse_spec_text(&text).unwrap_err().contains("duplicada"));
}

#[test]
fn arquivo_alterado_no_escopo_e_aceito() {
    let root = fixture("allowed");
    let path = write_spec(
        &root,
        &("allowed_change = allowed.txt\n".to_string()
            + &command("printf-x-to-allowed")
                .replace("printf-x-to-allowed", "printf x > allowed.txt")),
    );
    assert_eq!(executar(&path).unwrap(), EXIT_ACCEPTED);
}

#[test]
fn arquivo_alterado_fora_do_escopo_e_rejeitado() {
    let root = fixture("outside");
    let path = write_spec(
        &root,
        &("allowed_change = allowed.txt\n".to_string()
            + &command("printf-x-to-outside")
                .replace("printf-x-to-outside", "printf x > outside.txt")),
    );
    assert_eq!(executar(&path).unwrap(), EXIT_BLOCKED);
}
#[test]
fn absoluto_dentro_da_raiz_e_aceito() {
    let root = fixture("absolute");
    let text = base(
        &root.display().to_string(),
        &root.join("worktree").display().to_string(),
        &root.join("delegated").display().to_string(),
        "allowed_write = artefatos\n",
    );
    assert!(parse_spec_text(&text).is_ok());
}
#[test]
fn git_status_first_line_is_preserved_and_rejects_out_of_scope() {
    let root = fixture("first-line");
    let path = write_spec(
        &root,
        &("allowed_change = allowed.txt\n".to_string() + &command("true")),
    );
    fs::write(root.join("worktree/README"), "modified\n").unwrap();
    let porcelain = Command::new("git")
        .current_dir(root.join("worktree"))
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&porcelain.stdout).lines().next(),
        Some(" M README")
    );
    assert_eq!(executar(&path).unwrap(), EXIT_BLOCKED);
}

#[test]
fn git_check_rejeita_path_absoluto() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "check.g.kind = git\ncheck.g.expected_change = /etc/passwd\n",
    );
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("relativo inválido"));
}

#[test]
fn marker_check_rejeita_escape() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "check.m.kind = marker-only\ncheck.m.path = ../x\ncheck.m.base_sha256 = x\ncheck.m.expected_regions = 1\ncheck.m.expected_marker_lines = 5\n",
    );
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("relativo inválido"));
}

#[test]
fn mutation_target_escape_e_rejeitado() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "mutation.x.target = ../delegated/x\nmutation.x.search_file = before\nmutation.x.replacement_file = after\nmutation.x.expected_matches = 1\nmutation.x.probe_program = false\nmutation.x.probe_expected_exit = 1\n",
    );
    assert!(parse_spec_text(&text)
        .unwrap_err()
        .contains("relativo inválido"));
}

#[test]
fn mutation_snippet_escape_e_rejeitado() {
    let text = base(
        "/repo",
        "/repo/worktree",
        "/repo/delegated",
        "mutation.x.target = src/x\nmutation.x.search_file = ../outside\nmutation.x.replacement_file = after\nmutation.x.expected_matches = 1\nmutation.x.probe_program = false\nmutation.x.probe_expected_exit = 1\n",
    );
    assert!(parse_spec_text(&text).unwrap_err().contains("fuga"));
}
// @pinker-nav:end evidencia.agent.limits
