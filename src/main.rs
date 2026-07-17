use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::change;
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
use pinker_v0::projection;
use pinker_v0::repl;
use pinker_v0::semantic;
use pinker_v0::token::Span;
use pinker_v0::{ast, error::PinkerError};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// @pinker-nav:start cli.config.modelos
// @pinker-nav:domain config
// @pinker-nav:layer cli
// @pinker-nav:summary Constantes de códigos de saída (EXIT_OK/EXIT_USAGE/EXIT_CATALOG/EXIT_NORESULT/EXIT_SOURCE) e limites de paginação (LIMIT_MIN/MAX, LIMIT_DEFAULT_ROTA/BUSCAR); clamp_limit ajusta um Option<usize> aos limites via .clamp; json_escape escapa aspas/barra/controle para JSON; json_string_array serializa Vec<String>. Structs de configuração por subcomando (Config, BuildConfig, EditorConfig, ReplConfig, DocConfigCli, NavConfigCli) e os enums de subcomando (DocSub, NavSub, CliCommand) usados pelo parsing e roteamento a seguir.
/// Códigos de saída padronizados das consultas da Trama (especificação §7.4).
const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 2;
const EXIT_CATALOG: i32 = 3;
const EXIT_NORESULT: i32 = 4;
const EXIT_SOURCE: i32 = 5;

/// Limites de resultados por subcomando (§7).
const LIMIT_MIN: usize = 1;
const LIMIT_MAX: usize = 20;
const LIMIT_DEFAULT_ROTA: usize = 5;
const LIMIT_DEFAULT_BUSCAR: usize = 10;

/// Ajusta o limite pedido aos contornos [1, 20], usando `default` se ausente.
fn clamp_limit(requested: Option<usize>, default: usize) -> usize {
    requested.unwrap_or(default).clamp(LIMIT_MIN, LIMIT_MAX)
}

/// Escapa uma string para JSON estável (idêntico ao usado nos catálogos).
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_string_array(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| json_escape(s)).collect();
    format!("[{}]", parts.join(","))
}

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
    /// Aplica a política do marco a um número de PR; com `corpo`, importa o
    /// bloco `pinker-change` e grava o manifesto versionado. Com `check`,
    /// valida sem escrever (modo somente-leitura).
    ImportarPr {
        pr: u64,
        corpo: Option<String>,
        check: bool,
    },
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
    json: bool,
    limite: Option<usize>,
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
    json: bool,
    limite: Option<usize>,
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
// @pinker-nav:end cli.config.modelos

// @pinker-nav:start cli.ajuda.usage
// @pinker-nav:domain ajuda
// @pinker-nav:layer cli
// @pinker-nav:summary Funções que montam as strings de uso/ajuda (usage, nav_usage, doc_usage, build_usage, editor_usage, repl_usage) impressas em stderr quando `--help`/`-h` é pedido ou o parsing rejeita os argumentos; cada uma apenas formata texto com format!, sem side effects.
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
           --repo      raiz do repositório (padrão: .)\n\
           --json      saída estável em JSON (mostrar/buscar/listar)\n\
           --limite N  máximo de resultados (1..20; buscar=10)\n\
         \n\
         Códigos de saída: 0 sucesso · 2 uso inválido · 3 catálogo ausente/inválido\n\
                           · 4 sem resultado · 5 fonte/âncora divergente\n",
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
           importar-pr <n>     aplica a política do marco a um PR (E-DOC-BASELINE);\n\
                               com --corpo <arquivo>, importa o bloco pinker-change\n\
                               e grava .pinker/changes/pr-<n>.yaml;\n\
                               com --check, valida sem escrever\n\
           mostrar <id>        extrai a seção/documento pelo id semântico\n\
           listar <territorio> lista documentos de um território (domain)\n\
           buscar <consulta>   busca seções por id, título, tags, aliases, resumo\n\
           rota <consulta>     melhores destinos para uma intenção\n\
           sincronizar         regenera o catálogo docs/navigation.jsonl\n\
           verificar           valida documentação e catálogo (não corrige)\n\
         \n\
         Opções:\n\
           --repo      raiz do repositório (padrão: .)\n\
           --corpo     arquivo com o corpo do PR (para importar-pr)\n\
           --check     valida sem escrever (importar-pr)\n\
           --json      saída estável em JSON (mostrar/buscar/rota/listar)\n\
           --limite N  máximo de resultados (1..20; rota=5, buscar=10)\n\
         \n\
         Códigos de saída: 0 sucesso · 2 uso inválido · 3 catálogo ausente/inválido\n\
                           · 4 sem resultado · 5 fonte/âncora divergente\n",
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
// @pinker-nav:end cli.ajuda.usage

