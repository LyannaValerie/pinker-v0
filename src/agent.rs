//! Runner local, estrito e sem dependências para tarefas operacionais Pinker.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const EXIT_ACCEPTED: i32 = 0;
pub const EXIT_BLOCKED: i32 = 1;
pub const EXIT_NEEDS_HUMAN: i32 = 2;

// @pinker-nav:start development.agent.spec
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Modelo e parser determinístico do formato line-oriented agent-spec-v1: campos escalares únicos, listas repetíveis explícitas e comandos ordenados; rejeita schema, campo e comando duplicado, shell implícito e valores malformados sem depender de crates externas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Program,
    Pinker,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: String,
    pub kind: CommandKind,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub expected_exit: i32,
    pub shell: bool,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub schema: u32,
    pub task_id: String,
    pub repo_root: PathBuf,
    pub worktree: PathBuf,
    pub delegated_root: PathBuf,
    pub expected_base: String,
    pub allowed_writes: Vec<String>,
    pub allowed_changes: Vec<String>,
    pub commands: Vec<CommandSpec>,
    pub accepted_verdict: String,
    pub blocked_verdict: String,
    pub human_verdict: String,
}

#[derive(Default)]
struct CommandBuilder {
    kind: Option<CommandKind>,
    program: Option<String>,
    argv: Vec<String>,
    cwd: Option<String>,
    expected_exit: Option<i32>,
    shell: Option<bool>,
    env: BTreeMap<String, String>,
}

fn parse_bool(value: &str, line: usize) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("linha {line}: booleano inválido: {value}")),
    }
}

fn assign_once<T>(slot: &mut Option<T>, value: T, name: &str, line: usize) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("linha {line}: campo duplicado: {name}"));
    }
    *slot = Some(value);
    Ok(())
}

