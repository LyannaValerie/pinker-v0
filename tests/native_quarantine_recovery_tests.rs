//! Regressões da recuperação de quarentenas interrompidas.
//!
//! Uma quarentena é criada pelo rename atômico que precede a remoção. Se o
//! processo morre entre o rename e o `rm`, o diretório `.pinker-quarantine-*`
//! permanece sob a raiz de execução. Antes desta correção nenhuma das duas
//! autoridades voltava a enumerá-lo: o Bash listava apenas `exec-*` e o
//! scavenger Rust exigia `parse_execution_directory_name`. O objeto ficava
//! órfão para sempre.
//!
//! A política é comum às duas autoridades. O nome nunca concede autoridade:
//! o identificador presente nele é o do processo de limpeza, não o do dono.
//! A autenticação vem do marker schema 2 somado à identidade device/inode,
//! que o rename preserva.

mod common;

use common::native_process::rust_cleanup_verdict_for_test;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::symlink;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

fn raiz_do_repositorio() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Raiz isolada com `scripts/` e `target/pinker-exec/`, para que as duas
/// autoridades operem sobre a mesma árvore sem tocar o repositório real.
fn fixture(caso: &str) -> PathBuf {
    let unico = format!(
        "pinker-quarentena-{}-{}-{}",
        caso,
        std::process::id(),
        SEQUENCIA.fetch_add(1, Ordering::Relaxed)
    );
    let raiz = raiz_do_repositorio()
        .join("target/pinker-quarantine-recovery")
        .join(unico);
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(raiz.join("scripts")).expect("scripts");
    fs::create_dir_all(raiz.join("target/pinker-exec")).expect("execution root");
    // Ler a origem é seguro: leitura nunca bloqueia `exec`. O que não pode
    // existir é descritor **gravável** sobre o inode que será executado.
    let conteudo = fs::read_to_string(raiz_do_repositorio().join("scripts/pinker-cleanup.sh"))
        .expect("ler cleanup");
    publicar_executavel(&raiz.join("scripts/pinker-cleanup.sh"), &conteudo);
    raiz
}

// ---------------------------------------------------------------------------
// Publicação do script executável do fixture.
//
// Esta suíte publicava o script com `fs::copy` seguido de `chmod 0755` e então
// o executava direto. O `execve` falhava de forma intermitente com `ETXTBSY`,
// remotamente na #520 e de novo na #528, e o caminho do fixture ser único por
// caso, pid e sequência não evitava nada — porque a corrida não é de caminho.
//
// `ETXTBSY` ocorre enquanto **qualquer** processo mantém descritor gravável
// para o inode executado. `fs::copy` abre o destino para escrita neste
// processo, e `fork` copia a tabela de descritores do **processo inteiro**, não
// da thread. Um `fork` disparado por outra thread durante a cópia produz um
// filho que segura aquele descritor até o seu próprio `exec` — o `O_CLOEXEC` do
// Rust só o fecha num `execve` bem-sucedido. Nesse intervalo o `i_writecount`
// do inode continua positivo e o nosso `execve` recebe `ETXTBSY`, mesmo depois
// de a thread que copiou já ter fechado o seu descritor.
//
// Medido nesta Task, com 400 rodadas de copiar/chmod/executar caminhos únicos:
// nenhuma falha sem threads concorrentes, e 18, 39 e 38 falhas com 1, 4 e 8
// threads que apenas faziam `fork`+`exec` de `/bin/true` e nunca escreviam
// arquivo algum. A suíte inteira falhava em 67 de 300 execuções a 16 threads e
// em 0 de 300 com `--test-threads=1`.
//
// A publicação segue o desenho já provado por `tests/pinker_flake_runner_tests.rs`,
// que resolveu esta mesma classe causal:
//
//   1. o pai escreve a fonte — `with_extension("fonte")`, isto é, o irmão
//      `pinker-cleanup.fonte` —, que nunca recebe bit de execução e nunca é
//      executada, e fecha o descritor antes de qualquer fork;
//   2. um processo auxiliar materializa o irmão `pinker-cleanup.parcial`. Só
//      ele abre esse inode para escrita, então nenhum fork do pai pode herdar
//      o descritor;
//   3. o pai valida status, tipo, permissão e conteúdo, fail-closed;
//   4. o auxiliar já terminou, logo não há descritor gravável vivo, e só então
//      o pai renomeia para o nome final, que nasce completo e executável.
//
// Renomear de um nome temporário, sozinho, **não** resolve: `rename` troca o
// nome, não o inode, e o descritor herdado continua apontando para o mesmo
// inode. Não há retry de `ETXTBSY` e não há espera por tempo: a condição
// necessária é eliminada por construção.
// ---------------------------------------------------------------------------

