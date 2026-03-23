use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
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
use pinker_v0::token::Span;
use pinker_v0::{ast, error::PinkerError};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    print_asm_s: bool,
    run_program: bool,
    check_only: bool,
}

struct BuildConfig {
    input: String,
    out_dir: String,
}

enum CliCommand {
    Analyze(Config),
    Build(BuildConfig),
}

fn usage(binary: &str) -> String {
    format!(
        "Uso: {binary} [--tokens] [--ast] [--json-ast] [--ir] [--cfg-ir] [--selected] [--machine] [--pseudo-asm] [--asm-s] [--run] [--check] <arquivo.pink>\n\
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
           --asm-s     imprime backend textual `.s` (Fase 54, ABI textual mínima)\n\
           --run       interpreta a machine validada e executa principal\n\
           --check     executa apenas a validação semântica\n"
    )
}

fn build_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} build [--out-dir <diretorio>] <arquivo.pink>\n\
         \n\
         Comando:\n\
           build      executa o pipeline de build e grava artefato `.s` no disco\n\
         \n\
         Opções:\n\
           --out-dir  diretório de saída (padrão: build)\n"
    )
}

fn parse_build_args(binary: &str, args: &[String]) -> Result<BuildConfig, String> {
    let mut input: Option<String> = None;
    let mut out_dir = "build".to_string();
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(build_usage(binary)),
            "--out-dir" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--out-dir' requer um valor.\n\n{}",
                        build_usage(binary)
                    ));
                }
                out_dir.clone_from(&args[i]);
            }
            _ if arg.starts_with("--") => {
                return Err(format!(
                    "Flag desconhecida no comando build: '{}'\n\n{}",
                    arg,
                    build_usage(binary)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado em 'build'.\n\n{}",
                        build_usage(binary)
                    ));
                }
                input = Some(arg.clone());
            }
        }
        i += 1;
    }

    let Some(input) = input else {
        return Err(build_usage(binary));
    };
    Ok(BuildConfig { input, out_dir })
}

fn parse_args() -> Result<CliCommand, String> {
    let mut input: Option<String> = None;
    let mut print_tokens = false;
    let mut print_ast = false;
    let mut print_json_ast = false;
    let mut print_ir = false;
    let mut print_cfg_ir = false;
    let mut print_selected = false;
    let mut print_machine = false;
    let mut print_pseudo_asm = false;
    let mut run_program = false;
    let mut print_asm_s = false;
    let mut check_only = false;

    let raw_args: Vec<String> = env::args().collect();
    let binary = raw_args
        .first()
        .cloned()
        .unwrap_or_else(|| "pink".to_string());
    let cli_args = &raw_args[1..];

    if let Some(cmd) = cli_args.first() {
        if cmd == "build" {
            return parse_build_args(&binary, &cli_args[1..]).map(CliCommand::Build);
        }
    }

    for arg in cli_args {
        match arg.as_str() {
            "--tokens" => print_tokens = true,
            "--ast" => print_ast = true,
            "--json-ast" => print_json_ast = true,
            "--ir" => print_ir = true,
            "--cfg-ir" => print_cfg_ir = true,
            "--selected" => print_selected = true,
            "--machine" => print_machine = true,
            "--pseudo-asm" => print_pseudo_asm = true,
            "--asm" | "--asm-s" | "--s" => print_asm_s = true,
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
                input = Some(arg.clone());
            }
        }
    }

    let Some(input) = input else {
        return Err(usage(&binary));
    };

    Ok(CliCommand::Analyze(Config {
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
        print_asm_s,
        check_only,
    }))
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
    let command = match parse_args() {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    match command {
        CliCommand::Analyze(config) => run_analyze(config),
        CliCommand::Build(config) => run_build(config),
    }
}

