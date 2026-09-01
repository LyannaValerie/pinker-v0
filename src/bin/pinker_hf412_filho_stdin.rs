//! Filho auditável para a matriz de SIGPIPE do runtime nativo (hotfix pós-PR
//! #411, item R5).
//!
//! Cada modo cobre uma célula da matriz de comportamento do filho diante do
//! stdin escrito pelo pai. Nenhum modo escreve em stdout, para que a evidência
//! de paridade compare apenas a saída do programa Pinker.
//!
//! O modo `sigpipe-disposicao` sonda a disposição herdada de `SIGPIPE`: o
//! processo Pinker ignora `SIGPIPE` e a própria Pinker restaura `SIG_DFL` no
//! contexto pré-`exec` do filho, então um filho correto deve observar
//! `SIG_DFL`. A restauração é feita pelo runtime da Pinker; não é delegada à
//! biblioteca padrão.
//!
//! Os modos de sonda drenam o stdin antes de reportar. Isso não é cosmético:
//! sem a drenagem, o filho podia encerrar antes de o pai terminar de escrever,
//! e o pai colhia `EPIPE` — o diagnóstico controlado, correto em si, mas que
//! suprime a linha `codigo=` e transforma a medida da disposição numa corrida
//! decidida pelo escalonador da máquina.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

const SIGPIPE: i32 = 13;
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
const SIG_ERR: usize = usize::MAX;

// `espera` é uma sonda Linux da matriz: o pai escreve num pipe e o filho
// observa sua ocupação sem consumir um byte sequer.
#[cfg(target_os = "linux")]
const FIONREAD: usize = 0x541B;
#[cfg(target_os = "linux")]
const F_SETPIPE_SZ: i32 = 1031;
#[cfg(target_os = "linux")]
const F_GETPIPE_SZ: i32 = 1032;
#[cfg(target_os = "linux")]
const CAPACIDADE_ESPERADA_PIPE: i32 = 65536;
const TAMANHO_STDIN_R5: &str = "PINKER_R5_STDIN_TAMANHO";

/// Sentinela distinta de qualquer disposição real; sinaliza que o construtor
/// não rodou.
const NAO_OBSERVADA: usize = usize::MAX - 1;

extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
    #[cfg(target_os = "linux")]
    fn fcntl(fd: i32, command: i32, argument: i32) -> i32;
    #[cfg(target_os = "linux")]
    fn ioctl(fd: i32, request: usize, argument: *mut i32) -> i32;
}

/// Espera a escrita do pai alcançar uma condição observável, sem chamar
/// `read(2)`. `FIONREAD` vê bytes já escritos no pipe; quando alcança todo o
/// tamanho, a escrita pequena terminou. Para uma escrita maior, alcançar a
/// capacidade prova que o writer está sob pressão, pois este processo não
/// drena o pipe.
#[cfg(target_os = "linux")]
fn esperar_escrita(tamanho: u64) -> Result<(), ()> {
    if tamanho == 0 {
        return Ok(());
    }

    let mut capacidade = unsafe { fcntl(0, F_GETPIPE_SZ, 0) };
    // Só a célula de 65536 precisa ampliar o pipe. As escritas maiores devem
    // observar a capacidade real, não disputar a cota de pipes do host.
    if tamanho <= CAPACIDADE_ESPERADA_PIPE as u64 && capacidade < tamanho as i32 {
        capacidade = unsafe { fcntl(0, F_SETPIPE_SZ, tamanho as i32) };
    }
    if capacidade < tamanho.min(CAPACIDADE_ESPERADA_PIPE as u64) as i32 {
        return Err(());
    }
    let alvo = tamanho.min(capacidade as u64);

    loop {
        let mut disponiveis = 0;
        if unsafe { ioctl(0, FIONREAD, &mut disponiveis) } != 0 {
            return Err(());
        }
        if disponiveis as u64 >= alvo {
            return Ok(());
        }
        std::hint::spin_loop();
    }
}

