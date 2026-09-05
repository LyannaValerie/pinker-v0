use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::backend_s;
use pinker_v0::backend_text;
use pinker_v0::backend_text_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::change;
use pinker_v0::diff_coverage;
use pinker_v0::doc;
use pinker_v0::doc_index;
use pinker_v0::editor_tui::EditorTui;
use pinker_v0::generic_identity::GenericOrigin;
use pinker_v0::inline_asm;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::lexer::Lexer;
use pinker_v0::module_graph::ModuleGraph;
use pinker_v0::module_resolve;
use pinker_v0::nav;
use pinker_v0::nav_projection_lifecycle::{self, ProjectionError};
use pinker_v0::nav_projection_report;
use pinker_v0::nav_projection_store::ProjectionStore;
use pinker_v0::parser::{ContextoDeImport, Parser};
use pinker_v0::printer;
use pinker_v0::project_state;
use pinker_v0::project_state_report;
use pinker_v0::projection;
use pinker_v0::repl;
use pinker_v0::semantic;
use pinker_v0::source_map::{SourceId, SourceMap};
use pinker_v0::symbol_index;
use pinker_v0::token::{Span, Token};
use pinker_v0::tooling;
use pinker_v0::{ast, error::PinkerError};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

// Decomposição física #605, unidade MAIN-5+2+3: a implementação destas três
// famílias mora em `src/pink_cli/`. A orquestração continua neste arquivo.
#[path = "pink_cli/cli_parsing.rs"]
mod cli_parsing;
#[path = "pink_cli/doc_cli.rs"]
mod doc_cli;
#[path = "pink_cli/modules.rs"]
mod modules;

use cli_parsing::parse_args;
use doc_cli::{load_doc_config, run_doc, write_atomic};
use modules::{base_dir_de, carregar_e_projetar, contexto_de_import};

// @pinker-nav:start cli.config.modelos
// @pinker-nav:domain config
// @pinker-nav:layer cli
// @pinker-nav:summary Constantes e helpers JSON, modelos dos comandos históricos e configurações de doctor/verificar usados pelo parsing e roteamento determinísticos da CLI.
/// Códigos de saída públicos da CLI e das consultas da Trama (especificação §7.4).
const EXIT_OK: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;
const EXIT_CATALOG: i32 = 3;
const EXIT_NORESULT: i32 = 4;
const EXIT_SOURCE: i32 = 5;
const EXIT_HARNESS: i32 = 6;
const EXIT_POLICY: i32 = 7;
const EXIT_STALE: i32 = 8;

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
        freeze: bool,
        artifact: Option<String>,
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
    Localizar { symbol: String },
    CoberturaDiff,
    Impacto { diff: String },
    Listar { seletor: String },
    Mapa { filtro: Option<String> },
    Sincronizar,
    Verificar,
    Projecao(ProjectionSub),
}

enum ProjectionSub {
    Listar,
    Mostrar {
        id: String,
        observado: bool,
    },
    Verificar {
        id: Option<String>,
    },
    Preparar {
        id: String,
        justificativa: Option<String>,
        predecessor: Option<String>,
        autorizar: Option<String>,
    },
    Aceitar {
        id: String,
        autorizar: Option<String>,
    },
}

struct NavConfigCli {
    repo: String,
    json: bool,
    limite: Option<usize>,
    sub: NavSub,
}

struct StateConfigCli {
    repo: String,
    json: bool,
}

struct DoctorConfigCli {
    repo: String,
    json: bool,
}

struct VerifyConfigCli {
    repo: String,
    diff: String,
    documentation_frozen: bool,
    corpo: Option<PathBuf>,
    json: bool,
}

enum CliCommand {
    Help(String),
    Version,
    VersionJson,
    Analyze(Config),
    Build(BuildConfig),
    Editor(EditorConfig),
    Repl(ReplConfig),
    Doc(DocConfigCli),
    Nav(NavConfigCli),
    State(StateConfigCli),
    Doctor(DoctorConfigCli),
    Verify(VerifyConfigCli),
}
// @pinker-nav:end cli.config.modelos

