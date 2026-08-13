//! Execução hospedada da superfície estruturada de processos — Parte D, Step 3.
//!
//! Esta é a ponta operacional do interpretador. Ela não é usada pelas
//! superfícies históricas e não implementa o runtime nativo.

use crate::limite_tempo::LimiteTempo;
use crate::saida_processo::SaidaProcesso;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, ExitStatus, Stdio};
use std::time::{Duration, Instant};

// @pinker-nav:start processos.estruturado.hospedado
// @pinker-nav:domain processos
// @pinker-nav:layer interpreter
// @pinker-nav:summary Implementa a nova execução estruturada apenas no interpretador: recusa Ate(0) antes de configurar ou criar o filho, configura argv/cwd/ambiente e PATH saneada, faz um único spawn para os demais limites, move stdin/stdout/stderr por uma única malha poll com fds não-bloqueantes e quantum justo por canal, aplica deadline monotônico, mata e reapa somente o filho direto no timeout ou erro pós-spawn, valida UTF-8 estritamente após reap e só então devolve um snapshot imutável.

/// PATH default da nova superfície. Um overlay explícito de PATH é aplicado
/// depois e, portanto, vence este valor.
pub const PATH_PROCESSOS_ESTRUTURADOS: &str = "/usr/local/bin:/usr/bin:/bin";

/// Configuração já resolvida das representações do interpretador.
pub(crate) struct ConfiguracaoProcessoEstruturado<'a> {
    pub programa: &'a str,
    pub argumentos: &'a [String],
    pub entrada: &'a str,
    pub diretorio: &'a str,
    pub ambiente: &'a HashMap<String, String>,
    pub limite: LimiteTempo,
}

pub(crate) fn executar(
    configuracao: &ConfiguracaoProcessoEstruturado<'_>,
) -> Result<SaidaProcesso, String> {
    executar_com_controle(configuracao, false, None)
}

