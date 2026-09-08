//! Análise e build nativo do binário `pink` (`cli.analise.pipeline` e
//! `cli.build.nativo`), unidade MAIN-4 da decomposição física #601.
//!
//! Movimento físico: as decisões, o estado e a ordem são os do entrypoint.
//! `main.rs` continua dono da orquestração; aqui mora só a implementação.
//!
//! O `macro_rules! try_or_exit` não se move: ele fica em `src/main.rs`,
//! junto do `main` que o define. Este irmão o enxerga por escopo textual,
//! porque o `mod` que o declara está abaixo da definição da macro.

use super::*;

// @pinker-nav:start cli.analise.pipeline
// @pinker-nav:domain analise
// @pinker-nav:layer cli
// @pinker-nav:summary run_analyze lê o arquivo de entrada, registra-o como unidade-fonte primária no SourceMap para que todo span nasça vinculado, e conduz o pipeline de análise: tokeniza, parseia, compõe os módulos preservando a unidade (carregar_e_projetar, que devolve o programa projetado e o grafo resolvido), roda a verificação semântica ciente da composição (semantic::check_program_composto, que recebe os tratos visíveis por fonte) e, conforme as flags do Config, cada etapa a jusante (IR, CFG IR, seleção de instruções, máquina abstrata, backend `.s` textual, execução via interpretador, backend pseudo-asm) só é computada se alguma flag de saída a exigir (`needs_ir`/`needs_cfg`/`needs_selected`/`needs_machine`); a falha ao ler o arquivo é tratada diretamente com `eprintln!` e `process::exit(1)`, enquanto erros Pinker das etapas de tokenização, parsing, importação, semântica e lowerings são tratados por `try_or_exit!`; esta função não monta nem linka um binário — a emissão `--asm-s` é apenas texto impresso, e `--run` executa via interpreter::run_program_with_args, não via processo nativo.
pub(super) fn run_analyze(config: Config) {
    let source = match fs::read_to_string(&config.input) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Falha ao ler '{}': {}", config.input, err);
            std::process::exit(1);
        }
    };

    // --- Frontend: léxico + parsing ---
    // A raiz é a primeira unidade-fonte registrada, então recebe
    // `SourceId::ROOT`. Todo span produzido a partir daqui já nasce sabendo a
    // que texto pertence.
    let mut sources = SourceMap::new();
    let root_source_id = sources.register_root(config.input.clone(), source.clone());
    let mut lexer = Lexer::com_fonte(&source, root_source_id);
    let tokens = try_or_exit!(lexer.tokenize(), &sources);

    if config.print_tokens && !config.check_only {
        println!("=== TOKENS ===");
        for token in &tokens {
            println!("{} '{}' [{}]", token.kind.name(), token.lexeme, token.span);
        }
    }

    // Parte G: o que só a autoridade de import sabe é resolvido aqui e
    // entregue pronto ao parser — nunca depois da canonicalização, que é
    // irreversível.
    let contexto = contexto_de_import(&tokens, &base_dir_de(&config.input));
    let mut parser = Parser::com_contexto_de_import(tokens, GenericOrigin::Root, contexto);
    let parsed_program = try_or_exit!(parser.parse(), &sources);
    // O empréstimo mutável do mapa de fontes termina antes da renderização de
    // erro, que precisa lê-lo.
    let carregado = carregar_e_projetar(&config.input, parsed_program, &mut sources);
    let (program, grafo) = try_or_exit!(carregado, &sources);
    let tratos_visiveis = module_resolve::tratos_visiveis_por_fonte(&grafo);
    let fontes_de_modulo = module_resolve::fontes_de_modulo(&grafo);

    if config.print_ast && !config.check_only {
        println!("=== AST TEXTUAL ===");
        print!("{}", printer::render_program(&program));
    }

    if config.print_json_ast && !config.check_only {
        println!("=== AST JSON ===");
        println!("{}", printer::render_program_json(&program));
    }

    // --- Semântica ---
    try_or_exit!(
        semantic::check_program_composto(
            &program,
            tratos_visiveis.clone(),
            fontes_de_modulo.clone()
        ),
        &sources
    );

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
        let lowered = try_or_exit!(
            ir::lower_program_composto(&program, tratos_visiveis.clone()),
            &sources
        );
        try_or_exit!(ir_validate::validate_program(&lowered), &sources);
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
        let cfg = try_or_exit!(
            cfg_ir::lower_program(program_ir.as_ref().unwrap()),
            &sources
        );
        try_or_exit!(cfg_ir_validate::validate_program(&cfg), &sources);
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
            &sources
        );
        try_or_exit!(instr_select_validate::validate_program(&selected), &sources);
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
            &sources
        );
        try_or_exit!(
            abstract_machine_validate::validate_program(&machine),
            &sources
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

    // --- Backend textual `.s` ---
    // Esta saída textual parte de `selected_program`, com ABI textual mínima interna
    // (ainda sem ABI/registradores reais de plataforma).
    if config.print_asm_s {
        let out = try_or_exit!(
            backend_s::emit_from_selected(selected_program.as_ref().unwrap()),
            &sources
        );
        println!("=== ASM .S (TEXTUAL) ===");
        print!("{}", out);
    }

    // --- Execução via interpretador ---
    if config.run_program {
        let result = try_or_exit!(
            interpreter::run_program_with_args(machine_program.as_ref().unwrap(), &config.run_args),
            &sources
        );
        std::process::exit(result.exit_status.unwrap_or(0));
    }

    // --- Backend textual (pseudo-asm) ---
    // Nota (HF-6): `--pseudo-asm` parte de `selected_program` (não de `machine_program`),
    // enquanto `--run` parte de `machine_program`. Essa bifurcação é intencional:
    // o backend textual é uma representação alternativa da seleção de instruções,
    // e o interpretador precisa da Machine validada para execução.
    if config.print_pseudo_asm {
        let lowered_backend = try_or_exit!(
            backend_text::lower_selected_program(selected_program.as_ref().unwrap()),
            &sources
        );
        try_or_exit!(
            backend_text_validate::validate_program(&lowered_backend),
            &sources
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
// @pinker-nav:end cli.analise.pipeline

// @pinker-nav:start cli.build.nativo
// @pinker-nav:domain build
// @pinker-nav:layer cli
// @pinker-nav:summary run_build repete o front-end (lex/parse/imports/semântica/IR/CFG/seleção) e grava o `.s` resultante em <out_dir>/<stem>.s via fs::write; com --nativo, emite via emit_external_toolchain_subset_nativo e, após gravar, chama link_nativo. locate_pinker_rt_lib localiza (não constrói) a staticlib libpinker_rt.a pré-buildada: usa a env PINKER_RT_LIB se apontar para um arquivo existente, senão procura ao lado do executável atual via std::env::current_exe; retorna Err com uma mensagem sugerindo `cargo build` se não encontrar. detect_cc_driver detecta um driver C disponível testando `cc --version`/`gcc --version`/`clang --version` via std::process::Command e retorna o primeiro que responder com status de sucesso. link_nativo invoca esse driver externo em dois passos: primeiro `-c` sobre o `.s` para um objeto cujo basename deriva do próprio `.s` mas que vive dentro de um DiretorioIntermediario possuído pela execução, depois a linkedição desse objeto com a staticlib localizada e -lpthread/-ldl/-lm para produzir o binário via -o. A montagem e a linkedição continuam sendo feitas pelo driver externo, não por este arquivo; o que este arquivo controla é o basename do objeto, porque entregar o `.s` direto ao driver deixaria o intermediário com nome temporário aleatório e o linker o registraria como símbolo `STT_FILE` do executável, quebrando o determinismo byte a byte entre dois builds da mesma fonte. O diretório do objeto não atravessa a linkedição, então o intermediário nunca ocupa `<out_dir>/<stem>.o`: arquivo preexistente do usuário com esse nome não é sobrescrito nem apagado, em nenhum desfecho. Antes de linkar, link_nativo chama verificar_artefato_sussurro, que relê o `.s` gravado e delega a inline_asm::verify_native_artifact — o invariante de artefato roda no caminho produtivo, não só em fixture de teste: monta o assembly emitido e a baseline derivada sem os envelopes noutro DiretorioIntermediario sob o out_dir, compara as superfícies dos dois objetos e aborta o build com E-BACKEND-ASM-ARTIFACT diante de qualquer delta de seção ou de símbolo definido; os dois diretórios intermediários são removidos em qualquer desfecho pelo próprio Drop e a verificação só imprime linha de confirmação quando existe ao menos um envelope. DiretorioIntermediario é esse espaço de scratch: `criar` monta `<out_dir>/.pinker-<propósito>-<pid>[-<n>]` com fs::create_dir, que falha com AlreadyExists em vez de adotar diretório preexistente — a posse fica provada, não presumida —, e Drop remove recursivamente só o que essa criação exclusiva produziu, de modo que nenhum desfecho (sucesso, recusa do assembler, recusa do linker, erro de I/O) apaga arquivo que a execução não criou.
pub(super) fn run_build(config: BuildConfig) {
    let source = match fs::read_to_string(&config.input) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Falha ao ler '{}': {}", config.input, err);
            std::process::exit(1);
        }
    };

    let mut sources = SourceMap::new();
    let root_source_id = sources.register_root(config.input.clone(), source.clone());
    let mut lexer = Lexer::com_fonte(&source, root_source_id);
    let tokens = try_or_exit!(lexer.tokenize(), &sources);
    // Parte G: o que só a autoridade de import sabe é resolvido aqui e
    // entregue pronto ao parser — nunca depois da canonicalização, que é
    // irreversível.
    let contexto = contexto_de_import(&tokens, &base_dir_de(&config.input));
    let mut parser = Parser::com_contexto_de_import(tokens, GenericOrigin::Root, contexto);
    let parsed_program = try_or_exit!(parser.parse(), &sources);
    // O empréstimo mutável do mapa de fontes termina antes da renderização de
    // erro, que precisa lê-lo.
    let carregado = carregar_e_projetar(&config.input, parsed_program, &mut sources);
    let (program, grafo) = try_or_exit!(carregado, &sources);
    let tratos_visiveis = module_resolve::tratos_visiveis_por_fonte(&grafo);
    let fontes_de_modulo = module_resolve::fontes_de_modulo(&grafo);
    try_or_exit!(
        semantic::check_program_composto(
            &program,
            tratos_visiveis.clone(),
            fontes_de_modulo.clone()
        ),
        &sources
    );

    let program_ir = try_or_exit!(
        ir::lower_program_composto(&program, tratos_visiveis.clone()),
        &sources
    );
    try_or_exit!(ir_validate::validate_program(&program_ir), &sources);
    let cfg_program = try_or_exit!(cfg_ir::lower_program(&program_ir), &sources);
    try_or_exit!(cfg_ir_validate::validate_program(&cfg_program), &sources);
    let selected_program = try_or_exit!(instr_select::lower_program(&cfg_program), &sources);
    try_or_exit!(
        instr_select_validate::validate_program(&selected_program),
        &sources
    );
    let output = if config.nativo {
        try_or_exit!(
            backend_s::emit_external_toolchain_subset_nativo(&selected_program),
            &sources
        )
    } else {
        try_or_exit!(backend_s::emit_from_selected(&selected_program), &sources)
    };

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

    if config.nativo {
        let bin_path = out_dir.join(stem);
        match link_nativo(&output_path, &bin_path) {
            Ok(()) => println!("Executável nativo: {}", bin_path.display()),
            Err(msg) => {
                eprintln!("Falha no link nativo: {}", msg);
                std::process::exit(1);
            }
        }
    }
}

/// Localiza a staticlib do runtime nativo: env `PINKER_RT_LIB` tem precedência;
/// caso contrário, procura `libpinker_rt.a` ao lado do executável `pink`
/// (layout padrão do `target/` do cargo).
fn locate_pinker_rt_lib() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("PINKER_RT_LIB") {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "PINKER_RT_LIB aponta para '{}', que não existe",
            path.display()
        ));
    }
    let exe = std::env::current_exe()
        .map_err(|err| format!("não foi possível localizar o executável atual: {}", err))?;
    let candidate = exe
        .parent()
        .map(|dir| dir.join("libpinker_rt.a"))
        .ok_or_else(|| "executável atual sem diretório pai".to_string())?;
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(format!(
        "runtime nativo 'libpinker_rt.a' não encontrado em '{}'; construa o workspace (cargo build) ou defina PINKER_RT_LIB",
        candidate.display()
    ))
}

