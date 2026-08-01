use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pinker-cli-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("criar diretório temporário");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pink() -> &'static str {
    env!("CARGO_BIN_EXE_pink")
}

fn repo_root() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn run(args: &[&str]) -> Output {
    Command::new(pink())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("executar pink")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr UTF-8")
}

fn assert_help(args: &[&str], expected_usage: &str) -> String {
    let output = run(args);
    assert_eq!(output.status.code(), Some(0), "args={args:?}");
    assert!(
        output.stderr.is_empty(),
        "args={args:?}: {}",
        stderr(&output)
    );
    let help = stdout(&output);
    assert!(help.contains(expected_usage), "args={args:?}: {help}");
    assert!(!help.is_empty(), "args={args:?}");
    help
}

#[test]
fn ajuda_principal_e_aliases_sao_equivalentes_e_bem_sucedidos() {
    let long = assert_help(&["--help"], "Uso: pink [OPÇÕES] ARQUIVO");
    let short = assert_help(&["-h"], "Uso: pink [OPÇÕES] ARQUIVO");
    let alias = assert_help(&["help"], "Uso: pink [OPÇÕES] ARQUIVO");
    assert_eq!(long, short);
    assert_eq!(long, alias);
}

#[test]
fn ajuda_de_todos_os_comandos_tem_tres_formas_equivalentes() {
    for command in ["build", "editor", "repl", "doc", "nav", "agente"] {
        let expected = format!("Uso: pink {command}");
        let by_help = assert_help(&["help", command], &expected);
        let by_long = assert_help(&[command, "--help"], &expected);
        let by_short = assert_help(&[command, "-h"], &expected);
        assert_eq!(by_help, by_long, "comando={command}");
        assert_eq!(by_help, by_short, "comando={command}");
    }
}

#[test]
fn ajuda_prevalece_sobre_argumentos_operacionais_sem_executar_comando() {
    assert_help(
        &["build", "arquivo-inexistente.pink", "--help"],
        "Uso: pink build",
    );
    assert_help(
        &["editor", "arquivo-inexistente.pink", "-h"],
        "Uso: pink editor",
    );
    assert_help(&["repl", "--help"], "Uso: pink repl");
    assert_help(&["doc", "mostrar", "ausente", "-h"], "Uso: pink doc");
    assert_help(&["nav", "mostrar", "ausente", "--help"], "Uso: pink nav");
    assert_help(
        &["agente", "status", "spec-ausente", "-h"],
        "Uso: pink agente",
    );
}

