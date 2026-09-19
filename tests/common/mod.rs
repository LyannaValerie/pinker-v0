#![allow(dead_code)]

pub mod fonte_de_modulo;
pub mod native_process;
pub mod rust_source;
#[allow(unused_imports)]
pub use native_process::{ControlledCommand, NativeArtifactDir};

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
use std::path::PathBuf;

// @pinker-nav:start evidence.frontend.basic-pipeline
// @pinker-nav:domain frontend
// @pinker-nav:layer evidence
// @pinker-nav:summary Defines the three shared basic frontend helpers used by the suites: tokenize (source -> Lexer -> tokens), parse (tokens -> Parser -> AST) and parse_and_check (parse followed by a semantic check via semantic::check_program).
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
// @pinker-nav:end evidence.frontend.basic-pipeline

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

// @pinker-nav:start evidence.backend-text.pipeline-helper
// @pinker-nav:domain backend-text
// @pinker-nav:layer evidence
// @pinker-nav:summary Runs the shared helper render_backend_text: parse and semantic check, lowering and validation through IR, CFG and selection, lowering and validation of the textual backend and final rendering of the pseudo-assembly. It is an in-memory pipeline, not a CLI process nor the native backend.
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
// @pinker-nav:end evidence.backend-text.pipeline-helper

// @pinker-nav:start evidence.backend-text.cli-presentation-helper
// @pinker-nav:domain backend-text
// @pinker-nav:layer evidence
// @pinker-nav:summary Assembles the synthetic presentation of the render_cli_pseudo_asm_output helper in memory: it adds the `=== PSEUDO ASM ===` header, the text of render_backend_text and the historical footer `Análise semântica concluída sem erros.`. It neither creates nor executes a CLI process.
pub fn render_cli_pseudo_asm_output(code: &str) -> Result<String, PinkerError> {
    let mut out = String::new();
    out.push_str("=== PSEUDO ASM ===\n");
    out.push_str(&render_backend_text(code)?);
    out.push_str("Análise semântica concluída sem erros.\n");
    Ok(out)
}
// @pinker-nav:end evidence.backend-text.cli-presentation-helper

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

// @pinker-nav:start evidence.backend-s.pipeline-helper
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Runs the shared helper render_backend_s entirely in memory: parse and semantic check, lowering and validation through IR, CFG and selection, followed by the emission of the textual .s backend via emit_from_selected. It does not use the external subset helper, an assembler, a linker or native execution.
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
// @pinker-nav:end evidence.backend-s.pipeline-helper

// @pinker-nav:start evidence.external-backend-s.pipeline-helper
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Runs the shared helper render_backend_s_external_subset entirely in memory: parse and semantic check, lowering and validation through IR, CFG and selection, followed by the hosted assemblable emission via emit_external_toolchain_subset, which uses runtime_init=false. It invokes no assembler, linker or binary; the external tools are called only by real-flow tests that consume its output.
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

// @pinker-nav:end evidence.external-backend-s.pipeline-helper

// @pinker-nav:start evidence.backend-s.cli-presentation-helper
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Assembles the synthetic presentation of render_cli_asm_s_output in memory: it concatenates the `=== ASM .S (TEXTUAL) ===` header, the output of render_backend_s and the historical semantic-success footer. It neither creates nor executes a CLI process.
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
// @pinker-nav:end evidence.backend-s.cli-presentation-helper

// @pinker-nav:start evidence.native.capacity
// @pinker-nav:domain testing
// @pinker-nav:layer evidence
// @pinker-nav:summary Centralizes native evidence capability: platform, C driver and optional staticlib are classified with enumerated reasons; every skip emits a canonical JSON ledger and PINKER_EXIGE_NATIVO=1 turns absence into a failure.
pub fn render_backend_s_external_subset_nativo(code: &str) -> Result<String, PinkerError> {
    let program = parse(code)?;
    semantic::check_program(&program)?;
    let program_ir = ir::lower_program(&program)?;
    ir_validate::validate_program(&program_ir)?;
    let cfg = cfg_ir::lower_program(&program_ir)?;
    cfg_ir_validate::validate_program(&cfg)?;
    let selected = instr_select::lower_program(&cfg)?;
    instr_select_validate::validate_program(&selected)?;
    backend_s::emit_external_toolchain_subset_nativo(&selected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEvidenceCapability {
    Executable {
        driver: String,
        runtime_lib: Option<PathBuf>,
    },
    Skipped {
        reason: &'static str,
    },
    Unavailable {
        reason: &'static str,
    },
}

fn native_cc_driver() -> Option<String> {
    ["cc", "gcc", "clang"].iter().find_map(|candidate| {
        let probe = ControlledCommand::new(candidate)
            .arg("--version")
            .output()
            .ok()?;
        probe.status.success().then(|| (*candidate).to_string())
    })
}

fn native_runtime_lib() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PINKER_RT_LIB").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }

    let executable = std::env::current_exe().ok()?;
    let deps = executable.parent()?;
    for directory in [Some(deps), deps.parent()].into_iter().flatten() {
        let candidate = directory.join("libpinker_rt.a");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn native_evidence_capability(needs_runtime: bool) -> NativeEvidenceCapability {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return NativeEvidenceCapability::Skipped {
            reason: "unsupported_platform",
        };
    }
    let Some(driver) = native_cc_driver() else {
        return NativeEvidenceCapability::Unavailable {
            reason: "cc_not_found",
        };
    };
    let runtime_lib = needs_runtime.then(native_runtime_lib).flatten();
    if needs_runtime && runtime_lib.is_none() {
        return NativeEvidenceCapability::Unavailable {
            reason: "runtime_library_not_found",
        };
    }
    NativeEvidenceCapability::Executable {
        driver,
        runtime_lib,
    }
}

pub fn require_native_evidence(
    test: &str,
    needs_runtime: bool,
) -> Option<(String, Option<PathBuf>)> {
    match native_evidence_capability(needs_runtime) {
        NativeEvidenceCapability::Executable {
            driver,
            runtime_lib,
        } => Some((driver, runtime_lib)),
        NativeEvidenceCapability::Skipped { reason } => {
            eprintln!(
                "{{\"event\":\"native_evidence\",\"reason\":\"{reason}\",\"status\":\"skipped\",\"test\":\"{test}\"}}"
            );
            assert_ne!(
                std::env::var("PINKER_EXIGE_NATIVO").as_deref(),
                Ok("1"),
                "evidência nativa obrigatória indisponível: {reason} ({test})"
            );
            None
        }
        NativeEvidenceCapability::Unavailable { reason } => {
            eprintln!(
                "{{\"event\":\"native_evidence\",\"reason\":\"{reason}\",\"status\":\"unavailable\",\"test\":\"{test}\"}}"
            );
            assert_ne!(
                std::env::var("PINKER_EXIGE_NATIVO").as_deref(),
                Ok("1"),
                "evidência nativa obrigatória indisponível: {reason} ({test})"
            );
            None
        }
    }
}
// @pinker-nav:end evidence.native.capacity