// @pinker-nav:start cli.parsing.subcomandos
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary Parsers de argumentos por subcomando (parse_build_args, parse_editor_args, parse_repl_args, parse_doc_args, parse_nav_args): percorrem `args: &[String]` reconhecendo flags (--out-dir, --nativo, --repo, --corpo, --check, --json, --limite, --help/-h) e o argumento posicional de entrada/subcomando, retornando Result<Config..., String> com a mensagem de uso correspondente em caso de flag desconhecida ou argumento ausente.
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
    let mut corpo: Option<String> = None;
    let mut check = false;
    let mut json = false;
    let mut limite: Option<usize> = None;
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
            "--corpo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--corpo' requer um caminho de arquivo.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                corpo = Some(args[i].clone());
            }
            "--check" => check = true,
            "--json" => json = true,
            "--limite" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--limite' requer um valor.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                let raw = &args[i];
                let value = raw.parse::<usize>().map_err(|_| {
                    format!(
                        "Valor de '--limite' inválido: '{}'\n\n{}",
                        raw,
                        doc_usage(binary)
                    )
                })?;
                limite = Some(value);
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
            DocSub::ImportarPr { pr, corpo, check }
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

    Ok(DocConfigCli {
        repo,
        json,
        limite,
        sub,
    })
}

fn parse_nav_args(binary: &str, args: &[String]) -> Result<NavConfigCli, String> {
    let mut repo = ".".to_string();
    let mut json = false;
    let mut limite: Option<usize> = None;
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
            "--json" => json = true,
            "--limite" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--limite' requer um valor.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                let raw = &args[i];
                let value = raw.parse::<usize>().map_err(|_| {
                    format!(
                        "Valor de '--limite' inválido: '{}'\n\n{}",
                        raw,
                        nav_usage(binary)
                    )
                })?;
                limite = Some(value);
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

    Ok(NavConfigCli {
        repo,
        json,
        limite,
        sub,
    })
}
// @pinker-nav:end cli.parsing.subcomandos

// @pinker-nav:start cli.parsing.roteamento
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary parse_args: lê env::args(), separa o argv em flag_args e runtime_tail (delimitados por '--'), despacha para build/editor/repl/doc/nav quando o primeiro argumento bate um desses nomes, senão interpreta as flags de análise (--tokens/--ast/--json-ast/--ir/--cfg-ir/--selected/--machine/--pseudo-asm/--asm-s (aliases --asm/--s)/--run/--check) e monta CliCommand::Analyze(Config); erros retornam Err(String) com a mensagem de usage.
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
// @pinker-nav:end cli.parsing.roteamento

// @pinker-nav:start cli.execucao.entrada
// @pinker-nav:domain execucao
// @pinker-nav:layer cli
// @pinker-nav:summary try_or_exit! extrai um Result::Ok ou imprime o erro renderizado com a fonte e chama std::process::exit(1); main() chama parse_args, e em Err imprime a mensagem e sai com EXIT_USAGE (para doc/nav) ou 1 (demais), senão despacha CliCommand para run_analyze/run_build/run_editor/run_repl/run_doc/run_nav; scan_code chama nav::CodeIndex::scan_repo e sai com 1 em Err; run_nav roteia NavSub para run_nav_mostrar/buscar/listar/sincronizar/verificar.
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
            // Uso inválido de `doc`/`nav` sai com 2 (§7.4); demais mantêm 1.
            let raw: Vec<String> = env::args().collect();
            let is_trama = matches!(raw.get(1).map(String::as_str), Some("doc") | Some("nav"));
            std::process::exit(if is_trama { EXIT_USAGE } else { 1 });
        }
    };

    match command {
        CliCommand::Analyze(config) => run_analyze(config),
        CliCommand::Build(config) => run_build(config),
        CliCommand::Editor(config) => run_editor(config),
        CliCommand::Repl(config) => run_repl(config),
        CliCommand::Doc(config) => std::process::exit(run_doc(config)),
        CliCommand::Nav(config) => std::process::exit(run_nav(config)),
    }
}