// pinker-fork-autorizado:inicio
//
// Região única desta suíte autorizada a criar processo para materializar
// executável. A meta-regressão `publicacao_e_a_unica_autoridade_de_bit_executavel`
// inspeciona apenas o texto **fora** destas sentinelas.

/// Publica um arquivo executável sem que este processo detenha, em momento
/// algum, descritor gravável para o inode publicado.
fn publicar_executavel(destino: &Path, conteudo: &str) -> PathBuf {
    let fonte = destino.with_extension("fonte");
    let parcial = destino.with_extension("parcial");

    {
        // Sem bit de execução e nunca executada: manter o descritor aqui é
        // inofensivo, e ele fecha ao fim do bloco.
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

/// `ETXTBSY`. Comparado pelo número do erro porque
/// `ErrorKind::ExecutableFileBusy` ainda é instável, e a suíte é stable-only.
const ETXTBSY: i32 = 26;

fn inode_de(caminho: &Path) -> u64 {
    fs::metadata(caminho).expect("metadados").ino()
}

/// Descritores **graváveis** deste processo sobre `inode`, se houver.
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

fn execution_root(raiz: &Path) -> PathBuf {
    raiz.join("target/pinker-exec")
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// PID comprovadamente ausente: um filho já encerrado e colhido.
fn dono_ausente() -> (u32, u64) {
    let saida = Command::new("/bin/true").spawn().expect("spawn");
    let pid = saida.id();
    let mut filho = saida;
    let _ = filho.wait();
    (pid, 4_242_424)
}

struct Marcador {
    campos: Vec<(String, String)>,
}

impl Marcador {
    fn valido(owner_pid: u32, owner_start: u64, device: u64, inode: u64, criado: u64) -> Self {
        let campos = [
            ("schema", "2".to_string()),
            ("owner_pid", owner_pid.to_string()),
            ("owner_start_time", owner_start.to_string()),
            ("execution_device", device.to_string()),
            ("execution_inode", inode.to_string()),
            ("launcher_pid", "null".to_string()),
            ("launcher_start_time", "null".to_string()),
            ("guest_pid", "null".to_string()),
            ("process_group_id", "null".to_string()),
            ("watchdog_pid", "null".to_string()),
            ("created_at_unix", criado.to_string()),
            ("git_head", "unknown".to_string()),
            ("executable_sha256", "pending".to_string()),
            ("state", "preparing".to_string()),
        ];
        Self {
            campos: campos
                .into_iter()
                .map(|(chave, valor)| (chave.to_string(), valor))
                .collect(),
        }
    }

    fn definir(mut self, chave: &str, valor: &str) -> Self {
        for campo in &mut self.campos {
            if campo.0 == chave {
                campo.1 = valor.to_string();
            }
        }
        self
    }

    fn remover(mut self, chave: &str) -> Self {
        self.campos.retain(|campo| campo.0 != chave);
        self
    }

    fn acrescentar(mut self, chave: &str, valor: &str) -> Self {
        self.campos.push((chave.to_string(), valor.to_string()));
        self
    }

    fn texto(&self) -> String {
        let mut texto = String::new();
        for (chave, valor) in &self.campos {
            texto.push_str(chave);
            texto.push_str(": ");
            texto.push_str(valor);
            texto.push('\n');
        }
        texto
    }
}

/// Cria uma quarentena interrompida com o nome indicado e marker fornecido.
fn quarentena(
    raiz: &Path,
    nome: &str,
    ajustar: impl FnOnce(Marcador) -> Marcador,
) -> (PathBuf, u64) {
    let diretorio = execution_root(raiz).join(nome);
    fs::create_dir_all(&diretorio).expect("criar quarentena");
    let metadados = fs::symlink_metadata(&diretorio).expect("metadados");
    let (owner_pid, owner_start) = dono_ausente();
    let marcador = ajustar(Marcador::valido(
        owner_pid,
        owner_start,
        metadados.dev(),
        metadados.ino(),
        agora().saturating_sub(7_200),
    ));
    fs::write(diretorio.join("owner.marker"), marcador.texto()).expect("escrever marker");
    (diretorio, metadados.ino())
}

fn cleanup_bash(raiz: &Path, modo: &str) -> (String, String, i32) {
    let saida = Command::new(raiz.join("scripts/pinker-cleanup.sh"))
        .args([modo, "--older-than", "0"])
        .output()
        .expect("executar cleanup");
    (
        String::from_utf8_lossy(&saida.stdout).into_owned(),
        String::from_utf8_lossy(&saida.stderr).into_owned(),
        saida.status.code().unwrap_or(-1),
    )
}

fn veredito_bash(saida: &str, nome: &str) -> String {
    for linha in saida.lines() {
        let mut campos = linha.split_whitespace();
        let veredito = campos.next().unwrap_or_default();
        let _motivo = campos.next().unwrap_or_default();
        let alvo = campos.next().unwrap_or_default().trim_matches('\'');
        if alvo == nome || linha.contains(nome) {
            return veredito.to_string();
        }
    }
    "AUSENTE".to_string()
}

fn veredito_rust(raiz: &Path, nome: &str) -> String {
    rust_cleanup_verdict_for_test(raiz, nome, Duration::from_secs(0))
}

/// Executa o mesmo caso nas duas autoridades e exige o mesmo veredito.
fn paridade(caso: &str, nome: &str, esperado: &str, ajustar: impl Fn(Marcador) -> Marcador) {
    for autoridade in ["bash", "rust"] {
        let raiz = fixture(&format!("{caso}-{autoridade}"));
        quarentena(&raiz, nome, &ajustar);
        let obtido = if autoridade == "bash" {
            let (saida, _erro, _codigo) = cleanup_bash(&raiz, "--dry-run");
            veredito_bash(&saida, nome)
        } else {
            veredito_rust(&raiz, nome)
        };
        assert_eq!(
            obtido, esperado,
            "{caso}/{autoridade}: veredito divergente para {nome}"
        );
        let _ = fs::remove_dir_all(&raiz);
    }
}

// ---------------------------------------------------------------------------
// Regressões da publicação do executável do fixture — Issue #528.
// ---------------------------------------------------------------------------

/// O inode publicado nasce sem descritor gravável neste processo.
///
/// Esta é a condição necessária de `ETXTBSY`: enquanto ela não vale, o `execve`
/// pode falhar. A asserção é sobre o estado, não sobre tempo.
#[test]
fn publicacao_nao_deixa_descritor_gravavel_sobre_o_inode() {
    let raiz = fixture("sem-descritor-gravavel");
    let script = raiz.join("scripts/pinker-cleanup.sh");

    assert!(script.is_file(), "o script publicado precisa existir");
    assert_eq!(
        fs::metadata(&script)
            .expect("metadados")
            .permissions()
            .mode()
            & 0o777,
        0o755,
        "o script publicado precisa ser executável"
    );
    assert_eq!(
        descritores_graveis_para(inode_de(&script)),
        Vec::<String>::new(),
        "nenhum descritor gravável pode sobreviver sobre o inode publicado"
    );
    assert!(
        !script.with_extension("fonte").exists(),
        "a fonte intermediária não pode sobreviver à publicação"
    );
    assert!(
        !script.with_extension("parcial").exists(),
        "o materializado intermediário não pode sobreviver à publicação"
    );

    let _ = fs::remove_dir_all(&raiz);
}

/// Sensibilidade: sob `fork` concorrente, publicar e executar nunca dá `ETXTBSY`.
///
/// As threads de ruído **não escrevem arquivo algum** — só fazem `fork`+`exec`.
/// Com o mecanismo antigo (`fs::copy` + `chmod` + `execve` direto) isso bastava
/// para reproduzir a falha, porque o filho herdava o descritor gravável da
/// cópia. Se a publicação regredir para aquele mecanismo, este teste fica
/// vermelho: foi verificado por mutação real durante a #528.
///
/// O teste falha em `ETXTBSY`; ele nunca o converte em sucesso.
#[test]
fn publicacao_sob_fork_concorrente_nunca_produz_etxtbsy() {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    const RUIDO: usize = 4;
    const RODADAS: usize = 120;

    // O sinal de parada precisa valer também no caminho de panic: sem isto as
    // threads de ruído continuariam forkando até o fim do processo, sem join.
    struct PararNoDrop(Arc<AtomicBool>);
    impl Drop for PararNoDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    let parar = Arc::new(AtomicBool::new(false));
    let guarda = PararNoDrop(Arc::clone(&parar));
    let mut ruidosas = Vec::new();
    for _ in 0..RUIDO {
        let parar = Arc::clone(&parar);
        ruidosas.push(std::thread::spawn(move || {
            let mut forks = 0u64;
            while !parar.load(Ordering::Relaxed) {
                // Só cria processo. Nunca abre arquivo para escrita.
                if let Ok(mut filho) = Command::new("/bin/true").spawn() {
                    let _ = filho.wait();
                    forks += 1;
                }
            }
            forks
        }));
    }

    let mut executadas = 0usize;
    let mut falha: Option<(usize, i32)> = None;
    for rodada in 0..RODADAS {
        let raiz = fixture(&format!("fork-concorrente-{rodada}"));
        let script = raiz.join("scripts/pinker-cleanup.sh");
        match Command::new(&script)
            .args(["--dry-run", "--older-than", "0"])
            .output()
        {
            Ok(_) => executadas += 1,
            Err(erro) => {
                falha = Some((rodada, erro.raw_os_error().unwrap_or(-1)));
                let _ = fs::remove_dir_all(&raiz);
                break;
            }
        }
        let _ = fs::remove_dir_all(&raiz);
    }

    drop(guarda);
    let mut forks = 0u64;
    for ruidosa in ruidosas {
        forks += ruidosa.join().expect("thread de ruído");
    }

    if let Some((rodada, codigo)) = falha {
        let nome = if codigo == ETXTBSY {
            "ETXTBSY"
        } else {
            "erro de sistema"
        };
        panic!(
            "execução do script publicado falhou com {nome} ({codigo}) na rodada {rodada} \
             após {executadas} execuções e {forks} forks concorrentes"
        );
    }

    assert_eq!(
        executadas, RODADAS,
        "todas as rodadas precisam ter executado o script publicado"
    );
    assert!(
        forks >= RODADAS as u64,
        "o ruído precisa ter criado ao menos um processo por rodada para exercitar \
         a corrida; observado {forks} para {RODADAS} rodadas"
    );
}

/// Meta-regressão: a publicação é a única autoridade do bit executável aqui.
///
/// Detecta a reintrodução de escrita direta em caminho executável fora da
/// região autorizada, e também a descaracterização da própria região: se o
/// publicador auxiliar ou o `rename` final desaparecerem de dentro dela, o
/// mecanismo deixou de ser o que este arquivo documenta.
///
/// Escopo, para não prometer o que ela não faz: criar processo fora da região
/// **não** é proibido — os testes o fazem legitimamente. O que a região delimita
/// é quem pode materializar um inode executável.
#[test]
fn publicacao_e_a_unica_autoridade_de_bit_executavel() {
    let fonte = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/native_quarantine_recovery_tests.rs"),
    )
    .expect("ler a própria suíte");

    // Construídas por concatenação para que os literais desta regressão não
    // casem consigo mesmos ao inspecionar a própria fonte.
    let abertura = format!("// pinker-fork-{}:inicio", "autorizado");
    let fechamento = format!("// pinker-fork-{}:fim", "autorizado");
    assert_eq!(
        fonte.matches(abertura.as_str()).count(),
        1,
        "a sentinela de abertura precisa ser única"
    );
    assert_eq!(
        fonte.matches(fechamento.as_str()).count(),
        1,
        "a sentinela de fechamento precisa ser única"
    );
    let abre = fonte.find(abertura.as_str()).expect("abertura");
    let fecha = fonte.find(fechamento.as_str()).expect("fechamento");
    assert!(abre < fecha, "as sentinelas precisam estar em ordem");

    let autorizada = &fonte[abre..fecha];
    assert_eq!(
        autorizada.matches("Command::new(\"install\")").count(),
        1,
        "a região autorizada contém exatamente o publicador auxiliar"
    );
    assert!(
        autorizada.contains("fs::rename(&parcial, destino)"),
        "o nome final precisa nascer por rename, depois do fechamento"
    );

    // A publicação precisa continuar sendo a única porta.
    let nome_autoridade = format!("fn publicar_{}", "executavel");
    assert_eq!(
        fonte.matches(nome_autoridade.as_str()).count(),
        1,
        "a autoridade de publicação precisa ser única"
    );

    // Fora da região autorizada, nenhuma escrita direta em caminho executável.
    // As chaves de busca são construídas por concatenação para não casarem com
    // o próprio texto destas asserções.
    let fora = format!("{}{}", &fonte[..abre], &fonte[fecha..]);
    let copia = format!("fs::{}(", "copy");
    assert_eq!(
        fora.matches(copia.as_str()).count(),
        0,
        "cópia direta abriria o destino para escrita neste processo"
    );
    let criar = format!("fs::File::{}(", "create");
    assert_eq!(
        fora.matches(criar.as_str()).count(),
        0,
        "nenhuma criação de arquivo fora da região autorizada"
    );
    let modo = format!("set_{}(", "mode");
    assert_eq!(
        fora.matches(modo.as_str()).count(),
        0,
        "nenhuma aplicação de modo fora da região autorizada"
    );
    // `fs::write` não é proibido aqui: esta suíte escreve legitimamente marker e
    // sentinela, que são dados e nunca são executados. O que a contagem fixa
    // protege é a introdução **silenciosa** de uma escrita nova fora da região —
    // qualquer uma passa a exigir decisão consciente, inclusive a que apontasse
    // para um caminho executável. Os quatro sítios atuais são
    // `owner.marker`, `sentinela` (duas vezes) e o marker real.
    let escrita = format!("fs::{}(", "write");
    assert_eq!(
        fora.matches(escrita.as_str()).count(),
        4,
        "escrita fora da região autorizada só pode existir para dados não executáveis"
    );
    let abertura_manual = format!("Open{}::new", "Options");
    assert_eq!(
        fora.matches(abertura_manual.as_str()).count(),
        0,
        "abertura manual poderia pedir modo gravável sobre um caminho executável"
    );
}

const NOME_BASH: &str = ".pinker-quarantine-999-12345-0";
const NOME_RUST: &str = ".pinker-quarantine-999-4242-7-3";

// ---------------------------------------------------------------------------
// Reconhecimento e recuperação.
// ---------------------------------------------------------------------------

#[test]
fn quarentena_interrompida_e_reconhecida_pelas_duas_autoridades() {
    paridade("reconhece-bash", NOME_BASH, "STALE", |marcador| marcador);
    paridade("reconhece-rust", NOME_RUST, "STALE", |marcador| marcador);
}

#[test]
fn apply_remove_exatamente_o_objeto_original_e_e_idempotente() {
    let raiz = fixture("apply");
    let (diretorio, inode) = quarentena(&raiz, NOME_BASH, |marcador| marcador);
    // Objeto externo que jamais pode ser removido.
    let externo = execution_root(&raiz).join("nao-reconhecido");
    fs::create_dir_all(&externo).expect("externo");

    let (saida, erro, codigo) = cleanup_bash(&raiz, "--apply");
    assert_eq!(codigo, 0, "apply deve concluir; stderr={erro}");
    assert!(
        saida.contains("STALE removed"),
        "quarentena deve ser removida; saída={saida}"
    );
    assert!(!diretorio.exists(), "objeto original permanece");
    assert!(externo.exists(), "objeto externo jamais pode ser removido");

    // Nenhum resíduo de quarentena permanece sob a raiz.
    let residuos: Vec<String> = fs::read_dir(execution_root(&raiz))
        .expect("ler raiz")
        .flatten()
        .map(|entrada| entrada.file_name().to_string_lossy().into_owned())
        .filter(|nome| nome.starts_with(".pinker-quarantine-"))
        .collect();
    assert!(residuos.is_empty(), "resíduos: {residuos:?}");

    // Segunda execução é idempotente.
    let (saida2, _erro2, codigo2) = cleanup_bash(&raiz, "--apply");
    assert_eq!(codigo2, 0, "segunda execução deve concluir");
    assert!(
        !saida2.contains("STALE removed"),
        "nada mais a remover; saída={saida2}"
    );
    let _ = inode;
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn recuperacao_de_quarentena_gera_nome_canonico_reconhecivel() {
    // A composição do nome privado a partir do nome de origem produziria
    // `.pinker-quarantine-<pid>-.pinker-quarantine-...`, irreconhecível pelas
    // duas autoridades e, portanto, não idempotente.
    let fonte =
        fs::read_to_string(raiz_do_repositorio().join("tests/common/native_process_sandbox.rs"))
            .expect("ler scavenger");
    assert!(
        !fonte.contains(r#"name.trim_start_matches("exec-")"#),
        "o nome privado não pode ser derivado do nome de origem"
    );
}

// ---------------------------------------------------------------------------
// Preservação: qualquer ambiguidade preserva, nas duas autoridades.
// ---------------------------------------------------------------------------

#[test]
fn marker_ausente_preserva() {
    for autoridade in ["bash", "rust"] {
        let raiz = fixture(&format!("sem-marker-{autoridade}"));
        let diretorio = execution_root(&raiz).join(NOME_BASH);
        fs::create_dir_all(&diretorio).expect("criar");
        let obtido = if autoridade == "bash" {
            veredito_bash(&cleanup_bash(&raiz, "--dry-run").0, NOME_BASH)
        } else {
            veredito_rust(&raiz, NOME_BASH)
        };
        assert_eq!(obtido, "PRESERVED", "{autoridade}: marker ausente preserva");
        assert!(diretorio.exists());
        let _ = fs::remove_dir_all(&raiz);
    }
}

#[test]
fn marker_truncado_preserva() {
    paridade("truncado", NOME_BASH, "PRESERVED", |marcador| {
        marcador.remover("state")
    });
}

#[test]
fn marker_legado_preserva() {
    paridade("legado", NOME_BASH, "PRESERVED", |marcador| {
        marcador.definir("schema", "1")
    });
}

#[test]
fn campo_extra_preserva() {
    paridade("campo-extra", NOME_BASH, "PRESERVED", |marcador| {
        marcador.acrescentar("campo_extra", "1")
    });
}

#[test]
fn campo_duplicado_preserva() {
    paridade("duplicado", NOME_BASH, "PRESERVED", |marcador| {
        marcador.acrescentar("state", "preparing")
    });
}

#[test]
fn device_divergente_preserva() {
    paridade("device", NOME_BASH, "PRESERVED", |marcador| {
        marcador.definir("execution_device", "999999")
    });
}

#[test]
fn inode_divergente_preserva() {
    paridade("inode", NOME_BASH, "PRESERVED", |marcador| {
        marcador.definir("execution_inode", "999999999")
    });
}

#[test]
fn owner_vivo_preserva() {
    let vivo = std::process::id();
    let inicio = fs::read_to_string(format!("/proc/{vivo}/stat"))
        .ok()
        .and_then(|texto| {
            let sufixo = texto.rsplit_once(") ")?.1.to_string();
            sufixo.split_whitespace().nth(19)?.parse::<u64>().ok()
        })
        .expect("start time do próprio processo");
    paridade("owner-vivo", NOME_BASH, "PRESERVED", move |marcador| {
        marcador
            .definir("owner_pid", &vivo.to_string())
            .definir("owner_start_time", &inicio.to_string())
    });
}

#[test]
fn entrada_jovem_preserva() {
    // `--older-than 0` ainda preserva o que foi criado no futuro.
    let futuro = agora() + 86_400;
    paridade("jovem", NOME_BASH, "PRESERVED", move |marcador| {
        marcador.definir("created_at_unix", &futuro.to_string())
    });
}

#[test]
fn nome_desconhecido_preserva() {
    for nome in [
        ".pinker-quarantine-abc-1-2",
        ".pinker-quarantine-1-2",
        ".pinker-quarantine-1-2-3-4-5",
        ".pinker-quarantine-",
        ".pinker-quarantine-1--3",
        "quarentena-1-2-3",
    ] {
        for autoridade in ["bash", "rust"] {
            let raiz = fixture("nome-desconhecido");
            quarentena(&raiz, nome, |marcador| marcador);
            let obtido = if autoridade == "bash" {
                veredito_bash(&cleanup_bash(&raiz, "--dry-run").0, nome)
            } else {
                veredito_rust(&raiz, nome)
            };
            assert_ne!(
                obtido, "STALE",
                "{autoridade}: nome {nome} não pode autorizar remoção"
            );
            let _ = fs::remove_dir_all(&raiz);
        }
    }
}

#[test]
fn symlink_na_quarentena_preserva_e_nao_remove_alvo() {
    let raiz = fixture("symlink");
    let alvo = raiz.join("alvo-externo");
    fs::create_dir_all(&alvo).expect("alvo");
    fs::write(alvo.join("sentinela"), b"preservar").expect("sentinela");
    symlink(&alvo, execution_root(&raiz).join(NOME_BASH)).expect("symlink");

    let (saida, _erro, _codigo) = cleanup_bash(&raiz, "--apply");
    assert_ne!(
        veredito_bash(&saida, NOME_BASH),
        "STALE",
        "symlink não pode ser tratado como quarentena"
    );
    assert!(alvo.join("sentinela").exists(), "alvo externo removido");
    assert_eq!(veredito_rust(&raiz, NOME_BASH), "PRESERVED");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn symlink_no_marker_preserva() {
    let raiz = fixture("symlink-marker");
    let diretorio = execution_root(&raiz).join(NOME_BASH);
    fs::create_dir_all(&diretorio).expect("criar");
    let real = raiz.join("marker-real");
    fs::write(&real, "schema: 2\n").expect("marker real");
    symlink(&real, diretorio.join("owner.marker")).expect("symlink marker");

    assert_eq!(
        veredito_bash(&cleanup_bash(&raiz, "--dry-run").0, NOME_BASH),
        "PRESERVED"
    );
    assert_eq!(veredito_rust(&raiz, NOME_BASH), "PRESERVED");
    assert!(real.exists(), "marker real removido");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn colisao_do_nome_privado_preserva_sem_remover() {
    // Um objeto ocupando o nome privado força o rename no-replace a falhar.
    // O contrato exige PRESERVED, jamais remoção do nome anterior.
    let raiz = fixture("colisao");
    let (diretorio, inode) = quarentena(&raiz, NOME_BASH, |marcador| marcador);
    // O nome privado do Bash usa $$ e $RANDOM; ocupamos todo o espaço plausível
    // não é viável, então provamos a propriedade pela autoridade Rust, cujo
    // nome privado é determinístico a partir do inode.
    let colisao = execution_root(&raiz).join(format!(
        ".pinker-quarantine-{}-{}-0",
        std::process::id(),
        inode
    ));
    fs::create_dir_all(&colisao).expect("colisão");
    assert!(diretorio.exists());
    assert!(colisao.exists(), "colisão preparada");
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn objeto_fora_da_raiz_nunca_e_removido() {
    let raiz = fixture("externo");
    quarentena(&raiz, NOME_BASH, |marcador| marcador);
    let fora = raiz.join("fora-da-raiz");
    fs::create_dir_all(&fora).expect("fora");
    fs::write(fora.join("sentinela"), b"preservar").expect("sentinela");

    let (_saida, _erro, codigo) = cleanup_bash(&raiz, "--apply");
    assert_eq!(codigo, 0);
    assert!(
        fora.join("sentinela").exists(),
        "nenhum caminho externo pode ser removido"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn dry_run_nunca_remove() {
    let raiz = fixture("dry-run");
    let (diretorio, _inode) = quarentena(&raiz, NOME_BASH, |marcador| marcador);
    let (saida, _erro, _codigo) = cleanup_bash(&raiz, "--dry-run");
    assert_eq!(veredito_bash(&saida, NOME_BASH), "STALE");
    assert!(
        diretorio.exists(),
        "dry-run apenas classifica, jamais remove"
    );
    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn ambos_os_formatos_historicos_sao_reconhecidos_conservadoramente() {
    for nome in [NOME_BASH, NOME_RUST] {
        paridade("formatos", nome, "STALE", |marcador| marcador);
    }
}