pub fn parse_spec_text(text: &str) -> Result<Spec, String> {
    let mut schema = None;
    let mut task_id = None;
    let mut repo_root = None;
    let mut worktree = None;
    let mut delegated_root = None;
    let mut expected_base = None;
    let mut accepted_verdict = None;
    let mut blocked_verdict = None;
    let mut human_verdict = None;
    let mut allowed_writes = Vec::new();
    let mut allowed_changes = Vec::new();
    let mut command_order = Vec::new();
    let mut commands: BTreeMap<String, CommandBuilder> = BTreeMap::new();

    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(format!("linha {line_no}: esperado chave = valor"));
        };
        let key = raw_key.trim();
        let value = raw_value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(format!("linha {line_no}: chave ou valor vazio"));
        }
        match key {
            "schema" => assign_once(
                &mut schema,
                value
                    .parse::<u32>()
                    .map_err(|_| format!("linha {line_no}: schema inválido"))?,
                key,
                line_no,
            )?,
            "task_id" => assign_once(&mut task_id, value.to_string(), key, line_no)?,
            "repo_root" => assign_once(&mut repo_root, PathBuf::from(value), key, line_no)?,
            "worktree" => assign_once(&mut worktree, PathBuf::from(value), key, line_no)?,
            "delegated_root" => {
                assign_once(&mut delegated_root, PathBuf::from(value), key, line_no)?
            }
            "expected_base" => assign_once(&mut expected_base, value.to_string(), key, line_no)?,
            "allowed_write" => allowed_writes.push(value.to_string()),
            "allowed_change" => allowed_changes.push(value.to_string()),
            "verdict.accepted" => {
                assign_once(&mut accepted_verdict, value.to_string(), key, line_no)?
            }
            "verdict.blocked" => {
                assign_once(&mut blocked_verdict, value.to_string(), key, line_no)?
            }
            "verdict.human" => assign_once(&mut human_verdict, value.to_string(), key, line_no)?,
            _ if key.starts_with("command.") => {
                let rest = &key[8..];
                let Some((id, field)) = rest.split_once('.') else {
                    return Err(format!("linha {line_no}: campo de comando inválido: {key}"));
                };
                if id.is_empty()
                    || !id
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                {
                    return Err(format!("linha {line_no}: id de comando inválido: {id}"));
                }
                if !commands.contains_key(id) {
                    command_order.push(id.to_string());
                    commands.insert(id.to_string(), CommandBuilder::default());
                }
                let command = commands.get_mut(id).expect("comando inserido");
                match field {
                    "kind" => {
                        let kind = match value {
                            "program" => CommandKind::Program,
                            "pinker" => CommandKind::Pinker,
                            _ => return Err(format!("linha {line_no}: kind inválido: {value}")),
                        };
                        assign_once(&mut command.kind, kind, key, line_no)?;
                    }
                    "program" => {
                        assign_once(&mut command.program, value.to_string(), key, line_no)?
                    }
                    "arg" => command.argv.push(value.to_string()),
                    "cwd" => assign_once(&mut command.cwd, value.to_string(), key, line_no)?,
                    "expect" => assign_once(
                        &mut command.expected_exit,
                        value
                            .parse::<i32>()
                            .map_err(|_| format!("linha {line_no}: exit esperado inválido"))?,
                        key,
                        line_no,
                    )?,
                    "shell" => assign_once(
                        &mut command.shell,
                        parse_bool(value, line_no)?,
                        key,
                        line_no,
                    )?,
                    _ if field.starts_with("env.") => {
                        let name = &field[4..];
                        if name.is_empty()
                            || !name.chars().all(|ch| {
                                ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_'
                            })
                        {
                            return Err(format!(
                                "linha {line_no}: variável não autorizável: {name}"
                            ));
                        }
                        if command
                            .env
                            .insert(name.to_string(), value.to_string())
                            .is_some()
                        {
                            return Err(format!("linha {line_no}: variável duplicada: {name}"));
                        }
                    }
                    _ => return Err(format!("linha {line_no}: campo desconhecido: {key}")),
                }
            }
            _ => return Err(format!("linha {line_no}: campo desconhecido: {key}")),
        }
    }

    let schema = schema.ok_or("campo obrigatório ausente: schema")?;
    if schema != 1 {
        return Err(format!("schema não suportado: {schema}"));
    }
    let mut built = Vec::new();
    for id in command_order {
        let command = commands.remove(&id).expect("ordem consistente");
        let kind = command
            .kind
            .ok_or_else(|| format!("command.{id}.kind ausente"))?;
        let program = command
            .program
            .ok_or_else(|| format!("command.{id}.program ausente"))?;
        let shell = command.shell.unwrap_or(false);
        if !shell && matches!(program.as_str(), "sh" | "bash" | "/bin/sh" | "/bin/bash") {
            return Err(format!("command.{id}: shell implícito rejeitado"));
        }
        if matches!(kind, CommandKind::Pinker) && program != "pink" {
            return Err(format!(
                "command.{id}: comando Pinker tipado exige programa pink"
            ));
        }
        built.push(CommandSpec {
            id,
            kind,
            program,
            argv: command.argv,
            cwd: command.cwd.unwrap_or_else(|| ".".to_string()),
            expected_exit: command.expected_exit.unwrap_or(0),
            shell,
            env: command.env,
        });
    }
    let spec = Spec {
        schema,
        task_id: task_id.ok_or("campo obrigatório ausente: task_id")?,
        repo_root: repo_root.ok_or("campo obrigatório ausente: repo_root")?,
        worktree: worktree.ok_or("campo obrigatório ausente: worktree")?,
        delegated_root: delegated_root.ok_or("campo obrigatório ausente: delegated_root")?,
        expected_base: expected_base.ok_or("campo obrigatório ausente: expected_base")?,
        allowed_writes,
        allowed_changes,
        commands: built,
        accepted_verdict: accepted_verdict.ok_or("campo obrigatório ausente: verdict.accepted")?,
        blocked_verdict: blocked_verdict.ok_or("campo obrigatório ausente: verdict.blocked")?,
        human_verdict: human_verdict.ok_or("campo obrigatório ausente: verdict.human")?,
    };
    validate_paths(&spec)?;
    Ok(spec)
}

