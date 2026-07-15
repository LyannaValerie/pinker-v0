use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::doc;
use pinker_v0::doc_index;
use pinker_v0::editor_tui::EditorTui;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::lexer::Lexer;
use pinker_v0::nav;
use pinker_v0::parser::Parser;
use pinker_v0::printer;
use pinker_v0::repl;
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
    run_args: Vec<String>,
    check_only: bool,
}

struct BuildConfig {
    input: String,
    out_dir: String,
    nativo: bool,
}

struct EditorConfig {
    input: String,
}

struct ReplConfig;

/// Subcomando de `pink doc` (Trama Pinker — Etapas 0 e 2).
enum DocSub {
    /// Aplica a política do marco a um número de PR.
    ImportarPr { pr: u64 },
    /// Exibe o marco documental configurado.
    Marco,
    /// Extrai uma seção ou documento pelo id semântico.
    Mostrar { id: String },
    /// Lista os documentos de um território.
    Listar { territorio: String },
    /// Busca seções por id, título, tags, aliases e resumo.
    Buscar { consulta: String },
    /// Rota: melhores destinos para uma intenção.
    Rota { consulta: String },
    /// Regenera o catálogo `docs/navigation.jsonl`.
    Sincronizar,
    /// Valida documentação e catálogo (não corrige).
    Verificar,
}

struct DocConfigCli {
    repo: String,
    sub: DocSub,
}

/// Subcomando de `pink nav` (Trama Pinker — Etapa 3, navegação do código).
enum NavSub {
    Mostrar { key: String },
    Buscar { consulta: String },
    Listar { seletor: String },
    Sincronizar,
    Verificar,
}

struct NavConfigCli {
    repo: String,
    sub: NavSub,
}

enum CliCommand {
    Analyze(Config),
    Build(BuildConfig),
    Editor(EditorConfig),
    Repl(ReplConfig),
    Doc(DocConfigCli),
    Nav(NavConfigCli),
}

fn usage(binary: &str) -> String {
    format!(
        "Uso: {binary} [--tokens] [--ast] [--json-ast] [--ir] [--cfg-ir] [--selected] [--machine] [--pseudo-asm] [--asm-s] [--run] [--check] <arquivo.pink> [-- <args...>]\n\
         Uso: {binary} build [--out-dir <diretorio>] <arquivo.pink>\n\
         Uso: {binary} editor <arquivo.pink>\n\
         Uso: {binary} repl\n\
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
           --asm-s     imprime backend textual `.s` (ABI textual mínima)\n\
           --run       interpreta a machine validada e executa principal\n\
           --          separa argumentos repassados ao runtime de --run (argv posicional e nomeado mínimo)\n\
           --check     executa apenas a validação semântica\n\
         \n\
         Comandos:\n\
          build       gera artefato textual `.s` em disco\n\
          editor      abre a TUI oficial mínima da Pinker (Fase 136)\n\
          repl        abre o REPL mínimo auditável (Fase 167)\n\
          doc         ferramenta documental da Trama Pinker (marco / importação)\n\
          nav         navegação semântica do código da Trama Pinker\n"
    )
}

fn nav_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} nav <subcomando> [--repo <dir>] [args...]\n\
         \n\
         Comando:\n\
           nav         navegação semântica do código da Trama Pinker\n\
         \n\
         Subcomandos:\n\
           mostrar <key>       extrai a região de código pela chave\n\
           buscar <consulta>   busca regiões por chave, domínio, camada, resumo\n\
           listar <seletor>    lista regiões de uma camada (layer) ou domínio\n\
           sincronizar         regenera o catálogo src/navigation.jsonl\n\
           verificar           valida os marcadores e o catálogo (não corrige)\n\
         \n\
         Opções:\n\
           --repo      raiz do repositório (padrão: .)\n",
    )
}

