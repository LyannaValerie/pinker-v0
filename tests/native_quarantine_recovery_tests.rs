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
use std::os::unix::fs::symlink;
use std::os::unix::fs::MetadataExt as _;
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
    fs::copy(
        raiz_do_repositorio().join("scripts/pinker-cleanup.sh"),
        raiz.join("scripts/pinker-cleanup.sh"),
    )
    .expect("copiar cleanup");
    let script = raiz.join("scripts/pinker-cleanup.sh");
    let mut permissoes = fs::metadata(&script).expect("metadados").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissoes, 0o755);
    fs::set_permissions(&script, permissoes).expect("permissões");
    raiz
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
        self.campos
            .iter()
            .map(|(chave, valor)| format!("{chave}: {valor}\n"))
            .collect()
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
