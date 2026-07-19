#![allow(dead_code)]

use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::error::PinkerError;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::lexer::Lexer;
use pinker_v0::parser::Parser;
use pinker_v0::printer;
use pinker_v0::semantic;

// @pinker-nav:start evidencia.frontend.pipeline-basico
// @pinker-nav:domain frontend
// @pinker-nav:layer evidencia
// @pinker-nav:summary Define os três helpers básicos compartilhados do frontend usados pelas suítes: tokenize (source -> Lexer -> tokens), parse (tokens -> Parser -> AST) e parse_and_check (parse seguido de checagem semântica via semantic::check_program).
pub fn tokenize(code: &str) -> Result<Vec<pinker_v0::token::Token>, PinkerError> {
    let mut lexer = Lexer::new(code);
    lexer.tokenize()
}

pub fn parse(code: &str) -> Result<pinker_v0::ast::Program, PinkerError> {
    let tokens = tokenize(code)?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

pub fn parse_and_check(code: &str) -> Result<(), PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)
}
// @pinker-nav:end evidencia.frontend.pipeline-basico

pub fn render_ast(code: &str) -> Result<String, PinkerError> {
    Ok(printer::render_program(&parse(code)?))
}

pub fn render_json_ast(code: &str) -> Result<String, PinkerError> {
    Ok(printer::render_program_json(&parse(code)?))
}

pub fn render_ir(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    Ok(ir::render_program(&program_ir))
}

pub fn render_cfg_ir(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    Ok(cfg_ir::render_program(&cfg))
}

pub fn render_cli_ir_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== IR ===\n");
    out.push_str(&render_ir(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}

pub fn render_cli_cfg_ir_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== CFG IR ===\n");
    out.push_str(&render_cfg_ir(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}

// @pinker-nav:start evidencia.backend-text.pipeline-helper
// @pinker-nav:domain backend-text
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa o helper compartilhado render_backend_text: parse e checagem semântica, lowering e validação por IR, CFG e seleção, lowering e validação do backend textual e renderização final do pseudo-assembly. É pipeline em memória, não processo CLI nem backend nativo.
pub fn render_backend_text(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    let backend = backend_text::lower_selected_program(&selected)?;
    backend_text_validate::validate_program(&backend)?;
    Ok(backend_text::render_program(&backend))
}
// @pinker-nav:end evidencia.backend-text.pipeline-helper

// @pinker-nav:start evidencia.backend-text.apresentacao-cli-helper
// @pinker-nav:domain backend-text
// @pinker-nav:layer evidencia
// @pinker-nav:summary Monta a apresentação sintética do helper render_cli_pseudo_asm_output em memória: acrescenta o cabeçalho `=== PSEUDO ASM ===`, o texto de render_backend_text e o rodapé histórico `Análise semântica concluída sem erros.`. Não cria nem executa um processo CLI.
pub fn render_cli_pseudo_asm_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== PSEUDO ASM ===\n");
    out.push_str(&render_backend_text(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}
// @pinker-nav:end evidencia.backend-text.apresentacao-cli-helper

pub fn render_selected(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    Ok(instr_select::render_program(&selected))
}

pub fn render_cli_selected_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== SELECTED ===\n");
    out.push_str(&render_selected(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}

pub fn render_machine(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    let machine = abstract_machine::lower_program(&selected)?;
    abstract_machine_validate::validate_program(&machine)?;
    Ok(abstract_machine::render_program(&machine))
}

pub fn render_cli_machine_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== MACHINE ===\n");
    out.push_str(&render_machine(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}

// @pinker-nav:start evidencia.backend-s.pipeline-helper
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa o helper compartilhado render_backend_s inteiramente em memória: parse e checagem semântica, lowering e validação por IR, CFG e seleção, seguidos da emissão do backend .s textual via emit_from_selected. Não usa o helper do subset externo, assembler, linker nem execução nativa.
pub fn render_backend_s(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    backend_s::emit_from_selected(&selected)
}
// @pinker-nav:end evidencia.backend-s.pipeline-helper

pub fn render_backend_s_external_subset(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    backend_s::emit_external_toolchain_subset(&selected)
}

// @pinker-nav:start evidencia.backend-s.apresentacao-cli-helper
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Monta a apresentação sintética de render_cli_asm_s_output em memória: concatena o cabeçalho `=== ASM .S (TEXTUAL) ===`, a saída de render_backend_s e o rodapé histórico de sucesso semântico. Não cria nem executa um processo CLI.
pub fn render_cli_asm_s_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str(
        "=== ASM .S (TEXTUAL) ===
",
    );
    out.push_str(&render_backend_s(code)?);
    out.push_str(
        "Análise semântica concluída sem erros.
",
    );
    Ok(out)
}
// @pinker-nav:end evidencia.backend-s.apresentacao-cli-helper
