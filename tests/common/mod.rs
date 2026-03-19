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

pub fn render_cli_pseudo_asm_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== PSEUDO ASM ===\n");
    out.push_str(&render_backend_text(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}

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
