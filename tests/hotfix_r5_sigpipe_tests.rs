//! Hotfix pós-PR #411 — item R5: a disposição de `SIGPIPE` do runtime nativo
//! não pode depender da ordem de execução do programa.
//!
//! Antes da correção, `SIG_IGN` só era instalado a partir do primeiro `falar`.
//! Um programa que escrevesse stdin de um filho **sem** ter falado antes morria
//! por sinal (exit 141, stderr vazio) em vez de alcançar a agregação de erro de
//! `executar_com_entrada`; com um `falar` antes, o mesmo programa terminava com
//! exit 1 e diagnóstico. O interpretador sempre deu exit 1 nos dois casos, então
//! a divergência também quebrava paridade.
//!
//! A evidência aqui é a matriz completa exigida pelo contrato: stdout anterior ×
//! comportamento do filho × tamanho do stdin, comparando interpretador e nativo
//! célula a célula.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EXEMPLO: &str = "examples/hotfix_r5_sigpipe_matriz_valido.pink";

/// Teto de tempo por célula. Qualquer deadlock (writer órfão, `wait` antes da
/// escrita, pipe nunca fechado) estoura este limite em vez de travar a suíte.
const LIMITE: Duration = Duration::from_secs(30);

/// Comportamentos do filho diante do stdin escrito pelo pai.
const MODOS_FILHO: [&str; 5] = [
    "encerra",    // encerra na hora, sem ler
    "espera",     // nunca lê e permanece vivo antes de encerrar
    "le-tudo",    // lê tudo até EOF
    "le-parcial", // lê um byte e sai
    "fecha-cedo", // fecha o stdin explicitamente e sai
];

/// Tamanhos de stdin, incluindo a capacidade padrão do pipe (65536) e valores
/// acima dela, onde a escrita necessariamente bloqueia antes de falhar.
const TAMANHOS: [u64; 6] = [0, 1, 4096, 65536, 262144, 1048576];

/// Escritas em stdout antes do processo: nenhuma, um `falar`, várias.
const ESCRITAS_ANTERIORES: [u64; 3] = [0, 1, 2];

struct Saida {
    codigo: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Saida {
    /// Classe observável do resultado, usada para comparar back-ends sem
    /// depender do código exato do filho.
    fn classe(&self) -> &'static str {
        match self.codigo {
            Some(0) => "ok",
            Some(1) if self.stderr.contains("falha ao escrever stdin") => "epipe-diagnosticado",
            Some(_) => "outro-erro",
            None => "terminado-por-sinal",
        }
    }
}

