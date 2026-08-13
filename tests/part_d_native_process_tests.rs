//! Parte D, Step 4 — paridade real entre interpretador e ELF nativo.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

// @pinker-nav:start evidencia.processos.parte-d-native-step-4
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Executa a mesma superfície estruturada no interpretador e em ELF ligado ao runtime nativo real, comparando argv sem shell, stdin+EOF, stdout/stderr separados e grandes, ambiente/cwd/PATH, status normal e não-zero, falhas de spawn, término anormal, UTF-8 estrito, Ate(0) sem spawn nem efeito externo, timeout simples/descendente/output contínuo, captura SemLimite, single-spawn e accessors sem reexecução sob watchdog externo.

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

fn runtime_nativo() -> &'static Path {
    static RUNTIME: OnceLock<PathBuf> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            let raiz = std::env::current_dir().expect("raiz do repositório");
            let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
            let build = cargo
                .args(["build", "--manifest-path", "runtime/pinker_rt/Cargo.toml"])
                .logical_case("part-d-step4-runtime-build")
                .timeout(Duration::from_secs(120))
                .output()
                .expect("compilar runtime nativo Step 4");
            assert!(
                build.status.success(),
                "runtime nativo falhou: {}",
                String::from_utf8_lossy(&build.stderr)
            );
            let biblioteca = raiz.join("target/debug/libpinker_rt.a");
            assert!(biblioteca.is_file(), "staticlib nativa ausente");
            biblioteca
        })
        .as_path()
}

struct ProgramaNativo {
    fonte: PathBuf,
    binario: PathBuf,
}

fn compilar(dir: &NativeArtifactDir, nome: &str, codigo: &str) -> ProgramaNativo {
    let fonte = dir.path().join(format!("{nome}.pink"));
    fs::write(&fonte, codigo).expect("gravar fonte Step 4");
    let saida = dir.path().join(format!("out-{nome}"));
    fs::create_dir(&saida).expect("diretório de build Step 4");
    let mut pink = Command::new(env!("CARGO_BIN_EXE_pink"));
    let build = pink
        .args(["build", "--nativo", "--out-dir"])
        .arg(&saida)
        .arg(&fonte)
        .env("PINKER_RT_LIB", runtime_nativo())
        .logical_case(&format!("part-d-step4-{nome}-build"))
        .timeout(Duration::from_secs(60))
        .output()
        .expect("build nativo Step 4");
    assert!(
        build.status.success(),
        "build nativo {nome} falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    ProgramaNativo {
        fonte,
        binario: saida.join(nome),
    }
}

fn executar(
    programa: &ProgramaNativo,
    nativo: bool,
    ambiente: &[(&str, &str)],
    cwd: Option<&Path>,
    timeout: Duration,
    caso: &str,
) -> (Output, Duration) {
    let mut comando = if nativo {
        Command::new(&programa.binario)
    } else {
        let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
        comando.arg("--run").arg(&programa.fonte);
        comando
    };
    for (chave, valor) in ambiente {
        comando.env(chave, valor);
    }
    if let Some(cwd) = cwd {
        comando.current_dir(cwd);
    }
    let inicio = Instant::now();
    let output = comando
        .logical_case(caso)
        .timeout(timeout)
        .capture_limit(16 * 1024 * 1024)
        .output()
        .expect("executar ponta Step 4");
    (output, inicio.elapsed())
}

fn paridade(
    dir: &NativeArtifactDir,
    nome: &str,
    codigo: &str,
    ambiente: &[(&str, &str)],
    cwd: Option<&Path>,
    timeout: Duration,
) -> (Output, Output, Duration, Duration) {
    let programa = compilar(dir, nome, codigo);
    let (interpretado, tempo_interpretado) = executar(
        &programa,
        false,
        ambiente,
        cwd,
        timeout,
        &format!("{nome}-interpretado"),
    );
    let (nativo, tempo_nativo) = executar(
        &programa,
        true,
        ambiente,
        cwd,
        timeout,
        &format!("{nome}-nativo"),
    );
    (interpretado, nativo, tempo_interpretado, tempo_nativo)
}

