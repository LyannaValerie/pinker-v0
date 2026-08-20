use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::agent;
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
use pinker_v0::nav;
use pinker_v0::nav_projection_lifecycle::{self, ProjectionError};
use pinker_v0::nav_projection_report;
use pinker_v0::nav_projection_store::ProjectionStore;
use pinker_v0::parser::Parser;
use pinker_v0::printer;
use pinker_v0::project_state;
use pinker_v0::project_state_report;
use pinker_v0::projection;
use pinker_v0::repl;
use pinker_v0::semantic;
use pinker_v0::symbol_index;
use pinker_v0::token::Span;
use pinker_v0::tooling;
use pinker_v0::{ast, error::PinkerError};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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

enum AgentSub {
    Iniciar,
    Executar,
    Verificar,
    Sensibilidade,
    Publicar,
    Retomar,
    Status { json: bool },
    Relatorio,
}

struct AgentConfigCli {
    spec: PathBuf,
    sub: AgentSub,
}

struct StateConfigCli {
    repo: String,
    json: bool,
    agent_spec: Option<PathBuf>,
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
    Agent(AgentConfigCli),
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
          agente      runner local auditável para tarefas operacionais\n\
           estado      estado consolidado somente leitura do projeto\n\
           doctor      identidade e compatibilidade operacional do pink\n\
           verificar   preflight estruturado antes da suíte completa\n"
    )
}

fn state_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} estado [--repo DIRETÓRIO] [--agente-spec ARQUIVO] [--json]\n\
         \n\
         Comando:\n\
           estado      consolida autoridades locais sem escrever nem usar rede\n\
         \n\
         Opções:\n\
           --repo DIRETÓRIO       ponto de partida para descobrir o repositório\n\
           --agente-spec ARQUIVO  spec explícita do pink agente (opcional)\n\
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

fn agent_usage(binary: &str) -> String {
    format!(
        "Uso: {binary} agente iniciar <spec>\n\
         Uso: {binary} agente executar <spec>\n\
         Uso: {binary} agente verificar <spec>\n\
         Uso: {binary} agente sensibilidade <spec>\n\
         Uso: {binary} agente publicar <spec>\n\
         Uso: {binary} agente retomar <spec>\n\
         Uso: {binary} agente status <spec> [--json]\n\
         Uso: {binary} agente relatorio <spec>\n"
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
        "agente" => Some(agent_usage(program)),
        "estado" => Some(state_usage(program)),
        "doctor" => Some(doctor_usage(program)),
        "verificar" => Some(verify_usage(program)),
        _ => None,
    }
}
// @pinker-nav:end cli.ajuda.usage