// @pinker-nav:start cli.ajuda.usage
// @pinker-nav:domain ajuda
// @pinker-nav:layer cli
// @pinker-nav:summary program_name reduz argv[0] ao componente final e as funções de ajuda formatam, sem side effects, a superfície principal e os nove comandos incluindo doctor e verificar.
fn program_name(argv0: Option<&String>) -> String {
    argv0
        .and_then(|raw| Path::new(raw).file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("pink")
        .to_string()
}

fn usage(program: &str) -> String {
    format!(
        "Uso: {program} [OPÇÕES] ARQUIVO [-- ARGS...]\n\
         Uso: {program} COMANDO [OPÇÕES]\n\
         Uso: {program} help [COMANDO]\n\
         \n\
         Opções principais:\n\
           -h, --help  exibe esta ajuda e termina com sucesso\n\
           -V, --version  exibe a versão do pacote e termina com sucesso\n\
           --version-json  exibe path, versão e commit do binário em JSON\n\
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
          nav         navegação semântica do código da Trama Pinker\n\
           estado      estado consolidado somente leitura do projeto\n\
           doctor      identidade e compatibilidade operacional do pink\n\
           verificar   preflight estruturado antes da suíte completa\n"
    )
}

fn state_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} estado [--repo DIRETÓRIO] [--json]\n\
         \n\
         Comando:\n\
           estado      consolida autoridades locais sem escrever nem usar rede\n\
         \n\
         Opções:\n\
           --repo DIRETÓRIO       ponto de partida para descobrir o repositório\n\
           --json                 JSON determinístico com schema público 1\n\
           -h, --help             exibe esta ajuda e termina com sucesso\n\
         \n\
         Códigos de saída: 0 relatório produzido · 1 falha interna\n\
                           · 2 uso inválido · 3 root/autoridade mínima ausente\n"
    )
}

fn doctor_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} doctor [--repo DIRETÓRIO] [--json]\n\
         Identifica binário/repositório e recomenda a próxima ação determinística.\n"
    )
}

fn verify_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} verificar --diff REF [--repo DIRETÓRIO] [--documentation-frozen] [--corpo ARQUIVO] --json\n\
         Compõe doctor, nav impacto, projeções, pinker-change e estado documental.\n"
    )
}

fn nav_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} nav SUBCOMANDO [--repo DIRETÓRIO] [ARGS...]\n\
         \n\
         Comando:\n\
           nav         navegação semântica do código da Trama Pinker\n\
         \n\
         Subcomandos:\n\
           mostrar CHAVE       extrai a região de código pela chave\n\
           buscar CONSULTA     busca regiões por chave, domínio, camada, resumo\n\
           localizar SÍMBOLO   resolve identidade estrutural e vínculos explícitos\n\
           cobertura-diff      relaciona unified diff de stdin a superfícies explícitas\n\
           impacto --diff REF  obtém e relaciona um diff Git sem mutar o repositório\n\
           listar SELETOR      lista regiões de uma camada (layer) ou domínio\n\
           mapa [FILTRO]       agrupa regiões por arquivo\n\
           sincronizar         regenera o catálogo src/navigation.jsonl\n\
           verificar           valida os marcadores e o catálogo (não corrige)\n\
           projecao            lifecycle dos snapshots históricos de navegação\n\
         \n\
         Opções:\n\
           --repo      raiz do repositório (padrão: .)\n\
           --json      saída estável em JSON (mostrar/buscar/localizar/cobertura-diff/impacto/listar/mapa)\n\
           --limite N  máximo de resultados (1..20; buscar=10)\n\
         \n\
         Códigos de saída: 0 sucesso · 2 uso inválido · 3 catálogo ausente/inválido\n\
                           · 4 sem resultado · 5 fonte/âncora ou drift\n\
                           · 6 harness · 7 política · 8 plano obsoleto\n",
    )
}

fn projection_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} nav projecao SUBCOMANDO [--repo DIRETÓRIO] [--json]\n\
         \n\
         Subcomandos:\n\
           listar\n\
           mostrar ID [--observado]\n\
           verificar [ID]\n\
           preparar ID --justificativa TEXTO --predecessor ID [--autorizar DIGEST]\n\
           aceitar ID [--autorizar DIGEST]\n\
         \n\
         Sem --autorizar, preparar e aceitar exibem plano e digest sem escrever.\n\
         Códigos adicionais: 6 harness · 7 política · 8 plano obsoleto\n"
    )
}

fn projection_subcommand_usage(binary: &str, command: &str) -> String {
    match command {
        "listar" => format!("Uso: {binary} nav projecao listar [--repo DIRETÓRIO] [--json]\n"),
        "mostrar" => format!("Uso: {binary} nav projecao mostrar ID [--observado] [--repo DIRETÓRIO] [--json]\n"),
        "verificar" => format!("Uso: {binary} nav projecao verificar [ID] [--repo DIRETÓRIO] [--json]\n"),
        "preparar" => format!("Uso: {binary} nav projecao preparar ID --justificativa TEXTO --predecessor ID [--autorizar DIGEST] [--repo DIRETÓRIO] [--json]\n"),
        "aceitar" => format!("Uso: {binary} nav projecao aceitar ID [--autorizar DIGEST] [--repo DIRETÓRIO] [--json]\n"),
        _ => projection_usage(binary),
    }
}

