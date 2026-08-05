//! Regressões determinísticas do runner de estabilidade.
//!
//! Cobrem duas classes de falso verde observadas na PR 422:
//!
//! 1. `runs` inválido chegava ao laço sem validação. `seq` atuava como
//!    validador de fato: `0` e `-1` produziam zero iteração, zero falha e
//!    saída zero, ou seja, sucesso aparente sem nenhum teste executado.
//! 2. O código de saída do lote era a própria contagem de falhas. O shell
//!    trunca em módulo 256, de modo que 256 falhas produziriam zero.
//!
//! Também cobrem o veredito por iteração: código zero do harness é
//! necessário, nunca suficiente.
//!
//! Nenhuma dependência externa. O harness é substituído por um binário
//! falso controlado, e cada caso roda em uma raiz própria para não tocar a
//! evidência real do repositório.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

fn caminho_do_runner() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/pinker-flake-runner.sh")
}

/// Cria uma raiz isolada contendo apenas `scripts/pinker-flake-runner.sh`.
///
/// O runner deriva a raiz do repositório a partir da própria localização, de
/// modo que copiá-lo para um diretório temporário mantém toda a evidência do
/// caso dentro desse diretório.
fn raiz_isolada(caso: &str) -> PathBuf {
    let unico = format!(
        "pinker-flake-runner-{}-{}-{}",
        caso,
        std::process::id(),
        SEQUENCIA.fetch_add(1, Ordering::Relaxed)
    );
    let raiz = std::env::temp_dir().join(unico);
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(raiz.join("scripts")).expect("criar raiz isolada");
    fs::copy(
        caminho_do_runner(),
        raiz.join("scripts/pinker-flake-runner.sh"),
    )
    .expect("copiar runner");
    let destino = raiz.join("scripts/pinker-flake-runner.sh");
    let mut permissoes = fs::metadata(&destino).expect("metadados").permissions();
    permissoes.set_mode(0o755);
    fs::set_permissions(&destino, permissoes).expect("permissões do runner");
    raiz
}

/// Binário falso que reproduz um desfecho exato do harness.
fn binario_falso(raiz: &Path, saida_padrao: &str, codigo: i32) -> PathBuf {
    binario_falso_com_atraso(raiz, saida_padrao, codigo, 0)
}

fn binario_falso_com_atraso(
    raiz: &Path,
    saida_padrao: &str,
    codigo: i32,
    atraso_segundos: u64,
) -> PathBuf {
    let caminho = raiz.join("harness-falso.sh");
    let mut arquivo = fs::File::create(&caminho).expect("criar binário falso");
    writeln!(arquivo, "#!/usr/bin/env bash").expect("escrever");
    if atraso_segundos > 0 {
        writeln!(arquivo, "sleep {atraso_segundos}").expect("escrever");
    }
    for linha in saida_padrao.lines() {
        writeln!(arquivo, "printf '%s\\n' {}", aspas_simples(linha)).expect("escrever");
    }
    writeln!(arquivo, "exit {codigo}").expect("escrever");
    drop(arquivo);
    let mut permissoes = fs::metadata(&caminho).expect("metadados").permissions();
    permissoes.set_mode(0o755);
    fs::set_permissions(&caminho, permissoes).expect("permissões do binário falso");
    caminho
}

fn aspas_simples(valor: &str) -> String {
    format!("'{}'", valor.replace('\'', "'\\''"))
}

struct Execucao {
    codigo: i32,
    saida_padrao: String,
    saida_erro: String,
    raiz: PathBuf,
}

impl Execucao {
    fn evidencia(&self) -> PathBuf {
        self.raiz.join("target/pinker-flake-evidence")
    }

    fn resumo(&self, modo: &str) -> Option<String> {
        fs::read_to_string(self.evidencia().join(format!("SUMMARY-{modo}.txt"))).ok()
    }