fn executar_com_controle(
    configuracao: &ConfiguracaoProcessoEstruturado<'_>,
    falhar_setup_depois_spawn: bool,
    mut pid_observado: Option<&mut u32>,
) -> Result<SaidaProcesso, String> {
    let operacao = crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO;
    if configuracao.programa.is_empty() {
        return Err(format!("programa vazio em '{operacao}'"));
    }
    for (chave, valor) in configuracao.ambiente {
        crate::ambiente_processo::validar_entrada(chave, valor)
            .map_err(|erro| format!("ambiente inválido em '{operacao}': {erro:?}"))?;
    }
    if configuracao.limite.expira_imediatamente() {
        return Err(format!("limite de tempo excedido em '{operacao}'"));
    }
    let deadline = match configuracao.limite.duracao() {
        Some(duracao) => Some(
            Instant::now()
                .checked_add(duracao)
                .ok_or_else(|| "limite de tempo fora da faixa monotônica suportada".to_string())?,
        ),
        None => None,
    };

    let mut comando = crate::interpreter::comando_de_processo(configuracao.programa);
    for argumento in configuracao.argumentos {
        comando.arg(argumento);
    }
    comando
        .env("PATH", PATH_PROCESSOS_ESTRUTURADOS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !configuracao.diretorio.is_empty() {
        comando.current_dir(configuracao.diretorio);
    }
    crate::ambiente_processo::aplicar_overlay(
        &mut comando,
        configuracao
            .ambiente
            .iter()
            .map(|(chave, valor)| (chave.as_str(), valor.as_str())),
    )
    .map_err(|erro| format!("ambiente inválido em '{operacao}': {erro:?}"))?;

    let filho = comando.spawn().map_err(|erro| {
        format!(
            "falha ao criar processo '{}' em '{operacao}': {erro}",
            configuracao.programa
        )
    })?;
    let mut filho = FilhoReapavel::novo(filho);
    if let Some(destino) = pid_observado.as_mut() {
        **destino = filho.id();
    }

    if falhar_setup_depois_spawn {
        return falhar_depois_spawn(
            "falha de setup injetada depois do spawn".to_string(),
            &mut filho,
        );
    }

    let stdin = filho
        .take_stdin()
        .ok_or_else(|| "stdin configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_depois_spawn(causa, &mut filho))?;
    let stdout = filho
        .take_stdout()
        .ok_or_else(|| "stdout configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_depois_spawn(causa, &mut filho))?;
    let stderr = filho
        .take_stderr()
        .ok_or_else(|| "stderr configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_depois_spawn(causa, &mut filho))?;

    if let Err(erro) = configurar_nao_bloqueante(&stdin)
        .and_then(|_| configurar_nao_bloqueante(&stdout))
        .and_then(|_| configurar_nao_bloqueante(&stderr))
    {
        return falhar_depois_spawn(
            format!("falha ao configurar pipes não-bloqueantes: {erro}"),
            &mut filho,
        );
    }

    let entrada = configuracao.entrada.as_bytes();
    let mut entrada_enviada = 0usize;
    let mut stdin = if entrada.is_empty() {
        // Fechar também no caso vazio é o EOF observável do contrato.
        drop(stdin);
        None
    } else {
        Some(stdin)
    };
    let mut stdout = Some(stdout);
    let mut stderr = Some(stderr);
    let mut bytes_stdout = Vec::new();
    let mut bytes_stderr = Vec::new();
    loop {
        if deadline.is_some_and(|fim| Instant::now() >= fim) {
            return falhar_depois_spawn(
                format!("limite de tempo excedido em '{operacao}'"),
                &mut filho,
            );
        }

        if let Err(erro) = filho.atualizar_status() {
            return falhar_depois_spawn(
                format!("falha ao observar término do processo estruturado: {erro}"),
                &mut filho,
            );
        }
        if filho.status().is_some() && stdin.is_none() && stdout.is_none() && stderr.is_none() {
            break;
        }

        let mut descritores = Vec::with_capacity(3);
        if let Some(pipe) = stdin.as_ref() {
            descritores.push(DescritorPoll::novo(
                fd(pipe),
                POLL_OUT | POLL_ERR | POLL_HUP,
                CanalPoll::Stdin,
            ));
        }
        if let Some(pipe) = stdout.as_ref() {
            descritores.push(DescritorPoll::novo(
                fd(pipe),
                POLL_IN | POLL_ERR | POLL_HUP,
                CanalPoll::Stdout,
            ));
        }
        if let Some(pipe) = stderr.as_ref() {
            descritores.push(DescritorPoll::novo(
                fd(pipe),
                POLL_IN | POLL_ERR | POLL_HUP,
                CanalPoll::Stderr,
            ));
        }

        let timeout = timeout_poll(deadline);
        match poll_descritores(&mut descritores, timeout) {
            Ok(ResultadoPoll::Eventos) => {}
            Ok(ResultadoPoll::Interrompido) => continue,
            Err(erro) => {
                return falhar_depois_spawn(
                    format!("falha em poll dos pipes do processo estruturado: {erro}"),
                    &mut filho,
                );
            }
        }

        let mut fechar_stdin = false;
        let mut fechar_stdout = false;
        let mut fechar_stderr = false;
        for descritor in &descritores {
            if descritor.revents & POLL_INVALID != 0 {
                return falhar_depois_spawn(
                    "poll encontrou descritor de pipe inválido".to_string(),
                    &mut filho,
                );
            }
            match descritor.canal {
                CanalPoll::Stdin if descritor.revents & (POLL_OUT | POLL_ERR | POLL_HUP) != 0 => {
                    let Some(pipe) = stdin.as_mut() else {
                        continue;
                    };
                    match escrever_disponivel(pipe, entrada, &mut entrada_enviada) {
                        Ok(true) => fechar_stdin = true,
                        Ok(false) => {}
                        Err(erro) => {
                            return falhar_depois_spawn(
                                format!("falha ao enviar stdin integralmente: {erro}"),
                                &mut filho,
                            )
                        }
                    }
                }
                CanalPoll::Stdout if descritor.revents & (POLL_IN | POLL_ERR | POLL_HUP) != 0 => {
                    let Some(pipe) = stdout.as_mut() else {
                        continue;
                    };
                    match drenar_disponivel(pipe, &mut bytes_stdout) {
                        Ok(eof) => fechar_stdout = eof,
                        Err(erro) => {
                            return falhar_depois_spawn(
                                format!("falha ao capturar stdout: {erro}"),
                                &mut filho,
                            )
                        }
                    }
                }
                CanalPoll::Stderr if descritor.revents & (POLL_IN | POLL_ERR | POLL_HUP) != 0 => {
                    let Some(pipe) = stderr.as_mut() else {
                        continue;
                    };
                    match drenar_disponivel(pipe, &mut bytes_stderr) {
                        Ok(eof) => fechar_stderr = eof,
                        Err(erro) => {
                            return falhar_depois_spawn(
                                format!("falha ao capturar stderr: {erro}"),
                                &mut filho,
                            )
                        }
                    }
                }
                _ => {}
            }
        }
        if fechar_stdin {
            stdin = None;
        }
        if fechar_stdout {
            stdout = None;
        }
        if fechar_stderr {
            stderr = None;
        }
    }

    let status = filho
        .status()
        .expect("laço só conclui depois de reapear o filho");
    let codigo = status.code().ok_or_else(|| {
        "processo estruturado terminou sem código normal; nenhum código mágico foi fabricado"
            .to_string()
    })? as u64;
    let stdout = String::from_utf8(bytes_stdout)
        .map_err(|_| "stdout do processo estruturado não é UTF-8 válido".to_string())?;
    let stderr = String::from_utf8(bytes_stderr)
        .map_err(|_| "stderr do processo estruturado não é UTF-8 válido".to_string())?;
    Ok(SaidaProcesso::nova(codigo, stdout, stderr))
}

/// Poll recebe no máximo este intervalo para também reapear prontamente o filho
/// direto quando um descendente mantém os write-ends herdados.
const TICK_REAP: Duration = Duration::from_millis(25);

/// Limite de trabalho por canal entre duas observações do deadline absoluto.
///
/// Bytes e syscalls são limitados juntos: o primeiro impede uma única passagem
/// volumosa; o segundo impede progresso em fragmentos mínimos de monopolizar a
/// malha. Como stdin, stdout e stderr recebem no máximo um quantum por ciclo,
/// múltiplos canais prontos progridem antes da próxima chamada a `poll`.
const QUANTUM_IO_BYTES: usize = 64 * 1024;
const QUANTUM_IO_SYSCALLS: usize = 4;
const BLOCO_IO_BYTES: usize = 16 * 1024;

fn timeout_poll(deadline: Option<Instant>) -> i32 {
    let espera = match deadline {
        Some(fim) => fim.saturating_duration_since(Instant::now()).min(TICK_REAP),
        None => TICK_REAP,
    };
    let millis = espera.as_millis().max(1);
    millis.min(i32::MAX as u128) as i32
}

fn escrever_disponivel<W: Write>(
    pipe: &mut W,
    entrada: &[u8],
    enviados: &mut usize,
) -> io::Result<bool> {
    let inicio = *enviados;
    let mut syscalls = 0usize;
    while *enviados < entrada.len()
        && *enviados - inicio < QUANTUM_IO_BYTES
        && syscalls < QUANTUM_IO_SYSCALLS
    {
        let restantes_quantum = QUANTUM_IO_BYTES - (*enviados - inicio);
        let fim = (*enviados + restantes_quantum.min(BLOCO_IO_BYTES)).min(entrada.len());
        match pipe.write(&entrada[*enviados..fim]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "stdin não avançou",
                ))
            }
            Ok(n) => {
                *enviados += n;
                syscalls += 1;
            }
            Err(erro) if erro.kind() == io::ErrorKind::Interrupted => return Ok(false),
            Err(erro) if erro.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(erro) => return Err(erro),
        }
    }
    Ok(*enviados == entrada.len())
}

fn drenar_disponivel<R: Read>(pipe: &mut R, destino: &mut Vec<u8>) -> io::Result<bool> {
    let mut bloco = [0u8; BLOCO_IO_BYTES];
    let mut bytes = 0usize;
    let mut syscalls = 0usize;
    while bytes < QUANTUM_IO_BYTES && syscalls < QUANTUM_IO_SYSCALLS {
        let limite = (QUANTUM_IO_BYTES - bytes).min(bloco.len());
        match pipe.read(&mut bloco[..limite]) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                destino.extend_from_slice(&bloco[..n]);
                bytes += n;
                syscalls += 1;
            }
            Err(erro) if erro.kind() == io::ErrorKind::Interrupted => return Ok(false),
            Err(erro) if erro.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(erro) => return Err(erro),
        }
    }
    Ok(false)
}

