use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
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
    print_pseudo_asm: bool,
    check_only: bool,
}

fn usage(binary: &str) -> String {
    format!(
        "Uso: {binary} [--tokens] [--ast] [--json-ast] [--ir] [--cfg-ir] [--pseudo-asm] [--check] <arquivo.pink>\n\
         \n\
         Modos:\n\
           --tokens    imprime a lista de tokens com spans\n\
           --ast       imprime a AST textual legível\n\
           --json-ast  imprime a AST em JSON estável\n\
           --ir        imprime a IR estruturada após parsing + semântica\n\
           --cfg-ir    imprime a IR em blocos rotulados e saltos explícitos\n\
           --pseudo-asm imprime backend textual pseudo-assembly baseado na CFG IR\n\
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
    let mut print_pseudo_asm = false;
    let mut check_only = false;

    let binary = env::args().next().unwrap_or_else(|| "pink".to_string());
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--tokens" => print_tokens = true,
            "--ast" => print_ast = true,
            "--json-ast" => print_json_ast = true,
            "--ir" => print_ir = true,
            "--cfg-ir" => print_cfg_ir = true,
            "--pseudo-asm" => print_pseudo_asm = true,
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
        print_pseudo_asm,
        check_only,
    })
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

    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(tokens) => tokens,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    if config.print_tokens && !config.check_only {
        println!("=== TOKENS ===");
        for token in &tokens {
            println!("{} '{}' [{}]", token.kind.name(), token.lexeme, token.span);
        }
    }

    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => program,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    if config.print_ast && !config.check_only {
        println!("=== AST TEXTUAL ===");
        print!("{}", printer::render_program(&program));
    }

    if config.print_json_ast && !config.check_only {
        println!("=== AST JSON ===");
        println!("{}", printer::render_program_json(&program));
    }

    match semantic::check_program(&program) {
        Ok(()) => {
            let program_ir = if !config.check_only
                && (config.print_ir || config.print_cfg_ir || config.print_pseudo_asm)
            {
                let lowered = match ir::lower_program(&program) {
                    Ok(program_ir) => program_ir,
                    Err(err) => {
                        eprintln!("{}", err);
                        std::process::exit(1);
                    }
                };

                if let Err(err) = ir_validate::validate_program(&lowered) {
                    eprintln!("{}", err);
                    std::process::exit(1);
                }

                Some(lowered)
            } else {
                None
            };

            if config.print_ir && !config.check_only {
                println!("=== IR ===");
                print!("{}", ir::render_program(program_ir.as_ref().unwrap()));
            }

            let cfg_ir_program =
                if !config.check_only && (config.print_cfg_ir || config.print_pseudo_asm) {
                    let cfg = match cfg_ir::lower_program(program_ir.as_ref().unwrap()) {
                        Ok(cfg) => cfg,
                        Err(err) => {
                            eprintln!("{}", err);
                            std::process::exit(1);
                        }
                    };
                    if let Err(err) = cfg_ir_validate::validate_program(&cfg) {
                        eprintln!("{}", err);
                        std::process::exit(1);
                    }
                    Some(cfg)
                } else {
                    None
                };

            if config.print_cfg_ir && !config.check_only {
                println!("=== CFG IR ===");
                print!(
                    "{}",
                    cfg_ir::render_program(cfg_ir_program.as_ref().unwrap())
                );
            }

            if config.print_pseudo_asm && !config.check_only {
                let lowered_backend =
                    match backend_text::lower_program(cfg_ir_program.as_ref().unwrap()) {
                        Ok(lowered_backend) => lowered_backend,
                        Err(err) => {
                            eprintln!("{}", err);
                            std::process::exit(1);
                        }
                    };
                if let Err(err) = backend_text_validate::validate_program(&lowered_backend) {
                    eprintln!("{}", err);
                    std::process::exit(1);
                }
                println!("=== PSEUDO ASM ===");
                print!("{}", backend_text::render_program(&lowered_backend));
            }

            if !config.check_only {
                println!("Análise semântica concluída sem erros.");
            }
        }
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    }
}