fn detect_cc_driver() -> Result<String, String> {
    for candidate in ["cc", "gcc", "clang"] {
        let probe = std::process::Command::new(candidate)
            .arg("--version")
            .output();
        if let Ok(output) = probe {
            if output.status.success() {
                return Ok(candidate.to_string());
            }
        }
    }
    Err("nenhum driver C encontrado no sistema (procurado: cc, gcc, clang)".to_string())
}

/// Monta e linka o `.s` nativo com o runtime `pinker_rt`, produzindo um
/// executável ELF real. As libs de sistema extras cobrem as dependências da
/// std do Rust embutida na staticlib do runtime.
/// Verifica o objeto realmente produzido antes de linkar.
///
/// A política estrutural governa a fonte; este é o invariante do artefato. Ele
/// monta o assembly emitido e a baseline sem os envelopes e recusa qualquer
/// delta de seção ou de símbolo definido atribuível ao bloco de `sussurro`.
fn verificar_artefato_sussurro(
    asm_path: &Path,
    out_dir: &Path,
    driver: &str,
) -> Result<Option<inline_asm::ArtifactCheck>, String> {
    let asm = fs::read_to_string(asm_path)
        .map_err(|err| format!("falha ao reler '{}': {}", asm_path.display(), err))?;
    // O diretório de verificação é intermediário e possuído por esta execução:
    // não sobrevive ao build, nem quando a verificação recusa.
    let workdir = DiretorioIntermediario::criar(out_dir, "sussurro-verificacao")?;
    let resultado = inline_asm::verify_native_artifact(&asm, driver, workdir.path());
    let check = resultado.map_err(|error| error.to_string())?;
    Ok((check.envelopes > 0).then_some(check))
}