fn doc_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} doc <subcomando> [--repo <dir>] [args...]\n\
         \n\
         Comando:\n\
           doc         ferramenta documental da Trama Pinker\n\
         \n\
         Subcomandos:\n\
           marco               exibe o marco documental configurado em {config}\n\
           importar-pr <n>     aplica a política do marco a um PR (E-DOC-BASELINE)\n\
           mostrar <id>        extrai a seção/documento pelo id semântico\n\
           listar <territorio> lista documentos de um território (domain)\n\
           buscar <consulta>   busca seções por id, título, tags, aliases, resumo\n\
           rota <consulta>     melhores destinos para uma intenção\n\
           sincronizar         regenera o catálogo docs/navigation.jsonl\n\
           verificar           valida documentação e catálogo (não corrige)\n\
         \n\
         Opções:\n\
           --repo      raiz do repositório (padrão: .)\n",
        binary = binary,
        config = doc::CONFIG_RELATIVE_PATH,
    )
}

fn build_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} build [--out-dir <diretorio>] [--nativo] <arquivo.pink>\n\
         \n\
         Comando:\n\
           build      executa o pipeline de build e grava artefato `.s` no disco\n\
         \n\
         Opções:\n\
           --out-dir  diretório de saída (padrão: build)\n\
           --nativo   além do `.s`, monta e linka um executável nativo real\n\
                      (driver C do sistema + runtime `libpinker_rt.a`;\n\
                       localização do runtime via env PINKER_RT_LIB ou ao lado do `pink`)\n"
    )
}

fn editor_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} editor <arquivo.pink>\n\
         \n\
         Comando:\n\
           editor     abre a TUI oficial mínima da Pinker (Fase 136)\n\
         \n\
         Comandos disponíveis na TUI:\n\
           :tokens    executa ação Pinker real e mostra saída no painel\n\
           :ast       mostra preview da AST no painel\n\
           :append    adiciona uma linha no final\n\
           :set       altera linha existente\n\
           :save      salva arquivo atual\n\
           :quit      sai do editor (requer :save se houver alterações)\n"
    )
}

fn repl_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} repl\n\
         \n\
         Comando:\n\
           repl       abre o REPL mínimo auditável da Pinker (Fase 167)\n\
         \n\
         Limites do REPL:\n\
           cada linha vira um corpo temporário de `principal`\n\
           não há estado persistente entre linhas\n\
           sem multiline amplo; use `:quit` ou `:sair` para encerrar\n"
    )
}

fn parse_build_args(binary: &str, args: &[String]) -> Result<BuildConfig, String> {
    let mut input: Option<String> = None;
    let mut out_dir = "build".to_string();
    let mut nativo = false;
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
            "--nativo" => {
                nativo = true;
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
    Ok(BuildConfig {
        input,
        out_dir,
        nativo,
    })
}

fn parse_editor_args(binary: &str, args: &[String]) -> Result<EditorConfig, String> {
    let mut input: Option<String> = None;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => return Err(editor_usage(binary)),
            _ if arg.starts_with("--") => {
                return Err(format!(
                    "Flag desconhecida no comando editor: '{}'\n\n{}",
                    arg,
                    editor_usage(binary)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado em 'editor'.\n\n{}",
                        editor_usage(binary)
                    ));
                }
                input = Some(arg.clone());
            }
        }
    }

    let Some(input) = input else {
        return Err(editor_usage(binary));
    };
    Ok(EditorConfig { input })
}

fn parse_repl_args(binary: &str, args: &[String]) -> Result<ReplConfig, String> {
    if args.is_empty() {
        return Ok(ReplConfig);
    }

    let arg = &args[0];
    match arg.as_str() {
        "--help" | "-h" => Err(repl_usage(binary)),
        _ if arg.starts_with("--") => Err(format!(
            "Flag desconhecida no comando repl: '{}'\n\n{}",
            arg,
            repl_usage(binary)
        )),
        _ => Err(format!(
            "O comando repl não aceita argumentos posicionais.\n\n{}",
            repl_usage(binary)
        )),
    }
}

