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
//! A interrupção tem autoridade terminal própria: depois que INT, TERM ou HUP
//! é congelado, nenhum retorno do `wait` nem trap posterior pode devolver o
//! controle ao veredito normal. Barreiras causais cobrem o `wait`, o reaping e
//! o EXIT trap; um trace opt-in prova a ordem sem usar tempo como sincronização.
//!
//! Nenhuma dependência externa. O harness é substituído por um binário
//! falso controlado, e cada caso roda em uma raiz própria para não tocar a
//! evidência real do repositório.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

fn caminho_do_runner() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/pinker-flake-runner.sh")
}

// ---------------------------------------------------------------------------
// Publicação de executáveis.
//
// `ETXTBSY` no `exec` ocorre enquanto **qualquer** processo mantém descritor
// gravável para o inode executado. Escrever o arquivo aqui, no processo que roda
// os testes, abre duas janelas sob paralelismo: o próprio processo mantém o
// descritor enquanto escreve, e um `fork` concorrente — que copia a tabela de
// descritores do processo inteiro, não da thread — produz um filho que segura
// aquele descritor até o seu próprio `exec`.
//
// Escrever em nome temporário e renomear, isolado, **não** resolve: `rename`
// troca o nome publicado, não o inode. A regressão
// `rename_nao_remove_descritor_gravavel_sobre_o_inode` demonstra isso.
//
// A publicação passa a ter quatro passos, e cada um responde por um invariante:
//
//   1. o pai escreve `<destino>.fonte`, que nunca recebe bit de execução e nunca
//      é executada, e fecha o descritor antes de qualquer fork;
//   2. um processo auxiliar materializa `<destino>.parcial`. Só ele abre esse
//      inode para escrita, e a tabela de descritores do pai nunca o contém,
//      portanto nenhum fork do pai pode herdá-lo;
//   3. o pai valida exit status, tipo, permissão e conteúdo. Falha é
//      fail-closed;
//   4. o auxiliar já terminou, logo o descritor está fechado, e só então o pai
//      renomeia para o nome final — que nasce completo, executável e sem writer.
//
// Não há retry de `ETXTBSY` e não há espera probabilística. A classe causal é
// eliminada por construção, não contornada.
// ---------------------------------------------------------------------------

// pinker-fork-autorizado:inicio
//
// Região única autorizada a criar processo para materializar executável. A
// meta-regressão `publicacao_e_a_unica_autoridade_de_bit_executavel` inspeciona
// apenas o texto **fora** destas sentinelas.

/// Publica um arquivo executável sem que este processo detenha, em momento
/// algum, descritor gravável para o inode publicado.
fn publicar_executavel(destino: &Path, conteudo: &str) -> PathBuf {
    let fonte = destino.with_extension("fonte");
    let parcial = destino.with_extension("parcial");

    {
        // Sem bit de execução e nunca executada: manter o descritor aqui é
        // inofensivo, e ele fecha ao fim do bloco, antes de qualquer fork.
        let mut arquivo = fs::File::create(&fonte).expect("criar fonte não executável");
        arquivo
            .write_all(conteudo.as_bytes())
            .expect("escrever fonte");
        arquivo.sync_all().expect("sincronizar fonte");
    }

    // O único processo que abre o inode executável para escrita é este auxiliar.
    let status = Command::new("install")
        .arg("-m")
        .arg("0755")
        .arg(&fonte)
        .arg(&parcial)
        .status()
        .expect("executar o publicador auxiliar");
    assert!(
        status.success(),
        "publicação falhou fechada para {}: {status:?}",
        destino.display()
    );

    // O auxiliar terminou: nenhum descritor gravável sobrevive sobre o inode.
    let meta = fs::metadata(&parcial).expect("metadados do materializado");
    assert!(
        meta.is_file(),
        "o materializado precisa ser arquivo regular"
    );
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o755,
        "permissões aplicadas antes da publicação"
    );
    assert_eq!(
        fs::read_to_string(&parcial).expect("reler materializado"),
        conteudo,
        "conteúdo íntegro antes da publicação"
    );

    // Só agora o nome final passa a existir, já completo e executável.
    fs::rename(&parcial, destino).expect("publicar nome final");
    fs::remove_file(&fonte).expect("remover fonte");

    destino.to_path_buf()
}

// pinker-fork-autorizado:fim

/// Processo auxiliar que mantém descritor **gravável** sobre um caminho.
///
/// Serve às provas de causalidade. A abertura é confirmada por sinalização no
/// stdout do filho, de modo que a janela é determinada por sincronização
/// explícita e nunca por tempo.
struct EscritorConcorrente {
    filho: Child,
    saida: BufReader<std::process::ChildStdout>,
}

