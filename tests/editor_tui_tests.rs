use pinker_v0::editor_tui::EditorTui;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file_path(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock ok")
        .as_nanos();
    path.push(format!("{}_{}.pink", prefix, now));
    path
}

#[test]
fn editor_tui_tokens_acao_minima_funciona() {
    let path = temp_file_path("pinker_editor_tui_tokens");
    std::fs::write(
        &path,
        "pacote main;\ncarinho principal() -> bombom { falar(1); mimo 0; }",
    )
    .expect("write fixture");

    let mut editor = EditorTui::from_path(path.as_path()).expect("open editor");
    editor.execute_command(":tokens").expect("tokens command");

    assert!(editor
        .output_panel_lines()
        .iter()
        .any(|line| line.starts_with("TOKENS:")));

    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn editor_tui_ast_falha_em_codigo_invalido() {
    let path = temp_file_path("pinker_editor_tui_ast_fail");
    std::fs::write(
        &path,
        "pacote main;\ncarinho principal( -> bombom { mimo 0; }",
    )
    .expect("write fixture");

    let mut editor = EditorTui::from_path(path.as_path()).expect("open editor");
    let result = editor.execute_command(":ast");
    assert!(result.is_err());

    std::fs::remove_file(path).expect("cleanup");
}
