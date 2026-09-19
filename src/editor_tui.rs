// @pinker-nav:start editor.state.model
// @pinker-nav:domain state
// @pinker-nav:layer editor
// @pinker-nav:summary OUTPUT_LINES and EDITOR_LINES fix how many lines of the output panel and of the file body are displayed by render(); the EditorTui struct holds file_path, the buffer of file lines (lines), the panel message history (output) and the dirty flag; from_path reads the file via fs::read_to_string and uses source.lines() to split the content, storing neither the original terminators nor the presence of a final newline, initializing the panel with a welcome message and returning Err(String) if the read fails.
use crate::ast::Program;
use crate::lexer::Lexer;
use crate::palette;
use crate::parser::Parser;
use crate::printer;
use crate::semantic;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const OUTPUT_LINES: usize = 10;
const EDITOR_LINES: usize = 18;

pub struct EditorTui {
    file_path: PathBuf,
    lines: Vec<String>,
    output: Vec<String>,
    dirty: bool,
}

impl EditorTui {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let file_path = path.as_ref().to_path_buf();
        let source = fs::read_to_string(&file_path)
            .map_err(|err| format!("Falha ao abrir '{}': {}", file_path.display(), err))?;
        Ok(Self {
            file_path,
            lines: source.lines().map(|line| line.to_string()).collect(),
            output: vec!["Editor Pinker iniciado. Digite :help para comandos.".to_string()],
            dirty: false,
        })
    }
    // @pinker-nav:end editor.state.model

    // @pinker-nav:start editor.session.commands
    // @pinker-nav:domain session
    // @pinker-nav:layer editor
    // @pinker-nav:summary run() is the editor's REPL loop: it renders, reads a line from stdin and calls execute_command, terminating when the latter returns Ok(false); execute_command interprets the text commands (:quit, :help, :tokens, :ast, :save, :append <text>, :set <line> <text>) and unknown messages become a line in the panel; run_tokens_command tokenizes the current source and lists the first lexemes in the panel; run_ast_command parses + semantically checks (parse_and_check_program) and lists the first lines of the rendered AST; save_file writes with fs::write (no atomic write) the source recomposed by current_source, so :save preserves neither CRLF byte for byte nor the original final newline, and clears dirty; set_line replaces an existing line by 1-based index, with panel messages for a missing/out-of-range index; errors from the Pinker actions (:tokens/:ast) return as Err(String) via render_for_cli_with_source, distinct from the routine messages pushed to the panel.
    pub fn run(&mut self) -> Result<(), String> {
        loop {
            self.render();
            print!("\ncomando> ");
            io::stdout()
                .flush()
                .map_err(|err| format!("Falha ao flush da saída: {err}"))?;

            let mut command = String::new();
            io::stdin()
                .read_line(&mut command)
                .map_err(|err| format!("Falha ao ler comando do editor: {err}"))?;

            if !self.execute_command(command.trim())? {
                break;
            }
        }

        Ok(())
    }

    pub fn execute_command(&mut self, command: &str) -> Result<bool, String> {
        if command.is_empty() {
            return Ok(true);
        }

        if command == ":quit" {
            if self.dirty {
                self.push_output(
                    "Arquivo com alterações não salvas. Use :save antes de :quit.".to_string(),
                );
                return Ok(true);
            }
            self.push_output("Encerrando editor Pinker.".to_string());
            return Ok(false);
        }

        if command == ":help" {
            self.push_output(
                ":tokens | :ast | :append <texto> | :set <linha> <texto> | :save | :quit"
                    .to_string(),
            );
            return Ok(true);
        }

        if command == ":tokens" {
            return self.run_tokens_command();
        }

        if command == ":ast" {
            return self.run_ast_command();
        }

        if command == ":save" {
            return self.save_file();
        }

        if let Some(text) = command.strip_prefix(":append ") {
            self.lines.push(text.to_string());
            self.dirty = true;
            self.push_output(format!(
                "Linha adicionada no final (total: {}).",
                self.lines.len()
            ));
            return Ok(true);
        }

        if let Some(rest) = command.strip_prefix(":set ") {
            return self.set_line(rest);
        }

        self.push_output(format!("Comando desconhecido: {command}"));
        Ok(true)
    }

    pub fn output_panel_lines(&self) -> &[String] {
        &self.output
    }

    fn run_tokens_command(&mut self) -> Result<bool, String> {
        let source = self.current_source();
        let mut lexer = Lexer::new(&source);
        let tokens = lexer
            .tokenize()
            .map_err(|err| err.render_for_cli_with_source(&source))?;

        self.push_output(format!("TOKENS: {}", tokens.len()));
        for token in tokens.iter().take(6) {
            self.push_output(format!("- {} '{}'", token.kind.name(), token.lexeme));
        }
        Ok(true)
    }

    fn run_ast_command(&mut self) -> Result<bool, String> {
        let source = self.current_source();
        let program = parse_and_check_program(&source)
            .map_err(|err| err.render_for_cli_with_source(&source))?;
        let rendered = printer::render_program(&program);
        self.push_output("AST: primeiras linhas".to_string());
        for line in rendered.lines().take(6) {
            self.push_output(format!("- {line}"));
        }
        Ok(true)
    }

    fn save_file(&mut self) -> Result<bool, String> {
        fs::write(&self.file_path, self.current_source())
            .map_err(|err| format!("Falha ao salvar '{}': {}", self.file_path.display(), err))?;
        self.dirty = false;
        self.push_output(format!("Arquivo salvo em '{}'.", self.file_path.display()));
        Ok(true)
    }

    fn set_line(&mut self, rest: &str) -> Result<bool, String> {
        let mut parts = rest.splitn(2, ' ');
        let Some(line_part) = parts.next() else {
            self.push_output("Uso: :set <linha> <texto>".to_string());
            return Ok(true);
        };
        let Some(text) = parts.next() else {
            self.push_output("Uso: :set <linha> <texto>".to_string());
            return Ok(true);
        };

        let line_number = line_part
            .parse::<usize>()
            .map_err(|_| format!("Linha inválida para :set: {line_part}"))?;
        if line_number == 0 || line_number > self.lines.len() {
            self.push_output(format!("Linha fora da faixa (1..={}).", self.lines.len()));
            return Ok(true);
        }

        self.lines[line_number - 1] = text.to_string();
        self.dirty = true;
        self.push_output(format!("Linha {} atualizada.", line_number));
        Ok(true)
    }
    // @pinker-nav:end editor.session.commands

    // @pinker-nav:start editor.render.output
    // @pinker-nav:domain render
    // @pinker-nav:layer editor
    // @pinker-nav:summary current_source joins `lines` with '\n' to form the current source: on save it normalizes separators to LF and does not restore a final newline that was originally present; render clears the screen with ANSI sequences, prints header/status (via palette), up to EDITOR_LINES lines of the file with a count of omitted lines, and the last OUTPUT_LINES messages of the output panel; push_output only pushes a String onto `output`.
    fn current_source(&self) -> String {
        self.lines.join("\n")
    }

    fn render(&self) {
        print!("\x1b[2J\x1b[H");
        let header = palette::negrito_se(
            palette::TEMA_PINKER.keyword,
            "Pinker Editor/TUI — Fase 136 (camada 1 conservadora)",
        );
        let status = palette::colorir_se(
            palette::TEMA_PINKER.texto_suave,
            &format!(
                "arquivo: {} | linhas: {} | alterado: {}",
                self.file_path.display(),
                self.lines.len(),
                if self.dirty { "sim" } else { "não" }
            ),
        );
        println!("{header}");
        println!("{status}");
        println!();
        println!("=== editor ===");
        for (idx, line) in self.lines.iter().take(EDITOR_LINES).enumerate() {
            println!("{:>4} | {}", idx + 1, line);
        }
        if self.lines.len() > EDITOR_LINES {
            println!("... ({} linhas omitidas)", self.lines.len() - EDITOR_LINES);
        }

        println!("\n=== saída pinker ===");
        let start = self.output.len().saturating_sub(OUTPUT_LINES);
        for line in &self.output[start..] {
            println!("{line}");
        }
    }

    fn push_output(&mut self, msg: String) {
        self.output.push(msg);
    }
}
// @pinker-nav:end editor.render.output