fn parse_doc_args(binary: &str, args: &[String]) -> Result<DocConfigCli, String> {
    let mut repo = ".".to_string();
    let mut subcommand: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(doc_usage(binary)),
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                repo.clone_from(&args[i]);
            }
            _ if arg.starts_with("--") => {
                return Err(format!(
                    "Flag desconhecida no comando doc: '{}'\n\n{}",
                    arg,
                    doc_usage(binary)
                ));
            }
            _ => {
                if subcommand.is_none() {
                    subcommand = Some(arg.clone());
                } else {
                    positionals.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    let Some(subcommand) = subcommand else {
        return Err(doc_usage(binary));
    };

    let require_one = |what: &str| -> Result<String, String> {
        if positionals.len() != 1 {
            return Err(format!(
                "O subcomando '{}' requer exatamente um argumento.\n\n{}",
                what,
                doc_usage(binary)
            ));
        }
        Ok(positionals[0].clone())
    };
    let require_none = |what: &str| -> Result<(), String> {
        if !positionals.is_empty() {
            return Err(format!(
                "O subcomando '{}' não aceita argumentos posicionais.\n\n{}",
                what,
                doc_usage(binary)
            ));
        }
        Ok(())
    };

    let sub = match subcommand.as_str() {
        "importar-pr" => {
            let raw = require_one("importar-pr")?;
            let pr = raw.parse::<u64>().map_err(|_| {
                format!("Número de PR inválido: '{}'\n\n{}", raw, doc_usage(binary))
            })?;
            DocSub::ImportarPr { pr }
        }
        "marco" => {
            require_none("marco")?;
            DocSub::Marco
        }
        "mostrar" => DocSub::Mostrar {
            id: require_one("mostrar")?,
        },
        "listar" => DocSub::Listar {
            territorio: require_one("listar")?,
        },
        "buscar" => DocSub::Buscar {
            consulta: positionals.join(" "),
        },
        "rota" => DocSub::Rota {
            consulta: positionals.join(" "),
        },
        "sincronizar" => {
            require_none("sincronizar")?;
            DocSub::Sincronizar
        }
        "verificar" => {
            require_none("verificar")?;
            DocSub::Verificar
        }
        other => {
            return Err(format!(
                "Subcomando doc desconhecido: '{}'\n\n{}",
                other,
                doc_usage(binary)
            ));
        }
    };

    if matches!(sub, DocSub::Buscar { .. } | DocSub::Rota { .. }) && positionals.is_empty() {
        return Err(format!(
            "O subcomando '{}' requer uma consulta.\n\n{}",
            subcommand,
            doc_usage(binary)
        ));
    }

    Ok(DocConfigCli { repo, sub })
}

fn parse_nav_args(binary: &str, args: &[String]) -> Result<NavConfigCli, String> {
    let mut repo = ".".to_string();
    let mut subcommand: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(nav_usage(binary)),
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                repo.clone_from(&args[i]);
            }
            _ if arg.starts_with("--") => {
                return Err(format!(
                    "Flag desconhecida no comando nav: '{}'\n\n{}",
                    arg,
                    nav_usage(binary)
                ));
            }
            _ => {
                if subcommand.is_none() {
                    subcommand = Some(arg.clone());
                } else {
                    positionals.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    let Some(subcommand) = subcommand else {
        return Err(nav_usage(binary));
    };

    let require_one = |what: &str| -> Result<String, String> {
        if positionals.len() != 1 {
            return Err(format!(
                "O subcomando '{}' requer exatamente um argumento.\n\n{}",
                what,
                nav_usage(binary)
            ));
        }
        Ok(positionals[0].clone())
    };
    let require_none = |what: &str| -> Result<(), String> {
        if !positionals.is_empty() {
            return Err(format!(
                "O subcomando '{}' não aceita argumentos posicionais.\n\n{}",
                what,
                nav_usage(binary)
            ));
        }
        Ok(())
    };

    let sub = match subcommand.as_str() {
        "mostrar" => NavSub::Mostrar {
            key: require_one("mostrar")?,
        },
        "listar" => NavSub::Listar {
            seletor: require_one("listar")?,
        },
        "buscar" => {
            if positionals.is_empty() {
                return Err(format!(
                    "O subcomando 'buscar' requer uma consulta.\n\n{}",
                    nav_usage(binary)
                ));
            }
            NavSub::Buscar {
                consulta: positionals.join(" "),
            }
        }
        "sincronizar" => {
            require_none("sincronizar")?;
            NavSub::Sincronizar
        }
        "verificar" => {
            require_none("verificar")?;
            NavSub::Verificar
        }
        other => {
            return Err(format!(
                "Subcomando nav desconhecido: '{}'\n\n{}",
                other,
                nav_usage(binary)
            ));
        }
    };

    Ok(NavConfigCli { repo, sub })
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
    let mut cli_tail_start = cli_args.len();
    for (i, arg) in cli_args.iter().enumerate() {
        if arg == "--" {
            cli_tail_start = i;
            break;
        }
    }
    let flag_args = &cli_args[..cli_tail_start];
    let runtime_tail = if cli_tail_start < cli_args.len() {
        &cli_args[(cli_tail_start + 1)..]
    } else {
        &[]
    };

    if let Some(cmd) = flag_args.first() {
        if cmd == "build" {
            return parse_build_args(&binary, &flag_args[1..]).map(CliCommand::Build);
        }
        if cmd == "editor" {
            return parse_editor_args(&binary, &flag_args[1..]).map(CliCommand::Editor);
        }
        if cmd == "repl" {
            return parse_repl_args(&binary, &flag_args[1..]).map(CliCommand::Repl);
        }
        if cmd == "doc" {
            return parse_doc_args(&binary, &flag_args[1..]).map(CliCommand::Doc);
        }
        if cmd == "nav" {
            return parse_nav_args(&binary, &flag_args[1..]).map(CliCommand::Nav);
        }
    }

    for arg in flag_args {
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
    if !run_program && !runtime_tail.is_empty() {
        return Err(format!(
            "Argumentos após '--' exigem '--run'.\n\n{}",
            usage(&binary)
        ));
    }

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
        run_args: runtime_tail.to_vec(),
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
        CliCommand::Editor(config) => run_editor(config),
        CliCommand::Repl(config) => run_repl(config),
        CliCommand::Doc(config) => run_doc(config),
        CliCommand::Nav(config) => run_nav(config),
    }
}

fn scan_code(repo_root: &Path) -> nav::CodeIndex {
    let src_root = repo_root.join("src");
    match nav::CodeIndex::scan(&src_root) {
        Ok(index) => index,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run_nav(config: NavConfigCli) {
    let repo_root = Path::new(&config.repo);
    match config.sub {
        NavSub::Mostrar { key } => {
            let index = scan_code(repo_root);
            let Some(region) = index.region(&key) else {
                eprintln!(
                    "chave de código não encontrada: '{key}'. Tente `pink nav buscar \"{key}\"`."
                );
                std::process::exit(1);
            };
            println!(
                "// {} — {}:{}-{}",
                region.key, region.file, region.content_start, region.content_end
            );
            if !region.summary.is_empty() {
                println!("// {}", region.summary);
            }
            println!();
            let path = repo_root.join(&region.file);
            match fs::read_to_string(&path) {
                Ok(text) => {
                    let lines: Vec<&str> = text.lines().collect();
                    let start = region.content_start.saturating_sub(1);
                    let end = region.content_end.min(lines.len());
                    for line in &lines[start..end] {
                        println!("{line}");
                    }
                }
                Err(err) => {
                    eprintln!("Falha ao ler '{}': {}", path.display(), err);
                    std::process::exit(1);
                }
            }
        }
        NavSub::Buscar { consulta } => {
            let index = scan_code(repo_root);
            let hits = index.search(&consulta);
            if hits.is_empty() {
                println!("Nenhuma região encontrada para: {consulta}");
                return;
            }
            for region in hits.iter().take(10) {
                println!("{}", region.key);
                if !region.summary.is_empty() {
                    println!("   {}", region.summary);
                }
                println!(
                    "   {}:{}-{}",
                    region.file, region.content_start, region.content_end
                );
            }
        }
        NavSub::Listar { seletor } => {
            let index = scan_code(repo_root);
            let regions = index.list(&seletor);
            if regions.is_empty() {
                println!("Nenhuma região na camada/domínio '{seletor}'.");
                return;
            }
            println!("Regiões em '{seletor}':");
            for region in regions {
                println!(
                    "- {} [{}/{}] {}:{}-{}",
                    region.key,
                    region.domain.as_deref().unwrap_or("-"),
                    region.layer.as_deref().unwrap_or("-"),
                    region.file,
                    region.content_start,
                    region.content_end
                );
            }
        }
        NavSub::Sincronizar => {
            let doc_config = load_doc_config(repo_root);
            let index = scan_code(repo_root);
            let rendered = index.render_jsonl();
            let path = repo_root.join(&doc_config.generated.code_index);
            if let Some(parent) = path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Falha ao criar '{}': {}", parent.display(), err);
                    std::process::exit(1);
                }
            }
            if let Err(err) = fs::write(&path, rendered) {
                eprintln!("Falha ao gravar '{}': {}", path.display(), err);
                std::process::exit(1);
            }
            println!(
                "Catálogo de código sincronizado: {} ({} regiões).",
                doc_config.generated.code_index,
                index.regions.len()
            );
        }
        NavSub::Verificar => {
            let doc_config = load_doc_config(repo_root);
            let index = scan_code(repo_root);
            let mut errors = index.verify();
            let path = repo_root.join(&doc_config.generated.code_index);
            let rendered = index.render_jsonl();
            let on_disk = fs::read_to_string(path).unwrap_or_default();
            if on_disk != rendered {
                errors.push(nav::NavVerifyError::IndexOutOfDate {
                    path: doc_config.generated.code_index.clone(),
                });
            }
            if errors.is_empty() {
                println!("Marcadores e catálogo de código verificados: ok.");
                return;
            }
            eprintln!(
                "E-NAV-VERIFY: {} divergência(s) encontrada(s):",
                errors.len()
            );
            for error in &errors {
                eprintln!("  - {error}");
            }
            std::process::exit(1);
        }
    }
}

fn load_doc_config(repo_root: &Path) -> doc::DocConfig {
    match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run_doc(config: DocConfigCli) {
    let repo_root = Path::new(&config.repo);
    let doc_config = match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    match config.sub {
        DocSub::Marco => {
            let github = &doc_config.github;
            let limite = if github.baseline_inclusive {
                "inclusivo"
            } else {
                "exclusivo"
            };
            println!("Trama Pinker — marco documental");
            println!("  modo:    {}", github.mode);
            println!("  marco:   PR #{}, {}", github.baseline_pr, limite);
            println!("  commit:  {}", github.baseline_commit);
            println!("  docs:    {}", doc_config.generated.docs_index);
            println!("  código:  {}", doc_config.generated.code_index);
        }
        DocSub::ImportarPr { pr } => {
            if let Err(rejection) = doc_config.baseline_gate(pr) {
                eprintln!("{rejection}");
                std::process::exit(1);
            }
            println!(
                "PR #{pr} posterior ao marco #{} — elegível para importação.",
                doc_config.github.baseline_pr
            );
            println!(
                "(Etapa 0 valida o marco; a geração de manifesto estrutural chega na Etapa 4 da Trama.)"
            );
        }
        DocSub::Mostrar { id } => run_doc_mostrar(repo_root, &id),
        DocSub::Listar { territorio } => run_doc_listar(repo_root, &territorio),
        DocSub::Buscar { consulta } => run_doc_buscar(repo_root, &consulta),
        DocSub::Rota { consulta } => run_doc_rota(repo_root, &consulta),
        DocSub::Sincronizar => run_doc_sincronizar(repo_root, &doc_config),
        DocSub::Verificar => run_doc_verificar(repo_root, &doc_config),
    }
}

fn scan_docs(repo_root: &Path) -> doc_index::DocIndex {
    let docs_root = repo_root.join("docs");
    match doc_index::DocIndex::scan(&docs_root) {
        Ok(index) => index,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run_doc_mostrar(repo_root: &Path, id: &str) {
    let index = scan_docs(repo_root);
    if let Some(section) = index.section(id) {
        println!(
            "# {} — {}:{}-{}",
            section.id, section.file, section.start, section.end
        );
        if !section.summary.is_empty() {
            println!("# {}", section.summary);
        }
        println!();
        let path = repo_root.join(&section.file);
        match fs::read_to_string(&path) {
            Ok(text) => {
                let lines: Vec<&str> = text.lines().collect();
                let start = section.start.saturating_sub(1);
                let end = section.end.min(lines.len());
                for line in &lines[start..end] {
                    println!("{line}");
                }
            }
            Err(err) => {
                eprintln!("Falha ao ler '{}': {}", path.display(), err);
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(doc) = index.document(id) {
        println!("# documento {} ({})", doc.id, doc.kind);
        println!("  território: {}", doc.domain);
        println!("  arquivo:    {}", doc.file);
        if !doc.canonical_for.is_empty() {
            println!("  autoridade: {}", doc.canonical_for.join(", "));
        }
        let sections: Vec<&doc_index::DocSection> = index
            .sections
            .iter()
            .filter(|s| s.document == doc.id)
            .collect();
        if sections.is_empty() {
            println!("  seções:     (nenhuma âncora)");
        } else {
            println!("  seções:");
            for section in sections {
                println!(
                    "    - {} ({}:{}-{})",
                    section.id, section.file, section.start, section.end
                );
            }
        }
        return;
    }

    eprintln!("id documental não encontrado: '{id}'. Tente `pink doc buscar \"{id}\"`.");
    std::process::exit(1);
}

fn run_doc_listar(repo_root: &Path, territorio: &str) {
    let index = scan_docs(repo_root);
    let docs: Vec<&doc_index::DocDocument> = index
        .documents
        .iter()
        .filter(|d| d.domain == territorio)
        .collect();
    if docs.is_empty() {
        println!("Nenhum documento estrutural no território '{territorio}'.");
        return;
    }
    println!("Território '{territorio}':");
    for doc in docs {
        println!("- {} [{}] {}", doc.id, doc.kind, doc.file);
        for section in index.sections.iter().filter(|s| s.document == doc.id) {
            println!("    · {} — {}", section.id, section.title);
        }
    }
}

fn run_doc_buscar(repo_root: &Path, consulta: &str) {
    let index = scan_docs(repo_root);
    let hits = index.search(consulta);
    if hits.is_empty() {
        println!("Nenhuma seção encontrada para: {consulta}");
        return;
    }
    for hit in hits.iter().take(10) {
        println!("{}", hit.id);
        println!("   {}", hit.summary);
        println!("   {}:{}-{}", hit.file, hit.start, hit.end);
    }
}

fn run_doc_rota(repo_root: &Path, consulta: &str) {
    let index = scan_docs(repo_root);
    let hits = index.search(consulta);
    println!("Consulta: {consulta}");
    if hits.is_empty() {
        println!("Nenhuma rota encontrada. Tente `pink doc buscar`.");
        return;
    }
    for (i, hit) in hits.iter().take(5).enumerate() {
        println!("{}. {}", i + 1, hit.id);
        println!("   {}", hit.summary);
        println!("   {}:{}-{}", hit.file, hit.start, hit.end);
    }
    println!();
    println!("Use:");
    println!("    pink doc mostrar {}", hits[0].id);
}

fn run_doc_sincronizar(repo_root: &Path, config: &doc::DocConfig) {
    let index = scan_docs(repo_root);
    let rendered = index.render_jsonl();
    let path = repo_root.join(&config.generated.docs_index);
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("Falha ao criar '{}': {}", parent.display(), err);
            std::process::exit(1);
        }
    }
    if let Err(err) = fs::write(&path, rendered) {
        eprintln!("Falha ao gravar '{}': {}", path.display(), err);
        std::process::exit(1);
    }
    println!(
        "Catálogo documental sincronizado: {} ({} seções).",
        config.generated.docs_index,
        index.sections.len()
    );
}

fn run_doc_verificar(repo_root: &Path, config: &doc::DocConfig) {
    let index = scan_docs(repo_root);
    let mut errors = index.verify();

    let path = repo_root.join(&config.generated.docs_index);
    let rendered = index.render_jsonl();
    let on_disk = fs::read_to_string(path).unwrap_or_default();
    if on_disk != rendered {
        errors.push(doc_index::DocVerifyError::CatalogOutOfDate {
            path: config.generated.docs_index.clone(),
        });
    }

    if errors.is_empty() {
        println!("Documentação e catálogo verificados: ok.");
        return;
    }
    eprintln!(
        "E-DOC-VERIFY: {} divergência(s) encontrada(s):",
        errors.len()
    );
    for error in &errors {
        eprintln!("  - {error}");
    }
    std::process::exit(1);
}

fn run_editor(config: EditorConfig) {
    let mut editor = match EditorTui::from_path(config.input) {
        Ok(editor) => editor,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };
    if let Err(err) = editor.run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run_repl(_config: ReplConfig) {
    if let Err(err) = repl::run_repl() {
        eprintln!("{err}");
        std::process::exit(1);
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

    // --- Backend textual `.s` ---
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
            interpreter::run_program_with_args(machine_program.as_ref().unwrap(), &config.run_args),
            &source
        );
        if let Some(value) = result.return_value {
            match value {
                interpreter::RuntimeValue::Int(v) => println!("{}", v),
                interpreter::RuntimeValue::IntSigned(v) => println!("{}", v),
                interpreter::RuntimeValue::Ptr(v) => println!("{}", v),
                interpreter::RuntimeValue::Bool(_) => {}
                interpreter::RuntimeValue::Str(v) => println!("{}", v),
                interpreter::RuntimeValue::ListBombom(_) => {}
                interpreter::RuntimeValue::ListVerso(_) => {}
                interpreter::RuntimeValue::MapVersoBombom(_) => {}
                interpreter::RuntimeValue::MapVersoVerso(_) => {}
                interpreter::RuntimeValue::MapBombomBombom(_) => {}
                interpreter::RuntimeValue::MapBombomVerso(_) => {}
            }
        }
        if let Some(code) = result.exit_status {
            std::process::exit(code);
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
    let output = if config.nativo {
        try_or_exit!(
            backend_s::emit_external_toolchain_subset_nativo(&selected_program),
            &source
        )
    } else {
        try_or_exit!(backend_s::emit_from_selected(&selected_program), &source)
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
fn link_nativo(asm_path: &Path, bin_path: &Path) -> Result<(), String> {
    let driver = detect_cc_driver()?;
    let runtime_lib = locate_pinker_rt_lib()?;
    let output = std::process::Command::new(&driver)
        .arg(asm_path)
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
        ast::Item::Struct(struct_decl) => Some(struct_decl.name.as_str()),
        ast::Item::TypeAlias(alias) => Some(alias.name.as_str()),
        ast::Item::Enum(enum_decl) => Some(enum_decl.name.as_str()),
        ast::Item::Trait(trait_decl) => Some(trait_decl.name.as_str()),
    }
}

fn importable_item_clone(item: &ast::Item) -> Option<ast::Item> {
    match item {
        ast::Item::Function(_)
        | ast::Item::Const(_)
        | ast::Item::Struct(_)
        | ast::Item::TypeAlias(_)
        | ast::Item::Enum(_)
        | ast::Item::Trait(_) => Some(item.clone()),
    }
}

fn qualified_type_item_clone(module: &str, item: &ast::Item) -> Option<ast::Item> {
    match item {
        ast::Item::Struct(struct_decl) => {
            let mut cloned = struct_decl.clone();
            cloned.name = format!("{}.{}", module, struct_decl.name);
            Some(ast::Item::Struct(cloned))
        }
        ast::Item::TypeAlias(alias) => {
            let mut cloned = alias.clone();
            cloned.name = format!("{}.{}", module, alias.name);
            Some(ast::Item::TypeAlias(cloned))
        }
        _ => None,
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
    let mut imported_qualified_type_names = HashSet::<String>::new();
    let local_names: HashSet<String> = root_program
        .items
        .iter()
        .filter_map(importable_item_name)
        .map(ToOwned::to_owned)
        .collect();

    for import in &root_program.imports {
        // Fases 186–188 — famílias built-in importáveis não correspondem a
        // arquivo .pink. As intrínsecas já estão disponíveis globalmente; basta
        // pular a carga de módulo.
        if semantic::is_importable_builtin_family_import(import) {
            continue;
        }

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
            let qualified_name = format!("{}.{}", import.module, symbol);
            if imported_qualified_type_names.insert(qualified_name) {
                if let Some(qualified_item) =
                    qualified_type_item_clone(import.module.as_str(), item)
                {
                    imported_items.push(qualified_item);
                }
            }
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
                let qualified_name = format!("{}.{}", import.module, importable_name);
                if imported_qualified_type_names.insert(qualified_name) {
                    if let Some(qualified_item) =
                        qualified_type_item_clone(import.module.as_str(), item)
                    {
                        imported_items.push(qualified_item);
                    }
                }
            }
        }
    }

    root_program.items.splice(0..0, imported_items);
    root_program.imports.clear();
    Ok(root_program)
}
