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

// @pinker-nav:start repl.ciclo.leitura-avaliacao
// @pinker-nav:domain fluxo
// @pinker-nav:layer repl
// @pinker-nav:summary Laço leitura-avaliação-impressão do REPL: lê uma linha, trata `:quit`/`:sair` e EOF, avalia o trecho e imprime o resultado ou o erro sem manter estado entre linhas.
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
    writeln!(
        out,
        "Intrínseca exige o import na MESMA linha: `trazer texto.tamanho; falar(tamanho(\"abc\"));`."
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
        RuntimeValue::MapBombomBombom(handle) => format!("<mapa:bombom,bombom:{handle}>"),
        RuntimeValue::MapBombomVerso(handle) => format!("<mapa:bombom,verso:{handle}>"),
        RuntimeValue::Map(handle) => format!("<mapa:generico:{handle}>"),
        RuntimeValue::Callable(handle) => format!("<carinho:{handle}>"),
        RuntimeValue::SaidaProcesso(handle) => format!("<SaidaProcesso:{handle}>"),
        RuntimeValue::ValorJson(handle) => format!("<ValorJson:{handle}>"),
    }
}
// @pinker-nav:end repl.ciclo.leitura-avaliacao

// @pinker-nav:start repl.avaliacao.pipeline
// @pinker-nav:domain fluxo
// @pinker-nav:layer repl
// @pinker-nav:summary Envolve a linha do REPL como corpo temporário de `principal` e a conduz por todo o pipeline (léxico, parser, semântica, IR, CFG, seleção, máquina e interpretador), devolvendo o valor produzido.
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

/// Envolve o trecho do usuário num programa completo.
///
/// #505: com a superfície intrínseca global removida, chamar qualquer
/// intrínseca no REPL exige `trazer`, e `trazer` é item de topo — não cabe
/// dentro do corpo de `principal`. Sem separar as duas partes, o REPL ficaria
/// reduzido a `falar` e aritmética, e o diagnóstico ainda mandaria o usuário
/// escrever um `trazer` que a própria forma não aceitaria.
///
/// Os `trazer` iniciais do trecho sobem para o topo; o resto continua indo
/// para o corpo, exatamente como antes.
fn wrap_snippet(snippet: &str) -> String {
    let (imports, corpo) = separar_imports(snippet);
    let mut source = String::from("pacote main;\n");
    for import in &imports {
        source.push_str(import);
        source.push('\n');
    }
    source.push_str("carinho principal() -> bombom {\n    ");
    source.push_str(&corpo);
    source.push('\n');
    if !snippet_has_explicit_return(&corpo) {
        source.push_str("    mimo 0;\n");
    }
    source.push_str("}\n");
    source
}

/// Separa os `trazer` iniciais do restante do trecho.
///
/// O REPL não guarda estado entre linhas (Fase 167), então o import precisa
/// vir na MESMA linha da chamada: `trazer texto.tamanho; falar(tamanho("a"));`.
/// Por isso a separação é por statement e não por linha. Só os do INÍCIO
/// sobem: um `trazer` no meio do trecho continua sendo erro de sintaxe, que é
/// a verdade sobre onde a declaração pode aparecer.
fn separar_imports(snippet: &str) -> (Vec<String>, String) {
    let mut imports = Vec::new();
    let mut resto = snippet.trim_start();
    while let Some(depois) = resto.strip_prefix("trazer ") {
        let Some(fim) = depois.find(';') else {
            break;
        };
        imports.push(format!("trazer {};", depois[..fim].trim()));
        resto = depois[fim + 1..].trim_start();
    }
    (imports, resto.to_string())
}

fn snippet_has_explicit_return(snippet: &str) -> bool {
    snippet.contains("mimo")
}
// @pinker-nav:end repl.avaliacao.pipeline

#[cfg(test)]
mod tests {
    use super::{is_exit_command, wrap_snippet};

    /// #505: o REPL precisa aceitar o import, ou nenhuma intrínseca é
    /// alcançável nele. O teste mede o programa montado, não a string do
    /// wrapper: `trazer` tem de sair do corpo e subir para o topo.
    #[test]
    fn repl_iça_o_import_para_o_topo_do_programa() {
        let fonte = wrap_snippet("trazer texto.tamanho; falar(tamanho(\"abc\"));");
        let topo = fonte
            .find("trazer texto.tamanho;")
            .expect("import precisa existir");
        let corpo = fonte
            .find("carinho principal()")
            .expect("corpo precisa existir");
        assert!(topo < corpo, "o import ficou dentro do corpo:\n{fonte}");
        assert!(
            !fonte.contains("    trazer"),
            "import indentado no corpo:\n{fonte}"
        );
        crate::semantic::check_program(
            &crate::parser::Parser::new(
                crate::lexer::Lexer::new(&fonte).tokenize().expect("lexer"),
            )
            .parse()
            .expect("parser"),
        )
        .expect("o programa montado precisa passar na semântica");
    }

    /// `trazer` no MEIO do trecho continua sendo erro: a separação é do
    /// cabeçalho, não uma licença para mover declaração de qualquer lugar.
    #[test]
    fn repl_nao_ica_import_do_meio_do_trecho() {
        let fonte = wrap_snippet("falar(1); trazer texto.tamanho;");
        assert!(
            fonte.contains("    falar(1); trazer texto.tamanho;"),
            "{fonte}"
        );
    }

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