fn scan_code(repo_root: &Path) -> nav::CodeIndex {
    match nav::CodeIndex::scan_repo(repo_root) {
        Ok(index) => index,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run_nav(config: NavConfigCli) -> i32 {
    let repo_root = Path::new(&config.repo);
    match config.sub {
        NavSub::Mostrar { key } => run_nav_mostrar(repo_root, &key, config.json),
        NavSub::Buscar { consulta } => {
            run_nav_buscar(repo_root, &consulta, config.json, config.limite)
        }
        NavSub::Listar { seletor } => run_nav_listar(repo_root, &seletor, config.json),
        NavSub::Sincronizar => run_nav_sincronizar(repo_root),
        NavSub::Verificar => run_nav_verificar(repo_root),
    }
}
// @pinker-nav:end cli.execucao.entrada

// @pinker-nav:start cli.nav.consulta
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:summary load_code_catalog lê o catálogo gerado (nav::CodeCatalog::load) sem escrever; run_nav_mostrar extrai uma região por chave e, via nav::validate_region, verifica se o marcador/hash da fonte ainda bate com o catálogo antes de imprimir o conteúdo (texto ou JSON), retornando EXIT_SOURCE em divergência; run_nav_buscar e run_nav_listar apenas consultam o catálogo em memória (busca textual e filtro por camada/domínio) e imprimem os resultados — nenhuma das três funções grava em disco.
/// Carrega o catálogo de código versionado (superfície de consulta — §5).
fn load_code_catalog(repo_root: &Path) -> Result<nav::CodeCatalog, i32> {
    let doc_config = load_doc_config(repo_root);
    let path = repo_root.join(doc_config.generated.code_index.clone());
    match nav::CodeCatalog::load(&path) {
        Ok(catalog) => Ok(catalog),
        Err(err) => {
            eprintln!("{err}");
            Err(EXIT_CATALOG)
        }
    }
}

fn run_nav_mostrar(repo_root: &Path, key: &str, json: bool) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let Some(region) = catalog.region(key) else {
        eprintln!("chave de código não encontrada: '{key}'. Tente `pink nav buscar \"{key}\"`.");
        return EXIT_NORESULT;
    };
    let path = repo_root.join(&region.file);
    let source = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "E-NAV-SOURCE\nFalha ao ler fonte '{}': {}",
                path.display(),
                err
            );
            return EXIT_SOURCE;
        }
    };
    match nav::validate_region(&source, region) {
        nav::RegionCheck::Ok => {}
        nav::RegionCheck::AnchorDrift => {
            eprintln!(
                "E-NAV-SOURCE\nMarcador divergente para '{}' em {}; catálogo desatualizado. Rode `pink nav sincronizar`.",
                region.key, region.file
            );
            return EXIT_SOURCE;
        }
        nav::RegionCheck::HashMismatch { expected, found } => {
            eprintln!(
                "E-NAV-SOURCE\nHash divergente para '{}' em {} (esperado {}, obtido {}); catálogo desatualizado. Rode `pink nav sincronizar`.",
                region.key, region.file, expected, found
            );
            return EXIT_SOURCE;
        }
    }
    let content = nav::extract_region_content(&source, region);
    if json {
        let mut out = String::new();
        out.push_str("{\"schema\":1");
        out.push_str(&format!(",\"key\":{}", json_escape(&region.key)));
        out.push_str(&format!(",\"kind\":{}", json_escape(&region.kind)));
        if let Some(domain) = &region.domain {
            out.push_str(&format!(",\"domain\":{}", json_escape(domain)));
        }
        if let Some(layer) = &region.layer {
            out.push_str(&format!(",\"layer\":{}", json_escape(layer)));
        }
        if let Some(phase) = region.phase {
            out.push_str(&format!(",\"phase\":{}", phase));
        }
        out.push_str(&format!(",\"file\":{}", json_escape(&region.file)));
        out.push_str(&format!(",\"content_start\":{}", region.content_start));
        out.push_str(&format!(",\"content_end\":{}", region.content_end));
        out.push_str(&format!(",\"hash\":{}", json_escape(&region.hash)));
        out.push_str(&format!(
            ",\"content\":{}",
            json_escape(&content.join("\n"))
        ));
        out.push('}');
        println!("{out}");
    } else {
        println!(
            "// {} — {}:{}-{}",
            region.key, region.file, region.content_start, region.content_end
        );
        if !region.summary.is_empty() {
            println!("// {}", region.summary);
        }
        println!();
        for line in &content {
            println!("{line}");
        }
    }
    EXIT_OK
}