fn exigir_sucesso_paritario(nome: &str, interpretado: &Output, nativo: &Output) -> String {
    assert_eq!(interpretado.status.code(), Some(0), "{nome}: interpretador");
    assert_eq!(nativo.status.code(), Some(0), "{nome}: nativo");
    assert!(
        interpretado.stderr.is_empty(),
        "{nome}: stderr interpretador"
    );
    assert!(
        nativo.stderr.is_empty(),
        "{nome}: stderr nativo: {}",
        String::from_utf8_lossy(&nativo.stderr)
    );
    assert_eq!(
        interpretado.stdout, nativo.stdout,
        "{nome}: observável divergiu"
    );
    String::from_utf8(nativo.stdout.clone()).expect("stdout Step 4 UTF-8")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn paridade_nativa_argv_stdin_canais_status_e_sem_limite() {
    let dir = NativeArtifactDir::create().expect("sandbox Step 4 positivo");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");

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
    let mut argv = vec!["argv".to_string()];
    argv.extend(especiais.iter().map(|item| (*item).to_string()));
    let codigo = fonte(
        fixture,
        &argv,
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (i, n, _, _) = paridade(&dir, "argv", &codigo, &[], None, Duration::from_secs(5));
    let texto = exigir_sucesso_paritario("argv", &i, &n);
    for (indice, item) in especiais.iter().enumerate() {
        assert!(
            texto.contains(&format!("ARG {indice} {}", item.len())),
            "argv {indice}: {texto}"
        );
    }

    let entrada = "linha de entrada\n".repeat(10_000);
    let codigo = fonte(
        fixture,
        &["stdin".to_string()],
        &entrada,
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (i, n, _, _) = paridade(&dir, "stdin", &codigo, &[], None, Duration::from_secs(5));
    let texto = exigir_sucesso_paritario("stdin", &i, &n);
    assert!(texto.contains(&format!(
        "STDIN {} {:016x}\nEOF",
        entrada.len(),
        fnv1a64(entrada.as_bytes())
    )));

    for (nome, modo, esperado) in [
        (
            "canais-pequenos",
            "small",
            "OK\n0\nstdout-small\nstderr-small\n",
        ),
        ("stdout-only", "stdout-only", "OK\n0\nstdout-only\n\n"),
        ("stderr-only", "stderr-only", "OK\n0\n\nstderr-only\n"),
        ("exit-nao-zero", "status", "OK\n7\n\n\n"),
    ] {
        let argumentos = if modo == "status" {
            vec![modo.to_string(), "7".to_string()]
        } else {
            vec![modo.to_string()]
        };
        let codigo = fonte(
            fixture,
            &argumentos,
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(processo_codigo(saida)); falar(processo_saida(saida)); falar(processo_erro(saida));",
        );
        let (i, n, _, _) = paridade(&dir, nome, &codigo, &[], None, Duration::from_secs(5));
        assert_eq!(exigir_sucesso_paritario(nome, &i, &n), esperado);
    }

    // Esta fixture captura SIGPIPE em `.init_array`, antes de `lang_start`
    // alterar a disposição. Assim a prova mede o estado realmente herdado do
    // `exec`, e não o estado posterior imposto pelo runtime da própria fixture.
    let fixture_sigpipe = env!("CARGO_BIN_EXE_pinker_hf412_filho_stdin");
    let codigo = fonte(
        fixture_sigpipe,
        &["sigpipe-disposicao-imprime".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_codigo(saida)); falar(processo_saida(saida));",
    );
    let (i, n, _, _) = paridade(
        &dir,
        "sigpipe-default",
        &codigo,
        &[],
        None,
        Duration::from_secs(5),
    );
    assert_eq!(
        exigir_sucesso_paritario("sigpipe-default", &i, &n),
        "OK\n0\nSIG_DFL\n"
    );

    let codigo = fonte(
        fixture,
        &["large".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(tamanho_verso(processo_saida(saida))); falar(tamanho_verso(processo_erro(saida)));",
    );
    let (i, n, _, _) = paridade(&dir, "grande", &codigo, &[], None, Duration::from_secs(8));
    assert_eq!(
        exigir_sucesso_paritario("grande", &i, &n),
        "OK\n2097152\n2097152\n"
    );

    #[cfg(target_os = "linux")]
    {
        let pidfile = dir.path().join("sem-limite-descendente.pid");
        let codigo = fonte(
            fixture,
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
        let (i, n, tempo_i, tempo_n) = paridade(
            &dir,
            "sem-limite-descendente",
            &codigo,
            &[],
            None,
            Duration::from_secs(3),
        );
        assert_eq!(
            exigir_sucesso_paritario("sem-limite-descendente", &i, &n),
            "OK\n0\n"
        );
        assert!(tempo_i >= Duration::from_millis(300));
        assert!(tempo_n >= Duration::from_millis(300));
    }
}

#[test]
fn paridade_nativa_cwd_ambiente_e_path() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = NativeArtifactDir::create().expect("sandbox Step 4 host");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");
    let cwd_filho = dir.path().join("cwd-filho");
    fs::create_dir(&cwd_filho).unwrap();
    let cwd_pai = std::env::current_dir().unwrap();
    let codigo = fonte(
        fixture,
        &["cwd".to_string()],
        "",
        cwd_filho.to_str().unwrap(),
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let (i, n, _, _) = paridade(&dir, "cwd", &codigo, &[], None, Duration::from_secs(5));
    let texto = exigir_sucesso_paritario("cwd", &i, &n);
    assert!(texto.contains(cwd_filho.to_str().unwrap()));
    assert_eq!(std::env::current_dir().unwrap(), cwd_pai);

    let arquivo_cwd = dir.path().join("nao-diretorio");
    fs::write(&arquivo_cwd, "x").unwrap();
    for (indice, invalido) in [dir.path().join("cwd-ausente"), arquivo_cwd]
        .into_iter()
        .enumerate()
    {
        let codigo = fonte(
            fixture,
            &["small".to_string()],
            "",
            invalido.to_str().unwrap(),
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        let nome = format!("cwd-invalido-{indice}");
        let (i, n, _, _) = paridade(&dir, &nome, &codigo, &[], None, Duration::from_secs(5));
        for output in [i, n] {
            assert_eq!(output.status.code(), Some(0));
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .starts_with("ERRO\n"));
        }
        assert_eq!(std::env::current_dir().unwrap(), cwd_pai);
    }

    let mut overlay = BTreeMap::new();
    overlay.insert("PINKER_STEP4_OVERRIDE".into(), "novo".into());
    overlay.insert("PINKER_TEST".into(), "a=b=c".into());
    let codigo = fonte(
        fixture,
        &[
            "env".to_string(),
            "PINKER_STEP4_INHERITED".to_string(),
            "PINKER_STEP4_OVERRIDE".to_string(),
            "PINKER_TEST".to_string(),
            "PATH".to_string(),
        ],
        "",
        "",
        &overlay,
        "LimiteTempo.SemLimite",
        "falar(processo_saida(saida));",
    );
    let ambiente = [
        ("PINKER_STEP4_INHERITED", "herdada"),
        ("PINKER_STEP4_OVERRIDE", "antigo"),
    ];
    let (i, n, _, _) = paridade(
        &dir,
        "ambiente",
        &codigo,
        &ambiente,
        None,
        Duration::from_secs(5),
    );
    let texto = exigir_sucesso_paritario("ambiente", &i, &n);
    for esperado in [
        "ENV PINKER_STEP4_INHERITED 7 68657264616461",
        "ENV PINKER_STEP4_OVERRIDE 4 6e6f766f",
        "ENV PINKER_TEST 5 613d623d63",
        "ENV PATH 28 2f7573722f6c6f63616c2f62696e3a2f7573722f62696e3a2f62696e",
    ] {
        assert!(texto.contains(esperado), "{esperado}: {texto}");
    }

    for (indice, chave) in ["", "NOME=INVALIDO"].into_iter().enumerate() {
        let mut invalido = BTreeMap::new();
        invalido.insert(chave.to_string(), "valor".to_string());
        let codigo = fonte(
            fixture,
            &["small".to_string()],
            "",
            "",
            &invalido,
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        let nome = format!("ambiente-invalido-{indice}");
        let (i, n, _, _) = paridade(&dir, &nome, &codigo, &[], None, Duration::from_secs(5));
        for output in [i, n] {
            assert_eq!(output.status.code(), Some(0));
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .starts_with("ERRO\n"));
        }
    }

    let falso = dir.path().join("true");
    fs::write(&falso, "#!/bin/sh\nprintf 'FAKE_TRUE\\n'\nexit 73\n").unwrap();
    fs::set_permissions(&falso, fs::Permissions::from_mode(0o755)).unwrap();
    let ambient_path = format!("{}:/usr/local/bin:/usr/bin:/bin", dir.path().display());
    let corpo = "falar(processo_codigo(saida)); falar(processo_saida(saida));";
    let codigo = fonte(
        "true",
        &[],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        corpo,
    );
    let (i, n, _, _) = paridade(
        &dir,
        "path-default",
        &codigo,
        &[("PATH", ambient_path.as_str())],
        None,
        Duration::from_secs(5),
    );
    assert_eq!(
        exigir_sucesso_paritario("path-default", &i, &n),
        "OK\n0\n\n"
    );

    let mut path_overlay = BTreeMap::new();
    path_overlay.insert("PATH".into(), dir.path().display().to_string());
    let codigo = fonte(
        "true",
        &[],
        "",
        "",
        &path_overlay,
        "LimiteTempo.SemLimite",
        corpo,
    );
    let (i, n, _, _) = paridade(
        &dir,
        "path-overlay",
        &codigo,
        &[("PATH", ambient_path.as_str())],
        None,
        Duration::from_secs(5),
    );
    assert_eq!(
        exigir_sucesso_paritario("path-overlay", &i, &n),
        "OK\n73\nFAKE_TRUE\n\n"
    );

    let sem_permissao = dir.path().join("sem-permissao");
    fs::write(&sem_permissao, "#!/bin/false\n").unwrap();
    fs::set_permissions(&sem_permissao, fs::Permissions::from_mode(0o644)).unwrap();
    for (indice, programa) in [
        "".to_string(),
        "/executavel/definitivamente/ausente".to_string(),
        sem_permissao.display().to_string(),
    ]
    .into_iter()
    .enumerate()
    {
        let codigo = fonte(
            &programa,
            &[],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        let nome = format!("spawn-invalido-{indice}");
        let (i, n, _, _) = paridade(&dir, &nome, &codigo, &[], None, Duration::from_secs(5));
        for output in [i, n] {
            assert_eq!(output.status.code(), Some(0));
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .starts_with("ERRO\n"));
            assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        }
    }
}

#[test]
fn paridade_nativa_erros_utf8_e_timeouts_inclusive_hr5() {
    let dir = NativeArtifactDir::create().expect("sandbox Step 4 negativo");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");

    for modo in ["abnormal", "invalid-stdout", "invalid-stderr"] {
        let codigo = fonte(
            fixture,
            &[modo.to_string()],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(\"OK_INESPERADO\");",
        );
        let (i, n, _, _) = paridade(&dir, modo, &codigo, &[], None, Duration::from_secs(5));
        let texto_i = String::from_utf8(i.stdout).unwrap();
        let texto_n = String::from_utf8(n.stdout).unwrap();
        assert!(texto_i.starts_with("ERRO\n"), "{modo}: {texto_i}");
        assert!(texto_n.starts_with("ERRO\n"), "{modo}: {texto_n}");
        if modo == "abnormal" {
            assert!(texto_i.contains("sem código normal"));
            assert!(texto_n.contains("sem código normal"));
        } else {
            assert!(texto_i.contains("UTF-8 válido"));
            assert!(texto_n.contains("UTF-8 válido"));
            assert!(!texto_n.contains('�'));
        }
    }

    for (nome, modo, janela, limite) in [
        ("timeout-zero", "sleep", "2000", "LimiteTempo.Ate(0)"),
        ("timeout-simples", "sleep", "2000", "LimiteTempo.Ate(150)"),
        (
            "timeout-continuous-stdout",
            "continuous-stdout",
            "2000",
            "LimiteTempo.Ate(150)",
        ),
        (
            "timeout-continuous-both",
            "continuous-both",
            "2000",
            "LimiteTempo.Ate(150)",
        ),
    ] {
        let codigo = fonte(
            fixture,
            &[modo.to_string(), janela.to_string()],
            "",
            "",
            &BTreeMap::new(),
            limite,
            "falar(\"OK_INESPERADO\");",
        );
        let (i, n, tempo_i, tempo_n) =
            paridade(&dir, nome, &codigo, &[], None, Duration::from_secs(4));
        for texto in [
            String::from_utf8(i.stdout).unwrap(),
            String::from_utf8(n.stdout).unwrap(),
        ] {
            assert!(texto.starts_with("ERRO\n"), "{nome}: {texto}");
            assert!(
                texto.contains("limite de tempo excedido"),
                "{nome}: {texto}"
            );
        }
        assert!(tempo_i < Duration::from_millis(1200), "{nome}: {tempo_i:?}");
        assert!(tempo_n < Duration::from_millis(1200), "{nome}: {tempo_n:?}");
    }
}

#[test]
fn hr6_ate_zero_nao_executa_fixture_em_nenhum_backend() {
    let dir = NativeArtifactDir::create().expect("sandbox HR6");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");

    for (ponta, nativo) in [("interpretador", false), ("nativo", true)] {
        let contador_zero = dir.path().join(format!("contador-zero-{ponta}.txt"));
        let codigo_zero = fonte(
            fixture,
            &["counter".to_string(), contador_zero.display().to_string()],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(0)",
            "falar(\"OK_INESPERADO\");",
        );
        let programa_zero = compilar(&dir, &format!("hr6-zero-{ponta}"), &codigo_zero);
        let (output_zero, _) = executar(
            &programa_zero,
            nativo,
            &[],
            None,
            Duration::from_secs(5),
            &format!("hr6-zero-{ponta}"),
        );
        assert_eq!(output_zero.status.code(), Some(0));
        let texto_zero = String::from_utf8(output_zero.stdout).unwrap();
        assert!(texto_zero.starts_with("ERRO\n"), "{ponta}: {texto_zero}");
        assert!(
            texto_zero.contains("limite de tempo excedido"),
            "{ponta}: {texto_zero}"
        );
        assert!(
            !contador_zero.exists(),
            "{ponta}: Ate(0) criou o marcador; o filho chegou a executar"
        );

        let contador_controle = dir.path().join(format!("contador-controle-{ponta}.txt"));
        let codigo_controle = fonte(
            fixture,
            &[
                "counter".to_string(),
                contador_controle.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(processo_codigo(saida));",
        );
        let programa_controle = compilar(&dir, &format!("hr6-controle-{ponta}"), &codigo_controle);
        let (output_controle, _) = executar(
            &programa_controle,
            nativo,
            &[],
            None,
            Duration::from_secs(5),
            &format!("hr6-controle-{ponta}"),
        );
        assert_eq!(output_controle.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output_controle.stdout).unwrap(),
            "OK\n7\n"
        );
        assert_eq!(
            fs::read_to_string(&contador_controle).unwrap(),
            "1",
            "{ponta}: controle positivo não executou a fixture"
        );
    }
}

#[test]
fn nativo_faz_um_spawn_accessors_nao_reexecutam_e_timeout_reapeia() {
    let dir = NativeArtifactDir::create().expect("sandbox Step 4 identidade");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");
    for ponta in ["interpretado", "nativo"] {
        let contador = dir.path().join(format!("contador-{ponta}.txt"));
        let pidfile = dir.path().join(format!("contador-{ponta}.pid"));
        let codigo = fonte(
            fixture,
            &[
                "counter".to_string(),
                contador.display().to_string(),
                pidfile.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(processo_codigo(saida)); falar(processo_saida(saida)); falar(processo_erro(saida)); falar(processo_codigo(saida)); falar(processo_saida(saida)); falar(processo_erro(saida));",
        );
        let programa = compilar(&dir, &format!("single-{ponta}"), &codigo);
        let (output, _) = executar(
            &programa,
            ponta == "nativo",
            &[],
            None,
            Duration::from_secs(5),
            &format!("single-{ponta}"),
        );
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::read_to_string(&contador).unwrap(), "1");
        let texto = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            texto,
            "OK\n7\ncontador-stdout\ncontador-stderr\n7\ncontador-stdout\ncontador-stderr\n"
        );
        #[cfg(target_os = "linux")]
        assert!(!Path::new(&format!(
            "/proc/{}",
            fs::read_to_string(&pidfile).unwrap().trim()
        ))
        .exists());
    }

    for ponta in ["interpretado", "nativo"] {
        let pidfile = dir.path().join(format!("timeout-{ponta}.pid"));
        let codigo = fonte(
            fixture,
            &[
                "sleep-pid".to_string(),
                "2000".to_string(),
                pidfile.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(150)",
            "falar(\"OK_INESPERADO\");",
        );
        let programa = compilar(&dir, &format!("reap-{ponta}"), &codigo);
        let (output, elapsed) = executar(
            &programa,
            ponta == "nativo",
            &[],
            None,
            Duration::from_secs(4),
            &format!("reap-{ponta}"),
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("ERRO\n"));
        assert!(elapsed < Duration::from_millis(1200));
        #[cfg(target_os = "linux")]
        assert!(!Path::new(&format!(
            "/proc/{}",
            fs::read_to_string(&pidfile).unwrap().trim()
        ))
        .exists());
    }

    #[cfg(target_os = "linux")]
    for ponta in ["interpretado", "nativo"] {
        let pidfile = dir.path().join(format!("descendente-{ponta}.pid"));
        let codigo = fonte(
            fixture,
            &[
                "descendant".to_string(),
                "2000".to_string(),
                pidfile.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(150)",
            "falar(\"OK_INESPERADO\");",
        );
        let programa = compilar(&dir, &format!("descendente-{ponta}"), &codigo);
        let (output, elapsed) = executar(
            &programa,
            ponta == "nativo",
            &[],
            None,
            Duration::from_secs(4),
            &format!("descendente-{ponta}"),
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("ERRO\n"));
        assert!(elapsed < Duration::from_millis(1200));
        let pid = fs::read_to_string(&pidfile).unwrap().trim().to_string();
        assert!(!pid.is_empty(), "fixture descendente não atingida");
        // O runtime mata somente o filho direto. O watchdog ControlledCommand
        // encerra o grupo depois que a ponta testada retorna, então o estado do
        // descendente já pode ter mudado antes desta observação externa.
        if Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", &pid])
                .status();
        }
    }
}

// @pinker-nav:end evidencia.processos.parte-d-native-step-4