fn doc_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} doc SUBCOMANDO [--repo DIRETÓRIO] [ARGS...]\n\
         \n\
         Comando:\n\
           doc         ferramenta documental da Trama Pinker\n\
         \n\
         Subcomandos:\n\
           marco               exibe o marco documental configurado em {config}\n\
           importar-pr N       aplica a política do marco a um PR (E-DOC-BASELINE);\n\
                               com --corpo ARQUIVO, importa o bloco pinker-change\n\
                               e grava .pinker/changes/pr-N.yaml;\n\
                               com --check, valida sem escrever\n\
           mostrar ID          extrai a seção/documento pelo id semântico\n\
           listar TERRITÓRIO   lista documentos de um território (domain)\n\
           buscar CONSULTA     busca seções por id, título, tags, aliases, resumo\n\
           rota CONSULTA       melhores destinos para uma intenção\n\
           sincronizar         regenera o catálogo docs/navigation.jsonl\n\
           verificar           valida documentação e catálogo (não corrige)\n\
         \n\
         Opções:\n\
           --repo      raiz do repositório (padrão: .)\n\
           --corpo     arquivo com o corpo do PR (para importar-pr)\n\
           --check     valida sem escrever (importar-pr)\n\
           --freeze    valida e preserva artifact sem mutar documentação canônica\n\
           --artifact  destino obrigatório da evidência quando --freeze é usado\n\
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
        "Uso: {binary} build [--out-dir DIRETÓRIO] [--nativo] ARQUIVO\n\
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
        "Uso: {binary} editor ARQUIVO\n\
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

fn help_for_command(program: &str, command: &str) -> Option<String> {
    match command {
        "build" => Some(build_usage(program)),
        "editor" => Some(editor_usage(program)),
        "repl" => Some(repl_usage(program)),
        "doc" => Some(doc_usage(program)),
        "nav" => Some(nav_usage(program)),
        "estado" => Some(state_usage(program)),
        "doctor" => Some(doctor_usage(program)),
        "verificar" => Some(verify_usage(program)),
        _ => None,
    }
}
// @pinker-nav:end cli.ajuda.usage

// @pinker-nav:start cli.execucao.entrada
// @pinker-nav:domain execucao
// @pinker-nav:layer cli
// @pinker-nav:summary main preserva exits de domínio ao despachar análise e os nove comandos, incluindo adaptadores estruturados read-only para doctor, nav impacto e verificar.
/// Macro para encurtar o padrão "try or exit(1)" repetido no pipeline.
macro_rules! try_or_exit {
    ($result:expr, $sources:expr) => {
        match $result {
            Ok(val) => val,
            Err(err) => {
                // O trecho vem da fonte que o span reivindica. Passar o texto
                // primário aqui era o que fazia um erro de módulo ser desenhado
                // sobre a raiz.
                eprintln!("{}", err.render_for_cli_with_sources($sources));
                std::process::exit(EXIT_FAILURE);
            }
        }
    };
}

fn main() {
    let command = match parse_args() {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(EXIT_USAGE);
        }
    };

    match command {
        CliCommand::Help(help) => print!("{help}"),
        CliCommand::Version => println!("pink {}", env!("CARGO_PKG_VERSION")),
        CliCommand::VersionJson => match tooling::render_binary_identity_json() {
            Ok(identity) => println!("{identity}"),
            Err(error) => {
                eprintln!("E-IDENTITY: {error}");
                std::process::exit(EXIT_FAILURE);
            }
        },
        CliCommand::Analyze(config) => run_analyze(config),
        CliCommand::Build(config) => run_build(config),
        CliCommand::Editor(config) => run_editor(config),
        CliCommand::Repl(config) => run_repl(config),
        CliCommand::Doc(config) => std::process::exit(run_doc(config)),
        CliCommand::Nav(config) => std::process::exit(run_nav(config)),
        CliCommand::State(config) => std::process::exit(run_state(config)),
        CliCommand::Doctor(config) => std::process::exit(run_doctor(config)),
        CliCommand::Verify(config) => std::process::exit(run_verify(config)),
    }
}

fn run_doctor(config: DoctorConfigCli) -> i32 {
    match tooling::collect_doctor(Path::new(&config.repo)) {
        Ok(report) => {
            if config.json {
                println!("{}", tooling::render_doctor_json(&report));
            } else {
                println!("pink doctor");
                println!("  binary: {}", report.binary_path);
                println!("  version: {}", report.binary_version);
                println!("  commit: {}", report.binary_commit);
                println!("  repo: {} ({})", report.repo_root, report.repo_head);
                println!("  compatibility: {}", report.compatibility.as_str());
                println!("  navigation: {}", report.navigation_catalog);
                println!("  projections: {}", report.projection_state);
                println!("  next: {}", report.recommended_next_action);
            }
            if report.compatibility.usable() {
                EXIT_OK
            } else {
                EXIT_FAILURE
            }
        }
        Err(error) => {
            eprintln!("E-DOCTOR: {error}");
            EXIT_FAILURE
        }
    }
}