fn run_nav_buscar(repo_root: &Path, consulta: &str, json: bool, limite: Option<usize>) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_BUSCAR);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            println!(
                "{{\"schema\":1,\"query\":{},\"normalized\":{},\"results\":[]}}",
                json_escape(consulta),
                json_escape(&pinker_v0::text_norm::normalize(consulta))
            );
        } else {
            eprintln!("Nenhuma região encontrada para: {consulta}");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&nav::CodeRegion> = hits.into_iter().take(limit).collect();
    if json {
        let results: Vec<String> = shown
            .iter()
            .map(|r| {
                let mut o = String::from("{");
                o.push_str(&format!("\"key\":{}", json_escape(&r.key)));
                if let Some(domain) = &r.domain {
                    o.push_str(&format!(",\"domain\":{}", json_escape(domain)));
                }
                if let Some(layer) = &r.layer {
                    o.push_str(&format!(",\"layer\":{}", json_escape(layer)));
                }
                o.push_str(&format!(",\"file\":{}", json_escape(&r.file)));
                o.push_str(&format!(",\"content_start\":{}", r.content_start));
                o.push_str(&format!(",\"content_end\":{}", r.content_end));
                if !r.summary.is_empty() {
                    o.push_str(&format!(",\"summary\":{}", json_escape(&r.summary)));
                }
                o.push('}');
                o
            })
            .collect();
        println!(
            "{{\"schema\":1,\"query\":{},\"normalized\":{},\"results\":[{}]}}",
            json_escape(consulta),
            json_escape(&pinker_v0::text_norm::normalize(consulta)),
            results.join(",")
        );
    } else {
        for region in shown {
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
    EXIT_OK
}

fn run_nav_listar(repo_root: &Path, seletor: &str, json: bool) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let regions = catalog.list(seletor);
    if regions.is_empty() {
        if json {
            println!("{{\"selector\":{},\"results\":[]}}", json_escape(seletor));
        } else {
            eprintln!("Nenhuma região na camada/domínio '{seletor}'.");
        }
        return EXIT_NORESULT;
    }
    if json {
        let results: Vec<String> = regions.iter().map(|r| json_escape(&r.key)).collect();
        println!(
            "{{\"selector\":{},\"results\":[{}]}}",
            json_escape(seletor),
            results.join(",")
        );
    } else {
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
    EXIT_OK
}
// @pinker-nav:end cli.nav.consulta

// @pinker-nav:start cli.nav.sincronizacao-verificacao
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:summary run_nav_sincronizar reescaneia o repositório (scan_code), roda index.verify() e só grava src/navigation.jsonl via write_atomic quando não há divergências (senão retorna EXIT_SOURCE sem tocar o arquivo); run_nav_verificar também reescaneia e roda verify(), compara o conteúdo renderizado com o arquivo em disco e reporta divergências em stderr, mas não escreve — é somente leitura, retornando EXIT_SOURCE em caso de erro e EXIT_OK caso contrário.
fn run_nav_sincronizar(repo_root: &Path) -> i32 {
    let doc_config = load_doc_config(repo_root);
    let index = scan_code(repo_root);
    // Validação antes de escrever (§8): não sobrescreve catálogo válido com
    // árvore inválida.
    let problems = index.verify();
    if !problems.is_empty() {
        eprintln!(
            "E-NAV-SYNC: {} divergência(s); catálogo NÃO alterado.",
            problems.len()
        );
        for problem in &problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }
    let rendered = index.render_jsonl();
    let path = repo_root.join(&doc_config.generated.code_index);
    if let Err(code) = write_atomic(&path, &rendered) {
        return code;
    }
    println!(
        "Catálogo de código sincronizado: {} ({} regiões).",
        doc_config.generated.code_index,
        index.regions.len()
    );
    EXIT_OK
}

fn run_nav_verificar(repo_root: &Path) -> i32 {
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
        return EXIT_OK;
    }
    eprintln!(
        "E-NAV-VERIFY: {} divergência(s) encontrada(s):",
        errors.len()
    );
    for error in &errors {
        eprintln!("  - {error}");
    }
    EXIT_SOURCE
}
// @pinker-nav:end cli.nav.sincronizacao-verificacao

// @pinker-nav:start cli.doc.consulta
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary load_doc_config carrega doc::DocConfig::load (sai com 1 em erro); run_doc despacha DocSub (Marco/ImportarPr/Mostrar/Listar/Buscar/Rota/Sincronizar/Verificar) para as funções correspondentes; scan_docs varre docs/ via doc_index::DocIndex::scan; load_doc_catalog lê o catálogo gerado; write_atomic é o único mecanismo desta base que grava atomicamente — escreve um arquivo `.jsonl.tmp` e usa fs::rename por cima do caminho final, usado pelas rotinas de sincronização (não pelas consultas abaixo); run_doc_mostrar/run_doc_listar/run_doc_buscar/run_doc_rota e print_doc_results_json apenas leem o catálogo e imprimem resultados em texto ou JSON, sem escrever em disco.
fn load_doc_config(repo_root: &Path) -> doc::DocConfig {
    match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run_doc(config: DocConfigCli) -> i32 {
    let repo_root = Path::new(&config.repo);
    let doc_config = match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            return EXIT_CATALOG;
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
            EXIT_OK
        }
        DocSub::ImportarPr { pr, corpo, check } => {
            if let Err(rejection) = doc_config.baseline_gate(pr) {
                eprintln!("{rejection}");
                return EXIT_SOURCE;
            }
            match corpo {
                None => {
                    println!(
                        "PR #{pr} posterior ao marco #{} — elegível para importação.",
                        doc_config.github.baseline_pr
                    );
                    println!(
                        "Forneça --corpo <arquivo> para gerar o manifesto .pinker/changes/pr-{pr}.yaml."
                    );
                    EXIT_OK
                }
                Some(corpo) => run_doc_importar(repo_root, &doc_config, pr, &corpo, check),
            }
        }
        DocSub::Mostrar { id } => run_doc_mostrar(repo_root, &doc_config, &id, config.json),
        DocSub::Listar { territorio } => {
            run_doc_listar(repo_root, &doc_config, &territorio, config.json)
        }
        DocSub::Buscar { consulta } => run_doc_buscar(
            repo_root,
            &doc_config,
            &consulta,
            config.json,
            config.limite,
        ),
        DocSub::Rota { consulta } => run_doc_rota(
            repo_root,
            &doc_config,
            &consulta,
            config.json,
            config.limite,
        ),
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

/// Carrega o catálogo documental versionado (superfície de consulta — §5).
fn load_doc_catalog(
    repo_root: &Path,
    config: &doc::DocConfig,
) -> Result<doc_index::DocCatalog, i32> {
    let path = repo_root.join(&config.generated.docs_index);
    match doc_index::DocCatalog::load(&path) {
        Ok(catalog) => Ok(catalog),
        Err(err) => {
            eprintln!("{err}");
            Err(EXIT_CATALOG)
        }
    }
}

/// Escrita atômica: grava em arquivo temporário e renomeia por cima (§8).
fn write_atomic(path: &Path, content: &str) -> Result<(), i32> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("Falha ao criar '{}': {}", parent.display(), err);
            return Err(1);
        }
    }
    let tmp = path.with_extension("jsonl.tmp");
    if let Err(err) = fs::write(&tmp, content) {
        eprintln!("Falha ao gravar temporário '{}': {}", tmp.display(), err);
        return Err(1);
    }
    if let Err(err) = fs::rename(&tmp, path) {
        eprintln!(
            "Falha ao substituir '{}' por '{}': {}",
            path.display(),
            tmp.display(),
            err
        );
        let _ = fs::remove_file(&tmp);
        return Err(1);
    }
    Ok(())
}