/// Executa com teto de tempo. Devolve `None` quando o limite estoura — e, nesse
/// caso, **mata** o processo antes de voltar, para que o próprio detector de
/// deadlock não deixe processo órfão atrás de si.
fn executar_com_limite(mut comando: Command) -> Option<Saida> {
    let mut filho = comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("disparar processo da célula");
    let mut stdout = filho.stdout.take().expect("stdout da célula");
    let mut stderr = filho.stderr.take().expect("stderr da célula");

    // As leituras acontecem em threads para que um pipe cheio não vire deadlock
    // do próprio teste enquanto se espera o encerramento do filho.
    let leitor_stdout = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut texto = String::new();
        let _ = stdout.read_to_string(&mut texto);
        texto
    });
    let leitor_stderr = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut texto = String::new();
        let _ = stderr.read_to_string(&mut texto);
        texto
    });

    // `try_wait` em laço mantém o handle nesta thread, o que é justamente o que
    // permite matar o filho quando o limite estoura.
    let prazo = Instant::now() + LIMITE;
    let codigo = loop {
        match filho.try_wait().expect("consultar processo da célula") {
            Some(status) => break status.code(),
            None if Instant::now() >= prazo => {
                let _ = filho.kill();
                let _ = filho.wait();
                let _ = leitor_stdout.join();
                let _ = leitor_stderr.join();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    Some(Saida {
        codigo,
        stdout: leitor_stdout.join().unwrap_or_default(),
        stderr: leitor_stderr.join().unwrap_or_default(),
    })
}

fn helper_filho() -> &'static str {
    env!("CARGO_BIN_EXE_pinker_hf412_filho_stdin")
}

fn celula_interpretada(modo: &str, tamanho: u64, antes: u64) -> Option<Saida> {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.args([
        "--run",
        EXEMPLO,
        "--",
        helper_filho(),
        modo,
        &tamanho.to_string(),
        &antes.to_string(),
    ]);
    executar_com_limite(comando)
}

fn celula_nativa(binario: &Path, modo: &str, tamanho: u64, antes: u64) -> Option<Saida> {
    let mut comando = Command::new(binario);
    comando.args([
        helper_filho(),
        modo,
        &tamanho.to_string(),
        &antes.to_string(),
    ]);
    executar_com_limite(comando)
}

/// Compila o exemplo uma única vez; a matriz inteira roda sobre o mesmo ELF.
fn compilar_nativo(runtime_lib: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_hf412_r5_{nanos}"));
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(&out_dir)
        .arg(EXEMPLO)
        .env("PINKER_RT_LIB", runtime_lib)
        .output()
        .expect("invocar pink build --nativo");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    out_dir.join("hotfix_r5_sigpipe_matriz_valido")
}

/// Invariantes exigidas de qualquer célula, em qualquer back-end.
fn exigir_invariantes(rotulo: &str, saida: &Saida) {
    assert!(
        saida.codigo.is_some(),
        "{rotulo}: processo terminou por sinal (SIGPIPE não pode escapar do runtime)"
    );
    assert_ne!(
        saida.codigo,
        Some(141),
        "{rotulo}: exit 141 indica morte por SIGPIPE mascarada pelo shell"
    );
    match saida.classe() {
        "ok" => assert!(
            saida.stdout.contains("codigo="),
            "{rotulo}: sucesso sem a linha final do programa\nstdout: {}",
            saida.stdout
        ),
        "epipe-diagnosticado" => assert!(
            !saida.stderr.is_empty(),
            "{rotulo}: EPIPE precisa virar diagnóstico visível"
        ),
        outra => panic!(
            "{rotulo}: classe inesperada {outra} (exit {:?})\nstderr: {}",
            saida.codigo, saida.stderr
        ),
    }
}

// @pinker-nav:start evidencia.hotfix.r5-sigpipe-ordem
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Matriz R5 de SIGPIPE: stdout anterior (nenhum/um falar/várias escritas) × comportamento do filho (encerra, espera sem ler, lê tudo, lê parcial, fecha stdin cedo) × tamanho de stdin (0, 1, 4096, 65536, 262144, acima da capacidade do pipe), exigindo em cada célula ausência de término por sinal, ausência de exit 141, ausência de deadlock sob teto de tempo, EPIPE convertido em diagnóstico e paridade de classe entre interpretador e nativo; cobre ainda a disposição de SIGPIPE herdada pelo filho após exec, medida antes da inicialização da std.
#[test]
fn r5_matriz_sigpipe_independe_da_ordem_e_mantem_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let binario = compilar_nativo(&runtime_lib);

    for modo in MODOS_FILHO {
        for tamanho in TAMANHOS {
            for antes in ESCRITAS_ANTERIORES {
                let rotulo = format!("modo={modo} tamanho={tamanho} escritas_antes={antes}");

                let interpretado = celula_interpretada(modo, tamanho, antes).unwrap_or_else(|| {
                    panic!("{rotulo}: interpretador não terminou dentro do limite (deadlock)")
                });
                let nativo = celula_nativa(&binario, modo, tamanho, antes).unwrap_or_else(|| {
                    panic!("{rotulo}: nativo não terminou dentro do limite (deadlock)")
                });

                exigir_invariantes(&format!("interpretador {rotulo}"), &interpretado);
                exigir_invariantes(&format!("nativo {rotulo}"), &nativo);

                assert_eq!(
                    interpretado.classe(),
                    nativo.classe(),
                    "{rotulo}: back-ends divergem\ninterpretador: exit {:?} / {}\nnativo: exit {:?} / {}",
                    interpretado.codigo,
                    interpretado.stderr,
                    nativo.codigo,
                    nativo.stderr
                );
                assert_eq!(
                    interpretado.stdout, nativo.stdout,
                    "{rotulo}: stdout divergente entre back-ends"
                );
            }
        }
    }

    let _ = std::fs::remove_dir_all(binario.parent().expect("diretório do build"));
}

/// A disposição instalada pelo runtime é confinada ao processo Pinker.
///
/// `SIG_IGN` sobrevive a `exec`, então esta célula existe para provar que o
/// filho ainda observa `SIG_DFL`. A leitura acontece num construtor de
/// `.init_array` do binário auxiliar, antes do `lang_start` da std — que
/// instalaria `SIG_IGN` e mascararia a medida. `codigo=0` significa `SIG_DFL`
/// herdado; `codigo=1` significaria `SIG_IGN` vazando para o filho.
#[test]
fn r5_filho_herda_disposicao_padrao_de_sigpipe() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let binario = compilar_nativo(&runtime_lib);

    for antes in ESCRITAS_ANTERIORES {
        let interpretado = celula_interpretada("sigpipe-disposicao", 16, antes)
            .expect("interpretador dentro do limite");
        let nativo = celula_nativa(&binario, "sigpipe-disposicao", 16, antes)
            .expect("nativo dentro do limite");
        for (rotulo, saida) in [("interpretador", &interpretado), ("nativo", &nativo)] {
            assert!(
                saida.stdout.contains("codigo=0"),
                "{rotulo} (escritas_antes={antes}): filho não herdou SIG_DFL para SIGPIPE\nstdout: {}",
                saida.stdout
            );
        }
    }

    let _ = std::fs::remove_dir_all(binario.parent().expect("diretório do build"));
}
// @pinker-nav:end evidencia.hotfix.r5-sigpipe-ordem