fn run_analyze(config: Config) {
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
    let parsed_program = try_or_exit!(parser.parse(), &source);
    let program = try_or_exit!(
        load_program_with_imports(&config.input, parsed_program),
        &source
    );

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
        || config.run_program
        || config.print_asm_s;
    let needs_cfg = config.print_cfg_ir
        || config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program
        || config.print_asm_s;
    let needs_selected = config.print_selected
        || config.print_machine
        || config.print_pseudo_asm
        || config.run_program
        || config.print_asm_s;
    let needs_machine = config.print_machine || config.run_program;

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

    // --- Backend textual `.s` (Fase 54) ---
    // Esta saída textual parte de `selected_program`, com ABI textual mínima interna
    // (ainda sem ABI/registradores reais de plataforma).
    if config.print_asm_s {
        let out = try_or_exit!(
            backend_s::emit_from_selected(selected_program.as_ref().unwrap()),
            &source
        );
        println!("=== ASM .S (TEXTUAL) ===");
        print!("{}", out);
    }

    // --- Execução via interpretador ---
    if config.run_program {
        let result = try_or_exit!(
            interpreter::run_program(machine_program.as_ref().unwrap()),
            &source
        );
        if let Some(value) = result {
            match value {
                interpreter::RuntimeValue::Int(v) => println!("{}", v),
                interpreter::RuntimeValue::IntSigned(v) => println!("{}", v),
                interpreter::RuntimeValue::Ptr(v) => println!("{}", v),
                interpreter::RuntimeValue::Bool(_) => {}
                interpreter::RuntimeValue::Str(v) => println!("{}", v),
            }
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
        || config.run_program
        || config.print_asm_s;
    if !any_output {
        println!("Análise semântica concluída sem erros.");
    }
}

fn run_build(config: BuildConfig) {
    let source = match fs::read_to_string(&config.input) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Falha ao ler '{}': {}", config.input, err);
            std::process::exit(1);
        }
    };

    let mut lexer = Lexer::new(&source);
    let tokens = try_or_exit!(lexer.tokenize(), &source);
    let mut parser = Parser::new(tokens);
    let parsed_program = try_or_exit!(parser.parse(), &source);
    let program = try_or_exit!(
        load_program_with_imports(&config.input, parsed_program),
        &source
    );
    try_or_exit!(semantic::check_program(&program), &source);

    let program_ir = try_or_exit!(ir::lower_program(&program), &source);
    try_or_exit!(ir_validate::validate_program(&program_ir), &source);
    let cfg_program = try_or_exit!(cfg_ir::lower_program(&program_ir), &source);
    try_or_exit!(cfg_ir_validate::validate_program(&cfg_program), &source);
    let selected_program = try_or_exit!(instr_select::lower_program(&cfg_program), &source);
    try_or_exit!(
        instr_select_validate::validate_program(&selected_program),
        &source
    );
    let output = try_or_exit!(backend_s::emit_from_selected(&selected_program), &source);

    let out_dir = PathBuf::from(&config.out_dir);
    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "Falha ao criar diretório de saída '{}': {}",
            out_dir.display(),
            err
        );
        std::process::exit(1);
    }

    let stem = Path::new(&config.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("saida");
    let output_path = out_dir.join(format!("{}.s", stem));
    if let Err(err) = fs::write(&output_path, output) {
        eprintln!(
            "Falha ao gravar artefato de build '{}': {}",
            output_path.display(),
            err
        );
        std::process::exit(1);
    }

    println!("Build concluído: {}", output_path.display());
}

fn parse_program_from_source(source: &str) -> Result<ast::Program, PinkerError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

fn importable_item_name(item: &ast::Item) -> Option<&str> {
    match item {
        ast::Item::Function(function) => Some(function.name.as_str()),
        ast::Item::Const(constant) => Some(constant.name.as_str()),
        _ => None,
    }
}

fn importable_item_clone(item: &ast::Item) -> Option<ast::Item> {
    match item {
        ast::Item::Function(_) | ast::Item::Const(_) => Some(item.clone()),
        ast::Item::TypeAlias(_) | ast::Item::Struct(_) => None,
    }
}