// @pinker-nav:start cli.parsing.subcomandos
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary Parsers estritos dos subcomandos, incluindo estado, doctor e verificar: validam flags, posicionais, duplicatas e requisitos cruzados antes de produzir modelos tipados.
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
            _ if arg.starts_with('-') => {
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
            _ if arg.starts_with('-') => {
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
        _ if arg.starts_with('-') => Err(format!(
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
    let mut freeze = false;
    let mut artifact: Option<String> = None;
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
            "--freeze" => freeze = true,
            "--artifact" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--artifact' requer um caminho de arquivo.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                artifact = Some(args[i].clone());
            }
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
            _ if arg.starts_with('-') => {
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
            if freeze && check {
                return Err(format!(
                    "Use --freeze ou --check, não ambos.\n\n{}",
                    doc_usage(binary)
                ));
            }
            if freeze && (corpo.is_none() || artifact.is_none()) {
                return Err(format!(
                    "--freeze exige --corpo e --artifact.\n\n{}",
                    doc_usage(binary)
                ));
            }
            if !freeze && artifact.is_some() {
                return Err(format!(
                    "--artifact exige --freeze.\n\n{}",
                    doc_usage(binary)
                ));
            }
            DocSub::ImportarPr {
                pr,
                corpo,
                check,
                freeze,
                artifact,
            }
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
    let mut observado = false;
    let mut justificativa: Option<String> = None;
    let mut predecessor: Option<String> = None;
    let mut autorizar: Option<String> = None;
    let mut diff: Option<String> = None;
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
            "--diff" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--diff' requer uma referência Git.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                if diff.is_some() {
                    return Err(format!(
                        "A opção '--diff' não pode ser repetida.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                diff = Some(args[i].clone());
            }
            "--observado" => observado = true,
            "--justificativa" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--justificativa' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                justificativa = Some(args[i].clone());
            }
            "--predecessor" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--predecessor' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                predecessor = Some(args[i].clone());
            }
            "--autorizar" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--autorizar' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                autorizar = Some(args[i].clone());
            }
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
            _ if arg.starts_with('-') => {
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

    let has_projection_options =
        observado || justificativa.is_some() || predecessor.is_some() || autorizar.is_some();
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
        "localizar" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav localizar.\n\n{}",
                    nav_usage(binary)
                ));
            }
            NavSub::Localizar {
                symbol: require_one("localizar")?,
            }
        }
        "cobertura-diff" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav cobertura-diff.\n\n{}",
                    nav_usage(binary)
                ));
            }
            require_none("cobertura-diff")?;
            NavSub::CoberturaDiff
        }
        "impacto" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav impacto.\n\n{}",
                    nav_usage(binary)
                ));
            }
            require_none("impacto")?;
            let diff = diff
                .clone()
                .ok_or_else(|| format!("nav impacto exige --diff REF.\n\n{}", nav_usage(binary)))?;
            NavSub::Impacto { diff }
        }
        "mapa" => NavSub::Mapa {
            filtro: if positionals.is_empty() {
                None
            } else {
                Some(positionals.join(" "))
            },
        },
        "sincronizar" => {
            require_none("sincronizar")?;
            NavSub::Sincronizar
        }
        "verificar" => {
            require_none("verificar")?;
            NavSub::Verificar
        }
        "projecao" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav projecao.\n\n{}",
                    projection_usage(binary)
                ));
            }
            let Some(command) = positionals.first() else {
                return Err(projection_usage(binary));
            };
            let arguments = &positionals[1..];
            let require_projection_id = || -> Result<String, String> {
                if arguments.len() != 1 {
                    return Err(format!(
                        "O subcomando '{}' requer exatamente um ID.\n\n{}",
                        command,
                        projection_subcommand_usage(binary, command)
                    ));
                }
                Ok(arguments[0].clone())
            };
            let projection = match command.as_str() {
                "listar" => {
                    if !arguments.is_empty() || has_projection_options {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Listar
                }
                "mostrar" => {
                    if justificativa.is_some() || predecessor.is_some() || autorizar.is_some() {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Mostrar {
                        id: require_projection_id()?,
                        observado,
                    }
                }
                "verificar" => {
                    if has_projection_options {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    if arguments.len() > 1 {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Verificar {
                        id: arguments.first().cloned(),
                    }
                }
                "preparar" => {
                    if observado {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Preparar {
                        id: require_projection_id()?,
                        justificativa,
                        predecessor,
                        autorizar,
                    }
                }
                "aceitar" => {
                    if observado || justificativa.is_some() || predecessor.is_some() {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Aceitar {
                        id: require_projection_id()?,
                        autorizar,
                    }
                }
                _ => {
                    return Err(format!(
                        "Subcomando nav projecao desconhecido: '{}'.\n\n{}",
                        command,
                        projection_usage(binary)
                    ))
                }
            };
            NavSub::Projecao(projection)
        }
        other => {
            return Err(format!(
                "Subcomando nav desconhecido: '{}'\n\n{}",
                other,
                nav_usage(binary)
            ));
        }
    };

    if !matches!(sub, NavSub::Impacto { .. }) && diff.is_some() {
        return Err(format!(
            "A opção '--diff' pertence somente a nav impacto.\n\n{}",
            nav_usage(binary)
        ));
    }

    if !matches!(sub, NavSub::Projecao(_)) && has_projection_options {
        return Err(format!(
            "Opção exclusiva de nav projecao usada em '{}'.\n\n{}",
            subcommand,
            nav_usage(binary)
        ));
    }

    Ok(NavConfigCli {
        repo,
        json,
        limite,
        sub,
    })
}

fn parse_agent_args(binary: &str, args: &[String]) -> Result<AgentConfigCli, String> {
    let Some(subcommand) = args.first() else {
        return Err(agent_usage(binary));
    };
    if matches!(subcommand.as_str(), "--help" | "-h") {
        return Err(agent_usage(binary));
    }
    let json = args.iter().skip(1).any(|arg| arg == "--json");
    let positional: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|arg| arg.as_str() != "--json")
        .collect();
    if positional.len() != 1 || (json && subcommand != "status") {
        return Err(agent_usage(binary));
    }
    let sub = match subcommand.as_str() {
        "iniciar" => AgentSub::Iniciar,
        "executar" => AgentSub::Executar,
        "verificar" => AgentSub::Verificar,
        "sensibilidade" => AgentSub::Sensibilidade,
        "publicar" => AgentSub::Publicar,
        "retomar" => AgentSub::Retomar,
        "status" => AgentSub::Status { json },
        "relatorio" => AgentSub::Relatorio,
        _ => {
            return Err(format!(
                "Subcomando agente desconhecido: '{subcommand}'\n\n{}",
                agent_usage(binary)
            ))
        }
    };
    Ok(AgentConfigCli {
        spec: PathBuf::from(positional[0]),
        sub,
    })
}

fn parse_state_args(binary: &str, args: &[String]) -> Result<StateConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut agent_spec: Option<PathBuf> = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(state_usage(binary)),
            "--repo" => {
                if repo.is_some() {
                    return Err(format!(
                        "A opção '--repo' não pode ser repetida.\n\n{}",
                        state_usage(binary)
                    ));
                }
                i += 1;
                if i >= args.len() || args[i].starts_with('-') {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        state_usage(binary)
                    ));
                }
                repo = Some(args[i].clone());
            }
            "--agente-spec" => {
                if agent_spec.is_some() {
                    return Err(format!(
                        "A opção '--agente-spec' não pode ser repetida.\n\n{}",
                        state_usage(binary)
                    ));
                }
                i += 1;
                if i >= args.len() || args[i].starts_with('-') {
                    return Err(format!(
                        "Flag '--agente-spec' requer um valor.\n\n{}",
                        state_usage(binary)
                    ));
                }
                agent_spec = Some(PathBuf::from(&args[i]));
            }
            "--json" => {
                if json {
                    return Err(format!(
                        "A opção '--json' não pode ser repetida.\n\n{}",
                        state_usage(binary)
                    ));
                }
                json = true;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando estado: '{}'.\n\n{}",
                    value,
                    state_usage(binary)
                ));
            }
            value => {
                return Err(format!(
                    "O comando estado não aceita argumento posicional: '{}'.\n\n{}",
                    value,
                    state_usage(binary)
                ));
            }
        }
        i += 1;
    }
    Ok(StateConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        json,
        agent_spec,
    })
}