fn link_nativo(asm_path: &Path, bin_path: &Path) -> Result<(), String> {
    let driver = detect_cc_driver()?;
    let runtime_lib = locate_pinker_rt_lib()?;
    let out_dir = asm_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if let Some(check) = verificar_artefato_sussurro(asm_path, &out_dir, &driver)? {
        println!(
            "Artefato verificado: {} envelope(s) de 'sussurro', sem delta de seção ou símbolo ({} seções, {} símbolos definidos)",
            check.envelopes, check.sections, check.defined_symbols
        );
    }
    // A montagem é um passo próprio, com basename de objeto derivado do `.s`.
    // Entregar o `.s` direto ao driver deixa o objeto intermediário com um nome
    // temporário aleatório, e o linker o registra como símbolo `STT_FILE` do
    // executável assim que o objeto passa a contribuir símbolos locais — o
    // binário deixaria de ser byte-determinístico entre dois builds da mesma
    // fonte. O que atravessa a linkedição é o basename, não o diretório: por
    // isso o objeto vive num diretório intermediário possuído por esta
    // execução, e não em `<out_dir>/<stem>.o`, que é pathname do usuário.
    let workdir = DiretorioIntermediario::criar(&out_dir, "montagem")?;
    let object_name = asm_path
        .with_extension("o")
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| format!("assembly sem nome de arquivo: '{}'", asm_path.display()))?;
    let object_path = workdir.path().join(object_name);
    let assemble = std::process::Command::new(&driver)
        .arg("-c")
        .arg(asm_path)
        .arg("-o")
        .arg(&object_path)
        .output()
        .map_err(|err| format!("falha ao invocar '{}': {}", driver, err))?;
    if !assemble.status.success() {
        return Err(format!(
            "'{}' retornou erro:\n{}",
            driver,
            String::from_utf8_lossy(&assemble.stderr)
        ));
    }
    let output = std::process::Command::new(&driver)
        .arg(&object_path)
        .arg(&runtime_lib)
        .arg("-lpthread")
        .arg("-ldl")
        .arg("-lm")
        .arg("-o")
        .arg(bin_path)
        .output()
        .map_err(|err| format!("falha ao invocar '{}': {}", driver, err))?;
    if !output.status.success() {
        return Err(format!(
            "'{}' retornou erro:\n{}",
            driver,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
/// Diretório intermediário do build, criado e possuído por esta execução.
struct DiretorioIntermediario {
    path: PathBuf,
}

impl DiretorioIntermediario {
    /// Limite de tentativas de nome. Só é alcançado se muitos diretórios do
    /// mesmo pid já existirem no out_dir, o que indica lixo de execução morta.
    const MAX_TENTATIVAS: u32 = 64;

    /// Cria um diretório novo sob `out_dir`, provando a posse.
    ///
    /// `fs::create_dir` falha com `AlreadyExists` quando o caminho já existe;
    /// a criação nunca adota diretório de terceiros, e por isso o `Drop` pode
    /// remover recursivamente sem risco de apagar arquivo alheio.
    fn criar(out_dir: &Path, proposito: &str) -> Result<Self, String> {
        let pid = std::process::id();
        for tentativa in 0..Self::MAX_TENTATIVAS {
            let nome = if tentativa == 0 {
                format!(".pinker-{proposito}-{pid}")
            } else {
                format!(".pinker-{proposito}-{pid}-{tentativa}")
            };
            let candidato = out_dir.join(nome);
            match fs::create_dir(&candidato) {
                Ok(()) => return Ok(Self { path: candidato }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "falha ao criar diretório intermediário '{}': {}",
                        candidato.display(),
                        err
                    ))
                }
            }
        }
        Err(format!(
            "falha ao criar diretório intermediário de '{proposito}' em '{}': todos os nomes candidatos já existem",
            out_dir.display()
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DiretorioIntermediario {
    fn drop(&mut self) {
        // Só o que `criar` produziu: diretório preexistente nunca foi adotado.
        let _ = fs::remove_dir_all(&self.path);
    }
}

// @pinker-nav:end cli.build.nativo