    fn diretorios_preservados(&self) -> Vec<PathBuf> {
        let Ok(entradas) = fs::read_dir(self.evidencia()) else {
            return Vec::new();
        };
        let mut encontrados: Vec<PathBuf> = entradas
            .flatten()
            .filter(|entrada| entrada.path().is_dir())
            .map(|entrada| entrada.path())
            .collect();
        encontrados.sort();
        encontrados
    }
}

fn executar(raiz: &Path, argumentos: &[&str], ambiente: &[(&str, &str)]) -> Execucao {
    let mut comando = Command::new(raiz.join("scripts/pinker-flake-runner.sh"));
    comando.args(argumentos);
    comando.stdout(Stdio::piped()).stderr(Stdio::piped());
    comando.env_remove("PINKER_FLAKE_RUN_TIMEOUT_SECONDS");
    comando.env_remove("PINKER_FLAKE_TEST_BINARY");
    for (chave, valor) in ambiente {
        comando.env(chave, valor);
    }
    let Output {
        status,
        stdout,
        stderr,
    } = comando.output().expect("executar runner");
    Execucao {
        codigo: status.code().unwrap_or(-1),
        saida_padrao: String::from_utf8_lossy(&stdout).into_owned(),
        saida_erro: String::from_utf8_lossy(&stderr).into_owned(),
        raiz: raiz.to_path_buf(),
    }
}

const RESUMO_COM_TESTE: &str =
    "running 1 test\ntest exemplo ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s";
const RESUMO_ZERO_TESTES: &str =
    "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s";
const SEM_RESUMO: &str = "compilando\nnenhuma linha de resumo reconhecivel aqui";
const RESUMO_COM_FALHA: &str =
    "running 2 tests\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";

const EXIT_USO: i32 = 2;
const EXIT_FALHAS: i32 = 1;
const EXIT_INTERROMPIDO: i32 = 130;

// ---------------------------------------------------------------------------
// Validação do argumento `runs`.
//
// A validação precede a criação de evidência, a remoção de resumo anterior e
// qualquer início de teste.
// ---------------------------------------------------------------------------