fn run_verify(config: VerifyConfigCli) -> i32 {
    match tooling::collect_preflight(
        Path::new(&config.repo),
        &config.diff,
        config.documentation_frozen,
        config.corpo.as_deref(),
    ) {
        Ok(report) => {
            if config.json {
                println!("{}", tooling::render_preflight_json(&report));
            } else {
                println!(
                    "status: {}",
                    if report.blocking.is_empty() {
                        "READY"
                    } else {
                        "BLOCKED"
                    }
                );
                println!("blocking: {}", report.blocking.len());
                println!("warnings: {}", report.warnings.len());
                println!("expected_deferred: {}", report.expected_deferred.len());
            }
            tooling::preflight_exit_code(&report)
        }
        Err(error) => {
            eprintln!("E-PREFLIGHT: {error}");
            EXIT_FAILURE
        }
    }
}

fn run_state(config: StateConfigCli) -> i32 {
    match project_state::collect(Path::new(&config.repo)) {
        Ok(state) => {
            if config.json {
                println!("{}", project_state_report::render_json(&state));
            } else {
                print!("{}", project_state_report::render_human(&state));
            }
            EXIT_OK
        }
        Err(project_state::CollectError::Root(error)) => {
            eprintln!("E-STATE-ROOT: {error}");
            EXIT_CATALOG
        }
    }
}

fn scan_code(repo_root: &Path) -> nav::CodeIndex {
    match nav::CodeIndex::scan_repo(repo_root) {
        Ok(index) => index,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(EXIT_FAILURE);
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
        NavSub::Localizar { symbol } => run_nav_localizar(repo_root, &symbol, config.json),
        NavSub::CoberturaDiff => run_nav_cobertura_diff(repo_root, config.json),
        NavSub::Impacto { diff } => run_nav_impacto(repo_root, &diff, config.json),
        NavSub::Listar { seletor } => run_nav_listar(repo_root, &seletor, config.json),
        NavSub::Mapa { filtro } => run_nav_mapa(repo_root, filtro.as_deref(), config.json),
        NavSub::Sincronizar => run_nav_sincronizar(repo_root),
        NavSub::Verificar => run_nav_verificar(repo_root),
        NavSub::Projecao(command) => run_nav_projecao(repo_root, config.json, command),
    }
}
// @pinker-nav:end cli.execucao.entrada

// @pinker-nav:start cli.nav.projecao
// @pinker-nav:domain projecoes
// @pinker-nav:layer cli
// @pinker-nav:summary Adaptador final `pink nav projecao`: despacha listar, mostrar, verificar, preparar e aceitar; descobre root pelo automation core, deriva texto e JSON dos mesmos modelos, recalcula planos antes de toda autorização e preserva exits distintos para drift, harness, política e stale.
fn run_nav_projecao(repo: &Path, json: bool, command: ProjectionSub) -> i32 {
    let root = match pinker_v0::automation::RepoRoot::discover(repo) {
        Ok(root) => root,
        Err(error) => {
            return print_projection_error("projecao", json, &ProjectionError::Automation(error))
        }
    };
    match command {
        ProjectionSub::Listar => {
            let store = match ProjectionStore::load(root.path()) {
                Ok(store) => store,
                Err(error) => {
                    return print_projection_error(
                        "listar",
                        json,
                        &ProjectionError::Authority(error),
                    )
                }
            };
            if json {
                println!("{}", nav_projection_report::render_inventory_json(&store));
            } else {
                print!("{}", nav_projection_report::render_inventory_human(&store));
            }
            if store.errors().is_empty() {
                EXIT_OK
            } else {
                EXIT_HARNESS
            }
        }
        ProjectionSub::Mostrar { id, observado } => {
            run_projection_show(&root, json, &id, observado)
        }
        ProjectionSub::Verificar { id } => run_projection_verify(&root, json, id.as_deref()),
        ProjectionSub::Preparar {
            id,
            justificativa,
            predecessor,
            autorizar,
        } => {
            let Some(justification) = justificativa else {
                return print_projection_error(
                    "preparar",
                    json,
                    &ProjectionError::Policy {
                        message: "--justificativa é obrigatória".to_string(),
                    },
                );
            };
            let Some(predecessor) = predecessor else {
                return print_projection_error(
                    "preparar",
                    json,
                    &ProjectionError::Policy {
                        message: "--predecessor é obrigatório".to_string(),
                    },
                );
            };
            let catalog = match load_projection_catalog(&root) {
                Ok(catalog) => catalog,
                Err(error) => return print_projection_error("preparar", json, &error),
            };
            let planning = match nav_projection_lifecycle::plan_prepare(
                &root,
                &catalog.regions,
                &id,
                &predecessor,
                &justification,
            ) {
                Ok(planning) => planning,
                Err(error) => return print_projection_error("preparar", json, &error),
            };
            match autorizar {
                None => {
                    if json {
                        println!(
                            "{}",
                            nav_projection_report::render_plan_json("preparar", &planning)
                        );
                    } else {
                        print!("{}", nav_projection_report::render_plan_human(&planning));
                    }
                    EXIT_OK
                }
                Some(digest) => match nav_projection_lifecycle::apply_prepare(
                    &root,
                    &catalog.regions,
                    &planning,
                    &digest,
                ) {
                    Ok(applied) => {
                        if json {
                            println!(
                                "{}",
                                nav_projection_report::render_apply_json("preparar", &applied)
                            );
                        } else {
                            print!("{}", nav_projection_report::render_apply_human(&applied));
                        }
                        EXIT_OK
                    }
                    Err(error) => print_projection_error("preparar", json, &error),
                },
            }
        }
        ProjectionSub::Aceitar { id, autorizar } => {
            let catalog = match load_projection_catalog(&root) {
                Ok(catalog) => catalog,
                Err(error) => return print_projection_error("aceitar", json, &error),
            };
            let planning = match nav_projection_lifecycle::plan_accept(&root, &catalog.regions, &id)
            {
                Ok(planning) => planning,
                Err(error) => return print_projection_error("aceitar", json, &error),
            };
            match autorizar {
                None => {
                    if json {
                        println!(
                            "{}",
                            nav_projection_report::render_plan_json("aceitar", &planning)
                        );
                    } else {
                        print!("{}", nav_projection_report::render_plan_human(&planning));
                    }
                    EXIT_OK
                }
                Some(digest) => match nav_projection_lifecycle::apply_accept(
                    &root,
                    &catalog.regions,
                    &planning,
                    &digest,
                ) {
                    Ok(applied) => {
                        if json {
                            println!(
                                "{}",
                                nav_projection_report::render_apply_json("aceitar", &applied)
                            );
                        } else {
                            print!("{}", nav_projection_report::render_apply_human(&applied));
                        }
                        EXIT_OK
                    }
                    Err(error) => print_projection_error("aceitar", json, &error),
                },
            }
        }
    }
}