fn falhar_depois_spawn<T>(causa: String, filho: &mut FilhoReapavel) -> Result<T, String> {
    compor_falha_com_cleanup(causa, filho.encerrar_e_reapear())
}

fn compor_falha_com_cleanup<T>(
    causa: String,
    limpeza: Result<(), FalhasCleanupProcesso>,
) -> Result<T, String> {
    match limpeza {
        Ok(()) => Err(causa),
        Err(limpeza) => Err(format!("{causa}; cleanup do filho direto: {limpeza}")),
    }
}

trait OperacoesCleanupProcesso {
    type Status;

    fn observar_status(&mut self) -> io::Result<Option<Self::Status>>;
    fn encerrar(&mut self) -> io::Result<()>;
    fn esperar(&mut self) -> io::Result<Self::Status>;
}

impl OperacoesCleanupProcesso for Child {
    type Status = ExitStatus;

    fn observar_status(&mut self) -> io::Result<Option<Self::Status>> {
        self.try_wait()
    }

    fn encerrar(&mut self) -> io::Result<()> {
        self.kill()
    }

    fn esperar(&mut self) -> io::Result<Self::Status> {
        self.wait()
    }
}

#[derive(Debug, Default)]
struct FalhasCleanupProcesso {
    observacao: Option<io::Error>,
    encerramento: Option<io::Error>,
    espera: Option<io::Error>,
}