impl EscritorConcorrente {
    fn abrir(caminho: &Path) -> Self {
        let mut filho = Command::new("/bin/sh")
            .arg("-c")
            // `>>` abre para escrita **sem truncar**: o descritor continua
            // gravável, que é a condição causal, e o conteúdo sobrevive.
            .arg(format!(
                "exec 9>> {}; printf 'aberto\n'; read _fecha; exec 9>&-",
                aspas_simples(caminho.to_str().expect("utf-8"))
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("iniciar escritor concorrente");
        let mut saida = BufReader::new(filho.stdout.take().expect("stdout do escritor"));
        let mut confirmacao = String::new();
        saida
            .read_line(&mut confirmacao)
            .expect("aguardar abertura do descritor");
        assert_eq!(
            confirmacao.trim(),
            "aberto",
            "escritor concorrente não confirmou a abertura"
        );
        Self { filho, saida }
    }

    fn pid(&self) -> u32 {
        self.filho.id()
    }

    /// Fecha o descritor e recolhe o processo, sem sinal e sem espera cega.
    fn fechar(mut self) {
        drop(self.filho.stdin.take());
        let _ = self.saida;
        self.filho.wait().expect("recolher escritor concorrente");
    }
}

fn inode_de(caminho: &Path) -> u64 {
    fs::metadata(caminho).expect("metadados").ino()
}

/// `ETXTBSY`. Comparado pelo número do erro porque
/// `ErrorKind::ExecutableFileBusy` ainda é instável, e a suíte é stable-only.
const ETXTBSY: i32 = 26;

/// Erro do sistema devolvido ao tentar executar `caminho`, se houver.
fn erro_ao_executar(caminho: &Path) -> Option<i32> {
    match Command::new(caminho)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => None,
        Err(erro) => Some(erro.raw_os_error().unwrap_or(-1)),
    }
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
    // Ler a origem é seguro: leitura não bloqueia `exec`. O que não pode existir
    // é descritor **gravável** sobre o inode de destino.
    let conteudo = fs::read_to_string(caminho_do_runner()).expect("ler runner");
    publicar_executavel(&raiz.join("scripts/pinker-flake-runner.sh"), &conteudo);
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
    let mut conteudo = String::from("#!/usr/bin/env bash\n");
    if atraso_segundos > 0 {
        conteudo.push_str(&format!("sleep {atraso_segundos}\n"));
    }
    for linha in saida_padrao.lines() {
        conteudo.push_str(&format!("printf '%s\\n' {}\n", aspas_simples(linha)));
    }
    conteudo.push_str(&format!("exit {codigo}\n"));
    publicar_executavel(&caminho, &conteudo)
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

/// Duração do harness falso nos casos que precisam sinalizar uma iteração
/// **em curso**.
///
/// A janela útil desses casos é o intervalo entre o início da iteração e o seu
/// fim. Trinta segundos pareciam folgados e não eram: sob execuções paralelas
/// com os núcleos saturados, a fome de escalonamento entre observar o lote e
/// enviar o sinal chegou perto desse valor, a iteração terminou antes, e o caso
/// que exige `130` recebeu `0`. Uma máquina ociosa esconde exatamente isso.
///
/// O valor fica bem abaixo do `per_run_timeout` padrão do runner, de trezentos
/// segundos, para que o `timeout` do produto nunca seja quem encerra a
/// iteração: o caso precisa que quem encerre seja o sinal.
const HARNESS_LONGO_SEGUNDOS: u64 = 240;

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
    let evidencia = raiz.join("target/pinker-flake-evidence");

    // Este caso usava espera própria: aguardava só o lote `.running-` e
    // sinalizava a seguir, tipicamente aos cinquenta milissegundos. O lote nasce
    // **antes** de o runner capturar o start time do `setsid timeout` que
    // acabara de criar, e sem esse dado o `cleanup_active` do runner sai sem
    // matar nada. O caso terminava verde — recebia mesmo o `130` — deixando
    // `timeout` e harness órfãos até o `timeout` deles expirar, trezentos
    // segundos depois. Oito iterações de um burn-in de quarenta reportaram o
    // mesmo par de PIDs por isso.
    //
    // `CampanhaViva` espera pela árvore antes de devolver o controle e contém o
    // que sobrar. Espera duplicada, e mais fraca, em cada chamador é exatamente
    // como esta classe de vazamento se reintroduz.
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let lote = dona.lote.clone();
    assert!(
        evidencia.join(".lock/owner.marker").is_file(),
        "campanha em andamento precisa deter o lock"
    );

    // SIGINT no runner, exatamente como uma interrupção de terminal.
    let relatorio = dona.encerrar_com_relatorio(SIGINT);
    assert_eq!(
        relatorio.codigo, EXIT_INTERROMPIDO,
        "interrupção deve retornar 130; controlador={:?} sessoes={:?}",
        relatorio.controlador, relatorio.sessoes
    );
    assert!(
        relatorio.restantes.is_empty(),
        "a interrupção não pode deixar árvore para trás: {:?}",
        relatorio.papeis_restantes()
    );
    assert!(
        relatorio.grupo_vazio && relatorio.sessao_vazia,
        "grupo e sessões da campanha precisam ficar vazios"
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

/// Identidade do controlador não pôde ser provada. Distinto de falha de teste e
/// distinto de lock: nenhuma iteração chegou a ser observada com segurança.
const EXIT_IDENTIDADE: i32 = 4;

// ---------------------------------------------------------------------------
// Contenção da campanha proprietária.
//
// `SIGKILL` não executa trap algum. Matar o controlador deixava vivos o
// `timeout` que ele cria por `setsid`, o harness sob esse `timeout`, o `sleep`
// do harness e o subshell monitor — quatro processos por iteração, reparentados
// para o init. Foi o que produziu os dezesseis órfãos observados na PR 424.
//
// A autoridade de encerramento pertence à **fixture**, não ao runner de
// produto: o encerramento é artificial, provocado pelo teste, e o runner não
// pode ser responsabilizado por sobreviver ao próprio assassinato. O contrato
// tem três peças:
//
//   1. o controlador nasce em sessão e grupo exclusivos (`setsid` entre o
//      `fork` e o `exec`), de modo que a árvore da campanha jamais se confunde
//      com a do `cargo test` e conter a campanha jamais alcança o processo de
//      testes;
//   2. a árvore é capturada **antes** de qualquer sinal, porque o `SIGKILL`
//      reparenta os sobreviventes e destrói a ancestralidade que os
//      identificaria depois. O que sobrevive ao reparenting é a **sessão**, e é
//      por isso que as sessões observadas viram propriedade registrada;
//   3. nenhum sinal parte sem revalidar `(pid, start time, pgid, sid)`. Nunca se
//      sinaliza um grupo inteiro, nunca se casa por nome e nunca se age sobre
//      identidade não comprovada. Diante de ambiguidade, falha fechada.
// ---------------------------------------------------------------------------

// pinker-contencao:inicio
//
// Região única autorizada a enviar sinal. A regressão de sensibilidade
// `sensibilidade_das_guardas_de_contencao_detecta_cada_variacao` inspeciona
// este recorte.

// Chamadas de sistema exigidas pela contenção. A suíte é sem dependência
// externa: em vez de uma crate de bindings, declara exatamente as duas chamadas
// de que precisa.
extern "C" {
    fn kill(pid: i32, sinal: i32) -> i32;
    fn setsid() -> i32;
    fn signal(sinal: i32, manipulador: usize) -> usize;
}

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGQUIT: i32 = 3;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

const SIG_DFL: usize = 0;
const SIG_ERR: usize = usize::MAX;

/// Devolve os sinais de encerramento à disposição padrão, entre o `fork` e o
/// `exec`.
///
/// Um sinal **ignorado na entrada do shell não pode ser trapeado**: o Bash
/// recusa `trap ... INT` para ele, silenciosamente. O runner seguiria até o fim
/// como se nada tivesse acontecido, e é o modo de falha mais confuso que existe,
/// porque `kill` devolve zero e erro algum aparece em lugar nenhum.
///
/// A disposição ignorada atravessa o `exec`, então precisa ser desfeita aqui, no
/// filho — não no processo de testes, que não pode alterar a própria sem afetar
/// os demais casos.
fn restaurar_sinais_padrao() -> std::io::Result<()> {
    for sinal in [SIGINT, SIGTERM, SIGHUP, SIGQUIT] {
        // Chamada de sistema sem efeito sobre memória, num filho que ainda não
        // executou nada além do `fork`.
        if unsafe { signal(sinal, SIG_DFL) } == SIG_ERR {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Máscaras de sinal publicadas pelo kernel, para diagnóstico.
///
/// `SigIgn` com o bit do sinal aceso explica, sozinho, um `kill` que devolve
/// zero e não produz efeito nenhum.
fn mascaras_de_sinal(pid: i32) -> String {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .map(|texto| {
            texto
                .lines()
                .filter(|linha| linha.starts_with("Sig"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|_| String::from("<indisponível>"))
}

/// O sinal está ignorado neste processo, segundo o kernel?
///
/// `SigIgn` é máscara hexadecimal com o bit `sinal - 1`.
fn sinal_ignorado(pid: i32, sinal: i32) -> bool {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).expect("status do processo");
    let mascara = status
        .lines()
        .find_map(|linha| linha.strip_prefix("SigIgn:"))
        .map(str::trim)
        .expect("campo SigIgn");
    let bits = u64::from_str_radix(mascara, 16).expect("SigIgn hexadecimal");
    bits & (1u64 << (sinal - 1)) != 0
}

/// Prazo de cada etapa da contenção. Generoso o bastante para uma máquina de CI
/// carregada, curto o bastante para que um vazamento real apareça como falha em
/// vez de travar a suíte.
const PRAZO_DE_CONTENCAO: Duration = Duration::from_secs(10);

/// Falhas da limpeza de contingência, observáveis pelo teste.
///
/// `Drop` não pode entrar em pânico: faria o processo abortar durante o
/// desenrolar de outro pânico e esconderia o original, que é justamente a
/// informação que o teste precisa mostrar. O contador é o canal observável que
/// substitui o pânico proibido.
static FALHAS_DE_LIMPEZA_EM_DROP: AtomicU64 = AtomicU64::new(0);

/// Quantas vezes o `Drop` precisou agir por conta própria.
static CONTENCOES_EM_DROP: AtomicU64 = AtomicU64::new(0);

/// Identidade suficiente para autorizar um sinal.
///
/// O par `(pid, start_time)` é o que o kernel não reutiliza junto; `pgid` e
/// `sid` amarram o processo à árvore que a campanha criou. `comm` entra apenas
/// como rótulo de evidência: **nunca** autoriza sinal.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identidade {
    pid: i32,
    start_time: u64,
    pgid: i32,
    sid: i32,
    comm: String,
}

/// Mesma taxonomia que a autoridade de contenção nativa do repositório usa para
/// classificar dono de sandbox e dono de lock. Manter as duas iguais é
/// intencional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClasseIdentidade {
    /// O PID existe e todos os campos capturados conferem.
    Viva,
    /// `/proc/<pid>` comprovadamente não existe: o processo terminou.
    Ausente,
    /// O PID existe com outro start time: o número passou a nomear outro
    /// processo.
    Reutilizada,
    /// Não foi possível provar a identidade, ou grupo/sessão divergiram do
    /// capturado. **Nunca autoriza nada.**
    Desconhecida,
}

/// Campos de `/proc/<pid>/stat` que identificam o processo.
///
/// O corte usa o **último** `')'`: `comm` aceita espaço e parêntese, e cortar
/// pelo primeiro desloca todos os campos seguintes — inclusive `starttime`, que
/// é exatamente o que distingue um PID vivo de um número herdado.
fn campos_do_stat(pid: i32) -> Option<(String, i32, i32, i32, u64)> {
    let bruto = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let abre = bruto.find('(')?;
    let fecha = bruto.rfind(')')?;
    if fecha <= abre {
        return None;
    }
    let comm = bruto[abre + 1..fecha].to_string();
    let campos: Vec<&str> = bruto[fecha + 1..].split_whitespace().collect();
    // Depois do corte, `campos[0]` é o campo 3 do `stat` (`state`), de modo que
    // o índice do documento menos três dá o índice aqui: `ppid` é o campo 4,
    // `pgrp` o 5, `session` o 6 e `starttime` o 22.
    let ppid = campos.get(1)?.parse::<i32>().ok()?;
    let pgid = campos.get(2)?.parse::<i32>().ok()?;
    let sid = campos.get(3)?.parse::<i32>().ok()?;
    let start_time = campos.get(19)?.parse::<u64>().ok()?;
    Some((comm, ppid, pgid, sid, start_time))
}

fn identidade_de(pid: i32) -> Option<Identidade> {
    let (comm, _ppid, pgid, sid, start_time) = campos_do_stat(pid)?;
    Some(Identidade {
        pid,
        start_time,
        pgid,
        sid,
        comm,
    })
}

/// Reclassifica uma identidade capturada contra o estado atual do sistema.
fn classificar_identidade(identidade: &Identidade) -> ClasseIdentidade {
    if identidade.pid <= 1 {
        return ClasseIdentidade::Desconhecida;
    }
    match campos_do_stat(identidade.pid) {
        None => {
            if Path::new(&format!("/proc/{}", identidade.pid)).exists() {
                // O diretório existe mas o `stat` não pôde ser lido: sem prova,
                // sem autorização.
                ClasseIdentidade::Desconhecida
            } else {
                ClasseIdentidade::Ausente
            }
        }
        Some((_comm, _ppid, pgid, sid, start_time)) => {
            if start_time != identidade.start_time {
                ClasseIdentidade::Reutilizada
            } else if pgid != identidade.pgid || sid != identidade.sid {
                ClasseIdentidade::Desconhecida
            } else {
                ClasseIdentidade::Viva
            }
        }
    }
}

fn pids_vivos() -> Vec<i32> {
    let Ok(entradas) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entradas
        .flatten()
        .filter_map(|entrada| entrada.file_name().to_str()?.parse::<i32>().ok())
        .collect()
}

/// Fecho transitivo de descendentes, montado a partir da tabela de pais.
///
/// Precisa ser tirado enquanto o controlador vive: depois do `SIGKILL` os
/// sobreviventes passam a ter `ppid = 1` e esta função não os encontra mais.
fn descendentes_de(raiz_pid: i32) -> Vec<Identidade> {
    let mut por_pai: HashMap<i32, Vec<Identidade>> = HashMap::new();
    for pid in pids_vivos() {
        if let Some((comm, ppid, pgid, sid, start_time)) = campos_do_stat(pid) {
            por_pai.entry(ppid).or_default().push(Identidade {
                pid,
                start_time,
                pgid,
                sid,
                comm,
            });
        }
    }
    let mut encontrados = Vec::new();
    let mut visitados = HashSet::new();
    let mut fila = vec![raiz_pid];
    while let Some(atual) = fila.pop() {
        if !visitados.insert(atual) {
            continue;
        }
        for filho in por_pai.get(&atual).into_iter().flatten() {
            encontrados.push(filho.clone());
            fila.push(filho.pid);
        }
    }
    encontrados.sort_by_key(|identidade| identidade.pid);
    encontrados
}

/// Envia `sinal` somente se a identidade continuar sendo exatamente a capturada.
///
/// A revalidação acontece imediatamente antes do `kill`, e a classe precisa ser
/// `Viva`: `Reutilizada` significa que o número passou a nomear outro processo,
/// `Ausente` que já morreu, e `Desconhecida` nunca autoriza nada. Devolve se o
/// sinal chegou a partir.
fn sinalizar_identidade(identidade: &Identidade, sinal: i32) -> bool {
    if identidade.pid <= 1 || identidade.pid == std::process::id() as i32 {
        return false;
    }
    if classificar_identidade(identidade) != ClasseIdentidade::Viva {
        return false;
    }
    unsafe { kill(identidade.pid, sinal) == 0 }
}

/// Sinal explícito a um PID conhecido do próprio caso, com falha ruidosa.
///
/// Usada apenas onde o teste acabou de observar o processo vivo e uma falha de
/// entrega é defeito, não corrida.
fn enviar_sinal(pid: i32, sinal: i32) {
    unsafe {
        assert_eq!(kill(pid, sinal), 0, "enviar sinal {sinal} para {pid}");
    }
}

/// Papel de um processo dentro da campanha.
///
/// Existe para **relatar**, não para decidir: a autorização de sinal vem de
/// identidade comprovada, nunca de nome. Um classificador por nome que também
/// autorizasse seria o casamento por nome que este módulo existe para evitar —
/// e a guarda de sensibilidade proíbe até a menção literal das ferramentas que
/// o fazem, de modo que este comentário as descreve sem as nomear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PapelNaCampanha {
    Timeout,
    Harness,
    ShellIntermediario,
    Descendente,
}

fn papel_de(identidade: &Identidade) -> PapelNaCampanha {
    if identidade.comm == "timeout" {
        PapelNaCampanha::Timeout
    } else if identidade.comm.starts_with("harness-falso") {
        PapelNaCampanha::Harness
    } else if matches!(identidade.comm.as_str(), "sh" | "bash" | "dash") {
        PapelNaCampanha::ShellIntermediario
    } else {
        PapelNaCampanha::Descendente
    }
}

/// Resultado observável de um encerramento, para que o teste afirme sobre o que
/// de fato aconteceu em vez de sobre o que deveria ter acontecido.
#[derive(Debug)]
struct RelatorioEncerramento {
    codigo: i32,
    controlador: Identidade,
    sessoes: Vec<i32>,
    membros_antes: Vec<Identidade>,
    controlador_morto: bool,
    sobreviventes_apos_controlador: Vec<Identidade>,
    term_enviados: Vec<i32>,
    kill_enviados: Vec<i32>,
    filho_recolhido: bool,
    restantes: Vec<Identidade>,
    grupo_vazio: bool,
    sessao_vazia: bool,
    /// Quantos membros capturados citavam a raiz temporária do caso. Fator
    /// corroborante de propriedade, nunca autoridade de sinal.
    membros_ligados_a_raiz: usize,
}

impl RelatorioEncerramento {
    fn papeis_sobreviventes(&self) -> Vec<PapelNaCampanha> {
        self.sobreviventes_apos_controlador
            .iter()
            .map(papel_de)
            .collect()
    }

    fn papeis_restantes(&self) -> Vec<PapelNaCampanha> {
        self.restantes.iter().map(papel_de).collect()
    }
}

/// Campanha proprietária viva, controlada pelo teste.
///
/// Mantém o lock enquanto o binário falso dorme, e é encerrada por sinal
/// explícito ao final do caso. A fixture é dona da árvore inteira que criou, e
/// responde por ela mesmo quando o sinal escolhido impede o alvo de se limpar.
struct CampanhaViva {
    filho: Child,
    evidencia: PathBuf,
    lote: PathBuf,
    raiz: PathBuf,
    controlador: Identidade,
    /// Sessões criadas pela campanha: a do controlador e a que o `setsid` do
    /// runner abre por iteração. É a única propriedade que sobrevive ao
    /// reparenting provocado pelo `SIGKILL`.
    sessoes: Vec<i32>,
    /// Última captura da árvore, sempre anterior a qualquer sinal.
    membros: Vec<Identidade>,
    filho_recolhido: bool,
}

impl CampanhaViva {
    fn iniciar(raiz: &Path, modo: &str) -> Self {
        let mut campanha = Self::nascer(raiz, modo, &[]);
        campanha.aguardar_iteracao();
        campanha
    }

    /// Cria a campanha e prova o isolamento, sem esperar pela iteração.
    ///
    /// Os casos da janela de inicialização congelam o runner **antes** de a
    /// árvore existir, de modo que esperar por ela aqui os tornaria
    /// impossíveis. A espera pertence a quem afirma sobre a árvore.
    fn nascer(raiz: &Path, modo: &str, ambiente: &[(&str, String)]) -> Self {
        Self::nascer_com_harness(raiz, modo, ambiente, HARNESS_LONGO_SEGUNDOS)
    }

    fn nascer_com_harness(
        raiz: &Path,
        modo: &str,
        ambiente: &[(&str, String)],
        atraso_segundos: u64,
    ) -> Self {
        Self::nascer_completo(raiz, modo, "1", ambiente, atraso_segundos)
    }

    fn nascer_completo(
        raiz: &Path,
        modo: &str,
        execucoes: &str,
        ambiente: &[(&str, String)],
        atraso_segundos: u64,
    ) -> Self {
        let binario = binario_falso_com_atraso(raiz, RESUMO_COM_TESTE, 0, atraso_segundos);
        let mut comando = Command::new(raiz.join("scripts/pinker-flake-runner.sh"));
        comando
            .args([modo, execucoes])
            .env("PINKER_FLAKE_TEST_BINARY", &binario)
            .env_remove("PINKER_FLAKE_RUN_TIMEOUT_SECONDS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (chave, valor) in ambiente {
            comando.env(chave, valor);
        }
        // Sessão e grupo exclusivos, estabelecidos entre o `fork` e o `exec`.
        // Sem isto a campanha nasce no grupo do `cargo test`, e qualquer
        // contenção por grupo alcançaria o próprio processo de testes.
        unsafe {
            comando.pre_exec(|| {
                if setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Sinal ignorado na entrada do shell não pode ser trapeado, e o
                // contrato de encerramento normal depende dos traps do runner.
                restaurar_sinais_padrao()
            });
        }
        let filho = comando.spawn().expect("iniciar campanha proprietária");
        let pid = filho.id() as i32;
        let evidencia = raiz.join("target/pinker-flake-evidence");
        let mut campanha = Self {
            filho,
            evidencia,
            lote: PathBuf::new(),
            raiz: raiz.to_path_buf(),
            controlador: Identidade {
                pid,
                start_time: 0,
                pgid: 0,
                sid: 0,
                comm: String::new(),
            },
            sessoes: Vec::new(),
            membros: Vec::new(),
            filho_recolhido: false,
        };
        // A partir daqui qualquer pânico passa pelo `Drop`, que contém a árvore
        // já criada: falhar a construção não pode vazar processo.
        campanha.exigir_isolamento();
        campanha
    }

    /// Falha fechada quando a sessão exclusiva não puder ser comprovada.
    ///
    /// O `setsid` roda no filho, depois do `fork`: o pai só o observa quando o
    /// kernel já publicou o novo `sid`. A espera é por estado observado, nunca
    /// por tempo arbitrário.
    fn exigir_isolamento(&mut self) {
        let proprio =
            identidade_de(std::process::id() as i32).expect("identidade do processo de testes");
        let limite = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(identidade) = identidade_de(self.controlador.pid) {
                if identidade.sid == identidade.pid && identidade.pgid == identidade.pid {
                    assert_ne!(
                        identidade.sid, proprio.sid,
                        "a campanha não pode compartilhar sessão com o processo de testes"
                    );
                    assert_ne!(
                        identidade.pgid, proprio.pgid,
                        "a campanha não pode compartilhar grupo com o processo de testes"
                    );
                    self.sessoes.push(identidade.sid);
                    self.controlador = identidade;
                    return;
                }
            } else if matches!(self.filho.try_wait(), Ok(Some(_))) {
                self.filho_recolhido = true;
                panic!("controlador terminou antes de estabelecer sessão exclusiva");
            }
            assert!(
                Instant::now() < limite,
                "controlador não estabeleceu sessão e grupo exclusivos: isolamento falha fechado"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Espera o estado em que os casos afirmam operar: lock adquirido, lote
    /// criado e iteração em curso.
    ///
    /// Parar no marker deixaria uma janela em que o lote ainda não existe e o
    /// `trap` não teria o que preservar.
    fn aguardar_iteracao(&mut self) {
        let limite = Instant::now() + Duration::from_secs(30);
        while Instant::now() < limite {
            if self.evidencia.join(".lock/owner.marker").is_file() {
                if let Some(lote) = lote_em_execucao(&self.evidencia) {
                    self.lote = lote;
                    // O diretório `.running-` nasce **antes** de o runner criar o
                    // `setsid` da iteração. Devolver o controle aqui entregaria
                    // uma campanha cuja árvore ainda está vazia, e a contenção do
                    // SIGKILL depende justamente de conhecê-la.
                    assert!(
                        self.aguardar_arvore(limite),
                        "a iteração começou mas a árvore da campanha não apareceu"
                    );
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("campanha proprietária não chegou a executar uma iteração");
    }

    /// Espera até que a sessão própria da iteração exista e esteja capturada.
    ///
    /// O `setsid` do runner abre uma sessão por iteração, e é exatamente essa que
    /// o grupo do controlador não alcança. A sua presença é, portanto, o sinal de
    /// que a árvore está completa o bastante para ser contida — esperar por ela é
    /// esperar pelo estado que a fixture promete, e não por um tempo arbitrário.
    fn aguardar_arvore(&mut self, limite: Instant) -> bool {
        while Instant::now() < limite {
            self.recapturar_membros();
            // Dois membros na sessão da iteração, não um. O primeiro é o
            // `timeout`; o segundo é o harness que ele executa. Esperar pelo
            // segundo garante que o runner já passou do laço que captura o start
            // time do `timeout` — e é esse start time que o `cleanup_active` do
            // runner exige para poder derrubar a própria árvore. Parar no
            // primeiro devolvia uma campanha que o runner ainda não sabia
            // encerrar, e o encerramento normal vazava `timeout` e harness em
            // silêncio, com o caso terminando verde.
            let internos = self
                .membros
                .iter()
                .filter(|membro| membro.sid != self.controlador.sid)
                .count();
            if internos >= 2 {
                // Uma reabsorção a mais antes de decidir: o monitor pode ter
                // nascido entre a varredura e esta verificação.
                self.absorver_retardatarios();
                // O subshell monitor nasce na sessão do próprio runner.
                // Exigi-lo junto fecha a última janela: sem ele, a espera
                // podia terminar num instante em que a iteração já tem árvore
                // mas o runner ainda não criou o auxiliar que também precisa
                // ser aguardado no encerramento.
                let monitor = self
                    .membros
                    .iter()
                    .any(|membro| membro.sid == self.controlador.sid);
                if monitor {
                    return true;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Captura a árvore viva da campanha e as sessões que ela abriu.
    fn recapturar_membros(&mut self) {
        for descendente in descendentes_de(self.controlador.pid) {
            if !self.sessoes.contains(&descendente.sid) {
                self.sessoes.push(descendente.sid);
            }
            self.registrar_membro(descendente);
        }
        self.absorver_retardatarios();
    }

    /// Recolhe processos que apareceram numa sessão já reconhecida como da
    /// campanha depois da última varredura de ancestralidade.
    fn absorver_retardatarios(&mut self) {
        for identidade in self.membros_por_sessao() {
            self.registrar_membro(identidade);
        }
    }

    /// Registra um processo como membro, uma única vez.
    ///
    /// A identidade de um membro é `(pid, start_time)`; `comm` é rótulo, e um
    /// processo continua sendo o mesmo processo quando troca de nome. O
    /// controlador da iteração troca: ele publica a própria identidade e então
    /// `exec`uta o `timeout`, de modo que a mesma entrada aparece antes como
    /// shell e depois como `timeout`. Guardar as duas faria a espera pela árvore
    /// aceitar **um** processo como se fossem dois, e devolver uma campanha cuja
    /// árvore ainda não existe.
    fn registrar_membro(&mut self, identidade: Identidade) {
        if identidade.pid == self.controlador.pid || identidade.pid <= 1 {
            return;
        }
        if let Some(existente) = self.membros.iter_mut().find(|membro| {
            membro.pid == identidade.pid && membro.start_time == identidade.start_time
        }) {
            // O rótulo mais recente é o que descreve o processo agora.
            *existente = identidade;
            return;
        }
        self.membros.push(identidade);
    }

    /// Processos vivos numa das sessões exclusivas da campanha.
    ///
    /// A sessão é propriedade comprovada: o controlador é líder da sua por
    /// `setsid` desta fixture, e as demais foram abertas por descendentes dele
    /// observados enquanto a ancestralidade ainda existia.
    fn membros_por_sessao(&self) -> Vec<Identidade> {
        let proprio = std::process::id() as i32;
        pids_vivos()
            .into_iter()
            .filter(|pid| *pid > 1 && *pid != proprio && *pid != self.controlador.pid)
            .filter_map(|pid| campos_do_stat(pid).map(|campos| (pid, campos)))
            .filter(|(_, (_, ppid, _, sid, _))| {
                self.sessoes.contains(sid) && self.pai_conhecido(*ppid)
            })
            .map(|(pid, (comm, _ppid, pgid, sid, start_time))| Identidade {
                pid,
                start_time,
                pgid,
                sid,
                comm,
            })
            .collect()
    }

    /// O pai é o controlador ou um membro já capturado?
    ///
    /// Um número de sessão é reciclável, e o kernel recicla depressa sob
    /// campanha — nesta VM o contador de PID dá a volta em poucas horas. A
    /// sessão sozinha, portanto, não prova propriedade. Exigir que o **pai**
    /// seja identidade já conhecida distingue o neto legítimo, forkado entre a
    /// captura e o sinal, do processo alheio que apenas herdou o número.
    fn pai_conhecido(&self, ppid: i32) -> bool {
        ppid == self.controlador.pid || self.membros.iter().any(|membro| membro.pid == ppid)
    }

    /// A identidade é comprovadamente da campanha?
    ///
    /// Ou já foi capturada com o mesmo start time, ou o seu pai é identidade
    /// conhecida. Qualquer outra coisa que apenas compartilhe grupo ou sessão é
    /// número reciclado, e número reciclado não é resíduo.
    fn pertence_a_campanha(&self, identidade: &Identidade) -> bool {
        if self.membros.iter().any(|membro| {
            membro.pid == identidade.pid && membro.start_time == identidade.start_time
        }) {
            return true;
        }
        campos_do_stat(identidade.pid)
            .map(|(_, ppid, _, _, _)| self.pai_conhecido(ppid))
            .unwrap_or(false)
    }

    /// Vínculo de um processo com a raiz temporária deste caso.
    ///
    /// Fator **corroborante**, registrado como evidência. Não autoriza sinal por
    /// si: a autorização vem de identidade comprovada e de pertencer a uma
    /// sessão que esta fixture criou. Um processo pode ser legitimamente da
    /// campanha e não citar a raiz — o `sleep` do harness herda o diretório de
    /// trabalho do runner, não da raiz isolada.
    fn relacionado_a_raiz(&self, identidade: &Identidade) -> bool {
        let alvo = self.raiz.to_string_lossy().into_owned();
        for campo in ["cwd", "exe"] {
            if let Ok(destino) = fs::read_link(format!("/proc/{}/{campo}", identidade.pid)) {
                if destino.to_string_lossy().contains(&alvo) {
                    return true;
                }
            }
        }
        fs::read_to_string(format!("/proc/{}/cmdline", identidade.pid))
            .map(|linha| linha.contains(&alvo))
            .unwrap_or(false)
    }

    /// Membros capturados cuja identidade ainda confere exatamente.
    fn sobreviventes(&self) -> Vec<Identidade> {
        self.membros
            .iter()
            .filter(|membro| classificar_identidade(membro) == ClasseIdentidade::Viva)
            .cloned()
            .collect()
    }

    /// Processos vivos no grupo do controlador, por varredura independente.
    fn vivos_no_grupo(&self) -> Vec<Identidade> {
        pids_vivos()
            .into_iter()
            .filter_map(identidade_de)
            .filter(|identidade| identidade.pgid == self.controlador.pgid)
            .filter(|identidade| self.pertence_a_campanha(identidade))
            .collect()
    }

    /// Processos vivos em qualquer sessão da campanha, por varredura
    /// independente da lista de membros.
    fn vivos_nas_sessoes(&self) -> Vec<Identidade> {
        pids_vivos()
            .into_iter()
            .filter_map(identidade_de)
            .filter(|identidade| self.sessoes.contains(&identidade.sid))
            .filter(|identidade| self.pertence_a_campanha(identidade))
            .collect()
    }

    fn viva(&mut self) -> bool {
        matches!(self.filho.try_wait(), Ok(None))
    }

    fn marker(&self) -> String {
        fs::read_to_string(self.evidencia.join(".lock/owner.marker")).expect("marker do lock")
    }

    fn encerrar(self, sinal: i32) -> i32 {
        self.encerrar_com_relatorio(sinal).codigo
    }

    /// Encerra a campanha e devolve o que foi observado em cada etapa.
    fn encerrar_com_relatorio(mut self, sinal: i32) -> RelatorioEncerramento {
        let capturado = self.capturar_antes_do_sinal();
        enviar_sinal(self.controlador.pid, sinal);
        self.colher_relatorio(capturado)
    }

    /// Captura a árvore e prova a identidade do controlador **antes** de
    /// qualquer sinal.
    ///
    /// Depois do `SIGKILL` não há mais ancestralidade que reconstrua a árvore, e
    /// depois de um sinal qualquer a campanha pode terminar a qualquer instante:
    /// afirmar sobre o controlador vivo só é determinístico antes.
    fn capturar_antes_do_sinal(&mut self) -> (Vec<Identidade>, usize) {
        self.recapturar_membros();
        let membros_antes = self.membros.clone();
        let membros_ligados_a_raiz = membros_antes
            .iter()
            .filter(|membro| self.relacionado_a_raiz(membro))
            .count();
        assert_eq!(
            classificar_identidade(&self.controlador),
            ClasseIdentidade::Viva,
            "o controlador precisa manter a identidade capturada antes de receber sinal"
        );
        assert_eq!(
            self.controlador.sid, self.controlador.pid,
            "o controlador precisa continuar líder da própria sessão"
        );
        (membros_antes, membros_ligados_a_raiz)
    }

    /// Recolhe e relata um encerramento cujo sinal já partiu.
    fn colher_relatorio(mut self, capturado: (Vec<Identidade>, usize)) -> RelatorioEncerramento {
        let (membros_antes, membros_ligados_a_raiz) = capturado;
        let codigo = self.recolher_filho_direto();
        let controlador_morto = classificar_identidade(&self.controlador) != ClasseIdentidade::Viva;

        // Só agora os remanescentes são tratados, e sempre por identidade.
        self.absorver_retardatarios();
        let sobreviventes_apos_controlador = self.sobreviventes();
        let contencao = self.conter_remanescentes(PRAZO_DE_CONTENCAO);

        let relatorio = RelatorioEncerramento {
            codigo,
            controlador: self.controlador.clone(),
            sessoes: self.sessoes.clone(),
            membros_antes,
            controlador_morto,
            sobreviventes_apos_controlador,
            term_enviados: contencao.term,
            kill_enviados: contencao.kill,
            filho_recolhido: self.filho_recolhido,
            grupo_vazio: self.vivos_no_grupo().is_empty(),
            sessao_vazia: self.vivos_nas_sessoes().is_empty(),
            membros_ligados_a_raiz,
            restantes: contencao.restantes,
        };
        assert!(
            relatorio.restantes.is_empty(),
            "a fixture não conseguiu conter a própria campanha: {:?}",
            relatorio.restantes
        );
        relatorio
    }

    /// `wait` no filho direto.
    ///
    /// Sem isto o controlador vira zumbi dentro do processo de testes, que é
    /// resíduo tanto quanto um processo vivo.
    fn recolher_filho_direto(&mut self) -> i32 {
        if self.filho_recolhido {
            return -1;
        }
        let status = self.filho.wait().expect("aguardar campanha proprietária");
        self.filho_recolhido = true;
        status.code().unwrap_or(-1)
    }

    /// Encerra os membros comprovados que sobreviveram ao controlador.
    ///
    /// `TERM` primeiro, para que quem tiver trap possa preservar evidência;
    /// `KILL` só nos que continuarem sendo comprovadamente a mesma identidade
    /// depois do prazo. Devolve os PIDs sinalizados em cada etapa e o que restou.
    fn conter_remanescentes(&mut self, prazo: Duration) -> ResultadoContencao {
        let term = self.fase_de_contencao(SIGTERM, prazo / 3);
        let kill = self.fase_de_contencao(SIGKILL, prazo / 3);
        self.drenar(prazo / 3);
        ResultadoContencao {
            term,
            kill,
            restantes: self.sobreviventes(),
        }
    }

    /// Dreno final, por varredura independente da lista de membros.
    ///
    /// Confirma que nem o grupo nem as sessões guardam processo comprovadamente
    /// da campanha, e escala o que ainda aparecer. Cobre a janela de
    /// microssegundos entre o último `fork` de um pai moribundo e a checagem de
    /// vazio — janela pequena, mas que sob cem execuções paralelas deixa de ser
    /// hipotética.
    fn drenar(&mut self, prazo: Duration) {
        let limite = Instant::now() + prazo;
        loop {
            self.absorver_retardatarios();
            for alvo in self.sobreviventes() {
                sinalizar_identidade(&alvo, SIGKILL);
            }
            if self.vivos_no_grupo().is_empty() && self.vivos_nas_sessoes().is_empty() {
                return;
            }
            if Instant::now() >= limite {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Uma fase de contenção: sinaliza, reabsorve e repete até a árvore esvaziar
    /// ou o prazo expirar.
    ///
    /// O laço não é zelo excessivo. Enquanto o `timeout` da iteração continua
    /// vivo, o subshell monitor do runner segue forkando `ps`, `find` e `wc` a
    /// cada 50 ms; um neto nascido entre a captura e o sinal pertence à campanha
    /// tanto quanto o pai que o criou, e uma passada única o deixaria para trás.
    /// Era exatamente essa a origem das cinco falhas de `grupo_vazio` em oitenta
    /// e quatro execuções paralelas.
    fn fase_de_contencao(&mut self, sinal: i32, prazo: Duration) -> Vec<i32> {
        let mut sinalizados: Vec<i32> = Vec::new();
        let limite = Instant::now() + prazo;
        loop {
            self.absorver_retardatarios();
            let vivos = self.sobreviventes();
            if vivos.is_empty() {
                break;
            }
            for alvo in &vivos {
                if !sinalizados.contains(&alvo.pid) && sinalizar_identidade(alvo, sinal) {
                    sinalizados.push(alvo.pid);
                }
            }
            if Instant::now() >= limite {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        sinalizados.sort_unstable();
        sinalizados
    }
}

/// O que uma contenção sinalizou e o que sobrou dela.
#[derive(Debug, Default)]
struct ResultadoContencao {
    term: Vec<i32>,
    kill: Vec<i32>,
    restantes: Vec<Identidade>,
}

impl Drop for CampanhaViva {
    /// Defesa secundária. A autoridade principal continua sendo `encerrar`.
    ///
    /// Nunca entra em pânico e nunca oculta o pânico original: uma falha de
    /// limpeza aqui é registrada em contador observável, e o teste que quiser
    /// afirmar sobre ela lê o contador.
    fn drop(&mut self) {
        // Fechar os pipes antes de qualquer espera: um filho bloqueado
        // escrevendo em pipe cheio nunca morreria por TERM.
        drop(self.filho.stdout.take());
        drop(self.filho.stderr.take());
        drop(self.filho.stdin.take());

        if !self.filho_recolhido {
            CONTENCOES_EM_DROP.fetch_add(1, Ordering::Relaxed);
            if classificar_identidade(&self.controlador) == ClasseIdentidade::Viva {
                sinalizar_identidade(&self.controlador, SIGTERM);
                let limite = Instant::now() + PRAZO_DE_CONTENCAO;
                while Instant::now() < limite
                    && classificar_identidade(&self.controlador) == ClasseIdentidade::Viva
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                sinalizar_identidade(&self.controlador, SIGKILL);
            }
            if self.filho.wait().is_ok() {
                self.filho_recolhido = true;
            }
        }

        let contencao = self.conter_remanescentes(PRAZO_DE_CONTENCAO);
        if !contencao.restantes.is_empty() || !self.filho_recolhido {
            FALHAS_DE_LIMPEZA_EM_DROP.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// pinker-contencao:fim

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
    let binario = publicar_executavel(
        &raiz.join("harness-sentinela.sh"),
        &format!(
            "#!/usr/bin/env bash\nprintf 'sim\\n' > {}\nexit 0\n",
            sentinela.display()
        ),
    );

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

    let substituto = marker_valido("1", "1", "outra", "lote-outro");
    let gancho = publicar_executavel(
        &raiz.join("gancho.sh"),
        &format!(
            "#!/usr/bin/env bash\nprintf '%s' {} > \"$2/owner.marker\"\nexit 0\n",
            aspas_simples(&substituto)
        ),
    );

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
    let relatorio = dona.encerrar_com_relatorio(sinal);
    let codigo = relatorio.codigo;
    assert_eq!(codigo, EXIT_INTERROMPIDO, "{caso}: saída de interrupção");

    // O encerramento normal permite ao runner rodar os próprios traps, e é ele
    // quem derruba a árvore. A fixture confirma o resultado; não o substitui.
    assert!(
        relatorio.filho_recolhido,
        "{caso}: o filho direto precisa ser aguardado"
    );
    assert!(
        relatorio.controlador_morto,
        "{caso}: o controlador precisa ter terminado"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "{caso}: nenhum descendente pode sobreviver: {:?}",
        relatorio.papeis_restantes()
    );
    assert!(
        relatorio.grupo_vazio,
        "{caso}: o grupo do controlador precisa ficar vazio"
    );
    assert!(
        relatorio.sessao_vazia,
        "{caso}: as sessões da campanha precisam ficar sem membros"
    );
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
    caso_sinal_libera_lock("lock-sigterm", SIGTERM);
}

#[test]
fn sighup_libera_o_lock_e_preserva_evidencia() {
    caso_sinal_libera_lock("lock-sighup", 1);
}

// --- isolamento e contenção da campanha ------------------------------------

#[test]
fn campanha_nasce_em_sessao_e_grupo_exclusivos() {
    let raiz = raiz_isolada("campanha-isolada");
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let proprio = identidade_de(std::process::id() as i32).expect("identidade própria");

    let controlador = dona.controlador.clone();
    assert_eq!(
        controlador.sid, controlador.pid,
        "o controlador precisa liderar a própria sessão"
    );
    assert_eq!(
        controlador.pgid, controlador.pid,
        "o controlador precisa liderar o próprio grupo"
    );
    assert_ne!(
        controlador.sid, proprio.sid,
        "sessão compartilhada com o cargo test"
    );
    assert_ne!(
        controlador.pgid, proprio.pgid,
        "grupo compartilhado com o cargo test"
    );
    assert!(
        controlador.start_time > 0,
        "start time precisa ser capturado"
    );
    assert!(
        !dona.membros.is_empty(),
        "a árvore da campanha precisa ser capturada na criação"
    );
    dona.encerrar(SIGTERM);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn campanha_nasce_com_sinais_de_encerramento_no_padrao() {
    // Um sinal ignorado na entrada do shell **não pode ser trapeado**: o Bash
    // recusa `trap ... INT` silenciosamente, e o runner seguiria até o fim como
    // se nada tivesse acontecido, com `kill` devolvendo zero o tempo todo. O
    // contrato de encerramento normal inteiro depende desta máscara.
    let raiz = raiz_isolada("sinais-no-padrao");
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let pid = dona.controlador.pid;
    // `SIGQUIT` fica de fora de propósito: o Bash não interativo o ignora por
    // decisão própria, e não por herança. Afirmar sobre ele codificaria uma
    // expectativa falsa — a medição mostra `SigIgn: 0x4` num controlador
    // perfeitamente saudável, cujo `SigCgt` traz `SIGINT`, `SIGTERM` e `SIGHUP`
    // capturados.
    for (sinal, nome) in [(SIGINT, "SIGINT"), (SIGTERM, "SIGTERM"), (SIGHUP, "SIGHUP")] {
        assert!(
            !sinal_ignorado(pid, sinal),
            "{nome} ignorado no controlador: o trap do runner seria no-op \
             silencioso; mascaras={}",
            mascaras_de_sinal(pid)
        );
    }
    // E o contrato vale na prática, não só na máscara.
    assert_eq!(dona.encerrar(SIGINT), EXIT_INTERROMPIDO);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn controlador_timeout_e_harness_pertencem_a_arvore_capturada() {
    let raiz = raiz_isolada("arvore-capturada");
    // `iniciar` só devolve o controle depois que a sessão da iteração existe e
    // está capturada: não há laço de espera aqui porque a espera é contrato da
    // fixture, não responsabilidade de cada caso.
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    let papeis: Vec<PapelNaCampanha> = dona.membros.iter().map(papel_de).collect();
    assert!(
        papeis.contains(&PapelNaCampanha::Timeout),
        "o `timeout` criado pelo runner precisa entrar na captura: {papeis:?}"
    );
    assert!(
        papeis.len() >= 2,
        "a árvore tem mais que o controlador: {papeis:?}"
    );
    // Toda a árvore pertence a sessões que esta fixture reconhece como suas.
    for membro in &dona.membros {
        assert!(
            dona.sessoes.contains(&membro.sid),
            "membro fora das sessões da campanha: {membro:?}"
        );
    }
    // O `timeout` vive numa sessão própria, criada pelo `setsid` do runner: é
    // exatamente por isso que o grupo do controlador não basta para alcançá-lo.
    let sessoes_distintas: HashSet<i32> = dona.membros.iter().map(|m| m.sid).collect();
    assert!(
        sessoes_distintas.len() >= 2,
        "o runner abre sessão própria por iteração: {sessoes_distintas:?}"
    );
    dona.encerrar(SIGTERM);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn sigkill_mata_o_controlador_e_a_fixture_limpa_os_descendentes() {
    // O defeito corrigido: `SIGKILL` não executa trap, de modo que o runner não
    // derruba a própria árvore. Sem esta limpeza, cada execução deste caso
    // deixava `timeout`, harness, `sleep` e o subshell monitor vivos — foi o que
    // produziu os dezesseis órfãos observados na PR 424.
    let raiz = raiz_isolada("sigkill-limpa-arvore");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let dona = CampanhaViva::iniciar(&raiz, "morta");
    let relatorio = dona.encerrar_com_relatorio(SIGKILL);

    assert!(
        relatorio.controlador_morto,
        "o SIGKILL precisa ter matado o controlador"
    );
    assert_ne!(
        classificar_identidade(&relatorio.controlador),
        ClasseIdentidade::Viva,
        "a identidade capturada do controlador não pode continuar viva"
    );
    assert!(
        relatorio.filho_recolhido,
        "o filho direto precisa ser aguardado, sob pena de zumbi no cargo test"
    );
    assert!(
        !relatorio.membros_antes.is_empty(),
        "a árvore precisa ter sido capturada antes do sinal"
    );
    assert!(
        !relatorio.sobreviventes_apos_controlador.is_empty(),
        "matar apenas o controlador não derruba a árvore: é este o defeito que a \
         limpeza da fixture existe para cobrir"
    );
    assert!(
        relatorio
            .papeis_sobreviventes()
            .contains(&PapelNaCampanha::Timeout),
        "o `timeout` sobrevive ao controlador: {:?}",
        relatorio.papeis_sobreviventes()
    );
    assert!(
        !relatorio.term_enviados.is_empty(),
        "os remanescentes precisam receber TERM antes de KILL"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "nenhum resíduo pode sobreviver à fixture: {:?}",
        relatorio.restantes
    );
    assert!(
        relatorio.grupo_vazio,
        "grupo do controlador precisa esvaziar"
    );
    assert!(
        relatorio.sessao_vazia,
        "as sessões da campanha precisam ficar sem membros"
    );
    for papel in [
        PapelNaCampanha::Timeout,
        PapelNaCampanha::Harness,
        PapelNaCampanha::ShellIntermediario,
        PapelNaCampanha::Descendente,
    ] {
        assert!(
            !relatorio.papeis_restantes().contains(&papel),
            "papel {papel:?} sobreviveu à limpeza"
        );
    }
    assert!(
        relatorio.membros_ligados_a_raiz > 0,
        "ao menos um membro precisa citar a raiz temporária do caso"
    );
    assert!(
        relatorio.sessoes.len() >= 2,
        "a campanha registra a própria sessão e a que o runner abre: {:?}",
        relatorio.sessoes
    );

    // O lock é objeto deste teste e precisa sobreviver ao SIGKILL.
    assert!(
        evidencia.join(".lock/owner.marker").is_file(),
        "SIGKILL deixa o lock para trás"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn lock_deixado_por_sigkill_e_recuperavel() {
    // SIGKILL não executa trap: o lock sobrevive ao proprietário. A identidade
    // registrada é o que permite a uma campanha posterior classificá-lo.
    let raiz = raiz_isolada("lock-sigkill");
    let dona = CampanhaViva::iniciar(&raiz, "morta");
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let lote_morto = dona.lote.clone();
    let relatorio = dona.encerrar_com_relatorio(SIGKILL);
    assert!(
        evidencia.join(".lock/owner.marker").is_file(),
        "SIGKILL deixa o lock para trás"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "a recuperação do lock não pode depender de resíduos vivos: {:?}",
        relatorio.restantes
    );

    let seguinte = executar_com_lock(&raiz, "seguinte");
    assert_eq!(seguinte.codigo, 0, "stderr={}", seguinte.saida_erro);
    assert!(seguinte.saida_erro.contains("lock obsoleto recuperado"));
    assert!(!evidencia.join(".lock").exists(), "zero locks ao final");

    // O lote da campanha recuperadora conclui e não deixa iteração em curso. O
    // lote morto conserva o seu `.running-`: `SIGKILL` não executa trap, logo
    // ninguém o promoveu a `INTERRUPTED-`, e esse diretório é **evidência
    // preservada** do que a campanha estava fazendo quando morreu — não resíduo.
    // Recuperar o lock é competência do runner; reescrever a evidência de uma
    // campanha morta não é, e o produto não foi alterado para fingir que é.
    let lotes = seguinte.lotes();
    assert_eq!(lotes.len(), 2, "o lote morto e o recuperador: {lotes:?}");
    let em_curso: Vec<PathBuf> = lotes
        .iter()
        .filter(|lote| {
            diretorios_de(lote).iter().any(|caminho| {
                caminho
                    .file_name()
                    .map(|nome| nome.to_string_lossy().starts_with(".running-"))
                    .unwrap_or(false)
            })
        })
        .cloned()
        .collect();
    assert_eq!(
        em_curso,
        vec![lote_morto],
        "somente o lote morto pelo SIGKILL preserva a iteração em curso"
    );
    let recuperador = lotes
        .iter()
        .find(|lote| **lote != em_curso[0])
        .expect("lote da campanha recuperadora");
    assert!(
        recuperador.join("SUMMARY.txt").is_file(),
        "a campanha recuperadora precisa concluir o próprio lote"
    );
    let vivos: Vec<Identidade> = pids_vivos()
        .into_iter()
        .filter_map(identidade_de)
        .filter(|identidade| relatorio.sessoes.contains(&identidade.sid))
        .collect();
    assert!(
        vivos.is_empty(),
        "processos residuais da campanha: {vivos:?}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// --- proteção de identidade: o que a fixture se recusa a sinalizar ---------

/// Processo externo à campanha, do mesmo usuário, para provar que a contenção
/// não o alcança.
fn processo_externo() -> (Child, Identidade) {
    let filho = Command::new("/bin/sleep")
        .arg("120")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("iniciar processo externo");
    let pid = filho.id() as i32;
    let limite = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(identidade) = identidade_de(pid) {
            return (filho, identidade);
        }
        assert!(
            Instant::now() < limite,
            "processo externo não apareceu em /proc"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn encerrar_externo(mut filho: Child, identidade: &Identidade) {
    assert!(
        sinalizar_identidade(identidade, SIGKILL),
        "o próprio caso encerra o que criou"
    );
    filho.wait().expect("recolher processo externo");
}

#[test]
fn identidade_divergente_nao_e_sinalizada() {
    let (filho, identidade) = processo_externo();
    let divergente = Identidade {
        start_time: identidade.start_time + 1,
        ..identidade.clone()
    };
    assert_eq!(
        classificar_identidade(&divergente),
        ClasseIdentidade::Reutilizada,
        "start time diferente significa outro processo sob o mesmo número"
    );
    assert!(
        !sinalizar_identidade(&divergente, SIGKILL),
        "identidade divergente nunca é sinalizada"
    );
    assert_eq!(
        classificar_identidade(&identidade),
        ClasseIdentidade::Viva,
        "o processo real precisa seguir intacto"
    );
    encerrar_externo(filho, &identidade);
}

#[test]
fn pid_reutilizado_nao_e_sinalizado() {
    // Um PID já recolhido e um start time fabricado: a classe precisa provar a
    // morte, e nenhuma das provas positivas autoriza sinal.
    let livre = pid_encerrado();
    let fantasma = Identidade {
        pid: livre,
        start_time: 1,
        pgid: livre,
        sid: livre,
        comm: String::from("fantasma"),
    };
    assert!(matches!(
        classificar_identidade(&fantasma),
        ClasseIdentidade::Ausente | ClasseIdentidade::Reutilizada
    ));
    assert!(
        !sinalizar_identidade(&fantasma, SIGKILL),
        "PID sem identidade comprovada nunca é sinalizado"
    );
}

#[test]
fn pgid_divergente_nao_e_sinalizado() {
    let (filho, identidade) = processo_externo();
    let divergente = Identidade {
        pgid: identidade.pgid + 1,
        ..identidade.clone()
    };
    assert_eq!(
        classificar_identidade(&divergente),
        ClasseIdentidade::Desconhecida,
        "grupo divergente torna a propriedade ambígua"
    );
    assert!(!sinalizar_identidade(&divergente, SIGKILL));
    assert_eq!(classificar_identidade(&identidade), ClasseIdentidade::Viva);
    encerrar_externo(filho, &identidade);
}

#[test]
fn sid_divergente_nao_e_sinalizado() {
    let (filho, identidade) = processo_externo();
    let divergente = Identidade {
        sid: identidade.sid + 1,
        ..identidade.clone()
    };
    assert_eq!(
        classificar_identidade(&divergente),
        ClasseIdentidade::Desconhecida,
        "sessão divergente torna a propriedade ambígua"
    );
    assert!(!sinalizar_identidade(&divergente, SIGKILL));
    encerrar_externo(filho, &identidade);
}

#[test]
fn grupo_desconhecido_falha_fechado() {
    // `unknown` é a classe que nunca autoriza nada. PID não positivo e o init
    // são os dois casos que a fixture precisa recusar sem hesitar.
    for pid in [0, -1, 1] {
        let opaco = Identidade {
            pid,
            start_time: 1,
            pgid: pid,
            sid: pid,
            comm: String::from("opaco"),
        };
        assert_eq!(
            classificar_identidade(&opaco),
            ClasseIdentidade::Desconhecida,
            "pid {pid} não pode ser classificado como próprio"
        );
        assert!(
            !sinalizar_identidade(&opaco, SIGTERM),
            "pid {pid} nunca é sinalizado"
        );
    }
}

#[test]
fn processo_externo_no_mesmo_usuario_nao_e_tocado() {
    let (externo, identidade_externa) = processo_externo();
    let raiz = raiz_isolada("externo-intocado");
    let dona = CampanhaViva::iniciar(&raiz, "modo");
    assert!(
        !dona.sessoes.contains(&identidade_externa.sid),
        "o processo externo não pode ser confundido com a campanha"
    );
    let relatorio = dona.encerrar_com_relatorio(SIGKILL);
    assert!(
        !relatorio.term_enviados.contains(&identidade_externa.pid),
        "o processo externo não pode receber TERM"
    );
    assert!(
        !relatorio.kill_enviados.contains(&identidade_externa.pid),
        "o processo externo não pode receber KILL"
    );
    assert_eq!(
        classificar_identidade(&identidade_externa),
        ClasseIdentidade::Viva,
        "o processo externo precisa sobreviver intacto à contenção"
    );
    encerrar_externo(externo, &identidade_externa);
    let _ = fs::remove_dir_all(&raiz);
}

/// Identidade do controlador e sessões da campanha, publicadas pelo fecho que
/// entra em pânico para que o caso possa afirmar sobre elas depois do `Drop`.
type ObservacaoCompartilhada = Arc<Mutex<Option<(Identidade, Vec<i32>)>>>;

#[test]
fn limpeza_de_contingencia_em_drop_apos_panico_controlado() {
    let raiz = raiz_isolada("drop-panico");
    let contencoes_antes = CONTENCOES_EM_DROP.load(Ordering::Relaxed);
    let observado: ObservacaoCompartilhada = Arc::new(Mutex::new(None));

    let alvo = Arc::clone(&observado);
    let caminho = raiz.clone();
    let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let dona = CampanhaViva::iniciar(&caminho, "modo");
        *alvo.lock().expect("registrar identidade") =
            Some((dona.controlador.clone(), dona.sessoes.clone()));
        // Falha de asserção com a campanha viva: só o `Drop` pode contê-la.
        panic!("panico controlado da regressao de Drop");
    }));
    assert!(resultado.is_err(), "o pânico precisa ter ocorrido");

    let (controlador, sessoes) = observado
        .lock()
        .expect("ler identidade")
        .clone()
        .expect("a campanha chegou a existir");

    assert_ne!(
        classificar_identidade(&controlador),
        ClasseIdentidade::Viva,
        "o Drop precisa ter encerrado o controlador"
    );
    let vivos: Vec<Identidade> = pids_vivos()
        .into_iter()
        .filter_map(identidade_de)
        .filter(|identidade| sessoes.contains(&identidade.sid))
        .collect();
    assert!(
        vivos.is_empty(),
        "o Drop precisa ter esvaziado as sessões da campanha: {vivos:?}"
    );
    assert!(
        CONTENCOES_EM_DROP.load(Ordering::Relaxed) > contencoes_antes,
        "a contingência precisa ser observável pelo teste"
    );
    assert_eq!(
        FALHAS_DE_LIMPEZA_EM_DROP.load(Ordering::Relaxed),
        0,
        "nenhuma limpeza de contingência pode ter falhado"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// Janela de interrupção ao iniciar a campanha.
//
// O runner criava o controlador e só depois capturava o seu start time. Os
// `trap` de INT, TERM e HUP já estavam ativos durante essa sequência, e o
// encerramento começava por uma validação que exigia a identidade completa.
// Um sinal recebido depois do `setsid` e antes da captura encontrava a
// validação falhando: o encerramento voltava sem matar nada, o runner saía com
// `130`, liberava o lock e preservava a evidência como interrompida, deixando
// `timeout`, harness e descendentes vivos. Interrupção correta por fora, árvore
// viva por dentro.
//
// A janela é fechada por construção, nunca por temporização: durante `starting`
// o sinal é registrado como interrupção pendente e só é processado depois de
// PID, start time, PGID e SID capturados, revalidados e promovidos.
//
// A sincronização destes casos é causal, não temporal. O gancho de
// inicialização congela o runner num ponto exato lendo uma linha da entrada
// padrão; o caso envia o sinal enquanto o runner está bloqueado e só então
// libera a barreira. `sleep` nenhum coordena coisa alguma, e a inspeção da
// árvore por varredura serve apenas de confirmação, com prazo limitado.
// ---------------------------------------------------------------------------

/// Ponto da inicialização em que a barreira congela o runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EstagioDeInicializacao {
    AntesDoSpawn,
    DepoisDoSpawn,
    DepoisDaIdentidade,
    DepoisDeAtivo,
    DepoisDoMonitor,
    DuranteReaping,
    DuranteExitTrap,
}

impl EstagioDeInicializacao {
    fn nome(self) -> &'static str {
        match self {
            Self::AntesDoSpawn => "before-spawn",
            Self::DepoisDoSpawn => "after-spawn",
            Self::DepoisDaIdentidade => "after-identity",
            Self::DepoisDeAtivo => "after-active",
            Self::DepoisDoMonitor => "after-monitor",
            Self::DuranteReaping => "reap-begin",
            Self::DuranteExitTrap => "exit-trap-begin",
        }
    }

    /// O controlador já foi criado quando a barreira segura este estágio?
    fn controlador_existe(self) -> bool {
        !matches!(self, Self::AntesDoSpawn)
    }

    /// Estado em que o runner recebe o sinal, tal como registrado no manifesto.
    fn estado_esperado(self) -> &'static str {
        match self {
            Self::AntesDoSpawn | Self::DepoisDoSpawn | Self::DepoisDaIdentidade => "starting",
            Self::DepoisDeAtivo
            | Self::DepoisDoMonitor
            | Self::DuranteReaping
            | Self::DuranteExitTrap => "active",
        }
    }
}

/// Prazo das confirmações por varredura. Nunca é sincronização: a barreira já
/// garantiu a ordem, e isto só espera o efeito aparecer.
const PRAZO_DE_CONFIRMACAO: Duration = Duration::from_secs(30);

/// Campanha congelada em um ou mais pontos exatos da inicialização.
///
/// Vários pontos existem porque a ordem de entrega de sinais simultaneamente
/// pendentes é do kernel, não de quem envia: no Linux, o menor número vence.
/// Provar que a **primeira causa** é preservada exige, portanto, que os sinais
/// cheguem em instantes distintos e ordenados, e é a barreira que os ordena.
struct CampanhaNaJanela {
    campanha: CampanhaViva,
    area: PathBuf,
    estagios: Vec<EstagioDeInicializacao>,
    atual: usize,
    capturado: Option<(Vec<Identidade>, usize)>,
}

impl CampanhaNaJanela {
    fn nova(raiz: &Path, modo: &str, estagio: EstagioDeInicializacao) -> Self {
        Self::nova_com_harness(raiz, modo, &[estagio], HARNESS_LONGO_SEGUNDOS)
    }

    fn nova_multi(raiz: &Path, modo: &str, estagios: &[EstagioDeInicializacao]) -> Self {
        Self::nova_com_harness(raiz, modo, estagios, HARNESS_LONGO_SEGUNDOS)
    }

    fn nova_com_harness(
        raiz: &Path,
        modo: &str,
        estagios: &[EstagioDeInicializacao],
        atraso_segundos: u64,
    ) -> Self {
        Self::nova_completa(raiz, modo, "1", estagios, atraso_segundos)
    }

    fn nova_completa(
        raiz: &Path,
        modo: &str,
        execucoes: &str,
        estagios: &[EstagioDeInicializacao],
        atraso_segundos: u64,
    ) -> Self {
        assert!(!estagios.is_empty(), "a barreira precisa de um estágio");
        let unico = SEQUENCIA.fetch_add(1, Ordering::Relaxed);
        let area = raiz.join(format!("barreira-{unico}"));
        fs::create_dir_all(&area).expect("criar área da barreira");

        // O gancho publica a chegada do estágio por rename atômico e então
        // bloqueia lendo uma linha da entrada padrão, que é o cano deste
        // processo. Enquanto ele não retornar, o runner não avança um único
        // comando — é essa a barreira, e ela é causal, não temporal.
        let gancho = publicar_executavel(
            &raiz.join(format!("gancho-inicializacao-{unico}.sh")),
            &format!(
                "#!/usr/bin/env bash\nalvo={}/chegou-$1\nprintf '%s\\n' \"$1\" > \"$alvo.parcial\"\nmv -f -- \"$alvo.parcial\" \"$alvo\"\nIFS= read -r _liberado\nexit 0\n",
                aspas_simples(area.to_str().expect("utf-8")),
            ),
        );

        let nomes: Vec<&str> = estagios.iter().map(|estagio| estagio.nome()).collect();
        let ambiente = vec![
            (
                "PINKER_FLAKE_STARTUP_HOOK",
                gancho.to_str().expect("utf-8").to_string(),
            ),
            ("PINKER_FLAKE_STARTUP_HOOK_STAGES", nomes.join(" ")),
            (
                "PINKER_FLAKE_TRACE_FILE",
                area.join("trace.tsv").to_str().expect("utf-8").to_string(),
            ),
        ];
        let campanha =
            CampanhaViva::nascer_completo(raiz, modo, execucoes, &ambiente, atraso_segundos);
        let mut janela = Self {
            campanha,
            area,
            estagios: estagios.to_vec(),
            atual: 0,
            capturado: None,
        };
        janela.aguardar_barreira();
        janela
    }

    fn estagio(&self) -> EstagioDeInicializacao {
        self.estagios[self.atual.min(self.estagios.len() - 1)]
    }

    /// Espera o runner chegar ao estágio corrente e ficar bloqueado nele.
    fn aguardar_barreira(&mut self) {
        let estagio = self.estagio();
        let chegada = self.area.join(format!("chegou-{}", estagio.nome()));
        let limite = Instant::now() + PRAZO_DE_CONFIRMACAO;
        while Instant::now() < limite {
            if chegada.is_file() {
                return;
            }
            if matches!(self.campanha.filho.try_wait(), Ok(Some(_))) {
                self.campanha.filho_recolhido = true;
                panic!(
                    "o runner terminou antes de alcançar o estágio {}",
                    estagio.nome()
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("o runner não alcançou o estágio {}", estagio.nome());
    }

    /// Confirma o que o estágio promete sobre a árvore.
    ///
    /// Antes do spawn nada pode existir fora da sessão do runner; depois dele o
    /// `timeout` e o harness precisam existir, e é isso que torna o caso uma
    /// reprodução da janela e não um exercício vazio.
    fn confirmar_arvore(&mut self) {
        if !self.estagio().controlador_existe() {
            self.campanha.recapturar_membros();
            let fora: Vec<&Identidade> = self
                .campanha
                .membros
                .iter()
                .filter(|membro| membro.sid != self.campanha.controlador.sid)
                .collect();
            assert!(
                fora.is_empty(),
                "antes do spawn não pode existir árvore de iteração: {fora:?}"
            );
            return;
        }
        let limite = Instant::now() + PRAZO_DE_CONFIRMACAO;
        assert!(
            self.campanha.aguardar_arvore(limite),
            "o estágio {} promete controlador e harness vivos",
            self.estagio().nome()
        );
        let papeis: HashSet<PapelNaCampanha> = self
            .campanha
            .membros
            .iter()
            .filter(|membro| membro.sid != self.campanha.controlador.sid)
            .map(papel_de)
            .collect();
        assert!(
            papeis.contains(&PapelNaCampanha::Timeout),
            "o controlador da iteração precisa existir: {papeis:?}"
        );
    }

    /// Libera a barreira escrevendo a linha que o gancho aguarda.
    fn liberar(&mut self) {
        let entrada = self
            .campanha
            .filho
            .stdin
            .as_mut()
            .expect("entrada padrão da campanha");
        entrada
            .write_all(b"liberado\n")
            .expect("liberar a barreira");
        entrada.flush().expect("liberar a barreira");
    }

    /// Envia sinais com o runner congelado no estágio corrente.
    ///
    /// A árvore é capturada antes do primeiro sinal e reaproveitada: capturá-la
    /// de novo depois de um sinal deixaria de ser determinístico.
    fn sinalizar(&mut self, sinais: &[i32]) {
        if self.capturado.is_none() {
            self.capturado = Some(self.campanha.capturar_antes_do_sinal());
        }
        for sinal in sinais {
            enviar_sinal(self.campanha.controlador.pid, *sinal);
        }
    }

    /// Libera o estágio corrente e para no próximo, se houver.
    fn avancar(&mut self) {
        self.liberar();
        self.atual += 1;
        if self.atual < self.estagios.len() {
            self.aguardar_barreira();
        }
    }

    /// Libera a barreira e só retorna quando o Bash publicou a entrada no
    /// `wait` normal e `/proc/<pid>/wchan` confirma `do_wait`. O trace fornece a
    /// causalidade e `wchan` confirma o estado; tempo nenhum decide o avanço.
    fn avancar_ate_wait_normal(&mut self) {
        self.liberar();
        self.atual += 1;
        let limite = Instant::now() + PRAZO_DE_CONFIRMACAO;
        let wchan = PathBuf::from(format!("/proc/{}/wchan", self.campanha.controlador.pid));
        let trace = self.area.join("trace.tsv");
        while Instant::now() < limite {
            let wait_publicado = fs::read_to_string(&trace)
                .map(|conteudo| conteudo.contains("NORMAL_WAIT_BEGIN"))
                .unwrap_or(false);
            let wait_confirmado = fs::read_to_string(&wchan)
                .map(|estado| estado.trim() == "do_wait")
                .unwrap_or(false);
            if wait_publicado && wait_confirmado {
                return;
            }
            if matches!(self.campanha.filho.try_wait(), Ok(Some(_))) {
                self.campanha.filho_recolhido = true;
                panic!("o runner terminou antes do wait normal");
            }
            std::thread::yield_now();
        }
        panic!("o runner não alcançou o wait normal");
    }

    /// Libera o que restar da barreira e recolhe o resultado.
    fn colher(mut self) -> RelatorioEncerramento {
        let capturado = match self.capturado.take() {
            Some(capturado) => capturado,
            None => self.campanha.capturar_antes_do_sinal(),
        };
        while self.atual < self.estagios.len() {
            self.liberar();
            self.atual += 1;
            if self.atual < self.estagios.len() {
                self.aguardar_barreira();
            }
        }
        self.campanha.colher_relatorio(capturado)
    }

    /// Envia os sinais com o runner congelado, libera e recolhe o resultado.
    fn interromper(mut self, sinais: &[i32]) -> RelatorioEncerramento {
        self.sinalizar(sinais);
        self.colher()
    }

    /// Libera a barreira sem sinal algum e recolhe o resultado.
    ///
    /// Serve aos casos em que o desfecho é decidido pelo próprio runner — falha
    /// de identidade, por exemplo —, e cuja janela precisa mesmo assim ser
    /// congelada para que a captura da árvore não corra com o desfecho.
    fn concluir(self) -> RelatorioEncerramento {
        self.colher()
    }
}

/// Evidência de uma iteração interrompida, dentro do lote único da campanha.
fn manifesto_interrompido(evidencia: &Path) -> String {
    let lotes = diretorios_de(&evidencia.join("batches"));
    assert_eq!(lotes.len(), 1, "esperado exatamente um lote: {lotes:?}");
    let preservados: Vec<PathBuf> = diretorios_de(&lotes[0])
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
    fs::read_to_string(preservados[0].join("manifest.txt")).expect("manifesto da interrupção")
}

fn campo_do_manifesto(manifesto: &str, chave: &str) -> String {
    manifesto
        .lines()
        .find_map(|linha| linha.strip_prefix(&format!("{chave}=")))
        .unwrap_or_else(|| panic!("manifesto sem campo {chave}: {manifesto}"))
        .to_string()
}

/// Linha estruturada com o que o caso observou, para o registro das campanhas.
///
/// Sai apenas sob `--nocapture`. Existe para que uma campanha externa registre
/// por iteração as identidades **observadas pelo caso**, em vez de reconstruí-las
/// depois a partir de uma máquina que já mudou.
fn registrar_iteracao(
    caso: &str,
    estagio: &str,
    sinal: &str,
    relatorio: &RelatorioEncerramento,
    evidencia: &Path,
) {
    let runner = &relatorio.controlador;
    let auxiliares: Vec<i32> = relatorio
        .membros_antes
        .iter()
        .filter(|membro| membro.sid == runner.sid)
        .map(|membro| membro.pid)
        .collect();
    let arvore: Vec<String> = relatorio
        .membros_antes
        .iter()
        .filter(|membro| membro.sid != runner.sid)
        .map(|membro| {
            format!(
                "{:?}:{}/{}/{}/{}",
                papel_de(membro),
                membro.pid,
                membro.start_time,
                membro.pgid,
                membro.sid
            )
        })
        .collect();
    println!(
        "JANELA caso={caso} estagio={estagio} sinal={sinal} \
runner_pid={} runner_start={} runner_pgid={} runner_sid={} \
auxiliares={auxiliares:?} arvore={arvore:?} sessoes={:?} exit={} \
lock={} evidencia={} sobreviventes={} restantes={} grupo_vazio={} sessao_vazia={}",
        runner.pid,
        runner.start_time,
        runner.pgid,
        runner.sid,
        relatorio.sessoes,
        relatorio.codigo,
        evidencia.join(".lock").exists(),
        evidencia.display(),
        relatorio.sobreviventes_apos_controlador.len(),
        relatorio.restantes.len(),
        relatorio.grupo_vazio,
        relatorio.sessao_vazia,
    );
}

/// Nenhum resíduo, lock liberado, evidência preservada e `130`.
fn exigir_interrupcao_limpa(
    caso: &str,
    relatorio: &RelatorioEncerramento,
    evidencia: &Path,
    causas_aceitas: &[&str],
    estado_esperado: &str,
) {
    registrar_iteracao(
        caso,
        estado_esperado,
        &causas_aceitas.join("|"),
        relatorio,
        evidencia,
    );
    assert_eq!(
        relatorio.codigo, EXIT_INTERROMPIDO,
        "{caso}: interrupção precisa retornar 130; controlador={:?} sessoes={:?}",
        relatorio.controlador, relatorio.sessoes
    );
    assert!(
        relatorio.filho_recolhido,
        "{caso}: o filho direto precisa ser aguardado"
    );
    assert!(
        relatorio.controlador_morto,
        "{caso}: o controlador precisa ter terminado"
    );
    // A asserção que a janela antiga violava: quando o runner sai, nada da
    // árvore pode ter sobrevivido a ele.
    assert!(
        relatorio.sobreviventes_apos_controlador.is_empty(),
        "{caso}: o runner deixou árvore viva atrás de si: {:?}",
        relatorio.papeis_sobreviventes()
    );
    assert!(
        relatorio.term_enviados.is_empty() && relatorio.kill_enviados.is_empty(),
        "{caso}: a fixture não pode ter precisado conter nada: term={:?} kill={:?}",
        relatorio.term_enviados,
        relatorio.kill_enviados
    );
    assert!(
        relatorio.restantes.is_empty(),
        "{caso}: nenhum descendente pode sobreviver: {:?}",
        relatorio.papeis_restantes()
    );
    assert!(
        relatorio.grupo_vazio && relatorio.sessao_vazia,
        "{caso}: grupo e sessões da campanha precisam ficar vazios"
    );
    assert!(
        !evidencia.join(".lock").exists(),
        "{caso}: interrupção precisa liberar o lock"
    );
    assert!(
        !evidencia.join("SUMMARY-modo.txt").exists(),
        "{caso}: lote interrompido nunca publica projeção legada"
    );
    let manifesto = manifesto_interrompido(evidencia);
    assert_eq!(
        campo_do_manifesto(&manifesto, "reason"),
        "interrupted",
        "{caso}: manifesto={manifesto}"
    );
    assert_eq!(
        campo_do_manifesto(&manifesto, "exit_code"),
        EXIT_INTERROMPIDO.to_string(),
        "{caso}: manifesto={manifesto}"
    );
    let causa = campo_do_manifesto(&manifesto, "interrupt_signal");
    assert!(
        causas_aceitas.contains(&causa.as_str()),
        "{caso}: a causa registrada precisa estar entre {causas_aceitas:?}: {manifesto}"
    );
    assert_eq!(
        campo_do_manifesto(&manifesto, "interrupt_state"),
        estado_esperado,
        "{caso}: o estado da primeira causa precisa ser registrado: {manifesto}"
    );
}

fn caso_interrupcao_na_janela(
    caso: &str,
    estagio: EstagioDeInicializacao,
    sinal: i32,
    nome_sinal: &str,
) {
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", estagio);
    janela.confirmar_arvore();
    let controlador_existia = estagio.controlador_existe();
    let relatorio = janela.interromper(&[sinal]);
    exigir_interrupcao_limpa(
        caso,
        &relatorio,
        &evidencia,
        &[nome_sinal],
        estagio.estado_esperado(),
    );
    let manifesto = manifesto_interrompido(&evidencia);
    assert_eq!(
        campo_do_manifesto(&manifesto, "interrupt_controller_existed"),
        if controlador_existia { "yes" } else { "no" },
        "{caso}: a existência do controlador na primeira causa: {manifesto}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

macro_rules! casos_da_janela {
    ($($nome:ident => ($estagio:expr, $sinal:expr, $rotulo:expr);)*) => {
        $(
            #[test]
            fn $nome() {
                caso_interrupcao_na_janela(stringify!($nome), $estagio, $sinal, $rotulo);
            }
        )*
    };
}

casos_da_janela! {
    int_antes_do_spawn_nao_inicia_controlador =>
        (EstagioDeInicializacao::AntesDoSpawn, SIGINT, "INT");
    term_antes_do_spawn_nao_inicia_controlador =>
        (EstagioDeInicializacao::AntesDoSpawn, SIGTERM, "TERM");
    hup_antes_do_spawn_nao_inicia_controlador =>
        (EstagioDeInicializacao::AntesDoSpawn, SIGHUP, "HUP");

    int_depois_do_spawn_e_antes_da_identidade =>
        (EstagioDeInicializacao::DepoisDoSpawn, SIGINT, "INT");
    term_depois_do_spawn_e_antes_da_identidade =>
        (EstagioDeInicializacao::DepoisDoSpawn, SIGTERM, "TERM");
    hup_depois_do_spawn_e_antes_da_identidade =>
        (EstagioDeInicializacao::DepoisDoSpawn, SIGHUP, "HUP");

    int_depois_da_identidade_e_antes_de_ativo =>
        (EstagioDeInicializacao::DepoisDaIdentidade, SIGINT, "INT");
    term_depois_da_identidade_e_antes_de_ativo =>
        (EstagioDeInicializacao::DepoisDaIdentidade, SIGTERM, "TERM");
    hup_depois_da_identidade_e_antes_de_ativo =>
        (EstagioDeInicializacao::DepoisDaIdentidade, SIGHUP, "HUP");

    int_imediatamente_depois_de_ativo =>
        (EstagioDeInicializacao::DepoisDeAtivo, SIGINT, "INT");
    term_imediatamente_depois_de_ativo =>
        (EstagioDeInicializacao::DepoisDeAtivo, SIGTERM, "TERM");
    hup_imediatamente_depois_de_ativo =>
        (EstagioDeInicializacao::DepoisDeAtivo, SIGHUP, "HUP");

    int_depois_da_criacao_do_monitor =>
        (EstagioDeInicializacao::DepoisDoMonitor, SIGINT, "INT");
    term_depois_da_criacao_do_monitor =>
        (EstagioDeInicializacao::DepoisDoMonitor, SIGTERM, "TERM");
    hup_depois_da_criacao_do_monitor =>
        (EstagioDeInicializacao::DepoisDoMonitor, SIGHUP, "HUP");
}

/// Sinais em dois pontos distintos da fase STARTING.
///
/// A ordem entre eles é do caso, não do kernel: o primeiro chega com o runner
/// congelado depois do spawn, e o restante só depois de a barreira ter avançado
/// para o ponto seguinte, ainda dentro de STARTING. Enviar todos de uma vez
/// provaria outra coisa — o Linux entrega o menor número primeiro, e a asserção
/// mediria a ordem de entrega em vez da autoridade da primeira causa.
fn caso_sinais_repetidos(caso: &str, primeiro: i32, seguintes: &[i32], rotulo: &str) {
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova_multi(
        &raiz,
        "modo",
        &[
            EstagioDeInicializacao::DepoisDoSpawn,
            EstagioDeInicializacao::DepoisDaIdentidade,
        ],
    );
    janela.confirmar_arvore();
    janela.sinalizar(&[primeiro]);
    janela.avancar();
    janela.sinalizar(seguintes);
    let relatorio = janela.colher();
    exigir_interrupcao_limpa(caso, &relatorio, &evidencia, &[rotulo], "starting");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn dois_sinais_durante_starting_preservam_o_primeiro() {
    caso_sinais_repetidos("dois-sinais-starting", SIGINT, &[SIGTERM], "INT");
}

#[test]
fn dois_sinais_durante_starting_preservam_o_primeiro_mesmo_maior() {
    // A primeira causa é a primeira **recebida**, não a de menor número: sem
    // este caso, a preservação passaria por acidente da ordem do kernel.
    caso_sinais_repetidos("dois-sinais-starting-maior", SIGTERM, &[SIGINT], "TERM");
}

#[test]
fn tres_sinais_durante_starting_preservam_o_primeiro() {
    caso_sinais_repetidos("tres-sinais-starting", SIGHUP, &[SIGINT, SIGTERM], "HUP");
}

fn caso_sinais_durante_encerramento(
    caso: &str,
    primeiro: i32,
    rotulo_primeiro: &str,
    seguintes: &[i32],
    estagio: EstagioDeInicializacao,
    evento_do_estagio: &str,
) {
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova_multi(
        &raiz,
        "modo",
        &[EstagioDeInicializacao::DepoisDoMonitor, estagio],
    );
    janela.confirmar_arvore();
    let trace = janela.area.join("trace.tsv");

    let controlador = janela.campanha.controlador.clone();
    janela.avancar_ate_wait_normal();
    janela.sinalizar(&[primeiro]);
    janela.aguardar_barreira();
    for sinal in seguintes {
        assert!(
            sinalizar_identidade(&controlador, *sinal),
            "{caso}: sinal posterior precisa alcançar o mesmo runner"
        );
    }

    let relatorio = janela.colher();
    exigir_interrupcao_limpa(caso, &relatorio, &evidencia, &[rotulo_primeiro], "active");

    let trace_conteudo = fs::read_to_string(trace).expect("trace causal");
    let linhas: Vec<Vec<&str>> = trace_conteudo
        .lines()
        .map(|linha| linha.split('\t').collect())
        .collect();
    assert!(
        linhas.iter().all(|campos| campos.len() == 9),
        "{caso}: trace estruturado inválido: {linhas:?}"
    );
    for (indice, campos) in linhas.iter().enumerate() {
        assert_eq!(
            campos[0].parse::<usize>(),
            Ok(indice + 1),
            "{caso}: sequência causal precisa ser contínua"
        );
    }
    let eventos: Vec<&str> = linhas.iter().map(|campos| campos[7]).collect();
    let posicao = |evento: &str| {
        eventos
            .iter()
            .position(|atual| *atual == evento)
            .unwrap_or_else(|| panic!("{caso}: evento ausente {evento}: {linhas:?}"))
    };
    let latch = posicao("INTERRUPT_LATCHED");
    let estagio = posicao(evento_do_estagio);
    let ignorado = posicao("INTERRUPT_IGNORED_ALREADY_IN_PROGRESS");
    assert!(
        posicao("NORMAL_WAIT_BEGIN") < latch && latch < estagio && estagio < ignorado,
        "{caso}: ordem causal inválida: {linhas:?}"
    );
    assert_eq!(
        eventos
            .iter()
            .filter(|evento| **evento == "INTERRUPT_LATCHED")
            .count(),
        1,
        "{caso}: exatamente um latch"
    );
    assert_eq!(linhas[latch][3], rotulo_primeiro, "{caso}: causa congelada");
    assert!(
        !eventos.contains(&"NORMAL_VERDICT_BEGIN"),
        "{caso}: latch proíbe o veredito normal"
    );
    let saida = posicao("PROCESS_EXIT");
    assert_eq!(linhas[saida][8], EXIT_INTERROMPIDO.to_string());
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn sinais_repetidos_durante_o_encerramento_nao_reentram() {
    caso_sinais_durante_encerramento(
        "int-term-durante-reaping",
        SIGINT,
        "INT",
        &[SIGTERM],
        EstagioDeInicializacao::DuranteReaping,
        "REAP_BEGIN",
    );
}

#[test]
fn term_seguido_de_int_durante_reaping_preserva_term() {
    caso_sinais_durante_encerramento(
        "term-int-durante-reaping",
        SIGTERM,
        "TERM",
        &[SIGINT],
        EstagioDeInicializacao::DuranteReaping,
        "REAP_BEGIN",
    );
}

#[test]
fn hup_e_sinais_posteriores_durante_reaping_preservam_hup() {
    caso_sinais_durante_encerramento(
        "hup-multiplos-durante-reaping",
        SIGHUP,
        "HUP",
        &[SIGINT, SIGTERM],
        EstagioDeInicializacao::DuranteReaping,
        "REAP_BEGIN",
    );
}

#[test]
fn sinais_posteriores_durante_exit_trap_nao_mudam_status() {
    caso_sinais_durante_encerramento(
        "int-term-durante-exit-trap",
        SIGINT,
        "INT",
        &[SIGTERM],
        EstagioDeInicializacao::DuranteExitTrap,
        "EXIT_TRAP_BEGIN",
    );
}

fn caso_controle_devolve_ao_wait(
    caso: &str,
    remover_commit: bool,
) -> (PathBuf, PathBuf, RelatorioEncerramento) {
    let efeito_que_retorna = "    interrupt_in_progress=1\n    cleanup_active\n    return 0\n}\n\n# Processa uma interrupcao";
    let mut substituicoes = vec![(EFEITO_TERMINAL_DO_HANDLER, efeito_que_retorna)];
    if remover_commit {
        substituicoes.push((COMMIT_APOS_WAIT, "    true\n    active_state=reaping"));
        substituicoes.push((
            COMMIT_ANTES_DO_VEREDITO,
            "    true\n    pinker_flake_trace NORMAL_VERDICT_BEGIN",
        ));
    }
    let raiz = raiz_com_runner_variado(caso, &substituicoes);
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", EstagioDeInicializacao::DepoisDoMonitor);
    janela.confirmar_arvore();
    let trace = janela.area.join("trace.tsv");
    janela.avancar_ate_wait_normal();
    janela.sinalizar(&[SIGINT]);
    let relatorio = janela.colher();
    (raiz, trace, relatorio)
}

#[test]
fn latch_reassume_o_desfecho_se_o_handler_devolve_controle_ao_wait() {
    let caso = "latch-reassume-depois-do-wait";
    let (raiz, trace, relatorio) = caso_controle_devolve_ao_wait(caso, false);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    exigir_interrupcao_limpa(caso, &relatorio, &evidencia, &["INT"], "active");
    let trace = fs::read_to_string(trace).expect("trace do commit terminal");
    let wait = trace.find("NORMAL_WAIT_RETURN").expect("retorno do wait");
    let finalize = trace[wait..]
        .find("INTERRUPTED_FINALIZE_BEGIN")
        .map(|posicao| posicao + wait)
        .expect("o latch precisa retomar a finalização");
    assert!(wait < finalize);
    assert!(!trace.contains("NORMAL_VERDICT_BEGIN"));
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn sensibilidade_sem_commit_reassume_veredito_normal_e_sai_um() {
    let caso = "sem-commit-depois-do-wait";
    let (raiz, trace, relatorio) = caso_controle_devolve_ao_wait(caso, true);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    assert_eq!(
        relatorio.codigo, 1,
        "sem ownership terminal o batch reclassifica a interrupção como falha"
    );
    assert!(relatorio.filho_recolhido, "o runner ainda é aguardado");
    assert!(
        relatorio.sobreviventes_apos_controlador.is_empty()
            && relatorio.restantes.is_empty()
            && relatorio.grupo_vazio
            && relatorio.sessao_vazia,
        "a reprodução limpa toda a árvore apesar do status errado: {relatorio:?}"
    );
    assert!(!evidencia.join(".lock").exists(), "o lock é removido");
    assert!(
        evidencia.join("SUMMARY-modo.txt").is_file(),
        "o defeito alcança e publica a projeção normal"
    );
    let trace = fs::read_to_string(trace).expect("trace da sensibilidade");
    let eventos = [
        "NORMAL_WAIT_BEGIN",
        "SIGNAL_RECEIVED",
        "INTERRUPT_LATCHED",
        "REAP_BEGIN",
        "REAP_END",
        "NORMAL_WAIT_RETURN",
        "NORMAL_VERDICT_BEGIN",
        "EXIT_TRAP_BEGIN",
        "PROCESS_EXIT",
    ];
    let mut anterior = 0;
    for evento in eventos {
        let posicao = trace[anterior..]
            .find(evento)
            .map(|local| local + anterior)
            .unwrap_or_else(|| panic!("evento causal ausente {evento}: {trace}"));
        anterior = posicao + evento.len();
    }
    let retorno = trace
        .lines()
        .find(|linha| linha.contains("NORMAL_WAIT_RETURN"))
        .expect("retorno normal registrado");
    assert!(
        retorno.ends_with("\t130"),
        "o wait interrompido devolve 130 antes de virar batch failure: {retorno}"
    );
    let saida = trace
        .lines()
        .find(|linha| linha.contains("PROCESS_EXIT"))
        .expect("saída registrada");
    assert!(saida.ends_with("\t1"), "o batch termina em 1: {saida}");
    let lotes = diretorios_de(&evidencia.join("batches"));
    assert_eq!(lotes.len(), 1);
    assert!(
        diretorios_de(&lotes[0]).iter().all(|diretorio| !diretorio
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("INTERRUPTED-")),
        "o fluxo normal não preserva evidência INTERRUPTED"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn controlador_que_termina_antes_da_captura_nao_perde_identidade() {
    // O Bash colhe filhos de background por conta própria, e com isso
    // `/proc/<pid>` de um controlador rápido desaparece antes que o runner
    // consiga lê-lo. É por isso que a identidade é publicada pelo próprio
    // controlador, antes do harness: uma identidade anunciada não corre com a
    // morte de quem a anunciou.
    let caso = "controlador-termina-antes-da-captura";
    let raiz = raiz_isolada(caso);
    let mut janela = CampanhaNaJanela::nova_com_harness(
        &raiz,
        "modo",
        &[EstagioDeInicializacao::DepoisDoSpawn],
        0,
    );

    // Confirmação por varredura, com prazo: o controlador precisa ter sumido
    // antes de a barreira ser liberada.
    let limite = Instant::now() + PRAZO_DE_CONFIRMACAO;
    let mut sumiu = false;
    while Instant::now() < limite {
        janela.campanha.recapturar_membros();
        let vivos = janela
            .campanha
            .membros
            .iter()
            .filter(|membro| membro.sid != janela.campanha.controlador.sid)
            .filter(|membro| classificar_identidade(membro) == ClasseIdentidade::Viva)
            .count();
        if vivos == 0 && !janela.campanha.membros.is_empty() {
            sumiu = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(sumiu, "o controlador precisa terminar antes da liberação");

    janela.liberar();
    let status = janela
        .campanha
        .filho
        .wait()
        .expect("aguardar a campanha rápida");
    janela.campanha.filho_recolhido = true;
    assert_eq!(
        status.code(),
        Some(0),
        "identidade anunciada sobrevive à morte do controlador"
    );
    let evidencia = raiz.join("target/pinker-flake-evidence");
    assert!(
        evidencia.join("SUMMARY-modo.txt").is_file(),
        "a campanha precisa concluir normalmente"
    );
    assert!(!evidencia.join(".lock").exists(), "o lock é liberado");
    drop(janela);
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn interrupcao_nao_inicia_a_iteracao_seguinte() {
    // Um lote de duas iterações interrompido na primeira não pode retomar o
    // fluxo normal: o lote guarda a evidência da iteração interrompida e de
    // nenhuma outra, e `PROGRESS.txt` — escrito apenas ao fim de cada
    // iteração — nunca chega a existir.
    let caso = "sem-iteracao-seguinte";
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova_completa(
        &raiz,
        "modo",
        "2",
        &[EstagioDeInicializacao::DepoisDoMonitor],
        HARNESS_LONGO_SEGUNDOS,
    );
    janela.confirmar_arvore();
    let relatorio = janela.interromper(&[SIGINT]);
    exigir_interrupcao_limpa(caso, &relatorio, &evidencia, &["INT"], "active");

    let lotes = diretorios_de(&evidencia.join("batches"));
    assert_eq!(lotes.len(), 1, "lote único: {lotes:?}");
    let preservados = diretorios_de(&lotes[0]);
    assert_eq!(
        preservados.len(),
        1,
        "somente a iteração interrompida pode existir: {preservados:?}"
    );
    assert!(
        !lotes[0].join("PROGRESS.txt").exists(),
        "nenhuma iteração foi concluída, logo não há progresso publicado"
    );
    assert!(
        !lotes[0].join("SUMMARY.txt").exists(),
        "um lote interrompido nunca publica o próprio resumo"
    );
    let manifesto = fs::read_to_string(lotes[0].join("MANIFEST.txt")).expect("manifesto do lote");
    assert!(
        manifesto.contains("runs=2"),
        "o lote pedia duas iterações: {manifesto}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn processo_externo_nao_e_tocado_por_interrupcao_na_janela() {
    let caso = "externo-na-janela";
    let raiz = raiz_isolada(caso);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let (externo, identidade) = processo_externo();
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", EstagioDeInicializacao::DepoisDoSpawn);
    janela.confirmar_arvore();
    let relatorio = janela.interromper(&[SIGINT]);
    exigir_interrupcao_limpa(caso, &relatorio, &evidencia, &["INT"], "starting");
    assert_eq!(
        classificar_identidade(&identidade),
        ClasseIdentidade::Viva,
        "o processo externo não pode ser alcançado"
    );
    encerrar_externo(externo, &identidade);
    let _ = fs::remove_dir_all(&raiz);
}

// --- variações do runner: reprodução e falha de identidade -----------------

/// Trechos do runner que carregam, cada um, um invariante da janela.
///
/// Ficam em constantes porque servem a três consumidores que precisam
/// concordar byte a byte: a guarda textual, as variações de sensibilidade e as
/// reproduções que reintroduzem o defeito num runner publicado à parte.
const DEFERRAL_EM_STARTING: &str = "    if [[ $active_state == starting ]]; then";
const CLEANUP_DO_FILHO_DIRETO: &str =
    "    [[ -n $active_pid ]] || return 0\n    active_state=reaping";
const CAPTURA_DE_IDENTIDADE: &str = "    if ! pinker_flake_capture_identity; then";
const PRIMEIRA_CAUSA: &str = "    if [[ -z $interruption_latched ]]; then";
const REGISTRO_DA_CAUSA: &str = "        pending_signal=$sinal";
const GUARDA_DE_REENTRANCIA: &str = "    if [[ -n $interrupt_in_progress ]]; then";
const GUARDA_DA_DEFERRAL: &str = "    [[ -z $interrupt_in_progress ]] || return 0";
const COMMIT_APOS_WAIT: &str = "    pinker_flake_commit_interrupted\n    active_state=reaping";
const COMMIT_ANTES_DO_VEREDITO: &str =
    "    pinker_flake_commit_interrupted\n    pinker_flake_trace NORMAL_VERDICT_BEGIN";
const EFEITO_TERMINAL_DO_HANDLER: &str = "    interrupt_in_progress=1\n    pinker_flake_finish_interrupted\n}\n\n# Processa uma interrupcao";
const PROMOCAO_PARA_ATIVO: &str = "    active_state=active";
const CAMPO_START: &str = "    active_start=$start";
const CAMPO_PGID: &str = "    active_pgid=$pgid";
const CAMPO_SID: &str = "    active_sid=$sid";
const EXIGENCIA_DE_START: &str = "    [[ $active_start =~ ^[0-9]+$ ]] || return 1";
const WAIT_DO_CONTROLADOR: &str = "    wait \"$active_pid\" 2>/dev/null || true";
const WAIT_DO_MONITOR: &str = "    wait \"$monitor_pid\" 2>/dev/null || true";
const SAIDA_INTERROMPIDA: &str = "    exit \"$PINKER_FLAKE_EXIT_INTERRUPTED\"";
/// Fecha o canal de prontidão **só para o comando** que cria o controlador.
const FECHAMENTO_DO_CANAL: &str = " {identity_fd}>&- &";
/// Linha em que o próprio controlador publica a sua identidade, antes do
/// harness. Única no runner, e por isso endereçável byte a byte pelas variações.
const ANUNCIO_DA_IDENTIDADE: &str =
    "    \"$$\" \"${campos[19]}\" \"${campos[2]}\" \"${campos[3]}\" > \"$canal\" || exit 97";

/// Publica em uma raiz própria uma variação do runner real.
///
/// O arquivo do repositório nunca é reescrito: a variação existe apenas dentro
/// da raiz temporária do caso.
fn raiz_com_runner_variado(caso: &str, substituicoes: &[(&str, &str)]) -> PathBuf {
    let unico = format!(
        "pinker-flake-runner-{}-{}-{}",
        caso,
        std::process::id(),
        SEQUENCIA.fetch_add(1, Ordering::Relaxed)
    );
    let raiz = std::env::temp_dir().join(unico);
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(raiz.join("scripts")).expect("criar raiz da variação");
    let original = fs::read_to_string(caminho_do_runner()).expect("ler runner");
    let mut variado = original.clone();
    for (de, para) in substituicoes {
        assert!(
            variado.contains(de),
            "a variação exige o trecho ausente no runner: {de:?}"
        );
        variado = variado.replace(de, para);
    }
    assert_ne!(variado, original, "a variação precisa alterar o runner");
    publicar_executavel(&raiz.join("scripts/pinker-flake-runner.sh"), &variado);
    raiz
}

#[test]
fn reproducao_da_janela_antiga_deixa_a_arvore_viva() {
    // Reintroduz exatamente o defeito, em dois pontos: o handler deixa de
    // diferir durante `starting`, e o encerramento volta a exigir identidade
    // completa antes de conter qualquer coisa. É o runner anterior a esta
    // correção, com o gancho apenas tornando a janela endereçável.
    //
    // O caso existe para provar que a reprodução é determinística. Sem ele, a
    // correção afirmaria fechar uma janela que ninguém demonstrou abrir.
    let caso = "reproducao-janela-antiga";
    let raiz = raiz_com_runner_variado(
        caso,
        &[
            (DEFERRAL_EM_STARTING, "    if false; then"),
            (
                CLEANUP_DO_FILHO_DIRETO,
                "    pinker_flake_identity_complete || return 0\n    active_state=reaping",
            ),
        ],
    );
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", EstagioDeInicializacao::DepoisDoSpawn);
    janela.confirmar_arvore();
    let relatorio = janela.interromper(&[SIGINT]);

    // O verde enganoso: código correto, lock liberado, evidência preservada.
    assert_eq!(
        relatorio.codigo, EXIT_INTERROMPIDO,
        "a janela antiga devolvia 130 assim mesmo"
    );
    assert!(
        !evidencia.join(".lock").exists(),
        "a janela antiga liberava o lock"
    );
    assert_eq!(
        campo_do_manifesto(&manifesto_interrompido(&evidencia), "reason"),
        "interrupted",
        "a janela antiga preservava a evidência como interrompida"
    );

    // E o defeito por baixo dele.
    assert!(
        !relatorio.sobreviventes_apos_controlador.is_empty(),
        "a reprodução precisa deixar árvore viva; papéis={:?}",
        relatorio.papeis_sobreviventes()
    );
    assert!(
        relatorio
            .papeis_sobreviventes()
            .contains(&PapelNaCampanha::Timeout),
        "o controlador da iteração precisa sobreviver ao runner: {:?}",
        relatorio.papeis_sobreviventes()
    );
    // A fixture é dona da árvore que o runner variado abandonou.
    assert!(
        !relatorio.kill_enviados.is_empty() || !relatorio.term_enviados.is_empty(),
        "a fixture precisa ter contido o que o runner variado deixou"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "a fixture precisa conter o resíduo da reprodução: {:?}",
        relatorio.papeis_restantes()
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn falha_de_identidade_e_fechada_e_nao_publica_resumo() {
    // Identidade que não pode ser provada não autoriza monitor, teste seguinte,
    // PASS nem resumo. O runner contém o filho direto, aguarda-o, libera o lock
    // e termina com código não zero estável.
    let caso = "identidade-falha-fechada";
    let raiz = raiz_com_runner_variado(
        caso,
        &[(
            "pinker_flake_capture_identity() {\n    local campos",
            "pinker_flake_capture_identity() {\n    return 1\n    local campos",
        )],
    );
    let evidencia = raiz.join("target/pinker-flake-evidence");
    // A janela é congelada depois do spawn: sem isso a captura da árvore
    // correria com um desfecho que acontece em milissegundos.
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", EstagioDeInicializacao::DepoisDoSpawn);
    janela.confirmar_arvore();
    let relatorio = janela.concluir();

    assert_eq!(
        relatorio.codigo, EXIT_IDENTIDADE,
        "falha de identidade termina com código não zero estável"
    );
    assert!(
        relatorio.sobreviventes_apos_controlador.is_empty(),
        "a falha fechada precisa conter o filho direto e a árvore dele: {:?}",
        relatorio.papeis_sobreviventes()
    );
    assert!(
        relatorio.filho_recolhido,
        "o filho direto precisa ser aguardado mesmo na falha de identidade"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "nada pode restar: {:?}",
        relatorio.papeis_restantes()
    );
    assert!(
        !evidencia.join(".lock").exists(),
        "a falha de identidade libera o lock"
    );
    assert!(
        !evidencia.join("SUMMARY-modo.txt").exists(),
        "falha de identidade nunca publica resumo"
    );
    let lotes = diretorios_de(&evidencia.join("batches"));
    assert_eq!(lotes.len(), 1, "lote único: {lotes:?}");
    assert!(
        !lotes[0].join("SUMMARY.txt").exists(),
        "o lote não publica resumo próprio"
    );
    let diagnostico: Vec<PathBuf> = diretorios_de(&lotes[0])
        .into_iter()
        .filter(|caminho| {
            caminho
                .file_name()
                .map(|nome| nome.to_string_lossy().starts_with("IDENTITY-FAILURE-"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        diagnostico.len(),
        1,
        "evidência diagnóstica preservada: {diagnostico:?}"
    );
    let manifesto =
        fs::read_to_string(diagnostico[0].join("manifest.txt")).expect("manifesto diagnóstico");
    assert_eq!(
        campo_do_manifesto(&manifesto, "reason"),
        "identity-capture-failed",
        "manifesto={manifesto}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

fn caso_identidade_divergente(caso: &str, substituicao: (&str, &str)) {
    let raiz = raiz_com_runner_variado(caso, &[substituicao]);
    let evidencia = raiz.join("target/pinker-flake-evidence");
    let mut janela = CampanhaNaJanela::nova(&raiz, "modo", EstagioDeInicializacao::DepoisDoSpawn);
    janela.confirmar_arvore();
    let relatorio = janela.concluir();
    assert_eq!(
        relatorio.codigo, EXIT_IDENTIDADE,
        "{caso}: identidade divergente falha fechada"
    );
    assert!(
        relatorio.filho_recolhido,
        "{caso}: o filho direto precisa ser aguardado"
    );
    assert!(
        relatorio.restantes.is_empty(),
        "{caso}: nada pode restar: {:?}",
        relatorio.papeis_restantes()
    );
    assert!(
        !evidencia.join("SUMMARY-modo.txt").exists(),
        "{caso}: identidade divergente nunca publica resumo"
    );
    assert!(!evidencia.join(".lock").exists(), "{caso}: lock liberado");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn pgid_divergente_do_controlador_falha_fechado() {
    // O anúncio deixa de coincidir com o grupo real. Sinalizar o grupo passaria
    // a alcançar processo que a campanha não criou, e por isso não acontece.
    caso_identidade_divergente(
        "pgid-divergente-controlador",
        (
            ANUNCIO_DA_IDENTIDADE,
            "    \"$$\" \"${campos[19]}\" \"$(( campos[2] + 1 ))\" \"${campos[3]}\" > \"$canal\" || exit 97",
        ),
    );
}

#[test]
fn sid_divergente_do_controlador_falha_fechado() {
    caso_identidade_divergente(
        "sid-divergente-controlador",
        (
            ANUNCIO_DA_IDENTIDADE,
            "    \"$$\" \"${campos[19]}\" \"${campos[2]}\" \"$(( campos[3] + 1 ))\" > \"$canal\" || exit 97",
        ),
    );
}

#[test]
fn start_time_divergente_do_controlador_falha_fechado() {
    // Start time divergente é a assinatura de PID reutilizado: o número existe,
    // mas passou a nomear outro processo. O anúncio deixa de bater com `/proc`
    // e a captura falha fechada.
    caso_identidade_divergente(
        "start-time-divergente-controlador",
        (
            ANUNCIO_DA_IDENTIDADE,
            "    \"$$\" \"$(( campos[19] + 1 ))\" \"${campos[2]}\" \"${campos[3]}\" > \"$canal\" || exit 97",
        ),
    );
}

#[test]
fn pid_divergente_do_filho_direto_falha_fechado() {
    // Um anúncio íntegro que não pertence ao filho direto desta iteração não
    // pode virar identidade ativa: seria autoridade sobre um processo alheio.
    caso_identidade_divergente(
        "pid-divergente-controlador",
        (
            ANUNCIO_DA_IDENTIDADE,
            "    \"$(( $$ + 1 ))\" \"${campos[19]}\" \"${campos[2]}\" \"${campos[3]}\" > \"$canal\" || exit 97",
        ),
    );
}

// --- higiene de descritores ------------------------------------------------

#[test]
fn o_harness_nao_herda_o_canal_de_prontidao() {
    // O canal de prontidão é aberto pelo runner **antes** do spawn, de modo que
    // sem fechamento explícito o controlador e toda a árvore do harness
    // herdariam um descritor do runner. Descritor herdado é exatamente a classe
    // de defeito que a suíte nativa existe para vigiar — foi um `ETXTBSY` por
    // descritor gravável herdado que motivou a PR #424 —, e um canal de
    // sincronização interna do runner não pode virar um deles.
    let raiz = raiz_isolada("canal-nao-vaza");
    let registro = raiz.join("descritores-do-harness.txt");
    let harness = publicar_executavel(
        &raiz.join("harness-que-lista-descritores.sh"),
        &format!(
            "#!/usr/bin/env bash\nfor alvo in /proc/self/fd/*; do printf '%s -> %s\\n' \"${{alvo##*/}}\" \"$(readlink -f \"$alvo\" 2>/dev/null)\"; done > {}\n{}\nexit 0\n",
            aspas_simples(registro.to_str().expect("utf-8")),
            RESUMO_COM_TESTE
                .lines()
                .map(|linha| format!("printf '%s\\n' {}", aspas_simples(linha)))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    );

    let execucao = executar(
        &raiz,
        &["modo", "1"],
        &[("PINKER_FLAKE_TEST_BINARY", harness.to_str().expect("utf-8"))],
    );
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);

    let descritores = fs::read_to_string(&registro).expect("descritores do harness");
    assert!(
        !descritores.contains(".fifo"),
        "o harness herdou o canal de prontidão do runner:\n{descritores}"
    );
    // Nem qualquer outro descritor apontando para dentro da evidência, além dos
    // dois que o runner abre de propósito: `stdout` e `stderr` da iteração.
    let para_a_evidencia: Vec<&str> = descritores
        .lines()
        .filter(|linha| linha.contains("pinker-flake-evidence"))
        .collect();
    assert!(
        para_a_evidencia
            .iter()
            .all(|linha| linha.ends_with("/stdout") || linha.ends_with("/stderr")),
        "descritor inesperado para a evidência: {para_a_evidencia:?}"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// --- gancho de inicialização ----------------------------------------------

#[test]
fn gancho_de_inicializacao_e_inativo_por_padrao() {
    // Producão não muda quando a variável não existe, e também não muda quando
    // apenas uma das duas existe: configuração parcial não ativa comportamento.
    let raiz = raiz_isolada("gancho-inativo");
    let marca = raiz.join("gancho-executou");
    let gancho = publicar_executavel(
        &raiz.join("gancho-proibido.sh"),
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$1\" >> {}\nexit 0\n",
            aspas_simples(marca.to_str().expect("utf-8"))
        ),
    );
    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["modo", "1"],
        &[
            ("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8")),
            ("PINKER_FLAKE_STARTUP_HOOK", gancho.to_str().expect("utf-8")),
        ],
    );
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    assert!(
        !marca.exists(),
        "sem a lista explícita de estágios o gancho não pode ser chamado"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn gancho_antigo_nao_recebe_estagios_novos() {
    // `PINKER_FLAKE_TEST_HOOK` continua sendo chamado apenas na remoção de lock.
    // Ampliá-lo faria o único consumidor existente — que reescreve o marker em
    // qualquer estágio que receba — agir em pontos que nunca esperou.
    let raiz = raiz_isolada("gancho-antigo-compativel");
    let registro = raiz.join("estagios-do-gancho-antigo.txt");
    let gancho = publicar_executavel(
        &raiz.join("gancho-antigo.sh"),
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$1\" >> {}\nexit 0\n",
            aspas_simples(registro.to_str().expect("utf-8"))
        ),
    );
    let evidencia = raiz.join("target/pinker-flake-evidence");
    fs::create_dir_all(&evidencia).expect("criar evidência");
    plantar_lock(
        &evidencia,
        &marker_valido(&pid_encerrado().to_string(), "999", "morta", "lote-morto"),
    );
    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
    let execucao = executar(
        &raiz,
        &["nova", "1"],
        &[
            ("PINKER_FLAKE_TEST_BINARY", binario.to_str().expect("utf-8")),
            ("PINKER_FLAKE_TEST_HOOK", gancho.to_str().expect("utf-8")),
        ],
    );
    assert_eq!(execucao.codigo, 0, "stderr={}", execucao.saida_erro);
    let estagios = fs::read_to_string(&registro).expect("estágios registrados");
    let observados: Vec<&str> = estagios.lines().collect();
    assert_eq!(
        observados,
        vec!["before-lock-removal"],
        "o gancho antigo recebe exatamente o estágio que sempre recebeu"
    );
    let _ = fs::remove_dir_all(&raiz);
}

// --- leitura de identidade em modo biblioteca ------------------------------

/// Avalia uma função do runner em modo biblioteca.
fn avaliar_no_runner(chamada: &str) -> (bool, String) {
    let script = format!(
        "PINKER_FLAKE_LIB_ONLY=1 source {}; {chamada}",
        caminho_do_runner().display()
    );
    let saida = Command::new("bash")
        .arg("-c")
        .arg(script)
        .output()
        .expect("executar função do runner");
    (
        saida.status.success(),
        String::from_utf8_lossy(&saida.stdout).trim().to_string(),
    )
}

#[test]
fn leitura_de_stat_rejeita_truncado_nao_numerico_e_ambiguo() {
    // `comm` aceita espaço e parêntese. Cortar pelo primeiro `)` deslocaria
    // todos os campos seguintes, inclusive o start time — que é exatamente o que
    // distingue um PID vivo de um número herdado.
    // Cauda em que o valor de cada campo é o próprio índice posicional: `$3` vale
    // 3, `$4` vale 4 e `$20` vale 20. Um deslocamento de campo aparece como
    // número trocado, e não como falha genérica.
    let cauda: Vec<String> = (2..=52).map(|indice| indice.to_string()).collect();
    let corpo = format!("S {}", cauda.join(" "));
    let normal = format!("7 (bash) {corpo}");
    let (ok, saida) = avaliar_no_runner(&format!(
        "pinker_flake_stat_fields {}",
        aspas_simples(&normal)
    ));
    assert!(ok, "linha normal precisa ser aceita: {normal}");
    assert_eq!(saida, "S 3 4 20", "campos extraídos: {saida}");

    let ambiguo = format!("7 (co) mm) {corpo}");
    let (ok_ambiguo, saida_ambigua) = avaliar_no_runner(&format!(
        "pinker_flake_stat_fields {}",
        aspas_simples(&ambiguo)
    ));
    assert!(ok_ambiguo, "comm com parêntese precisa ser aceito");
    assert_eq!(
        saida_ambigua, "S 3 4 20",
        "o corte precisa usar o último fecha-parêntese"
    );

    let sem_start = format!("7 (bash) S {}", cauda[..10].join(" "));
    let start_nao_numerico = normal.replacen(" 20 ", " x ", 1);
    let pgid_nao_numerico = normal.replacen(" 3 ", " x ", 1);
    for invalido in [
        "",
        "sem parenteses aqui",
        sem_start.as_str(),
        start_nao_numerico.as_str(),
        pgid_nao_numerico.as_str(),
    ] {
        let (aceito, _) = avaliar_no_runner(&format!(
            "pinker_flake_stat_fields {}",
            aspas_simples(invalido)
        ));
        assert!(
            !aceito,
            "estrutura inválida precisa ser rejeitada: {invalido:?}"
        );
    }

    // `/proc` indisponível: um PID que não existe não produz identidade parcial.
    let (aceito, _) =
        avaliar_no_runner(&format!("pinker_flake_stat_fields_of {}", pid_encerrado()));
    assert!(!aceito, "PID inexistente não pode produzir identidade");
}

#[test]
fn anuncio_de_identidade_e_validado_estritamente() {
    let (ok, saida) =
        avaliar_no_runner("pinker_flake_parse_identity_line 'pinker-flake-identity 1 7 4242 7 7'");
    assert!(ok, "anúncio íntegro precisa ser aceito");
    assert_eq!(saida, "7 4242 7 7");

    for invalido in [
        "",
        "pinker-flake-identity 1 7 4242 7",
        "pinker-flake-identity 1 7 4242 7 7 sobrando",
        "pinker-flake-identity 2 7 4242 7 7",
        "outra-marca 1 7 4242 7 7",
        "pinker-flake-identity 1 0 4242 7 7",
        "pinker-flake-identity 1 -7 4242 7 7",
        "pinker-flake-identity 1 7 x 7 7",
        "pinker-flake-identity 1 7 4242 0 7",
        "pinker-flake-identity 1 7 4242 7 0",
    ] {
        let (aceito, _) = avaliar_no_runner(&format!(
            "pinker_flake_parse_identity_line {}",
            aspas_simples(invalido)
        ));
        assert!(
            !aceito,
            "anúncio inválido precisa ser rejeitado: {invalido:?}"
        );
    }
}

// --- sensibilidade das guardas de inicialização ----------------------------

/// Invariantes que o ciclo de inicialização e interrupção precisa manter.
///
/// Cada um existe porque a sua ausência reintroduz um defeito concreto e
/// nomeado: sinal perdido, cleanup com identidade incompleta, promoção sem os
/// quatro campos, zumbi, monitor abandonado ou resumo publicado depois de uma
/// interrupção.
fn guarda_de_inicializacao(fonte: &str) -> Result<(), String> {
    // Casamento por nome nunca pode aparecer no runner.
    for proibido in [
        format!("{}kill", "p"),
        format!("kill{}", "all"),
        format!("{}grep", "p"),
    ] {
        if fonte.contains(proibido.as_str()) {
            return Err(format!("casamento por nome no runner: {proibido}"));
        }
    }

    // A fase STARTING precisa existir e diferir o sinal sem sair e sem limpar.
    let Some(deferral) = fonte.find(DEFERRAL_EM_STARTING) else {
        return Err(String::from("a deferral durante STARTING desapareceu"));
    };
    let corpo = &fonte[deferral..];
    let fim = corpo.find("\n    fi\n").unwrap_or(corpo.len());
    if !corpo[..fim].contains("        return 0") {
        return Err(String::from(
            "a fase STARTING deixou de retornar sem encerrar",
        ));
    }

    // Primeira causa imutável e handler não reentrante.
    if !fonte.contains(PRIMEIRA_CAUSA) || !fonte.contains(REGISTRO_DA_CAUSA) {
        return Err(String::from("a primeira causa deixou de ser registrada"));
    }
    if !fonte.contains(GUARDA_DE_REENTRANCIA) || !fonte.contains(GUARDA_DA_DEFERRAL) {
        return Err(String::from(
            "a guarda de reentrância não cobre handler e deferral",
        ));
    }

    // A promoção acontece uma única vez, depois da captura validada.
    if fonte.matches(PROMOCAO_PARA_ATIVO).count() != 1 {
        return Err(String::from("a promoção para ACTIVE deixou de ser única"));
    }
    let promocao = fonte.find(PROMOCAO_PARA_ATIVO).expect("promoção");
    let Some(captura) = fonte.find(CAPTURA_DE_IDENTIDADE) else {
        return Err(String::from("a captura de identidade desapareceu"));
    };
    let Some(spawn) = fonte.find("        setsid bash -c") else {
        return Err(String::from("o spawn do controlador desapareceu"));
    };
    let Some(starting) = fonte.find("    active_state=starting") else {
        return Err(String::from("a fase STARTING não é iniciada"));
    };
    let Some(monitor) = fonte.find("    monitor_pid=$!") else {
        return Err(String::from("o monitor deixou de ser identificado"));
    };
    if !(starting < spawn && spawn < captura && captura < promocao && promocao < monitor) {
        return Err(String::from("a ordem da janela crítica foi quebrada"));
    }

    // Os quatro campos da identidade ativa.
    for exigido in [CAMPO_START, CAMPO_PGID, CAMPO_SID, EXIGENCIA_DE_START] {
        if !fonte.contains(exigido) {
            return Err(format!("campo da identidade ativa ausente: {exigido}"));
        }
    }
    for exigido in [
        "    [[ $active_pgid =~ ^[1-9][0-9]*$ ]] || return 1",
        "    [[ $active_sid =~ ^[1-9][0-9]*$ ]] || return 1",
    ] {
        if !fonte.contains(exigido) {
            return Err(format!("a completude da identidade não exige: {exigido}"));
        }
    }

    // O encerramento contém o filho direto mesmo sem identidade completa, e o
    // sinal coletivo exige revalidação.
    if !fonte.contains(CLEANUP_DO_FILHO_DIRETO) {
        return Err(String::from(
            "o encerramento voltou a exigir identidade completa para agir",
        ));
    }
    if !fonte.contains("    [[ $(pinker_flake_active_identity_state) == live ]] || return 1") {
        return Err(String::from("sinal coletivo sem revalidação"));
    }

    // O canal de prontidão não pode ser herdado pela árvore do harness.
    if !fonte.contains(FECHAMENTO_DO_CANAL) {
        return Err(String::from(
            "o canal de prontidao vaza como descritor herdado para o harness",
        ));
    }

    // Filho direto e monitor aguardados.
    if !fonte.contains(WAIT_DO_CONTROLADOR) {
        return Err(String::from("wait do controlador ausente"));
    }
    if !fonte.contains(WAIT_DO_MONITOR) {
        return Err(String::from("wait do monitor ausente"));
    }
    if !fonte.contains("monitor_pid=\n") && !fonte.contains("    monitor_pid=\n") {
        return Err(String::from(
            "o monitor deixou de ser inicializado explicitamente",
        ));
    }

    // Interrupção termina em 130 e nunca cai no fluxo normal.
    if !fonte.contains(SAIDA_INTERROMPIDA) {
        return Err(String::from(
            "a interrupção deixou de sair com 130 e pode publicar resumo",
        ));
    }
    if !fonte.contains(COMMIT_APOS_WAIT) || !fonte.contains(COMMIT_ANTES_DO_VEREDITO) {
        return Err(String::from(
            "o latch deixou de vedar o veredito normal depois do wait",
        ));
    }
    if !fonte.contains("    exit \"$PINKER_FLAKE_EXIT_IDENTITY\"") {
        return Err(String::from("a falha de identidade deixou de ser fechada"));
    }

    Ok(())
}

#[test]
fn sensibilidade_das_guardas_de_inicializacao_detecta_cada_variacao() {
    let caminho = caminho_do_runner();
    let original = fs::read(&caminho).expect("ler o runner");
    let soma_original = soma_fnv1a(&original);
    let fonte = String::from_utf8(original.clone()).expect("o runner é utf-8");

    assert_eq!(
        guarda_de_inicializacao(&fonte),
        Ok(()),
        "o runner real precisa satisfazer as próprias guardas"
    );

    let area = raiz_isolada("sensibilidade-inicializacao");
    let copia = area.join("variacao.sh");

    let variacoes: Vec<(&str, String)> = vec![
        (
            "remoção da deferral durante STARTING",
            fonte.replace(DEFERRAL_EM_STARTING, "    if false; then"),
        ),
        (
            "promoção para ACTIVE antes da captura",
            fonte.replace(CAPTURA_DE_IDENTIDADE, "    if false; then"),
        ),
        (
            "promoção para ACTIVE antes de start time",
            fonte.replace(CAMPO_START, "    active_start="),
        ),
        (
            "promoção para ACTIVE antes de PGID",
            fonte.replace(CAMPO_PGID, "    active_pgid="),
        ),
        (
            "promoção para ACTIVE antes de SID",
            fonte.replace(CAMPO_SID, "    active_sid="),
        ),
        (
            "active_start vazio restaurado como estado permitido",
            fonte.replace(EXIGENCIA_DE_START, "    [[ -z $active_start ]] || return 0"),
        ),
        (
            "retorno silencioso do cleanup com filho vivo",
            fonte.replace(
                CLEANUP_DO_FILHO_DIRETO,
                "    pinker_flake_identity_complete || return 0\n    active_state=reaping",
            ),
        ),
        (
            "perda da interrupção pendente",
            fonte.replace(REGISTRO_DA_CAUSA, "        pending_signal="),
        ),
        (
            "segundo sinal substituindo o primeiro",
            fonte.replace(PRIMEIRA_CAUSA, "    if true; then"),
        ),
        (
            "handler reentrante",
            fonte.replace(GUARDA_DE_REENTRANCIA, "    true"),
        ),
        (
            "deferral reentrante",
            fonte.replace(GUARDA_DA_DEFERRAL, "    true"),
        ),
        (
            "veredito normal reassume autoridade depois do wait",
            fonte
                .replace(COMMIT_APOS_WAIT, "    true\n    active_state=reaping")
                .replace(
                    COMMIT_ANTES_DO_VEREDITO,
                    "    true\n    pinker_flake_trace NORMAL_VERDICT_BEGIN",
                ),
        ),
        (
            "remoção do wait do controlador",
            fonte.replace(WAIT_DO_CONTROLADOR, "    true"),
        ),
        (
            "remoção do wait do monitor",
            fonte.replace(WAIT_DO_MONITOR, "    true"),
        ),
        (
            "publicação de resumo após interrupção",
            fonte.replace(SAIDA_INTERROMPIDA, "    return 0"),
        ),
        (
            "sinal coletivo sem revalidação",
            fonte.replace(
                "    [[ $(pinker_flake_active_identity_state) == live ]] || return 1",
                "    true",
            ),
        ),
        (
            "canal de prontidao herdado pelo harness",
            fonte.replace(FECHAMENTO_DO_CANAL, " &"),
        ),
        (
            "casamento por substring no runner",
            format!("{fonte}\n# {}kill -f campanha\n", "p"),
        ),
    ];

    for (nome, mutada) in &variacoes {
        assert_ne!(
            mutada.as_str(),
            fonte.as_str(),
            "a variação {nome} precisa alterar a fonte"
        );
        fs::write(&copia, mutada.as_bytes()).expect("gravar variação");
        let lida = fs::read_to_string(&copia).expect("reler variação");
        assert!(
            guarda_de_inicializacao(&lida).is_err(),
            "a variação {nome} passou despercebida pelas guardas"
        );

        fs::write(&copia, &original).expect("restaurar variação");
        let restaurada = fs::read(&copia).expect("reler restauração");
        assert_eq!(
            soma_fnv1a(&restaurada),
            soma_original,
            "restauração de {nome} não é byte a byte"
        );
        assert_eq!(restaurada, original, "restauração de {nome} divergiu");
    }

    let depois = fs::read(&caminho).expect("reler o runner");
    assert_eq!(
        soma_fnv1a(&depois),
        soma_original,
        "o runner precisa continuar byte a byte idêntico"
    );
    assert_eq!(depois, original);
    let _ = fs::remove_dir_all(&area);
}

// --- sensibilidade das guardas de contenção --------------------------------

/// Soma de verificação estável, para provar restauração byte a byte.
///
/// FNV-1a de 64 bits: uma dependência a menos e determinismo suficiente para o
/// que se afirma aqui, que é igualdade de conteúdo e não resistência a colisão
/// adversarial.
fn soma_fnv1a(conteudo: &[u8]) -> u64 {
    let mut soma: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in conteudo {
        soma ^= u64::from(*byte);
        soma = soma.wrapping_mul(0x1000_0000_01b3);
    }
    soma
}

/// Recorte da região autorizada a sinalizar.
fn regiao_de_contencao(fonte: &str) -> Result<&str, String> {
    // Construídas por concatenação para que os literais desta guarda não casem
    // consigo mesmos ao inspecionar a própria fonte.
    let abertura = format!("// pinker-contencao{}", ":inicio");
    let fechamento = format!("// pinker-contencao{}", ":fim");
    if fonte.matches(abertura.as_str()).count() != 1 {
        return Err(String::from(
            "sentinela de abertura da contenção não é única",
        ));
    }
    if fonte.matches(fechamento.as_str()).count() != 1 {
        return Err(String::from(
            "sentinela de fechamento da contenção não é única",
        ));
    }
    let abre = fonte.find(abertura.as_str()).expect("abertura");
    let fecha = fonte.find(fechamento.as_str()).expect("fechamento");
    if abre >= fecha {
        return Err(String::from("sentinelas da contenção fora de ordem"));
    }
    Ok(&fonte[abre..fecha])
}

/// Invariantes que a contenção precisa manter.
///
/// Cada uma existe porque a sua ausência reintroduz um defeito concreto: árvore
/// não limpa, sinal sem revalidação, zumbi no processo de testes, casamento por
/// nome, ou sinalização de grupo inteiro.
fn guarda_de_contencao(fonte: &str) -> Result<(), String> {
    let regiao = regiao_de_contencao(fonte)?;

    // Casamento por nome nunca pode reaparecer, em lugar algum da suíte.
    for proibido in [
        format!("{}kill", "p"),
        format!("kill{}", "all"),
        format!("{}grep", "p"),
    ] {
        if fonte.contains(proibido.as_str()) {
            return Err(format!("casamento por nome reintroduzido: {proibido}"));
        }
    }

    // Sinalização de grupo inteiro: um PGID pode ter desaparecido e sido
    // reutilizado entre a captura e o sinal.
    let sinal_de_grupo = format!("kill{}", "(-");
    if fonte.contains(sinal_de_grupo.as_str()) {
        return Err(String::from(
            "sinalização de grupo sem prova de propriedade",
        ));
    }

    // A limpeza de descendentes precisa existir nos dois caminhos: o explícito
    // e a contingência do `Drop`.
    let limpeza = "self.conter_remanescentes(PRAZO_DE_CONTENCAO)";
    if regiao.matches(limpeza).count() < 2 {
        return Err(String::from(
            "limpeza de descendentes ausente no encerramento explícito ou no Drop",
        ));
    }

    // Todo sinal passa por revalidação de identidade imediatamente antes do
    // `kill`.
    if !regiao.contains("if classificar_identidade(identidade) != ClasseIdentidade::Viva {") {
        return Err(String::from("sinal sem revalidação de identidade"));
    }

    // Escalada TERM → KILL sobre os remanescentes, não apenas sobre o
    // controlador.
    for exigido in [
        "sinalizar_identidade(alvo, sinal)",
        "self.fase_de_contencao(SIGTERM,",
        "self.fase_de_contencao(SIGKILL,",
    ] {
        if !regiao.contains(exigido) {
            return Err(format!("escalada incompleta: {exigido} ausente"));
        }
    }

    // Todo shell que precise honrar trap nasce com os sinais de encerramento na
    // disposição padrão. Herdar `SIG_IGN` torna `trap ... INT` um no-op
    // silencioso, e o encerramento normal deixa de existir sem que erro algum
    // apareça.
    if !regiao.contains("restaurar_sinais_padrao()") {
        return Err(String::from(
            "campanha nasce sem restaurar a disposição de sinais",
        ));
    }

    // A espera pela árvore exige a sessão da iteração povoada, não apenas
    // aberta: é o que prova que o runner já sabe encerrar a própria árvore.
    if !regiao.contains("if internos >= 2 {") {
        return Err(String::from(
            "a espera aceita árvore incompleta e devolve campanha que o runner não sabe encerrar",
        ));
    }

    // A árvore é reabsorvida a cada rodada. Uma passada única deixa para trás o
    // neto forkado entre a captura e o sinal, que foi a origem das cinco falhas
    // de grupo não vazio em oitenta e quatro execuções paralelas.
    let reabsorcao = "self.absorver_retardatarios();";
    if regiao.matches(reabsorcao).count() < 3 {
        return Err(String::from(
            "reabsorção da árvore ausente em alguma etapa da contenção",
        ));
    }

    // O filho direto precisa ser aguardado, ou vira zumbi dentro do processo de
    // testes.
    if !regiao
        .contains(r#"let status = self.filho.wait().expect("aguardar campanha proprietária");"#)
    {
        return Err(String::from("wait do filho direto ausente"));
    }
    if !regiao.contains("let codigo = self.recolher_filho_direto();") {
        return Err(String::from(
            "o encerramento explícito não recolhe o filho direto",
        ));
    }

    Ok(())
}

#[test]
fn sensibilidade_das_guardas_de_contencao_detecta_cada_variacao() {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pinker_flake_runner_tests.rs");
    let original = fs::read(&caminho).expect("ler a própria suíte");
    let soma_original = soma_fnv1a(&original);
    let fonte = String::from_utf8(original.clone()).expect("a suíte é utf-8");

    assert_eq!(
        guarda_de_contencao(&fonte),
        Ok(()),
        "a fonte real precisa satisfazer as próprias guardas"
    );

    // Cada variação é aplicada a uma cópia em arquivo, e o arquivo é restaurado
    // a partir dos bytes originais logo em seguida. O arquivo real da suíte
    // nunca é reescrito.
    let area = raiz_isolada("sensibilidade-contencao");
    let copia = area.join("variacao.rs");

    let variacoes: Vec<(&str, String)> = vec![
        (
            "remoção da limpeza de descendentes",
            fonte.replace(
                "self.conter_remanescentes(PRAZO_DE_CONTENCAO)",
                "ResultadoContencao::default()",
            ),
        ),
        (
            "sinalização apenas do controlador",
            fonte.replace("sinalizar_identidade(alvo, sinal)", "false"),
        ),
        (
            "passada única sem reabsorção da árvore",
            fonte.replace("self.absorver_retardatarios();", ""),
        ),
        (
            "shell nasce com sinal herdado como ignorado",
            fonte.replace("restaurar_sinais_padrao()", "Ok(())"),
        ),
        (
            "espera aceita árvore incompleta",
            fonte.replace("if internos >= 2 {", "if internos >= 1 {"),
        ),
        (
            "uso de identidade sem revalidação",
            fonte.replace(
                "if classificar_identidade(identidade) != ClasseIdentidade::Viva {",
                "if false {",
            ),
        ),
        (
            "ausência de wait do filho direto",
            fonte.replace(
                r#"let status = self.filho.wait().expect("aguardar campanha proprietária");"#,
                "let status = std::process::ExitStatus::default();",
            ),
        ),
        (
            "casamento por substring",
            format!("{fonte}\n// {}kill -f campanha\n", "p"),
        ),
        (
            "sinalização de grupo sem revalidação",
            format!("{fonte}\n// kill{}pgid, SIGKILL)\n", "(-"),
        ),
    ];

    for (nome, mutada) in &variacoes {
        assert_ne!(
            mutada.as_str(),
            fonte.as_str(),
            "a variação {nome} precisa alterar a fonte"
        );
        fs::write(&copia, mutada.as_bytes()).expect("gravar variação");
        let lida = fs::read_to_string(&copia).expect("reler variação");
        assert!(
            guarda_de_contencao(&lida).is_err(),
            "a variação {nome} passou despercebida pelas guardas"
        );

        // Restauração byte a byte, conferida por hash.
        fs::write(&copia, &original).expect("restaurar variação");
        let restaurada = fs::read(&copia).expect("reler restauração");
        assert_eq!(
            soma_fnv1a(&restaurada),
            soma_original,
            "restauração de {nome} não é byte a byte"
        );
        assert_eq!(restaurada, original, "restauração de {nome} divergiu");
    }

    // A suíte real permanece intacta.
    let depois = fs::read(&caminho).expect("reler a própria suíte");
    assert_eq!(
        soma_fnv1a(&depois),
        soma_original,
        "a fonte da suíte precisa continuar byte a byte idêntica"
    );
    assert_eq!(depois, original);
    let _ = fs::remove_dir_all(&area);
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

// ---------------------------------------------------------------------------
// ETXTBSY: causalidade, correção e sensibilidade.
//
// O `exec` falha com `ETXTBSY` enquanto **qualquer** processo mantém descritor
// gravável para o inode executado. As provas abaixo não dependem de repetição
// nem de tempo: a janela é aberta e fechada por sincronização explícita entre o
// teste e um processo auxiliar.
// ---------------------------------------------------------------------------
mod publicacao_de_executaveis {
    use super::*;

    const CORPO: &str = "#!/usr/bin/env bash\nexit 0\n";

    /// Descritores **graváveis** deste processo que apontam para `inode`.
    ///
    /// `fork` copia a tabela deste processo, então esta é a origem que importa:
    /// se aqui não há descritor gravável, nenhum filho pode tê-lo herdado.
    fn descritores_graveis_para(inode: u64) -> Vec<String> {
        let mut encontrados = Vec::new();
        let Ok(entradas) = fs::read_dir("/proc/self/fd") else {
            return encontrados;
        };
        for entrada in entradas.flatten() {
            let numero = entrada.file_name().to_string_lossy().into_owned();
            let Ok(alvo) = fs::read_link(entrada.path()) else {
                continue;
            };
            let Ok(meta) = fs::metadata(&alvo) else {
                continue;
            };
            if meta.ino() != inode {
                continue;
            }
            let Ok(info) = fs::read_to_string(format!("/proc/self/fdinfo/{numero}")) else {
                continue;
            };
            let modo = info
                .lines()
                .find_map(|linha| linha.strip_prefix("flags:"))
                .and_then(|valor| u32::from_str_radix(valor.trim(), 8).ok())
                .unwrap_or(0);
            // O_WRONLY = 1, O_RDWR = 2.
            if modo & 0o3 != 0 {
                encontrados.push(format!("{numero} -> {}", alvo.display()));
            }
        }
        encontrados
    }

    fn raiz_de_prova(caso: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!(
            "pinker-etxtbsy-{}-{}-{}",
            caso,
            std::process::id(),
            SEQUENCIA.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(&raiz).expect("criar raiz de prova");
        raiz
    }

    // --- causalidade -------------------------------------------------------

    #[test]
    fn descritor_gravavel_do_proprio_processo_impede_exec() {
        let raiz = raiz_de_prova("proprio");
        let alvo = raiz.join("alvo.sh");

        let mut arquivo = fs::File::create(&alvo).expect("criar alvo");
        arquivo.write_all(CORPO.as_bytes()).expect("escrever");
        arquivo.flush().expect("flush");
        let mut permissoes = fs::metadata(&alvo).expect("metadados").permissions();
        permissoes.set_mode(0o755);
        fs::set_permissions(&alvo, permissoes).expect("permissões");

        let inode = inode_de(&alvo);
        assert_eq!(
            descritores_graveis_para(inode).len(),
            1,
            "o próprio processo detém exatamente um descritor gravável"
        );
        assert_eq!(
            erro_ao_executar(&alvo),
            Some(ETXTBSY),
            "descritor gravável aberto impede o exec (pid={} inode={inode})",
            std::process::id()
        );

        drop(arquivo);
        assert!(
            descritores_graveis_para(inode).is_empty(),
            "fechar remove o descritor da tabela deste processo"
        );

        // Deliberadamente **não** se afirma aqui que o inode volta a executar.
        // Esta é a única função da suíte que escreve um executável no processo
        // de teste, e por isso sofre do próprio mecanismo que demonstra: um
        // `fork` concorrente pode ter herdado este descritor e o mantém até o
        // seu `exec`. Essa direção é provada sem corrida por
        // `descritor_gravavel_de_processo_concorrente_impede_exec`, onde o
        // fechamento é sincronizado com o auxiliar.

        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn descritor_gravavel_de_processo_concorrente_impede_exec() {
        // O descritor causal não precisa ser deste processo. Basta existir.
        let raiz = raiz_de_prova("concorrente");
        let alvo = publicar_executavel(&raiz.join("alvo.sh"), CORPO);
        let inode = inode_de(&alvo);
        assert_eq!(erro_ao_executar(&alvo), None, "recém-publicado executa");

        let escritor = EscritorConcorrente::abrir(&alvo);
        let pid = escritor.pid();
        assert_eq!(
            erro_ao_executar(&alvo),
            Some(ETXTBSY),
            "descritor alheio (pid={pid}) impede o exec do inode {inode}"
        );
        assert!(
            descritores_graveis_para(inode).is_empty(),
            "o descritor causal é do auxiliar, não deste processo"
        );

        escritor.fechar();
        assert_eq!(erro_ao_executar(&alvo), None, "fechado, volta a executar");

        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn rename_nao_remove_descritor_gravavel_sobre_o_inode() {
        // Sensibilidade contra a solução tentadora e insuficiente: escrever num
        // nome temporário e renomear. `rename` troca o nome, não o inode.
        let raiz = raiz_de_prova("rename");
        let temporario = raiz.join("alvo.sh.tmp");
        let publicado = raiz.join("alvo.sh");

        publicar_executavel(&temporario, CORPO);
        let antes = inode_de(&temporario);

        let escritor = EscritorConcorrente::abrir(&temporario);
        fs::rename(&temporario, &publicado).expect("renomear");

        assert_eq!(
            antes,
            inode_de(&publicado),
            "rename preserva o inode: é por isso que ele não basta sozinho"
        );
        assert_eq!(
            erro_ao_executar(&publicado),
            Some(ETXTBSY),
            "o descritor alheio sobrevive ao rename"
        );

        escritor.fechar();
        assert_eq!(erro_ao_executar(&publicado), None);

        let _ = fs::remove_dir_all(&raiz);
    }

    // --- a correção --------------------------------------------------------

    #[test]
    fn publicacao_nao_deixa_descritor_gravavel_neste_processo() {
        let raiz = raiz_de_prova("sem-descritor");
        let alvo = publicar_executavel(&raiz.join("alvo.sh"), CORPO);
        let inode = inode_de(&alvo);

        assert!(
            descritores_graveis_para(inode).is_empty(),
            "publicar não deixa descritor gravável: {:?}",
            descritores_graveis_para(inode)
        );
        assert_eq!(erro_ao_executar(&alvo), None, "o publicado executa");
        assert!(
            !raiz.join("alvo.fonte").exists(),
            "a fonte não executável é removida"
        );
        assert!(
            !raiz.join("alvo.parcial").exists(),
            "o materializado intermediário não sobrevive"
        );

        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn permissoes_e_conteudo_completos_quando_o_caminho_e_devolvido() {
        let raiz = raiz_de_prova("completo");
        let mut corpo = String::from("#!/usr/bin/env bash\n");
        for indice in 0..2000 {
            corpo.push_str(&format!("# linha de enchimento {indice}\n"));
        }
        corpo.push_str("exit 7\n");

        let alvo = publicar_executavel(&raiz.join("grande.sh"), &corpo);

        assert_eq!(
            fs::read_to_string(&alvo).expect("reler"),
            corpo,
            "conteúdo íntegro quando o caminho é devolvido"
        );
        assert_eq!(
            fs::metadata(&alvo).expect("metadados").permissions().mode() & 0o777,
            0o755,
            "modo aplicado antes do uso"
        );
        let status = Command::new(&alvo).status().expect("executar publicado");
        assert_eq!(status.code(), Some(7), "executa o arquivo completo");

        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn publicacao_em_destino_impossivel_falha_fechada() {
        let raiz = raiz_de_prova("falha-fechada");
        let inexistente = raiz.join("nao-existe").join("alvo.sh");
        let resultado = std::panic::catch_unwind(|| publicar_executavel(&inexistente, CORPO));
        assert!(
            resultado.is_err(),
            "destino impossível falha fechado, nunca devolve caminho"
        );
        assert!(!inexistente.exists());
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caminhos_publicados_sao_exclusivos_entre_raizes_e_harnesses() {
        let primeira = raiz_isolada("exclusivo-a");
        let segunda = raiz_isolada("exclusivo-b");
        assert_ne!(primeira, segunda, "raízes isoladas são exclusivas");

        let a = binario_falso(&primeira, RESUMO_COM_TESTE, 0);
        let b = binario_falso(&primeira, RESUMO_COM_TESTE, 0);
        let c = binario_falso(&segunda, RESUMO_COM_TESTE, 0);
        assert_ne!(a, b, "harnesses na mesma raiz têm caminhos distintos");
        assert_ne!(a, c);
        assert_ne!(inode_de(&a), inode_de(&b), "inodes distintos");

        let _ = fs::remove_dir_all(&primeira);
        let _ = fs::remove_dir_all(&segunda);
    }

    // --- concorrência real --------------------------------------------------

    #[test]
    fn publicacao_concorrente_com_spawn_simultaneo_nunca_produz_etxtbsy() {
        // Metade das threads publica e executa; a outra metade forka sem parar,
        // que é o gesto que copia a tabela de descritores. Todas partem da mesma
        // barreira, então a sobreposição é garantida e não depende de sorte.
        const PUBLICADORAS: usize = 6;
        const FORKADORAS: usize = 6;
        const RODADAS: usize = 10;

        let barreira = Arc::new(Barrier::new(PUBLICADORAS + FORKADORAS));
        let mut linhas = Vec::new();

        for indice in 0..PUBLICADORAS {
            let barreira = Arc::clone(&barreira);
            linhas.push(std::thread::spawn(move || {
                let raiz = raiz_de_prova(&format!("concorrente-pub-{indice}"));
                barreira.wait();
                for rodada in 0..RODADAS {
                    let alvo = publicar_executavel(
                        &raiz.join(format!("alvo-{rodada}.sh")),
                        "#!/usr/bin/env bash\nexit 0\n",
                    );
                    let erro = erro_ao_executar(&alvo);
                    assert_eq!(
                        erro, None,
                        "publicação {indice}/{rodada} não pode falhar no exec: {erro:?}"
                    );
                }
                let _ = fs::remove_dir_all(&raiz);
            }));
        }

        for _ in 0..FORKADORAS {
            let barreira = Arc::clone(&barreira);
            linhas.push(std::thread::spawn(move || {
                barreira.wait();
                for _ in 0..RODADAS * 4 {
                    let status = Command::new("/bin/true")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .expect("fork concorrente");
                    assert!(status.success());
                }
            }));
        }

        for linha in linhas {
            linha.join().expect("thread de concorrência");
        }
    }

    #[test]
    fn raizes_isoladas_concorrentes_produzem_runners_executaveis() {
        const THREADS: usize = 6;
        let barreira = Arc::new(Barrier::new(THREADS));
        let mut linhas = Vec::new();

        for indice in 0..THREADS {
            let barreira = Arc::clone(&barreira);
            linhas.push(std::thread::spawn(move || {
                barreira.wait();
                let raiz = raiz_isolada(&format!("raiz-concorrente-{indice}"));
                let runner = raiz.join("scripts/pinker-flake-runner.sh");
                assert!(runner.is_file(), "runner publicado");
                assert!(
                    descritores_graveis_para(inode_de(&runner)).is_empty(),
                    "nenhum descritor gravável sobre o runner publicado"
                );
                // Uso real: erro de uso, que não inicia teste algum, mas exige
                // que o `exec` do runner funcione.
                let execucao = executar(&raiz, &["modo", "0"], &[]);
                assert_eq!(
                    execucao.codigo, EXIT_USO,
                    "o runner precisa ter executado; stderr={}",
                    execucao.saida_erro
                );
                let _ = fs::remove_dir_all(&raiz);
            }));
        }

        for linha in linhas {
            linha.join().expect("thread de raiz isolada");
        }
    }

    #[test]
    fn harnesses_falsos_concorrentes_sao_publicados_e_executam() {
        const THREADS: usize = 6;
        let barreira = Arc::new(Barrier::new(THREADS));
        let mut linhas = Vec::new();

        for indice in 0..THREADS {
            let barreira = Arc::clone(&barreira);
            linhas.push(std::thread::spawn(move || {
                let raiz = raiz_isolada(&format!("harness-concorrente-{indice}"));
                barreira.wait();
                for _ in 0..4 {
                    let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
                    assert_eq!(
                        erro_ao_executar(&binario),
                        None,
                        "harness recém-publicado precisa executar"
                    );
                }
                let _ = fs::remove_dir_all(&raiz);
            }));
        }

        for linha in linhas {
            linha.join().expect("thread de harness");
        }
    }

    #[test]
    fn duas_campanhas_proprietarias_em_raizes_independentes_coexistem() {
        // Raízes distintas são checkouts distintos: o lock é por checkout.
        let primeira = raiz_isolada("independente-a");
        let segunda = raiz_isolada("independente-b");

        let dona_a = CampanhaViva::iniciar(&primeira, "modo");
        let dona_b = CampanhaViva::iniciar(&segunda, "modo");

        assert!(primeira.join("target/pinker-flake-evidence/.lock").is_dir());
        assert!(segunda.join("target/pinker-flake-evidence/.lock").is_dir());
        assert_ne!(dona_a.lote, dona_b.lote);

        assert_eq!(dona_a.encerrar(15), EXIT_INTERROMPIDO);
        assert_eq!(dona_b.encerrar(15), EXIT_INTERROMPIDO);

        assert!(!primeira.join("target/pinker-flake-evidence/.lock").exists());
        assert!(!segunda.join("target/pinker-flake-evidence/.lock").exists());

        let _ = fs::remove_dir_all(&primeira);
        let _ = fs::remove_dir_all(&segunda);
    }

    #[test]
    fn limpeza_nao_deixa_residuo_de_publicacao() {
        let raiz = raiz_isolada("limpeza");
        let binario = binario_falso(&raiz, RESUMO_COM_TESTE, 0);
        assert!(binario.is_file());

        let restos: Vec<PathBuf> = fs::read_dir(&raiz)
            .expect("ler raiz")
            .flatten()
            .map(|entrada| entrada.path())
            .filter(|caminho| {
                caminho
                    .extension()
                    .map(|extensao| extensao == "fonte" || extensao == "parcial")
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            restos.is_empty(),
            "sem intermediários residuais: {restos:?}"
        );

        fs::remove_dir_all(&raiz).expect("remover raiz");
        assert!(!raiz.exists(), "limpeza determinística");
    }

    // --- sensibilidade sobre a própria fonte --------------------------------

    #[test]
    fn publicacao_e_a_unica_autoridade_de_bit_executavel() {
        // Detecta a reintrodução de escrita direta em caminho executável e de
        // criação de processo materializador fora da região autorizada.
        //
        // O recorte usa sentinelas estáveis em vez de contagem de chaves: chave
        // aparece em string, comentário e macro, e um contador ingênuo fecha a
        // região no lugar errado — foi exatamente o defeito da tentativa
        // anterior desta regressão.
        let fonte = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pinker_flake_runner_tests.rs"),
        )
        .expect("ler a própria suíte");

        // Construídas por concatenação para que os literais desta regressão não
        // casem consigo mesmos ao inspecionar a própria fonte.
        let abertura = format!("// pinker-fork-{}:inicio", "autorizado");
        let fechamento = format!("// pinker-fork-{}:fim", "autorizado");
        let inicio_marca = abertura.as_str();
        let fim_marca = fechamento.as_str();
        assert_eq!(
            fonte.matches(inicio_marca).count(),
            1,
            "a sentinela de abertura precisa ser única"
        );
        assert_eq!(
            fonte.matches(fim_marca).count(),
            1,
            "a sentinela de fechamento precisa ser única"
        );
        let abre = fonte.find(inicio_marca).expect("abertura");
        let fecha = fonte.find(fim_marca).expect("fechamento");
        assert!(abre < fecha, "as sentinelas precisam estar em ordem");

        let autorizada = &fonte[abre..fecha];
        assert_eq!(
            autorizada.matches("Command::new(\"install\")").count(),
            1,
            "a região autorizada contém exatamente o publicador auxiliar"
        );

        // Fora da região autorizada, nenhuma escrita direta em caminho
        // executável. Este teste vive fora dela, então o próprio texto das
        // asserções entra na amostra: as chaves de busca são construídas por
        // concatenação para não casarem consigo mesmas.
        let fora = format!("{}{}", &fonte[..abre], &fonte[fecha..]);
        let copia = format!("fs::{}(", "copy");
        assert_eq!(
            fora.matches(copia.as_str()).count(),
            0,
            "cópia direta abre o destino para escrita neste processo"
        );
        let criar = format!("fs::File::{}(", "create");
        assert_eq!(
            fora.matches(criar.as_str()).count(),
            1,
            "a única criação fora da região autorizada é a da prova de causalidade"
        );
        let modo = format!("set_{}(0o755)", "mode");
        assert_eq!(
            fora.matches(modo.as_str()).count(),
            1,
            "o único bit de execução aplicado por escrita direta é o da prova"
        );
        let posicao = fora.find(modo.as_str()).expect("posição da prova");
        assert!(
            fora[..posicao].contains("fn descritor_gravavel_do_proprio_processo_impede_exec"),
            "a escrita direta remanescente vive na prova de causalidade"
        );

        // A publicação precisa continuar sendo a única porta.
        let nome_autoridade = format!("fn publicar_{}", "executavel");
        assert_eq!(
            fonte.matches(nome_autoridade.as_str()).count(),
            1,
            "a autoridade de publicação precisa ser única"
        );
        assert!(
            autorizada.contains("fs::rename(&parcial, destino)"),
            "o nome final precisa nascer por rename, depois do fechamento"
        );
    }
}