fn caso_de_uso_invalido(caso: &str, argumentos: &[&str]) {
    let raiz = raiz_isolada(caso);
    let execucao = executar(&raiz, argumentos, &[]);
    assert_eq!(
        execucao.codigo, EXIT_USO,
        "{caso}: erro de uso deve ter código fixo e distinto; stderr={}",
        execucao.saida_erro
    );
    assert!(
        !execucao.saida_padrao.contains("PASS"),
        "{caso}: erro de uso não pode imprimir PASS"
    );
    assert!(
        !execucao.saida_padrao.contains("SUMMARY"),
        "{caso}: erro de uso não pode imprimir resumo verde"
    );
    assert!(
        !execucao.evidencia().exists(),
        "{caso}: erro de uso não pode criar a raiz de evidência"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn runs_ausente_e_erro_de_uso_antes_de_qualquer_efeito() {
    caso_de_uso_invalido("runs-ausente", &["modo"]);
}

#[test]
fn runs_vazio_e_erro_de_uso() {
    caso_de_uso_invalido("runs-vazio", &["modo", ""]);
}

#[test]
fn runs_texto_e_erro_de_uso() {
    caso_de_uso_invalido("runs-abc", &["modo", "abc"]);
}

#[test]
fn runs_zero_e_erro_de_uso_e_nunca_verde() {
    caso_de_uso_invalido("runs-zero", &["modo", "0"]);
}

#[test]
fn runs_negativo_e_erro_de_uso() {
    caso_de_uso_invalido("runs-negativo", &["modo", "-1"]);
}

#[test]
fn runs_parcialmente_numerico_e_erro_de_uso() {
    caso_de_uso_invalido("runs-parcial", &["modo", "1a"]);
}

#[test]
fn runs_com_zero_a_esquerda_e_erro_de_uso() {
    // `010` seria octal em contexto aritmético do Bash: o valor aceito
    // divergiria do valor escrito.
    caso_de_uso_invalido("runs-octal", &["modo", "010"]);
}

#[test]
fn runs_grande_demais_para_o_bash_e_erro_de_uso() {
    caso_de_uso_invalido("runs-enorme", &["modo", "99999999999999999999"]);
}

#[test]
fn modo_ausente_e_erro_de_uso() {
    caso_de_uso_invalido("modo-ausente", &[]);
}

#[test]
fn erro_de_uso_nao_apaga_resumo_anterior() {
    let raiz = raiz_isolada("preserva-resumo");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    let anterior = evidencia.join("SUMMARY-modo.txt");
    fs::write(&anterior, "resumo anterior preservado\n").expect("escrever resumo anterior");

    let execucao = executar(&raiz, &["modo", "0"], &[]);
    assert_eq!(execucao.codigo, EXIT_USO);
    assert_eq!(
        fs::read_to_string(&anterior).expect("ler resumo anterior"),
        "resumo anterior preservado\n",
        "validação deve preceder a remoção de resumos anteriores"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// Validação de PINKER_FLAKE_RUN_TIMEOUT_SECONDS.
// ---------------------------------------------------------------------------

fn caso_de_timeout_invalido(caso: &str, valor: &str) {
    let raiz = raiz_isolada(caso);
    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["modo", "1"],
        &[
            ("PINKER_FLAKE_RUN_TIMEOUT_SECONDS", valor),
            ("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8")),
        ],
    );
    assert_eq!(
        execucao.codigo, EXIT_USO,
        "{caso}: timeout inválido deve ser erro de uso; stderr={}",
        execucao.saida_erro
    );
    assert!(!execucao.saida_padrao.contains("PASS"), "{caso}: sem PASS");
    assert!(
        !execucao.evidencia().exists(),
        "{caso}: sem criação de evidência"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn timeout_vazio_e_erro_de_uso() {
    caso_de_timeout_invalido("timeout-vazio", "");
}

#[test]
fn timeout_zero_e_erro_de_uso() {
    caso_de_timeout_invalido("timeout-zero", "0");
}

#[test]
fn timeout_nao_numerico_e_erro_de_uso() {
    caso_de_timeout_invalido("timeout-texto", "trinta");
}

// ---------------------------------------------------------------------------
// Veredito por iteração.
// ---------------------------------------------------------------------------

#[test]
fn harness_zero_com_teste_executado_e_pass_sem_acumular_evidencia() {
    let raiz = raiz_isolada("pass");
    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["modo", "2"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );

    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    assert_eq!(
        execucao.saida_padrao.matches("PASS").count(),
        2,
        "duas iterações válidas produzem dois PASS"
    );
    let resumo = execucao.resumo("modo").expect("resumo presente");
    assert!(resumo.contains("completed=2"), "resumo: {resumo}");
    assert!(resumo.contains("failures=0"), "resumo: {resumo}");
    assert!(resumo.contains("tests_executed=2"), "resumo: {resumo}");
    assert!(
        execucao.diretorios_preservados().is_empty(),
        "sucesso não acumula evidência: {:?}",
        execucao.diretorios_preservados()
    );
    let _ = fs::remove_dir_all(&raiz);
}

/// Prova conjunta: zero teste executado, seleção exata inexistente e resumo
/// não reconhecido nunca podem ser PASS, e todos preservam evidência.
fn caso_falha_fechada(caso: &str, saida: &str, codigo_harness: i32, motivo_esperado: &str) {
    let raiz = raiz_isolada(caso);
    let binario = binario_falso(&raiz, saida, codigo_harness);
    let execucao = executar(
        &raiz,
        &["modo", "1"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );

    assert_eq!(
        execucao.codigo, EXIT_FALHAS,
        "{caso}: lote deve terminar não zero; stderr={}",
        execucao.saida_erro
    );
    assert!(
        !execucao.saida_padrao.contains("PASS"),
        "{caso}: não pode imprimir PASS; stdout={}",
        execucao.saida_padrao
    );
    assert!(
        execucao
            .saida_padrao
            .contains(&format!("reason={motivo_esperado}")),
        "{caso}: motivo estruturado ausente; stdout={}",
        execucao.saida_padrao
    );

    let preservados = execucao.diretorios_preservados();
    assert_eq!(
        preservados.len(),
        1,
        "{caso}: exatamente um diretório de execução preservado: {preservados:?}"
    );
    let diretorio = &preservados[0];
    for exigido in [
        "stdout",
        "stderr",
        "manifest.txt",
        "processes.txt",
        "sandbox-tree.txt",
    ] {
        assert!(
            diretorio.join(exigido).exists(),
            "{caso}: evidência {exigido} ausente em {diretorio:?}"
        );
    }
    assert!(
        diretorio.join("proc").is_dir(),
        "{caso}: dados de processo ausentes"
    );

    let manifesto = fs::read_to_string(diretorio.join("manifest.txt")).expect("ler manifesto");
    assert!(
        manifesto.contains(&format!("reason={motivo_esperado}")),
        "{caso}: manifesto sem motivo; conteúdo={manifesto}"
    );

    let resumo = execucao.resumo("modo").expect("resumo presente");
    assert!(resumo.contains("failures=1"), "{caso}: resumo={resumo}");
    assert!(resumo.contains("completed=1"), "{caso}: resumo={resumo}");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn harness_zero_com_zero_testes_falha_fechada() {
    caso_falha_fechada("zero-testes", RESUMO_ZERO_TESTES, 0, "no-tests-executed");
}

#[test]
fn selecao_exata_inexistente_falha_fechada() {
    // Uma seleção exata que não casa com nada produz exatamente a saída de
    // zero teste executado, com código zero do harness.
    caso_falha_fechada(
        "selecao-inexistente",
        RESUMO_ZERO_TESTES,
        0,
        "no-tests-executed",
    );
}

#[test]
fn harness_zero_sem_resumo_falha_fechada() {
    caso_falha_fechada("sem-resumo", SEM_RESUMO, 0, "unparseable-test-summary");
}

#[test]
fn harness_com_falha_preserva_evidencia() {
    caso_falha_fechada("harness-falha", RESUMO_COM_FALHA, 101, "harness-exit-101");
}

#[test]
fn resumo_final_corresponde_as_iteracoes_concluidas() {
    let raiz = raiz_isolada("contagem");
    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["modo", "3"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );
    let resumo = execucao.resumo("modo").expect("resumo presente");
    assert!(resumo.contains("runs=3"), "resumo={resumo}");
    assert!(resumo.contains("completed=3"), "resumo={resumo}");
    assert!(resumo.contains("exit_code=0"), "resumo={resumo}");
    let _ = fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// Política de código de saída.
//
// A política é provada pela função extraída, sem produzir centenas de falhas
// reais. `exit 256` seria truncado para zero pelo shell.
// ---------------------------------------------------------------------------

fn politica_para(falhas: &str) -> String {
    let runner = caminho_do_runner();
    let script = format!(
        "PINKER_FLAKE_LIB_ONLY=1 source {}; pinker_flake_exit_code_for {}",
        runner.display(),
        falhas
    );
    let saida = Command::new("bash")
        .arg("-c")
        .arg(script)
        .output()
        .expect("executar política");
    String::from_utf8_lossy(&saida.stdout).trim().to_string()
}

#[test]
fn politica_de_saida_nunca_trunca_a_contagem_de_falhas() {
    assert_eq!(politica_para("0"), "0", "nenhuma falha é sucesso");
    assert_eq!(politica_para("1"), "1");
    assert_eq!(politica_para("255"), "1");
    assert_eq!(
        politica_para("256"),
        "1",
        "256 falhas jamais podem virar sucesso"
    );
    assert_eq!(politica_para("512"), "1");
    assert_eq!(politica_para("abc"), "2", "entrada inválida é erro de uso");
}

#[test]
fn validador_aceita_somente_inteiro_decimal_estritamente_positivo() {
    let runner = caminho_do_runner();
    let avaliar = |valor: &str| -> bool {
        let script = format!(
            "PINKER_FLAKE_LIB_ONLY=1 source {}; pinker_flake_is_positive_int {}",
            runner.display(),
            aspas_simples(valor)
        );
        Command::new("bash")
            .arg("-c")
            .arg(script)
            .status()
            .expect("executar validador")
            .success()
    };
    for aceito in ["1", "2", "42", "999999999999999999"] {
        assert!(avaliar(aceito), "deveria aceitar {aceito}");
    }
    for rejeitado in [
        "",
        "0",
        "-1",
        "+1",
        "abc",
        "1a",
        "a1",
        " 1",
        "1 ",
        "01",
        "1.5",
        "0x10",
        "1e3",
        "99999999999999999999",
    ] {
        assert!(!avaliar(rejeitado), "deveria rejeitar {rejeitado:?}");
    }
}

// ---------------------------------------------------------------------------
// Interrupção.
// ---------------------------------------------------------------------------

#[test]
fn interrupcao_preserva_evidencia_e_retorna_130() {
    let raiz = raiz_isolada("interrupcao");
    let binario = binario_falso_com_atraso(&raiz, RESUMO_COM_TESTE, 0, 30);
    let mut filho = Command::new(raiz.join("scripts/pinker-flake-runner.sh"))
        .args(["modo", "1"])
        .env("PINKER_FLAKE_TEST_BINARY", &binario)
        .env_remove("PINKER_FLAKE_RUN_TIMEOUT_SECONDS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("iniciar runner");

    let evidencia = raiz.join("target/pinker-flake-evidence");
    let limite = Instant::now() + Duration::from_secs(20);
    let mut iniciou = false;
    while Instant::now() < limite {
        if let Ok(entradas) = fs::read_dir(&evidencia) {
            if entradas.flatten().any(|entrada| {
                entrada
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".running-")
            }) {
                iniciou = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(iniciou, "iteração não chegou a iniciar");

    let pid = filho.id() as i32;
    // SIGINT no runner, exatamente como uma interrupção de terminal.
    unsafe {
        extern "C" {
            fn kill(pid: i32, sinal: i32) -> i32;
        }
        assert_eq!(kill(pid, 2), 0, "enviar SIGINT");
    }
    let status = filho.wait().expect("aguardar runner");
    assert_eq!(
        status.code(),
        Some(EXIT_INTERROMPIDO),
        "interrupção deve retornar 130"
    );

    let preservados: Vec<PathBuf> = fs::read_dir(&evidencia)
        .expect("ler evidência")
        .flatten()
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho.is_dir()
                && caminho
                    .file_name()
                    .map(|nome| nome.to_string_lossy().starts_with("INTERRUPTED-"))
                    .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        preservados.len(),
        1,
        "interrupção preserva exatamente um diretório: {preservados:?}"
    );
    let manifesto =
        fs::read_to_string(preservados[0].join("manifest.txt")).expect("manifesto da interrupção");
    assert!(
        manifesto.contains("reason=interrupted"),
        "manifesto={manifesto}"
    );
    assert!(
        manifesto.contains(&format!("exit_code={EXIT_INTERROMPIDO}")),
        "manifesto={manifesto}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn evidencia_permanece_na_raiz_persistente_do_repositorio() {
    // A raiz de evidência é derivada da localização do runner, e não de /tmp,
    // de modo que o relatório sobrevive ao processo que o produziu.
    let conteudo = fs::read_to_string(caminho_do_runner()).expect("ler runner");
    assert!(
        conteudo.contains(r#"evidence_root="$repo_root/target/pinker-flake-evidence""#),
        "a evidência precisa continuar ancorada na raiz do repositório"
    );
    let _ = SystemTime::now().duration_since(UNIX_EPOCH);
}