impl FalhasCleanupProcesso {
    fn vazia(&self) -> bool {
        self.observacao.is_none() && self.encerramento.is_none() && self.espera.is_none()
    }
}

impl std::fmt::Display for FalhasCleanupProcesso {
    fn fmt(&self, saida: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut separador = "";
        for (etapa, erro) in [
            ("observação", self.observacao.as_ref()),
            ("kill", self.encerramento.as_ref()),
            ("wait", self.espera.as_ref()),
        ] {
            if let Some(erro) = erro {
                write!(saida, "{separador}{etapa}: {erro}")?;
                separador = "; ";
            }
        }
        Ok(())
    }
}

fn encerrar_e_reapear_com<O: OperacoesCleanupProcesso>(
    filho: &mut O,
    status: &mut Option<O::Status>,
) -> Result<(), FalhasCleanupProcesso> {
    if status.is_some() {
        return Ok(());
    }

    let mut falhas = FalhasCleanupProcesso::default();
    match filho.observar_status() {
        Ok(Some(observado)) => {
            *status = Some(observado);
            return Ok(());
        }
        Ok(None) => {}
        Err(erro) => falhas.observacao = Some(erro),
    }

    match filho.encerrar() {
        Ok(()) => {}
        Err(erro) if erro.kind() == io::ErrorKind::InvalidInput => {}
        Err(erro) => falhas.encerramento = Some(erro),
    }

    match filho.esperar() {
        Ok(observado) => *status = Some(observado),
        Err(erro) => falhas.espera = Some(erro),
    }

    if falhas.vazia() {
        Ok(())
    } else {
        Err(falhas)
    }
}

struct FilhoReapavel {
    filho: Child,
    status: Option<ExitStatus>,
}

impl FilhoReapavel {
    fn novo(filho: Child) -> Self {
        Self {
            filho,
            status: None,
        }
    }

    fn id(&self) -> u32 {
        self.filho.id()
    }

    fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.filho.stdin.take()
    }

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.filho.stdout.take()
    }

    fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.filho.stderr.take()
    }

    fn atualizar_status(&mut self) -> io::Result<()> {
        if self.status.is_none() {
            self.status = self.filho.try_wait()?;
        }
        Ok(())
    }

    fn status(&self) -> Option<ExitStatus> {
        self.status
    }

    fn encerrar_e_reapear(&mut self) -> Result<(), FalhasCleanupProcesso> {
        encerrar_e_reapear_com(&mut self.filho, &mut self.status)
    }
}

impl Drop for FilhoReapavel {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.filho.kill();
            let _ = self.filho.wait();
        }
    }
}

#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};

#[cfg(unix)]
fn fd<T: AsRawFd>(pipe: &T) -> RawFd {
    pipe.as_raw_fd()
}