fn parse_doctor_args(binary: &str, args: &[String]) -> Result<DoctorConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(doctor_usage(binary)),
            "--repo" => {
                if repo.is_some() {
                    return Err(format!(
                        "A opção '--repo' não pode ser repetida.\n\n{}",
                        doctor_usage(binary)
                    ));
                }
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        doctor_usage(binary)
                    ));
                }
                repo = Some(args[i].clone());
            }
            "--json" if !json => json = true,
            "--json" => {
                return Err(format!(
                    "A opção '--json' não pode ser repetida.\n\n{}",
                    doctor_usage(binary)
                ))
            }
            value => {
                return Err(format!(
                    "Argumento desconhecido em doctor: '{}'.\n\n{}",
                    value,
                    doctor_usage(binary)
                ))
            }
        }
        i += 1;
    }
    Ok(DoctorConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        json,
    })
}

fn parse_verify_args(binary: &str, args: &[String]) -> Result<VerifyConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut diff: Option<String> = None;
    let mut corpo: Option<PathBuf> = None;
    let mut documentation_frozen = false;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(verify_usage(binary)),
            "--repo" | "--diff" | "--corpo" => {
                let flag = args[i].clone();
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '{}' requer um valor.\n\n{}",
                        flag,
                        verify_usage(binary)
                    ));
                }
                match flag.as_str() {
                    "--repo" if repo.is_none() => repo = Some(args[i].clone()),
                    "--diff" if diff.is_none() => diff = Some(args[i].clone()),
                    "--corpo" if corpo.is_none() => corpo = Some(PathBuf::from(&args[i])),
                    _ => {
                        return Err(format!(
                            "A opção '{}' não pode ser repetida.\n\n{}",
                            flag,
                            verify_usage(binary)
                        ))
                    }
                }
            }
            "--documentation-frozen" if !documentation_frozen => documentation_frozen = true,
            "--json" if !json => json = true,
            "--documentation-frozen" | "--json" => {
                return Err(format!(
                    "A opção '{}' não pode ser repetida.\n\n{}",
                    args[i],
                    verify_usage(binary)
                ))
            }
            value => {
                return Err(format!(
                    "Argumento desconhecido em verificar: '{}'.\n\n{}",
                    value,
                    verify_usage(binary)
                ))
            }
        }
        i += 1;
    }
    let diff = diff.ok_or_else(|| {
        format!(
            "O comando verificar exige --diff REF.\n\n{}",
            verify_usage(binary)
        )
    })?;
    Ok(VerifyConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        diff,
        documentation_frozen,
        corpo,
        json,
    })
}
// @pinker-nav:end cli.parsing.subcomandos

