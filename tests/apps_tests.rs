mod common;

use pinker_v0::abstract_machine_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_{name}_{now}"))
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn fixture_repo(root: &Path) {
    write_file(
        root,
        "README.md",
        "| Fase funcional mais recente | Fase 239: teste |\n",
    );
    write_file(root, "MANUAL.md", "# Manual\n");
    write_file(
        root,
        "docs/handoff_codex.md",
        "| Fase funcional mais recente | **239** |\n| Ultimo hotfix | **HF-6** |\n",
    );
    write_file(root, "docs/roadmap.md", "# Bloco 20\nFase 239\n");
    write_file(root, "docs/history.md", "# Historico\n");
    write_file(root, "docs/history/indice.md", "# Indice\n");
    write_file(
        root,
        "docs/history/phases/indice.md",
        "- `201a250.md` — cobre até `239 - teste`.\n",
    );
    write_file(root, "docs/history/phases/201a250.md", "### 239 - teste\n");
    write_file(root, "apps/README.md", "# Apps\n");
}

fn run_guard_with_args(repo: &Path, extra_args: &[&str]) -> interpreter::RunOutcome {
    let code = include_str!("../apps/guardiao_pinker/principal.pink");
    let program = common::parse(code).unwrap();
    semantic::check_program(&program).unwrap();
    let program_ir = ir::lower_program(&program).unwrap();
    ir_validate::validate_program(&program_ir).unwrap();
    let cfg = cfg_ir::lower_program(&program_ir).unwrap();
    cfg_ir_validate::validate_program(&cfg).unwrap();
    let selected = instr_select::lower_program(&cfg).unwrap();
    instr_select_validate::validate_program(&selected).unwrap();
    let machine = pinker_v0::abstract_machine::lower_program(&selected).unwrap();
    abstract_machine_validate::validate_program(&machine).unwrap();
    let mut args = vec!["--repo".to_string(), repo.to_string_lossy().to_string()];
    args.extend(extra_args.iter().map(|arg| arg.to_string()));
    interpreter::run_program_with_args(&machine, &args).unwrap()
}

fn run_guard(repo: &Path) -> interpreter::RunOutcome {
    run_guard_with_args(repo, &[])
}

#[test]
fn guardiao_pinker_aprova_fixture_valida() {
    let root = temp_repo("guard_ok");
    fixture_repo(&root);

    let out = run_guard(&root);

    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
    assert_eq!(out.exit_status, None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn guardiao_pinker_reprova_docs_phases_legado() {
    let root = temp_repo("guard_phases");
    fixture_repo(&root);
    write_file(&root, "docs/phases.md", "# legado\n");

    let out = run_guard(&root);

    assert_eq!(out.exit_status, Some(1));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn guardiao_pinker_reprova_readme_com_fase_divergente() {
    let root = temp_repo("guard_readme_fase");
    fixture_repo(&root);
    write_file(
        &root,
        "README.md",
        "| Fase funcional mais recente | Fase 238: antiga |\n",
    );

    let out = run_guard(&root);

    assert_eq!(out.exit_status, Some(1));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn guardiao_pinker_consulta_doc_por_fase() {
    let root = temp_repo("guard_docs_fase");
    fixture_repo(&root);

    let out = run_guard_with_args(
        &root,
        &[
            "--docs",
            "--arquivo",
            "docs/handoff_codex.md",
            "--status",
            "fase",
            "--fase",
            "239",
        ],
    );

    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
    assert_eq!(out.exit_status, None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn guardiao_pinker_consulta_src_por_busca() {
    let root = temp_repo("guard_src_busca");
    fixture_repo(&root);
    write_file(
        &root,
        "src/cfg_ir.rs",
        "fn lower_short_circuit_value() {}\n",
    );

    let out = run_guard_with_args(
        &root,
        &[
            "--src",
            "--arquivo",
            "src/cfg_ir.rs",
            "--status",
            "busca",
            "--busca",
            "lower_short_circuit_value",
        ],
    );

    assert_eq!(out.return_value, Some(RuntimeValue::Int(0)));
    assert_eq!(out.exit_status, None);
    fs::remove_dir_all(root).unwrap();
}