#[cfg(not(unix))]
fn fd<T>(_pipe: &T) -> i32 {
    -1
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[derive(Clone, Copy)]
enum CanalPoll {
    Stdin,
    Stdout,
    Stderr,
}

struct DescritorPoll {
    raw: PollFd,
    canal: CanalPoll,
    revents: i16,
}

impl DescritorPoll {
    fn novo(fd: i32, events: i16, canal: CanalPoll) -> Self {
        Self {
            raw: PollFd {
                fd,
                events,
                revents: 0,
            },
            canal,
            revents: 0,
        }
    }
}

const POLL_IN: i16 = 0x0001;
const POLL_OUT: i16 = 0x0004;
const POLL_ERR: i16 = 0x0008;
const POLL_HUP: i16 = 0x0010;
const POLL_INVALID: i16 = 0x0020;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const O_NONBLOCK: i32 = 0x0004;

#[cfg(unix)]
extern "C" {
    fn poll(fds: *mut PollFd, quantidade: usize, timeout_ms: i32) -> i32;
    fn fcntl(fd: i32, comando: i32, ...) -> i32;
}

#[cfg(unix)]
fn configurar_nao_bloqueante<T: AsRawFd>(pipe: &T) -> io::Result<()> {
    let descritor = pipe.as_raw_fd();
    // SAFETY: fcntl recebe um fd vivo possuído pelo Child pipe; F_GETFL não
    // recebe argumento variádico e F_SETFL recebe somente os flags recuperados.
    let flags = unsafe { fcntl(descritor, F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { fcntl(descritor, F_SETFL, flags | O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn configurar_nao_bloqueante<T>(_pipe: &T) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "poll não-bloqueante requer plataforma Unix",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultadoPoll {
    Eventos,
    Interrompido,
}

#[cfg(unix)]
fn poll_descritores(descritores: &mut [DescritorPoll], timeout: i32) -> io::Result<ResultadoPoll> {
    poll_descritores_com(descritores, timeout, |raws, timeout| {
        // SAFETY: raws é um slice contíguo de PollFd com quantidade exata; para
        // slice vazio, o ponteiro não é desreferenciado porque quantidade é 0.
        let retorno = unsafe { poll(raws.as_mut_ptr(), raws.len(), timeout) };
        if retorno >= 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    })
}

#[cfg(unix)]
fn poll_descritores_com(
    descritores: &mut [DescritorPoll],
    timeout: i32,
    mut aguardar: impl FnMut(&mut [PollFd], i32) -> io::Result<()>,
) -> io::Result<ResultadoPoll> {
    let mut raws: Vec<PollFd> = descritores
        .iter()
        .map(|descritor| PollFd {
            fd: descritor.raw.fd,
            events: descritor.raw.events,
            revents: 0,
        })
        .collect();
    match aguardar(&mut raws, timeout) {
        Ok(()) => {
            for (destino, origem) in descritores.iter_mut().zip(raws.iter()) {
                destino.revents = origem.revents;
            }
            Ok(ResultadoPoll::Eventos)
        }
        Err(erro) if erro.kind() == io::ErrorKind::Interrupted => Ok(ResultadoPoll::Interrompido),
        Err(erro) => Err(erro),
    }
}

#[cfg(not(unix))]
fn poll_descritores(
    _descritores: &mut [DescritorPoll],
    _timeout: i32,
) -> io::Result<ResultadoPoll> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "poll requer plataforma Unix",
    ))
}

// @pinker-nav:end processos.estruturado.hospedado

// @pinker-nav:start evidencia.processos.estruturado-recursos
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita diretamente a disciplina de recursos da execução hospedada: uma falha de setup falsificada depois do spawn aciona kill e wait do filho direto antes de retornar, sem criar snapshot ou thread auxiliar.
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct StatusCleanupControlado;

    struct OperacoesCleanupControladas {
        falhar_observacao: bool,
        falhar_encerramento: bool,
        falhar_espera: bool,
        kill_tentado: bool,
        wait_tentado: bool,
    }

    impl OperacoesCleanupProcesso for OperacoesCleanupControladas {
        type Status = StatusCleanupControlado;

        fn observar_status(&mut self) -> io::Result<Option<Self::Status>> {
            if self.falhar_observacao {
                Err(io::Error::other("try_wait controlado"))
            } else {
                Ok(None)
            }
        }

        fn encerrar(&mut self) -> io::Result<()> {
            self.kill_tentado = true;
            if self.falhar_encerramento {
                Err(io::Error::other("kill controlado"))
            } else {
                Ok(())
            }
        }

        fn esperar(&mut self) -> io::Result<Self::Status> {
            self.wait_tentado = true;
            if self.falhar_espera {
                Err(io::Error::other("wait controlado"))
            } else {
                Ok(StatusCleanupControlado)
            }
        }
    }

    #[test]
    fn erro_de_try_wait_nao_impede_kill_wait_e_reap_hospedado() {
        let mut operacoes = OperacoesCleanupControladas {
            falhar_observacao: true,
            falhar_encerramento: false,
            falhar_espera: false,
            kill_tentado: false,
            wait_tentado: false,
        };
        let mut status = None;

        let falhas = encerrar_e_reapear_com(&mut operacoes, &mut status)
            .expect_err("erro de observação deve permanecer explícito");

        assert!(falhas.observacao.is_some(), "TRY_WAIT_ERROR_OBSERVED");
        assert!(operacoes.kill_tentado, "KILL_ATTEMPTED");
        assert!(operacoes.wait_tentado, "WAIT_ATTEMPTED");
        assert!(status.is_some(), "REAP_PATH_REACHED");
    }

    #[test]
    fn causa_primaria_e_falhas_secundarias_permanecem_no_erro_hospedado() {
        let mut operacoes = OperacoesCleanupControladas {
            falhar_observacao: true,
            falhar_encerramento: true,
            falhar_espera: true,
            kill_tentado: false,
            wait_tentado: false,
        };
        let mut status = None;
        let limpeza = encerrar_e_reapear_com(&mut operacoes, &mut status);
        let erro = compor_falha_com_cleanup::<()>("causa primária".to_string(), limpeza)
            .expect_err("falha primária com cleanup falho não pode virar sucesso");

        assert!(operacoes.kill_tentado, "KILL_ATTEMPTED");
        assert!(operacoes.wait_tentado, "WAIT_ATTEMPTED");
        assert!(erro.contains("causa primária"), "{erro}");
        assert!(erro.contains("try_wait controlado"), "{erro}");
        assert!(erro.contains("kill controlado"), "{erro}");
        assert!(erro.contains("wait controlado"), "{erro}");
    }

    struct LeitorControlado {
        sucessos_restantes: usize,
        chamadas: usize,
        byte: u8,
    }

    impl Read for LeitorControlado {
        fn read(&mut self, destino: &mut [u8]) -> io::Result<usize> {
            self.chamadas += 1;
            if self.sucessos_restantes == 0 {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            self.sucessos_restantes -= 1;
            destino.fill(self.byte);
            Ok(destino.len())
        }
    }

    #[test]
    fn quantum_de_drenagem_devolve_autoridade_temporal_antes_de_would_block() {
        let mut leitor = LeitorControlado {
            sucessos_restantes: QUANTUM_IO_SYSCALLS + 1,
            chamadas: 0,
            byte: b'Q',
        };
        let mut destino = Vec::new();

        let eof = drenar_disponivel(&mut leitor, &mut destino).expect("drenagem controlada");

        assert!(!eof, "quantum não significa EOF");
        assert_eq!(
            leitor.chamadas, QUANTUM_IO_SYSCALLS,
            "TARGET_SURFACE_REACHED"
        );
        assert_eq!(destino.len(), QUANTUM_IO_BYTES, "MUTANT_DETECTED");
    }

    #[test]
    fn canais_prontos_recebem_um_quantum_cada_sem_monopolio() {
        let mut stdout = LeitorControlado {
            sucessos_restantes: usize::MAX,
            chamadas: 0,
            byte: b'O',
        };
        let mut stderr = LeitorControlado {
            sucessos_restantes: usize::MAX,
            chamadas: 0,
            byte: b'E',
        };
        let mut bytes_stdout = Vec::new();
        let mut bytes_stderr = Vec::new();

        assert!(!drenar_disponivel(&mut stdout, &mut bytes_stdout).unwrap());
        assert!(!drenar_disponivel(&mut stderr, &mut bytes_stderr).unwrap());

        assert_eq!(bytes_stdout.len(), QUANTUM_IO_BYTES);
        assert_eq!(bytes_stderr.len(), QUANTUM_IO_BYTES);
        assert_eq!(stdout.chamadas, QUANTUM_IO_SYSCALLS);
        assert_eq!(stderr.chamadas, QUANTUM_IO_SYSCALLS);
    }

    #[test]
    fn stdin_grande_devolve_autoridade_temporal_e_depois_conclui_integralmente() {
        let entrada = vec![b'I'; QUANTUM_IO_BYTES * 2 + 7];
        let mut destino = Vec::new();
        let mut enviados = 0usize;

        assert!(!escrever_disponivel(&mut destino, &entrada, &mut enviados).unwrap());
        assert_eq!(enviados, QUANTUM_IO_BYTES);
        assert!(!escrever_disponivel(&mut destino, &entrada, &mut enviados).unwrap());
        assert_eq!(enviados, QUANTUM_IO_BYTES * 2);
        assert!(escrever_disponivel(&mut destino, &entrada, &mut enviados).unwrap());
        assert_eq!(destino, entrada, "quantum não trunca stdin");
    }

    #[cfg(unix)]
    #[test]
    fn eintr_devolve_controle_sem_repetir_timeout_relativo() {
        let mut chamadas = 0;
        let mut injecao_aplicada = false;
        let resultado = poll_descritores_com(&mut [], 317, |_, timeout| {
            injecao_aplicada = true;
            chamadas += 1;
            assert_eq!(timeout, 317, "TARGET_REACHED: timeout chegou à espera");
            Err(io::Error::from(io::ErrorKind::Interrupted))
        })
        .expect("Interrupted é controle do laço, não falha operacional");

        assert!(injecao_aplicada, "INJECTION_APPLIED");
        assert_eq!(resultado, ResultadoPoll::Interrompido, "TARGET_REACHED");
        assert_eq!(
            chamadas, 1,
            "MUTANT_DETECTED: retry interno reutilizaria o timeout relativo"
        );

        let revertido = poll_descritores_com(&mut [], 1, |_, _| Ok(()))
            .expect("seam controlada não altera a espera seguinte");
        assert_eq!(revertido, ResultadoPoll::Eventos, "REVERTED");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn falha_de_setup_depois_do_spawn_mata_e_reapeia_antes_de_retornar() {
        let argumentos = vec!["5".to_string()];
        let ambiente = HashMap::new();
        let configuracao = ConfiguracaoProcessoEstruturado {
            programa: "/bin/sleep",
            argumentos: &argumentos,
            entrada: "",
            diretorio: "",
            ambiente: &ambiente,
            limite: LimiteTempo::SemLimite,
        };
        let mut pid = 0;
        let erro = executar_com_controle(&configuracao, true, Some(&mut pid))
            .expect_err("setup falsificado deveria falhar");
        assert!(erro.contains("setup injetada"), "{erro}");
        assert!(pid > 1);
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "filho {pid} sobreviveu ou virou zumbi depois do retorno"
        );
    }

    #[test]
    fn configuracao_invalida_falha_antes_de_spawn() {
        for (programa, chave, valor) in [
            ("", "VALIDA", "valor"),
            ("/bin/true", "NOME=INVALIDO", "valor"),
            ("/bin/true", "NOME\0INVALIDO", "valor"),
            ("/bin/true", "VALIDA", "valor\0invalido"),
        ] {
            let argumentos = Vec::new();
            let ambiente = HashMap::from([(chave.to_string(), valor.to_string())]);
            let configuracao = ConfiguracaoProcessoEstruturado {
                programa,
                argumentos: &argumentos,
                entrada: "",
                diretorio: "",
                ambiente: &ambiente,
                limite: LimiteTempo::SemLimite,
            };
            let mut pid = 0;
            executar_com_controle(&configuracao, false, Some(&mut pid))
                .expect_err("configuração inválida deveria ser recuperável");
            assert_eq!(pid, 0, "configuração inválida chegou ao spawn");
        }
    }

    #[test]
    fn ate_zero_falha_antes_de_spawn_no_interpretador() {
        let argumentos = Vec::new();
        let ambiente = HashMap::new();
        let configuracao = ConfiguracaoProcessoEstruturado {
            programa: "/bin/true",
            argumentos: &argumentos,
            entrada: "",
            diretorio: "",
            ambiente: &ambiente,
            limite: LimiteTempo::Ate(0),
        };
        let mut pid = 0;

        let erro = executar_com_controle(&configuracao, false, Some(&mut pid))
            .expect_err("Ate(0) deve expirar antes do spawn");

        assert!(erro.contains("limite de tempo excedido"), "{erro}");
        assert_eq!(pid, 0, "INTERPRETER_SPAWN_COUNT precisa permanecer zero");
    }
}
// @pinker-nav:end evidencia.processos.estruturado-recursos