fn run_projection_show(
    root: &pinker_v0::automation::RepoRoot,
    json: bool,
    id: &str,
    observed: bool,
) -> i32 {
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(error) => {
            return print_projection_error("mostrar", json, &ProjectionError::Authority(error))
        }
    };
    if let Some(error) = store.snapshot_error(id) {
        return print_projection_error(
            "mostrar",
            json,
            &ProjectionError::Harness {
                path: Some(error.path.clone()),
                message: error.message.clone(),
            },
        );
    }
    let Some(stored) = store.snapshot(id) else {
        return print_projection_error(
            "mostrar",
            json,
            &ProjectionError::NotFound { id: id.to_string() },
        );
    };
    let verification = if observed {
        let catalog = match load_projection_catalog(root) {
            Ok(catalog) => catalog,
            Err(error) => return print_projection_error("mostrar", json, &error),
        };
        match nav_projection_report::verify_one(&store, id, &catalog.regions) {
            Ok(item) => Some(item.report),
            Err(error) => return print_projection_error("mostrar", json, &error),
        }
    } else {
        None
    };
    if json {
        println!(
            "{}",
            nav_projection_report::render_show_json(stored, verification.as_ref())
        );
    } else {
        print!(
            "{}",
            nav_projection_report::render_show_human(stored, verification.as_ref())
        );
    }
    match verification.as_ref().map(|report| &report.outcome) {
        Some(pinker_v0::nav_projection_snapshot::Outcome::Drift(_)) => EXIT_SOURCE,
        Some(pinker_v0::nav_projection_snapshot::Outcome::HarnessFailure(_)) => EXIT_HARNESS,
        _ => EXIT_OK,
    }
}

fn run_projection_verify(
    root: &pinker_v0::automation::RepoRoot,
    json: bool,
    id: Option<&str>,
) -> i32 {
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(error) => {
            return print_projection_error("verificar", json, &ProjectionError::Authority(error))
        }
    };
    let catalog = match load_projection_catalog(root) {
        Ok(catalog) => catalog,
        Err(error) => return print_projection_error("verificar", json, &error),
    };
    let batch = if let Some(id) = id {
        let item = match nav_projection_report::verify_one(&store, id, &catalog.regions) {
            Ok(item) => item,
            Err(error) => return print_projection_error("verificar", json, &error),
        };
        nav_projection_report::VerificationBatch {
            results: vec![item],
            causes: Vec::new(),
            errors: Vec::new(),
        }
    } else {
        nav_projection_report::verify_all(&store, &catalog.regions)
    };
    if json {
        println!(
            "{}",
            nav_projection_report::render_verification_json(&batch)
        );
    } else {
        print!(
            "{}",
            nav_projection_report::render_verification_human(&batch)
        );
    }
    match batch.outcome() {
        "MATCH" => EXIT_OK,
        "DRIFT" => EXIT_SOURCE,
        _ => EXIT_HARNESS,
    }
}