// @pinker-nav:start cli.parsing.roteamento
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary parse_args resolve ajuda e versão, separa runtime tail e despacha os nove comandos — incluindo doctor e verificar — ou análise, com erros uniformes de uso.
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
    let program = program_name(raw_args.first());
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

    if matches!(flag_args.first().map(String::as_str), Some("--help" | "-h")) {
        return Ok(CliCommand::Help(usage(&program)));
    }
    if matches!(
        flag_args.first().map(String::as_str),
        Some("--version" | "-V")
    ) {
        if flag_args.len() == 1 && runtime_tail.is_empty() {
            return Ok(CliCommand::Version);
        }
        return Err(format!(
            "A opção de versão não aceita argumentos.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("--version-json") {
        if flag_args.len() == 1 && runtime_tail.is_empty() {
            return Ok(CliCommand::VersionJson);
        }
        return Err(format!(
            "A opção de identidade não aceita argumentos.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("version") {
        return Err(format!(
            "Comando 'version' desconhecido. Use '--version' ou '-V'.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("help") {
        return match &flag_args[1..] {
            [] if runtime_tail.is_empty() => Ok(CliCommand::Help(usage(&program))),
            [command] if runtime_tail.is_empty() => help_for_command(&program, command)
                .map(CliCommand::Help)
                .ok_or_else(|| {
                    format!(
                        "Comando desconhecido para ajuda: '{}'.\n\n{}",
                        command,
                        usage(&program)
                    )
                }),
            _ => Err(format!(
                "O comando 'help' aceita no máximo um COMANDO.\n\n{}",
                usage(&program)
            )),
        };
    }

    if let Some(cmd) = flag_args.first() {
        if cmd == "nav"
            && flag_args.get(1).map(String::as_str) == Some("projecao")
            && flag_args[2..]
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            let help = flag_args
                .get(2)
                .filter(|value| !value.starts_with('-'))
                .map_or_else(
                    || projection_usage(&program),
                    |command| projection_subcommand_usage(&program, command),
                );
            return Ok(CliCommand::Help(help));
        }
        if let Some(help) = help_for_command(&program, cmd) {
            if flag_args[1..]
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
            {
                return Ok(CliCommand::Help(help));
            }
        }
        if cmd == "build" {
            return parse_build_args(&program, &flag_args[1..]).map(CliCommand::Build);
        }
        if cmd == "editor" {
            return parse_editor_args(&program, &flag_args[1..]).map(CliCommand::Editor);
        }
        if cmd == "repl" {
            return parse_repl_args(&program, &flag_args[1..]).map(CliCommand::Repl);
        }
        if cmd == "doc" {
            return parse_doc_args(&program, &flag_args[1..]).map(CliCommand::Doc);
        }
        if cmd == "nav" {
            return parse_nav_args(&program, &flag_args[1..]).map(CliCommand::Nav);
        }
        if cmd == "agente" {
            return parse_agent_args(&program, &flag_args[1..]).map(CliCommand::Agent);
        }
        if cmd == "estado" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando estado não aceita argumentos após '--'.\n\n{}",
                    state_usage(&program)
                ));
            }
            return parse_state_args(&program, &flag_args[1..]).map(CliCommand::State);
        }
        if cmd == "doctor" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando doctor não aceita argumentos após '--'.\n\n{}",
                    doctor_usage(&program)
                ));
            }
            return parse_doctor_args(&program, &flag_args[1..]).map(CliCommand::Doctor);
        }
        if cmd == "verificar" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando verificar não aceita argumentos após '--'.\n\n{}",
                    verify_usage(&program)
                ));
            }
            return parse_verify_args(&program, &flag_args[1..]).map(CliCommand::Verify);
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
            "--help" | "-h" => return Ok(CliCommand::Help(usage(&program))),
            "--version" | "-V" => {
                return Err(format!(
                    "A opção de versão deve ser usada sem ARQUIVO.\n\n{}",
                    usage(&program)
                ));
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida: '{}'\n\n{}",
                    arg,
                    usage(&program)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado.\n\n{}",
                        usage(&program)
                    ));
                }
                input = Some(arg.clone());
            }
        }
    }

    let Some(input) = input else {
        return Err(format!(
            "Uso inválido: nenhum argumento informado.\n\n{}",
            usage(&program)
        ));
    };
    if !run_program && !runtime_tail.is_empty() {
        return Err(format!(
            "Argumentos após '--' exigem '--run'.\n\n{}",
            usage(&program)
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
// @pinker-nav:summary main preserva exits de domínio ao despachar análise e os nove comandos, incluindo adaptadores estruturados read-only para doctor, nav impacto e verificar.
/// Macro para encurtar o padrão "try or exit(1)" repetido no pipeline.
macro_rules! try_or_exit {
    ($result:expr, $source:expr) => {
        match $result {
            Ok(val) => val,
            Err(err) => {
                eprintln!("{}", err.render_for_cli_with_source($source));
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
        CliCommand::Agent(config) => {
            let result = match config.sub {
                AgentSub::Iniciar => agent::iniciar(&config.spec),
                AgentSub::Executar => agent::executar(&config.spec),
                AgentSub::Verificar => agent::verificar(&config.spec),
                AgentSub::Sensibilidade => agent::sensibilidade(&config.spec),
                AgentSub::Publicar => agent::publicar(&config.spec),
                AgentSub::Retomar => agent::retomar(&config.spec),
                AgentSub::Status { json } => agent::status(&config.spec, json),
                AgentSub::Relatorio => agent::relatorio(&config.spec),
            };
            match result {
                Ok(code) => std::process::exit(code),
                Err(err) => {
                    eprintln!("E-AGENT: {err}");
                    std::process::exit(agent::EXIT_BLOCKED);
                }
            }
        }
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
    match project_state::collect(Path::new(&config.repo), config.agent_spec.as_deref()) {
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
        DocSub::ImportarPr {
            pr,
            corpo,
            check,
            freeze,
            artifact,
        } => {
            if let Err(rejection) = doc_config.baseline_gate(pr) {
                eprintln!("{rejection}");
                return EXIT_SOURCE;
            }
            if freeze {
                let body = corpo.expect("parser garante --corpo com --freeze");
                let artifact = artifact.expect("parser garante --artifact com --freeze");
                let report = tooling::freeze_import(
                    repo_root,
                    &doc_config,
                    pr,
                    Path::new(&body),
                    Path::new(&artifact),
                );
                if config.json {
                    println!("{}", tooling::render_freeze_import_json(&report));
                } else {
                    println!("{}: {}", report.classification.as_str(), report.detail);
                    if let Some(path) = &report.artifact {
                        println!("artifact: {path}");
                    }
                }
                return if report.classification
                    == tooling::FreezeImportClassification::ValidatedDeferredByFreeze
                {
                    EXIT_OK
                } else {
                    EXIT_SOURCE
                };
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
        doc::CHANGE_LEDGER_RELATIVE_PATH,
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
// @pinker-nav:summary CHANGE_LEDGER_RELATIVE_PATH é o caminho canônico do histórico mecânico; write_ledger renderiza os manifestos e grava via write_atomic, ou remove o arquivo quando não há manifestos; run_doc_importar lê, valida e serializa canonicamente um bloco novo e preserva manifestos existentes byte a byte.
fn write_ledger(repo_root: &Path, manifests: &change::Manifests) -> Result<(), i32> {
    let rendered = manifests.render_ledger();
    let path = repo_root.join(doc::CHANGE_LEDGER_RELATIVE_PATH);
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

    // Contrato de imutabilidade (§10): os bytes existentes são preservados. Uma
    // representação diferente só é idempotente quando o modelo integral coincide.
    if manifest_path.exists() {
        let existing = match fs::read_to_string(&manifest_path) {
            Ok(existing) => existing,
            Err(_) => {
                eprintln!("{}", change::immutable_error(pr));
                return EXIT_SOURCE;
            }
        };
        if existing == rendered {
            if check {
                println!("Manifesto pr-{pr}.yaml já sincronizado (idempotente).");
            } else {
                println!("Manifesto pr-{pr}.yaml inalterado (idempotente).");
            }
            return EXIT_OK;
        }
        let existing_manifest = match change::Change::parse_manifest(&existing) {
            Ok(existing_manifest) => existing_manifest,
            Err(_) => {
                eprintln!("{}", change::immutable_error(pr));
                return EXIT_SOURCE;
            }
        };
        let existing_valid = existing_manifest.validate().is_ok()
            && existing_manifest
                .source
                .as_ref()
                .is_some_and(|source| source.number == pr);
        if existing_valid && existing_manifest.semantically_equal(&manifest) {
            if check {
                println!("Manifesto pr-{pr}.yaml semanticamente sincronizado (bytes preservados).");
            } else {
                println!("Manifesto pr-{pr}.yaml semanticamente inalterado (bytes preservados).");
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
// @pinker-nav:summary run_doc_verificar renderiza o modelo somente leitura de doc::verify_repository, preservando diagnósticos estruturais, drift de catálogo, ledger e projeções e os mesmos códigos da CLI sem duplicar a autoridade observacional.
fn run_doc_verificar(repo_root: &Path, config: &doc::DocConfig) -> i32 {
    let verification = match doc::verify_repository(repo_root, config) {
        Ok(verification) => verification,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_FAILURE;
        }
    };
    if verification.is_ok() {
        println!("Documentação, catálogo, manifestos e projeções verificados: ok.");
        return EXIT_OK;
    }
    eprintln!(
        "E-DOC-VERIFY: {} divergência(s) encontrada(s):",
        verification.total_errors()
    );
    for error in &verification.source_errors {
        eprintln!("  - {error}");
    }
    if verification.catalog_out_of_date {
        eprintln!(
            "  - {}",
            doc_index::DocVerifyError::CatalogOutOfDate {
                path: config.generated.docs_index.clone()
            }
        );
    }
    for error in &verification.manifest_errors {
        eprintln!("  - {error}");
    }
    if verification.ledger_out_of_date {
        eprintln!(
            "  - histórico mecânico '{}' dessincronizado; rode `pink doc sincronizar`",
            doc::CHANGE_LEDGER_RELATIVE_PATH
        );
    }
    for drift in &verification.projection_drifts {
        eprintln!("  - {drift}");
    }
    if let Some(error) = &verification.projection_error {
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
// @pinker-nav:summary run_analyze lê o arquivo de entrada e conduz o pipeline de análise: tokeniza, parseia, resolve imports (load_program_with_imports), roda a verificação semântica (semantic::check_program) e, conforme as flags do Config, cada etapa a jusante (IR, CFG IR, seleção de instruções, máquina abstrata, backend `.s` textual, execução via interpretador, backend pseudo-asm) só é computada se alguma flag de saída a exigir (`needs_ir`/`needs_cfg`/`needs_selected`/`needs_machine`); a falha ao ler o arquivo é tratada diretamente com `eprintln!` e `process::exit(1)`, enquanto erros Pinker das etapas de tokenização, parsing, importação, semântica e lowerings são tratados por `try_or_exit!`; esta função não monta nem linka um binário — a emissão `--asm-s` é apenas texto impresso, e `--run` executa via interpreter::run_program_with_args, não via processo nativo.
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
// @pinker-nav:summary run_build repete o front-end (lex/parse/imports/semântica/IR/CFG/seleção) e grava o `.s` resultante em <out_dir>/<stem>.s via fs::write; com --nativo, emite via emit_external_toolchain_subset_nativo e, após gravar, chama link_nativo. locate_pinker_rt_lib localiza (não constrói) a staticlib libpinker_rt.a pré-buildada: usa a env PINKER_RT_LIB se apontar para um arquivo existente, senão procura ao lado do executável atual via std::env::current_exe; retorna Err com uma mensagem sugerindo `cargo build` se não encontrar. detect_cc_driver detecta um driver C disponível testando `cc --version`/`gcc --version`/`clang --version` via std::process::Command e retorna o primeiro que responder com status de sucesso. link_nativo invoca esse driver externo passando o `.s`, a staticlib localizada e -lpthread/-ldl/-lm para produzir o binário via -o; a montagem e a linkedição são feitas pelo driver externo, não por este arquivo. Antes de linkar, link_nativo chama verificar_artefato_sussurro, que relê o `.s` gravado e delega a inline_asm::verify_native_artifact — o invariante de artefato roda no caminho produtivo, não só em fixture de teste: monta o assembly emitido e a baseline derivada sem os envelopes num diretório intermediário `.pinker-sussurro-verificacao` sob o out_dir, compara as superfícies dos dois objetos e aborta o build com E-BACKEND-ASM-ARTIFACT diante de qualquer delta de seção ou de símbolo definido; o diretório intermediário é removido em qualquer desfecho e a verificação só imprime linha de confirmação quando existe ao menos um envelope.
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
    let workdir = out_dir.join(".pinker-sussurro-verificacao");
    let resultado = inline_asm::verify_native_artifact(&asm, driver, &workdir);
    // O diretório de verificação é intermediário: não sobrevive ao build, nem
    // quando a verificação recusa.
    let _ = fs::remove_dir_all(&workdir);
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
// @pinker-nav:summary parse_program_from_source tokeniza e parseia uma string de fonte (sem resolver imports). importable_item_name e importable_item_clone reconhecem e clonam os itens importáveis Function, Const, Struct, TypeAlias, Enum e Trait; qualified_type_item_clone requalifica com o prefixo `<módulo>.` somente Struct e TypeAlias, não Function, Const, Enum ou Trait. load_module_program lê o arquivo `<módulo>.pink` a partir de `base_dir`, detecta ciclo de módulos comparando com a pilha `loading` e recursa nos imports do módulo carregado antes de inserir o programa em `loaded`. load_program_with_imports é o ponto de entrada: para cada import do programa raiz, pula famílias built-in importáveis, detecta import duplicado pela chave `módulo::símbolo`, carrega o módulo via load_module_program e insere os itens importados (todo o módulo ou um símbolo específico) em `root_program.items`, reportando colisão de nome com itens locais ou com outro import. Import de família built-in não vira item: `modulo_real_existe` decide a precedência `REAL_MODULE_X > BUILTIN_FAMILY_X` — a forma seletiva cede a vez a um `<família>.pink` que exista de fato, e só na ausência dele o import sobrevive em `root_program.imports` para a autoridade semântica validá-lo.
fn parse_program_from_source(
    source: &str,
    generic_origin: GenericOrigin,
) -> Result<ast::Program, PinkerError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::with_generic_origin(tokens, generic_origin);
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
    let program = parse_program_from_source(&source, GenericOrigin::module(module)).map_err(
        |err| match err {
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
        },
    )?;

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

/// Parte G: existe um módulo `.pink` real com este nome ao lado da fonte?
///
/// Só a forma seletiva pergunta. A resposta decide precedência de import, e a
/// pergunta é a mesma que `load_module_program` faria em seguida — não é uma
/// busca nova, é a busca histórica antecipada para poder ceder a vez a ela.
fn modulo_real_existe(base_dir: &Path, module: &str) -> bool {
    base_dir.join(format!("{}.pink", module)).is_file()
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

    let mut family_imports = Vec::new();
    for import in &root_program.imports {
        // Fases 186–188 — famílias built-in importáveis não correspondem a
        // arquivo .pink. As intrínsecas já estão disponíveis globalmente; basta
        // pular a carga de módulo.
        //
        // Parte G — `REAL_MODULE_X > BUILTIN_FAMILY_X`.
        //
        // A família built-in não corresponde a arquivo `.pink`, mas o nome dela
        // não é reservado: um módulo real chamado `texto.pink` existia antes
        // desta Parte e continua vencendo. A precedência NÃO pode ser decidida
        // perguntando "a família exporta este membro?" — isso arrancaria de um
        // módulo histórico qualquer export cujo nome coincidisse com o de um
        // membro aprovado. Pergunta-se primeiro se o módulo resolve.
        //
        // `trazer X;` (família inteira) nunca carregou módulo, nem antes desta
        // Parte; só a forma seletiva tinha semântica de módulo, e é só ela que
        // precisa consultar o disco. Isso mantém intacto o invariante de que a
        // superfície aprovada não procura `<familia>.pink`.
        if pinker_v0::familia_superficie::familia_conhecida(import.module.as_str())
            && !(import.symbol.is_some() && modulo_real_existe(&base_dir, &import.module))
        {
            if let Some(symbol) = &import.symbol {
                // Colisão com item de topo é decidida pela autoridade semântica
                // — a mesma que o caminho de biblioteca atravessa. Repetir a
                // regra aqui daria duas políticas para uma pergunta só, que é
                // exatamente o defeito que a Parte G acabou de fechar.
                pinker_v0::semantic::validate_family_import_collision(import, &root_program.items)?;
                if let Some(previous_span) = imported_names.get(symbol) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' trazido por múltiplos módulos",
                            symbol
                        ),
                        span: previous_span.merge(import.span),
                    });
                }
                imported_names.insert(symbol.clone(), import.span);
            }
            family_imports.push(import.clone());
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
    // Imports de módulo já foram materializados como itens de topo e somem.
    // Imports de família sobrevivem porque quem os valida é a autoridade
    // semântica, não o carregador de arquivos.
    root_program.imports = family_imports;
    Ok(root_program)
}
// @pinker-nav:end cli.modulos.importacao
