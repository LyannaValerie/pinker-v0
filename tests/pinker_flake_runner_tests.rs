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
    // Cada binário falso recebe caminho próprio. Reescrever um arquivo ainda
    // em execução produz `ETXTBSY`, e um caso que mantém campanha proprietária
    // viva precisa poder criar um segundo binário sem tocar o primeiro.
    let caminho = raiz.join(format!(
        "harness-falso-{}.sh",
        SEQUENCIA.fetch_add(1, Ordering::Relaxed)
    ));
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

    /// Diretórios de lote sob `batches/`.
    ///
    /// Cada campanha possui o seu, e a autoridade do resultado vive ali. Os
    /// caminhos legados por `mode` são projeção do último lote concluído.
    fn lotes(&self) -> Vec<PathBuf> {
        diretorios_de(&self.evidencia().join("batches"))
    }

    fn lote_unico(&self) -> PathBuf {
        let lotes = self.lotes();
        assert_eq!(lotes.len(), 1, "esperado exatamente um lote: {lotes:?}");
        lotes.into_iter().next().expect("lote")
    }

    /// Evidências de iteração preservadas dentro do lote.
    fn diretorios_preservados(&self) -> Vec<PathBuf> {
        let lotes = self.lotes();
        let Some(lote) = lotes.last() else {
            return Vec::new();
        };
        diretorios_de(lote)
    }

    fn lock(&self) -> PathBuf {
        self.evidencia().join(".lock")
    }
}

/// Lote que possui uma iteração em curso, se houver.
///
/// Um checkout pode conter lotes já concluídos; apenas o lote em execução
/// carrega um diretório `.running-`.
fn lote_em_execucao(evidencia: &Path) -> Option<PathBuf> {
    diretorios_de(&evidencia.join("batches"))
        .into_iter()
        .find(|lote| {
            diretorios_de(lote).iter().any(|caminho| {
                caminho
                    .file_name()
                    .map(|nome| nome.to_string_lossy().starts_with(".running-"))
                    .unwrap_or(false)
            })
        })
}