fn run_doc_mostrar(repo_root: &Path, config: &doc::DocConfig, id: &str, json: bool) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if let Some(section) = catalog.section(id) {
        let path = repo_root.join(&section.file);
        let source = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                eprintln!(
                    "E-DOC-SOURCE\nFalha ao ler fonte '{}': {}",
                    path.display(),
                    err
                );
                return EXIT_SOURCE;
            }
        };
        // Valida que a âncora ainda delimita o intervalo registrado (§5).
        if !doc_index::validate_section_anchor(&source, section) {
            eprintln!(
                "E-DOC-SOURCE\nÂncora divergente para '{}' em {}; catálogo desatualizado. Rode `pink doc sincronizar`.",
                section.id, section.file
            );
            return EXIT_SOURCE;
        }
        let lines: Vec<&str> = source.lines().collect();
        let start = section.start.saturating_sub(1);
        let end = section.end.min(lines.len());
        let content: Vec<&str> = lines[start..end].to_vec();
        if json {
            let mut out = String::new();
            out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
            out.push_str(",\"record\":\"section\"");
            out.push_str(&format!(",\"id\":{}", json_escape(&section.id)));
            out.push_str(&format!(",\"document\":{}", json_escape(&section.document)));
            out.push_str(&format!(",\"file\":{}", json_escape(&section.file)));
            out.push_str(&format!(",\"start\":{}", section.start));
            out.push_str(&format!(",\"end\":{}", section.end));
            out.push_str(&format!(",\"title\":{}", json_escape(&section.title)));
            if !section.summary.is_empty() {
                out.push_str(&format!(",\"summary\":{}", json_escape(&section.summary)));
            }
            out.push_str(&format!(
                ",\"content\":{}",
                json_escape(&content.join("\n"))
            ));
            out.push('}');
            println!("{out}");
        } else {
            println!(
                "# {} — {}:{}-{}",
                section.id, section.file, section.start, section.end
            );
            if !section.summary.is_empty() {
                println!("# {}", section.summary);
            }
            println!();
            for line in &content {
                println!("{line}");
            }
        }
        return EXIT_OK;
    }

    if let Some(doc) = catalog.document(id) {
        let sections = catalog.sections_of(&doc.id);
        if json {
            let mut out = String::new();
            out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
            out.push_str(",\"record\":\"document\"");
            out.push_str(&format!(",\"id\":{}", json_escape(&doc.id)));
            out.push_str(&format!(",\"domain\":{}", json_escape(&doc.domain)));
            out.push_str(&format!(",\"kind\":{}", json_escape(&doc.kind)));
            out.push_str(&format!(",\"status\":{}", json_escape(&doc.status)));
            out.push_str(&format!(",\"file\":{}", json_escape(&doc.file)));
            if !doc.canonical_for.is_empty() {
                out.push_str(&format!(
                    ",\"canonical_for\":{}",
                    json_string_array(&doc.canonical_for)
                ));
            }
            let ids: Vec<String> = sections.iter().map(|s| s.id.clone()).collect();
            out.push_str(&format!(",\"sections\":{}", json_string_array(&ids)));
            out.push('}');
            println!("{out}");
        } else {
            println!("# documento {} ({})", doc.id, doc.kind);
            println!("  território: {}", doc.domain);
            println!("  arquivo:    {}", doc.file);
            if !doc.canonical_for.is_empty() {
                println!("  autoridade: {}", doc.canonical_for.join(", "));
            }
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
        }
        return EXIT_OK;
    }

    eprintln!("id documental não encontrado: '{id}'. Tente `pink doc buscar \"{id}\"`.");
    EXIT_NORESULT
}

fn run_doc_listar(repo_root: &Path, config: &doc::DocConfig, territorio: &str, json: bool) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let docs = catalog.documents_in_domain(territorio);
    if docs.is_empty() {
        if json {
            println!(
                "{{\"domain\":{},\"documents\":[]}}",
                json_escape(territorio)
            );
        } else {
            eprintln!("Nenhum documento estrutural no território '{territorio}'.");
        }
        return EXIT_NORESULT;
    }
    if json {
        let ids: Vec<String> = docs.iter().map(|d| d.id.clone()).collect();
        println!(
            "{{\"domain\":{},\"documents\":{}}}",
            json_escape(territorio),
            json_string_array(&ids)
        );
    } else {
        println!("Território '{territorio}':");
        for doc in docs {
            println!("- {} [{}] {}", doc.id, doc.kind, doc.file);
            for section in catalog.sections_of(&doc.id) {
                println!("    · {} — {}", section.id, section.title);
            }
        }
    }
    EXIT_OK
}

