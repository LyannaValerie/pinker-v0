use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::lexer::Lexer;
use pinker_v0::parser::Parser;
use pinker_v0::printer;
use pinker_v0::semantic;
use std::env;
use std::fs;

struct Config {
    input: String,
    print_tokens: bool,
    print_ast: bool,
    print_json_ast: bool,
    print_ir: bool,
    print_cfg_ir: bool,
    print_selected: bool,
    print_machine: bool,
    print_pseudo_asm: bool,
    run_program: bool,
    check_only: bool,
}

fn usage(binary: &str) -> String {
    format!(
        "Uso: {binary} [--tokens] [--ast] [--json-ast] [--ir] [--cfg-ir] [--selected] [--machine] [--pseudo-asm] [--run] [--check] <arquivo.pink>\n\
         \n\
         Modos:\n\
           --tokens    imprime a lista de tokens com spans\n\
           --ast       imprime a AST textual legível\n\
           --json-ast  imprime a AST em JSON estável\n\
           --ir        imprime a IR estruturada após parsing + semântica\n\
           --cfg-ir    imprime a IR em blocos rotulados e saltos explícitos\n\
           --selected  imprime a camada de seleção de instruções textual\n\
           --machine   imprime o alvo textual abstrato (máquina de pilha)\n\
           --pseudo-asm imprime backend textual pseudo-assembly final\n\
           --run       interpreta a machine validada e executa principal\n\
           --check     executa apenas a validação semântica\n"
    )
}

fn parse_args() -> Result<Config, String> {
    let mut input = None;
    let mut print_tokens = false;
    let mut print_ast = false;
    let mut print_json_ast = false;
    let mut print_ir = false;
    let mut print_cfg_ir = false;
    let mut print_selected = false;
    let mut print_machine = false;
    let mut print_pseudo_asm = false;
    let mut run_program = false;
    let mut check_only = false;

    let binary = env::args().next().unwrap_or_else(|| "pink".to_string());
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--tokens" => print_tokens = true,
            "--ast" => print_ast = true,
            "--json-ast" => print_json_ast = true,
            "--ir" => print_ir = true,
            "--cfg-ir" => print_cfg_ir = true,
            "--selected" => print_selected = true,
            "--machine" => print_machine = true,
            "--pseudo-asm" => print_pseudo_asm = true,
            "--run" => run_program = true,
            "--check" => check_only = true,
            "--help" | "-h" => return Err(usage(&binary)),
            _ if arg.starts_with("--") => {
                return Err(format!(
                    "Flag desconhecida: '{}'\n\n{}",
                    arg,
                    usage(&binary)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado.\n\n{}",
                        usage(&binary)
                    ));
                }
                input = Some(arg);
            }
        }
    }

    let Some(input) = input else {
        return Err(usage(&binary));
    };

    Ok(Config {
        input,
        print_tokens,
        print_ast,
        print_json_ast,
        print_ir,
        print_cfg_ir,
        print_selected,
        print_machine,
        print_pseudo_asm,
        run_program,
        check_only,
    })
}

/// Macro para encurtar o padrão "try or exit(1)" repetido no pipeline.
macro_rules! try_or_exit {
    ($result:expr, $source:expr) => {
        match $result {
            Ok(val) => val,
            Err(err) => {
                eprintln!("{}", err.render_for_cli_with_source($source));
                std::process::exit(1);
            }
        }
    };
}