fn diretorios_de(raiz: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = fs::read_dir(raiz) else {
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
    let limite = Instant::now() + Duration::from_secs(30);
    let mut lote_em_curso: Option<PathBuf> = None;
    while Instant::now() < limite {
        if let Some(lote) = lote_em_execucao(&evidencia) {
            lote_em_curso = Some(lote);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let lote = lote_em_curso.expect("iteração não chegou a iniciar");
    assert!(
        evidencia.join(".lock/owner.marker").is_file(),
        "campanha em andamento precisa deter o lock"
    );

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

    let preservados: Vec<PathBuf> = diretorios_de(&lote)
        .into_iter()
        .filter(|caminho| {
            caminho
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
    assert!(
        !evidencia.join(".lock").exists(),
        "SIGINT precisa liberar o lock"
    );
    assert!(
        !evidencia.join("SUMMARY-modo.txt").exists(),
        "lote interrompido nunca publica projeção legada"
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

// ---------------------------------------------------------------------------
// Exclusividade por checkout e namespace de lote.
//
// Duas campanhas sobre o mesmo `target` compartilhavam progresso, resumo,
// sandboxes e arquivos auxiliares, e o runner removia `PROGRESS-<mode>.txt` e
// `SUMMARY-<mode>.txt` no início de cada lote. A segunda campanha destruía a
// evidência da primeira e podia terminar verde escondendo as iterações
// falhadas da outra. Ocorreu de fato durante a correção da PR 422.
//
// Nenhuma destas regressões executa campanha longa: o harness é um binário
// falso, e a campanha proprietária só precisa permanecer viva o suficiente
// para que a segunda seja rejeitada.
// ---------------------------------------------------------------------------

const EXIT_TRAVADO: i32 = 3;

/// Campanha proprietária viva, controlada pelo teste.
///
/// Mantém o lock enquanto o binário falso dorme, e é encerrada por sinal
/// explícito ao final do caso.
struct CampanhaViva {
    filho: std::process::Child,
    evidencia: PathBuf,
    lote: PathBuf,
}

impl CampanhaViva {
    fn iniciar(raiz: &Path, modo: &str) -> Self {
        let binario = binario_falso_com_atraso(raiz, RESUMO_COM_TESTE, 0, 60);
        let filho = Command::new(raiz.join("scripts/pinker-flake-runner.sh"))
            .args([modo, "1"])
            .env("PINKER_FLAKE_TEST_BINARY", &binario)
            .env_remove("PINKER_FLAKE_RUN_TIMEOUT_SECONDS")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("iniciar campanha proprietária");
        let evidencia = raiz.join("target/pinker-flake-evidence");
        // Espera o estado em que os casos afirmam operar: lock adquirido, lote
        // criado e iteração em curso. Parar no marker deixaria uma janela em
        // que o lote ainda não existe e o `trap` não teria o que preservar.
        let limite = Instant::now() + Duration::from_secs(30);
        while Instant::now() < limite {
            if evidencia.join(".lock/owner.marker").is_file() {
                if let Some(lote) = lote_em_execucao(&evidencia) {
                    return Self {
                        filho,
                        evidencia,
                        lote,
                    };
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("campanha proprietária não chegou a executar uma iteração");
    }

    fn viva(&mut self) -> bool {
        matches!(self.filho.try_wait(), Ok(None))
    }

    fn marker(&self) -> String {
        fs::read_to_string(self.evidencia.join(".lock/owner.marker")).expect("marker do lock")
    }

    fn encerrar(mut self, sinal: i32) -> i32 {
        enviar_sinal(self.filho.id() as i32, sinal);
        let status = self.filho.wait().expect("aguardar campanha proprietária");
        status.code().unwrap_or(-1)
    }
}

fn enviar_sinal(pid: i32, sinal: i32) {
    unsafe {
        extern "C" {
            fn kill(pid: i32, sinal: i32) -> i32;
        }
        assert_eq!(kill(pid, sinal), 0, "enviar sinal {sinal} para {pid}");
    }
}

fn campo_do_marker(marker: &str, chave: &str) -> String {
    marker
        .lines()
        .find_map(|linha| linha.strip_prefix(&format!("{chave}: ")))
        .unwrap_or_else(|| panic!("marker sem campo {chave}: {marker}"))
        .to_string()
}

/// Escreve um lock cujo proprietário é uma identidade escolhida pelo teste.
fn plantar_lock(evidencia: &Path, marker: &str) {
    let lock = evidencia.join(".lock");
    fs::create_dir_all(&lock).expect("criar lock plantado");
    fs::write(lock.join("owner.marker"), marker).expect("escrever marker plantado");
}

fn marker_valido(pid: &str, start: &str, modo: &str, lote: &str) -> String {
    format!(
        "schema: 1\nrunner_pid: {pid}\nrunner_start_time: {start}\nmode: {modo}\n\
         head_git: unknown\ncreated_at_unix: 1700000000\nbatch_id: {lote}\n"
    )
}

/// PID livre e o start time que o tornaria `Missing`.
///
/// Usa um processo curto já encerrado e recolhido: o número deixou de nomear
/// processo algum, que é exatamente a classe `Missing`.
fn pid_encerrado() -> i32 {
    let mut filho = Command::new("/bin/true")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("processo curto");
    let pid = filho.id() as i32;
    filho.wait().expect("recolher processo curto");
    pid
}

fn executar_com_lock(raiz: &Path, modo: &str) -> Execucao {
    let binario = binario_falso(raiz, RESUMO_COM_TESTE, 0);
    executar(
        raiz,
        &[modo, "1"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    )
}

// --- aquisição, rejeição e integridade da campanha proprietária ------------

#[test]
fn primeira_campanha_adquire_o_lock_com_marker_completo() {
    let raiz = raiz_isolada("lock-adquire");
    let mut dona = CampanhaViva::iniciar(&raiz, "dona");
    let marker = dona.marker();
    for chave in [
        "schema",
        "runner_pid",
        "runner_start_time",
        "mode",
        "head_git",
        "created_at_unix",
        "batch_id",
    ] {
        assert!(
            marker.contains(&format!("{chave}: ")),
            "marker sem {chave}: {marker}"
        );
    }
    assert_eq!(campo_do_marker(&marker, "mode"), "dona");
    assert_eq!(campo_do_marker(&marker, "schema"), "1");
    assert_eq!(
        campo_do_marker(&marker, "runner_pid"),
        dona.filho.id().to_string()
    );
    assert!(dona.viva());
    dona.encerrar(15);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn segunda_campanha_no_mesmo_checkout_e_rejeitada() {
    let raiz = raiz_isolada("lock-rejeita");
    let mut dona = CampanhaViva::iniciar(&raiz, "dona");
    let segunda = executar_com_lock(&raiz, "dona");
    assert_eq!(
        segunda.codigo, EXIT_TRAVADO,
        "stderr={}",
        segunda.saida_erro
    );
    assert!(
        segunda.saida_erro.contains("campanha concorrente"),
        "diagnóstico determinístico ausente: {}",
        segunda.saida_erro
    );
    assert!(segunda.saida_erro.contains("identity=live"));
    assert!(dona.viva(), "campanha proprietária precisa seguir intacta");
    dona.encerrar(15);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn segunda_campanha_com_mode_diferente_tambem_e_rejeitada() {
    // O lock é do checkout, não do mode: nenhum par de modes pode coexistir.
    let raiz = raiz_isolada("lock-mode-diferente");
    let mut dona = CampanhaViva::iniciar(&raiz, "dona");
    let segunda = executar_com_lock(&raiz, "outro");
    assert_eq!(segunda.codigo, EXIT_TRAVADO);
    assert!(dona.viva());
    dona.encerrar(15);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn rejeicao_ocorre_antes_de_tocar_resumo_e_antes_do_binario_de_teste() {
    let raiz = raiz_isolada("lock-antes-de-tudo");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    let anterior = evidencia.join("SUMMARY-outro.txt");
    fs::write(&anterior, "resumo verde anterior\n").expect("resumo anterior");

    let mut dona = CampanhaViva::iniciar(&raiz, "dona");
    let lotes_antes = diretorios_de(&evidencia.join("batches"));
    assert_eq!(
        lotes_antes.len(),
        1,
        "apenas o lote da campanha proprietária existe neste ponto"
    );
    assert_eq!(
        lotes_antes[0], dona.lote,
        "o lote existente é o da proprietária"
    );

    // Binário falso que registra ter sido executado. Se a rejeição vier depois
    // do início do teste, o rastro existe.
    let sentinela = raiz.join("executou.txt");
    let binario = raiz.join("harness-sentinela.sh");
    fs::write(
        &binario,
        format!(
            "#!/usr/bin/env bash\nprintf 'sim\\n' > {}\nexit 0\n",
            sentinela.display()
        ),
    )
    .expect("escrever sentinela");
    let mut permissoes = fs::metadata(&binario).expect("metadados").permissions();
    permissoes.set_mode(0o755);
    fs::set_permissions(&binario, permissoes).expect("permissões");

    let segunda = executar(
        &raiz,
        &["outro", "1"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );

    assert_eq!(segunda.codigo, EXIT_TRAVADO);
    assert_eq!(
        fs::read_to_string(&anterior).expect("ler resumo anterior"),
        "resumo verde anterior\n",
        "rejeição não pode tocar resumo existente"
    );
    assert!(
        !sentinela.exists(),
        "rejeição não pode iniciar o binário de teste"
    );
    assert_eq!(
        diretorios_de(&evidencia.join("batches")),
        lotes_antes,
        "rejeição não pode criar lote"
    );
    assert!(dona.viva());
    dona.encerrar(15);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn lock_vivo_e_preservado_pela_campanha_rejeitada() {
    let raiz = raiz_isolada("lock-preserva-vivo");
    let dona = CampanhaViva::iniciar(&raiz, "dona");
    let marker_antes = dona.marker();
    let segunda = executar_com_lock(&raiz, "outro");
    assert_eq!(segunda.codigo, EXIT_TRAVADO);
    assert_eq!(
        dona.marker(),
        marker_antes,
        "o lock da campanha viva não pode ser alterado"
    );
    dona.encerrar(15);
    let _ = fs::remove_dir_all(&raiz);
}

// --- classificação de identidade do proprietário --------------------------

#[test]
fn lock_com_proprietario_missing_e_recuperado() {
    let raiz = raiz_isolada("lock-missing");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    plantar_lock(
        &evidencia,
        &marker_valido(&pid_encerrado().to_string(), "12345", "morta", "lote-morto"),
    );

    let execucao = executar_com_lock(&raiz, "nova");
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    assert!(
        execucao.saida_erro.contains("lock obsoleto recuperado"),
        "recuperação precisa ser explícita: {}",
        execucao.saida_erro
    );
    assert!(!execucao.lock().exists(), "lock liberado ao final");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn lock_com_proprietario_reused_e_recuperado() {
    // O PID existe, mas com outro start time: o número foi herdado por um
    // processo alheio e a campanha proprietária já terminou.
    let raiz = raiz_isolada("lock-reused");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    plantar_lock(
        &evidencia,
        &marker_valido(&std::process::id().to_string(), "1", "morta", "lote-morto"),
    );

    let execucao = executar_com_lock(&raiz, "nova");
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    assert!(execucao.saida_erro.contains("lock obsoleto recuperado"));
    assert!(!execucao.lock().exists());
    let _ = fs::remove_dir_all(&raiz);
}

fn classificar(pid: &str, start: &str) -> String {
    let runner = caminho_do_runner();
    let script = format!(
        "PINKER_FLAKE_LIB_ONLY=1 source {}; pinker_flake_classify_identity {} {}",
        runner.display(),
        aspas_simples(pid),
        aspas_simples(start)
    );
    let saida = Command::new("bash")
        .arg("-c")
        .arg(script)
        .output()
        .expect("executar classificador");
    String::from_utf8_lossy(&saida.stdout).trim().to_string()
}

#[test]
fn classificacao_de_identidade_distingue_as_quatro_classes() {
    let proprio = std::process::id().to_string();
    let start_proprio = fs::read_to_string(format!("/proc/{proprio}/stat"))
        .expect("stat do próprio processo")
        .rsplit_once(") ")
        .map(|(_, resto)| {
            resto
                .split_whitespace()
                .nth(19)
                .expect("campo starttime")
                .to_string()
        })
        .expect("recorte do stat");

    assert_eq!(classificar(&proprio, &start_proprio), "live");
    assert_eq!(
        classificar(&proprio, "1"),
        "reused",
        "mesmo PID com outro start time é número herdado"
    );
    assert_eq!(classificar(&pid_encerrado().to_string(), "1"), "missing");

    // `unknown` é a classe que nunca autoriza remoção. No caminho real ela
    // exige `/proc` inacessível, o que não é fabricável nesta VM; a autoridade
    // é provada diretamente sobre a função.
    assert_eq!(classificar("0", "1"), "unknown", "PID não positivo");
    assert_eq!(classificar("-1", "1"), "unknown");
    assert_eq!(classificar("abc", "1"), "unknown");
    assert_eq!(
        classificar(&proprio, "abc"),
        "unknown",
        "start time ilegível"
    );
    assert_eq!(classificar("", ""), "unknown");
}

#[test]
fn identidade_desconhecida_falha_fechada_e_preserva() {
    // Somente `missing` e `reused` são provas positivas de que a campanha
    // proprietária terminou. `unknown` precisa preservar, e o mapeamento vive
    // em um único ponto da autoridade.
    let fonte = fs::read_to_string(caminho_do_runner()).expect("ler runner");
    let recuperaveis = fonte
        .lines()
        .filter(|linha| linha.trim_start().starts_with("missing|reused)"))
        .count();
    assert_eq!(
        recuperaveis, 1,
        "a recuperação precisa ter exatamente um ponto de entrada"
    );
    assert!(
        fonte.contains("identidade do proprietario desconhecida, falha fechada"),
        "identidade desconhecida precisa falhar fechada"
    );
    assert!(
        fonte.contains("identity=unknown"),
        "o diagnóstico precisa nomear a classe observada"
    );
}

// --- integridade estrita do marker ----------------------------------------

fn caso_marker_invalido(caso: &str, conteudo: Option<&str>) {
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(evidencia.join(".lock")).expect("criar lock");
    if let Some(texto) = conteudo {
        fs::write(evidencia.join(".lock/owner.marker"), texto).expect("marker");
    }
    let execucao = executar_com_lock(&raiz, "nova");
    assert_eq!(
        execucao.codigo, EXIT_TRAVADO,
        "{caso}: marker inválido falha fechada; stderr={}",
        execucao.saida_erro
    );
    assert!(
        execucao.saida_erro.contains("invalido") || execucao.saida_erro.contains("inválido"),
        "{caso}: diagnóstico ausente; stderr={}",
        execucao.saida_erro
    );
    assert!(execucao.lock().exists(), "{caso}: lock preservado");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn marker_ausente_falha_fechado() {
    caso_marker_invalido("marker-ausente", None);
}

#[test]
fn marker_truncado_falha_fechado() {
    caso_marker_invalido(
        "marker-truncado",
        Some("schema: 1\nrunner_pid: 1\nrunner_start_time: 2\n"),
    );
}

#[test]
fn marker_com_campo_extra_falha_fechado() {
    let mut texto = marker_valido("1", "2", "modo", "lote");
    texto.push_str("extra: 1\n");
    caso_marker_invalido("marker-extra", Some(&texto));
}

#[test]
fn marker_com_campo_duplicado_falha_fechado() {
    let texto = "schema: 1\nrunner_pid: 1\nrunner_pid: 2\nrunner_start_time: 2\nmode: modo\n\
                 head_git: unknown\ncreated_at_unix: 1\nbatch_id: lote\n";
    caso_marker_invalido("marker-duplicado", Some(texto));
}

#[test]
fn marker_fora_de_ordem_falha_fechado() {
    let texto = "runner_pid: 1\nschema: 1\nrunner_start_time: 2\nmode: modo\n\
                 head_git: unknown\ncreated_at_unix: 1\nbatch_id: lote\n";
    caso_marker_invalido("marker-ordem", Some(texto));
}

#[test]
fn lock_symlink_e_rejeitado() {
    let raiz = raiz_isolada("lock-symlink");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    let alvo = raiz.join("alvo-do-symlink");
    fs::create_dir_all(&alvo).expect("criar alvo");
    std::os::unix::fs::symlink(&alvo, evidencia.join(".lock")).expect("criar symlink");

    let execucao = executar_com_lock(&raiz, "nova");
    assert_eq!(
        execucao.codigo, EXIT_TRAVADO,
        "stderr={}",
        execucao.saida_erro
    );
    assert!(
        execucao.saida_erro.contains("symlink"),
        "stderr={}",
        execucao.saida_erro
    );
    assert!(
        evidencia.join(".lock").symlink_metadata().is_ok(),
        "symlink preservado, nunca removido pelo nome"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn troca_de_identidade_antes_da_remocao_preserva_o_lock() {
    // O gancho substitui o marker exatamente entre a classificação e a
    // remoção. O lock deixa de ser o objeto validado e precisa sobreviver.
    let raiz = raiz_isolada("lock-troca-identidade");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    plantar_lock(
        &evidencia,
        &marker_valido(&pid_encerrado().to_string(), "999", "morta", "lote-morto"),
    );

    let gancho = raiz.join("gancho.sh");
    let substituto = marker_valido("1", "1", "outra", "lote-outro");
    fs::write(
        &gancho,
        format!(
            "#!/usr/bin/env bash\nprintf '%s' {} > \"$2/owner.marker\"\nexit 0\n",
            aspas_simples(&substituto)
        ),
    )
    .expect("escrever gancho");
    let mut permissoes = fs::metadata(&gancho).expect("metadados").permissions();
    permissoes.set_mode(0o755);
    fs::set_permissions(&gancho, permissoes).expect("permissões do gancho");

    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["nova", "1"],
        &[
            ("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8")),
            ("PINKER_FLAKE_TEST_HOOK", gancho.to_str().expect("utf-8")),
        ],
    );

    assert_eq!(
        execucao.codigo, EXIT_TRAVADO,
        "stderr={}",
        execucao.saida_erro
    );
    assert!(
        execucao.lock().join("owner.marker").is_file(),
        "identidade divergente preserva o lock"
    );
    assert_eq!(
        fs::read_to_string(execucao.lock().join("owner.marker")).expect("marker"),
        substituto,
        "o lock preservado é o novo, não o validado"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// --- liberação -------------------------------------------------------------

#[test]
fn sucesso_libera_o_lock() {
    let raiz = raiz_isolada("lock-libera-sucesso");
    let execucao = executar_com_lock(&raiz, "modo");
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    assert!(!execucao.lock().exists(), "sucesso precisa liberar o lock");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn falha_libera_o_lock() {
    let raiz = raiz_isolada("lock-libera-falha");
    let binario = binario_falso(&raiz, SEM_RESUMO, 0);
    let execucao = executar(
        &raiz,
        &["modo", "1"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );
    assert_eq!(execucao.codigo, EXIT_FALHAS);
    assert!(
        !execucao.lock().exists(),
        "falha comum também libera o lock"
    );
    let _ = fs::remove_dir_all(&raiz);
}

fn caso_sinal_libera_lock(caso: &str, sinal: i32) {
    let raiz = raiz_isolada(caso);
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let lote = dona.lote.clone();
    let codigo = dona.encerrar(sinal);
    assert_eq!(codigo, EXIT_INTERROMPIDO, "{caso}: saída de interrupção");
    assert!(
        !evidencia.join(".lock").exists(),
        "{caso}: sinal precisa liberar o lock"
    );
    let interrompidos: Vec<PathBuf> = diretorios_de(&lote)
        .into_iter()
        .filter(|caminho| {
            caminho
                .file_name()
                .map(|nome| nome.to_string_lossy().starts_with("INTERRUPTED-"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        interrompidos.len(),
        1,
        "{caso}: evidência da interrupção preservada: {interrompidos:?}"
    );
    assert!(
        !evidencia.join("SUMMARY-modo.txt").exists(),
        "{caso}: lote interrompido não publica projeção legada"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn sigint_libera_o_lock_e_preserva_evidencia() {
    caso_sinal_libera_lock("lock-sigint", 2);
}

#[test]
fn sigterm_libera_o_lock_e_preserva_evidencia() {
    caso_sinal_libera_lock("lock-sigterm", 15);
}

#[test]
fn lock_deixado_por_sigkill_e_recuperavel() {
    // SIGKILL não executa trap: o lock sobrevive ao proprietário. A identidade
    // registrada é o que permite a uma campanha posterior classificá-lo.
    let raiz = raiz_isolada("lock-sigkill");
    let dona = CampanhaViva::iniciar(&raiz, "morta");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    dona.encerrar(9);
    assert!(
        evidencia.join(".lock/owner.marker").is_file(),
        "SIGKILL deixa o lock para trás"
    );

    let seguinte = executar_com_lock(&raiz, "seguinte");
    assert_eq!(seguinte.codigo, 0, "stderr={}", seguinte.saida_erro);
    assert!(seguinte.saida_erro.contains("lock obsoleto recuperado"));
    assert!(!evidencia.join(".lock").exists());
    let _ = fs::remove_dir_all(&raiz);
}

// --- namespace de lote e projeção legada ----------------------------------

#[test]
fn cada_lote_possui_namespace_exclusivo() {
    let raiz = raiz_isolada("lote-exclusivo");
    let execucao = executar_com_lock(&raiz, "modo");
    assert_eq!(execucao.codigo, 0);
    let lote = execucao.lote_unico();
    for exigido in ["MANIFEST.txt", "PROGRESS.txt", "SUMMARY.txt"] {
        assert!(
            lote.join(exigido).is_file(),
            "lote sem {exigido}: {}",
            lote.display()
        );
    }
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn dois_lotes_sequenciais_mantem_manifests_proprios() {
    let raiz = raiz_isolada("lotes-sequenciais");
    let primeira = executar_com_lock(&raiz, "modo");
    assert_eq!(primeira.codigo, 0);
    let segunda = executar_com_lock(&raiz, "modo");
    assert_eq!(segunda.codigo, 0);

    let lotes = segunda.lotes();
    assert_eq!(lotes.len(), 2, "dois lotes distintos: {lotes:?}");
    let mut identificadores = Vec::new();
    for lote in &lotes {
        let manifesto = fs::read_to_string(lote.join("MANIFEST.txt")).expect("manifesto do lote");
        let identificador = manifesto
            .lines()
            .find_map(|linha| linha.strip_prefix("batch_id="))
            .expect("batch_id no manifesto")
            .to_string();
        assert!(
            lote.ends_with(&identificador),
            "manifesto pertence a outro lote: {manifesto}"
        );
        identificadores.push(identificador);
    }
    assert_ne!(
        identificadores[0], identificadores[1],
        "lotes precisam de identificadores distintos"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn resumo_de_lote_anterior_nao_e_removido_no_inicio() {
    // A projeção legada do primeiro lote precisa sobreviver ao segundo até que
    // o segundo conclua, e o resumo do primeiro lote permanece intacto no seu
    // próprio diretório para sempre.
    let raiz = raiz_isolada("lote-preserva-anterior");
    let primeira = executar_com_lock(&raiz, "modo");
    assert_eq!(primeira.codigo, 0);
    let primeiro_lote = primeira.lote_unico();
    let resumo_do_primeiro =
        fs::read_to_string(primeiro_lote.join("SUMMARY.txt")).expect("resumo do primeiro lote");

    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let projecao_durante = fs::read_to_string(primeira.evidencia().join("SUMMARY-modo.txt"))
        .expect("projeção do primeiro lote durante o segundo");
    assert!(
        projecao_durante.contains("completed=1"),
        "projeção anterior removida no início: {projecao_durante}"
    );
    dona.encerrar(15);

    assert_eq!(
        fs::read_to_string(primeiro_lote.join("SUMMARY.txt")).expect("resumo do primeiro lote"),
        resumo_do_primeiro,
        "o resumo do lote anterior é imutável"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn projecao_legada_e_publicada_atomicamente_e_identifica_o_lote() {
    let raiz = raiz_isolada("projecao-atomica");
    let execucao = executar_com_lock(&raiz, "modo");
    assert_eq!(execucao.codigo, 0);
    let projecao = execucao.resumo("modo").expect("projeção legada");
    assert!(
        projecao.contains("projection=last-completed-batch"),
        "projeção precisa se declarar não autoritativa: {projecao}"
    );
    assert!(projecao.contains("batch_id="), "projeção: {projecao}");
    assert!(projecao.contains("head_sha="), "projeção: {projecao}");
    assert!(projecao.contains("authority="), "projeção: {projecao}");

    // Nenhum temporário de publicação sobrevive.
    let restos: Vec<PathBuf> = fs::read_dir(execucao.evidencia())
        .expect("ler evidência")
        .flatten()
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho
                .file_name()
                .map(|nome| nome.to_string_lossy().contains(".parcial"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        restos.is_empty(),
        "temporário de publicação vazado: {restos:?}"
    );

    // O conteúdo da projeção corresponde ao resumo autoritativo do lote.
    let autoridade =
        fs::read_to_string(execucao.lote_unico().join("SUMMARY.txt")).expect("resumo do lote");
    for linha in autoridade.lines() {
        assert!(
            projecao.contains(linha),
            "projeção divergiu da autoridade na linha {linha}"
        );
    }
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn campanha_falhada_publica_resumo_falhado_do_proprio_lote() {
    let raiz = raiz_isolada("lote-falhado");
    let verde = executar_com_lock(&raiz, "modo");
    assert_eq!(verde.codigo, 0);
    let projecao_verde = verde.resumo("modo").expect("projeção verde");
    assert!(projecao_verde.contains("failures=0"));

    let binario = binario_falso(&raiz, RESUMO_COM_FALHA, 101);
    let vermelha = executar(
        &raiz,
        &["modo", "1"],
        &[("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8"))],
    );
    assert_eq!(vermelha.codigo, EXIT_FALHAS);

    let projecao = vermelha.resumo("modo").expect("projeção do lote falhado");
    assert!(
        projecao.contains("failures=1") && projecao.contains("exit_code=1"),
        "campanha falhada não pode produzir resumo verde: {projecao}"
    );

    let lotes = vermelha.lotes();
    assert_eq!(lotes.len(), 2);
    let resumo_falhado =
        fs::read_to_string(lotes[1].join("SUMMARY.txt")).expect("resumo do lote falhado");
    assert!(resumo_falhado.contains("failures=1"), "{resumo_falhado}");
    let resumo_verde =
        fs::read_to_string(lotes[0].join("SUMMARY.txt")).expect("resumo do lote verde");
    assert!(
        resumo_verde.contains("failures=0"),
        "o lote verde anterior permanece verde no próprio diretório: {resumo_verde}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn resumo_do_lote_registra_batch_id_e_head_sha() {
    let raiz = raiz_isolada("resumo-identifica-lote");
    let execucao = executar_com_lock(&raiz, "modo");
    assert_eq!(execucao.codigo, 0);
    let lote = execucao.lote_unico();
    let resumo = fs::read_to_string(lote.join("SUMMARY.txt")).expect("resumo do lote");
    let identificador = lote
        .file_name()
        .expect("nome do lote")
        .to_string_lossy()
        .into_owned();
    assert!(
        resumo.contains(&format!("batch_id={identificador}")),
        "resumo={resumo}"
    );
    assert!(resumo.contains("head_sha="), "resumo={resumo}");
    assert!(
        execucao
            .saida_padrao
            .contains(&format!("batch_id={identificador}")),
        "a linha SUMMARY impressa também identifica o lote: {}",
        execucao.saida_padrao
    );
    let _ = fs::remove_dir_all(&raiz);
}
