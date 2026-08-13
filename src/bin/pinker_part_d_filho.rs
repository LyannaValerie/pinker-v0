//! Fixture processual da Parte D. Não é uma superfície da linguagem.

use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};
use std::time::Duration;

fn hex(bytes: &[u8]) -> String {
    let mut saida = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut saida, "{byte:02x}").expect("String não falha");
    }
    saida
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn main() {
    let mut argumentos = std::env::args().skip(1);
    let modo = argumentos.next().unwrap_or_default();
    match modo.as_str() {
        "argv" => {
            for (indice, argumento) in argumentos.enumerate() {
                println!(
                    "ARG {indice} {} {}",
                    argumento.len(),
                    hex(argumento.as_bytes())
                );
            }
        }
        "stdin" => {
            let mut entrada = Vec::new();
            std::io::stdin()
                .read_to_end(&mut entrada)
                .expect("ler stdin até EOF");
            println!("STDIN {} {:016x}", entrada.len(), fnv1a64(&entrada));
            println!("EOF");
        }
        "small" => {
            print!("stdout-small");
            eprint!("stderr-small");
        }
        "large" => {
            let bloco_stdout = vec![b'O'; 16 * 1024];
            let bloco_stderr = vec![b'E'; 16 * 1024];
            let mut stdout = std::io::stdout().lock();
            let mut stderr = std::io::stderr().lock();
            for _ in 0..128 {
                stdout.write_all(&bloco_stdout).expect("stdout grande");
                stderr.write_all(&bloco_stderr).expect("stderr grande");
            }
        }
        "status" => {
            let codigo = argumentos
                .next()
                .expect("código")
                .parse::<i32>()
                .expect("código inteiro");
            std::process::exit(codigo);
        }
        "abnormal" => std::process::abort(),
        "invalid-stdout" => {
            std::io::stdout()
                .write_all(&[0xff, 0xfe, 0xfd])
                .expect("stdout inválido");
        }
        "invalid-stderr" => {
            std::io::stderr()
                .write_all(&[0xff, 0xfe, 0xfd])
                .expect("stderr inválido");
        }
        "cwd" => println!("{}", std::env::current_dir().expect("cwd").display()),
        "env" => {
            for chave in argumentos {
                match std::env::var_os(&chave) {
                    Some(valor) => {
                        let bytes = valor.to_string_lossy();
                        println!("ENV {chave} {} {}", bytes.len(), hex(bytes.as_bytes()));
                    }
                    None => println!("ENV {chave} AUSENTE"),
                }
            }
        }
        "counter" => {
            let caminho = argumentos.next().expect("arquivo contador");
            if let Some(pidfile) = argumentos.next() {
                std::fs::write(pidfile, std::process::id().to_string()).expect("registrar pid");
            }
            let atual = std::fs::read_to_string(&caminho)
                .ok()
                .and_then(|texto| texto.parse::<u64>().ok())
                .unwrap_or(0);
            std::fs::write(&caminho, (atual + 1).to_string()).expect("incrementar contador");
            print!("contador-stdout");
            eprint!("contador-stderr");
            std::process::exit(7);
        }
        "sleep" => {
            let ms = argumentos
                .next()
                .expect("milissegundos")
                .parse::<u64>()
                .expect("duração inteira");
            std::thread::sleep(Duration::from_millis(ms));
        }
        "sleep-pid" => {
            let ms = argumentos
                .next()
                .expect("milissegundos")
                .parse::<u64>()
                .expect("duração inteira");
            let pidfile = argumentos.next().expect("pidfile");
            std::fs::write(pidfile, std::process::id().to_string()).expect("registrar pid");
            std::thread::sleep(Duration::from_millis(ms));
        }
        "descendant" | "descendant-exit" => {
            let ms = argumentos
                .next()
                .expect("milissegundos do descendente")
                .parse::<u64>()
                .expect("duração inteira");
            let pidfile = argumentos.next().expect("pidfile");
            let descendente = Command::new(std::env::current_exe().expect("executável atual"))
                .arg("hold")
                .arg(ms.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("criar descendente");
            std::fs::write(pidfile, descendente.id().to_string()).expect("registrar pid");
            if modo == "descendant" {
                std::thread::sleep(Duration::from_secs(5));
            }
        }
        "hold" => {
            let ms = argumentos
                .next()
                .expect("milissegundos")
                .parse::<u64>()
                .expect("duração inteira");
            std::thread::sleep(Duration::from_millis(ms));
        }
        outro => {
            eprintln!("modo desconhecido: {outro}");
            std::process::exit(64);
        }
    }
}