fn load_module_program(
    module: &str,
    base_dir: &Path,
    source_path: &Path,
    import_span: Span,
    loading: &mut Vec<String>,
    loaded: &mut HashMap<String, ast::Program>,
) -> Result<(), PinkerError> {
    if loaded.contains_key(module) {
        return Ok(());
    }
    if loading.iter().any(|entry| entry == module) {
        return Err(PinkerError::Semantic {
            msg: format!(
                "ciclo de módulos detectado: {} -> {}",
                loading.join(" -> "),
                module
            ),
            span: import_span,
        });
    }

    let module_path = base_dir.join(format!("{}.pink", module));
    let source = fs::read_to_string(&module_path).map_err(|_| PinkerError::Semantic {
        msg: format!(
            "módulo '{}' não encontrado a partir de '{}'",
            module,
            source_path.display()
        ),
        span: import_span,
    })?;
    let program = parse_program_from_source(&source).map_err(|err| match err {
        PinkerError::Lexer { msg, span }
        | PinkerError::Parse { msg, span }
        | PinkerError::Expected {
            expected: msg,
            span,
            ..
        }
        | PinkerError::Semantic { msg, span } => PinkerError::Semantic {
            msg: format!("falha ao ler módulo '{}': {}", module, msg),
            span,
        },
        other => other,
    })?;

    loading.push(module.to_string());
    for import in &program.imports {
        load_module_program(
            import.module.as_str(),
            base_dir,
            &module_path,
            import.span,
            loading,
            loaded,
        )?;
    }
    loading.pop();
    loaded.insert(module.to_string(), program);
    Ok(())
}

fn load_program_with_imports(
    source_file: &str,
    mut root_program: ast::Program,
) -> Result<ast::Program, PinkerError> {
    if root_program.imports.is_empty() {
        return Ok(root_program);
    }

    let source_path = PathBuf::from(source_file);
    let base_dir = source_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mut loaded = HashMap::new();
    let mut loading = Vec::new();
    let mut seen_imports = HashSet::new();
    let mut imported_items = Vec::new();
    let mut imported_names = HashMap::<String, Span>::new();
    let local_names: HashSet<String> = root_program
        .items
        .iter()
        .filter_map(importable_item_name)
        .map(ToOwned::to_owned)
        .collect();

    for import in &root_program.imports {
        let import_key = format!(
            "{}::{}",
            import.module,
            import.symbol.as_deref().unwrap_or("*")
        );
        if !seen_imports.insert(import_key) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "import duplicado para '{}{}'",
                    import.module,
                    import
                        .symbol
                        .as_ref()
                        .map(|symbol| format!(".{}", symbol))
                        .unwrap_or_default()
                ),
                span: import.span,
            });
        }

        load_module_program(
            import.module.as_str(),
            &base_dir,
            &source_path,
            import.span,
            &mut loading,
            &mut loaded,
        )?;
        let module_program = loaded
            .get(import.module.as_str())
            .expect("módulo carregado");

        if let Some(symbol) = &import.symbol {
            if local_names.contains(symbol) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "colisão de nome no import: '{}' já existe no arquivo principal",
                        symbol
                    ),
                    span: import.span,
                });
            }
            if let Some(previous_span) = imported_names.get(symbol) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "colisão de nome no import: '{}' trazido por múltiplos módulos",
                        symbol
                    ),
                    span: previous_span.merge(import.span),
                });
            }
            let Some(item) = module_program
                .items
                .iter()
                .find(|item| importable_item_name(item) == Some(symbol.as_str()))
            else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "símbolo '{}' não encontrado no módulo '{}'",
                        symbol, import.module
                    ),
                    span: import.span,
                });
            };
            imported_items.push(item.clone());
            imported_names.insert(symbol.clone(), import.span);
        } else {
            for item in &module_program.items {
                let Some(importable_name) = importable_item_name(item) else {
                    continue;
                };
                if local_names.contains(importable_name) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' já existe no arquivo principal",
                            importable_name
                        ),
                        span: import.span,
                    });
                }
                if let Some(previous_span) = imported_names.get(importable_name) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' trazido por múltiplos módulos",
                            importable_name
                        ),
                        span: previous_span.merge(import.span),
                    });
                }
                imported_names.insert(importable_name.to_string(), import.span);
                if let Some(cloned) = importable_item_clone(item) {
                    imported_items.push(cloned);
                }
            }
        }
    }

    root_program.items.splice(0..0, imported_items);
    root_program.imports.clear();
    Ok(root_program)
}
