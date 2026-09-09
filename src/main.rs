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
// Decomposição física #640, unidade MAIN-1: os comandos `nav` — consulta,
// sincronização/verificação e o adaptador de projeções — moram em
// `src/pink_cli/nav_cli.rs`. O `mod` fica aqui, com os irmãos da #605, porque
// a unidade não usa `try_or_exit!`: a #601 mediu os 29 usos da macro todos
// dentro da MAIN-4, e nenhum dentro desta.
#[path = "pink_cli/nav_cli.rs"]
mod nav_cli;

use cli_parsing::parse_args;
use doc_cli::{load_doc_config, run_doc, write_atomic};
use modules::{base_dir_de, carregar_e_projetar, contexto_de_import};
use nav_cli::{
    run_nav_buscar, run_nav_cobertura_diff, run_nav_impacto, run_nav_listar, run_nav_localizar,
    run_nav_mapa, run_nav_mostrar, run_nav_projecao, run_nav_sincronizar, run_nav_verificar,
};

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

// Decomposição física #638, unidade MAIN-4: a análise e o build nativo moram
// em `src/pink_cli/analysis_build.rs`. A declaração vem aqui embaixo, e não
// junto dos outros irmãos, porque escopo de `macro_rules!` é textual e não
// de item: um `mod` acima da definição de `try_or_exit!` não enxergaria a
// macro, e os 29 usos que a #601 mediu vivem todos dentro desta unidade.
#[path = "pink_cli/analysis_build.rs"]
mod analysis_build;

use analysis_build::{run_analyze, run_build};