pub fn load_spec(path: &Path) -> Result<Spec, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("falha ao ler spec '{}': {err}", path.display()))?;
    parse_spec_text(&text)
}
// @pinker-nav:end development.agent.spec

// @pinker-nav:start development.agent.paths
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Confinamento lexical e canônico de repo, worktree, diretório delegado, cwd, escritas e mudanças permitidas; rejeita componentes pai, raízes fora do repositório canônico e qualquer cwd fora do worktree.
fn lexical_clean(path: &Path) -> Result<PathBuf, String> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => clean.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => clean.push(value),
            Component::ParentDir => {
                return Err(format!("caminho com fuga '..': {}", path.display()))
            }
        }
    }
    Ok(clean)
}

fn inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn resolve_under(root: &Path, value: &Path) -> Result<PathBuf, String> {
    let candidate = if value.is_absolute() {
        lexical_clean(value)?
    } else {
        lexical_clean(&root.join(value))?
    };
    if !inside(&candidate, root) {
        return Err(format!(
            "caminho fora da raiz autorizada: {}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

pub fn validate_paths(spec: &Spec) -> Result<(), String> {
    if !spec.repo_root.is_absolute()
        || !spec.worktree.is_absolute()
        || !spec.delegated_root.is_absolute()
    {
        return Err("repo_root, worktree e delegated_root devem ser absolutos".to_string());
    }
    let repo = lexical_clean(&spec.repo_root)?;
    let worktree = lexical_clean(&spec.worktree)?;
    let delegated = lexical_clean(&spec.delegated_root)?;
    if !inside(&worktree, &repo) || !inside(&delegated, &repo) {
        return Err("worktree e delegated_root devem ficar dentro de repo_root".to_string());
    }
    for path in &spec.allowed_writes {
        resolve_under(&delegated, Path::new(path))?;
    }
    for path in &spec.allowed_changes {
        resolve_under(&worktree, Path::new(path))?;
    }
    for command in &spec.commands {
        resolve_under(&worktree, Path::new(&command.cwd))?;
    }
    Ok(())
}
// @pinker-nav:end development.agent.paths

// @pinker-nav:start development.agent.artifacts
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Escrita atômica de JSON/Markdown, append durável de eventos monotônicos, escape JSON, SHA-256 zero-dependency e manifesto terminal ordenado que deliberadamente exclui a si próprio.
fn json_escape(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("arquivo sem pai: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("falha ao criar '{}': {err}", parent.display()))?;
    let tmp = parent.join(format!(
        ".{}.agent-tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&tmp, content)
        .map_err(|err| format!("falha ao escrever '{}': {err}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|err| format!("falha ao substituir '{}': {err}", path.display()))
}

fn append_event(
    path: &Path,
    sequence: u64,
    command: &str,
    status: &str,
    exit: Option<i32>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    writeln!(
        file,
        "{{\"sequence\":{sequence},\"command\":{},\"status\":{},\"exit_code\":{}}}",
        json_escape(command),
        json_escape(status),
        exit.map_or("null".to_string(), |code| code.to_string())
    )
    .map_err(|err| err.to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let o = i * 4;
            *word = u32::from_be_bytes(chunk[o..o + 4].try_into().expect("quatro bytes"));
        }
        for i in 16..64 {
            let a = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let b = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(a)
                .wrapping_add(w[i - 7])
                .wrapping_add(b);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut digest = String::with_capacity(64);
    for value in state {
        write!(&mut digest, "{value:08x}").expect("escrita em String");
    }
    digest
}

fn artifact_manifest(root: &Path) -> Result<String, String> {
    let artifacts = root.join("artefatos");
    let mut entries = Vec::new();
    for entry in fs::read_dir(artifacts).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_file()
            || path.file_name().and_then(|v| v.to_str()) == Some("artifact-manifest.json")
        {
            continue;
        }
        let bytes = fs::read(&path).map_err(|err| err.to_string())?;
        entries.push((
            path.file_name().unwrap().to_string_lossy().into_owned(),
            sha256_hex(&bytes),
        ));
    }
    entries.sort();
    let body = entries
        .iter()
        .map(|(name, hash)| format!("    {}: {}", json_escape(name), json_escape(hash)))
        .collect::<Vec<_>>()
        .join(",\n");
    Ok(format!("{{\n  \"schema\": 1,\n  \"algorithm\": \"SHA-256\",\n  \"self_included\": false,\n  \"artifacts\": {{\n{body}\n  }}\n}}\n"))
}
// @pinker-nav:end development.agent.artifacts

// @pinker-nav:start development.agent.runner
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Executor de processos estruturados ou Pinker tipado: cwd e env confinados, shell somente quando declarado, captura simultânea de stdout/stderr, persistência por comando, eco terminal, duração e comparação do código observado com o esperado.
#[derive(Clone, Debug)]
struct CommandResult {
    id: String,
    status: &'static str,
    exit_code: Option<i32>,
    expected_exit: i32,
    duration_ms: u128,
    shell: bool,
}

fn execute_one(spec: &Spec, command: &CommandSpec) -> Result<(CommandResult, Output), String> {
    let cwd = resolve_under(&spec.worktree, Path::new(&command.cwd))?;
    let mut process = if command.shell {
        let mut shell = Command::new("/bin/sh");
        shell.arg("-c").arg(&command.program);
        shell.args(&command.argv);
        shell
    } else if matches!(command.kind, CommandKind::Pinker) {
        let current = env::current_exe().map_err(|err| err.to_string())?;
        let executable = if current.exists() {
            current
        } else {
            let display = current.to_string_lossy();
            display
                .strip_suffix(" (deleted)")
                .map(PathBuf::from)
                .filter(|path| path.exists())
                .ok_or_else(|| format!("executável Pinker corrente indisponível: {display}"))?
        };
        let mut pink = Command::new(executable);
        pink.args(&command.argv);
        pink
    } else {
        let mut program = Command::new(&command.program);
        program.args(&command.argv);
        program
    };
    process.current_dir(cwd).env_clear();
    for key in ["PATH", "LANG", "LC_ALL", "TERM"] {
        if let Some(value) = env::var_os(key) {
            process.env(key, value);
        }
    }
    for (key, value) in &command.env {
        process.env(key, value);
    }
    let started = Instant::now();
    let output = process
        .output()
        .map_err(|err| format!("falha de harness em '{}': {err}", command.id))?;
    let exit_code = output.status.code();
    let status = if exit_code == Some(command.expected_exit) {
        "PASSED"
    } else {
        "FAILED"
    };
    Ok((
        CommandResult {
            id: command.id.clone(),
            status,
            exit_code,
            expected_exit: command.expected_exit,
            duration_ms: started.elapsed().as_millis(),
            shell: command.shell,
        },
        output,
    ))
}
// @pinker-nav:end development.agent.runner

// @pinker-nav:start development.agent.lifecycle
// @pinker-nav:domain development
// @pinker-nav:layer agent
// @pinker-nav:summary Ciclo iniciar/executar/verificar/status/relatorio: snapshots Git, execução fail-fast com NOT_RUN, validação de escopo exato, estados ACCEPTED/BLOCKED, códigos de saída mecânicos e emissão dos artefatos terminais canônicos.
fn git_output(worktree: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(args)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn workspace_json(spec: &Spec) -> Result<String, String> {
    let head = git_output(&spec.worktree, &["rev-parse", "HEAD"])?;
    let branch = git_output(&spec.worktree, &["branch", "--show-current"])?;
    let status = git_output(&spec.worktree, &["status", "--porcelain"])?;
    Ok(format!(
        "{{\n  \"head\": {},\n  \"branch\": {},\n  \"clean\": {},\n  \"status\": {}\n}}\n",
        json_escape(&head),
        json_escape(&branch),
        status.is_empty(),
        json_escape(&status)
    ))
}

fn ensure_layout(spec: &Spec) -> Result<(), String> {
    for relative in ["artefatos", "estado", "logs"] {
        fs::create_dir_all(spec.delegated_root.join(relative)).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn iniciar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let before = workspace_json(&spec)?;
    atomic_write(&spec.delegated_root.join("artefatos/environment.json"), format!("{{\n  \"schema\": 1,\n  \"task_id\": {},\n  \"repo_root\": {},\n  \"worktree\": {},\n  \"delegated_root\": {}\n}}\n", json_escape(&spec.task_id), json_escape(&spec.repo_root.display().to_string()), json_escape(&spec.worktree.display().to_string()), json_escape(&spec.delegated_root.display().to_string())).as_bytes())?;
    atomic_write(
        &spec.delegated_root.join("artefatos/workspace-before.json"),
        before.as_bytes(),
    )?;
    atomic_write(&spec.delegated_root.join("estado/run-state.json"), format!("{{\n  \"schema\": 1,\n  \"status\": \"READY\",\n  \"task_id\": {},\n  \"last_sequence\": 0\n}}\n", json_escape(&spec.task_id)).as_bytes())?;
    Ok(EXIT_ACCEPTED)
}

fn changed_paths(spec: &Spec) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&spec.worktree)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = stdout
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn scope_ok(spec: &Spec, changed: &[String]) -> bool {
    let allowed: BTreeSet<&str> = spec.allowed_changes.iter().map(String::as_str).collect();
    changed.iter().all(|path| allowed.contains(path.as_str()))
}

fn results_json(results: &[CommandResult]) -> String {
    results.iter().map(|result| format!("    {{\"id\":{},\"status\":{},\"exit_code\":{},\"expected_exit\":{},\"duration_ms\":{},\"shell\":{}}}", json_escape(&result.id), json_escape(result.status), result.exit_code.map_or("null".to_string(), |value| value.to_string()), result.expected_exit, result.duration_ms, result.shell)).collect::<Vec<_>>().join(",\n")
}

pub fn executar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    ensure_layout(&spec)?;
    let _ = iniciar(spec_path)?;
    let event_path = spec.delegated_root.join("estado/command-events.jsonl");
    atomic_write(&event_path, b"")?;
    let mut results = Vec::new();
    let mut blocked = false;
    for (index, command) in spec.commands.iter().enumerate() {
        let sequence = (index + 1) as u64;
        if blocked {
            append_event(&event_path, sequence, &command.id, "NOT_RUN", None)?;
            results.push(CommandResult {
                id: command.id.clone(),
                status: "NOT_RUN",
                exit_code: None,
                expected_exit: command.expected_exit,
                duration_ms: 0,
                shell: command.shell,
            });
            continue;
        }
        let (result, output) = execute_one(&spec, command)?;
        std::io::stdout()
            .write_all(&output.stdout)
            .map_err(|err| err.to_string())?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(|err| err.to_string())?;
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/{}.stdout.txt", command.id)),
            &output.stdout,
        )?;
        atomic_write(
            &spec
                .delegated_root
                .join(format!("logs/{}.stderr.txt", command.id)),
            &output.stderr,
        )?;
        append_event(
            &event_path,
            sequence,
            &command.id,
            result.status,
            result.exit_code,
        )?;
        blocked = result.status == "FAILED";
        results.push(result);
    }
    let changed = changed_paths(&spec)?;
    let scope = scope_ok(&spec, &changed);
    blocked |= !scope;
    let status = if blocked { "BLOCKED" } else { "ACCEPTED" };
    let verdict = if blocked {
        &spec.blocked_verdict
    } else {
        &spec.accepted_verdict
    };
    let changed_json = changed
        .iter()
        .map(|path| json_escape(path))
        .collect::<Vec<_>>()
        .join(",");
    atomic_write(
        &spec.delegated_root.join("artefatos/workspace-after.json"),
        workspace_json(&spec)?.as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/scope.json"),
        format!("{{\n  \"valid\": {scope},\n  \"changed_files\": [{changed_json}]\n}}\n")
            .as_bytes(),
    )?;
    atomic_write(
        &spec.delegated_root.join("artefatos/validation.json"),
        format!(
            "{{\n  \"status\": {},\n  \"commands\": [\n{}\n  ]\n}}\n",
            json_escape(status),
            results_json(&results)
        )
        .as_bytes(),
    )?;
    let result = format!("{{\n  \"status\": {},\n  \"verdict\": {},\n  \"task_id\": {},\n  \"scope_valid\": {scope},\n  \"commands\": [\n{}\n  ]\n}}\n", json_escape(status), json_escape(verdict), json_escape(&spec.task_id), results_json(&results));
    atomic_write(
        &spec.delegated_root.join("artefatos/resultado.json"),
        result.as_bytes(),
    )?;
    let report = format!("# Relatório pink agente\n\n- task: `{}`\n- status: **{}**\n- verdict: `{}`\n- comandos: {}\n- escopo válido: {}\n", spec.task_id, status, verdict, results.len(), scope);
    atomic_write(
        &spec.delegated_root.join("artefatos/RELATORIO.md"),
        report.as_bytes(),
    )?;
    atomic_write(&spec.delegated_root.join("estado/run-state.json"), format!("{{\n  \"schema\": 1,\n  \"status\": {},\n  \"verdict\": {},\n  \"last_sequence\": {}\n}}\n", json_escape(status), json_escape(verdict), results.len()).as_bytes())?;
    let manifest = artifact_manifest(&spec.delegated_root)?;
    atomic_write(
        &spec.delegated_root.join("artefatos/artifact-manifest.json"),
        manifest.as_bytes(),
    )?;
    Ok(if blocked { EXIT_BLOCKED } else { EXIT_ACCEPTED })
}

pub fn verificar(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let changed = changed_paths(&spec)?;
    if !scope_ok(&spec, &changed) {
        return Ok(EXIT_BLOCKED);
    }
    let expected = artifact_manifest(&spec.delegated_root)?;
    let actual = fs::read_to_string(spec.delegated_root.join("artefatos/artifact-manifest.json"))
        .map_err(|err| err.to_string())?;
    Ok(if expected == actual {
        EXIT_ACCEPTED
    } else {
        EXIT_BLOCKED
    })
}

pub fn status(spec_path: &Path, json: bool) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let result = fs::read_to_string(spec.delegated_root.join("artefatos/resultado.json"))
        .map_err(|err| err.to_string())?;
    if json {
        print!("{result}");
    } else if result.contains("\"status\": \"ACCEPTED\"") {
        println!("ACCEPTED");
    } else if result.contains("NEEDS_HUMAN_DECISION") {
        println!("NEEDS_HUMAN_DECISION");
    } else {
        println!("BLOCKED");
    }
    Ok(if result.contains("\"status\": \"ACCEPTED\"") {
        EXIT_ACCEPTED
    } else if result.contains("NEEDS_HUMAN_DECISION") {
        EXIT_NEEDS_HUMAN
    } else {
        EXIT_BLOCKED
    })
}

pub fn relatorio(spec_path: &Path) -> Result<i32, String> {
    let spec = load_spec(spec_path)?;
    let report = fs::read_to_string(spec.delegated_root.join("artefatos/RELATORIO.md"))
        .map_err(|err| err.to_string())?;
    print!("{report}");
    Ok(if report.contains("**ACCEPTED**") {
        EXIT_ACCEPTED
    } else {
        EXIT_BLOCKED
    })
}

pub fn timestamp_metadata() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
// @pinker-nav:end development.agent.lifecycle