fn run_doc_buscar(
    repo_root: &Path,
    config: &doc::DocConfig,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_BUSCAR);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            print_doc_results_json(consulta, &[], None);
        } else {
            eprintln!("Nenhuma seção encontrada para: {consulta}");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&doc_index::SearchHit> = hits.iter().take(limit).collect();
    if json {
        print_doc_results_json(consulta, &shown, None);
    } else {
        for hit in &shown {
            println!("{}", hit.id);
            println!("   {}", hit.summary);
            println!("   {}:{}-{}", hit.file, hit.start, hit.end);
        }
    }
    EXIT_OK
}

fn run_doc_rota(
    repo_root: &Path,
    config: &doc::DocConfig,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_ROTA);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            print_doc_results_json(consulta, &[], None);
        } else {
            println!("Consulta: {consulta}");
            eprintln!("Nenhuma rota encontrada. Tente `pink doc buscar`.");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&doc_index::SearchHit> = hits.iter().take(limit).collect();
    let next = format!("pink doc mostrar {}", shown[0].id);
    if json {
        print_doc_results_json(consulta, &shown, Some(&next));
    } else {
        println!("Consulta: {consulta}");
        println!();
        for (i, hit) in shown.iter().enumerate() {
            println!("{}. {}", i + 1, hit.id);
            println!("   {}", hit.summary);
            println!("   {}:{}-{}", hit.file, hit.start, hit.end);
        }
        println!();
        println!("Use:");
        println!("    {next}");
    }
    EXIT_OK
}

/// Saída JSON estável de resultados de `buscar`/`rota` (§7.2).
fn print_doc_results_json(consulta: &str, hits: &[&doc_index::SearchHit], next: Option<&str>) {
    let results: Vec<String> = hits
        .iter()
        .map(|h| {
            let mut o = String::from("{");
            o.push_str(&format!("\"id\":{}", json_escape(&h.id)));
            o.push_str(&format!(",\"score\":{}", h.score));
            o.push_str(&format!(",\"file\":{}", json_escape(&h.file)));
            o.push_str(&format!(",\"start\":{}", h.start));
            o.push_str(&format!(",\"end\":{}", h.end));
            o.push_str(&format!(",\"summary\":{}", json_escape(&h.summary)));
            o.push_str(&format!(
                ",\"next\":{}",
                json_escape(&format!("pink doc mostrar {}", h.id))
            ));
            o.push('}');
            o
        })
        .collect();
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
    out.push_str(&format!(",\"query\":{}", json_escape(consulta)));
    out.push_str(&format!(
        ",\"normalized\":{}",
        json_escape(&pinker_v0::text_norm::normalize(consulta))
    ));
    out.push_str(&format!(",\"results\":[{}]", results.join(",")));
    if let Some(next) = next {
        out.push_str(&format!(",\"next\":{}", json_escape(next)));
    }
    out.push('}');
    println!("{out}");
}
// @pinker-nav:end cli.doc.consulta

// @pinker-nav:start cli.doc.sincronizacao
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary run_doc_sincronizar reescaneia docs/ e manifestos de mudança, roda verify() em ambos e só prossegue se não houver divergência; calcula o plano de projeções (projection::plan), grava o catálogo via write_atomic, grava o histórico mecânico via write_ledger e aplica as escritas do plano (fs::write por projeção) — é a rotina que efetivamente altera arquivos em disco nesta região documental.
fn run_doc_sincronizar(repo_root: &Path, config: &doc::DocConfig) -> i32 {
    let index = scan_docs(repo_root);
    // Validação completa antes de qualquer escrita (§8): uma árvore inválida
    // nunca sobrescreve o último catálogo válido.
    let problems = index.verify();
    if !problems.is_empty() {
        eprintln!(
            "E-DOC-SYNC: {} divergência(s); catálogo e projeções NÃO alterados.",
            problems.len()
        );
        for problem in &problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }
    let manifests = change::Manifests::load(&repo_root.join(".pinker/changes"));
    if !manifests.problems.is_empty() {
        eprintln!(
            "E-DOC-SYNC: {} problema(s) em manifestos; nada alterado.",
            manifests.problems.len()
        );
        for problem in &manifests.problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }

    // Renderiza tudo em memória antes de tocar o disco.
    let rendered = index.render_jsonl();
    let catalog_path = repo_root.join(&config.generated.docs_index);

    // Projeções documentais (§12): calculadas em memória e validadas.
    let plan = match projection::plan(repo_root, config, &manifests) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("{err}");
            return EXIT_SOURCE;
        }
    };

    // Escrita atômica do catálogo.
    if let Err(code) = write_atomic(&catalog_path, &rendered) {
        return code;
    }
    if let Err(code) = write_ledger(repo_root, &manifests) {
        return code;
    }
    // Aplica as projeções (regiões geradas) idempotentemente.
    for change in &plan.writes {
        if let Err(err) = fs::write(&change.path, &change.content) {
            eprintln!(
                "Falha ao gravar projeção '{}': {}",
                change.path.display(),
                err
            );
            return 1;
        }
    }

    println!(
        "Catálogo documental sincronizado: {} ({} documentos, {} seções).",
        config.generated.docs_index,
        index.documents.len(),
        index.sections.len()
    );
    println!(
        "Histórico mecânico sincronizado: {} ({} manifesto(s)).",
        LEDGER_REL,
        manifests.changes.len()
    );
    if !plan.writes.is_empty() {
        println!("Projeções aplicadas: {}.", plan.summary());
    }
    EXIT_OK
}
// @pinker-nav:end cli.doc.sincronizacao