/// Disposição de `SIGPIPE` herdada através de `exec`, capturada **antes** da
/// inicialização do runtime Rust.
///
/// Isto é essencial: o `lang_start` da std instala `SIG_IGN` para `SIGPIPE`
/// antes de `main`, então uma sonda feita em `main` mediria a std, não a
/// herança. O construtor abaixo roda em `.init_array`, durante a partida da
/// libc, antes de qualquer código da std.
///
/// A ordem não é presumida: o modo `sigpipe-sonda-ordem` compara esta medida
/// com uma segunda leitura feita em `main`. Como a std instala `SIG_IGN` entre
/// as duas, as leituras só divergem se o construtor tiver mesmo rodado antes —
/// o que torna a divergência prova positiva da ordem neste binário.
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

/// Lê o stdin até EOF e devolve o conteúdo como bytes.
fn ler_stdin() -> Vec<u8> {
    let mut entrada = std::io::stdin().lock();
    let mut conteudo = Vec::new();
    let _ = entrada.read_to_end(&mut conteudo);
    conteudo
}

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

/// `pipeline_minimo(produtor, consumidor)` não aceita argumentos por processo,
/// então os dois papéis do pipeline são selecionados pelo nome do executável.
/// O teste cria duas cópias do auxiliar com estes nomes.
const PAPEL_PRODUTOR: &str = "pinker_hf412_pipeline_produtor";
const PAPEL_CONSUMIDOR: &str = "pinker_hf412_pipeline_consumidor";

/// Modo implícito quando não há `argv[1]`, derivado do nome do executável.
fn modo_por_papel() -> &'static str {
    let Some(nome) = std::env::args().next() else {
        return "";
    };
    let base = std::path::Path::new(&nome)
        .file_name()
        .and_then(|nome| nome.to_str())
        .unwrap_or_default()
        .to_string();
    if base == PAPEL_PRODUTOR {
        "pipeline-produtor"
    } else if base == PAPEL_CONSUMIDOR {
        "pipeline-consumidor"
    } else {
        ""
    }
}