fn load_projection_catalog(
    root: &pinker_v0::automation::RepoRoot,
) -> Result<nav::CodeCatalog, ProjectionError> {
    nav::CodeCatalog::load(&root.path().join("src/navigation.jsonl")).map_err(|error| {
        ProjectionError::Harness {
            path: Some("src/navigation.jsonl".to_string()),
            message: error.to_string(),
        }
    })
}

fn print_projection_error(command: &str, json: bool, error: &ProjectionError) -> i32 {
    if json {
        println!(
            "{}",
            nav_projection_report::render_error_json(command, error)
        );
    } else {
        eprintln!("{error}");
    }
    projection_error_exit(error)
}

fn projection_error_exit(error: &ProjectionError) -> i32 {
    use pinker_v0::automation::Failure;
    let failure_exit = |failure: &Failure| match failure {
        Failure::HarnessFailure(pinker_v0::automation::HarnessCause::RootNotFound { .. }) => {
            EXIT_CATALOG
        }
        Failure::HarnessFailure(_) => EXIT_HARNESS,
        Failure::PolicyViolation(_) => EXIT_POLICY,
        Failure::StalePlan { .. } => EXIT_STALE,
        Failure::IoFailure { .. } | Failure::VerifyAfterApplyFailure { .. } => EXIT_FAILURE,
    };
    match error {
        ProjectionError::Authority(_) => EXIT_CATALOG,
        ProjectionError::NotFound { .. } => EXIT_NORESULT,
        ProjectionError::Harness { path, .. }
            if path.as_deref() == Some("src/navigation.jsonl") =>
        {
            EXIT_CATALOG
        }
        ProjectionError::Harness { .. } => EXIT_HARNESS,
        ProjectionError::Policy { .. } => EXIT_POLICY,
        ProjectionError::Drift { .. } => EXIT_SOURCE,
        ProjectionError::Automation(failure) => failure_exit(failure),
        ProjectionError::Apply(report) => {
            report.failure.as_ref().map_or(EXIT_FAILURE, failure_exit)
        }
        ProjectionError::VerifyAfterApply { .. } => EXIT_FAILURE,
    }
}
// @pinker-nav:end cli.nav.projecao

// @pinker-nav:start cli.nav.consulta
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:related-symbol pinker_v0::symbol_index::locate
// @pinker-nav:related-symbol pinker_v0::diff_coverage::analyze
// @pinker-nav:summary Consultas nav read-only carregam catálogo e símbolos; cobertura-diff analisa stdin e impacto compõe git diff limitado com as autoridades correntes sem mutar o repositório.
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