fn main() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    let source = match fs::read_to_string(&config.input) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Falha ao ler '{}': {}", config.input, err);
            std::process::exit(1);
        }
    };

    // --- Frontend: léxico + parsing ---
    let mut lexer = Lexer::new(&source);
    let tokens = try_or_exit!(lexer.tokenize(), &source);

    if config.print_tokens && !config.check_only {
        println!("=== TOKENS ===");
        for token in &tokens {
            println!("{} '{}' [{}]", token.kind.name(), token.lexeme, token.span);
        }
    }

    let mut parser = Parser::new(tokens);
    let program = try_or_exit!(parser.parse(), &source);

    if config.print_ast && !config.check_only {
        println!("=== AST TEXTUAL ===");
        print!("{}", printer::render_program(&program));
    }

    if config.print_json_ast && !config.check_only {
        println!("=== AST JSON ===");
        println!("{}", printer::render_program_json(&program));
    }

    // --- Semântica ---
    try_or_exit!(semantic::check_program(&program), &source);

    if config.check_only {
        return;
    }

    // Booleanos de necessidade do pipeline — cada fase só executa se algum
    // modo de saída a jusante a exigir. Adicionar um novo modo exige tocar
    // apenas a linha correspondente aqui.
    let needs_ir = config.print_ir
        || config.print_cfg_ir
        || config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program;
    let needs_cfg = config.print_cfg_ir
        || config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program;
    let needs_selected = config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program;
    let needs_machine = config.print_machine || config.print_pseudo_asm || config.run_program;

    // --- IR estruturada ---
    let program_ir = if needs_ir {
        let lowered = try_or_exit!(ir::lower_program(&program), &source);
        try_or_exit!(ir_validate::validate_program(&lowered), &source);
        Some(lowered)
    } else {
        None
    };

    if config.print_ir {
        println!("=== IR ===");
        print!("{}", ir::render_program(program_ir.as_ref().unwrap()));
    }

    // --- CFG IR ---
    let cfg_ir_program = if needs_cfg {
        let cfg = try_or_exit!(cfg_ir::lower_program(program_ir.as_ref().unwrap()), &source);
        try_or_exit!(cfg_ir_validate::validate_program(&cfg), &source);
        Some(cfg)
    } else {
        None
    };

    if config.print_cfg_ir {
        println!("=== CFG IR ===");
        print!(
            "{}",
            cfg_ir::render_program(cfg_ir_program.as_ref().unwrap())
        );
    }

    // --- Seleção de instruções ---
    let selected_program = if needs_selected {
        let selected = try_or_exit!(
            instr_select::lower_program(cfg_ir_program.as_ref().unwrap()),
            &source
        );
        try_or_exit!(instr_select_validate::validate_program(&selected), &source);
        Some(selected)
    } else {
        None
    };

    if config.print_selected {
        println!("=== SELECTED ===");
        print!(
            "{}",
            instr_select::render_program(selected_program.as_ref().unwrap())
        );
    }

    // --- Machine abstrata ---
    let machine_program = if needs_machine {
        let machine = try_or_exit!(
            abstract_machine::lower_program(selected_program.as_ref().unwrap()),
            &source
        );
        try_or_exit!(
            abstract_machine_validate::validate_program(&machine),
            &source
        );
        Some(machine)
    } else {
        None
    };

    if config.print_machine {
        println!("=== MACHINE ===");
        print!(
            "{}",
            abstract_machine::render_program(machine_program.as_ref().unwrap())
        );
    }

    // --- Execução via interpretador ---
    if config.run_program {
        let result = try_or_exit!(
            interpreter::run_program(machine_program.as_ref().unwrap()),
            &source
        );
        if let Some(interpreter::RuntimeValue::Int(v)) = result {
            println!("{}", v);
        }
    }

    // --- Backend textual (pseudo-asm) ---
    // Nota (HF-6): `--pseudo-asm` parte de `selected_program` (não de `machine_program`),
    // enquanto `--run` parte de `machine_program`. Essa bifurcação é intencional:
    // o backend textual é uma representação alternativa da seleção de instruções,
    // e o interpretador precisa da Machine validada para execução.
    if config.print_pseudo_asm {
        let lowered_backend = try_or_exit!(
            backend_text::lower_selected_program(selected_program.as_ref().unwrap()),
            &source
        );
        try_or_exit!(
            backend_text_validate::validate_program(&lowered_backend),
            &source
        );
        println!("=== PSEUDO ASM ===");
        print!("{}", backend_text::render_program(&lowered_backend));
    }

    // HF-15: só imprime mensagem de sucesso quando nenhuma flag de saída foi ativa.
    let any_output = config.print_tokens
        || config.print_ast
        || config.print_json_ast
        || config.print_ir
        || config.print_cfg_ir
        || config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program;
    if !any_output {
        println!("Análise semântica concluída sem erros.");
    }
}