// @pinker-nav:start cli.doc.mudancas
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary LEDGER_REL é o caminho fixo do histórico mecânico (.pinker/changes/index.jsonl); write_ledger renderiza os manifestos e grava via write_atomic, ou remove o arquivo quando não há manifestos; run_doc_importar lê o corpo de um PR, parseia e valida o manifesto (change::Change::parse_pr_body/validate), e com `check=true` apenas reporta se o manifesto seria criado sem gravar; sem `check`, grava o YAML em .pinker/changes/pr-<n>.yaml e atualiza o ledger — imutabilidade: se o manifesto já existe com conteúdo idêntico, é tratado como idempotente; conteúdo diferente retorna erro (change::immutable_error).
const LEDGER_REL: &str = ".pinker/changes/index.jsonl";

fn write_ledger(repo_root: &Path, manifests: &change::Manifests) -> Result<(), i32> {
    let rendered = manifests.render_ledger();
    let path = repo_root.join(LEDGER_REL);
    if rendered.is_empty() {
        // Zero manifestos: não materializa arquivo (mantém a árvore limpa).
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    write_atomic(&path, &rendered)
}

fn run_doc_importar(
    repo_root: &Path,
    config: &doc::DocConfig,
    pr: u64,
    corpo: &str,
    check: bool,
) -> i32 {
    let body = match fs::read_to_string(corpo) {
        Ok(body) => body,
        Err(err) => {
            eprintln!("Falha ao ler corpo do PR '{}': {}", corpo, err);
            return EXIT_SOURCE;
        }
    };
    let mut manifest = match change::Change::parse_pr_body(&body) {
        Ok(manifest) => manifest,
        Err(err) => {
            eprintln!("{err}");
            return EXIT_SOURCE;
        }
    };
    if let Err(err) = manifest.validate() {
        eprintln!("{err}");
        return EXIT_SOURCE;
    }
    manifest.source = Some(change::Source {
        kind: "github-pr".to_string(),
        number: pr,
        repository: config.github.repository.clone(),
    });
    let rendered = manifest.render_yaml();

    let changes_dir = repo_root.join(".pinker/changes");
    let manifest_path = changes_dir.join(format!("pr-{pr}.yaml"));

    // Contrato de imutabilidade (§10): se já existe, o conteúdo canônico precisa
    // ser idêntico; conteúdo diferente falha com E-CHANGE-IMMUTABLE.
    if manifest_path.exists() {
        let existing = fs::read_to_string(&manifest_path).unwrap_or_default();
        if existing == rendered {
            if check {
                println!("Manifesto pr-{pr}.yaml já sincronizado (idempotente).");
            } else {
                println!("Manifesto pr-{pr}.yaml inalterado (idempotente).");
            }
            return EXIT_OK;
        }
        eprintln!("{}", change::immutable_error(pr));
        return EXIT_SOURCE;
    }

    if check {
        println!("Modo --check: manifesto pr-{pr}.yaml válido e ausente (seria criado).");
        return EXIT_OK;
    }

    if let Err(err) = fs::create_dir_all(&changes_dir) {
        eprintln!("Falha ao criar '{}': {}", changes_dir.display(), err);
        return 1;
    }
    if let Err(err) = fs::write(&manifest_path, &rendered) {
        eprintln!("Falha ao gravar '{}': {}", manifest_path.display(), err);
        return 1;
    }

    // Atualiza o histórico mecânico (idempotente por número de PR).
    let manifests = change::Manifests::load(&changes_dir);
    if let Err(code) = write_ledger(repo_root, &manifests) {
        return code;
    }

    println!(
        "Manifesto importado: .pinker/changes/pr-{pr}.yaml (fase {:?}, bloco {:?}).",
        manifest.phase, manifest.block
    );
    println!("Rode `pink doc sincronizar` e revise os documentos derivados.");
    EXIT_OK
}
// @pinker-nav:end cli.doc.mudancas

// @pinker-nav:start cli.doc.verificacao
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary run_doc_verificar reescaneia docs/ e manifestos, recomputa o catálogo, o ledger e o plano de projeções em memória e compara cada um com o conteúdo em disco (incluindo o baseline_gate por PR), acumulando divergências em `errors`/`manifest_errors`; não escreve em nenhum arquivo — reporta 'ok' ou a lista de divergências e retorna EXIT_OK/EXIT_SOURCE conforme o resultado.
fn run_doc_verificar(repo_root: &Path, config: &doc::DocConfig) -> i32 {
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

    // Manifestos de mudança (Etapa 4): esquema, número, baseline e histórico.
    let mut manifest_errors: Vec<String> = Vec::new();
    let changes_dir = repo_root.join(".pinker/changes");
    let manifests = change::Manifests::load(&changes_dir);
    for problem in &manifests.problems {
        manifest_errors.push(problem.to_string());
    }
    for manifest in &manifests.changes {
        if let Some(source) = &manifest.source {
            if let Err(rejection) = config.baseline_gate(source.number) {
                manifest_errors.push(format!(
                    "manifesto pr-{} anterior ou igual ao marco #{}",
                    source.number, rejection.baseline_pr
                ));
            }
        } else {
            manifest_errors.push(format!("manifesto '{}' sem source.number", manifest.title));
        }
    }
    let ledger_rendered = manifests.render_ledger();
    let ledger_on_disk = fs::read_to_string(repo_root.join(LEDGER_REL)).unwrap_or_default();
    if ledger_on_disk != ledger_rendered {
        manifest_errors.push(format!(
            "histórico mecânico '{}' dessincronizado; rode `pink doc sincronizar`",
            LEDGER_REL
        ));
    }

    // Projeções documentais (§12): compara o versionado com o gerado em memória.
    if manifests.problems.is_empty() {
        match projection::plan(repo_root, config, &manifests) {
            Ok(plan) => {
                for drift in plan.drift() {
                    manifest_errors.push(drift);
                }
            }
            Err(err) => manifest_errors.push(err.to_string()),
        }
    }

    if errors.is_empty() && manifest_errors.is_empty() {
        println!("Documentação, catálogo, manifestos e projeções verificados: ok.");
        return EXIT_OK;
    }
    let total = errors.len() + manifest_errors.len();
    eprintln!("E-DOC-VERIFY: {} divergência(s) encontrada(s):", total);
    for error in &errors {
        eprintln!("  - {error}");
    }
    for error in &manifest_errors {
        eprintln!("  - {error}");
    }
    EXIT_SOURCE
}
// @pinker-nav:end cli.doc.verificacao

// @pinker-nav:start cli.execucao.editor-repl
// @pinker-nav:domain execucao
// @pinker-nav:layer cli
// @pinker-nav:summary run_editor abre EditorTui::from_path e chama editor.run(); em Err de qualquer uma das duas chamadas, imprime o erro e chama std::process::exit(1). run_repl delega a repl::run_repl() (definido em outro módulo, não é um stub local) e, em Err, imprime e também sai com process::exit(1).
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
// @pinker-nav:end cli.execucao.editor-repl

// @pinker-nav:start cli.analise.pipeline
// @pinker-nav:domain analise
// @pinker-nav:layer cli
// @pinker-nav:summary run_analyze lê o arquivo de entrada e conduz o pipeline de análise: tokeniza, parseia, resolve imports (load_program_with_imports), roda a verificação semântica (semantic::check_program) e, conforme as flags do Config, cada etapa a jusante (IR, CFG IR, seleção de instruções, máquina abstrata, backend `.s` textual, execução via interpretador, backend pseudo-asm) só é computada se alguma flag de saída a exigir (`needs_ir`/`needs_cfg`/`needs_selected`/`needs_machine`); qualquer Err em qualquer etapa passa por try_or_exit! e termina com process::exit(1); esta função não monta nem linka um binário — a emissão `--asm-s` é apenas texto impresso, e `--run` executa via interpreter::run_program_with_args, não via processo nativo.
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
// @pinker-nav:end cli.analise.pipeline

// @pinker-nav:start cli.build.nativo
// @pinker-nav:domain build
// @pinker-nav:layer cli
// @pinker-nav:summary run_build repete o front-end (lex/parse/imports/semântica/IR/CFG/seleção) e grava o `.s` resultante em <out_dir>/<stem>.s via fs::write; com --nativo, emite via emit_external_toolchain_subset_nativo e, após gravar, chama link_nativo. locate_pinker_rt_lib localiza (não constrói) a staticlib libpinker_rt.a pré-buildada: usa a env PINKER_RT_LIB se apontar para um arquivo existente, senão procura ao lado do executável atual via std::env::current_exe; retorna Err com uma mensagem sugerindo `cargo build` se não encontrar. detect_cc_driver detecta um driver C disponível testando `cc --version`/`gcc --version`/`clang --version` via std::process::Command e retorna o primeiro que responder com status de sucesso. link_nativo invoca esse driver externo passando o `.s`, a staticlib localizada e -lpthread/-ldl/-lm para produzir o binário via -o; a montagem e a linkedição são feitas pelo driver externo, não por este arquivo.
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
// @pinker-nav:end cli.build.nativo

// @pinker-nav:start cli.modulos.importacao
// @pinker-nav:domain modulos
// @pinker-nav:layer cli
// @pinker-nav:summary parse_program_from_source tokeniza e parseia uma string de fonte (sem resolver imports). importable_item_name/importable_item_clone/qualified_type_item_clone extraem o nome, clonam ou requalificam (com prefixo `<módulo>.`) um ast::Item importável (Function/Const/Struct/TypeAlias/Enum/Trait). load_module_program lê o arquivo `<módulo>.pink` a partir de `base_dir`, detecta ciclo de módulos comparando com a pilha `loading` e recursa nos imports do módulo carregado antes de inserir o programa em `loaded`. load_program_with_imports é o ponto de entrada: para cada import do programa raiz, pula famílias built-in importáveis, detecta import duplicado pela chave `módulo::símbolo`, carrega o módulo via load_module_program e insere os itens importados (todo o módulo ou um símbolo específico) em `root_program.items`, reportando colisão de nome com itens locais ou com outro import antes de limpar `root_program.imports`.
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
// @pinker-nav:end cli.modulos.importacao