fn main() -> ExitCode {
    let argv1 = std::env::args().nth(1);
    let tamanho: u64 = std::env::var(TAMANHO_STDIN_R5)
        .ok()
        .and_then(|valor| valor.parse().ok())
        .unwrap_or_default();
    let modo = match argv1.as_deref() {
        Some(modo) if !modo.is_empty() => modo,
        _ => modo_por_papel(),
    };
    match modo {
        // Encerra sem ler nada, imediatamente.
        "encerra" => ExitCode::from(0),
        // Linux/test-only: nunca lê; só encerra depois que a ocupação do pipe
        // prova escrita completa ou pressão real sobre o writer.
        "espera" => {
            #[cfg(target_os = "linux")]
            {
                return match esperar_escrita(tamanho) {
                    Ok(()) => ExitCode::from(0),
                    Err(()) => {
                        eprintln!("sonda Linux do pipe indisponível");
                        ExitCode::from(2)
                    }
                };
            }
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!("modo espera requer Linux");
                ExitCode::from(2)
            }
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
        // pelo construtor de `.init_array`. Drena o stdin antes de reportar,
        // para que a medida nunca dispute com a escrita do pai.
        "sigpipe-disposicao" => {
            drenar_stdin();
            codigo_da_disposicao(SIGPIPE_HERDADO.load(Ordering::SeqCst))
        }
        // Igual ao anterior, mas o veredicto sai por stdout, para que
        // `capturar_stdout` também possa observá-lo.
        "sigpipe-disposicao-imprime" => {
            drenar_stdin();
            let rotulo = rotulo_da_disposicao(SIGPIPE_HERDADO.load(Ordering::SeqCst));
            print!("{rotulo}");
            let _ = std::io::stdout().flush();
            ExitCode::from(0)
        }
        // Veredicto por stderr, para `capturar_stderr`.
        "sigpipe-disposicao-imprime-erro" => {
            drenar_stdin();
            let rotulo = rotulo_da_disposicao(SIGPIPE_HERDADO.load(Ordering::SeqCst));
            eprint!("{rotulo}");
            let _ = std::io::stderr().flush();
            ExitCode::from(0)
        }
        // Produtor do pipeline: escreve a própria disposição no stdout, que é
        // o pipe lido pelo consumidor. Não lê stdin — o stdin do produtor é o
        // do processo Pinker, e lê-lo tornaria o exemplo dependente dele.
        "pipeline-produtor" => {
            let rotulo = rotulo_da_disposicao(SIGPIPE_HERDADO.load(Ordering::SeqCst));
            print!("{rotulo}");
            let _ = std::io::stdout().flush();
            ExitCode::from(0)
        }
        // Consumidor do pipeline: só devolve 0 quando as duas pontas do
        // pipeline observaram `SIG_DFL` — a sua própria e a que o produtor
        // reportou pelo pipe.
        "pipeline-consumidor" => {
            let do_produtor = ler_stdin();
            let propria = SIGPIPE_HERDADO.load(Ordering::SeqCst);
            if do_produtor != b"SIG_DFL" {
                return ExitCode::from(5);
            }
            codigo_da_disposicao(propria)
        }
        // Prova que a sonda de `.init_array` precede a inicialização da
        // linguagem: compara a medida do construtor com uma segunda leitura
        // feita em `main`, já depois de a std instalar `SIG_IGN`.
        "sigpipe-sonda-ordem" => {
            drenar_stdin();
            let construtor = SIGPIPE_HERDADO.load(Ordering::SeqCst);
            // SAFETY: leitura pontual em processo de teste sem outras threads;
            // `signal(2)` devolve a disposição anterior, restaurada em seguida.
            let em_main = unsafe {
                let anterior = signal(SIGPIPE, SIG_DFL);
                if anterior != SIG_ERR && anterior != SIG_DFL {
                    signal(SIGPIPE, anterior);
                }
                anterior
            };
            match (construtor, em_main) {
                // Herança SIG_DFL no construtor e SIG_IGN em `main`: a std
                // rodou entre as duas leituras, logo o construtor a precedeu.
                (SIG_DFL, SIG_IGN) => ExitCode::from(0),
                (NAO_OBSERVADA, _) => ExitCode::from(4),
                (SIG_ERR, _) | (_, SIG_ERR) => ExitCode::from(3),
                // Leituras iguais: a sonda não distingue as duas fases e a
                // ordem seria inconclusiva.
                (a, b) if a == b => ExitCode::from(1),
                _ => ExitCode::from(2),
            }
        }
        _ => ExitCode::from(9),
    }
}

/// Traduz uma disposição observada no código de saída da sonda.
fn codigo_da_disposicao(observada: usize) -> ExitCode {
    match observada {
        SIG_DFL => ExitCode::from(0),
        SIG_IGN => ExitCode::from(1),
        SIG_ERR => ExitCode::from(3),
        NAO_OBSERVADA => ExitCode::from(4),
        // Handler personalizado. Inalcançável por herança — a POSIX exige que
        // `exec` reinicie para `SIG_DFL` qualquer sinal com handler, e só
        // `SIG_IGN` sobrevive —, mas distinguido explicitamente para que a
        // sonda nunca confunda esse estado com os demais.
        _ => ExitCode::from(2),
    }
}

/// Nome textual da disposição, para os modos que reportam por stdout/stderr.
fn rotulo_da_disposicao(observada: usize) -> &'static str {
    match observada {
        SIG_DFL => "SIG_DFL",
        SIG_IGN => "SIG_IGN",
        SIG_ERR => "SIG_ERR",
        NAO_OBSERVADA => "NAO_OBSERVADA",
        _ => "HANDLER",
    }
}