#[test]
fn versao_curta_e_longa_sao_equivalentes_e_usam_versao_do_pacote() {
    let long = run(&["--version"]);
    let short = run(&["-V"]);
    for output in [&long, &short] {
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty(), "{}", stderr(output));
        assert!(!output.stdout.is_empty());
        assert_eq!(
            stdout(output),
            format!("pink {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    assert_eq!(long.stdout, short.stdout);
}

#[test]
fn uso_invalido_e_uniformemente_diagnosticado_em_stderr_com_codigo_2() {
    let cases: &[(&[&str], &str)] = &[
        (&[], "nenhum argumento"),
        (&["--desconhecida"], "Flag desconhecida"),
        (&["-x"], "Flag desconhecida"),
        (&["version"], "Comando 'version' desconhecido"),
        (&["--version", "extra"], "não aceita argumentos"),
        (&["help", "desconhecido"], "Comando desconhecido para ajuda"),
        (&["help", "build", "extra"], "no máximo um COMANDO"),
        (&["build"], "Uso: pink build"),
        (&["build", "um.pink", "dois.pink"], "Apenas um arquivo"),
        (&["editor"], "Uso: pink editor"),
        (&["editor", "um.pink", "dois.pink"], "Apenas um arquivo"),
        (&["repl", "extra"], "não aceita argumentos posicionais"),
        (&["doc", "desconhecido"], "Subcomando doc desconhecido"),
        (&["doc", "mostrar"], "requer exatamente um argumento"),
        (
            &["doc", "marco", "extra"],
            "não aceita argumentos posicionais",
        ),
        (&["nav", "desconhecido"], "Subcomando nav desconhecido"),
        (&["nav", "mostrar"], "requer exatamente um argumento"),
        (
            &["nav", "sincronizar", "extra"],
            "não aceita argumentos posicionais",
        ),
        (
            &["agente", "desconhecido", "spec"],
            "Subcomando agente desconhecido",
        ),
        (&["agente", "status"], "Uso: pink agente"),
        (&["agente", "executar", "a", "b"], "Uso: pink agente"),
        (&["um.pink", "dois.pink"], "Apenas um arquivo"),
        (&["--", "arg-runtime"], "nenhum argumento"),
    ];

    for (args, diagnostic) in cases {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(
            output.stdout.is_empty(),
            "args={args:?}: {}",
            stdout(&output)
        );
        let error = stderr(&output);
        assert!(error.contains(diagnostic), "args={args:?}: {error}");
        assert!(error.contains("Uso: pink"), "args={args:?}: {error}");
    }
}

#[test]
fn falha_operacional_de_leitura_permanece_distinta_de_erro_de_uso() {
    let output = run(&["arquivo-que-nao-existe-414.pink"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("Falha ao ler"), "{error}");
    assert!(!error.contains("Uso: pink"), "{error}");
}

#[test]
fn modos_preexistentes_de_arquivo_check_build_doc_nav_e_agente_continuam_alcancaveis() {
    let source = Path::new(repo_root()).join("examples/principal_valida.pink");
    let source_text = source.to_str().expect("caminho UTF-8");

    for args in [
        [source_text].as_slice(),
        ["--check", source_text].as_slice(),
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(output.stderr.is_empty(), "{}", stderr(&output));
    }

    let build_dir = TempDir::new("build");
    let build = run(&[
        "build",
        "--out-dir",
        build_dir.path().to_str().expect("caminho UTF-8"),
        source_text,
    ]);
    assert_eq!(build.status.code(), Some(0), "{}", stderr(&build));
    assert!(build_dir.path().join("principal_valida.s").is_file());

    let doc = run(&["doc", "--repo", repo_root(), "marco"]);
    assert_eq!(doc.status.code(), Some(0), "{}", stderr(&doc));
    assert!(!doc.stdout.is_empty());

    let nav = run(&[
        "nav",
        "--repo",
        repo_root(),
        "buscar",
        "cli",
        "--limite",
        "1",
    ]);
    assert_eq!(nav.status.code(), Some(0), "{}", stderr(&nav));
    assert!(!nav.stdout.is_empty());

    let operational_agent = run(&["agente", "status", "spec-que-nao-existe-414"]);
    assert_eq!(operational_agent.status.code(), Some(1));
    assert!(stderr(&operational_agent).contains("E-AGENT"));
}

#[test]
fn codigos_estruturados_de_catalogo_e_ausencia_de_resultado_sao_preservados() {
    let empty_repo = TempDir::new("catalogo-ausente");
    fs::create_dir(empty_repo.path().join(".pinker")).expect("criar configuração");
    fs::copy(
        Path::new(repo_root()).join(".pinker/doc.toml"),
        empty_repo.path().join(".pinker/doc.toml"),
    )
    .expect("copiar configuração");
    let missing_catalog = Command::new(pink())
        .args(["nav", "buscar", "cli"])
        .current_dir(empty_repo.path())
        .output()
        .expect("nav sem catálogo");
    assert_eq!(missing_catalog.status.code(), Some(3));

    let no_result = run(&["nav", "--repo", repo_root(), "buscar", "zzqv414nohit"]);
    assert_eq!(no_result.status.code(), Some(4), "{}", stderr(&no_result));
}

#[test]
fn ajuda_e_versao_funcionam_fora_do_checkout_sem_efeitos_colaterais() {
    let outside = TempDir::new("fora-checkout");
    for args in [["--help"].as_slice(), ["--version"].as_slice()] {
        let output = Command::new(pink())
            .args(args)
            .current_dir(outside.path())
            .output()
            .expect("executar fora do checkout");
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(output.stderr.is_empty());
    }
    assert_eq!(fs::read_dir(outside.path()).expect("listar").count(), 0);
}

#[test]
fn nome_de_invocacao_usa_apenas_componente_final_em_paths_absolutos_e_com_espacos() {
    let absolute = Command::new(pink())
        .arg("--help")
        .output()
        .expect("path absoluto");
    assert_eq!(absolute.status.code(), Some(0));
    let absolute_help = stdout(&absolute);
    assert!(absolute_help.contains("Uso: pink "));
    assert!(!absolute_help.contains(repo_root()), "{absolute_help}");

    let temp = TempDir::new("caminho com espaços");
    let spaced_dir = temp.path().join("caminho com espaços");
    fs::create_dir(&spaced_dir).expect("diretório com espaços");
    let copied = spaced_dir.join("pink");
    fs::copy(pink(), &copied).expect("copiar pink");
    let spaced = Command::new(&copied)
        .arg("--help")
        .output()
        .expect("path com espaços");
    assert_eq!(spaced.status.code(), Some(0));
    let spaced_help = stdout(&spaced);
    assert!(spaced_help.contains("Uso: pink "));
    assert!(!spaced_help.contains(temp.path().to_str().expect("UTF-8")));
}

#[test]
fn nome_de_invocacao_relativo_preserva_apenas_o_nome_final() {
    let temp = TempDir::new("relativo");
    let copied = temp.path().join("pink");
    fs::copy(pink(), copied).expect("copiar pink");
    let output = Command::new("./pink")
        .arg("--help")
        .current_dir(temp.path())
        .output()
        .expect("path relativo");
    assert_eq!(output.status.code(), Some(0));
    let help = stdout(&output);
    assert!(help.contains("Uso: pink "));
    assert!(!help.contains("./pink"));
}

#[cfg(unix)]
#[test]
fn symlink_preserva_nome_alternativo_na_ajuda_principal_e_de_subcomando() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("symlink");
    let link = temp.path().join("symlink-chamado-pinker");
    symlink(pink(), &link).expect("criar symlink");

    for args in [["--help"].as_slice(), ["build", "--help"].as_slice()] {
        let output = Command::new(&link)
            .args(args)
            .output()
            .expect("executar symlink");
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(output.stderr.is_empty());
        let help = stdout(&output);
        assert!(help.contains("Uso: symlink-chamado-pinker "), "{help}");
        assert!(!help.contains(temp.path().to_str().expect("UTF-8")));
    }
}

#[cfg(unix)]
#[test]
fn nome_de_invocacao_vazio_usa_fallback_pink() {
    use std::os::unix::process::CommandExt;

    let output = Command::new(pink())
        .arg0("")
        .arg("--help")
        .output()
        .expect("argv0 vazio");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Uso: pink "));
}