// @pinker-nav:start editor.analysis.check
// @pinker-nav:domain analysis
// @pinker-nav:layer editor
// @pinker-nav:summary parse_and_check_program: a free function (outside the impl) that tokenizes, parses and runs semantic::check_program over a source string, used ONLY by :ast (via run_ast_command) as a preview step — it does not alter the editor's persistent `self.lines`/AST, it only produces the Program in memory for rendering in the panel; :tokens (run_tokens_command) uses only Lexer::tokenize directly and does not call this function.
fn parse_and_check_program(source: &str) -> Result<Program, crate::error::PinkerError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    semantic::check_program(&program)?;
    Ok(program)
}
// @pinker-nav:end editor.analysis.check

// @pinker-nav:start evidence.editor.file-session
// @pinker-nav:domain editor
// @pinker-nav:layer evidence
// @pinker-nav:summary Proofs of the TUI editor: opening an existing file loads its content, a nonexistent file fails instead of creating an empty session, the tokens command produces output and the line-change command changes the indicated line.
#[cfg(test)]
mod tests {
    use super::EditorTui;
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
    fn editor_abre_arquivo_existente() {
        let path = temp_file_path("pinker_editor_open");
        std::fs::write(
            &path,
            "pacote main;\ncarinho principal() -> bombom { mimo 0; }",
        )
        .expect("write fixture");

        let editor = EditorTui::from_path(path.as_path()).expect("open editor");
        assert_eq!(editor.lines.len(), 2);

        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn editor_falha_para_arquivo_inexistente() {
        let path = temp_file_path("pinker_editor_missing");
        let result = EditorTui::from_path(path.as_path());
        assert!(result.is_err());
        let error = result.err().unwrap_or_default();
        assert!(error.contains("Falha ao abrir"));
    }

    #[test]
    fn editor_comando_tokens_produz_saida() {
        let path = temp_file_path("pinker_editor_tokens");
        std::fs::write(
            &path,
            "pacote main;\ncarinho principal() -> bombom { mimo 0; }",
        )
        .expect("write fixture");

        let mut editor = EditorTui::from_path(path.as_path()).expect("open editor");
        let keep_running = editor.execute_command(":tokens").expect("run tokens");
        assert!(keep_running);
        assert!(editor.output.iter().any(|line| line.starts_with("TOKENS:")));

        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn editor_comando_set_altera_linha() {
        let path = temp_file_path("pinker_editor_set");
        std::fs::write(&path, "linha1\nlinha2").expect("write fixture");

        let mut editor = EditorTui::from_path(path.as_path()).expect("open editor");
        editor.execute_command(":set 2 alterada").expect("set line");
        assert_eq!(editor.lines[1], "alterada");

        std::fs::remove_file(path).expect("cleanup");
    }
}
// @pinker-nav:end evidence.editor.file-session
