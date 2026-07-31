//! Filho auditável para a matriz de SIGPIPE do runtime nativo (hotfix pós-PR
//! #411, item R5).
//!
//! Cada modo cobre uma célula da matriz de comportamento do filho diante do
//! stdin escrito pelo pai. Nenhum modo escreve em stdout, para que a evidência
//! de paridade compare apenas a saída do programa Pinker.
//!
//! O modo `sigpipe-disposicao` sonda a disposição herdada de `SIGPIPE`: o
//! runtime ignora `SIGPIPE` no processo Pinker e restaura `SIG_DFL` no caminho
//! pré-`exec`, então um filho correto deve observar `SIG_DFL`.

use std::io::Read as _;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

const SIGPIPE: i32 = 13;
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
const SIG_ERR: usize = usize::MAX;

/// Sentinela distinta de qualquer disposição real; sinaliza que o construtor
/// não rodou.
const NAO_OBSERVADA: usize = usize::MAX - 1;

extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
}

/// Disposição de `SIGPIPE` herdada através de `exec`, capturada **antes** da
/// inicialização do runtime Rust.
///
/// Isto é essencial: o `lang_start` da std instala `SIG_IGN` para `SIGPIPE`
/// antes de `main`, então uma sonda feita em `main` mediria a std, não a
/// herança. O construtor abaixo roda em `.init_array`, durante a partida da
/// libc, antes de qualquer código da std.
static SIGPIPE_HERDADO: AtomicUsize = AtomicUsize::new(NAO_OBSERVADA);

/// Lê a disposição corrente de `SIGPIPE` restaurando-a em seguida. `signal(2)`
/// devolve a disposição anterior, então ler exige uma troca imediatamente
/// desfeita.
///
/// # Safety
/// Roda em `.init_array`, antes de threads e antes da std; usa apenas
/// `signal(2)`.
unsafe extern "C" fn capturar_sigpipe_herdado() {
    let anterior = signal(SIGPIPE, SIG_DFL);
    if anterior != SIG_ERR && anterior != SIG_DFL {
        signal(SIGPIPE, anterior);
    }
    SIGPIPE_HERDADO.store(anterior, Ordering::SeqCst);
}

#[used]
#[cfg_attr(target_os = "linux", link_section = ".init_array")]
static CONSTRUTOR_SIGPIPE: unsafe extern "C" fn() = capturar_sigpipe_herdado;

/// Lê e descarta o stdin até EOF; devolve a quantidade de bytes lidos.
fn drenar_stdin() -> u64 {
    let mut entrada = std::io::stdin().lock();
    let mut buffer = [0u8; 8192];
    let mut total = 0u64;
    loop {
        match entrada.read(&mut buffer) {
            Ok(0) => return total,
            Ok(lidos) => total += lidos as u64,
            Err(erro) if erro.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return total,
        }
    }
}

fn main() -> ExitCode {
    let modo = std::env::args().nth(1).unwrap_or_default();
    match modo.as_str() {
        // Encerra sem ler nada, imediatamente.
        "encerra" => ExitCode::from(0),
        // Nunca lê e permanece vivo tempo suficiente para o pai preencher o
        // buffer do pipe e bloquear; só então encerra, fechando o pipe.
        "espera" => {
            std::thread::sleep(std::time::Duration::from_millis(300));
            ExitCode::from(0)
        }
        // Lê tudo até EOF.
        "le-tudo" => {
            drenar_stdin();
            ExitCode::from(0)
        }
        // Lê um único byte e sai, deixando o resto do stdin sem consumidor.
        "le-parcial" => {
            let mut byte = [0u8; 1];
            let _ = std::io::stdin().lock().read(&mut byte);
            ExitCode::from(0)
        }
        // Fecha o stdin explicitamente antes de encerrar.
        "fecha-cedo" => {
            drop(std::io::stdin());
            #[cfg(unix)]
            {
                extern "C" {
                    fn close(fd: i32) -> i32;
                }
                // SAFETY: fecha apenas o descritor 0 do próprio processo.
                unsafe {
                    close(0);
                }
            }
            ExitCode::from(0)
        }
        // Relata a disposição de SIGPIPE herdada através de `exec`, capturada
        // pelo construtor de `.init_array`.
        "sigpipe-disposicao" => match SIGPIPE_HERDADO.load(Ordering::SeqCst) {
            SIG_DFL => ExitCode::from(0),
            SIG_IGN => ExitCode::from(1),
            SIG_ERR => ExitCode::from(3),
            NAO_OBSERVADA => ExitCode::from(4),
            _ => ExitCode::from(2),
        },
        _ => ExitCode::from(9),
    }
}
