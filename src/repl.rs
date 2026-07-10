use crate::abstract_machine;
use crate::abstract_machine_validate;
use crate::cfg_ir;
use crate::cfg_ir_validate;
use crate::instr_select;
use crate::instr_select_validate;
use crate::interpreter::{self, RuntimeValue};
use crate::ir;
use crate::ir_validate;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::semantic;
use std::io::{self, BufRead, Write};

const PROMPT: &str = "pinker> ";

pub fn run_repl() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut reader = stdin.lock();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    run_repl_with_io(&mut reader, &mut out, &mut err)
}

pub fn run_repl_with_io<R: BufRead, W: Write, E: Write>(
    reader: &mut R,
    out: &mut W,
    err: &mut E,
) -> Result<(), String> {
    writeln!(out, "=== Pinker REPL ===").map_err(|e| e.to_string())?;
    writeln!(
        out,
        "Fase 167: cada linha vira o corpo temporário de `principal`; sem estado entre linhas."
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "Use `falar(...)` para inspecionar saída, `mimo ...;` para retorno explícito e `:quit` para sair."
    )
    .map_err(|e| e.to_string())?;

    loop {
        write!(out, "{PROMPT}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        let bytes = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if bytes == 0 {
            writeln!(out).map_err(|e| e.to_string())?;
            writeln!(out, "Encerrando REPL Pinker.").map_err(|e| e.to_string())?;
            return Ok(());
        }

        let snippet = line.trim();
        if snippet.is_empty() {
            continue;
        }
        if is_exit_command(snippet) {
            writeln!(out, "Encerrando REPL Pinker.").map_err(|e| e.to_string())?;
            return Ok(());
        }

        match evaluate_snippet(snippet) {
            Ok(value) => {
                if should_print_result(snippet, &value) {
                    writeln!(out, "=> {}", render_value(&value)).map_err(|e| e.to_string())?;
                } else {
                    writeln!(out, "ok").map_err(|e| e.to_string())?;
                }
            }
            Err(message) => {
                writeln!(err, "{message}").map_err(|e| e.to_string())?;
            }
        }
    }
}

fn is_exit_command(snippet: &str) -> bool {
    matches!(snippet, ":quit" | ":sair")
}

fn should_print_result(snippet: &str, value: &RuntimeValue) -> bool {
    if snippet_has_explicit_return(snippet) {
        return true;
    }
    !matches!(value, RuntimeValue::Int(0))
}

fn render_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Int(v) => v.to_string(),
        RuntimeValue::IntSigned(v) => v.to_string(),
        RuntimeValue::Ptr(v) => v.to_string(),
        RuntimeValue::Bool(v) => v.to_string(),
        RuntimeValue::Str(v) => v.clone(),
        RuntimeValue::ListBombom(handle) => format!("<lista:bombom:{handle}>"),
        RuntimeValue::ListVerso(handle) => format!("<lista:verso:{handle}>"),
        RuntimeValue::MapVersoBombom(handle) => format!("<mapa:verso,bombom:{handle}>"),
        RuntimeValue::MapVersoVerso(handle) => format!("<mapa:verso,verso:{handle}>"),
    }
}

fn evaluate_snippet(snippet: &str) -> Result<RuntimeValue, String> {
    let source = wrap_snippet(snippet);
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    let mut parser = Parser::new(tokens);
    let program = parser
        .parse()
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    semantic::check_program(&program).map_err(|err| err.render_for_cli_with_source(&source))?;
    let program_ir =
        ir::lower_program(&program).map_err(|err| err.render_for_cli_with_source(&source))?;
    ir_validate::validate_program(&program_ir)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    let cfg = cfg_ir::lower_program(&program_ir)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    cfg_ir_validate::validate_program(&cfg)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    let selected =
        instr_select::lower_program(&cfg).map_err(|err| err.render_for_cli_with_source(&source))?;
    instr_select_validate::validate_program(&selected)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    let machine = abstract_machine::lower_program(&selected)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    abstract_machine_validate::validate_program(&machine)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    let result = interpreter::run_program(&machine)
        .map_err(|err| err.render_for_cli_with_source(&source))?;
    Ok(result.unwrap_or(RuntimeValue::Int(0)))
}

fn wrap_snippet(snippet: &str) -> String {
    let mut source = String::from("pacote main;\ncarinho principal() -> bombom {\n    ");
    source.push_str(snippet);
    source.push('\n');
    if !snippet_has_explicit_return(snippet) {
        source.push_str("    mimo 0;\n");
    }
    source.push_str("}\n");
    source
}

fn snippet_has_explicit_return(snippet: &str) -> bool {
    snippet.contains("mimo")
}

#[cfg(test)]
mod tests {
    use super::{is_exit_command, wrap_snippet};

    #[test]
    fn repl_reconhece_comando_de_saida_minimo() {
        assert!(is_exit_command(":quit"));
        assert!(is_exit_command(":sair"));
        assert!(!is_exit_command(":help"));
    }

    #[test]
    fn repl_envolve_snippet_em_principal_temporaria() {
        let source = wrap_snippet("falar(42);");
        assert!(source.contains("pacote main;"));
        assert!(source.contains("carinho principal() -> bombom"));
        assert!(source.contains("falar(42);"));
        assert!(source.contains("mimo 0;"));
    }
}
