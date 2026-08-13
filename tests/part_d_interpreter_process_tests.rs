//! Parte D, Step 3 — execução estruturada real no interpretador hospedado.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

// @pinker-nav:start evidencia.processos.parte-d-interpreter-step-3
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova a execução estruturada real no interpretador: spawn único sem shell, fronteiras de argv, stdin integral e EOF, captura simultânea e separada de stdout/stderr grandes, cwd só do filho, ambiente herdado+overlay e PATH, status normal/não-zero, término anormal e UTF-8 inválido recuperáveis, timeout simples e adversarial com descendente segurando pipes, reap/cleanup e accessors sem reexecução.

fn literal(texto: &str) -> String {
    let mut saida = String::from("\"");
    for caractere in texto.chars() {
        match caractere {
            '\\' => saida.push_str("\\\\"),
            '"' => saida.push_str("\\\""),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            outro => saida.push(outro),
        }
    }
    saida.push('"');
    saida
}

fn fonte(
    programa: &str,
    argumentos: &[String],
    entrada: &str,
    diretorio: &str,
    ambiente: &BTreeMap<String, String>,
    limite: &str,
    corpo_ok: &str,
) -> String {
    let anexos = argumentos
        .iter()
        .map(|argumento| {
            format!(
                "    lista_verso_anexar(argumentos, {});",
                literal(argumento)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let overlays = ambiente
        .iter()
        .map(|(chave, valor)| {
            format!(
                "    mapa_verso_verso_definir(ambiente, {}, {});",
                literal(chave),
                literal(valor)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"pacote main;
apelido Res = Resultado<SaidaProcesso, verso>;

carinho principal() -> bombom {{
    nova muda argumentos: lista<verso> = lista_verso_criar();
{anexos}
    nova muda ambiente: mapa<verso,verso> = mapa_verso_verso_criar();
{overlays}
    nova resultado: Res = executar_processo_estruturado(
        {programa}, argumentos, {entrada}, {diretorio}, ambiente, {limite}
    );
    encaixe resultado {{
        caso Res.Ok(saida) {{
            falar("OK");
            {corpo_ok}
            mimo 0;
        }}
        caso Res.Erro(erro) {{
            falar("ERRO");
            falar(erro);
            mimo 0;
        }}
    }}
    mimo 99;
}}
"#,
        programa = literal(programa),
        entrada = literal(entrada),
        diretorio = literal(diretorio),
    )
}

fn rodar(fonte: &str, ambiente_pai: &[(&str, &str)]) -> (Output, Duration) {
    let dir = common::NativeArtifactDir::create().expect("sandbox de execução");
    let caminho = dir.path().join("step3.pink");
    fs::write(&caminho, fonte).expect("gravar fonte");
    let inicio = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&caminho)
        .envs(ambiente_pai.iter().copied())
        .output()
        .expect("executar interpretador");
    (output, inicio.elapsed())
}

fn rodar_com_watchdog(
    fonte: &str,
    ambiente_pai: &[(&str, &str)],
    watchdog: Duration,
) -> (Output, Duration) {
    let dir = common::NativeArtifactDir::create().expect("sandbox de execução com watchdog");
    let caminho = dir.path().join("step3-watchdog.pink");
    fs::write(&caminho, fonte).expect("gravar fonte");
    let inicio = Instant::now();
    let mut pink = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&caminho)
        .envs(ambiente_pai.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executar interpretador com watchdog");

    loop {
        if pink.try_wait().expect("observar interpretador").is_some() {
            let output = pink.wait_with_output().expect("coletar interpretador");
            return (output, inicio.elapsed());
        }
        if inicio.elapsed() >= watchdog {
            let _ = pink.kill();
            let output = pink.wait_with_output().expect("coletar watchdog");
            panic!(
                "watchdog externo excedido ({watchdog:?}); stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn sucesso(output: &Output) -> String {
    assert!(
        output.status.success(),
        "pink falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr externo inesperado: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("stdout Pinker UTF-8")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn erro_operacional(fonte: &str) -> String {
    let (output, _) = rodar(fonte, &[]);
    let texto = sucesso(&output);
    assert!(texto.starts_with("ERRO\n"), "{texto}");
    assert!(!texto.contains("OK_INESPERADO"), "{texto}");
    texto
}

#[test]
fn argv_preserva_fronteiras_e_nao_introduz_shell() {
    let especiais = [
        "com espaço",
        "$HOME",
        "*",
        ";",
        "|",
        "&",
        "linha\nnova",
        "\"aspas\"",
        "'aspas'",
    ];
    let mut argumentos = vec!["argv".to_string()];
    argumentos.extend(especiais.iter().map(|item| (*item).to_string()));
    let fonte = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &argumentos,
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (output, _) = rodar(&fonte, &[("HOME", "/valor-que-nao-pode-expandir")]);
    let texto = sucesso(&output);
    assert!(texto.starts_with("OK\n"), "{texto}");
    for (indice, esperado) in especiais.iter().enumerate() {
        let bytes = esperado.as_bytes();
        let hex = bytes.iter().fold(String::new(), |mut texto, byte| {
            use std::fmt::Write as _;
            write!(&mut texto, "{byte:02x}").expect("String não falha");
            texto
        });
        assert!(
            texto.contains(&format!("ARG {indice} {} {hex}", bytes.len())),
            "argv {indice} perdeu fronteira: {texto}"
        );
    }
}

#[test]
fn stdin_e_enviado_integralmente_e_writer_fecha_inclusive_vazio() {
    for entrada in [String::new(), "linha 1\nlinha 2\n".repeat(8 * 1024)] {
        let argumentos = vec!["stdin".to_string()];
        let fonte = fonte(
            env!("CARGO_BIN_EXE_pinker_part_d_filho"),
            &argumentos,
            &entrada,
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(3000)",
            "falar(processo_saida(saida));",
        );
        let (output, elapsed) = rodar(&fonte, &[]);
        let texto = sucesso(&output);
        assert!(
            elapsed < Duration::from_secs(3),
            "EOF não chegou: {elapsed:?}"
        );
        assert!(
            texto.contains(&format!(
                "STDIN {} {:016x}\nEOF",
                entrada.len(),
                fnv1a64(entrada.as_bytes())
            )),
            "{texto}"
        );
    }
}

#[test]
fn stdout_stderr_mesma_execucao_canais_distintos_e_grandes_sem_deadlock() {
    let pequeno = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &["small".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        r#"falar(processo_codigo(saida));
            falar(processo_saida(saida));
            falar(processo_erro(saida));"#,
    );
    let (output, _) = rodar(&pequeno, &[]);
    let texto = sucesso(&output);
    assert_eq!(texto, "OK\n0\nstdout-small\nstderr-small\n");

    let grande = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &["large".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.Ate(5000)",
        r#"falar(tamanho_verso(processo_saida(saida)));
            falar(tamanho_verso(processo_erro(saida)));"#,
    );
    let (output, elapsed) = rodar(&grande, &[]);
    let texto = sucesso(&output);
    assert_eq!(texto, "OK\n2097152\n2097152\n");
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
}

#[test]
fn snapshot_accessors_nao_reexecutam_e_exit_nao_zero_continua_ok() {
    let dir = common::NativeArtifactDir::create().expect("sandbox contador");
    let contador = dir.path().join("contador.txt");
    let pidfile = dir.path().join("contador.pid");
    let argumentos = vec![
        "counter".to_string(),
        contador.display().to_string(),
        pidfile.display().to_string(),
    ];
    let fonte = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &argumentos,
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        r#"falar(processo_codigo(saida));
            falar(processo_saida(saida));
            falar(processo_erro(saida));
            falar(processo_codigo(saida));
            falar(processo_saida(saida));
            falar(processo_erro(saida));"#,
    );
    let (output, _) = rodar(&fonte, &[]);
    let texto = sucesso(&output);
    assert_eq!(
        texto,
        "OK\n7\ncontador-stdout\ncontador-stderr\n7\ncontador-stdout\ncontador-stderr\n"
    );
    assert_eq!(fs::read_to_string(contador).expect("contador"), "1");
    let pid = fs::read_to_string(pidfile).expect("pid da execução");
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{}", pid.trim())).exists(),
        "filho positivo não foi reapado antes dos accessors/retorno"
    );
}

#[test]
fn implementacao_tem_um_spawn_um_poll_loop_e_zero_threads() {
    let implementacao = include_str!("../src/processo_estruturado_hospedado.rs");
    assert_eq!(implementacao.matches("comando.spawn()").count(), 1);
    assert!(implementacao.contains("poll_descritores(&mut descritores"));
    assert!(!implementacao.contains("thread::spawn"));
}

#[cfg(unix)]
#[test]
fn cwd_ambiente_e_path_afetam_so_o_filho_e_erros_sao_recuperaveis() {
    use std::os::unix::fs::{symlink, PermissionsExt as _};

    let dir = common::NativeArtifactDir::create().expect("sandbox cwd");
    let cwd_filho = dir.path().join("cwd-filho");
    fs::create_dir(&cwd_filho).expect("cwd filho");
    let cwd_pai = std::env::current_dir().expect("cwd pai");

    let cwd = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &["cwd".to_string()],
        "",
        cwd_filho.to_str().expect("cwd UTF-8"),
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (output, _) = rodar(&cwd, &[]);
    assert!(
        sucesso(&output).contains(cwd_filho.to_str().expect("cwd UTF-8")),
        "filho não observou cwd solicitado"
    );
    assert_eq!(std::env::current_dir().expect("cwd pai depois"), cwd_pai);

    let mut overlay = BTreeMap::new();
    overlay.insert("PINKER_STEP3_OVERRIDE".to_string(), "novo".to_string());
    overlay.insert("PINKER_STEP3_EQUALS".to_string(), "a=b=c".to_string());
    let env_fonte = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &[
            "env".to_string(),
            "PINKER_STEP3_INHERITED".to_string(),
            "PINKER_STEP3_OVERRIDE".to_string(),
            "PINKER_STEP3_EQUALS".to_string(),
            "PATH".to_string(),
        ],
        "",
        "",
        &overlay,
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (output, _) = rodar(
        &env_fonte,
        &[
            ("PINKER_STEP3_INHERITED", "herdada"),
            ("PINKER_STEP3_OVERRIDE", "antigo"),
        ],
    );
    let texto = sucesso(&output);
    for esperado in [
        "ENV PINKER_STEP3_INHERITED 7 68657264616461",
        "ENV PINKER_STEP3_OVERRIDE 4 6e6f766f",
        "ENV PINKER_STEP3_EQUALS 5 613d623d63",
        "ENV PATH 28 2f7573722f6c6f63616c2f62696e3a2f7573722f62696e3a2f62696e",
    ] {
        assert!(texto.contains(esperado), "{esperado}: {texto}");
    }

    let atalho = dir.path().join("pinker-step3-filho");
    symlink(env!("CARGO_BIN_EXE_pinker_part_d_filho"), atalho).expect("symlink PATH");
    let mut path_overlay = BTreeMap::new();
    path_overlay.insert("PATH".to_string(), dir.path().display().to_string());
    let via_path = fonte(
        "pinker-step3-filho",
        &["small".to_string()],
        "",
        "",
        &path_overlay,
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (output, _) = rodar(&via_path, &[]);
    assert_eq!(sucesso(&output), "OK\nstdout-small\n");

    for diretorio in [dir.path().join("inexistente"), {
        let arquivo = dir.path().join("nao-diretorio");
        fs::write(&arquivo, "x").expect("arquivo cwd");
        arquivo
    }] {
        let fonte = fonte(
            env!("CARGO_BIN_EXE_pinker_part_d_filho"),
            &["small".to_string()],
            "",
            diretorio.to_str().expect("path UTF-8"),
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        erro_operacional(&fonte);
        assert_eq!(std::env::current_dir().expect("cwd pai"), cwd_pai);
    }

    let sem_permissao = dir.path().join("sem-permissao");
    fs::write(&sem_permissao, "#!/bin/false\n").expect("fixture sem permissão");
    fs::set_permissions(&sem_permissao, fs::Permissions::from_mode(0o644)).expect("permissões");
    for programa in [
        "",
        "/executavel/definitivamente/ausente",
        sem_permissao.to_str().unwrap(),
    ] {
        let fonte = fonte(
            programa,
            &[],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        erro_operacional(&fonte);
    }

    let mut ambiente_invalido = BTreeMap::new();
    ambiente_invalido.insert("NOME=INVALIDO".to_string(), "x".to_string());
    let fonte = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &["small".to_string()],
        "",
        "",
        &ambiente_invalido,
        "LimiteTempo.SemLimite",
        "falar(\"OK_INESPERADO\");",
    );
    erro_operacional(&fonte);
}

#[cfg(unix)]
#[test]
fn path_ambiente_nao_decide_executavel_mas_overlay_explicito_decide() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = common::NativeArtifactDir::create().expect("sandbox PATH HR3");
    let falso_true = dir.path().join("true");
    fs::write(&falso_true, "#!/bin/sh\nprintf 'FAKE_TRUE\\n'\nexit 73\n")
        .expect("fixture true falsa");
    fs::set_permissions(&falso_true, fs::Permissions::from_mode(0o755))
        .expect("fixture executável");
    let path_ambiente = format!("{}:/usr/local/bin:/usr/bin:/bin", dir.path().display());
    let corpo = "falar(processo_codigo(saida)); falar(processo_saida(saida));";

    let default_saneada = fonte(
        "true",
        &[],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        corpo,
    );
    let (output, _) = rodar(&default_saneada, &[("PATH", path_ambiente.as_str())]);
    assert_eq!(
        sucesso(&output),
        "OK\n0\n\n",
        "PATH ambiente não pode selecionar a fixture falsa"
    );

    let mut overlay = BTreeMap::new();
    overlay.insert("PATH".to_string(), dir.path().display().to_string());
    let override_explicito = fonte(
        "true",
        &[],
        "",
        "",
        &overlay,
        "LimiteTempo.SemLimite",
        corpo,
    );
    let (output, _) = rodar(&override_explicito, &[("PATH", path_ambiente.as_str())]);
    assert_eq!(
        sucesso(&output),
        "OK\n73\nFAKE_TRUE\n\n",
        "overlay PATH explícito precisa selecionar a fixture falsa"
    );
}

#[test]
fn terminacao_anormal_utf8_invalido_e_timeout_simples_sao_erros_sem_snapshot() {
    for modo in ["abnormal", "invalid-stdout", "invalid-stderr"] {
        let fonte = fonte(
            env!("CARGO_BIN_EXE_pinker_part_d_filho"),
            &[modo.to_string()],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        let texto = erro_operacional(&fonte);
        if modo == "abnormal" {
            assert!(texto.contains("sem código normal"), "{texto}");
            assert!(!texto.contains("128"), "{texto}");
        } else {
            assert!(texto.contains("não é UTF-8 válido"), "{texto}");
            assert!(!texto.contains('�'), "{texto}");
        }
    }

    let dir = common::NativeArtifactDir::create().expect("sandbox timeout");
    let pidfile = dir.path().join("filho.pid");
    let fonte = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &[
            "sleep-pid".to_string(),
            "5000".to_string(),
            pidfile.display().to_string(),
        ],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.Ate(300)",
        "falar(\"OK_INESPERADO\");",
    );
    let (output, elapsed) = rodar(&fonte, &[]);
    let texto = sucesso(&output);
    assert!(texto.starts_with("ERRO\n"), "{texto}");
    assert!(texto.contains("limite de tempo excedido"), "{texto}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    let pid = fs::read_to_string(&pidfile).expect("pid direto");
    assert!(
        !std::path::Path::new(&format!("/proc/{}", pid.trim())).exists(),
        "filho direto não foi reapado"
    );
}

#[test]
fn hr5_timeout_governa_io_continuo_e_sem_limite_nao_trunca() {
    for modo in ["continuous-stdout", "continuous-both"] {
        let fonte = fonte(
            env!("CARGO_BIN_EXE_pinker_part_d_filho"),
            &[modo.to_string(), "2000".to_string()],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(150)",
            "falar(\"OK_INESPERADO\");",
        );
        let (output, elapsed) = rodar_com_watchdog(&fonte, &[], Duration::from_secs(4));
        let texto = sucesso(&output);
        assert!(texto.starts_with("ERRO\n"), "{modo}: {texto}");
        assert!(
            texto.contains("limite de tempo excedido"),
            "{modo}: {texto}"
        );
        assert!(
            elapsed < Duration::from_millis(1200),
            "{modo} ocultou deadline: {elapsed:?}"
        );
    }

    let sem_limite = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &["large".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        r#"falar(tamanho_verso(processo_saida(saida)));
            falar(tamanho_verso(processo_erro(saida)));"#,
    );
    let (output, _) = rodar_com_watchdog(&sem_limite, &[], Duration::from_secs(5));
    assert_eq!(
        sucesso(&output),
        "OK\n2097152\n2097152\n",
        "quantum não pode truncar captura SemLimite"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn timeout_nao_espera_eof_de_descendente_mas_sem_limite_espera() {
    let dir = common::NativeArtifactDir::create().expect("sandbox descendente");
    let pidfile = dir.path().join("descendente-timeout.pid");
    let timeout = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &[
            "descendant".to_string(),
            "2000".to_string(),
            pidfile.display().to_string(),
        ],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.Ate(300)",
        "falar(\"OK_INESPERADO\");",
    );
    let (output, elapsed) = rodar(&timeout, &[]);
    let texto = sucesso(&output);
    assert!(texto.starts_with("ERRO\n"), "{texto}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    let pid = fs::read_to_string(&pidfile)
        .expect("pid descendente")
        .trim()
        .parse::<u32>()
        .expect("pid inteiro");
    assert!(
        std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "o descendente deveria continuar vivo segurando write-end"
    );
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status();

    let pidfile = dir.path().join("descendente-sem-limite.pid");
    let sem_limite = fonte(
        env!("CARGO_BIN_EXE_pinker_part_d_filho"),
        &[
            "descendant-exit".to_string(),
            "350".to_string(),
            pidfile.display().to_string(),
        ],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_codigo(saida));",
    );
    let (output, elapsed) = rodar(&sem_limite, &[]);
    assert_eq!(sucesso(&output), "OK\n0\n");
    assert!(
        elapsed >= Duration::from_millis(300),
        "SemLimite não esperou EOF herdado: {elapsed:?}"
    );
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

// @pinker-nav:end evidencia.processos.parte-d-interpreter-step-3
