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
// @pinker-nav:summary Executa a mesma superfície estruturada no interpretador e em ELF ligado ao runtime nativo real, comparando argv sem shell, stdin+EOF, stdout/stderr separados e grandes, ambiente/cwd/PATH, status normal e não-zero, falhas de spawn, término anormal, UTF-8 estrito, Ate(0) sem spawn nem efeito externo, timeout simples/descendente/output contínuo, captura SemLimite, single-spawn e accessors sem reexecução sob watchdog externo; fecha ainda composição genérica com tentar/propagar? e um workflow read-only real com git, múltiplos argv e consumo de status/stdout/stderr.

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
                "    lista.verso_anexar(argumentos, {});",
                literal(argumento)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let overlays = ambiente
        .iter()
        .map(|(chave, valor)| {
            format!(
                "    mapa.verso_verso_definir(ambiente, {}, {});",
                literal(chave),
                literal(valor)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"pacote main; trazer lista; trazer mapa; trazer processo; trazer texto;
apelido Res = Resultado<SaidaProcesso, verso>;

carinho principal() -> bombom {{
    nova muda argumentos: lista<verso> = lista.verso_criar();
{anexos}
    nova muda ambiente: mapa<verso,verso> = mapa.verso_verso_criar();
{overlays}
    nova resultado: Res = processo.executar_estruturado(
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

const CONTRATO_PIPELINE: (&str, u64, u64) = ("Pipeline", 4 * 1024 * 1024 * 1024, 60);

fn executar(
    programa: &ProgramaNativo,
    nativo: bool,
    ambiente: &[(&str, &str)],
    cwd: Option<&Path>,
    timeout: Duration,
    caso: &str,
) -> (Output, Duration) {
    let mut comando = if nativo {
        // O binário gerado tem nome escolhido por este teste, então a
        // inferência por identidade do executável o classificaria como
        // executável arbitrário. A intenção é declarada: é o mesmo programa
        // Pinker que o lado interpretado roda por `pink --run`, e precisa da
        // mesma classe de recurso.
        let mut comando = Command::new(&programa.binario);
        comando.pinker_pipeline_guest();
        comando
    } else {
        let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
        comando.arg("--run").arg(&programa.fonte);
        comando
    };
    // Oráculo da paridade de recurso: os dois lados executam o MESMO programa
    // Pinker e por isso precisam da mesma classe. O lado interpretado a obtém
    // pela identidade de `pink`; o lado nativo, de nome arbitrário, só a obtém
    // por intenção declarada.
    assert_eq!(
        comando.resource_contract_for_test(),
        CONTRATO_PIPELINE,
        "lado {} de {caso} sem a intenção de recurso do pipeline",
        if nativo { "nativo" } else { "interpretado" }
    );
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
        "falar(processo.saida(saida));",
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
        "falar(processo.saida(saida));",
    );
    let (i, n, _, _) = paridade(&dir, "stdin", &codigo, &[], None, Duration::from_secs(5));
    let texto = exigir_sucesso_paritario("stdin", &i, &n);
    assert!(texto.contains(&format!(
        "STDIN {} {:016x}\nEOF",
        entrada.len(),
        fnv1a64(entrada.as_bytes())
    )));

    let codigo = fonte(
        fixture,
        &["stdin".to_string()],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.SemLimite",
        "falar(processo.saida(saida));",
    );
    let (i, n, _, _) = paridade(
        &dir,
        "stdin-vazio-eof",
        &codigo,
        &[],
        None,
        Duration::from_secs(5),
    );
    assert_eq!(
        exigir_sucesso_paritario("stdin-vazio-eof", &i, &n),
        "OK\nSTDIN 0 cbf29ce484222325\nEOF\n\n"
    );

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
            "falar(processo.codigo(saida)); falar(processo.saida(saida)); falar(processo.erro(saida));",
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
        "falar(processo.codigo(saida)); falar(processo.saida(saida));",
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
        "falar(texto.tamanho(processo.saida(saida))); falar(texto.tamanho(processo.erro(saida)));",
    );
    let (i, n, _, _) = paridade(&dir, "grande", &codigo, &[], None, Duration::from_secs(8));
    assert_eq!(
        exigir_sucesso_paritario("grande", &i, &n),
        "OK\n2097152\n2097152\n"
    );

    for (nome, modo, esperado_stdout, esperado_stderr) in [
        ("stdout-grande-isolado", "large-stdout-only", 2_097_152, 0),
        ("stderr-grande-isolado", "large-stderr-only", 0, 2_097_152),
    ] {
        let codigo = fonte(
            fixture,
            &[modo.to_string()],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.SemLimite",
            "falar(texto.tamanho(processo.saida(saida))); falar(texto.tamanho(processo.erro(saida)));",
        );
        let (i, n, _, _) = paridade(&dir, nome, &codigo, &[], None, Duration::from_secs(8));
        assert_eq!(
            exigir_sucesso_paritario(nome, &i, &n),
            format!("OK\n{esperado_stdout}\n{esperado_stderr}\n")
        );
    }

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
            "falar(processo.codigo(saida));",
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
        "falar(processo.saida(saida));",
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
        "falar(processo.saida(saida));",
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
    let corpo = "falar(processo.codigo(saida)); falar(processo.saida(saida));";
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

    // Evidência de timeout: erro do contrato MAIS ausência do marker de
    // conclusão natural. O relógio de parede do programa inteiro não serve como
    // autoridade semântica — ele inclui exec, carga do ELF, inicialização do
    // runtime e o pipeline Pinker antes de a operação processual sequer começar.
    //
    //   WHOLE_PROGRAM_WALL_CLOCK != PROCESS_TIMEOUT_SEMANTIC_CLOCK
    //
    // A contenção temporal continua existindo no watchdog de 4 s do harness, que
    // é proteção catastrófica e não SLA de `LimiteTempo`.
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
        let marker_i = dir.path().join(format!("{nome}-interpretado.natural"));
        let marker_n = dir.path().join(format!("{nome}-nativo.natural"));
        for (sufixo, marker) in [("interpretado", &marker_i), ("nativo", &marker_n)] {
            let codigo = fonte(
                fixture,
                &[
                    modo.to_string(),
                    janela.to_string(),
                    marker.display().to_string(),
                ],
                "",
                "",
                &BTreeMap::new(),
                limite,
                "falar(\"OK_INESPERADO\");",
            );
            let programa = compilar(&dir, &format!("{nome}-{sufixo}"), &codigo);
            let (output, _) = executar(
                &programa,
                sufixo == "nativo",
                &[],
                None,
                Duration::from_secs(4),
                &format!("{nome}-{sufixo}"),
            );
            let texto = String::from_utf8(output.stdout).unwrap();
            assert!(texto.starts_with("ERRO\n"), "{nome}/{sufixo}: {texto}");
            assert!(
                texto.contains("limite de tempo excedido"),
                "{nome}/{sufixo}: {texto}"
            );
            assert!(
                !marker.exists(),
                "{nome}/{sufixo}: a fixture alcançou a conclusão natural apesar do timeout"
            );
        }
    }

    // Controle positivo do oráculo. Sem ele, "marker ausente" poderia
    // significar apenas que a fixture nunca escreveria marker nenhum.
    //
    //   MARKER_ORACLE_REQUIRES_POSITIVE_CONTROL
    for (nome, modo) in [
        ("natural-sleep", "sleep"),
        ("natural-continuous", "continuous-stdout"),
    ] {
        for sufixo in ["interpretado", "nativo"] {
            let marker = dir.path().join(format!("{nome}-{sufixo}.natural"));
            let codigo = fonte(
                fixture,
                &[
                    modo.to_string(),
                    "100".to_string(),
                    marker.display().to_string(),
                ],
                "",
                "",
                &BTreeMap::new(),
                // Limite folgado: a fixture conclui naturalmente.
                "LimiteTempo.Ate(3000)",
                "falar(\"CONCLUIU\");",
            );
            let programa = compilar(&dir, &format!("{nome}-{sufixo}"), &codigo);
            let (output, _) = executar(
                &programa,
                sufixo == "nativo",
                &[],
                None,
                Duration::from_secs(8),
                &format!("{nome}-{sufixo}"),
            );
            let texto = String::from_utf8(output.stdout).unwrap();
            assert!(
                !texto.starts_with("ERRO\n"),
                "{nome}/{sufixo}: deveria concluir sem timeout: {texto}"
            );
            assert!(
                texto.contains("CONCLUIU"),
                "{nome}/{sufixo}: a fixture deveria ter concluído: {texto}"
            );
            assert!(
                marker.exists(),
                "{nome}/{sufixo}: o marker precisa existir quando a fixture conclui, \
                 senão a ausência dele não prova nada"
            );
        }
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
            "falar(processo.codigo(saida));",
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
            "falar(processo.codigo(saida)); falar(processo.saida(saida)); falar(processo.erro(saida)); falar(processo.codigo(saida)); falar(processo.saida(saida)); falar(processo.erro(saida));",
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
        let marker = dir.path().join(format!("timeout-{ponta}.natural"));
        let codigo = fonte(
            fixture,
            &[
                "sleep-pid".to_string(),
                "2000".to_string(),
                pidfile.display().to_string(),
                marker.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(150)",
            "falar(\"OK_INESPERADO\");",
        );
        let programa = compilar(&dir, &format!("reap-{ponta}"), &codigo);
        let (output, _elapsed) = executar(
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
        // (A) o filho não chegou à conclusão natural...
        assert!(
            !marker.exists(),
            "reap-{ponta}: a fixture concluiu naturalmente apesar do timeout"
        );
        // (B) ...e o filho DIRETO foi terminado e reapado. São duas
        // propriedades distintas: o marker prova (A), o /proc prova (B).
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
        // Marker do filho DIRETO, cuja janela natural é de 5 s neste modo.
        let marker = dir.path().join(format!("descendente-{ponta}.natural"));
        let codigo = fonte(
            fixture,
            &[
                "descendant".to_string(),
                "2000".to_string(),
                pidfile.display().to_string(),
                marker.display().to_string(),
            ],
            "",
            "",
            &BTreeMap::new(),
            "LimiteTempo.Ate(150)",
            "falar(\"OK_INESPERADO\");",
        );
        let programa = compilar(&dir, &format!("descendente-{ponta}"), &codigo);
        let (output, _elapsed) = executar(
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
        // O filho DIRETO não alcançou o fim da sua janela natural de 5 s. A
        // política continua sendo `process_tree_scope = direct child`: nada aqui
        // exige que o descendente tenha morrido.
        assert!(
            !marker.exists(),
            "descendente-{ponta}: o filho direto concluiu naturalmente apesar do timeout"
        );
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

#[test]
fn resultado_estruturado_compoe_com_tentar_e_propagar_sem_caso_especial() {
    let dir = NativeArtifactDir::create().expect("sandbox de composição Parte D");
    let fixture = env!("CARGO_BIN_EXE_pinker_part_d_filho");
    let ausente = dir.path().join("executavel-ausente");
    let codigo = format!(
        r#"pacote main; trazer lista; trazer mapa; trazer processo;
apelido Res = Resultado<SaidaProcesso, verso>;

carinho sucesso() -> Res {{
    nova muda argumentos: lista<verso> = lista.verso_criar();
    lista.verso_anexar(argumentos, "small");
    nova ambiente: mapa<verso,verso> = mapa.verso_verso_criar();
    propagar? processo.executar_estruturado(
        {fixture}, argumentos, "", "", ambiente, LimiteTempo.SemLimite
    ) como Res.Ok(saida);
    mimo Res.Ok(saida);
}}

carinho falha() -> Res {{
    nova argumentos: lista<verso> = lista.verso_criar();
    nova ambiente: mapa<verso,verso> = mapa.verso_verso_criar();
    propagar? processo.executar_estruturado(
        {ausente}, argumentos, "", "", ambiente, LimiteTempo.SemLimite
    ) como Res.Ok(saida);
    mimo Res.Ok(saida);
}}

carinho principal() -> bombom {{
    tentar sucesso() {{
        sucesso Res.Ok(saida) {{
            falar("TENTAR_OK");
            falar(processo.codigo(saida));
            falar(processo.saida(saida));
            falar(processo.erro(saida));
        }}
        falha Res.Erro(causa) {{ falar("SUCESSO_INESPERADAMENTE_FALHOU"); }}
    }}
    tentar falha() {{
        sucesso Res.Ok(saida) {{ falar("FALHA_INESPERADAMENTE_OK"); }}
        falha Res.Erro(causa) {{ falar("PROPAGAR_ERRO"); }}
    }}
    mimo 0;
}}
"#,
        fixture = literal(fixture),
        ausente = literal(&ausente.display().to_string()),
    );

    let (interpretado, nativo, _, _) = paridade(
        &dir,
        "resultado-composicao",
        &codigo,
        &[],
        None,
        Duration::from_secs(5),
    );
    assert_eq!(
        exigir_sucesso_paritario("resultado-composicao", &interpretado, &nativo),
        "TENTAR_OK\n0\nstdout-small\nstderr-small\nPROPAGAR_ERRO\n"
    );

    let parser = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/parser.rs"))
        .expect("autoridade do desugaring legível");
    let inicio = parser
        .find("// @pinker-nav:start parser.resultado.tentar-propagar")
        .expect("início do desugaring");
    let fim = parser
        .find("// @pinker-nav:end parser.resultado.tentar-propagar")
        .expect("fim do desugaring");
    let desugaring = &parser[inicio..fim];
    assert!(desugaring.contains("parse_tentar_desugared"));
    assert!(desugaring.contains("parse_propagar_desugared"));
    assert!(!desugaring.contains("executar_processo_estruturado"));
}

#[test]
fn workflow_real_git_observa_status_stdout_stderr_com_argv_estrutural() {
    let dir = NativeArtifactDir::create().expect("sandbox do workflow real Parte D");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("repo canônico");
    let repo_texto = repo.display().to_string();

    let codigo = fonte(
        "git",
        &[
            "-C".to_string(),
            repo_texto.clone(),
            "rev-parse".to_string(),
            "--show-toplevel".to_string(),
        ],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.Ate(5000)",
        "falar(processo.codigo(saida)); falar(processo.saida(saida)); falar(processo.erro(saida));",
    );
    let (interpretado, nativo, _, _) = paridade(
        &dir,
        "workflow-git-ok",
        &codigo,
        &[],
        None,
        Duration::from_secs(8),
    );
    assert_eq!(
        exigir_sucesso_paritario("workflow-git-ok", &interpretado, &nativo),
        format!("OK\n0\n{repo_texto}\n\n\n")
    );

    let codigo_nao_zero = fonte(
        "git",
        &[
            "-C".to_string(),
            repo_texto,
            "rev-parse".to_string(),
            "--verify".to_string(),
            "refs/heads/__parte_d_ref_ausente__".to_string(),
        ],
        "",
        "",
        &BTreeMap::new(),
        "LimiteTempo.Ate(5000)",
        "falar(processo.codigo(saida)); falar(processo.saida(saida)); falar(processo.erro(saida));",
    );
    let (interpretado, nativo, _, _) = paridade(
        &dir,
        "workflow-git-nao-zero",
        &codigo_nao_zero,
        &[],
        None,
        Duration::from_secs(8),
    );
    let texto = exigir_sucesso_paritario("workflow-git-nao-zero", &interpretado, &nativo);
    assert!(texto.starts_with("OK\n128\n\n"), "{texto}");
    assert!(texto.contains("Needed a single revision"), "{texto}");
}

#[test]
fn superficies_historicas_preservam_observaveis_em_interpretador_e_nativo() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = NativeArtifactDir::create().expect("sandbox de compatibilidade histórica");
    let script = |nome: &str, corpo: &str| {
        let caminho = dir.path().join(nome);
        fs::write(&caminho, format!("#!/bin/sh\n{corpo}\n")).expect("gravar fixture legada");
        fs::set_permissions(&caminho, fs::Permissions::from_mode(0o755))
            .expect("tornar fixture executável");
        caminho
    };
    let status = script("status.sh", "exit 7");
    let stdout = script("stdout.sh", "printf 'legacy-out'");
    let stderr = script("stderr.sh", "printf 'legacy-err' >&2");
    let stdin = script(
        "stdin.sh",
        "IFS= read -r linha; test \"$linha\" = dado; exit $((9 + $?))",
    );
    let produtor = script("produtor.sh", "printf 'pipe'");
    let consumidor = script(
        "consumidor.sh",
        "conteudo=$(cat); test \"$conteudo\" = pipe; exit $((11 + $?))",
    );
    let resultado = script("resultado.sh", "exit 5");
    let codigo = format!(
        r#"pacote main; trazer processo.capturar_stderr; trazer processo.capturar_stdout; trazer processo.executar; trazer processo.executar_com_entrada; trazer processo.executar_resultado; trazer processo.pipeline_minimo;
apelido Res = Resultado<bombom, verso>;

carinho principal() -> bombom {{
    falar(executar({status}));
    falar(capturar_stdout({stdout}));
    falar(capturar_stderr({stderr}));
    falar(executar_com_entrada({stdin}, "dado\n"));
    falar(pipeline_minimo({produtor}, {consumidor}));
    tentar executar_resultado({resultado}) {{
        sucesso Res.Ok(codigo) {{ falar(codigo); }}
        falha Res.Erro(causa) {{ falar("ERRO_INESPERADO"); }}
    }}
    mimo 0;
}}
"#,
        status = literal(&status.display().to_string()),
        stdout = literal(&stdout.display().to_string()),
        stderr = literal(&stderr.display().to_string()),
        stdin = literal(&stdin.display().to_string()),
        produtor = literal(&produtor.display().to_string()),
        consumidor = literal(&consumidor.display().to_string()),
        resultado = literal(&resultado.display().to_string()),
    );
    let (interpretado, nativo, _, _) = paridade(
        &dir,
        "compatibilidade-historica",
        &codigo,
        &[],
        None,
        Duration::from_secs(10),
    );
    assert_eq!(
        exigir_sucesso_paritario("compatibilidade-historica", &interpretado, &nativo),
        "7\nlegacy-out\nlegacy-err\n9\n11\n5\n"
    );
}

// @pinker-nav:end evidencia.processos.parte-d-native-step-4