fn run_nav_localizar(repo_root: &Path, symbol: &str, json: bool) -> i32 {
    let code = match load_code_catalog(repo_root) {
        Ok(catalog) => catalog,
        Err(code) => return code,
    };
    let doc_config = load_doc_config(repo_root);
    let doc_path = repo_root.join(doc_config.generated.docs_index);
    let docs = match doc_index::DocCatalog::load(&doc_path) {
        Ok(catalog) => Some(catalog),
        Err(doc_index::CatalogError::Missing { .. }) => None,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let report = match symbol_index::locate(&code, docs.as_ref(), symbol) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    if json {
        println!("{}", symbol_index::render_json(&report));
    } else if report.found() {
        print!("{}", symbol_index::render_human(&report));
    } else {
        eprint!("{}", symbol_index::render_human(&report));
    }
    if report.found() {
        EXIT_OK
    } else {
        EXIT_NORESULT
    }
}

fn run_nav_cobertura_diff(repo_root: &Path, json: bool) -> i32 {
    let root = match pinker_v0::automation::RepoRoot::discover(repo_root) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("E-DIFF-ROOT\n{error}");
            return EXIT_HARNESS;
        }
    };
    let mut bytes = Vec::new();
    if let Err(error) = io::stdin()
        .take((diff_coverage::MAX_DIFF_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
    {
        eprintln!("E-DIFF-IO\nFalha ao ler stdin: {error}");
        return EXIT_HARNESS;
    }
    if bytes.len() > diff_coverage::MAX_DIFF_BYTES {
        eprintln!(
            "{}",
            diff_coverage::CoverageError::TooLarge {
                bytes: bytes.len(),
                limit: diff_coverage::MAX_DIFF_BYTES,
            }
        );
        return EXIT_HARNESS;
    }
    let input = match std::str::from_utf8(&bytes) {
        Ok(input) => input,
        Err(_) => {
            eprintln!("{}", diff_coverage::CoverageError::InvalidUtf8);
            return EXIT_HARNESS;
        }
    };
    let config = match doc::DocConfig::load(root.path()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let code_path = root.path().join(&config.generated.code_index);
    let code = match nav::CodeCatalog::load(&code_path) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let docs_path = root.path().join(&config.generated.docs_index);
    let docs = match doc_index::DocCatalog::load(&docs_path) {
        Ok(docs) => Some(docs),
        Err(doc_index::CatalogError::Missing { .. }) => None,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let projection_store = ProjectionStore::load(root.path()).ok();
    let manifests = change::Manifests::load(&root.path().join(".pinker/changes"));
    let report = match diff_coverage::analyze(
        input,
        diff_coverage::CoverageAuthorities {
            code: &code,
            docs: docs.as_ref(),
            projection_store: projection_store.as_ref(),
            doc_config: Some(&config),
            manifests: Some(&manifests),
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_HARNESS;
        }
    };
    if json {
        println!("{}", diff_coverage::render_json(&report));
    } else {
        print!("{}", diff_coverage::render_human(&report));
    }
    EXIT_OK
}

fn run_nav_impacto(repo_root: &Path, diff: &str, json: bool) -> i32 {
    match tooling::collect_impact(repo_root, diff) {
        Ok(report) => {
            if json {
                println!("{}", tooling::render_impact_json(&report));
            } else {
                println!("pink nav impacto");
                println!("  diff: {}", report.diff);
                println!("  changed_files: {}", report.changed_files.len());
                println!(
                    "  changed_regions: {}",
                    report.changed_regions.status.as_str()
                );
                println!(
                    "  projections_affected: {}",
                    report.projections_affected.status.as_str()
                );
                println!("  catalog_status: {}", report.catalog_status);
            }
            EXIT_OK
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_HARNESS
        }
    }
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

fn run_nav_mapa(repo_root: &Path, filtro: Option<&str>, json: bool) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let selected = catalog.map_regions(filtro);
    if selected.is_empty() {
        if json {
            println!(
                "{{\"schema\":1,\"filter\":{},\"files\":[]}}",
                filtro
                    .map(json_escape)
                    .unwrap_or_else(|| "null".to_string())
            );
        } else if let Some(filtro) = filtro {
            eprintln!("Nenhuma região encontrada para o mapa: {filtro}");
        } else {
            eprintln!("Nenhuma região disponível para o mapa.");
        }
        return EXIT_NORESULT;
    }

    let mut files: BTreeMap<&str, Vec<&nav::CodeRegion>> = BTreeMap::new();
    for region in selected {
        files.entry(&region.file).or_default().push(region);
    }
    for sections in files.values_mut() {
        sections.sort_by(|a, b| {
            a.content_start
                .cmp(&b.content_start)
                .then(a.content_end.cmp(&b.content_end))
                .then(a.key.cmp(&b.key))
        });
    }

    if json {
        let rendered_files: Vec<String> = files
            .iter()
            .map(|(path, sections)| {
                let domains: BTreeSet<&str> = sections
                    .iter()
                    .filter_map(|region| region.domain.as_deref())
                    .collect();
                let layers: BTreeSet<&str> = sections
                    .iter()
                    .filter_map(|region| region.layer.as_deref())
                    .collect();
                let start = sections
                    .iter()
                    .map(|region| region.content_start)
                    .min()
                    .unwrap_or(0);
                let end = sections
                    .iter()
                    .map(|region| region.content_end)
                    .max()
                    .unwrap_or(0);
                let rendered_sections: Vec<String> = sections
                    .iter()
                    .map(|region| {
                        format!(
                            "{{\"key\":{},\"summary\":{},\"domain\":{},\"layer\":{},\"range\":{{\"start\":{},\"end\":{}}}}}",
                            json_escape(&region.key),
                            if region.summary.is_empty() {
                                "null".to_string()
                            } else {
                                json_escape(&region.summary)
                            },
                            region
                                .domain
                                .as_deref()
                                .map(json_escape)
                                .unwrap_or_else(|| "null".to_string()),
                            region
                                .layer
                                .as_deref()
                                .map(json_escape)
                                .unwrap_or_else(|| "null".to_string()),
                            region.content_start,
                            region.content_end
                        )
                    })
                    .collect();
                let domain_values: Vec<String> = domains.iter().map(|v| json_escape(v)).collect();
                let layer_values: Vec<String> = layers.iter().map(|v| json_escape(v)).collect();
                format!(
                    "{{\"path\":{},\"region_count\":{},\"domains\":[{}],\"layers\":[{}],\"range\":{{\"start\":{},\"end\":{}}},\"sections\":[{}]}}",
                    json_escape(path),
                    sections.len(),
                    domain_values.join(","),
                    layer_values.join(","),
                    start,
                    end,
                    rendered_sections.join(",")
                )
            })
            .collect();
        println!(
            "{{\"schema\":1,\"filter\":{},\"files\":[{}]}}",
            filtro
                .map(json_escape)
                .unwrap_or_else(|| "null".to_string()),
            rendered_files.join(",")
        );
    } else {
        let absolute_root = repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf());
        for (file_index, (path, sections)) in files.iter().enumerate() {
            if file_index > 0 {
                println!();
            }
            let domains: BTreeSet<&str> = sections
                .iter()
                .filter_map(|region| region.domain.as_deref())
                .collect();
            let layers: BTreeSet<&str> = sections
                .iter()
                .filter_map(|region| region.layer.as_deref())
                .collect();
            let start = sections
                .iter()
                .map(|region| region.content_start)
                .min()
                .unwrap_or(0);
            let end = sections
                .iter()
                .map(|region| region.content_end)
                .max()
                .unwrap_or(0);
            let domain_text = if domains.is_empty() {
                "-".to_string()
            } else {
                domains.into_iter().collect::<Vec<_>>().join(", ")
            };
            let layer_text = if layers.is_empty() {
                "-".to_string()
            } else {
                layers.into_iter().collect::<Vec<_>>().join(", ")
            };
            println!("{path}");
            println!("  absoluto: {}", absolute_root.join(path).display());
            println!("  regiões: {}", sections.len());
            println!("  domínios: {domain_text}");
            println!("  camadas: {layer_text}");
            println!("  intervalo: {start}-{end}");
            for region in sections {
                println!();
                println!("  {}", region.key);
                println!(
                    "    resumo: {}",
                    if region.summary.is_empty() {
                        "-"
                    } else {
                        &region.summary
                    }
                );
                println!("    domínio: {}", region.domain.as_deref().unwrap_or("-"));
                println!("    camada: {}", region.layer.as_deref().unwrap_or("-"));
                println!(
                    "    intervalo: {}-{}",
                    region.content_start, region.content_end
                );
            }
        }
    }
    EXIT_OK
}
// @pinker-nav:end cli.nav.consulta

// @pinker-nav:start cli.nav.sincronizacao-verificacao
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:summary run_nav_sincronizar reescaneia e grava o catálogo somente após validação; run_nav_verificar reutiliza nav::verify_repository e valida em memória os vínculos estruturados do índice de símbolos contra os catálogos de código e documentação, sem escrever e sem duplicar autoridade.
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
    let verification = match nav::verify_repository(repo_root, &doc_config.generated.code_index) {
        Ok(verification) => verification,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_FAILURE;
        }
    };
    if !verification.is_ok() {
        eprintln!(
            "E-NAV-VERIFY: {} divergência(s) encontrada(s):",
            verification.total_errors()
        );
        for error in &verification.source_errors {
            eprintln!("  - {error}");
        }
        if verification.catalog_out_of_date {
            eprintln!(
                "  - {}",
                nav::NavVerifyError::IndexOutOfDate {
                    path: doc_config.generated.code_index.clone()
                }
            );
        }
        return EXIT_SOURCE;
    }

    let code = match nav::CodeCatalog::load(&repo_root.join(&doc_config.generated.code_index)) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let requires_docs = code
        .regions
        .iter()
        .any(|region| !region.symbol_docs.is_empty());
    let docs = if requires_docs {
        match doc_index::DocCatalog::load(&repo_root.join(&doc_config.generated.docs_index)) {
            Ok(catalog) => Some(catalog),
            Err(error) => {
                eprintln!("{error}");
                return EXIT_CATALOG;
            }
        }
    } else {
        None
    };
    if let Err(error) = symbol_index::locate(&code, docs.as_ref(), "") {
        eprintln!("E-NAV-VERIFY: vínculo explícito de símbolo inválido:\n  - {error}");
        return EXIT_SOURCE;
    }

    println!("Marcadores, vínculos e catálogo de código verificados: ok.");
    EXIT_OK
}
// @pinker-nav:end cli.nav.sincronizacao-verificacao

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
// @pinker-nav:summary run_analyze lê o arquivo de entrada, registra-o como unidade-fonte primária no SourceMap para que todo span nasça vinculado, e conduz o pipeline de análise: tokeniza, parseia, compõe os módulos preservando a unidade (carregar_e_projetar, que devolve o programa projetado e o grafo resolvido), roda a verificação semântica ciente da composição (semantic::check_program_composto, que recebe os tratos visíveis por fonte) e, conforme as flags do Config, cada etapa a jusante (IR, CFG IR, seleção de instruções, máquina abstrata, backend `.s` textual, execução via interpretador, backend pseudo-asm) só é computada se alguma flag de saída a exigir (`needs_ir`/`needs_cfg`/`needs_selected`/`needs_machine`); a falha ao ler o arquivo é tratada diretamente com `eprintln!` e `process::exit(1)`, enquanto erros Pinker das etapas de tokenização, parsing, importação, semântica e lowerings são tratados por `try_or_exit!`; esta função não monta nem linka um binário — a emissão `--asm-s` é apenas texto impresso, e `--run` executa via interpreter::run_program_with_args, não via processo nativo.
fn run_analyze(config: Config) {
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
fn run_build(config: BuildConfig) {
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
