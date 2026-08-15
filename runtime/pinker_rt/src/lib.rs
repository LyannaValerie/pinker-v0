//! Runtime nativo da Pinker (`pinker_rt`) — Eixo B do Bloco 20, fase B1.
//!
//! Esta staticlib é linkada aos executáveis gerados por `pink build --nativo`.
//! Toda a superfície pública usa ABI C estável (`extern "C"` + `#[no_mangle]`),
//! para que o backend `.s` chame os serviços por símbolo simples.
//!
//! Nesta fase o runtime entrega:
//! - inicialização (`pinker_rt_iniciar`), capturando `argc`/`argv` do `main`
//!   para uso futuro das intrínsecas de ambiente (fase B10);
//! - alocador real (`pinker_alocar`/`pinker_liberar`) com cabeçalho de
//!   tamanho, alinhamento de 16 bytes e liberação segura de ponteiro nulo.
//!
//! As fases B4–B10 acrescentam aqui strings dinâmicas, coleções, leques e
//! intrínsecas de sistema. O runtime é substituível no futuro por uma
//! implementação em Pinker (convergência com a direção self-hosting).

use pinker_memory_contract::{
    release_public_live_bytes, reserve_public_allocation, PublicAllocationVerdict,
    PublicMemoryBudget, PublicMemoryLimits, PUBLIC_MEMORY_LIMITS,
};
use std::alloc::{alloc, dealloc, Layout};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Once;

// @pinker-nav:start runtime.inicializacao.bootstrap
// @pinker-nav:domain inicializacao
// @pinker-nav:layer runtime
// @pinker-nav:summary Define constantes de layout do alocador (ALINHAMENTO, CABECALHO) e o estado global (ARGC/ARGV em atômicos) capturado por pinker_rt_iniciar; expõe leitura de argc/argv e a versão da ABI (pinker_rt_versao) — as constantes de alocação ficam fisicamente no preâmbulo, junto ao estado global de inicialização.
/// Alinhamento garantido dos blocos devolvidos por `pinker_alocar`.
const ALINHAMENTO: usize = 16;

/// Bytes reservados antes do ponteiro devolvido; guardam o tamanho pedido e
/// preservam o alinhamento de 16 do bloco visível.
const CABECALHO: usize = 16;

static ARGC: AtomicI64 = AtomicI64::new(0);
static ARGV: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "linux")]
fn desabilitar_core_dump() {
    #[repr(C)]
    struct RLimit {
        current: u64,
        maximum: u64,
    }
    extern "C" {
        fn getrlimit(resource: i32, limit: *mut RLimit) -> i32;
        fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
    }
    const RLIMIT_CORE: i32 = 4;
    let mut limit = RLimit {
        current: 0,
        maximum: 0,
    };
    if unsafe { getrlimit(RLIMIT_CORE, &mut limit) } != 0 {
        eprintln!(
            "Erro Runtime: E-RUNTIME-HOST-CORE-LIMIT: falha ao consultar limite de core dump: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(1);
    }
    limit.current = 0;
    if unsafe { setrlimit(RLIMIT_CORE, &limit) } != 0 {
        eprintln!(
            "Erro Runtime: E-RUNTIME-HOST-CORE-LIMIT: falha ao desabilitar core dump: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn desabilitar_core_dump() {}

/// Inicialização do runtime; chamada pelo prólogo do `main` gerado em modo
/// nativo, com `argc` em `%rdi` e `argv` em `%rsi` (ABI C do `main`).
///
/// O `main` gerado não passa pelo `lang_start` da std, então nenhuma
/// inicialização de runtime Rust acontece antes daqui. A disposição de sinais
/// do processo é estabelecida neste ponto — antes de qualquer `falar`, escrita
/// de stdin de filho ou outra operação capaz de receber `SIGPIPE` — para que o
/// comportamento observável não dependa da ordem de execução do programa.
///
/// # Safety
/// `argv` deve ser o vetor de argumentos recebido pelo `main` C; o runtime
/// apenas o armazena para consulta posterior.
#[no_mangle]
pub unsafe extern "C" fn pinker_rt_iniciar(argc: i64, argv: *const *const u8) {
    desabilitar_core_dump();
    preparar_disposicao_sinais();
    ARGC.store(argc, Ordering::SeqCst);
    ARGV.store(argv as usize, Ordering::SeqCst);
}

/// Quantidade de argumentos capturada na inicialização (0 antes de iniciar).
#[no_mangle]
pub extern "C" fn pinker_rt_argc() -> i64 {
    ARGC.load(Ordering::SeqCst)
}

/// Ponteiro de `argv` capturado na inicialização (nulo antes de iniciar).
#[no_mangle]
pub extern "C" fn pinker_rt_argv() -> *const *const u8 {
    ARGV.load(Ordering::SeqCst) as *const *const u8
}

/// Versão da ABI do runtime; incrementada quando a superfície C muda de forma
/// incompatível. Serve também como símbolo de fumaça para verificação de link.
#[no_mangle]
pub extern "C" fn pinker_rt_versao() -> u64 {
    1
}
// @pinker-nav:end runtime.inicializacao.bootstrap

// @pinker-nav:start runtime.memoria.alocador
// @pinker-nav:domain memoria
// @pinker-nav:layer runtime
// @pinker-nav:summary Alocador manual e regiões públicas: pinker_alocar mantém o alocador interno; a superfície pública decide cotas numa unidade pura compartilhada, cria um mmap anônimo proporcional e lazy por alocação, registra identidade/base/tamanho/vida e mantém mapeamentos liberados inacessíveis até o fim do processo. Acesso, liberação e derivação validam proveniência, domínio, alinhamento, limites, use-after-free, double free e escapes.
fn layout_para(tamanho_total: usize) -> Option<Layout> {
    Layout::from_size_align(tamanho_total, ALINHAMENTO).ok()
}

/// Aloca `tamanho` bytes e devolve ponteiro alinhado a 16 bytes.
/// Pedido de 0 bytes devolve um bloco válido (tratado como 1 byte).
/// Devolve nulo apenas se o sistema recusar a alocação.
#[no_mangle]
pub extern "C" fn pinker_alocar(tamanho: u64) -> *mut u8 {
    let pedido = (tamanho as usize).max(1);
    let total = match pedido.checked_add(CABECALHO) {
        Some(total) => total,
        None => return std::ptr::null_mut(),
    };
    let Some(layout) = layout_para(total) else {
        return std::ptr::null_mut();
    };
    unsafe {
        let base = alloc(layout);
        if base.is_null() {
            return std::ptr::null_mut();
        }
        (base as *mut u64).write(total as u64);
        base.add(CABECALHO)
    }
}

/// Libera um bloco devolvido por `pinker_alocar`. Ponteiro nulo é aceito e
/// tratado como operação nula, no estilo de `free`.
///
/// # Safety
/// `ptr` deve ser nulo ou um ponteiro devolvido por `pinker_alocar` que ainda
/// não foi liberado.
#[no_mangle]
pub unsafe extern "C" fn pinker_liberar(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let base = ptr.sub(CABECALHO);
    let total = (base as *const u64).read() as usize;
    if let Some(layout) = layout_para(total) {
        dealloc(base, layout);
    }
}

/// Tamanho do descritor público de callable `{code_ptr, env_ptr}`.
const CALLABLE_DESCRIPTOR_BYTES: u64 = 16;

fn callable_allocation_bytes(capture_count: u64) -> Option<u64> {
    capture_count
        .checked_mul(8)
        .and_then(|environment_bytes| CALLABLE_DESCRIPTOR_BYTES.checked_add(environment_bytes))
}

/// Materializa atomicamente o storage possuído por uma closure dinâmica.
///
/// O descritor ocupa as duas primeiras palavras. Quando há capturas, `env_ptr`
/// aponta para o storage trailing da mesma alocação; quando não há, permanece
/// nulo. Assim não existe uma segunda alocação capaz de falhar depois de o
/// ambiente já ter sido criado e ficado sem owner alcançável.
#[no_mangle]
pub extern "C" fn pinker_callable_alocar(capture_count: u64) -> *mut u8 {
    let total = callable_allocation_bytes(capture_count).unwrap_or_else(|| {
        erro_fatal("E-RUNTIME-CALLABLE-ALLOCATION: overflow no layout da closure")
    });
    if usize::try_from(total).is_err() {
        erro_fatal("E-RUNTIME-CALLABLE-ALLOCATION: layout da closure excede a plataforma");
    }

    let descriptor = pinker_alocar(total);
    if descriptor.is_null() {
        erro_fatal("E-RUNTIME-CALLABLE-ALLOCATION: runtime não pôde alocar a closure");
    }

    unsafe {
        (descriptor as *mut u64).write(0);
        let environment = if capture_count == 0 {
            0
        } else {
            descriptor.add(CALLABLE_DESCRIPTOR_BYTES as usize) as u64
        };
        (descriptor.add(8) as *mut u64).write(environment);
    }
    descriptor
}

#[derive(Clone, Copy)]
struct AlocacaoPublica {
    identidade: u64,
    base: usize,
    tamanho: usize,
    reservado: usize,
    viva: bool,
}

struct MemoriaPublica {
    budget: PublicMemoryBudget,
    alocacoes: Vec<AlocacaoPublica>,
}

#[cfg(test)]
const PAGINA_PUBLICA: usize = pinker_memory_contract::PUBLIC_PAGE_BYTES;
#[cfg(test)]
const MAX_IDENTIDADES_PUBLICAS: usize = pinker_memory_contract::MAX_PUBLIC_IDENTITIES as usize;
#[cfg(test)]
const MAX_METADATA_PUBLICA_BYTES: usize =
    pinker_memory_contract::MAX_PUBLIC_METADATA_BYTES as usize;

#[cfg(test)]
static LIMITE_IDENTIDADES_PUBLICAS_TESTE: AtomicUsize =
    AtomicUsize::new(pinker_memory_contract::MAX_PUBLIC_IDENTITIES as usize);

#[cfg(not(test))]
fn limites_memoria_publica() -> PublicMemoryLimits {
    PUBLIC_MEMORY_LIMITS
}

#[cfg(test)]
fn limites_memoria_publica() -> PublicMemoryLimits {
    let mut limits = PUBLIC_MEMORY_LIMITS;
    limits.max_identities = LIMITE_IDENTIDADES_PUBLICAS_TESTE.load(Ordering::SeqCst) as u64;
    limits
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReservaIdentidade {
    Concedida { identidade: u64, proxima: u64 },
    Esgotada,
    Exaurida,
}

#[cfg(test)]
fn reservar_identidade_publica(
    identidades_registradas: usize,
    proxima_identidade: u64,
    limite: usize,
) -> ReservaIdentidade {
    if identidades_registradas >= limite {
        return ReservaIdentidade::Esgotada;
    }
    match proxima_identidade.checked_add(1) {
        Some(proxima) => ReservaIdentidade::Concedida {
            identidade: proxima_identidade,
            proxima,
        },
        None => ReservaIdentidade::Exaurida,
    }
}

#[cfg(target_os = "linux")]
fn mapear_regiao_publica(tamanho: usize) -> Result<usize, ()> {
    use std::ffi::c_void;

    extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut c_void;
    }
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_NORESERVE: i32 = 0x4000;
    let base = unsafe {
        mmap(
            std::ptr::null_mut(),
            tamanho,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE,
            -1,
            0,
        )
    };
    (base as usize != usize::MAX)
        .then_some(base as usize)
        .ok_or(())
}

#[cfg(not(target_os = "linux"))]
fn mapear_regiao_publica(_tamanho: usize) -> Result<usize, ()> {
    Err(())
}

fn memoria_publica() -> &'static Mutex<MemoriaPublica> {
    static MEMORIA: OnceLock<Mutex<MemoriaPublica>> = OnceLock::new();
    MEMORIA.get_or_init(|| {
        Mutex::new(MemoriaPublica {
            budget: PublicMemoryBudget::default(),
            alocacoes: Vec::new(),
        })
    })
}

#[cfg(target_os = "linux")]
fn descomprometer_paginas_publicas(base: usize, tamanho: usize) -> Result<(), ()> {
    use std::ffi::c_void;

    extern "C" {
        fn madvise(address: *mut c_void, length: usize, advice: i32) -> i32;
        fn mprotect(address: *mut c_void, length: usize, protection: i32) -> i32;
    }
    const MADV_DONTNEED: i32 = 4;
    const PROT_NONE: i32 = 0;
    if unsafe { madvise(base as *mut c_void, tamanho, MADV_DONTNEED) } != 0 {
        return Err(());
    }
    (unsafe { mprotect(base as *mut c_void, tamanho, PROT_NONE) } == 0)
        .then_some(())
        .ok_or(())
}

#[cfg(not(target_os = "linux"))]
fn descomprometer_paginas_publicas(_base: usize, _tamanho: usize) -> Result<(), ()> {
    Err(())
}

fn indice_base_publica_mais_recente(registro: &[AlocacaoPublica], base: usize) -> Option<usize> {
    registro.iter().rposition(|alocacao| alocacao.base == base)
}

fn erro_memoria_publica(mensagem: &str) -> ! {
    eprintln!("Erro Runtime: {mensagem}");
    std::process::exit(1);
}

/// Primeiro endereço **depois** da região.
///
/// `base + tamanho` não é uma classe de erro classificável: é um invariante do
/// único construtor produtivo de `AlocacaoPublica`. `pinker_publico_alocar`
/// deriva `base` de `arena_base.checked_add(proximo_offset)`, mantém
/// `proximo_offset + reservado` abaixo de `MAX_ESPACO_VIRTUAL_PUBLICO_BYTES` e
/// só registra a entrada depois de comprometer as páginas — ou seja, a região
/// inteira está efetivamente mapeada, e um mapeamento não pode terminar depois
/// do fim do espaço de endereços. `tamanho` também é sempre ≥ 1, porque
/// `alocar` recusa zero.
///
/// O `debug_assert!` é onde esse invariante fica escrito de forma executável: em
/// teste e em debug uma entrada corrompida falha alto, e em release
/// `saturating_add` mantém a função pura — ela é a unidade que a matriz de
/// veredictos exercita, e não pode encerrar o processo.
fn fim_da_regiao(alocacao: &AlocacaoPublica) -> usize {
    debug_assert!(
        alocacao.base.checked_add(alocacao.tamanho).is_some(),
        "invariante da arena pública: base + tamanho precisa ser representável"
    );
    alocacao.base.saturating_add(alocacao.tamanho)
}

/// Entrada pública de `alocar`: decide todas as cotas antes do efeito, reserva
/// metadata, cria um mapeamento anônimo novo e só então publica identidade,
/// região e contadores. O kernel garante zero inicial sem toque integral.
fn tentar_alocar_publico_com(
    memoria: &mut MemoriaPublica,
    tamanho: u64,
    limits: PublicMemoryLimits,
    reservar_metadata: impl FnOnce(&mut Vec<AlocacaoPublica>) -> Result<(), ()>,
    mapear: impl FnOnce(usize) -> Result<usize, ()>,
) -> Result<usize, PublicAllocationVerdict> {
    let reservation =
        match reserve_public_allocation(memoria.budget, tamanho, isize::MAX as usize, limits) {
            PublicAllocationVerdict::Allowed(reservation) => reservation,
            verdict => return Err(verdict),
        };
    reservar_metadata(&mut memoria.alocacoes)
        .map_err(|_| PublicAllocationVerdict::MetadataBudgetExceeded)?;
    let base = mapear(reservation.reserved_page_bytes)
        .map_err(|_| PublicAllocationVerdict::MappingFailure)?;
    memoria.alocacoes.push(AlocacaoPublica {
        identidade: reservation.identity,
        base,
        tamanho: reservation.logical_bytes,
        reservado: reservation.reserved_page_bytes,
        viva: true,
    });
    memoria.budget = reservation.next_budget;
    Ok(base)
}

fn reservar_metadata_publica(alocacoes: &mut Vec<AlocacaoPublica>) -> Result<(), ()> {
    alocacoes.try_reserve(1).map_err(|_| ())
}

#[no_mangle]
pub extern "C" fn pinker_publico_alocar(tamanho: u64) -> *mut u8 {
    let mut memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    match tentar_alocar_publico_com(
        &mut memoria,
        tamanho,
        limites_memoria_publica(),
        reservar_metadata_publica,
        mapear_regiao_publica,
    ) {
        Ok(base) => base as *mut u8,
        Err(verdict) => erro_memoria_publica(
            verdict
                .diagnostic()
                .expect("todo veredicto recusado possui diagnóstico"),
        ),
    }
}

/// Fase 246: entrada pública de `liberar`. Somente o ponteiro-base de uma
/// alocação pública viva é aceito; ponteiros internos e double free falham
/// deterministicamente sem tocar no allocator interno.
///
/// # Safety
///
/// `ponteiro` deve ser exatamente o endereço-base retornado por
/// `pinker_publico_alocar`. O registro interno valida essa origem antes de
/// marcar a geração como liberada. As páginas são descartadas e o mapeamento
/// vira `PROT_NONE`, mas permanece reservado até o encerramento do processo.
#[no_mangle]
pub unsafe extern "C" fn pinker_publico_liberar(ponteiro: *mut u8) {
    if ponteiro.is_null() {
        erro_memoria_publica("'liberar' rejeita ponteiro nulo");
    }
    let mut memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    if let Some(indice) = indice_base_publica_mais_recente(&memoria.alocacoes, ponteiro as usize) {
        if !memoria.alocacoes[indice].viva {
            erro_memoria_publica("E-RUNTIME-MEM-DOUBLE-FREE: 'liberar' detectou double free");
        }
        let alocacao = memoria.alocacoes[indice];
        debug_assert!(alocacao.identidade > 0);
        debug_assert!(alocacao.tamanho > 0);
        descomprometer_paginas_publicas(alocacao.base, alocacao.reservado)
            .unwrap_or_else(|_| erro_memoria_publica("falha ao descomprometer memória pública"));
        let next_budget = release_public_live_bytes(memoria.budget, alocacao.reservado)
            .unwrap_or_else(|| erro_memoria_publica("underflow no orçamento público vivo"));
        memoria.alocacoes[indice].viva = false;
        memoria.budget = next_budget;
        return;
    }
    let endereco = ponteiro as usize;
    if memoria.alocacoes.iter().any(|alocacao| {
        alocacao
            .base
            .checked_add(alocacao.tamanho)
            .is_some_and(|fim| endereco > alocacao.base && endereco < fim)
    }) {
        erro_memoria_publica(
            "E-RUNTIME-MEM-INTERIOR-FREE: 'liberar' rejeita ponteiro interior; use o ponteiro-base",
        );
    }
    erro_memoria_publica(
        "E-RUNTIME-MEM-FOREIGN-FREE: 'liberar' rejeita ponteiro estrangeiro ou de domínio interno",
    );
}

/// Veredicto de um acesso à memória pública.
///
/// A decisão vive numa unidade **pura**, separada do efeito, pelo mesmo motivo
/// que `union_budget_reserve`: `erro_memoria_publica` encerra o processo, então
/// só uma função sem efeito pode ser exercitada por teste interno. Assim a
/// matriz de endereços (não mapeado, nulo, pilha, dado estático, função,
/// interno do runtime, região liberada, base viva, interior, primeiro e último
/// byte, um byte depois, acesso multibyte cruzando o limite) é verificável sem
/// derrubar o runner, e `load` e `store` compartilham exatamente o mesmo
/// predicado — não há como divergirem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VeredictoAcesso {
    Permitido,
    MetadadosInvalidos,
    OverflowEndereco,
    /// O endereço não cai em nenhuma região pública registrada. Cobre endereço
    /// fabricado a partir de inteiro, nulo, pilha, dado estático, código,
    /// alocação interna do runtime e mapeamento estrangeiro: para a Pinker,
    /// nada disso é memória pública endereçável.
    Desconhecido,
    UsoAposLiberar,
    Desalinhado,
    CruzaLimite,
    ForaDosLimites,
}

impl VeredictoAcesso {
    /// Diagnóstico estável do veredicto. `None` para o acesso permitido.
    fn diagnostico(self) -> Option<&'static str> {
        match self {
            Self::Permitido => None,
            Self::MetadadosInvalidos => Some("metadados inválidos de acesso à memória pública"),
            Self::OverflowEndereco => {
                Some("E-RUNTIME-MEM-ADDRESS-OVERFLOW: overflow no acesso à memória pública")
            }
            Self::Desconhecido => {
                Some("E-RUNTIME-MEM-UNKNOWN-ACCESS: acesso sem região pública registrada")
            }
            Self::UsoAposLiberar => {
                Some("E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública")
            }
            Self::Desalinhado => {
                Some("E-RUNTIME-MEM-MISALIGNED: acesso desalinhado à memória pública")
            }
            Self::CruzaLimite => Some(
                "E-RUNTIME-MEM-CROSS-BOUNDARY: acesso multibyte cruza o limite da alocação pública",
            ),
            Self::ForaDosLimites => {
                Some("E-RUNTIME-MEM-OUT-OF-BOUNDS: acesso fora dos limites da alocação pública")
            }
        }
    }
}

/// Classifica um acesso contra o registro público, sem efeito colateral.
///
/// A ordem das verificações é o contrato: metadados do acesso, overflow do
/// intervalo **consultado**, existência da região, estado vivo, alinhamento e,
/// por fim, contenção — distinguindo interior que cruza o limite de acesso que
/// começa antes da base.
///
/// O overflow que é classificado aqui é o de `endereco + largura`, que vem de
/// fora e pode ser qualquer coisa. Já `base + tamanho` não é classificado:
/// nenhuma entrada do registro pode tê-lo, por invariante do construtor
/// (`fim_da_regiao`). Por isso não existe veredicto de "metadados de região
/// inválidos" — ele seria inalcançável, e um veredicto inalcançável mente
/// sobre o contrato.
fn classificar_acesso_publico(
    registro: &[AlocacaoPublica],
    endereco: usize,
    largura: usize,
    alinhamento: usize,
) -> VeredictoAcesso {
    if largura == 0 || alinhamento == 0 || !alinhamento.is_power_of_two() {
        return VeredictoAcesso::MetadadosInvalidos;
    }
    let Some(fim_acesso) = endereco.checked_add(largura) else {
        return VeredictoAcesso::OverflowEndereco;
    };
    let candidata = registro.iter().rev().find(|alocacao| {
        let fim = fim_da_regiao(alocacao);
        (endereco >= alocacao.base && endereco <= fim)
            || (endereco < alocacao.base && fim_acesso > alocacao.base)
    });
    let Some(alocacao) = candidata else {
        return VeredictoAcesso::Desconhecido;
    };
    if !alocacao.viva {
        return VeredictoAcesso::UsoAposLiberar;
    }
    if endereco % alinhamento != 0 {
        return VeredictoAcesso::Desalinhado;
    }
    if endereco >= alocacao.base && fim_acesso <= fim_da_regiao(alocacao) {
        return VeredictoAcesso::Permitido;
    }
    if endereco >= alocacao.base {
        VeredictoAcesso::CruzaLimite
    } else {
        VeredictoAcesso::ForaDosLimites
    }
}

/// Fase 246 + hotfix pós-PR #411 (V4): validação de `deref_load`/`deref_store`
/// sobre memória pública.
///
/// O back-end nativo emite esta chamada para todo acesso através de ponteiro
/// classificado como `Public` ou `Fabricated`. Endereço nunca registrado é
/// recusado aqui, com diagnóstico estável, em vez de virar escrita em memória
/// real e SIGSEGV.
///
/// As outras duas classes de proveniência não chegam aqui, por razões
/// diferentes: `Internal` tem domínio próprio e confrontá-la com o registro
/// público rejeitaria acesso legítimo; `Unclassified` é um limite reconhecido
/// da análise, e um acesso dessa classe **pode** terminar por sinal. Este
/// símbolo não sustenta garantia universal sobre todo ponteiro — o contrato
/// exato está em `MANUAL.md`, seção "Memória explícita".
///
/// O interpretador não compartilha esta implementação: ele valida no seu modelo
/// de memória sintético. O que os dois modos garantem é resultado observável
/// correspondente nos casos cobertos, não um validador único.
#[no_mangle]
pub extern "C" fn pinker_publico_validar_acesso(
    ponteiro: *const u8,
    largura: u64,
    alinhamento: u64,
) {
    let endereco = ponteiro as usize;
    let largura = usize::try_from(largura)
        .unwrap_or_else(|_| erro_memoria_publica("largura de acesso excede a plataforma"));
    let alinhamento = usize::try_from(alinhamento)
        .unwrap_or_else(|_| erro_memoria_publica("alinhamento de acesso excede a plataforma"));
    let memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    let veredicto = classificar_acesso_publico(&memoria.alocacoes, endereco, largura, alinhamento);
    if let Some(diagnostico) = veredicto.diagnostico() {
        erro_memoria_publica(diagnostico);
    }
}

#[no_mangle]
pub extern "C" fn pinker_publico_validar_derivacao(origem: *const u8, derivado: *const u8) {
    let origem = origem as usize;
    let derivado = derivado as usize;
    let memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    let candidata = memoria.alocacoes.iter().rev().find(|alocacao| {
        let fim = fim_da_regiao(alocacao);
        origem >= alocacao.base && origem <= fim
    });
    let Some(alocacao) = candidata else {
        erro_memoria_publica(
            "E-RUNTIME-MEM-UNKNOWN-DERIVATION: origem sem região pública registrada",
        );
    };
    if !alocacao.viva {
        erro_memoria_publica(
            "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública",
        );
    }
    let fim = fim_da_regiao(alocacao);
    if derivado < alocacao.base || derivado > fim {
        erro_memoria_publica(
            "E-RUNTIME-MEM-OUT-OF-BOUNDS: derivação fora dos limites da alocação pública",
        );
    }
}

/// Deriva um ponteiro por uma quantidade de **elementos**, sem converter o
/// ponteiro em um inteiro sem tipo no contrato da linguagem. Proveniência e
/// bounds públicos são validados separadamente pelo chamador, após esta
/// aritmética checada.
#[no_mangle]
pub extern "C" fn pinker_ponteiro_derivar_tipado(
    origem: *const u8,
    deslocamento: u64,
    tamanho_elemento: u64,
    alinhamento_elemento: u64,
) -> *const u8 {
    if origem.is_null() {
        erro_memoria_publica("E-RUNTIME-POINTER-NULL-ARITHMETIC: aritmética sobre ponteiro nulo");
    }
    if tamanho_elemento == 0
        || alinhamento_elemento == 0
        || !alinhamento_elemento.is_power_of_two()
        || tamanho_elemento % alinhamento_elemento != 0
    {
        erro_memoria_publica("E-RUNTIME-POINTER-LAYOUT: layout de elemento inválido");
    }
    let delta = deslocamento
        .checked_mul(tamanho_elemento)
        .unwrap_or_else(|| {
            erro_memoria_publica(
                "E-RUNTIME-POINTER-OFFSET-OVERFLOW: overflow ao escalar deslocamento",
            )
        });
    let delta = usize::try_from(delta).unwrap_or_else(|_| {
        erro_memoria_publica("E-RUNTIME-POINTER-OFFSET-OVERFLOW: deslocamento excede a plataforma")
    });
    let derivado = origem.wrapping_add(delta);
    if derivado < origem {
        erro_memoria_publica("E-RUNTIME-POINTER-ADDRESS-OVERFLOW: overflow ao derivar endereço")
    }
    derivado
}

#[no_mangle]
pub extern "C" fn pinker_publico_validar_ponteiro_funcao(endereco: usize) {
    if endereco == 0 {
        erro_memoria_publica("chamada nula por ponteiro cru de função");
    }
}
// @pinker-nav:end runtime.memoria.alocador

// ---------------------------------------------------------------------------
// Verso dinâmico (Fase 215/B4)
//
// Representação nativa de `verso`: ponteiro único para um bloco
// `[tamanho_em_bytes: u64][bytes utf-8...]`. Literais estáticos em `.rodata`
// e versos de heap compartilham o mesmo layout, então todas as operações
// abaixo funcionam uniformemente sobre qualquer valor de verso.
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.texto.operacoes
// @pinker-nav:domain texto
// @pinker-nav:layer runtime
// @pinker-nav:summary Operações de verso (tamanho, concatenação, igualdade, busca, divisão, substituição, caixa) sobre o layout length-prefixed `[u64 len][bytes]`; os helpers `unsafe` (verso_bytes, verso_str) leem via from_raw_parts/from_utf8_unchecked confiando no chamador sem validar o ponteiro nem o UTF-8, e cada transformação aloca um novo bloco de verso cujo ownership passa ao chamador; erros de índice, separador vazio ou padrão vazio abortam o processo via erro_fatal.
/// Bytes de um verso length-prefixed, sem copiar.
///
/// # Safety
/// `v` deve apontar para um bloco `[u64 len][len bytes]` válido.
unsafe fn verso_bytes<'a>(v: *const u8) -> &'a [u8] {
    let len = (v as *const u64).read_unaligned() as usize;
    std::slice::from_raw_parts(v.add(8), len)
}

/// Quantidade de caracteres (code points Unicode) de um verso — espelha a
/// semântica de `tamanho_verso` do interpretador (`chars().count()`).
///
/// # Safety
/// `v` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_tamanho(v: *const u8) -> u64 {
    verso_bytes(v)
        .iter()
        .filter(|byte| (**byte & 0b1100_0000) != 0b1000_0000)
        .count() as u64
}

/// Concatena dois versos num novo bloco de heap (layout length-prefixed).
///
/// # Safety
/// `a` e `b` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_juntar(a: *const u8, b: *const u8) -> *mut u8 {
    let bytes_a = verso_bytes(a);
    let bytes_b = verso_bytes(b);
    let total = bytes_a.len() + bytes_b.len();
    let bloco = pinker_alocar(total as u64 + 8);
    if bloco.is_null() {
        return bloco;
    }
    (bloco as *mut u64).write_unaligned(total as u64);
    std::ptr::copy_nonoverlapping(bytes_a.as_ptr(), bloco.add(8), bytes_a.len());
    std::ptr::copy_nonoverlapping(
        bytes_b.as_ptr(),
        bloco.add(8 + bytes_a.len()),
        bytes_b.len(),
    );
    bloco
}

/// Igualdade byte a byte entre dois versos (1 = iguais, 0 = diferentes) —
/// espelha `igual_verso` do interpretador.
///
/// # Safety
/// `a` e `b` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_igual(a: *const u8, b: *const u8) -> u64 {
    u64::from(verso_bytes(a) == verso_bytes(b))
}

// ---------------------------------------------------------------------------
// Família texto completa (Fase 219/B8)
//
// Cada função converte os bytes UTF-8 do verso para `&str` e usa exatamente
// as mesmas chamadas da std que o interpretador usa (`trim`, `to_lowercase`,
// `split`, `replace`, `find`, `chars().nth`, `parse`, ...), garantindo
// paridade de comportamento por construção.
// ---------------------------------------------------------------------------

/// Visão `&str` de um verso (as fontes Pinker são UTF-8 válido).
///
/// # Safety
/// `v` deve apontar para um bloco de verso válido com bytes UTF-8.
unsafe fn verso_str<'a>(v: *const u8) -> &'a str {
    std::str::from_utf8_unchecked(verso_bytes(v))
}

/// Aloca um novo verso length-prefixed a partir de um `&str`.
fn verso_alocar(texto: &str) -> *mut u8 {
    let bytes = texto.as_bytes();
    let bloco = pinker_alocar(bytes.len() as u64 + 8);
    if bloco.is_null() {
        erro_fatal("sem memória ao alocar verso");
    }
    unsafe {
        (bloco as *mut u64).write_unaligned(bytes.len() as u64);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), bloco.add(8), bytes.len());
    }
    bloco
}

/// # Safety
/// `v` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_indice(v: *const u8, indice: u64) -> *mut u8 {
    let Some(ch) = verso_str(v).chars().nth(indice as usize) else {
        erro_fatal("índice fora da faixa em 'indice_verso'");
    };
    verso_alocar(&ch.to_string())
}

/// Converte um índice em Unicode scalar values para uma fronteira UTF-8.
/// Inclui a fronteira final, portanto `index == texto.chars().count()` é válido.
fn verso_codepoint_byte_offset(texto: &str, index: u64) -> Option<usize> {
    let mut logical = 0_u64;
    for (byte_offset, _) in texto.char_indices() {
        if logical == index {
            return Some(byte_offset);
        }
        logical = logical.checked_add(1)?;
    }
    (logical == index).then_some(texto.len())
}

/// Fatia `texto` por Unicode scalar values no intervalo zero-based `[inicio, fim)`.
/// Sempre aloca um novo verso, inclusive para a fatia vazia.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido com bytes UTF-8.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_fatiar(texto: *const u8, inicio: u64, fim: u64) -> *mut u8 {
    let texto = verso_str(texto);
    if inicio > fim {
        erro_fatal("intervalo inválido em 'fatiar_verso': início maior que fim");
    }
    let length = match u64::try_from(texto.chars().count()) {
        Ok(length) => length,
        Err(_) => erro_fatal("comprimento textual excede a faixa de índice de 'fatiar_verso'"),
    };
    if inicio > length {
        erro_fatal("índice inicial fora da faixa em 'fatiar_verso'");
    }
    if fim > length {
        erro_fatal("índice final fora da faixa em 'fatiar_verso'");
    }
    let inicio_byte = verso_codepoint_byte_offset(texto, inicio)
        .unwrap_or_else(|| erro_fatal("falha interna ao resolver início de 'fatiar_verso'"));
    let fim_byte = verso_codepoint_byte_offset(texto, fim)
        .unwrap_or_else(|| erro_fatal("falha interna ao resolver fim de 'fatiar_verso'"));
    verso_alocar(&texto[inicio_byte..fim_byte])
}

/// # Safety
/// `texto` e `trecho` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_contem(texto: *const u8, trecho: *const u8) -> u64 {
    u64::from(verso_str(texto).contains(verso_str(trecho)))
}

/// # Safety
/// `texto` e `prefixo` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_comeca_com(texto: *const u8, prefixo: *const u8) -> u64 {
    u64::from(verso_str(texto).starts_with(verso_str(prefixo)))
}

/// # Safety
/// `texto` e `sufixo` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_termina_com(texto: *const u8, sufixo: *const u8) -> u64 {
    u64::from(verso_str(texto).ends_with(verso_str(sufixo)))
}

/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_vazio(texto: *const u8) -> u64 {
    u64::from(verso_str(texto).is_empty())
}

/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_nao_vazio(texto: *const u8) -> u64 {
    u64::from(!verso_str(texto).is_empty())
}

/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_aparar(texto: *const u8) -> *mut u8 {
    verso_alocar(verso_str(texto).trim())
}

/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_minusculo(texto: *const u8) -> *mut u8 {
    verso_alocar(&verso_str(texto).to_lowercase())
}

/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_maiusculo(texto: *const u8) -> *mut u8 {
    verso_alocar(&verso_str(texto).to_uppercase())
}

/// Posição em bytes do trecho, ou `u64::MAX` se ausente (como o interpretador).
///
/// # Safety
/// `texto` e `trecho` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_indice_em(texto: *const u8, trecho: *const u8) -> u64 {
    verso_str(texto)
        .find(verso_str(trecho))
        .map_or(u64::MAX, |v| v as u64)
}

/// Como `indice_em`, mas rejeita padrão vazio (semântica de `buscar_verso`).
///
/// # Safety
/// `texto` e `padrao` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_buscar(texto: *const u8, padrao: *const u8) -> u64 {
    let padrao = verso_str(padrao);
    if padrao.is_empty() {
        erro_fatal("intrínseca 'buscar_verso' não aceita padrão vazio");
    }
    verso_str(texto).find(padrao).map_or(u64::MAX, |v| v as u64)
}

/// # Safety
/// `texto` e `sep` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_dividir_em(
    texto: *const u8,
    sep: *const u8,
    indice: u64,
) -> *mut u8 {
    let sep = verso_str(sep);
    if sep.is_empty() {
        erro_fatal("separador vazio em 'dividir_verso_em'");
    }
    let Some(parte) = verso_str(texto).split(sep).nth(indice as usize) else {
        erro_fatal("índice fora da faixa em 'dividir_verso_em' para o verso informado");
    };
    verso_alocar(parte)
}

/// # Safety
/// `texto` e `sep` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_dividir_contar(texto: *const u8, sep: *const u8) -> u64 {
    let sep = verso_str(sep);
    if sep.is_empty() {
        erro_fatal("separador vazio em 'dividir_verso_contar'");
    }
    verso_str(texto).split(sep).count() as u64
}

/// # Safety
/// `texto`, `de` e `para` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_substituir(
    texto: *const u8,
    de: *const u8,
    para: *const u8,
) -> *mut u8 {
    let de = verso_str(de);
    if de.is_empty() {
        erro_fatal("trecho de busca vazio em 'substituir_verso'");
    }
    verso_alocar(&verso_str(texto).replace(de, verso_str(para)))
}

/// `juntar_verso_com(a, sep, b)` — concatena com separador no meio.
///
/// # Safety
/// `a`, `sep` e `b` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_juntar_com(
    a: *const u8,
    sep: *const u8,
    b: *const u8,
) -> *mut u8 {
    verso_alocar(&format!(
        "{}{}{}",
        verso_str(a),
        verso_str(sep),
        verso_str(b)
    ))
}
// @pinker-nav:end runtime.texto.operacoes

// @pinker-nav:start runtime.conversoes.numero-texto
// @pinker-nav:domain conversoes
// @pinker-nav:layer runtime
// @pinker-nav:summary Conversão entre verso e bombom: pinker_verso_para_bombom faz trim+parse e aborta o processo (via eprintln + process::exit) em texto não numérico; pinker_bombom_para_verso aloca um novo verso decimal cujo ownership passa ao chamador.
/// Converte texto para `bombom` (`trim` + `parse`), abortando em falha —
/// espelha o erro do interpretador.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_para_bombom(texto: *const u8) -> u64 {
    let texto = verso_str(texto);
    match texto.trim().parse::<u64>() {
        Ok(valor) => valor,
        Err(_) => {
            eprintln!(
                "Erro de Execução (pinker_rt): falha ao converter '{}' para bombom",
                texto
            );
            std::process::exit(1)
        }
    }
}

/// Converte `bombom` para verso decimal.
#[no_mangle]
pub extern "C" fn pinker_bombom_para_verso(valor: u64) -> *mut u8 {
    verso_alocar(&valor.to_string())
}
// @pinker-nav:end runtime.conversoes.numero-texto

// @pinker-nav:start runtime.texto.formatacao
// @pinker-nav:domain texto
// @pinker-nav:layer runtime
// @pinker-nav:summary Autoridade geral `pinker_formatar_verso_pack(modelo,count,entries)`: valida count/size/ponteiros e formata um slice homogêneo de handles `verso`; wrappers 0..8 permanecem somente como adapters ABI legados que encaminham ao pack.
/// Núcleo do `formatar_verso`: placeholders `{}` na ordem, com validação de
/// contagem e de placeholders malformados — espelha o interpretador. Todos os
/// argumentos já chegam como versos (a IR converte `bombom` antes).
unsafe fn formatar_verso_nucleo(modelo: *const u8, args: &[*const u8]) -> *mut u8 {
    let modelo = verso_str(modelo);
    let mut saida = String::new();
    let mut ultimo_idx = 0usize;
    let mut arg_idx = 0usize;
    let mut chars = modelo.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '{' => {
                saida.push_str(&modelo[ultimo_idx..idx]);
                let Some((close_idx, next_ch)) = chars.next() else {
                    erro_fatal(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    );
                };
                if next_ch != '}' {
                    erro_fatal(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    );
                }
                let Some(arg) = args.get(arg_idx) else {
                    erro_fatal("quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos");
                };
                saida.push_str(verso_str(*arg));
                arg_idx += 1;
                ultimo_idx = close_idx + 1;
            }
            '}' => {
                erro_fatal(
                    "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                );
            }
            _ => {}
        }
    }
    saida.push_str(&modelo[ultimo_idx..]);
    if arg_idx != args.len() {
        erro_fatal(
            "quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos",
        );
    }
    verso_alocar(&saida)
}

fn formatar_verso_pack_len(count: u64) -> Option<usize> {
    let count = usize::try_from(count).ok()?;
    let bytes = count.checked_mul(std::mem::size_of::<*const u8>())?;
    (bytes <= isize::MAX as usize).then_some(count)
}

/// Formata um pack homogêneo de handles `verso`.
///
/// A análise semântica aceita somente `bombom | verso` e a IR converte
/// `bombom` para `verso` antes deste ABI. Assim cada entry possui a mesma
/// representação tipada, sem apagar words de famílias arbitrárias.
///
/// # Safety
/// `modelo` deve apontar para um bloco de verso válido. Quando `count > 0`,
/// `args` deve apontar para `count` handles de verso válidos durante a chamada.
#[no_mangle]
pub unsafe extern "C" fn pinker_formatar_verso_pack(
    modelo: *const u8,
    count: u64,
    args: *const *const u8,
) -> *mut u8 {
    let len = formatar_verso_pack_len(count).unwrap_or_else(|| {
        erro_fatal("E-RUNTIME-FORMAT-PACK: count excede a representação da plataforma")
    });
    let args = if len == 0 {
        &[]
    } else {
        if args.is_null() {
            erro_fatal("E-RUNTIME-FORMAT-PACK: ponteiro de entries nulo");
        }
        let entries = std::slice::from_raw_parts(args, len);
        if entries.iter().any(|entry| entry.is_null()) {
            erro_fatal("E-RUNTIME-FORMAT-PACK: handle de verso nulo");
        }
        entries
    };
    formatar_verso_nucleo(modelo, args)
}

macro_rules! formatar_wrappers {
    ($(($nome:ident, $($arg:ident),*)),* $(,)?) => {
        $(
            /// # Safety
            /// Todos os ponteiros devem apontar para blocos de verso válidos.
            #[no_mangle]
            pub unsafe extern "C" fn $nome(modelo: *const u8, $($arg: *const u8),*) -> *mut u8 {
                let args = [$($arg),*];
                pinker_formatar_verso_pack(modelo, args.len() as u64, args.as_ptr())
            }
        )*
    };
}

/// # Safety
/// `modelo` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_formatar_verso_0(modelo: *const u8) -> *mut u8 {
    pinker_formatar_verso_pack(modelo, 0, std::ptr::null())
}

formatar_wrappers!(
    (pinker_formatar_verso_1, a1),
    (pinker_formatar_verso_2, a1, a2),
    (pinker_formatar_verso_3, a1, a2, a3),
    (pinker_formatar_verso_4, a1, a2, a3, a4),
    (pinker_formatar_verso_5, a1, a2, a3, a4, a5),
    (pinker_formatar_verso_6, a1, a2, a3, a4, a5, a6),
    (pinker_formatar_verso_7, a1, a2, a3, a4, a5, a6, a7),
    (pinker_formatar_verso_8, a1, a2, a3, a4, a5, a6, a7, a8),
);
// @pinker-nav:end runtime.texto.formatacao

// ---------------------------------------------------------------------------
// `falar` nativo (Fase 215/B4) — espelha byte a byte as instruções de máquina
// do interpretador: PrintIntInline, PrintBoolInline, PrintStrValueInline,
// PrintSpace e PrintNewline. O flush acontece na quebra de linha (LineWriter).
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.io.saida
// @pinker-nav:domain io
// @pinker-nav:layer runtime
// @pinker-nav:summary Impressão uniforme de falar: bombom/logica/verso/espaço/newline passam pelo mesmo writer com write_all+flush; a disposição de SIGPIPE é estabelecida em pinker_rt_iniciar (e reafirmada por Once nas entradas de I/O) para que pipe fechado retorne erro em qualquer ordem de execução, toda falha de stdout termina pelo diagnóstico controlado de erro_fatal, e restaurar_disposicao_padrao expõe a operação mínima de sistema que os caminhos de subprocesso usam para devolver SIGPIPE a SIG_DFL no filho antes do exec.
#[cfg(unix)]
const SINAL_SIGPIPE: i32 = 13;
#[cfg(unix)]
const SINAL_HANDLER_PADRAO: usize = 0;
#[cfg(unix)]
const SINAL_HANDLER_IGNORAR: usize = 1;
#[cfg(unix)]
const SINAL_HANDLER_ERRO: usize = usize::MAX;

#[cfg(unix)]
extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
}

/// Estabelece a disposição de sinais do processo: `SIGPIPE` passa a ser
/// ignorado para que escrita em pipe fechado devolva `EPIPE` ao chamador em vez
/// de terminar o processo por sinal.
///
/// É idempotente (`Once`). O ponto autoritativo de chamada é
/// `pinker_rt_iniciar`, que roda antes da primeira instrução do programa
/// nativo; `escrever_stdout` também chama, preservando o comportamento anterior
/// para quem exercita o runtime por chamada nativa direta, sem passar pela
/// inicialização (caso dos testes internos da crate).
///
/// Os caminhos de subprocesso **não** repetem a chamada, de propósito: repetir
/// tornaria a inicialização imune a mutação, e a matriz de R5 continuaria verde
/// mesmo com a correção desfeita.
///
/// Sobre herança: `SIG_IGN` sobrevive a `exec`. Os filhos não são afetados
/// porque o próprio runtime restaura `SIG_DFL` no contexto pré-`exec` — ver
/// `comando_saneado` e `restaurar_disposicao_padrao`.
#[cfg(unix)]
fn preparar_disposicao_sinais() {
    static PREPARAR: Once = Once::new();
    PREPARAR.call_once(|| {
        // SAFETY: `signal` só é chamada aqui, uma única vez, com um handler
        // constante e sem executar código de usuário.
        let anterior = unsafe { signal(SINAL_SIGPIPE, SINAL_HANDLER_IGNORAR) };
        if anterior == SINAL_HANDLER_ERRO {
            erro_fatal("falha ao estabelecer a disposição de SIGPIPE do runtime");
        }
    });
}

#[cfg(not(unix))]
fn preparar_disposicao_sinais() {}

/// Devolve `sinal` à disposição padrão (`SIG_DFL`) no processo corrente.
///
/// É a operação de sistema mínima usada pelo contexto pré-`exec` dos
/// subprocessos: uma única chamada a `signal(2)`, sem alocação, sem formatação,
/// sem acesso ao ambiente e sem lock. `signal(2)` está na lista de funções
/// async-signal-safe da POSIX, o que a torna legítima entre `fork` e `exec`.
///
/// Falha vira `io::Error` para que o chamador a propague ao pai como erro
/// controlado de criação do processo — nunca silenciada.
///
/// # Safety
/// A `unsafe` cobre apenas a chamada a `signal(2)`, que é FFI. As precondições
/// são: `sinal` é um número de sinal válido; o handler passado é a constante
/// `SIG_DFL`, então nenhum código de usuário passa a ser executável por sinal; e
/// nenhuma memória é lida ou escrita através de ponteiro. A função não executa
/// código do chamador, portanto é segura para o contexto pré-`exec`.
#[cfg(unix)]
fn restaurar_disposicao_padrao(sinal: i32) -> std::io::Result<()> {
    // SAFETY: ver as precondições acima — FFI pura, handler constante, sem
    // ponteiros e sem código de usuário.
    let anterior = unsafe { signal(sinal, SINAL_HANDLER_PADRAO) };
    if anterior == SINAL_HANDLER_ERRO {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn escrever_stdout(bytes: &[u8]) {
    use std::io::Write as _;

    preparar_disposicao_sinais();
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(bytes)
        .and_then(|()| lock.flush())
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao escrever stdout: {err}")));
}

/// Imprime um `bombom` decimal sem quebra de linha.
#[no_mangle]
pub extern "C" fn pinker_falar_pedaco_bombom(valor: u64) {
    escrever_stdout(valor.to_string().as_bytes());
}

/// Imprime um inteiro com sinal decimal sem quebra de linha.
#[no_mangle]
pub extern "C" fn pinker_falar_pedaco_inteiro(valor: i64) {
    escrever_stdout(valor.to_string().as_bytes());
}

/// Imprime uma `logica` como `verdade`/`falso` sem quebra de linha.
#[no_mangle]
pub extern "C" fn pinker_falar_pedaco_logica(valor: u64) {
    escrever_stdout(if valor != 0 { b"verdade" } else { b"falso" });
}

/// Imprime os bytes de um verso sem quebra de linha.
///
/// # Safety
/// `v` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_falar_pedaco_verso(v: *const u8) {
    escrever_stdout(verso_bytes(v));
}

/// Separador entre argumentos de `falar` (espaço simples).
#[no_mangle]
pub extern "C" fn pinker_falar_espaco() {
    escrever_stdout(b" ");
}

/// Fim de um `falar` (quebra de linha; o LineWriter da std faz o flush).
#[no_mangle]
pub extern "C" fn pinker_falar_fim() {
    escrever_stdout(b"\n");
}
// @pinker-nav:end runtime.io.saida

// ---------------------------------------------------------------------------
// Listas nativas (Fase 216/B5)
//
// Uma lista é um ponteiro para um header fixo `[len: u64][cap: u64][dados:
// *mut u64]`; os elementos são palavras de 8 bytes (o valor de `bombom`, o
// ponteiro de um `verso` ou o valor/handle de um leque), então a mesma
// implementação serve `lista<bombom>`, `lista<verso>` e `lista<Leque>`.
// O header nunca muda de endereço; o crescimento realoca apenas `dados`.
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.listas.dinamicas
// @pinker-nav:domain listas
// @pinker-nav:layer runtime
// @pinker-nav:summary Lista dinâmica com header fixo `[len][cap][dados]` e elementos de 8 bytes (crescimento por dobra de capacidade); contém também erro_fatal, o helper que aborta o processo (eprintln + process::exit) e é compartilhado por todos os domínios seguintes do arquivo; leitura, escrita e inserção fora dos limites abortam via erro_fatal.
const LISTA_CAP_INICIAL: u64 = 8;

fn erro_fatal(msg: &str) -> ! {
    eprintln!("Erro de Execução (pinker_rt): {}", msg);
    std::process::exit(1)
}

#[no_mangle]
pub extern "C" fn pinker_erro_shift_count(contagem: u64, largura: u64) -> ! {
    erro_fatal(&format!(
        "E-RUNTIME-SHIFT-COUNT: contagem {contagem} fora da largura {largura}"
    ))
}

#[no_mangle]
pub extern "C" fn pinker_erro_divisao_zero() -> ! {
    erro_fatal("divisão por zero")
}

unsafe fn lista_len(l: *mut u8) -> u64 {
    (l as *const u64).read()
}

unsafe fn lista_cap(l: *mut u8) -> u64 {
    (l as *const u64).add(1).read()
}

unsafe fn lista_dados(l: *mut u8) -> *mut u64 {
    (l as *const usize).add(2).read() as *mut u64
}

/// Cria uma lista vazia. Devolve nulo apenas se o sistema recusar memória.
#[no_mangle]
pub extern "C" fn pinker_lista_criar() -> *mut u8 {
    let header = pinker_alocar(24);
    if header.is_null() {
        return header;
    }
    let dados = pinker_alocar(LISTA_CAP_INICIAL * 8);
    if dados.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        (header as *mut u64).write(0);
        (header as *mut u64).add(1).write(LISTA_CAP_INICIAL);
        (header as *mut usize).add(2).write(dados as usize);
    }
    header
}

/// Anexa um elemento ao fim da lista, dobrando a capacidade quando cheia.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_anexar(l: *mut u8, valor: u64) {
    let len = lista_len(l);
    let cap = lista_cap(l);
    if len == cap {
        let nova_cap = cap * 2;
        let novos = pinker_alocar(nova_cap * 8);
        if novos.is_null() {
            erro_fatal("sem memória ao crescer lista");
        }
        let antigos = lista_dados(l);
        std::ptr::copy_nonoverlapping(antigos as *const u8, novos, (len * 8) as usize);
        pinker_liberar(antigos as *mut u8);
        (l as *mut u64).add(1).write(nova_cap);
        (l as *mut usize).add(2).write(novos as usize);
    }
    lista_dados(l).add(len as usize).write(valor);
    (l as *mut u64).write(len + 1);
}

/// Quantidade de elementos da lista.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_tamanho(l: *mut u8) -> u64 {
    lista_len(l)
}

/// Elemento na posição `indice`; aborta com erro claro fora dos limites,
/// espelhando o erro de runtime do interpretador.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_obter(l: *mut u8, indice: u64) -> u64 {
    if indice >= lista_len(l) {
        erro_fatal("índice fora dos limites em leitura de lista");
    }
    lista_dados(l).add(indice as usize).read()
}

/// Substitui o elemento na posição `indice`.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_definir(l: *mut u8, indice: u64, valor: u64) {
    if indice >= lista_len(l) {
        erro_fatal("índice fora dos limites em escrita de lista");
    }
    lista_dados(l).add(indice as usize).write(valor);
}

/// Remove e devolve o último elemento; aborta em lista vazia.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_tirar_ultimo(l: *mut u8) -> u64 {
    let len = lista_len(l);
    if len == 0 {
        erro_fatal("remoção do fim em lista vazia");
    }
    let valor = lista_dados(l).add((len - 1) as usize).read();
    (l as *mut u64).write(len - 1);
    valor
}

/// Insere um elemento na posição `indice`, deslocando o sufixo.
///
/// # Safety
/// `l` deve ser uma lista criada por `pinker_lista_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_lista_inserir(l: *mut u8, indice: u64, valor: u64) {
    let len = lista_len(l);
    if indice > len {
        erro_fatal("índice fora dos limites em inserção de lista");
    }
    pinker_lista_anexar(l, 0);
    let dados = lista_dados(l);
    let mut i = lista_len(l) - 1;
    while i > indice {
        dados
            .add(i as usize)
            .write(dados.add((i - 1) as usize).read());
        i -= 1;
    }
    dados.add(indice as usize).write(valor);
}
// @pinker-nav:end runtime.listas.dinamicas

// ---------------------------------------------------------------------------
// Mapas nativos (Fase 217/B6)
//
// Um mapa é um ponteiro para o header `[len: u64][cap: u64][chaves: *mut u64]
// [valores: *mut u64][chave_e_verso: u64]`. Chaves e valores são palavras de
// 8 bytes; chaves `verso` (ponteiros) comparam por CONTEÚDO via
// `pinker_verso_igual`, chaves `bombom` comparam por valor. A ordem de
// inserção é preservada (inclusive na iteração e após remoções), o que torna
// a iteração nativa determinística.
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.mapas.dinamicos
// @pinker-nav:domain mapas
// @pinker-nav:layer runtime
// @pinker-nav:summary Mapa dinâmico com headers paralelos de chaves e valores (`[len][cap][chaves][valores][chave_e_verso]`), busca linear O(n), comparação de chave por conteúdo (pinker_verso_igual) quando chave_e_verso ou por valor caso contrário, remoção com deslocamento que preserva ordem de inserção, e cursor de iteração criado como snapshot das chaves (mutações no mapa após a criação do cursor não afetam a iteração já em curso); somente a leitura por pinker_mapa_obter aborta via erro_fatal em chave ausente — pinker_mapa_tem devolve 0 e pinker_mapa_remover é no-op quando a chave falta —, e o cursor esgotado (pinker_mapa_iterador_proxima) também aborta via erro_fatal.
const MAPA_CAP_INICIAL: u64 = 8;

unsafe fn mapa_len(m: *mut u8) -> u64 {
    (m as *const u64).read()
}

unsafe fn mapa_cap(m: *mut u8) -> u64 {
    (m as *const u64).add(1).read()
}

unsafe fn mapa_chaves(m: *mut u8) -> *mut u64 {
    (m as *const usize).add(2).read() as *mut u64
}

unsafe fn mapa_valores(m: *mut u8) -> *mut u64 {
    (m as *const usize).add(3).read() as *mut u64
}

unsafe fn mapa_chave_e_verso(m: *mut u8) -> bool {
    (m as *const u64).add(4).read() != 0
}

unsafe fn mapa_chave_igual(m: *mut u8, a: u64, b: u64) -> bool {
    if mapa_chave_e_verso(m) {
        pinker_verso_igual(a as *const u8, b as *const u8) != 0
    } else {
        a == b
    }
}

unsafe fn mapa_buscar(m: *mut u8, chave: u64) -> Option<u64> {
    let len = mapa_len(m);
    let chaves = mapa_chaves(m);
    let mut i = 0u64;
    while i < len {
        if mapa_chave_igual(m, chaves.add(i as usize).read(), chave) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn mapa_criar_com_tipo(chave_e_verso: u64) -> *mut u8 {
    let header = pinker_alocar(40);
    if header.is_null() {
        return header;
    }
    let chaves = pinker_alocar(MAPA_CAP_INICIAL * 8);
    let valores = pinker_alocar(MAPA_CAP_INICIAL * 8);
    if chaves.is_null() || valores.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        (header as *mut u64).write(0);
        (header as *mut u64).add(1).write(MAPA_CAP_INICIAL);
        (header as *mut usize).add(2).write(chaves as usize);
        (header as *mut usize).add(3).write(valores as usize);
        (header as *mut u64).add(4).write(chave_e_verso);
    }
    header
}

/// Cria um mapa com chave `bombom` (comparação por valor).
#[no_mangle]
pub extern "C" fn pinker_mapa_criar_chave_bombom() -> *mut u8 {
    mapa_criar_com_tipo(0)
}

/// Cria um mapa com chave `verso` (comparação por conteúdo).
#[no_mangle]
pub extern "C" fn pinker_mapa_criar_chave_verso() -> *mut u8 {
    mapa_criar_com_tipo(1)
}

/// Define/substitui o valor de uma chave, preservando a ordem de inserção.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_definir(m: *mut u8, chave: u64, valor: u64) {
    if let Some(indice) = mapa_buscar(m, chave) {
        mapa_valores(m).add(indice as usize).write(valor);
        return;
    }
    let len = mapa_len(m);
    let cap = mapa_cap(m);
    if len == cap {
        let nova_cap = cap * 2;
        let novas_chaves = pinker_alocar(nova_cap * 8);
        let novos_valores = pinker_alocar(nova_cap * 8);
        if novas_chaves.is_null() || novos_valores.is_null() {
            erro_fatal("sem memória ao crescer mapa");
        }
        std::ptr::copy_nonoverlapping(
            mapa_chaves(m) as *const u8,
            novas_chaves,
            (len * 8) as usize,
        );
        std::ptr::copy_nonoverlapping(
            mapa_valores(m) as *const u8,
            novos_valores,
            (len * 8) as usize,
        );
        pinker_liberar(mapa_chaves(m) as *mut u8);
        pinker_liberar(mapa_valores(m) as *mut u8);
        (m as *mut u64).add(1).write(nova_cap);
        (m as *mut usize).add(2).write(novas_chaves as usize);
        (m as *mut usize).add(3).write(novos_valores as usize);
    }
    mapa_chaves(m).add(len as usize).write(chave);
    mapa_valores(m).add(len as usize).write(valor);
    (m as *mut u64).write(len + 1);
}

/// Valor de uma chave; aborta com erro claro se a chave estiver ausente,
/// espelhando o erro de runtime do interpretador.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_obter(m: *mut u8, chave: u64) -> u64 {
    let Some(indice) = mapa_buscar(m, chave) else {
        erro_fatal("chave ausente em leitura de mapa");
    };
    mapa_valores(m).add(indice as usize).read()
}

/// 1 se a chave existe, 0 caso contrário.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_tem(m: *mut u8, chave: u64) -> u64 {
    u64::from(mapa_buscar(m, chave).is_some())
}

/// Quantidade de pares do mapa.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_tamanho(m: *mut u8) -> u64 {
    mapa_len(m)
}

/// Remove uma chave se existir (ausência é silenciosa, como no interpretador),
/// deslocando o sufixo para preservar a ordem de inserção.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_remover(m: *mut u8, chave: u64) {
    let Some(indice) = mapa_buscar(m, chave) else {
        return;
    };
    let len = mapa_len(m);
    let chaves = mapa_chaves(m);
    let valores = mapa_valores(m);
    let mut i = indice;
    while i + 1 < len {
        chaves
            .add(i as usize)
            .write(chaves.add((i + 1) as usize).read());
        valores
            .add(i as usize)
            .write(valores.add((i + 1) as usize).read());
        i += 1;
    }
    (m as *mut u64).write(len - 1);
}

/// Cria um cursor de iteração com snapshot das chaves (mesma semântica do
/// interpretador: mutações após a criação do cursor não afetam a iteração).
/// Layout do cursor: `[restante... na verdade: [len: u64][proximo: u64][chaves...]]`.
///
/// # Safety
/// `m` deve ser um mapa criado por `pinker_mapa_criar_*`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_iterador_criar(m: *mut u8) -> *mut u8 {
    let len = mapa_len(m);
    let cursor = pinker_alocar(16 + len * 8);
    if cursor.is_null() {
        erro_fatal("sem memória ao criar cursor de mapa");
    }
    (cursor as *mut u64).write(len);
    (cursor as *mut u64).add(1).write(0);
    std::ptr::copy_nonoverlapping(
        mapa_chaves(m) as *const u8,
        cursor.add(16),
        (len * 8) as usize,
    );
    cursor
}

/// Próxima chave do cursor; aborta se o cursor estiver esgotado (o desugaring
/// de `para cada` nunca avança além do tamanho do snapshot).
///
/// # Safety
/// `cursor` deve ter sido criado por `pinker_mapa_iterador_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_mapa_iterador_proxima(cursor: *mut u8) -> u64 {
    let len = (cursor as *const u64).read();
    let proximo = (cursor as *const u64).add(1).read();
    if proximo >= len {
        erro_fatal("cursor de mapa esgotado");
    }
    let chave = (cursor.add(16) as *const u64).add(proximo as usize).read();
    (cursor as *mut u64).add(1).write(proximo + 1);
    chave
}
// @pinker-nav:end runtime.mapas.dinamicos

// ---------------------------------------------------------------------------
// Leques com carga nativos (Fase 218/B7)
//
// Um valor de leque com carga é um ponteiro para o header `[tag: u64]
// [n: u64][cap: u64][cargas: *mut u64]`. As cargas são palavras de 8 bytes
// (valor de `bombom`, ponteiro de `verso` ou ponteiro de outro leque —
// habilitando AST recursiva). A construção espelha a cadeia da IR:
// `criar_0(tag)` seguido de um `anexar` por carga (que devolve o handle).
// Leques SEM carga continuam discriminantes imediatos e nunca chegam aqui.
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.leques.variantes
// @pinker-nav:domain leques
// @pinker-nav:layer runtime
// @pinker-nav:summary Leque com carga: header `[tag][n][cap][cargas]` construído por pinker_leque_criar_0, que inicializa a tag com n=0 cargas; anexos sucessivos via pinker_leque_anexar adicionam cargas e devolvem o mesmo handle (cadeia composável espelhando a IR); pinker_leque_carga verifica a tag antes de ler e aborta via erro_fatal em variante inconsistente ou índice fora da faixa.
const LEQUE_CAP_INICIAL: u64 = 4;

unsafe fn leque_n(l: *mut u8) -> u64 {
    (l as *const u64).add(1).read()
}

unsafe fn leque_cargas(l: *mut u8) -> *mut u64 {
    (l as *const usize).add(3).read() as *mut u64
}

/// Cria um valor de leque com a tag dada e zero cargas.
#[no_mangle]
pub extern "C" fn pinker_leque_criar_0(tag: u64) -> *mut u8 {
    let header = pinker_alocar(32);
    if header.is_null() {
        erro_fatal("sem memória ao criar leque");
    }
    let cargas = pinker_alocar(LEQUE_CAP_INICIAL * 8);
    if cargas.is_null() {
        erro_fatal("sem memória ao criar cargas de leque");
    }
    unsafe {
        (header as *mut u64).write(tag);
        (header as *mut u64).add(1).write(0);
        (header as *mut u64).add(2).write(LEQUE_CAP_INICIAL);
        (header as *mut usize).add(3).write(cargas as usize);
    }
    header
}

/// Anexa uma carga (palavra de 8 bytes) e devolve o mesmo handle,
/// espelhando a cadeia composável da IR.
///
/// # Safety
/// `l` deve ser um leque criado por `pinker_leque_criar_0`.
#[no_mangle]
pub unsafe extern "C" fn pinker_leque_anexar(l: *mut u8, valor: u64) -> *mut u8 {
    let n = leque_n(l);
    let cap = (l as *const u64).add(2).read();
    if n == cap {
        let nova_cap = cap * 2;
        let novas = pinker_alocar(nova_cap * 8);
        if novas.is_null() {
            erro_fatal("sem memória ao crescer cargas de leque");
        }
        std::ptr::copy_nonoverlapping(leque_cargas(l) as *const u8, novas, (n * 8) as usize);
        pinker_liberar(leque_cargas(l) as *mut u8);
        (l as *mut u64).add(2).write(nova_cap);
        (l as *mut usize).add(3).write(novas as usize);
    }
    leque_cargas(l).add(n as usize).write(valor);
    (l as *mut u64).add(1).write(n + 1);
    l
}

/// Tag (discriminante) de um valor de leque com carga.
///
/// # Safety
/// `l` deve ser um leque criado por `pinker_leque_criar_0`.
#[no_mangle]
pub unsafe extern "C" fn pinker_leque_tag(l: *mut u8) -> u64 {
    (l as *const u64).read()
}

/// Carga na posição `indice`, verificando a consistência da variante —
/// espelha a verificação de tag do interpretador (Fase 210).
///
/// # Safety
/// `l` deve ser um leque criado por `pinker_leque_criar_0`.
#[no_mangle]
pub unsafe extern "C" fn pinker_leque_carga(l: *mut u8, tag: u64, indice: u64) -> u64 {
    if pinker_leque_tag(l) != tag {
        erro_fatal("extração de carga com variante inconsistente em leque");
    }
    if indice >= leque_n(l) {
        erro_fatal("carga ausente em leque");
    }
    leque_cargas(l).add(indice as usize).read()
}
// @pinker-nav:end runtime.leques.variantes

// ---------------------------------------------------------------------------
// Uniões estruturais tagged (Fase 248)
//
// O valor que atravessa a ABI é sempre uma palavra: ponteiro para este
// descritor imutável. A alocação é deliberadamente privada (Box) e portanto
// não pertence ao domínio público de pinker_alocar/pinker_liberar. O lifetime
// é monotônico nesta fase.
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.unioes.descritor
// @pinker-nav:domain unioes
// @pinker-nav:layer runtime
// @pinker-nav:summary Descritor imutável de união estrutural com identidade internada, tag determinística, layout validado e snapshot integral do payload — escalar, handle opaco ou agregado multi-palavra copiado byte a byte para storage próprio do descritor; a contabilidade de descritores, bytes de payload e bytes de metadata é feita por uma unidade pura e atômica (`union_budget_reserve`), e criação e leitura usam uma ABI interna separada da memória pública.
/// Marca do descritor. Um handle que não a apresente não é um descritor criado
/// por este runtime, e nenhuma leitura adicional é feita nele.
const UNION_MAGIC: u64 = 0x504b_5f55_4e49_4f31;

/// Tetos de recurso. São os mesmos valores documentados em
/// `crate::union_payload` do compilador; a duplicação é a fronteira da ABI, não
/// uma segunda política.
const MAX_UNION_PAYLOAD_BYTES: u64 = 4096;
const MAX_UNION_PAYLOAD_ALIGN: u64 = 16;
const MAX_UNION_DESCRIPTORS: u64 = 1_000_000;
const MAX_UNION_TOTAL_PAYLOAD_BYTES: u64 = 256 * 1024 * 1024;
const UNION_DESCRIPTOR_METADATA_BYTES: u64 = 8 * 8;
const MAX_UNION_METADATA_BYTES: u64 = MAX_UNION_DESCRIPTORS * UNION_DESCRIPTOR_METADATA_BYTES;

/// Cabeçalho do descritor imutável de união.
///
/// O payload vive **no mesmo bloco**, a partir de `payload_offset` já alinhado.
/// Um único bloco mantém ownership e validação triviais: o descritor é dono de
/// tudo que devolve, e nada aponta para o storage do chamador.
#[repr(C)]
struct PinkerUnionDescriptor {
    magic: u64,
    union_type_id: u64,
    tag: u64,
    payload_size: u64,
    payload_align: u64,
    payload_offset: u64,
    allocation_size: u64,
    allocation_align: u64,
}

/// Contabilidade corrente dos recursos de união.
///
/// É um valor puro: não conhece alocador, mutex nem handle. Isso permite provar
/// cada fronteira de orçamento sem materializar um milhão de descritores e sem
/// atravessar a fronteira fatal de `erro_fatal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct UnionBudget {
    descriptors: u64,
    payload_bytes: u64,
    metadata_bytes: u64,
}

/// Tetos aplicáveis a um `UnionBudget`.
///
/// O runtime de produção usa exclusivamente [`UNION_BUDGET_LIMITS`], derivado
/// das constantes canônicas. Os testes passam limites pequenos pelo mesmo
/// parâmetro — não há variável de ambiente e o comportamento não muda entre
/// debug e release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnionBudgetLimits {
    max_descriptors: u64,
    max_payload_bytes: u64,
    max_metadata_bytes: u64,
}

/// Motivo estável de uma reserva recusada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnionBudgetError {
    Descriptors,
    PayloadBytes,
    MetadataBytes,
    DescriptorOverflow,
    PayloadOverflow,
    MetadataOverflow,
}

impl UnionBudgetError {
    /// Mensagem do diagnóstico fatal correspondente.
    fn message(self) -> &'static str {
        match self {
            UnionBudgetError::Descriptors => "orçamento de descritores de união esgotado",
            UnionBudgetError::PayloadBytes => "orçamento de bytes de payload de união esgotado",
            UnionBudgetError::MetadataBytes => {
                "orçamento de metadata de descritores de união esgotado"
            }
            UnionBudgetError::DescriptorOverflow => "overflow no orçamento de descritores de união",
            UnionBudgetError::PayloadOverflow => "overflow no orçamento de bytes de união",
            UnionBudgetError::MetadataOverflow => "overflow no orçamento de metadata de união",
        }
    }
}

/// Limites canônicos do runtime de produção.
const UNION_BUDGET_LIMITS: UnionBudgetLimits = UnionBudgetLimits {
    max_descriptors: MAX_UNION_DESCRIPTORS,
    max_payload_bytes: MAX_UNION_TOTAL_PAYLOAD_BYTES,
    max_metadata_bytes: MAX_UNION_METADATA_BYTES,
};

/// Reserva os recursos de um descritor novo.
///
/// A operação é **atômica por construção**: o orçamento corrente é um parâmetro
/// por valor e o novo orçamento só existe no caminho `Ok`. Uma recusa não pode
/// alterar o estado do chamador porque nada foi escrito.
fn union_budget_reserve(
    current: UnionBudget,
    limits: UnionBudgetLimits,
    payload_size: u64,
) -> Result<UnionBudget, UnionBudgetError> {
    let descriptors = current
        .descriptors
        .checked_add(1)
        .ok_or(UnionBudgetError::DescriptorOverflow)?;
    if descriptors > limits.max_descriptors {
        return Err(UnionBudgetError::Descriptors);
    }
    let payload_bytes = current
        .payload_bytes
        .checked_add(payload_size)
        .ok_or(UnionBudgetError::PayloadOverflow)?;
    if payload_bytes > limits.max_payload_bytes {
        return Err(UnionBudgetError::PayloadBytes);
    }
    let metadata_bytes = current
        .metadata_bytes
        .checked_add(UNION_DESCRIPTOR_METADATA_BYTES)
        .ok_or(UnionBudgetError::MetadataOverflow)?;
    if metadata_bytes > limits.max_metadata_bytes {
        return Err(UnionBudgetError::MetadataBytes);
    }
    Ok(UnionBudget {
        descriptors,
        payload_bytes,
        metadata_bytes,
    })
}

struct EstadoUnioes {
    /// Handles criados por este runtime. Um handle arbitrário nunca é
    /// dereferenciado antes de constar aqui.
    descritores: std::collections::HashSet<usize>,
    orcamento: UnionBudget,
}

fn estado_unioes() -> &'static Mutex<EstadoUnioes> {
    static UNIOES: OnceLock<Mutex<EstadoUnioes>> = OnceLock::new();
    UNIOES.get_or_init(|| {
        Mutex::new(EstadoUnioes {
            descritores: std::collections::HashSet::new(),
            orcamento: UnionBudget::default(),
        })
    })
}

fn union_layout_valid(size: u64, align: u64) -> bool {
    size > 0
        && size <= MAX_UNION_PAYLOAD_BYTES
        && align > 0
        && align.is_power_of_two()
        && align <= MAX_UNION_PAYLOAD_ALIGN
}

/// Calcula o layout do bloco único {cabeçalho, padding, payload} com operações
/// checadas. Overflow em qualquer etapa é diagnóstico, não pânico.
fn union_allocation_layout(payload_size: u64, payload_align: u64) -> Option<(u64, u64, u64)> {
    let header = std::mem::size_of::<PinkerUnionDescriptor>() as u64;
    let header_align = std::mem::align_of::<PinkerUnionDescriptor>() as u64;
    let allocation_align = header_align.max(payload_align);
    let payload_offset = header
        .checked_add(payload_align.checked_sub(1)?)?
        .checked_div(payload_align)?
        .checked_mul(payload_align)?;
    let total = payload_offset.checked_add(payload_size)?;
    let allocation_size = total
        .checked_add(allocation_align.checked_sub(1)?)?
        .checked_div(allocation_align)?
        .checked_mul(allocation_align)?;
    Some((payload_offset, allocation_size, allocation_align))
}

/// Ponto único de injeção de falha de alocação, exercitado apenas por testes do
/// próprio runtime. Não há variável de ambiente: a política documentada não
/// muda em execução, e debug e release se comportam igual.
#[cfg(test)]
static FALHA_ALOCACAO_UNIAO: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn union_alocacao_deve_falhar() -> bool {
    #[cfg(test)]
    {
        FALHA_ALOCACAO_UNIAO.load(std::sync::atomic::Ordering::SeqCst)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Cria um snapshot imutável para um valor de união.
///
/// A origem é copiada **integralmente** para storage próprio do descritor antes
/// de o handle ser exposto. Nenhum ponteiro do chamador permanece referenciado.
///
/// # Safety
/// `payload_source_ptr` deve apontar para ao menos `payload_size` bytes legíveis
/// e alinhados a `payload_align`.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_criar(
    union_type_id: u64,
    tag: u64,
    payload_size: u64,
    payload_align: u64,
    payload_source_ptr: *const u8,
) -> *mut u8 {
    if !union_layout_valid(payload_size, payload_align) {
        erro_fatal("layout inválido ao criar descritor de união estrutural");
    }
    if payload_source_ptr.is_null() {
        erro_fatal("origem nula ao criar descritor de união estrutural");
    }
    if (payload_source_ptr as usize) % (payload_align as usize) != 0 {
        erro_fatal("origem desalinhada ao criar descritor de união estrutural");
    }
    let Some((payload_offset, allocation_size, allocation_align)) =
        union_allocation_layout(payload_size, payload_align)
    else {
        erro_fatal("overflow no layout do descritor de união estrutural");
    };

    {
        let mut estado = estado_unioes()
            .lock()
            .unwrap_or_else(|_| erro_fatal("estado de uniões corrompido"));
        // A reserva é decidida fora do estado: ou o orçamento novo substitui o
        // antigo por inteiro, ou nada é escrito.
        match union_budget_reserve(estado.orcamento, UNION_BUDGET_LIMITS, payload_size) {
            Ok(orcamento) => estado.orcamento = orcamento,
            Err(erro) => erro_fatal(erro.message()),
        }
    }

    let Ok(layout) = Layout::from_size_align(allocation_size as usize, allocation_align as usize)
    else {
        erro_fatal("layout de alocação inválido para descritor de união estrutural");
    };
    // Falha de alocação é diagnóstico controlado; `alloc` devolvendo nulo nunca
    // vira abort de alocador nem dereferência de nulo.
    let base = if union_alocacao_deve_falhar() {
        std::ptr::null_mut()
    } else {
        alloc(layout)
    };
    if base.is_null() {
        erro_fatal("alocação de descritor de união estrutural falhou");
    }

    (base as *mut PinkerUnionDescriptor).write(PinkerUnionDescriptor {
        magic: UNION_MAGIC,
        union_type_id,
        tag,
        payload_size,
        payload_align,
        payload_offset,
        allocation_size,
        allocation_align,
    });
    std::ptr::copy_nonoverlapping(
        payload_source_ptr,
        base.add(payload_offset as usize),
        payload_size as usize,
    );

    // O registro acontece **antes** da exposição: um handle só é observável
    // depois de ser reconhecível.
    estado_unioes()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de uniões corrompido"))
        .descritores
        .insert(base as usize);
    base
}

/// Confirma que o handle foi criado por este runtime antes de qualquer leitura.
unsafe fn union_descriptor(handle: *mut u8, operacao: &str) -> &'static PinkerUnionDescriptor {
    if handle.is_null() {
        erro_fatal(&format!("handle nulo de união estrutural em '{operacao}'"));
    }
    let conhecido = estado_unioes()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de uniões corrompido"))
        .descritores
        .contains(&(handle as usize));
    if !conhecido {
        erro_fatal(&format!(
            "handle desconhecido de união estrutural em '{operacao}'"
        ));
    }
    let descriptor = &*(handle as *const PinkerUnionDescriptor);
    if descriptor.magic != UNION_MAGIC {
        erro_fatal(&format!(
            "descritor de união com marca inválida em '{operacao}'"
        ));
    }
    if !union_layout_valid(descriptor.payload_size, descriptor.payload_align) {
        erro_fatal(&format!(
            "descritor de união contém layout inválido em '{operacao}'"
        ));
    }
    descriptor
}

/// Obtém a tag determinística de uma união, validando também a identidade do
/// tipo de união esperado.
///
/// # Safety
/// `handle` deve ter sido devolvido por `pinker_uniao_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_tag(handle: *mut u8, expected_union_type_id: u64) -> u64 {
    let descriptor = union_descriptor(handle, "uniao_tag");
    if descriptor.union_type_id != expected_union_type_id {
        erro_fatal("leitura de tag com identidade de união divergente");
    }
    descriptor.tag
}

/// Copia o snapshot do payload para storage do chamador.
///
/// O ponteiro interno do descritor nunca é devolvido e o descritor não é
/// alterado.
///
/// # Safety
/// `handle` deve ter sido devolvido por `pinker_uniao_criar` e `destination_ptr`
/// deve apontar para ao menos `expected_size` bytes graváveis e alinhados.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_copiar_payload(
    handle: *mut u8,
    expected_union_type_id: u64,
    expected_tag: u64,
    expected_size: u64,
    expected_align: u64,
    destination_ptr: *mut u8,
) {
    let descriptor = union_descriptor(handle, "uniao_copiar_payload");
    if descriptor.union_type_id != expected_union_type_id {
        erro_fatal("extração de união com identidade de união divergente");
    }
    if descriptor.tag != expected_tag {
        erro_fatal("extração de união com tag incompatível");
    }
    if descriptor.payload_size != expected_size {
        erro_fatal("extração de união com tamanho divergente");
    }
    if descriptor.payload_align != expected_align {
        erro_fatal("extração de união com alinhamento divergente");
    }
    if destination_ptr.is_null() {
        erro_fatal("destino nulo na extração de união estrutural");
    }
    if (destination_ptr as usize) % (expected_align as usize) != 0 {
        erro_fatal("destino desalinhado na extração de união estrutural");
    }
    std::ptr::copy_nonoverlapping(
        handle.add(descriptor.payload_offset as usize) as *const u8,
        destination_ptr,
        descriptor.payload_size as usize,
    );
}
// @pinker-nav:end runtime.unioes.descritor

// ---------------------------------------------------------------------------
// Arquivo, caminho, tempo e acaso nativos (Fase 220/B9)
//
// O modelo de arquivo mantém um descritor aberto por handle; operações não
// re-resolvem o caminho, e handles fechados produzem erro distinto. O gerador
// de acaso replica o MESMO LCG do interpretador (paridade de sementes).
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::io::Seek as _;
use std::sync::{Mutex, OnceLock};

// @pinker-nav:start runtime.arquivos.io
// @pinker-nav:domain arquivos
// @pinker-nav:layer runtime
// @pinker-nav:summary Tabela limitada aos descritores ativos: cada handle mantém o File aberto e o modo, operações reposicionam e usam esse mesmo descritor sem re-resolver o caminho, criar usa create_new, leituras têm limite explícito, e handles ausentes abaixo de proximo_handle são classificados como fechados sem HashSet crescente.
const MAX_ARQUIVO_VERSO_BYTES: u64 = 64 * 1024 * 1024;

struct ArquivoAberto {
    arquivo: std::fs::File,
    anexo: bool,
}

struct EstadoIo {
    arquivos: HashMap<u64, ArquivoAberto>,
    proximo_handle: u64,
}

fn estado_io() -> &'static Mutex<EstadoIo> {
    static IO: OnceLock<Mutex<EstadoIo>> = OnceLock::new();
    IO.get_or_init(|| {
        Mutex::new(EstadoIo {
            arquivos: HashMap::new(),
            proximo_handle: 1,
        })
    })
}

fn io_lock() -> std::sync::MutexGuard<'static, EstadoIo> {
    estado_io()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de arquivos corrompido"))
}

fn abrir_com_flag(arquivo: std::fs::File, anexo: bool) -> u64 {
    let mut io = io_lock();
    let handle = io.proximo_handle;
    io.proximo_handle = io
        .proximo_handle
        .checked_add(1)
        .unwrap_or_else(|| erro_fatal("esgotamento de handles de arquivo"));
    io.arquivos.insert(handle, ArquivoAberto { arquivo, anexo });
    handle
}

fn handle_foi_fechado(io: &EstadoIo, handle: u64) -> bool {
    handle > 0 && handle < io.proximo_handle && !io.arquivos.contains_key(&handle)
}

enum ModoArquivo {
    Abrir,
    Criar,
    Anexar,
}

fn abrir_descritor(caminho: &str, modo: ModoArquivo) -> std::io::Result<std::fs::File> {
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.read(true);
    match modo {
        ModoArquivo::Abrir => {
            opcoes.write(true);
        }
        ModoArquivo::Criar => {
            opcoes.write(true).create_new(true);
        }
        ModoArquivo::Anexar => {
            opcoes.append(true);
        }
    }
    opcoes.open(caminho)
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_abrir(caminho: *const u8) -> u64 {
    let caminho = verso_str(caminho);
    let arquivo = abrir_descritor(caminho, ModoArquivo::Abrir)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao abrir arquivo '{caminho}': {err}")));
    abrir_com_flag(arquivo, false)
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_criar(caminho: *const u8) -> u64 {
    let caminho = verso_str(caminho);
    let arquivo = abrir_descritor(caminho, ModoArquivo::Criar)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao criar arquivo '{caminho}': {err}")));
    abrir_com_flag(arquivo, false)
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_abrir_anexo(caminho: *const u8) -> u64 {
    let caminho = verso_str(caminho);
    let arquivo = abrir_descritor(caminho, ModoArquivo::Anexar).unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao abrir arquivo para anexo '{caminho}': {err}"
        ))
    });
    abrir_com_flag(arquivo, true)
}

#[no_mangle]
pub extern "C" fn pinker_arquivo_fechar(handle: u64) {
    let mut io = io_lock();
    if io.arquivos.remove(&handle).is_none() {
        if handle_foi_fechado(&io, handle) {
            erro_fatal("handle de arquivo já fechado em 'fechar'");
        }
        erro_fatal("handle de arquivo inválido em 'fechar'");
    }
}

fn com_arquivo<R>(handle: u64, nome: &str, f: impl FnOnce(&mut ArquivoAberto) -> R) -> R {
    let mut io = io_lock();
    if let Some(arquivo) = io.arquivos.get_mut(&handle) {
        return f(arquivo);
    }
    if handle_foi_fechado(&io, handle) {
        erro_fatal(&format!("handle de arquivo já fechado em '{nome}'"));
    }
    erro_fatal(&format!("handle de arquivo inválido em '{nome}'"));
}

fn ler_descritor(arq: &mut ArquivoAberto, nome: &str) -> Vec<u8> {
    let tamanho = arq
        .arquivo
        .metadata()
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao medir arquivo em '{nome}': {err}")))
        .len();
    if tamanho > MAX_ARQUIVO_VERSO_BYTES {
        erro_fatal(&format!(
            "arquivo excede limite de {MAX_ARQUIVO_VERSO_BYTES} bytes em '{nome}'"
        ));
    }
    arq.arquivo
        .seek(std::io::SeekFrom::Start(0))
        .unwrap_or_else(|err| {
            erro_fatal(&format!("falha ao reposicionar arquivo em '{nome}': {err}"))
        });
    let mut bytes = Vec::with_capacity(usize::try_from(tamanho).unwrap_or(0));
    let mut limitado = std::io::Read::take(&mut arq.arquivo, MAX_ARQUIVO_VERSO_BYTES + 1);
    limitado
        .read_to_end(&mut bytes)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao ler arquivo em '{nome}': {err}")));
    if bytes.len() as u64 > MAX_ARQUIVO_VERSO_BYTES {
        erro_fatal(&format!(
            "arquivo excede limite de {MAX_ARQUIVO_VERSO_BYTES} bytes em '{nome}'"
        ));
    }
    bytes
}

fn substituir_descritor(arq: &mut ArquivoAberto, nome: &str, bytes: &[u8]) {
    arq.arquivo
        .set_len(0)
        .and_then(|()| arq.arquivo.seek(std::io::SeekFrom::Start(0)).map(|_| ()))
        .and_then(|()| arq.arquivo.write_all(bytes))
        .and_then(|()| arq.arquivo.flush())
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao escrever em '{nome}': {err}")));
}

#[no_mangle]
pub extern "C" fn pinker_arquivo_ler_bombom(handle: u64) -> u64 {
    com_arquivo(handle, "ler_arquivo", |arq| {
        let bytes = ler_descritor(arq, "ler_arquivo");
        let texto = std::str::from_utf8(&bytes)
            .unwrap_or_else(|_| erro_fatal("conteúdo inválido em 'ler_arquivo': UTF-8 esperado"));
        let aparado = texto.trim();
        if aparado.is_empty() {
            erro_fatal("arquivo vazio em 'ler_arquivo'");
        }
        aparado.parse::<u64>().unwrap_or_else(|_| {
            erro_fatal(&format!(
                "conteúdo não numérico em 'ler_arquivo': '{aparado}'"
            ))
        })
    })
}

#[no_mangle]
pub extern "C" fn pinker_arquivo_ler_verso(handle: u64) -> *mut u8 {
    com_arquivo(handle, "ler_verso_arquivo", |arq| {
        let bytes = ler_descritor(arq, "ler_verso_arquivo");
        let texto = std::str::from_utf8(&bytes).unwrap_or_else(|_| {
            erro_fatal("conteúdo inválido em 'ler_verso_arquivo': UTF-8 esperado")
        });
        verso_alocar(texto)
    })
}

#[no_mangle]
pub extern "C" fn pinker_arquivo_escrever_bombom(handle: u64, valor: u64) {
    com_arquivo(handle, "escrever", |arq| {
        let novo = valor.to_string();
        substituir_descritor(arq, "escrever", novo.as_bytes());
    })
}

/// # Safety
/// `valor` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_escrever_verso(handle: u64, valor: *const u8) {
    let valor = verso_str(valor);
    com_arquivo(handle, "escrever_verso", |arq| {
        substituir_descritor(arq, "escrever_verso", valor.as_bytes());
    })
}

#[no_mangle]
pub extern "C" fn pinker_arquivo_truncar(handle: u64) {
    com_arquivo(handle, "truncar_arquivo", |arq| {
        substituir_descritor(arq, "truncar_arquivo", b"");
    })
}

/// # Safety
/// `valor` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_anexar_verso(handle: u64, valor: *const u8) {
    let valor = verso_str(valor);
    com_arquivo(handle, "anexar_verso", |arq| {
        if !arq.anexo {
            erro_fatal("handle sem modo anexo em 'anexar_verso'; use 'abrir_anexo'");
        }
        arq.arquivo
            .seek(std::io::SeekFrom::End(0))
            .and_then(|_| arq.arquivo.write_all(valor.as_bytes()))
            .and_then(|()| arq.arquivo.flush())
            .unwrap_or_else(|err| erro_fatal(&format!("falha ao anexar verso: {err}")));
    })
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_ler_caminho_verso(caminho: *const u8) -> *mut u8 {
    let caminho = verso_str(caminho);
    let conteudo = std::fs::read_to_string(caminho)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao ler arquivo '{caminho}': {err}")));
    verso_alocar(&conteudo)
}

/// # Safety
/// `caminho` e `fallback` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_ou(caminho: *const u8, fallback: *const u8) -> *mut u8 {
    match std::fs::read_to_string(verso_str(caminho)) {
        Ok(conteudo) => verso_alocar(&conteudo),
        Err(_) => verso_alocar(verso_str(fallback)),
    }
}

/// # Safety
/// `origem` e `destino` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_copiar(origem: *const u8, destino: *const u8) {
    std::fs::copy(verso_str(origem), verso_str(destino))
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao copiar arquivo: {err}")));
}

/// # Safety
/// `de` e `para` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_renomear(de: *const u8, para: *const u8) {
    std::fs::rename(verso_str(de), verso_str(para))
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao renomear arquivo: {err}")));
}
// @pinker-nav:end runtime.arquivos.io

// @pinker-nav:start runtime.caminhos.sistema
// @pinker-nav:domain caminhos
// @pinker-nav:layer runtime
// @pinker-nav:summary Consultas e operações de sistema de arquivos sobre caminhos, delegando a std::fs/std::path: pinker_caminho_existe/e_arquivo/e_diretorio devolvem booleano puro (Path::exists/is_file/is_dir) sem nunca abortar, e pinker_caminho_juntar apenas monta o PathBuf; já pinker_caminho_tamanho_arquivo e pinker_caminho_e_vazio (ambas via std::fs::metadata, exigindo que o caminho seja arquivo) e as operações mutadoras (criar/remover diretório, remover arquivo, diretório atual) abortam via erro_fatal com a mensagem do erro original anexada quando o sistema operacional falha.
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_existe(caminho: *const u8) -> u64 {
    u64::from(std::path::Path::new(verso_str(caminho)).exists())
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_e_arquivo(caminho: *const u8) -> u64 {
    u64::from(std::path::Path::new(verso_str(caminho)).is_file())
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_e_diretorio(caminho: *const u8) -> u64 {
    u64::from(std::path::Path::new(verso_str(caminho)).is_dir())
}

/// # Safety
/// `base` e `filho` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_juntar(base: *const u8, filho: *const u8) -> *mut u8 {
    let junto = std::path::PathBuf::from(verso_str(base)).join(verso_str(filho));
    verso_alocar(&junto.to_string_lossy())
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_tamanho_arquivo(caminho: *const u8) -> u64 {
    let caminho = verso_str(caminho);
    let meta = std::fs::metadata(caminho)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao medir arquivo '{caminho}': {err}")));
    if !meta.is_file() {
        erro_fatal("caminho não é arquivo em 'tamanho_arquivo'");
    }
    meta.len()
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_e_vazio(caminho: *const u8) -> u64 {
    let caminho = verso_str(caminho);
    let meta = std::fs::metadata(caminho)
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao medir arquivo '{caminho}': {err}")));
    if !meta.is_file() {
        erro_fatal("caminho não é arquivo em 'e_vazio'");
    }
    u64::from(meta.len() == 0)
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_criar_diretorio(caminho: *const u8) {
    std::fs::create_dir_all(verso_str(caminho))
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao criar diretório: {err}")));
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_remover_arquivo(caminho: *const u8) {
    std::fs::remove_file(verso_str(caminho))
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao remover arquivo: {err}")));
}

/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_caminho_remover_diretorio(caminho: *const u8) {
    std::fs::remove_dir(verso_str(caminho))
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao remover diretório: {err}")));
}

#[no_mangle]
pub extern "C" fn pinker_caminho_diretorio_atual() -> *mut u8 {
    let atual = std::env::current_dir()
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao obter diretório atual: {err}")));
    verso_alocar(&atual.to_string_lossy())
}
// @pinker-nav:end runtime.caminhos.sistema

// @pinker-nav:start runtime.tempo.relogio
// @pinker-nav:domain tempo
// @pinker-nav:layer runtime
// @pinker-nav:summary Tempo Unix (segundos desde a época, abortando via erro_fatal se o relógio do sistema estiver anterior à época) e formatação para ISO-8601 UTC usando o mesmo algoritmo civil (civil_de_dias, Howard Hinnant) do interpretador; não há suporte a fuso horário além de UTC.
#[no_mangle]
pub extern "C" fn pinker_tempo_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| erro_fatal("relógio do sistema anterior à época Unix"))
        .as_secs()
}

fn civil_de_dias(dias: i64) -> (i64, u64, u64) {
    // Algoritmo civil idêntico ao do interpretador (Howard Hinnant).
    let z = dias
        .checked_add(719_468)
        .unwrap_or_else(|| erro_fatal("timestamp inválido em 'formatar_tempo_unix'"));
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut ano = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = mp + if mp < 10 { 3 } else { -9 };
    if mes <= 2 {
        ano += 1;
    }
    (ano, mes as u64, dia as u64)
}

#[no_mangle]
pub extern "C" fn pinker_formatar_tempo_unix(timestamp: u64) -> *mut u8 {
    let dias = i64::try_from(timestamp / 86_400)
        .unwrap_or_else(|_| erro_fatal("timestamp inválido em 'formatar_tempo_unix'"));
    let segundos_do_dia = timestamp % 86_400;
    let (ano, mes, dia) = civil_de_dias(dias);
    let hora = segundos_do_dia / 3_600;
    let minuto = (segundos_do_dia % 3_600) / 60;
    let segundo = segundos_do_dia % 60;
    verso_alocar(&format!(
        "{ano:04}-{mes:02}-{dia:02}T{hora:02}:{minuto:02}:{segundo:02}Z"
    ))
}
// @pinker-nav:end runtime.tempo.relogio

// @pinker-nav:start runtime.aleatorio.gerador
// @pinker-nav:domain aleatorio
// @pinker-nav:layer runtime
// @pinker-nav:summary Geradores de números aleatórios mantidos em tabela global protegida por Mutex (handle -> estado), avançados por um LCG (constantes idênticas às do interpretador, para paridade de sementes); não é um gerador criptográfico; handle inválido ou min maior que max abortam via erro_fatal.
struct EstadoAcaso {
    geradores: HashMap<u64, u64>,
    proximo_handle: u64,
}

fn estado_acaso() -> &'static Mutex<EstadoAcaso> {
    static ACASO: OnceLock<Mutex<EstadoAcaso>> = OnceLock::new();
    ACASO.get_or_init(|| {
        Mutex::new(EstadoAcaso {
            geradores: HashMap::new(),
            proximo_handle: 1,
        })
    })
}

/// LCG idêntico ao do interpretador — paridade de sementes garantida.
fn avancar_gerador(estado: &mut u64) -> u64 {
    *estado = estado
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *estado
}

#[no_mangle]
pub extern "C" fn pinker_aleatorio_criar(semente: u64) -> u64 {
    let mut acaso = estado_acaso()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de acaso corrompido"));
    let handle = acaso.proximo_handle;
    acaso.proximo_handle = acaso.proximo_handle.saturating_add(1);
    acaso.geradores.insert(handle, semente);
    handle
}

fn com_gerador<R>(handle: u64, nome: &str, f: impl FnOnce(&mut u64) -> R) -> R {
    let mut acaso = estado_acaso()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de acaso corrompido"));
    let Some(estado) = acaso.geradores.get_mut(&handle) else {
        erro_fatal(&format!("gerador inválido em '{nome}'"));
    };
    f(estado)
}

#[no_mangle]
pub extern "C" fn pinker_aleatorio_proximo(handle: u64) -> u64 {
    com_gerador(handle, "aleatorio_proximo", avancar_gerador)
}

#[no_mangle]
pub extern "C" fn pinker_aleatorio_entre(handle: u64, min: u64, max: u64) -> u64 {
    if min > max {
        erro_fatal("intrínseca 'aleatorio_entre': min não pode ser maior que max");
    }
    com_gerador(handle, "aleatorio_entre", |estado| {
        let bruto = avancar_gerador(estado);
        let faixa = max - min + 1;
        if faixa == 0 {
            bruto
        } else {
            min + (bruto % faixa)
        }
    })
}
// @pinker-nav:end runtime.aleatorio.gerador

// ---------------------------------------------------------------------------
// Ambiente e processo nativos (Fase 221/B10)
//
// Os argumentos do programa vêm do `argc`/`argv` capturados por
// `pinker_rt_iniciar` (B1): `argv[0]` é o binário, então os "argumentos do
// programa" são `argv[1..]` — o equivalente nativo do `cli_args` do
// interpretador. A busca por chave nomeada replica `find_named_cli_argument`
// (`chave valor` ou `chave=valor`). Subprocessos usam `std::process` com as
// mesmas validações (comando não vazio, UTF-8 estrito, exit code exigido).
// ---------------------------------------------------------------------------

// @pinker-nav:start runtime.ambiente.argumentos
// @pinker-nav:domain ambiente
// @pinker-nav:layer runtime
// @pinker-nav:summary Leitura dos argumentos de linha de comando a partir do argc/argv global capturado em pinker_rt_iniciar (argv[0] descartado como nome do binário) e das variáveis de ambiente via std::env::var, incluindo busca por chave nomeada no formato `chave valor` ou `chave=valor`; argumento ausente ou chave vazia abortam via erro_fatal.
fn argumentos_do_programa() -> Vec<String> {
    let argc = pinker_rt_argc();
    let argv = pinker_rt_argv();
    if argv.is_null() || argc <= 1 {
        return Vec::new();
    }
    let mut argumentos = Vec::with_capacity((argc - 1) as usize);
    for i in 1..argc {
        unsafe {
            let ptr = *argv.add(i as usize);
            if ptr.is_null() {
                break;
            }
            let cstr = std::ffi::CStr::from_ptr(ptr as *const std::os::raw::c_char);
            argumentos.push(cstr.to_string_lossy().to_string());
        }
    }
    argumentos
}

/// Réplica de `find_named_cli_argument`: `chave valor` ou `chave=valor`;
/// devolve `Some(valor)` apenas quando há valor presente.
fn buscar_argumento_nomeado(argumentos: &[String], chave: &str) -> Option<String> {
    let chave_igual = format!("{chave}=");
    for (indice, argumento) in argumentos.iter().enumerate() {
        if argumento == chave {
            return argumentos.get(indice + 1).cloned();
        }
        if let Some(valor) = argumento.strip_prefix(&chave_igual) {
            return Some(valor.to_string());
        }
    }
    None
}

fn exigir_chave_nao_vazia(nome: &str, chave: &str) {
    if chave.is_empty() {
        erro_fatal(&format!("intrínseca '{nome}' exige chave não vazia"));
    }
}

#[no_mangle]
pub extern "C" fn pinker_ambiente_quantos_argumentos() -> u64 {
    argumentos_do_programa().len() as u64
}

#[no_mangle]
pub extern "C" fn pinker_ambiente_argumento(indice: u64) -> *mut u8 {
    let argumentos = argumentos_do_programa();
    let Some(argumento) = argumentos.get(indice as usize) else {
        erro_fatal("argumento ausente em 'argumento'");
    };
    verso_alocar(argumento)
}

/// # Safety
/// `padrao` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_argumento_ou(indice: u64, padrao: *const u8) -> *mut u8 {
    let argumentos = argumentos_do_programa();
    match argumentos.get(indice as usize) {
        Some(argumento) => verso_alocar(argumento),
        None => verso_alocar(verso_str(padrao)),
    }
}

#[no_mangle]
pub extern "C" fn pinker_ambiente_tem_argumento(indice: u64) -> u64 {
    u64::from(argumentos_do_programa().get(indice as usize).is_some())
}

/// # Safety
/// `chave` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_tem_chave(chave: *const u8) -> u64 {
    let chave = verso_str(chave);
    exigir_chave_nao_vazia("tem_chave", chave);
    u64::from(buscar_argumento_nomeado(&argumentos_do_programa(), chave).is_some())
}

/// # Safety
/// `chave` e `padrao` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_pedir_argumento(
    chave: *const u8,
    padrao: *const u8,
) -> *mut u8 {
    let chave = verso_str(chave);
    exigir_chave_nao_vazia("pedir_argumento", chave);
    match buscar_argumento_nomeado(&argumentos_do_programa(), chave) {
        Some(valor) => verso_alocar(&valor),
        None => verso_alocar(verso_str(padrao)),
    }
}

/// # Safety
/// `chave` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_tem_flag(chave: *const u8) -> u64 {
    let chave = verso_str(chave);
    exigir_chave_nao_vazia("tem_flag", chave);
    u64::from(
        argumentos_do_programa()
            .iter()
            .any(|argumento| argumento == chave),
    )
}

/// # Safety
/// `chave` e `padrao` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_ou(chave: *const u8, padrao: *const u8) -> *mut u8 {
    let chave = verso_str(chave);
    exigir_chave_nao_vazia("ambiente_ou", chave);
    match std::env::var(chave) {
        Ok(valor) => verso_alocar(&valor),
        Err(_) => verso_alocar(verso_str(padrao)),
    }
}

/// # Safety
/// `chave_arg`, `chave_env` e `padrao` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_ambiente_buscar_contexto(
    chave_arg: *const u8,
    chave_env: *const u8,
    padrao: *const u8,
) -> *mut u8 {
    let chave_arg = verso_str(chave_arg);
    let chave_env = verso_str(chave_env);
    exigir_chave_nao_vazia("buscar_contexto", chave_arg);
    exigir_chave_nao_vazia("buscar_contexto", chave_env);
    if let Some(valor) = buscar_argumento_nomeado(&argumentos_do_programa(), chave_arg) {
        return verso_alocar(&valor);
    }
    match std::env::var(chave_env) {
        Ok(valor) => verso_alocar(&valor),
        Err(_) => verso_alocar(verso_str(padrao)),
    }
}
// @pinker-nav:end runtime.ambiente.argumentos

// @pinker-nav:start runtime.processos.execucao
// @pinker-nav:domain processos
// @pinker-nav:layer runtime
// @pinker-nav:summary Execução de subprocessos sem shell implícito: as superfícies históricas mantêm resolução pela PATH fixa; a nova superfície estruturada recusa Ate(0) antes de configurar ou criar o filho, aplica PATH saneada e depois overlay antes da resolução no spawn para os demais limites, faz um único spawn e move stdin/stdout/stderr em poll não-bloqueante com quantum justo, deadline absoluto, kill+reap e UTF-8 estrito; todos os filhos recebem SIGPIPE default por pre_exec, enquanto os observáveis históricos permanecem inalterados.
const PATH_PROCESSOS: &str = "/usr/local/bin:/usr/bin:/bin";

/// Discriminantes da identidade runtime-reservada LimiteTempo. A ordem é ABI:
/// o compilador materializa SemLimite como 0 e Ate(bombom) como 1.
pub const PINKER_LIMITE_TEMPO_TAG_SEM_LIMITE: u64 = 0;
pub const PINKER_LIMITE_TEMPO_TAG_ATE: u64 = 1;

#[derive(Clone)]
struct SaidaProcessoNativa {
    codigo: u64,
    stdout: String,
    stderr: String,
}

#[cfg_attr(not(test), allow(dead_code))]
struct EstadoSaidasProcesso {
    tabela: std::collections::HashMap<u64, SaidaProcessoNativa>,
    proximo: Option<u64>,
}

impl Default for EstadoSaidasProcesso {
    fn default() -> Self {
        Self {
            tabela: std::collections::HashMap::new(),
            proximo: Some(1),
        }
    }
}

impl EstadoSaidasProcesso {
    fn inserir(&mut self, saida: SaidaProcessoNativa) -> u64 {
        let handle = self
            .proximo
            .unwrap_or_else(|| erro_fatal("handles de SaidaProcesso esgotados"));
        if self.tabela.contains_key(&handle) {
            erro_fatal("handle de SaidaProcesso seria reutilizado");
        }
        self.proximo = handle.checked_add(1);
        self.tabela.insert(handle, saida);
        handle
    }
}

fn estado_saidas_processo() -> &'static Mutex<EstadoSaidasProcesso> {
    static SAIDAS: OnceLock<Mutex<EstadoSaidasProcesso>> = OnceLock::new();
    SAIDAS.get_or_init(|| Mutex::new(EstadoSaidasProcesso::default()))
}

#[cfg_attr(not(test), allow(dead_code))]
fn registrar_saida_processo(saida: SaidaProcessoNativa) -> u64 {
    let mut estado = estado_saidas_processo()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de SaidaProcesso envenenado"));
    estado.inserir(saida)
}

fn com_saida_processo<R>(handle: u64, f: impl FnOnce(&SaidaProcessoNativa) -> R) -> R {
    let estado = estado_saidas_processo()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de SaidaProcesso envenenado"));
    let saida = estado
        .tabela
        .get(&handle)
        .unwrap_or_else(|| erro_fatal("handle SaidaProcesso inválido"));
    f(saida)
}

#[no_mangle]
pub extern "C" fn pinker_saida_processo_codigo(handle: u64) -> u64 {
    com_saida_processo(handle, |saida| saida.codigo)
}

#[no_mangle]
pub extern "C" fn pinker_saida_processo_stdout(handle: u64) -> *mut u8 {
    com_saida_processo(handle, |saida| verso_alocar(&saida.stdout))
}

#[no_mangle]
pub extern "C" fn pinker_saida_processo_stderr(handle: u64) -> *mut u8 {
    com_saida_processo(handle, |saida| verso_alocar(&saida.stderr))
}

const PROCESSO_ESTRUTURADO_TICK: std::time::Duration = std::time::Duration::from_millis(25);
const PROCESSO_ESTRUTURADO_QUANTUM_BYTES: usize = 64 * 1024;
const PROCESSO_ESTRUTURADO_QUANTUM_SYSCALLS: usize = 4;
const PROCESSO_ESTRUTURADO_BLOCO_BYTES: usize = 16 * 1024;

#[cfg(test)]
static PROCESSO_ESTRUTURADO_SPAWNS_TESTE: AtomicUsize = AtomicUsize::new(0);

struct ConfiguracaoProcessoEstruturadoNativa {
    programa: String,
    argumentos: Vec<String>,
    entrada: String,
    diretorio: String,
    ambiente: Vec<(String, String)>,
    limite: Option<std::time::Duration>,
}

unsafe fn ler_lista_verso_nativa(lista: *mut u8) -> Result<Vec<String>, String> {
    if lista.is_null() {
        return Err("lista de argumentos nula em 'executar_processo_estruturado'".to_string());
    }
    let quantidade = pinker_lista_tamanho(lista);
    let mut argumentos = Vec::with_capacity(quantidade as usize);
    for indice in 0..quantidade {
        let verso = pinker_lista_obter(lista, indice) as *const u8;
        if verso.is_null() {
            return Err(format!(
                "argumento {indice} nulo em 'executar_processo_estruturado'"
            ));
        }
        argumentos.push(verso_str(verso).to_string());
    }
    Ok(argumentos)
}

unsafe fn ler_mapa_verso_verso_nativo(mapa: *mut u8) -> Result<Vec<(String, String)>, String> {
    if mapa.is_null() {
        return Err("mapa de ambiente nulo em 'executar_processo_estruturado'".to_string());
    }
    let quantidade = mapa_len(mapa);
    let chaves = mapa_chaves(mapa);
    let valores = mapa_valores(mapa);
    let mut ambiente = Vec::with_capacity(quantidade as usize);
    for indice in 0..quantidade as usize {
        let chave = chaves.add(indice).read() as *const u8;
        let valor = valores.add(indice).read() as *const u8;
        if chave.is_null() || valor.is_null() {
            return Err("entrada nula no mapa de ambiente estruturado".to_string());
        }
        let chave = verso_str(chave);
        let valor = verso_str(valor);
        if chave.is_empty() {
            return Err("chave de ambiente vazia".to_string());
        }
        if chave.contains('=') {
            return Err(format!("chave de ambiente contém '=': {chave:?}"));
        }
        if chave.contains('\0') {
            return Err("chave de ambiente contém NUL".to_string());
        }
        if valor.contains('\0') {
            return Err(format!("valor de ambiente contém NUL para {chave:?}"));
        }
        ambiente.push((chave.to_string(), valor.to_string()));
    }
    Ok(ambiente)
}

unsafe fn ler_limite_tempo_nativo(leque: *mut u8) -> Result<Option<std::time::Duration>, String> {
    if leque.is_null() {
        return Err("LimiteTempo nulo em 'executar_processo_estruturado'".to_string());
    }
    match pinker_leque_tag(leque) {
        PINKER_LIMITE_TEMPO_TAG_SEM_LIMITE => Ok(None),
        PINKER_LIMITE_TEMPO_TAG_ATE => Ok(Some(std::time::Duration::from_millis(
            pinker_leque_carga(leque, PINKER_LIMITE_TEMPO_TAG_ATE, 0),
        ))),
        tag => Err(format!(
            "tag LimiteTempo inválida em 'executar_processo_estruturado': {tag}"
        )),
    }
}

unsafe fn ler_configuracao_processo_estruturado(
    programa: *const u8,
    argumentos: *mut u8,
    entrada: *const u8,
    diretorio: *const u8,
    ambiente: *mut u8,
    limite: *mut u8,
) -> Result<ConfiguracaoProcessoEstruturadoNativa, String> {
    if programa.is_null() || entrada.is_null() || diretorio.is_null() {
        return Err("verso nulo em 'executar_processo_estruturado'".to_string());
    }
    let programa = verso_str(programa);
    if programa.is_empty() {
        return Err("programa vazio em 'executar_processo_estruturado'".to_string());
    }
    Ok(ConfiguracaoProcessoEstruturadoNativa {
        programa: programa.to_string(),
        argumentos: ler_lista_verso_nativa(argumentos)?,
        entrada: verso_str(entrada).to_string(),
        diretorio: verso_str(diretorio).to_string(),
        ambiente: ler_mapa_verso_verso_nativo(ambiente)?,
        limite: ler_limite_tempo_nativo(limite)?,
    })
}

fn comando_processo_estruturado(programa: &str) -> Result<std::process::Command, String> {
    if programa.is_empty() {
        return Err("programa vazio em 'executar_processo_estruturado'".to_string());
    }
    Ok(comando_saneado(std::path::PathBuf::from(programa)))
}

fn executar_processo_estruturado_nativo(
    configuracao: ConfiguracaoProcessoEstruturadoNativa,
) -> Result<SaidaProcessoNativa, String> {
    if configuracao.limite == Some(std::time::Duration::ZERO) {
        return Err("limite de tempo excedido em 'executar_processo_estruturado'".to_string());
    }
    let deadline = configuracao
        .limite
        .map(|duracao| {
            std::time::Instant::now()
                .checked_add(duracao)
                .ok_or_else(|| "limite de tempo fora da faixa monotônica suportada".to_string())
        })
        .transpose()?;

    let mut comando = comando_processo_estruturado(&configuracao.programa)?;
    comando.args(&configuracao.argumentos);
    if !configuracao.diretorio.is_empty() {
        comando.current_dir(&configuracao.diretorio);
    }
    for (chave, valor) in &configuracao.ambiente {
        comando.env(chave, valor);
    }
    comando
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let filho = comando.spawn().map_err(|erro| {
        format!(
            "falha ao criar processo '{}' em 'executar_processo_estruturado': {erro}",
            configuracao.programa
        )
    })?;
    #[cfg(test)]
    PROCESSO_ESTRUTURADO_SPAWNS_TESTE.fetch_add(1, Ordering::SeqCst);
    let mut filho = FilhoEstruturadoNativo::novo(filho);
    let stdin = filho
        .take_stdin()
        .ok_or_else(|| "stdin configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_processo_estruturado(causa, &mut filho))?;
    let stdout = filho
        .take_stdout()
        .ok_or_else(|| "stdout configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_processo_estruturado(causa, &mut filho))?;
    let stderr = filho
        .take_stderr()
        .ok_or_else(|| "stderr configurado não foi disponibilizado".to_string())
        .or_else(|causa| falhar_processo_estruturado(causa, &mut filho))?;

    if let Err(erro) = configurar_pipe_estruturado_nao_bloqueante(&stdin)
        .and_then(|_| configurar_pipe_estruturado_nao_bloqueante(&stdout))
        .and_then(|_| configurar_pipe_estruturado_nao_bloqueante(&stderr))
    {
        return falhar_processo_estruturado(
            format!("falha ao configurar pipes não-bloqueantes: {erro}"),
            &mut filho,
        );
    }

    let entrada = configuracao.entrada.as_bytes();
    let mut entrada_enviada = 0usize;
    let mut stdin = if entrada.is_empty() {
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
        if deadline.is_some_and(|fim| std::time::Instant::now() >= fim) {
            return falhar_processo_estruturado(
                "limite de tempo excedido em 'executar_processo_estruturado'".to_string(),
                &mut filho,
            );
        }
        if let Err(erro) = filho.atualizar_status() {
            return falhar_processo_estruturado(
                format!("falha ao observar término do processo estruturado: {erro}"),
                &mut filho,
            );
        }
        if filho.status().is_some() && stdin.is_none() && stdout.is_none() && stderr.is_none() {
            break;
        }

        let mut descritores = Vec::with_capacity(3);
        if let Some(pipe) = stdin.as_ref() {
            descritores.push(DescritorPollNativo::novo(
                fd_estruturado(pipe),
                POLL_OUT_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO,
                CanalPollNativo::Stdin,
            ));
        }
        if let Some(pipe) = stdout.as_ref() {
            descritores.push(DescritorPollNativo::novo(
                fd_estruturado(pipe),
                POLL_IN_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO,
                CanalPollNativo::Stdout,
            ));
        }
        if let Some(pipe) = stderr.as_ref() {
            descritores.push(DescritorPollNativo::novo(
                fd_estruturado(pipe),
                POLL_IN_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO,
                CanalPollNativo::Stderr,
            ));
        }

        let timeout = timeout_poll_estruturado(deadline);
        match poll_descritores_nativos(&mut descritores, timeout) {
            Ok(ResultadoPollNativo::Eventos) => {}
            Ok(ResultadoPollNativo::Interrompido) => continue,
            Err(erro) => {
                return falhar_processo_estruturado(
                    format!("falha em poll dos pipes do processo estruturado: {erro}"),
                    &mut filho,
                )
            }
        }

        let mut fechar_stdin = false;
        let mut fechar_stdout = false;
        let mut fechar_stderr = false;
        for descritor in &descritores {
            if descritor.revents & POLL_INVALID_NATIVO != 0 {
                return falhar_processo_estruturado(
                    "poll encontrou descritor de pipe inválido".to_string(),
                    &mut filho,
                );
            }
            match descritor.canal {
                CanalPollNativo::Stdin
                    if descritor.revents
                        & (POLL_OUT_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO)
                        != 0 =>
                {
                    if let Some(pipe) = stdin.as_mut() {
                        match escrever_quantum_nativo(pipe, entrada, &mut entrada_enviada) {
                            Ok(true) => fechar_stdin = true,
                            Ok(false) => {}
                            Err(erro) => {
                                return falhar_processo_estruturado(
                                    format!("falha ao enviar stdin integralmente: {erro}"),
                                    &mut filho,
                                )
                            }
                        }
                    }
                }
                CanalPollNativo::Stdout
                    if descritor.revents & (POLL_IN_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO)
                        != 0 =>
                {
                    if let Some(pipe) = stdout.as_mut() {
                        match drenar_quantum_nativo(pipe, &mut bytes_stdout) {
                            Ok(eof) => fechar_stdout = eof,
                            Err(erro) => {
                                return falhar_processo_estruturado(
                                    format!("falha ao capturar stdout: {erro}"),
                                    &mut filho,
                                )
                            }
                        }
                    }
                }
                CanalPollNativo::Stderr
                    if descritor.revents & (POLL_IN_NATIVO | POLL_ERR_NATIVO | POLL_HUP_NATIVO)
                        != 0 =>
                {
                    if let Some(pipe) = stderr.as_mut() {
                        match drenar_quantum_nativo(pipe, &mut bytes_stderr) {
                            Ok(eof) => fechar_stderr = eof,
                            Err(erro) => {
                                return falhar_processo_estruturado(
                                    format!("falha ao capturar stderr: {erro}"),
                                    &mut filho,
                                )
                            }
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
        .expect("laço nativo só conclui depois de reapear o filho");
    let codigo = status.code().ok_or_else(|| {
        "processo estruturado terminou sem código normal; nenhum código mágico foi fabricado"
            .to_string()
    })?;
    let codigo = u64::try_from(codigo)
        .map_err(|_| "processo estruturado terminou com código negativo inválido".to_string())?;
    let stdout = String::from_utf8(bytes_stdout)
        .map_err(|_| "stdout do processo estruturado não é UTF-8 válido".to_string())?;
    let stderr = String::from_utf8(bytes_stderr)
        .map_err(|_| "stderr do processo estruturado não é UTF-8 válido".to_string())?;
    Ok(SaidaProcessoNativa {
        codigo,
        stdout,
        stderr,
    })
}

/// Executa a nova superfície estruturada na ABI nativa real.
///
/// # Safety
/// Os versos, a lista, o mapa e o leque devem usar as representações nativas
/// emitidas pelo backend Pinker para a assinatura canônica de seis argumentos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_executar_estruturado(
    programa: *const u8,
    argumentos: *mut u8,
    entrada: *const u8,
    diretorio: *const u8,
    ambiente: *mut u8,
    limite: *mut u8,
) -> *mut u8 {
    let configuracao = match ler_configuracao_processo_estruturado(
        programa, argumentos, entrada, diretorio, ambiente, limite,
    ) {
        Ok(configuracao) => configuracao,
        Err(causa) => return resultado_erro(&causa),
    };
    match executar_processo_estruturado_nativo(configuracao) {
        Ok(saida) => resultado_ok_bombom(registrar_saida_processo(saida)),
        Err(causa) => resultado_erro(&causa),
    }
}

fn escrever_quantum_nativo<W: Write>(
    pipe: &mut W,
    entrada: &[u8],
    enviados: &mut usize,
) -> io::Result<bool> {
    let inicio = *enviados;
    let mut syscalls = 0usize;
    while *enviados < entrada.len()
        && *enviados - inicio < PROCESSO_ESTRUTURADO_QUANTUM_BYTES
        && syscalls < PROCESSO_ESTRUTURADO_QUANTUM_SYSCALLS
    {
        let restantes = PROCESSO_ESTRUTURADO_QUANTUM_BYTES - (*enviados - inicio);
        let fim = (*enviados + restantes.min(PROCESSO_ESTRUTURADO_BLOCO_BYTES)).min(entrada.len());
        match pipe.write(&entrada[*enviados..fim]) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
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

fn drenar_quantum_nativo<R: Read>(pipe: &mut R, destino: &mut Vec<u8>) -> io::Result<bool> {
    let mut bloco = [0u8; PROCESSO_ESTRUTURADO_BLOCO_BYTES];
    let mut bytes = 0usize;
    let mut syscalls = 0usize;
    while bytes < PROCESSO_ESTRUTURADO_QUANTUM_BYTES
        && syscalls < PROCESSO_ESTRUTURADO_QUANTUM_SYSCALLS
    {
        let limite = (PROCESSO_ESTRUTURADO_QUANTUM_BYTES - bytes).min(bloco.len());
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

fn timeout_poll_estruturado(deadline: Option<std::time::Instant>) -> i32 {
    let espera = match deadline {
        Some(fim) => fim
            .saturating_duration_since(std::time::Instant::now())
            .min(PROCESSO_ESTRUTURADO_TICK),
        None => PROCESSO_ESTRUTURADO_TICK,
    };
    espera.as_millis().max(1).min(i32::MAX as u128) as i32
}

fn falhar_processo_estruturado<T>(
    causa: String,
    filho: &mut FilhoEstruturadoNativo,
) -> Result<T, String> {
    compor_falha_cleanup_nativo(causa, filho.encerrar_e_reapear())
}

fn compor_falha_cleanup_nativo<T>(
    causa: String,
    limpeza: Result<(), FalhasCleanupNativo>,
) -> Result<T, String> {
    match limpeza {
        Ok(()) => Err(causa),
        Err(limpeza) => Err(format!("{causa}; cleanup do filho direto: {limpeza}")),
    }
}

trait OperacoesCleanupNativo {
    type Status;

    fn observar_status(&mut self) -> io::Result<Option<Self::Status>>;
    fn encerrar(&mut self) -> io::Result<()>;
    fn esperar(&mut self) -> io::Result<Self::Status>;
}

impl OperacoesCleanupNativo for std::process::Child {
    type Status = std::process::ExitStatus;

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
struct FalhasCleanupNativo {
    observacao: Option<io::Error>,
    encerramento: Option<io::Error>,
    espera: Option<io::Error>,
}

impl FalhasCleanupNativo {
    fn vazia(&self) -> bool {
        self.observacao.is_none() && self.encerramento.is_none() && self.espera.is_none()
    }
}

impl std::fmt::Display for FalhasCleanupNativo {
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

fn encerrar_e_reapear_nativo_com<O: OperacoesCleanupNativo>(
    filho: &mut O,
    status: &mut Option<O::Status>,
) -> Result<(), FalhasCleanupNativo> {
    if status.is_some() {
        return Ok(());
    }

    let mut falhas = FalhasCleanupNativo::default();
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

struct FilhoEstruturadoNativo {
    filho: std::process::Child,
    status: Option<std::process::ExitStatus>,
}

impl FilhoEstruturadoNativo {
    fn novo(filho: std::process::Child) -> Self {
        Self {
            filho,
            status: None,
        }
    }

    fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        self.filho.stdin.take()
    }

    fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.filho.stdout.take()
    }

    fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.filho.stderr.take()
    }

    fn atualizar_status(&mut self) -> io::Result<()> {
        if self.status.is_none() {
            self.status = self.filho.try_wait()?;
        }
        Ok(())
    }

    fn status(&self) -> Option<std::process::ExitStatus> {
        self.status
    }

    fn encerrar_e_reapear(&mut self) -> Result<(), FalhasCleanupNativo> {
        encerrar_e_reapear_nativo_com(&mut self.filho, &mut self.status)
    }
}

impl Drop for FilhoEstruturadoNativo {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.filho.kill();
            let _ = self.filho.wait();
        }
    }
}

#[repr(C)]
struct PollFdNativo {
    fd: i32,
    events: i16,
    revents: i16,
}

#[derive(Clone, Copy)]
enum CanalPollNativo {
    Stdin,
    Stdout,
    Stderr,
}

struct DescritorPollNativo {
    raw: PollFdNativo,
    canal: CanalPollNativo,
    revents: i16,
}

impl DescritorPollNativo {
    fn novo(fd: i32, events: i16, canal: CanalPollNativo) -> Self {
        Self {
            raw: PollFdNativo {
                fd,
                events,
                revents: 0,
            },
            canal,
            revents: 0,
        }
    }
}

const POLL_IN_NATIVO: i16 = 0x0001;
const POLL_OUT_NATIVO: i16 = 0x0004;
const POLL_ERR_NATIVO: i16 = 0x0008;
const POLL_HUP_NATIVO: i16 = 0x0010;
const POLL_INVALID_NATIVO: i16 = 0x0020;
const F_GETFL_NATIVO: i32 = 3;
const F_SETFL_NATIVO: i32 = 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NONBLOCK_NATIVO: i32 = 0o4000;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const O_NONBLOCK_NATIVO: i32 = 0x0004;

#[cfg(unix)]
extern "C" {
    #[link_name = "poll"]
    fn poll_nativo(fds: *mut PollFdNativo, quantidade: usize, timeout_ms: i32) -> i32;
    #[link_name = "fcntl"]
    fn fcntl_nativo(fd: i32, comando: i32, ...) -> i32;
}

#[cfg(unix)]
fn fd_estruturado<T: std::os::fd::AsRawFd>(pipe: &T) -> i32 {
    pipe.as_raw_fd()
}

#[cfg(not(unix))]
fn fd_estruturado<T>(_pipe: &T) -> i32 {
    -1
}

#[cfg(unix)]
fn configurar_pipe_estruturado_nao_bloqueante<T: std::os::fd::AsRawFd>(pipe: &T) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { fcntl_nativo(fd, F_GETFL_NATIVO) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { fcntl_nativo(fd, F_SETFL_NATIVO, flags | O_NONBLOCK_NATIVO) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn configurar_pipe_estruturado_nao_bloqueante<T>(_pipe: &T) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "poll não-bloqueante requer plataforma Unix",
    ))
}

enum ResultadoPollNativo {
    Eventos,
    Interrompido,
}

#[cfg(unix)]
fn poll_descritores_nativos(
    descritores: &mut [DescritorPollNativo],
    timeout: i32,
) -> io::Result<ResultadoPollNativo> {
    let mut raws: Vec<PollFdNativo> = descritores
        .iter()
        .map(|descritor| PollFdNativo {
            fd: descritor.raw.fd,
            events: descritor.raw.events,
            revents: 0,
        })
        .collect();
    let retorno = unsafe { poll_nativo(raws.as_mut_ptr(), raws.len(), timeout) };
    if retorno < 0 {
        let erro = io::Error::last_os_error();
        return if erro.kind() == io::ErrorKind::Interrupted {
            Ok(ResultadoPollNativo::Interrompido)
        } else {
            Err(erro)
        };
    }
    for (destino, origem) in descritores.iter_mut().zip(raws.iter()) {
        destino.revents = origem.revents;
    }
    Ok(ResultadoPollNativo::Eventos)
}

#[cfg(not(unix))]
fn poll_descritores_nativos(
    _descritores: &mut [DescritorPollNativo],
    _timeout: i32,
) -> io::Result<ResultadoPollNativo> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "poll requer plataforma Unix",
    ))
}

/// Constrói o `Command` comum a todas as famílias de processo, com a PATH
/// saneada.
///
/// A disposição de `SIGPIPE` do pai **não** é (re)estabelecida aqui de
/// propósito: ela pertence a `pinker_rt_iniciar`, que roda antes de qualquer
/// instrução do programa. Repetir a preparação neste ponto tornaria a correção
/// de R5 imune a mutação — a matriz continuaria passando mesmo com a
/// inicialização desfeita — e esconderia a regressão que ela existe para pegar.
///
/// Sobre a herança da disposição pelos filhos: `SIG_IGN` sobrevive a `exec`, o
/// que faria um programa externo disparado pela Pinker herdar uma disposição
/// não padrão. Por isso este construtor instala um `pre_exec` que devolve
/// `SIGPIPE` a `SIG_DFL` no filho, imediatamente antes do `exec`.
///
/// A configuração é explícita e pertence ao runtime da Pinker. Ela **não**
/// delega o contrato à biblioteca padrão: `std::process::Command` hoje também
/// restaura `SIGPIPE`, mas por caminhos internos distintos (`fork`/`exec` e
/// `posix_spawn`) e sob condições que a std pode mudar. O contrato observável
/// da Pinker não pode depender dessa escolha interna, da libc nem do runner.
///
/// Efeito colateral aceito e desejado: instalar uma closure de `pre_exec`
/// remove este `Command` do caminho de `posix_spawn` da std, tornando a
/// preparação do filho determinística em vez de condicional.
///
/// A closure respeita o contrato de [`CommandExt::pre_exec`]: roda no filho
/// depois do `fork`, faz uma única chamada async-signal-safe a `signal(2)` via
/// [`restaurar_disposicao_padrao`] e não aloca, não formata mensagem, não
/// acessa ambiente e não adquire lock. Falha de preparação vira `io::Error` e
/// chega ao pai como erro de criação do processo.
fn comando_saneado(resolvido: std::path::PathBuf) -> std::process::Command {
    let mut processo = std::process::Command::new(resolvido);
    processo.env("PATH", PATH_PROCESSOS);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: a closure roda no filho entre `fork` e `exec`. Ela executa
        // apenas `restaurar_disposicao_padrao`, cujo corpo é uma chamada a
        // `signal(2)` — async-signal-safe pela POSIX — sem alocação,
        // formatação, acesso a ambiente, lock ou código de usuário.
        unsafe {
            processo.pre_exec(|| restaurar_disposicao_padrao(SINAL_SIGPIPE));
        }
    }
    processo
}

fn exigir_comando_nao_vazio(nome: &str, comando: &str) {
    if comando.trim().is_empty() {
        erro_fatal(&format!("intrínseca '{nome}' exige comando não vazio"));
    }
}

fn comando_resolvido(nome: &str, comando: &str) -> Result<std::path::PathBuf, String> {
    if comando.contains('/') {
        return Ok(std::path::PathBuf::from(comando));
    }
    for diretorio in PATH_PROCESSOS.split(':') {
        let candidato = std::path::Path::new(diretorio).join(comando);
        let Ok(metadata) = std::fs::metadata(&candidato) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if metadata.permissions().mode() & 0o111 == 0 {
                continue;
            }
        }
        return Ok(candidato);
    }
    Err(format!(
        "comando '{comando}' não encontrado na PATH saneada em '{nome}'"
    ))
}

fn novo_processo(nome: &str, comando: &str) -> std::process::Command {
    exigir_comando_nao_vazio(nome, comando);
    let resolvido = comando_resolvido(nome, comando).unwrap_or_else(|err| erro_fatal(err.as_str()));
    comando_saneado(resolvido)
}

fn exit_code_ou_erro(nome: &str, codigo: Option<i32>) -> u64 {
    let Some(codigo) = codigo else {
        erro_fatal(&format!(
            "processo finalizado sem código de saída suportado em '{nome}'"
        ));
    };
    u64::try_from(codigo).unwrap_or_else(|_| {
        erro_fatal(&format!(
            "código de saída inválido em '{nome}': valor negativo"
        ))
    })
}

fn processo_executar(comando: &str, argv1: Option<&str>) -> u64 {
    let mut processo = novo_processo("executar_processo", comando);
    if let Some(argumento) = argv1 {
        processo.arg(argumento);
    }
    let status = processo.status().unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao executar processo em 'executar_processo': {err}"
        ))
    });
    exit_code_ou_erro("executar_processo", status.code())
}

/// # Safety
/// `comando` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_executar_1(comando: *const u8) -> u64 {
    processo_executar(verso_str(comando), None)
}

/// # Safety
/// `comando` e `argv1` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_executar_2(comando: *const u8, argv1: *const u8) -> u64 {
    processo_executar(verso_str(comando), Some(verso_str(argv1)))
}

fn processo_capturar(nome: &str, comando: &str, argv1: Option<&str>, stderr: bool) -> *mut u8 {
    let mut processo = novo_processo(nome, comando);
    if let Some(argumento) = argv1 {
        processo.arg(argumento);
    }
    let saida = processo.output().unwrap_or_else(|err| {
        erro_fatal(&format!("falha ao executar processo em '{nome}': {err}"))
    });
    let bytes = if stderr { saida.stderr } else { saida.stdout };
    match String::from_utf8(bytes) {
        Ok(texto) => verso_alocar(&texto),
        Err(_) => erro_fatal(&format!(
            "{} inválido em '{nome}': UTF-8 estrito é obrigatório",
            if stderr { "stderr" } else { "stdout" }
        )),
    }
}

/// # Safety
/// `comando` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_capturar_stdout_1(comando: *const u8) -> *mut u8 {
    processo_capturar("capturar_stdout", verso_str(comando), None, false)
}

/// # Safety
/// `comando` e `argv1` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_capturar_stdout_2(
    comando: *const u8,
    argv1: *const u8,
) -> *mut u8 {
    processo_capturar(
        "capturar_stdout",
        verso_str(comando),
        Some(verso_str(argv1)),
        false,
    )
}

/// # Safety
/// `comando` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_capturar_stderr_1(comando: *const u8) -> *mut u8 {
    processo_capturar("capturar_stderr", verso_str(comando), None, true)
}

/// # Safety
/// `comando` e `argv1` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_capturar_stderr_2(
    comando: *const u8,
    argv1: *const u8,
) -> *mut u8 {
    processo_capturar(
        "capturar_stderr",
        verso_str(comando),
        Some(verso_str(argv1)),
        true,
    )
}

fn processo_com_entrada_resultado(
    comando: &str,
    entrada: &str,
    argv1: Option<&str>,
) -> Result<u64, String> {
    if comando.trim().is_empty() {
        return Err("intrínseca 'executar_com_entrada' exige comando não vazio".to_string());
    }
    let resolvido = comando_resolvido("executar_com_entrada", comando)?;
    let mut processo = comando_saneado(resolvido);
    if let Some(argumento) = argv1 {
        processo.arg(argumento);
    }
    let mut filho = processo
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("falha ao executar processo em 'executar_com_entrada': {err}"))?;
    let Some(mut stdin) = filho.stdin.take() else {
        return Err(
            "stdin indisponível em 'executar_com_entrada': processo sem pipe configurado"
                .to_string(),
        );
    };
    let bytes = entrada.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        use std::io::Write as _;
        stdin.write_all(&bytes)
    });
    let wait_result = filho.wait();
    let write_result = writer
        .join()
        .map_err(|_| "thread de stdin falhou em 'executar_com_entrada'".to_string())?;
    write_result
        .map_err(|err| format!("falha ao escrever stdin em 'executar_com_entrada': {err}"))?;
    let status = wait_result
        .map_err(|err| format!("falha ao aguardar processo em 'executar_com_entrada': {err}"))?;
    let codigo = status.code().ok_or_else(|| {
        "processo finalizado sem código de saída suportado em 'executar_com_entrada'".to_string()
    })?;
    u64::try_from(codigo).map_err(|_| {
        "código de saída inválido em 'executar_com_entrada': valor negativo".to_string()
    })
}

fn processo_com_entrada(comando: &str, entrada: &str, argv1: Option<&str>) -> u64 {
    processo_com_entrada_resultado(comando, entrada, argv1)
        .unwrap_or_else(|err| erro_fatal(err.as_str()))
}

/// # Safety
/// `comando` e `entrada` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_com_entrada_2(
    comando: *const u8,
    entrada: *const u8,
) -> u64 {
    processo_com_entrada(verso_str(comando), verso_str(entrada), None)
}

/// # Safety
/// `comando`, `entrada` e `argv1` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_com_entrada_3(
    comando: *const u8,
    entrada: *const u8,
    argv1: *const u8,
) -> u64 {
    processo_com_entrada(
        verso_str(comando),
        verso_str(entrada),
        Some(verso_str(argv1)),
    )
}

/// # Safety
/// `produtor` e `consumidor` devem apontar para blocos de verso válidos.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_pipeline(
    produtor: *const u8,
    consumidor: *const u8,
) -> u64 {
    let produtor_nome = verso_str(produtor);
    let consumidor_nome = verso_str(consumidor);
    let mut produtor_comando = novo_processo("pipeline_minimo", produtor_nome);
    produtor_comando.stdout(std::process::Stdio::piped());
    let mut produtor = produtor_comando.spawn().unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao executar processo produtor em 'pipeline_minimo': {err}"
        ))
    });
    let Some(saida_produtor) = produtor.stdout.take() else {
        erro_fatal("stdout indisponível em 'pipeline_minimo': produtor sem pipe configurado");
    };
    let mut consumidor_comando = novo_processo("pipeline_minimo", consumidor_nome);
    consumidor_comando.stdin(std::process::Stdio::from(saida_produtor));
    let mut consumidor = consumidor_comando.spawn().unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao executar processo consumidor em 'pipeline_minimo': {err}"
        ))
    });
    produtor.wait().unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao aguardar produtor em 'pipeline_minimo': {err}"
        ))
    });
    let status = consumidor.wait().unwrap_or_else(|err| {
        erro_fatal(&format!(
            "falha ao aguardar consumidor em 'pipeline_minimo': {err}"
        ))
    });
    exit_code_ou_erro("pipeline_minimo", status.code())
}
// @pinker-nav:end runtime.processos.execucao
// @pinker-nav:start runtime.falha-operacional.superficies
// @pinker-nav:domain erros
// @pinker-nav:layer runtime
// @pinker-nav:summary Superfícies falíveis nativas da Parte B: leitura de arquivo por caminho, spawn de processo e conversão de texto para número devolvem um leque `Resultado<T,E>` construído pelos mesmos `pinker_leque_criar_0`/`pinker_leque_anexar` que o código gerado usa, com `Ok` na tag 0 e `Erro` na tag 1 e a causa sempre em `verso`. Falha ambiental vira valor; comando vazio, código de saída não representável e falta de memória continuam fatais por `erro_fatal`.

/// Tag da variante de sucesso de `Resultado<T,E>`.
///
/// Espelha `falha_operacional::TAG_OK` do compilador. O runtime é uma crate
/// separada e não pode importar aquele símbolo, então o acoplamento é fixado
/// por evidência: a paridade interpretador × nativo quebra imediatamente se as
/// duas pontas discordarem da ordem das variantes.
const RESULTADO_TAG_OK: u64 = 0;

/// Tag da variante de falha de `Resultado<T,E>`.
const RESULTADO_TAG_ERRO: u64 = 1;

/// `Resultado.Ok(valor)` com carga de uma palavra.
fn resultado_ok_bombom(valor: u64) -> *mut u8 {
    let leque = pinker_leque_criar_0(RESULTADO_TAG_OK);
    unsafe { pinker_leque_anexar(leque, valor) }
}

/// `Resultado.Ok(texto)` com carga textual.
fn resultado_ok_verso(texto: &str) -> *mut u8 {
    let leque = pinker_leque_criar_0(RESULTADO_TAG_OK);
    unsafe { pinker_leque_anexar(leque, verso_alocar(texto) as u64) }
}

/// `Resultado.Erro(causa)`. A causa é sempre `verso`.
fn resultado_erro(causa: &str) -> *mut u8 {
    let leque = pinker_leque_criar_0(RESULTADO_TAG_ERRO);
    unsafe { pinker_leque_anexar(leque, verso_alocar(causa) as u64) }
}

/// Leitura de arquivo inteiro por caminho, com falha ambiental como valor.
///
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_arquivo_ler_caminho_resultado(caminho: *const u8) -> *mut u8 {
    let caminho = verso_str(caminho);
    match std::fs::read_to_string(caminho) {
        Ok(conteudo) => resultado_ok_verso(&conteudo),
        Err(err) => resultado_erro(&format!("falha ao ler arquivo '{caminho}': {err}")),
    }
}

/// Spawn de processo, com impossibilidade de executar como valor.
///
/// O código de saída do filho é valor de sucesso — um código diferente de zero
/// não é falha desta superfície. Continuam fatais: comando vazio (erro de uso) e
/// término sem código representável, cuja modelagem pertence à Parte D.
///
/// # Safety
/// `comando` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_processo_executar_resultado(comando: *const u8) -> *mut u8 {
    let nome = "executar_processo_resultado";
    let comando = verso_str(comando);
    exigir_comando_nao_vazio(nome, comando);
    let resolvido = match comando_resolvido(nome, comando) {
        Ok(resolvido) => resolvido,
        Err(err) => return resultado_erro(&err),
    };
    match comando_saneado(resolvido).status() {
        Ok(status) => resultado_ok_bombom(exit_code_ou_erro(nome, status.code())),
        Err(err) => resultado_erro(&format!("falha ao executar processo '{comando}': {err}")),
    }
}

/// Conversão de texto externo para número, com texto malformado como valor.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_verso_para_bombom_resultado(texto: *const u8) -> *mut u8 {
    let texto = verso_str(texto);
    match texto.trim().parse::<u64>() {
        Ok(valor) => resultado_ok_bombom(valor),
        Err(_) => resultado_erro(&format!("falha ao converter '{texto}' para bombom")),
    }
}
// @pinker-nav:end runtime.falha-operacional.superficies

// @pinker-nav:start runtime.sha256.superficies
// @pinker-nav:domain integridade
// @pinker-nav:layer runtime
// @pinker-nav:summary Superfícies SHA-256 da Parte E2 no runtime nativo: `pinker_sha256_verso` hasheia os bytes UTF-8 exatos de um verso pelo layout length-prefixed, sem percorrer codepoints e sem normalizar, e `pinker_sha256_arquivo_resultado` abre o caminho, lê em blocos de 64 KiB e alimenta o mesmo acumulador incremental, devolvendo `Resultado<verso,verso>` com o digest canônico de 64 caracteres. Ambas delegam o núcleo a `pinker_sha256_contract`, o mesmo crate puro consumido pelo interpretador, de modo que a paridade do digest é por construção e não por duas implementações que concordam por acaso; a leitura é em bytes de propósito, porque `read_to_string` rejeitaria arquivo binário, e o handle e o buffer morrem dentro da própria chamada.

/// SHA-256 dos bytes UTF-8 exatos de um `verso`.
///
/// Contrato: `SHA256(verso) = SHA256(UTF8_BYTES(verso))`. `verso_bytes` devolve
/// exatamente os bytes do bloco length-prefixed — nada de percorrer codepoints,
/// nada de `pinker_verso_tamanho` (que conta caracteres, não bytes), nada de
/// normalização Unicode.
///
/// Dado já em memória não pode falhar, então a superfície é pura: devolve o
/// `verso` do digest, não um `Resultado`.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_sha256_verso(texto: *const u8) -> *mut u8 {
    let digest = pinker_sha256_contract::sha256_hex(verso_bytes(texto));
    verso_alocar(&digest)
}

/// SHA-256 dos bytes **exatos** de um arquivo, por caminho.
///
/// Lê em bytes, deliberadamente sem `read_to_string`: UTF-8 inválido é conteúdo
/// legítimo de arquivo, e nenhum byte pode ser validado, normalizado ou
/// substituído no caminho do hash. Newline não é tocado.
///
/// Symlink é **seguido**, porque isto é `open`/`read` e reutiliza a política
/// vigente dessa família — distinto do no-follow de `pinker_entrada_tipo_*`.
///
/// Diretório, arquivo ausente e permissão negada falham no próprio SO e viram
/// `Erro` recuperável. O `File` e o buffer são locais: saem de escopo por drop
/// no sucesso e no erro, então nenhuma identidade pública de recurso nasce aqui.
///
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_sha256_arquivo_resultado(caminho: *const u8) -> *mut u8 {
    use std::io::Read;

    let caminho = verso_str(caminho);
    let mut arquivo = match std::fs::File::open(caminho) {
        Ok(arquivo) => arquivo,
        Err(err) => return resultado_erro(&format!("falha ao hashear arquivo '{caminho}': {err}")),
    };
    let mut acumulador = pinker_sha256_contract::Sha256::novo();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        match arquivo.read(&mut buffer) {
            Ok(0) => break,
            Ok(lidos) => acumulador.atualizar(&buffer[..lidos]),
            Err(err) => {
                return resultado_erro(&format!("falha ao hashear arquivo '{caminho}': {err}"))
            }
        }
    }
    resultado_ok_verso(&acumulador.finalizar_hex())
}
// @pinker-nav:end runtime.sha256.superficies

// @pinker-nav:start runtime.filesystem.enumeracao-adulta
// @pinker-nav:domain filesystem
// @pinker-nav:layer runtime
// @pinker-nav:summary Superfícies de filesystem adulto da Parte C no runtime nativo: `pinker_diretorio_listar_resultado` recusa argumento symlink e path que não é diretório por `symlink_metadata`, coleta as entradas imediatas por `read_dir`, falha inteira diante de nome não representável como verso e ordena pelos bytes UTF-8 antes de montar `lista<verso>` pelos mesmos `pinker_lista_criar`/`pinker_lista_anexar` do código gerado; `pinker_entrada_tipo_resultado` devolve o discriminante de `TipoEntrada` classificado sem seguir link, e `pinker_entrada_tamanho_resultado` devolve o tamanho também sem seguir. Os discriminantes espelham `tipo_entrada::VARIANTES` do compilador e a paridade é fixada por evidência.

/// Discriminantes de `TipoEntrada`, espelhando a ordem de declaração fixada por
/// `tipo_entrada::VARIANTES` no compilador.
///
/// O runtime é uma crate separada e não pode importar aquela autoridade; como
/// nas tags de `Resultado`, o acoplamento é explícito e cobrado por evidência
/// de paridade.
const TIPO_ENTRADA_ARQUIVO: u64 = 0;
const TIPO_ENTRADA_DIRETORIO: u64 = 1;
const TIPO_ENTRADA_SYMLINK: u64 = 2;
const TIPO_ENTRADA_OUTRO: u64 = 3;

/// Classifica um `FileType` obtido por `symlink_metadata`.
///
/// `is_symlink` primeiro por contrato: a entrada decide sua classe, nunca o
/// alvo. Symlink quebrado continua symlink.
fn tipo_entrada_discriminante(tipo: std::fs::FileType) -> u64 {
    if tipo.is_symlink() {
        TIPO_ENTRADA_SYMLINK
    } else if tipo.is_file() {
        TIPO_ENTRADA_ARQUIVO
    } else if tipo.is_dir() {
        TIPO_ENTRADA_DIRETORIO
    } else {
        TIPO_ENTRADA_OUTRO
    }
}

/// Enumeração determinística das entradas imediatas, ou a causa da falha.
fn enumerar_diretorio(caminho: &str) -> Result<Vec<String>, String> {
    let meta = match std::fs::symlink_metadata(caminho) {
        Ok(meta) => meta,
        Err(err) => return Err(format!("falha ao listar diretório '{caminho}': {err}")),
    };
    if meta.file_type().is_symlink() {
        return Err(format!(
            "falha ao listar diretório '{caminho}': o caminho é um link simbólico e \
             não é seguido por padrão"
        ));
    }
    if !meta.is_dir() {
        return Err(format!(
            "falha ao listar diretório '{caminho}': o caminho não é um diretório"
        ));
    }
    let entradas = match std::fs::read_dir(caminho) {
        Ok(entradas) => entradas,
        Err(err) => return Err(format!("falha ao listar diretório '{caminho}': {err}")),
    };
    let mut nomes = Vec::new();
    for entrada in entradas {
        let entrada = match entrada {
            Ok(entrada) => entrada,
            Err(err) => return Err(format!("falha ao listar diretório '{caminho}': {err}")),
        };
        let bruto = entrada.file_name();
        match bruto.to_str() {
            Some(nome) => nomes.push(nome.to_string()),
            None => {
                // Forma escapada de `OsStr` (`\xFF`), nunca `to_string_lossy`:
                // U+FFFD colocaria uma versão lossy do nome num valor
                // observável. Espelha o interpretador byte a byte.
                return Err(format!(
                    "falha ao listar diretório '{caminho}': a entrada {bruto:?} não é \
                     representável como verso (UTF-8 inválido)"
                ));
            }
        }
    }
    nomes.sort_unstable();
    Ok(nomes)
}

/// `Resultado.Ok(lista<verso>)` — Parte C.
fn resultado_ok_lista_verso(nomes: &[String]) -> *mut u8 {
    let lista = pinker_lista_criar();
    if lista.is_null() {
        erro_fatal("sem memória ao criar lista de entradas de diretório");
    }
    for nome in nomes {
        unsafe { pinker_lista_anexar(lista, verso_alocar(nome) as u64) };
    }
    let leque = pinker_leque_criar_0(RESULTADO_TAG_OK);
    unsafe { pinker_leque_anexar(leque, lista as u64) }
}

/// Enumeração determinística das entradas imediatas de um diretório.
///
/// Devolve os NOMES das entradas, ordenados pelos bytes UTF-8, sem `.` nem
/// `..`, incluindo ocultos e qualquer tipo representável. Diretório vazio é
/// sucesso com lista vazia. Falha ambiental é valor; o argumento symlink é
/// recusado em vez de seguido.
///
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_diretorio_listar_resultado(caminho: *const u8) -> *mut u8 {
    let caminho = verso_str(caminho);
    match enumerar_diretorio(caminho) {
        Ok(nomes) => resultado_ok_lista_verso(&nomes),
        Err(causa) => resultado_erro(&causa),
    }
}

/// Classificação no-follow de uma entrada.
///
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_entrada_tipo_resultado(caminho: *const u8) -> *mut u8 {
    let caminho = verso_str(caminho);
    match std::fs::symlink_metadata(caminho) {
        Ok(meta) => resultado_ok_bombom(tipo_entrada_discriminante(meta.file_type())),
        Err(err) => resultado_erro(&format!("falha ao classificar entrada '{caminho}': {err}")),
    }
}

/// Tamanho em bytes de uma entrada, sem seguir link.
///
/// # Safety
/// `caminho` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_entrada_tamanho_resultado(caminho: *const u8) -> *mut u8 {
    let caminho = verso_str(caminho);
    match std::fs::symlink_metadata(caminho) {
        Ok(meta) => resultado_ok_bombom(meta.len()),
        Err(err) => resultado_erro(&format!("falha ao medir entrada '{caminho}': {err}")),
    }
}
// @pinker-nav:end runtime.filesystem.enumeracao-adulta

// @pinker-nav:start runtime.json.valor-adulto
// @pinker-nav:domain dados
// @pinker-nav:layer runtime
// @pinker-nav:summary Superfície JSON adulta da Parte E1 no runtime nativo: a árvore vive numa tabela global de handles monotônicos que nunca são reutilizados, e a gramática NÃO é reimplementada aqui — interpretação, domínio numérico, escapes, política de chave duplicada e ordem de serialização vêm de `pinker_json_contract`, o mesmo crate que o interpretador usa. É isso que torna a paridade uma propriedade de construção em vez de uma promessa: não existem duas gramáticas para divergir. `pinker_json_ler_resultado` devolve `Resultado` pelos mesmos `pinker_leque_criar_0`/`pinker_leque_anexar` do código gerado, e os acessores atravessam o nesting pelo mesmo handle, sem helper por formato.
use pinker_json_contract::{NoJson, TabelaJson};

#[derive(Default)]
struct EstadoValoresJson {
    tabela: TabelaJson,
}

fn estado_valores_json() -> &'static Mutex<EstadoValoresJson> {
    static VALORES: OnceLock<Mutex<EstadoValoresJson>> = OnceLock::new();
    VALORES.get_or_init(|| Mutex::new(EstadoValoresJson::default()))
}

fn com_valores_json<R>(f: impl FnOnce(&mut EstadoValoresJson) -> R) -> R {
    let mut estado = estado_valores_json()
        .lock()
        .unwrap_or_else(|_| erro_fatal("estado de ValorJson envenenado"));
    f(&mut estado)
}

/// Lê um nó já materializado, abortando em handle não produzido.
///
/// Handle inválido é violação de invariante interna, não dado externo
/// malformado: o valor só existe se a árvore foi aceita antes.
fn com_no_json<R>(handle: u64, f: impl FnOnce(&NoJson, &TabelaJson) -> R) -> R {
    com_valores_json(|estado| {
        let no = estado
            .tabela
            .obter(handle)
            .unwrap_or_else(|| erro_fatal("handle ValorJson inválido"))
            .clone();
        f(&no, &estado.tabela)
    })
}

fn erro_tipo_json(nome: &str, esperado: &str) -> ! {
    erro_fatal(&format!(
        "intrínseca '{nome}' exige valor JSON do tipo {esperado}"
    ))
}

/// Interpreta texto JSON externo, com dado malformado como valor.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_json_ler_resultado(texto: *const u8) -> *mut u8 {
    let texto = verso_str(texto);
    let interpretado =
        com_valores_json(|estado| pinker_json_contract::interpretar(texto, &mut estado.tabela));
    match interpretado {
        Ok(raiz) => {
            let leque = pinker_leque_criar_0(RESULTADO_TAG_OK);
            pinker_leque_anexar(leque, raiz)
        }
        Err(causa) => resultado_erro(&causa),
    }
}

/// Serialização determinística: objetos saem em ordem de chave.
#[no_mangle]
pub extern "C" fn pinker_json_emitir(handle: u64) -> *mut u8 {
    let texto = com_valores_json(|estado| pinker_json_contract::serializar(handle, &estado.tabela))
        .unwrap_or_else(|causa| erro_fatal(&causa));
    verso_alocar(&texto)
}

/// Discriminante de `TipoJson`, espelhando a ordem de declaração do contrato.
#[no_mangle]
pub extern "C" fn pinker_json_tipo(handle: u64) -> u64 {
    com_no_json(handle, |no, _| no.tipo().discriminante())
}

#[no_mangle]
pub extern "C" fn pinker_json_verso(handle: u64) -> *mut u8 {
    com_no_json(handle, |no, _| match no {
        NoJson::Verso(texto) => verso_alocar(texto),
        _ => erro_tipo_json("json_verso", "Verso"),
    })
}

#[no_mangle]
pub extern "C" fn pinker_json_numero(handle: u64) -> i64 {
    com_no_json(handle, |no, _| match no {
        NoJson::Numero(valor) => *valor,
        _ => erro_tipo_json("json_numero", "Numero"),
    })
}

#[no_mangle]
pub extern "C" fn pinker_json_logica(handle: u64) -> u64 {
    com_no_json(handle, |no, _| match no {
        NoJson::Logica(valor) => u64::from(*valor),
        _ => erro_tipo_json("json_logica", "Logica"),
    })
}

#[no_mangle]
pub extern "C" fn pinker_json_lista_tamanho(handle: u64) -> u64 {
    com_no_json(handle, |no, _| match no {
        NoJson::Lista(itens) => itens.len() as u64,
        _ => erro_tipo_json("json_lista_tamanho", "Lista"),
    })
}

/// Devolve o handle do item — é por aqui que o nesting é atravessado.
#[no_mangle]
pub extern "C" fn pinker_json_lista_obter(handle: u64, indice: u64) -> u64 {
    com_no_json(handle, |no, _| match no {
        NoJson::Lista(itens) => *itens
            .get(indice as usize)
            .unwrap_or_else(|| erro_fatal("índice fora da faixa em 'json_lista_obter'")),
        _ => erro_tipo_json("json_lista_obter", "Lista"),
    })
}

#[no_mangle]
pub extern "C" fn pinker_json_objeto_tamanho(handle: u64) -> u64 {
    com_no_json(handle, |no, _| match no {
        NoJson::Objeto(membros) => membros.len() as u64,
        _ => erro_tipo_json("json_objeto_tamanho", "Objeto"),
    })
}

/// # Safety
/// `chave` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_json_objeto_tem(handle: u64, chave: *const u8) -> u64 {
    let chave = verso_str(chave).to_string();
    com_no_json(handle, |no, _| match no {
        NoJson::Objeto(membros) => u64::from(membros.contains_key(&chave)),
        _ => erro_tipo_json("json_objeto_tem", "Objeto"),
    })
}

/// # Safety
/// `chave` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_json_objeto_obter(handle: u64, chave: *const u8) -> u64 {
    let chave = verso_str(chave).to_string();
    com_no_json(handle, |no, _| match no {
        NoJson::Objeto(membros) => *membros
            .get(&chave)
            .unwrap_or_else(|| erro_fatal("chave ausente em 'json_objeto_obter'")),
        _ => erro_tipo_json("json_objeto_obter", "Objeto"),
    })
}

/// Chaves em ordem de chave, pela mesma `lista<verso>` do código gerado.
#[no_mangle]
pub extern "C" fn pinker_json_objeto_chaves(handle: u64) -> *mut u8 {
    let chaves = com_no_json(handle, |no, _| match no {
        NoJson::Objeto(membros) => membros.keys().cloned().collect::<Vec<String>>(),
        _ => erro_tipo_json("json_objeto_chaves", "Objeto"),
    });
    let lista = pinker_lista_criar();
    if lista.is_null() {
        erro_fatal("sem memória ao criar lista de chaves JSON");
    }
    for chave in &chaves {
        unsafe { pinker_lista_anexar(lista, verso_alocar(chave) as u64) };
    }
    lista
}
// @pinker-nav:end runtime.json.valor-adulto

// @pinker-nav:start runtime.json.plano-legado
// @pinker-nav:domain dados
// @pinker-nav:layer runtime
// @pinker-nav:summary Owner nativo do recorte plano histórico, que antes não existia em backend nem runtime: `pinker_json_plano_ler` projeta o objeto de um nível para `mapa<verso,bombom>` pela mesma autoridade gramatical de `pinker_json_contract`, com domínio `u64` preservado inclusive acima de `i64::MAX`, e `pinker_json_plano_emitir` percorre o mapa pelo cursor do próprio runtime e serializa com chaves ordenadas e valores exatos, sem cast para `i64`. As recusas do recorte continuam fatais, como sempre foram nesta superfície — quem quer falha como valor usa `ler_json_resultado`.

/// Recorte plano histórico `verso -> bombom`, agora com dono nativo.
///
/// A falha continua fatal nesta superfície: o contrato histórico nunca
/// atravessou `Resultado`, e mudá-lo aqui quebraria compatibilidade. A porta
/// recuperável é `ler_json_resultado`.
///
/// # Safety
/// `texto` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_json_plano_ler(texto: *const u8) -> *mut u8 {
    let texto = verso_str(texto);
    let pares = pinker_json_contract::interpretar_plano_bombom(texto).unwrap_or_else(|causa| {
        erro_fatal(&format!(
            "json inválido em 'ler_json_plano_bombom': {causa}"
        ))
    });
    let mapa = pinker_mapa_criar_chave_verso();
    if mapa.is_null() {
        erro_fatal("sem memória ao criar mapa de json plano");
    }
    for (chave, valor) in &pares {
        pinker_mapa_definir(mapa, verso_alocar(chave) as u64, *valor);
    }
    mapa
}

/// Emissão do recorte plano: chaves ordenadas, valores `u64` exatos.
///
/// # Safety
/// `mapa` deve ser um handle de `mapa<verso,bombom>` válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_json_plano_emitir(mapa: *mut u8) -> *mut u8 {
    let total = pinker_mapa_tamanho(mapa);
    let cursor = pinker_mapa_iterador_criar(mapa);
    let mut pares: Vec<(String, u64)> = Vec::with_capacity(total as usize);
    for _ in 0..total {
        let chave = pinker_mapa_iterador_proxima(cursor);
        let texto = verso_str(chave as *const u8).to_string();
        let valor = pinker_mapa_obter(mapa, chave);
        pares.push((texto, valor));
    }
    pinker_liberar(cursor);
    let texto = pinker_json_contract::serializar_plano_bombom(&pares).unwrap_or_else(|causa| {
        erro_fatal(&format!(
            "json inválido em 'emitir_json_plano_bombom': {causa}"
        ))
    });
    verso_alocar(&texto)
}
// @pinker-nav:end runtime.json.plano-legado

// @pinker-nav:start evidencia.runtime.memoria-alocador
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Abertura do módulo de testes internos do runtime nativo e evidência em memória do alocador: alinhamento e usabilidade do bloco devolvido por `pinker_alocar`, não sobreposição entre alocações independentes, layout possuído e checked de closures, alocação de zero bytes e tolerância a `pinker_liberar` sobre ponteiro nulo.
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct StatusCleanupNativoControlado;

    struct OperacoesCleanupNativasControladas {
        falhar_observacao: bool,
        falhar_encerramento: bool,
        falhar_espera: bool,
        kill_tentado: bool,
        wait_tentado: bool,
    }

    impl OperacoesCleanupNativo for OperacoesCleanupNativasControladas {
        type Status = StatusCleanupNativoControlado;

        fn observar_status(&mut self) -> io::Result<Option<Self::Status>> {
            if self.falhar_observacao {
                Err(io::Error::other("try_wait nativo controlado"))
            } else {
                Ok(None)
            }
        }

        fn encerrar(&mut self) -> io::Result<()> {
            self.kill_tentado = true;
            if self.falhar_encerramento {
                Err(io::Error::other("kill nativo controlado"))
            } else {
                Ok(())
            }
        }

        fn esperar(&mut self) -> io::Result<Self::Status> {
            self.wait_tentado = true;
            if self.falhar_espera {
                Err(io::Error::other("wait nativo controlado"))
            } else {
                Ok(StatusCleanupNativoControlado)
            }
        }
    }

    #[test]
    fn erro_de_try_wait_nao_impede_kill_wait_e_reap_nativo() {
        let mut operacoes = OperacoesCleanupNativasControladas {
            falhar_observacao: true,
            falhar_encerramento: false,
            falhar_espera: false,
            kill_tentado: false,
            wait_tentado: false,
        };
        let mut status = None;

        let falhas = encerrar_e_reapear_nativo_com(&mut operacoes, &mut status)
            .expect_err("erro de observação deve permanecer explícito");

        assert!(falhas.observacao.is_some(), "TRY_WAIT_ERROR_OBSERVED");
        assert!(operacoes.kill_tentado, "KILL_ATTEMPTED");
        assert!(operacoes.wait_tentado, "WAIT_ATTEMPTED");
        assert!(status.is_some(), "REAP_PATH_REACHED");
    }

    #[test]
    fn causa_primaria_e_falhas_secundarias_permanecem_no_erro_nativo() {
        let mut operacoes = OperacoesCleanupNativasControladas {
            falhar_observacao: true,
            falhar_encerramento: true,
            falhar_espera: true,
            kill_tentado: false,
            wait_tentado: false,
        };
        let mut status = None;
        let limpeza = encerrar_e_reapear_nativo_com(&mut operacoes, &mut status);
        let erro = compor_falha_cleanup_nativo::<()>("causa primária".to_string(), limpeza)
            .expect_err("falha primária com cleanup falho não pode virar sucesso");

        assert!(operacoes.kill_tentado, "KILL_ATTEMPTED");
        assert!(operacoes.wait_tentado, "WAIT_ATTEMPTED");
        assert!(erro.contains("causa primária"), "{erro}");
        assert!(erro.contains("try_wait nativo controlado"), "{erro}");
        assert!(erro.contains("kill nativo controlado"), "{erro}");
        assert!(erro.contains("wait nativo controlado"), "{erro}");
    }

    struct LeitorQuantumNativo {
        sucessos_restantes: usize,
        chamadas: usize,
    }

    impl Read for LeitorQuantumNativo {
        fn read(&mut self, destino: &mut [u8]) -> io::Result<usize> {
            self.chamadas += 1;
            if self.sucessos_restantes == 0 {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            self.sucessos_restantes -= 1;
            destino.fill(b'N');
            Ok(destino.len())
        }
    }

    #[test]
    fn parte_d_quantum_nativo_devolve_controle_antes_de_would_block() {
        let mut leitor = LeitorQuantumNativo {
            sucessos_restantes: PROCESSO_ESTRUTURADO_QUANTUM_SYSCALLS + 1,
            chamadas: 0,
        };
        let mut destino = Vec::new();
        assert!(!drenar_quantum_nativo(&mut leitor, &mut destino).unwrap());
        assert_eq!(leitor.chamadas, PROCESSO_ESTRUTURADO_QUANTUM_SYSCALLS);
        assert_eq!(destino.len(), PROCESSO_ESTRUTURADO_QUANTUM_BYTES);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parte_d_cleanup_nativo_mata_e_reapeia_antes_do_retorno() {
        let filho = std::process::Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .expect("filho controlado para provar reap");
        let pid = filho.id();
        let mut guardiao = FilhoEstruturadoNativo::novo(filho);

        guardiao
            .encerrar_e_reapear()
            .expect("kill seguido de wait deve ser recuperável");

        assert!(guardiao.status().is_some());
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "filho direto ainda existe (inclusive como zumbi) após o retorno"
        );
    }

    #[test]
    fn parte_d_ate_zero_falha_antes_de_spawn_nativo() {
        let antes = PROCESSO_ESTRUTURADO_SPAWNS_TESTE.load(Ordering::SeqCst);
        let erro =
            match executar_processo_estruturado_nativo(ConfiguracaoProcessoEstruturadoNativa {
                programa: "/bin/true".to_string(),
                argumentos: Vec::new(),
                entrada: String::new(),
                diretorio: String::new(),
                ambiente: Vec::new(),
                limite: Some(std::time::Duration::ZERO),
            }) {
                Ok(_) => panic!("Ate(0) deve expirar antes do spawn nativo"),
                Err(erro) => erro,
            };

        assert!(erro.contains("limite de tempo excedido"), "{erro}");
        assert_eq!(
            PROCESSO_ESTRUTURADO_SPAWNS_TESTE.load(Ordering::SeqCst),
            antes,
            "NATIVE_SPAWN_COUNT precisa permanecer zero"
        );
    }

    #[test]
    fn parte_d_handle_nativo_emite_maximo_e_entra_em_esgotamento_sem_wrap() {
        let mut estado = EstadoSaidasProcesso::default();
        let antigo = estado.inserir(SaidaProcessoNativa {
            codigo: 11,
            stdout: "antigo".to_string(),
            stderr: String::new(),
        });
        estado.proximo = Some(u64::MAX);
        let ultimo = estado.inserir(SaidaProcessoNativa {
            codigo: 22,
            stdout: "ultimo".to_string(),
            stderr: String::new(),
        });

        assert_eq!(antigo, 1);
        assert_eq!(ultimo, u64::MAX);
        assert_eq!(estado.proximo, None);
        assert_eq!(estado.tabela.len(), 2);
        assert_eq!(estado.tabela.get(&antigo).unwrap().codigo, 11);
        assert_eq!(estado.tabela.get(&ultimo).unwrap().codigo, 22);
        assert!(!estado.tabela.contains_key(&0));
    }

    #[test]
    fn parte_d_ambiente_nativo_valida_nul_sem_rejeitar_igual_no_valor() {
        unsafe fn mapa_unitario(chave: &str, valor: &str) -> *mut u8 {
            let mapa = pinker_mapa_criar_chave_verso();
            pinker_mapa_definir(mapa, verso_alocar(chave) as u64, verso_alocar(valor) as u64);
            mapa
        }

        unsafe {
            assert_eq!(
                ler_mapa_verso_verso_nativo(mapa_unitario("PINKER_TEST", "a=b=c")).unwrap(),
                vec![("PINKER_TEST".to_string(), "a=b=c".to_string())]
            );
            for (chave, valor) in [("NUL\0CHAVE", "x"), ("CHAVE", "NUL\0VALOR")] {
                assert!(
                    ler_mapa_verso_verso_nativo(mapa_unitario(chave, valor)).is_err(),
                    "NUL precisa ser recusado antes do spawn"
                );
            }
        }
    }

    #[test]
    fn parte_d_limite_tempo_preserva_discriminantes_da_abi_nativa() {
        assert_eq!(PINKER_LIMITE_TEMPO_TAG_SEM_LIMITE, 0);
        assert_eq!(PINKER_LIMITE_TEMPO_TAG_ATE, 1);
    }

    #[test]
    fn parte_d_snapshot_nativo_e_imutavel_e_accessors_sao_tipados() {
        let handle = registrar_saida_processo(SaidaProcessoNativa {
            codigo: 17,
            stdout: "saida".to_string(),
            stderr: "erro".to_string(),
        });
        assert_eq!(pinker_saida_processo_codigo(handle), 17);
        let stdout = pinker_saida_processo_stdout(handle);
        let stderr = pinker_saida_processo_stderr(handle);
        assert_eq!(unsafe { verso_str(stdout.cast_const()) }, "saida");
        assert_eq!(unsafe { verso_str(stderr.cast_const()) }, "erro");
        unsafe {
            pinker_liberar(stdout);
            pinker_liberar(stderr);
        }
    }

    #[test]
    fn parte_d_resultado_saida_processo_roundtrip_pela_abi_nativa() {
        let handle = registrar_saida_processo(SaidaProcessoNativa {
            codigo: 23,
            stdout: "out".to_string(),
            stderr: "err".to_string(),
        });
        let resultado = pinker_leque_criar_0(0);
        let resultado = unsafe { pinker_leque_anexar(resultado, handle) };
        assert_eq!(unsafe { pinker_leque_tag(resultado) }, 0);
        let extraido = unsafe { pinker_leque_carga(resultado, 0, 0) };
        assert_eq!(extraido, handle);
        assert_eq!(pinker_saida_processo_codigo(extraido), 23);
        let stdout = pinker_saida_processo_stdout(extraido);
        let stderr = pinker_saida_processo_stderr(extraido);
        assert_eq!(unsafe { verso_str(stdout.cast_const()) }, "out");
        assert_eq!(unsafe { verso_str(stderr.cast_const()) }, "err");
        unsafe {
            pinker_liberar(stdout);
            pinker_liberar(stderr);
        }
    }

    #[test]
    fn d7_pack_valida_count_size_e_limite_isize() {
        assert_eq!(formatar_verso_pack_len(0), Some(0));
        assert_eq!(formatar_verso_pack_len(13), Some(13));
        let first_invalid = (isize::MAX as u64 / std::mem::size_of::<*const u8>() as u64) + 1;
        assert_eq!(formatar_verso_pack_len(first_invalid), None);
        assert_eq!(formatar_verso_pack_len(u64::MAX), None);
    }

    #[test]
    fn d7_pack_vazio_e_wrappers_legados_usam_a_mesma_autoridade() {
        let modelo_vazio = verso_alocar("sem substituições");
        let resultado_vazio =
            unsafe { pinker_formatar_verso_pack(modelo_vazio.cast_const(), 0, std::ptr::null()) };
        assert_eq!(
            unsafe { verso_str(resultado_vazio.cast_const()) },
            "sem substituições"
        );

        let modelo = verso_alocar("{}={}");
        let chave = verso_alocar("idade");
        let valor = verso_alocar("7");
        let direto = unsafe {
            let args = [chave.cast_const(), valor.cast_const()];
            pinker_formatar_verso_pack(modelo.cast_const(), 2, args.as_ptr())
        };
        let legado = unsafe {
            pinker_formatar_verso_2(modelo.cast_const(), chave.cast_const(), valor.cast_const())
        };
        assert_eq!(unsafe { verso_str(direto.cast_const()) }, "idade=7");
        assert_eq!(unsafe { verso_str(legado.cast_const()) }, "idade=7");

        unsafe {
            for ptr in [
                modelo_vazio,
                resultado_vazio,
                modelo,
                chave,
                valor,
                direto,
                legado,
            ] {
                pinker_liberar(ptr);
            }
        }
    }

    #[test]
    fn d3_layout_callable_e_checked_e_inclui_ambiente_trailing() {
        assert_eq!(callable_allocation_bytes(0), Some(16));
        assert_eq!(callable_allocation_bytes(3), Some(40));
        assert_eq!(callable_allocation_bytes(u64::MAX), None);
    }

    #[test]
    fn d3_callable_aloca_um_owner_para_descritor_e_tres_capturas() {
        let descriptor = pinker_callable_alocar(3);
        assert!(!descriptor.is_null());
        assert_eq!((descriptor as usize) % ALINHAMENTO, 0);

        unsafe {
            assert_eq!((descriptor as *const u64).read(), 0);
            let environment = (descriptor.add(8) as *const u64).read() as *mut u64;
            assert_eq!(environment as *mut u8, descriptor.add(16));
            environment.write(11);
            environment.add(1).write(22);
            environment.add(2).write(33);
            assert_eq!(environment.read(), 11);
            assert_eq!(environment.add(1).read(), 22);
            assert_eq!(environment.add(2).read(), 33);

            let total_with_allocator_header = (descriptor.sub(CABECALHO) as *const u64).read();
            assert_eq!(total_with_allocator_header, 40 + CABECALHO as u64);
            pinker_liberar(descriptor);
        }
    }

    #[cfg(unix)]
    fn filho_stdout(modo: &str) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args([
                "--exact",
                "tests::falha_stdout_termina_com_diagnostico_controlado",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_STDOUT_FILHO", modo)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("executar filho de teste")
    }

    #[cfg(unix)]
    #[test]
    fn falha_stdout_termina_com_diagnostico_controlado() {
        if let Some(modo) = std::env::var_os("PINKER_RT_TESTE_STDOUT_FILHO") {
            if modo == "full" {
                use std::os::fd::AsRawFd as _;

                let full = std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("/dev/full");
                extern "C" {
                    fn dup2(oldfd: i32, newfd: i32) -> i32;
                }
                assert_eq!(unsafe { dup2(full.as_raw_fd(), 1) }, 1);
            } else {
                extern "C" {
                    fn close(fd: i32) -> i32;
                    fn dup2(oldfd: i32, newfd: i32) -> i32;
                    fn pipe(fds: *mut i32) -> i32;
                }
                let mut fds = [0_i32; 2];
                assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
                assert_eq!(unsafe { close(fds[0]) }, 0);
                assert_eq!(unsafe { dup2(fds[1], 1) }, 1);
                assert_eq!(unsafe { close(fds[1]) }, 0);
            }
            unsafe {
                pinker_falar_pedaco_verso(verso_alocar("verso").cast_const());
            }
            pinker_falar_espaco();
            pinker_falar_pedaco_inteiro(-7);
            pinker_falar_fim();
            return;
        }

        let full_output = filho_stdout("full")
            .wait_with_output()
            .expect("aguardar filho /dev/full");
        assert_eq!(full_output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&full_output.stderr);
        assert!(stderr.contains("falha ao escrever stdout"), "{stderr}");
        assert!(!stderr.contains("panicked at"), "{stderr}");

        let pipe_output = filho_stdout("pipe")
            .wait_with_output()
            .expect("aguardar filho");
        assert_eq!(pipe_output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&pipe_output.stderr);
        assert!(stderr.contains("falha ao escrever stdout"), "{stderr}");
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }

    #[cfg(unix)]
    fn script_processo(nome: &str, corpo: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let path =
            std::env::temp_dir().join(format!("pinker-rt-processo-{nome}-{}", std::process::id()));
        std::fs::write(&path, format!("#!/bin/sh\n{corpo}\n")).expect("gravar script");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("permissões");
        path
    }

    #[cfg(unix)]
    #[test]
    fn processos_usam_resolucao_deterministica() {
        let resolvido = comando_resolvido("teste", "sh").expect("shell do sistema");
        assert!(
            resolvido == std::path::Path::new("/usr/local/bin/sh")
                || resolvido == std::path::Path::new("/usr/bin/sh")
                || resolvido == std::path::Path::new("/bin/sh"),
            "{}",
            resolvido.display()
        );
        assert_eq!(
            comando_resolvido("teste", "./ferramenta").unwrap(),
            std::path::Path::new("./ferramenta")
        );
        assert!(comando_resolvido("teste", "pinker-comando-certamente-ausente").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn stdin_concorre_com_espera_e_propaga_erros() {
        let leitor = script_processo("leitor", "cat >/dev/null\nexit 7");
        let entrada = "rosa".repeat(64 * 1024);
        assert_eq!(
            processo_com_entrada_resultado(leitor.to_str().unwrap(), &entrada, None).unwrap(),
            7
        );
        assert_eq!(
            processo_com_entrada_resultado("/bin/true", "", None).unwrap(),
            0
        );
        assert_eq!(
            processo_com_entrada_resultado("/bin/false", "", None).unwrap(),
            1
        );

        let nao_leitor = script_processo("nao-leitor", "exit 0");
        let erro = processo_com_entrada_resultado(nao_leitor.to_str().unwrap(), &entrada, None)
            .unwrap_err();
        assert!(erro.contains("falha ao escrever stdin"), "{erro}");
        assert!(
            processo_com_entrada_resultado("/caminho/ausente/pinker", "", None)
                .unwrap_err()
                .contains("falha ao executar processo")
        );

        std::fs::remove_file(leitor).expect("remover leitor");
        std::fs::remove_file(nao_leitor).expect("remover não leitor");
    }

    #[cfg(unix)]
    #[test]
    fn handles_de_arquivo_preservam_identidade_do_descritor() {
        use std::os::unix::fs::symlink;

        let raiz =
            std::env::temp_dir().join(format!("pinker-rt-descritores-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).expect("raiz");
        let caminho = raiz.join("aberto.txt");
        let movido = raiz.join("movido.txt");
        let externo = raiz.join("externo.txt");
        std::fs::write(&caminho, "inicial").expect("arquivo inicial");
        std::fs::write(&externo, "preservado").expect("arquivo externo");

        let arquivo = abrir_descritor(caminho.to_str().unwrap(), ModoArquivo::Abrir).unwrap();
        let mut aberto = ArquivoAberto {
            arquivo,
            anexo: false,
        };
        std::fs::rename(&caminho, &movido).expect("renomear");
        symlink(&externo, &caminho).expect("trocar caminho por symlink");
        substituir_descritor(&mut aberto, "teste", b"pelo descritor");
        assert_eq!(std::fs::read_to_string(&movido).unwrap(), "pelo descritor");
        assert_eq!(std::fs::read_to_string(&externo).unwrap(), "preservado");

        std::fs::remove_file(&movido).expect("remover nome do arquivo aberto");
        aberto
            .arquivo
            .seek(std::io::SeekFrom::Start(0))
            .expect("reposicionar removido");
        assert_eq!(ler_descritor(&mut aberto, "teste"), b"pelo descritor");

        std::fs::remove_file(&caminho).expect("remover symlink");
        std::fs::remove_file(&externo).expect("remover externo");
        std::fs::remove_dir(&raiz).expect("remover raiz");
    }

    #[cfg(unix)]
    #[test]
    fn criar_e_exclusivo_e_open_nao_materializa_arquivo_grande() {
        use std::os::unix::fs::symlink;

        let raiz =
            std::env::temp_dir().join(format!("pinker-rt-create-new-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).expect("raiz");
        let existente = raiz.join("existente.txt");
        let link = raiz.join("link.txt");
        let grande = raiz.join("grande.bin");
        std::fs::write(&existente, "preservar").expect("existente");
        symlink(&existente, &link).expect("symlink");
        assert!(abrir_descritor(existente.to_str().unwrap(), ModoArquivo::Criar).is_err());
        assert!(abrir_descritor(link.to_str().unwrap(), ModoArquivo::Criar).is_err());
        assert_eq!(std::fs::read_to_string(&existente).unwrap(), "preservar");

        let grande_file = std::fs::File::create(&grande).expect("grande");
        grande_file
            .set_len(MAX_ARQUIVO_VERSO_BYTES + 1)
            .expect("sparse");
        let aberto = abrir_descritor(grande.to_str().unwrap(), ModoArquivo::Abrir)
            .expect("open não lê conteúdo");
        assert_eq!(
            aberto.metadata().expect("metadata").len(),
            MAX_ARQUIVO_VERSO_BYTES + 1
        );

        std::fs::remove_dir_all(&raiz).expect("limpeza");
    }

    #[test]
    fn classificacao_de_fechados_nao_cresce_com_historico() {
        let io = EstadoIo {
            arquivos: HashMap::new(),
            proximo_handle: 1_000_001,
        };
        for handle in 1..=1_000_000 {
            assert!(handle_foi_fechado(&io, handle));
        }
        assert!(!handle_foi_fechado(&io, 0));
        assert!(!handle_foi_fechado(&io, 1_000_001));
        assert!(io.arquivos.is_empty());
    }

    #[cfg(target_os = "linux")]
    fn rss_atual_bytes() -> u64 {
        let statm = std::fs::read_to_string("/proc/self/statm").expect("/proc/self/statm");
        let residentes = statm
            .split_whitespace()
            .nth(1)
            .expect("residentes")
            .parse::<u64>()
            .expect("rss numérico");
        residentes * PAGINA_PUBLICA as u64
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn memoria_publica_descompromete_payload_com_metadata_limitada() {
        let ciclos = std::env::var("PINKER_RT_TESTE_CICLOS_PUBLICOS")
            .ok()
            .and_then(|valor| valor.parse::<usize>().ok())
            .unwrap_or(10_000);
        assert!(ciclos <= MAX_IDENTIDADES_PUBLICAS);
        let inicio_tempo = std::time::Instant::now();
        let rss_inicial = rss_atual_bytes();
        let mut rss_maximo = rss_inicial;
        let metadata_inicial = memoria_publica()
            .lock()
            .expect("metadata inicial")
            .alocacoes
            .len();
        for indice in 0..ciclos {
            let ponteiro = pinker_publico_alocar(1);
            unsafe {
                ponteiro.write(0xA5);
                pinker_publico_liberar(ponteiro);
            }
            if indice % 1_000 == 0 {
                rss_maximo = rss_maximo.max(rss_atual_bytes());
            }
        }
        let rss_final = rss_atual_bytes();
        let memoria = memoria_publica().lock().expect("metadata pública");
        assert!(memoria.alocacoes.len() >= metadata_inicial + ciclos);
        let limite_rss = rss_inicial
            .saturating_add(MAX_METADATA_PUBLICA_BYTES as u64)
            .saturating_add(64 * 1024 * 1024);
        assert!(rss_final <= limite_rss, "{rss_inicial} {rss_final}");
        eprintln!(
            "public-memory-profile ciclos={ciclos} rss_inicial={rss_inicial} rss_maximo={rss_maximo} rss_final={rss_final} metadata={} elapsed_ms={}",
            memoria.alocacoes.len(),
            inicio_tempo.elapsed().as_millis()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn validacao_publica_desconhecida_falha_fechada() {
        if std::env::var_os("PINKER_RT_TESTE_ACESSO_DESCONHECIDO").is_some() {
            pinker_publico_validar_acesso(0x1234usize as *const u8, 1, 1);
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args([
                "--exact",
                "tests::validacao_publica_desconhecida_falha_fechada",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_ACESSO_DESCONHECIDO", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("executar validação desconhecida");
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("E-RUNTIME-MEM-UNKNOWN-ACCESS"), "{stderr}");
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }

    #[test]
    fn alocar_devolve_bloco_alinhado_e_utilizavel() {
        let ptr = pinker_alocar(64);
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % ALINHAMENTO, 0);
        unsafe {
            for i in 0..64 {
                ptr.add(i).write(i as u8);
            }
            for i in 0..64 {
                assert_eq!(ptr.add(i).read(), i as u8);
            }
            pinker_liberar(ptr);
        }
    }

    #[test]
    fn alocacoes_independentes_nao_se_sobrepoem() {
        let a = pinker_alocar(32);
        let b = pinker_alocar(32);
        assert!(!a.is_null() && !b.is_null());
        assert_ne!(a, b);
        unsafe {
            a.write_bytes(0xAA, 32);
            b.write_bytes(0x55, 32);
            assert_eq!(a.read(), 0xAA);
            assert_eq!(b.read(), 0x55);
            pinker_liberar(a);
            pinker_liberar(b);
        }

        let mut registro = vec![
            AlocacaoPublica {
                identidade: 1,
                base: 0x1000,
                tamanho: 32,
                reservado: 32,
                viva: false,
            },
            AlocacaoPublica {
                identidade: 2,
                base: 0x1000,
                tamanho: 32,
                reservado: 32,
                viva: true,
            },
        ];
        let indice = indice_base_publica_mais_recente(&registro, 0x1000)
            .expect("a geração viva mais recente deve ser encontrada");
        assert_eq!(indice, 1);
        assert_eq!(registro[indice].identidade, 2);
        registro[indice].viva = false;
        registro.push(AlocacaoPublica {
            identidade: 3,
            base: 0x1000,
            tamanho: 32,
            reservado: 32,
            viva: true,
        });
        let indice = indice_base_publica_mais_recente(&registro, 0x1000)
            .expect("a terceira geração deve substituir a metadata antiga");
        assert_eq!(indice, 2);
        assert_eq!(registro[indice].identidade, 3);
        assert_eq!(registro.len(), 3, "a quarentena preserva todas as gerações");

        let handles = (0..8)
            .map(|byte| {
                std::thread::spawn(move || {
                    let ptr = pinker_publico_alocar(16);
                    assert!(!ptr.is_null());
                    unsafe {
                        ptr.write(byte);
                        assert_eq!(ptr.read(), byte);
                        pinker_publico_liberar(ptr);
                    }
                    ptr as usize
                })
            })
            .collect::<Vec<_>>();
        let mut enderecos = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread de alocação pública"))
            .collect::<Vec<_>>();
        enderecos.sort_unstable();
        enderecos.dedup();
        assert_eq!(enderecos.len(), 8);
    }

    #[test]
    fn alocar_zero_bytes_devolve_bloco_valido() {
        let ptr = pinker_alocar(0);
        assert!(!ptr.is_null());
        unsafe { pinker_liberar(ptr) };
    }

    #[test]
    fn liberar_nulo_e_seguro() {
        unsafe { pinker_liberar(std::ptr::null_mut()) };
    }

    #[cfg(target_os = "linux")]
    fn paginas_residentes(base: usize, tamanho: usize) -> usize {
        use std::ffi::c_void;
        extern "C" {
            fn mincore(address: *mut c_void, length: usize, vec: *mut u8) -> i32;
        }
        let paginas = tamanho.div_ceil(pinker_memory_contract::PUBLIC_PAGE_BYTES);
        let mut residencia = vec![0_u8; paginas];
        assert_eq!(
            unsafe {
                mincore(
                    base as *mut c_void,
                    paginas * pinker_memory_contract::PUBLIC_PAGE_BYTES,
                    residencia.as_mut_ptr(),
                )
            },
            0,
            "mincore: {}",
            std::io::Error::last_os_error()
        );
        residencia.into_iter().filter(|byte| byte & 1 != 0).count()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mapeamento_publico_grande_e_lazy_zero_e_proporcional() {
        let tamanho = 64 * 1024 * 1024;
        let mut memoria = MemoriaPublica {
            budget: PublicMemoryBudget::default(),
            alocacoes: Vec::new(),
        };
        let base = tentar_alocar_publico_com(
            &mut memoria,
            tamanho as u64,
            PUBLIC_MEMORY_LIMITS,
            reservar_metadata_publica,
            mapear_regiao_publica,
        )
        .expect("mapeamento público");
        let reservado = memoria.alocacoes[0].reservado;
        assert_eq!(reservado, tamanho);
        assert!(
            paginas_residentes(base, reservado) <= 8,
            "alocar sem tocar não pode materializar 64 MiB"
        );

        let offsets = [
            0,
            tamanho / 2,
            tamanho - pinker_memory_contract::PUBLIC_PAGE_BYTES,
        ];
        for offset in offsets {
            assert_eq!(
                unsafe { (base as *const u8).add(offset).read_volatile() },
                0
            );
            unsafe { (base as *mut u8).add(offset).write_volatile(0x5a) };
        }
        let residentes = paginas_residentes(base, reservado);
        // Em Linux x86-64 com THP em modo `always`, cada offset tocado pode
        // materializar uma PMD huge page de 2 MiB. Isso continua lazy e
        // proporcional aos três toques; a antiga zeragem ansiosa materializa
        // as 16.384 páginas do mapeamento e permanece fora desta tolerância.
        let paginas_por_thp = (2 * 1024 * 1024) / pinker_memory_contract::PUBLIC_PAGE_BYTES;
        let maximo_residente = offsets.len() * paginas_por_thp;
        assert!(
            (offsets.len()..=maximo_residente).contains(&residentes),
            "somente páginas ou THPs dos offsets tocados devem residir, observado {residentes}"
        );

        descomprometer_paginas_publicas(base, reservado).expect("descomprometer");
        memoria.budget =
            release_public_live_bytes(memoria.budget, reservado).expect("devolver bytes vivos");
        memoria.alocacoes[0].viva = false;
        assert_eq!(paginas_residentes(base, reservado), 0);
        assert_eq!(memoria.budget.live_reserved_bytes, 0);
        assert_eq!(memoria.budget.identity_count, 1);
        assert_eq!(memoria.budget.lifetime_virtual_bytes, tamanho as u64);

        let maps = std::fs::read_to_string("/proc/self/maps").expect("maps");
        let protegido = maps.lines().any(|line| {
            let Some((range, permissions)) = line.split_once(' ') else {
                return false;
            };
            let Some((start, end)) = range.split_once('-') else {
                return false;
            };
            let Ok(start) = usize::from_str_radix(start, 16) else {
                return false;
            };
            let Ok(end) = usize::from_str_radix(end, 16) else {
                return false;
            };
            base >= start && base < end && permissions.starts_with("---")
        });
        assert!(protegido, "região liberada precisa permanecer PROT_NONE");

        let novo = tentar_alocar_publico_com(
            &mut memoria,
            pinker_memory_contract::PUBLIC_PAGE_BYTES as u64,
            PUBLIC_MEMORY_LIMITS,
            reservar_metadata_publica,
            mapear_regiao_publica,
        )
        .expect("nova geração");
        assert_ne!(novo, base, "mapeamento morto não pode ser reutilizado");
        assert_eq!(memoria.budget.identity_count, 2);
    }

    #[test]
    fn falha_de_mapeamento_nao_consumo_orcamento_nem_identidade() {
        let mut memoria = MemoriaPublica {
            budget: PublicMemoryBudget::default(),
            alocacoes: Vec::new(),
        };
        let before = memoria.budget;
        let result = tentar_alocar_publico_com(
            &mut memoria,
            4096,
            PUBLIC_MEMORY_LIMITS,
            reservar_metadata_publica,
            |_| Err(()),
        );
        assert_eq!(result, Err(PublicAllocationVerdict::MappingFailure));
        assert_eq!(memoria.budget, before);
        assert!(memoria.alocacoes.is_empty());
    }

    #[test]
    fn falha_de_reserva_de_metadata_e_atomica_e_nao_mapeia() {
        let mut memoria = MemoriaPublica {
            budget: PublicMemoryBudget::default(),
            alocacoes: Vec::new(),
        };
        let before = memoria.budget;
        let map_called = std::cell::Cell::new(false);
        let result = tentar_alocar_publico_com(
            &mut memoria,
            4096,
            PUBLIC_MEMORY_LIMITS,
            |_| Err(()),
            |_| {
                map_called.set(true);
                Ok(0x1000)
            },
        );
        assert_eq!(result, Err(PublicAllocationVerdict::MetadataBudgetExceeded));
        assert!(!map_called.get());
        assert_eq!(memoria.budget, before);
        assert!(memoria.alocacoes.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mapeamento_publico_novo_e_integralmente_zero_em_tamanho_seguro() {
        let tamanho = 1024 * 1024;
        let base = mapear_regiao_publica(tamanho).expect("mapeamento anônimo");
        let bytes = unsafe { std::slice::from_raw_parts(base as *const u8, tamanho) };
        assert!(bytes.iter().all(|byte| *byte == 0));
        descomprometer_paginas_publicas(base, tamanho).expect("descomprometer");
    }

    // @pinker-nav:end evidencia.runtime.memoria-alocador

    // @pinker-nav:start evidencia.runtime.validacao-acesso-publico
    // @pinker-nav:domain memoria
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Matriz do veredicto de acesso à memória pública (hotfix pós-PR #411, item V4) sobre a unidade pura `classificar_acesso_publico`: endereços não registrados (4096 não mapeado, nulo, pilha, dado estático, função, alocação interna do runtime, mapeamento estrangeiro válido) recusados como E-RUNTIME-MEM-UNKNOWN-ACCESS; região liberada como use-after-free; base viva, interior, primeiro e último byte válidos permitidos; um byte após a região e acesso multibyte cruzando o limite recusados; e a matriz de larguras 1/2/4/8 com os alinhamentos correspondentes, idêntica para load e store porque ambos compartilham o mesmo predicado; mais a sustentação do construtor, que mostra por que não existe veredicto de metadata de região inválida — `pinker_publico_alocar` é a única origem produtiva de `AlocacaoPublica` e toda entrada publicada tem tamanho ≥ 1, `tamanho <= reservado` e `base + tamanho` representável.
    /// Registro sintético com uma região viva de 64 bytes e uma liberada de 16,
    /// em endereços que não colidem com nada real do processo.
    fn registro_de_teste() -> Vec<AlocacaoPublica> {
        vec![
            AlocacaoPublica {
                identidade: 1,
                base: 0x4000_0000,
                tamanho: 64,
                reservado: PAGINA_PUBLICA,
                viva: true,
            },
            AlocacaoPublica {
                identidade: 2,
                base: 0x5000_0000,
                tamanho: 16,
                reservado: PAGINA_PUBLICA,
                viva: false,
            },
        ]
    }

    /// Larguras e alinhamentos operacionais dos tipos escalares endereçáveis:
    /// `u8`/`i8`/`logica` (1), `u16`/`i16` (2), `u32`/`i32` (4) e
    /// `u64`/`i64`/`bombom` (8).
    const LARGURAS_OPERACIONAIS: [usize; 4] = [1, 2, 4, 8];

    #[test]
    fn acesso_a_endereco_nunca_registrado_e_recusado_em_toda_largura() {
        let registro = registro_de_teste();
        let pilha = 0_u64;
        static ESTATICO: u64 = 0;
        fn funcao_alvo() {}

        let interno = pinker_alocar(32);
        let candidatos: [(&str, usize); 7] = [
            ("4096 não mapeado", 4096),
            ("nulo", 0),
            ("pilha", std::ptr::addr_of!(pilha) as usize),
            ("dado estático", std::ptr::addr_of!(ESTATICO) as usize),
            ("função", funcao_alvo as usize),
            ("interno do runtime", interno as usize),
            // Mapeamento estrangeiro válido: a arena pública existe, mas este
            // offset não pertence a nenhuma região registrada.
            ("mapeamento estrangeiro", 0x4000_0000 + PAGINA_PUBLICA * 4),
        ];

        for (rotulo, endereco) in candidatos {
            for largura in LARGURAS_OPERACIONAIS {
                // O endereço é alinhado para a largura sob teste, de modo que a
                // recusa venha da ausência de região e não do alinhamento.
                let alinhado = endereco & !(largura - 1);
                assert_eq!(
                    classificar_acesso_publico(&registro, alinhado, largura, largura),
                    VeredictoAcesso::Desconhecido,
                    "{rotulo} com largura {largura} deveria ser recusado"
                );
            }
        }
        unsafe { pinker_liberar(interno) };
    }

    #[test]
    fn acessos_validos_sao_permitidos_em_toda_largura() {
        let registro = registro_de_teste();
        let base = 0x4000_0000_usize;
        for largura in LARGURAS_OPERACIONAIS {
            for (rotulo, endereco) in [
                ("base viva", base),
                ("interior válido", base + 16),
                ("primeiro byte", base),
                ("último byte válido", base + 64 - largura),
            ] {
                assert_eq!(
                    classificar_acesso_publico(&registro, endereco, largura, largura),
                    VeredictoAcesso::Permitido,
                    "{rotulo} com largura {largura} deveria ser permitido"
                );
            }
        }
    }

    #[test]
    fn limites_da_regiao_sao_recusados_com_diagnostico_proprio() {
        let registro = registro_de_teste();
        let base = 0x4000_0000_usize;

        // Um byte após a região: começa dentro do intervalo [base, fim] mas o
        // acesso não cabe.
        assert_eq!(
            classificar_acesso_publico(&registro, base + 64, 1, 1),
            VeredictoAcesso::CruzaLimite
        );
        // Acesso multibyte que começa dentro da região e termina depois dela.
        // O alinhamento exigido é 1 para que a recusa venha da contenção, e
        // não do alinhamento.
        for largura in [2, 4, 8] {
            let inicio = base + 64 - largura + 1;
            assert_eq!(
                classificar_acesso_publico(&registro, inicio, largura, 1),
                VeredictoAcesso::CruzaLimite,
                "largura {largura} começando em {inicio:#x} deveria cruzar o limite"
            );
            // O mesmo acesso, um byte antes, ainda cabe: prova que a fronteira
            // testada é exatamente o último byte válido.
            assert_eq!(
                classificar_acesso_publico(&registro, inicio - 1, largura, 1),
                VeredictoAcesso::Permitido,
                "largura {largura} deveria caber terminando no último byte"
            );
        }
        // Acesso que começa antes da base e invade a região.
        assert_eq!(
            classificar_acesso_publico(&registro, base - 4, 8, 1),
            VeredictoAcesso::ForaDosLimites
        );
    }

    #[test]
    fn regiao_liberada_e_desalinhamento_tem_veredictos_distintos() {
        let registro = registro_de_teste();
        assert_eq!(
            classificar_acesso_publico(&registro, 0x5000_0000, 8, 8),
            VeredictoAcesso::UsoAposLiberar
        );
        assert_eq!(
            classificar_acesso_publico(&registro, 0x4000_0001, 8, 8),
            VeredictoAcesso::Desalinhado
        );
    }

    #[test]
    fn metadados_invalidos_e_overflow_sao_classificados_antes_do_registro() {
        let registro = registro_de_teste();
        assert_eq!(
            classificar_acesso_publico(&registro, 0x4000_0000, 0, 8),
            VeredictoAcesso::MetadadosInvalidos
        );
        assert_eq!(
            classificar_acesso_publico(&registro, 0x4000_0000, 8, 0),
            VeredictoAcesso::MetadadosInvalidos
        );
        assert_eq!(
            classificar_acesso_publico(&registro, 0x4000_0000, 8, 3),
            VeredictoAcesso::MetadadosInvalidos
        );
        assert_eq!(
            classificar_acesso_publico(&registro, usize::MAX, 8, 1),
            VeredictoAcesso::OverflowEndereco
        );
    }

    #[test]
    fn todo_veredicto_recusado_tem_diagnostico_estavel() {
        assert_eq!(VeredictoAcesso::Permitido.diagnostico(), None);
        for veredicto in [
            VeredictoAcesso::MetadadosInvalidos,
            VeredictoAcesso::OverflowEndereco,
            VeredictoAcesso::Desconhecido,
            VeredictoAcesso::UsoAposLiberar,
            VeredictoAcesso::Desalinhado,
            VeredictoAcesso::CruzaLimite,
            VeredictoAcesso::ForaDosLimites,
        ] {
            let diagnostico = veredicto.diagnostico().expect("veredicto recusado");
            assert!(!diagnostico.is_empty());
        }
        assert_eq!(
            VeredictoAcesso::Desconhecido.diagnostico(),
            Some("E-RUNTIME-MEM-UNKNOWN-ACCESS: acesso sem região pública registrada")
        );
    }

    /// Não há registro vazio permissivo: sem nenhuma região registrada, todo
    /// endereço é recusado. Esta é a garantia que faltava antes do hotfix.
    #[test]
    fn registro_vazio_nao_permite_nenhum_acesso() {
        for endereco in [0, 1, 4096, 0x4000_0000, usize::MAX / 2] {
            assert_eq!(
                classificar_acesso_publico(&[], endereco, 1, 1),
                VeredictoAcesso::Desconhecido,
                "endereço {endereco:#x} não pode ser aceito com registro vazio"
            );
        }
    }

    /// O invariante da arena não é um comentário: é uma asserção executada.
    ///
    /// Se uma entrada com `base + tamanho` irrepresentável chegasse ao
    /// registro, `fim_da_regiao` falha alto em debug e em teste, em vez de
    /// devolver um fim inventado e classificar o acesso em cima dele. Este
    /// teste é o que impede que o guarda seja trocado por um `unwrap_or`
    /// silencioso numa mudança futura.
    #[test]
    #[should_panic(expected = "invariante da arena pública")]
    fn metadata_de_regiao_irrepresentavel_falha_alto() {
        let corrompida = AlocacaoPublica {
            identidade: 1,
            base: usize::MAX,
            tamanho: 2,
            reservado: 2,
            viva: true,
        };
        let _ = fim_da_regiao(&corrompida);
    }

    /// Sustentação do Ponto 4: não existe veredicto de "metadados de região
    /// inválidos" porque não existe entrada de registro com metadata inválida.
    ///
    /// A prova é sobre o **construtor**, não sobre o classificador:
    /// `pinker_publico_alocar` é a única origem produtiva de `AlocacaoPublica`,
    /// e toda entrada que ele publica satisfaz `tamanho >= 1`,
    /// `base + tamanho` representável e `tamanho <= reservado`. Enquanto isso
    /// valer, `fim_da_regiao` nunca satura e o antigo braço seria inalcançável.
    #[test]
    fn construtor_publico_so_registra_metadata_de_regiao_valida() {
        // A guarda de tamanho zero encerra o processo, então só um filho
        // re-executado consegue exercitá-la. É ela que sustenta `tamanho >= 1`
        // em toda entrada do registro — sem isso, uma região de tamanho nulo
        // entraria e a metadata deixaria de ser válida por construção.
        if std::env::var_os("PINKER_RT_TESTE_ALOCAR_ZERO").is_some() {
            pinker_publico_alocar(0);
            unreachable!("'alocar' precisa recusar tamanho zero");
        }
        let filho = std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args([
                "--exact",
                "tests::construtor_publico_so_registra_metadata_de_regiao_valida",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_ALOCAR_ZERO", "1")
            .output()
            .expect("executar filho de teste");
        assert_eq!(
            filho.status.code(),
            Some(1),
            "'alocar' com tamanho zero precisa encerrar por diagnóstico"
        );
        assert!(
            String::from_utf8_lossy(&filho.stderr).contains("'alocar' rejeita tamanho zero"),
            "diagnóstico inesperado: {}",
            String::from_utf8_lossy(&filho.stderr)
        );

        let mut ponteiros = Vec::new();
        for tamanho in [1_u64, 7, 8, 64, 4096, 4097] {
            let ptr = pinker_publico_alocar(tamanho);
            assert!(!ptr.is_null());
            ponteiros.push(ptr);
        }
        {
            let memoria = memoria_publica().lock().expect("registro público");
            assert!(
                !memoria.alocacoes.is_empty(),
                "o construtor precisa ter publicado entradas"
            );
            for alocacao in &memoria.alocacoes {
                assert!(
                    alocacao.tamanho >= 1,
                    "'alocar' recusa zero, então nenhuma entrada tem tamanho nulo"
                );
                assert!(
                    alocacao.tamanho <= alocacao.reservado,
                    "a região visível nunca excede as páginas comprometidas"
                );
                let fim = alocacao
                    .base
                    .checked_add(alocacao.tamanho)
                    .expect("base + tamanho precisa ser representável");
                assert_eq!(
                    fim,
                    fim_da_regiao(alocacao),
                    "fim_da_regiao não pode saturar sobre metadata produtiva"
                );
                assert!(
                    alocacao
                        .base
                        .checked_add(alocacao.reservado)
                        .is_some_and(|limite| limite >= fim),
                    "a reserva inteira também precisa ser representável"
                );
            }
        }
        for ptr in ponteiros {
            unsafe { pinker_publico_liberar(ptr) };
        }
    }
    // @pinker-nav:end evidencia.runtime.validacao-acesso-publico

    // @pinker-nav:start evidencia.runtime.cota-identidades-publicas
    // @pinker-nav:domain memoria
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência da cota vitalícia de identidades públicas (hotfix pós-PR #411, item V3): a unidade contada é a entrada de registro, liberar não devolve capacidade, identidades concedidas são estritamente crescentes e nunca reutilizadas, o esgotamento é irrecuperável no mesmo processo, e o diagnóstico é estável — verificado também de ponta a ponta num filho re-executado com o limite reduzido pela configuração interna de teste, sem interruptor público.
    #[test]
    fn identidade_publica_e_concedida_ate_a_cota_e_depois_esgota() {
        // A cota conta entradas de registro, não alocações vivas.
        for registradas in 0..4_usize {
            assert!(matches!(
                reservar_identidade_publica(registradas, 1, 4),
                ReservaIdentidade::Concedida { .. }
            ));
        }
        assert_eq!(
            reservar_identidade_publica(4, 1, 4),
            ReservaIdentidade::Esgotada
        );
        // Uma vez esgotada, continua esgotada: não há recuperação no processo.
        assert_eq!(
            reservar_identidade_publica(9_999, 1, 4),
            ReservaIdentidade::Esgotada
        );
    }

    #[test]
    fn liberar_nao_devolve_capacidade_de_identidade() {
        // Simula o laço `alocar(1024) + liberar` sempre pareado: nunca há mais
        // de uma alocação viva, mas o registro cresce a cada ciclo e as
        // identidades nunca se repetem.
        let limite = 8;
        let mut registradas = 0_usize;
        let mut proxima = 1_u64;
        let mut concedidas = Vec::new();
        loop {
            match reservar_identidade_publica(registradas, proxima, limite) {
                ReservaIdentidade::Concedida {
                    identidade,
                    proxima: seguinte,
                } => {
                    concedidas.push(identidade);
                    proxima = seguinte;
                    // `liberar` marcaria `viva = false` sem remover a entrada.
                    registradas += 1;
                }
                ReservaIdentidade::Esgotada => break,
                ReservaIdentidade::Exaurida => panic!("contador não deveria estourar"),
            }
        }
        assert_eq!(concedidas.len(), limite, "a cota é exatamente o limite");
        let mut unicas = concedidas.clone();
        unicas.sort_unstable();
        unicas.dedup();
        assert_eq!(unicas, concedidas, "identidades não podem se repetir");
        assert!(
            concedidas.windows(2).all(|par| par[0] < par[1]),
            "identidades precisam ser estritamente crescentes"
        );
    }

    #[test]
    fn contador_de_identidade_saturado_e_classificado() {
        assert_eq!(
            reservar_identidade_publica(0, u64::MAX, usize::MAX),
            ReservaIdentidade::Exaurida
        );
    }

    #[cfg(unix)]
    #[test]
    fn esgotamento_de_identidade_tem_diagnostico_estavel() {
        const LIMITE_REDUZIDO: usize = 4;
        if std::env::var_os("PINKER_RT_TESTE_COTA_IDENTIDADE").is_some() {
            LIMITE_IDENTIDADES_PUBLICAS_TESTE.store(LIMITE_REDUZIDO, Ordering::SeqCst);
            // Alocação e liberação sempre pareadas: os bytes voltam a cada
            // ciclo, mas a capacidade de identidade não. O ciclo seguinte à
            // cota precisa falhar mesmo sem nenhuma alocação viva.
            for _ in 0..LIMITE_REDUZIDO {
                let ponteiro = pinker_publico_alocar(16);
                assert!(!ponteiro.is_null());
                unsafe { pinker_publico_liberar(ponteiro) };
            }
            let _ = pinker_publico_alocar(16);
            unreachable!("a alocação além da cota deveria encerrar o processo");
        }

        let filho = std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args([
                "--exact",
                "tests::esgotamento_de_identidade_tem_diagnostico_estavel",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_COTA_IDENTIDADE", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("executar filho da cota de identidade");

        assert_eq!(
            filho.status.code(),
            Some(1),
            "o esgotamento precisa encerrar pelo diagnóstico controlado"
        );
        let stderr = String::from_utf8_lossy(&filho.stderr);
        assert!(
            stderr.contains("limite de identidades públicas esgotado"),
            "diagnóstico de esgotamento instável: {stderr}"
        );
        assert!(
            !stderr.contains("panicked at"),
            "o esgotamento não pode virar panic: {stderr}"
        );
    }

    /// Paridade de contabilidade: o domínio interno de união não toca a cota
    /// vitalícia de identidades públicas.
    ///
    /// Criar descritores e copiar payloads move **apenas** o orçamento de
    /// uniões; o registro de identidades públicas fica numericamente intacto —
    /// o mesmo contrato que o interpretador aplica na arena interna de binding.
    /// A medição roda num filho re-executado porque o registro público e o
    /// orçamento de uniões são estado do processo: só isolado o delta é exato.
    #[test]
    fn dominio_interno_de_uniao_nao_consome_identidade_publica() {
        if std::env::var_os("PINKER_RT_TESTE_CONTABILIDADE_UNIAO").is_some() {
            let antes_identidades = identidades_publicas_registradas();
            let antes_orcamento = orcamento_de_unioes();

            let origem: [u64; 3] = [111, 222, 333];
            let mut destino = [0_u64; 3];
            for _ in 0..8 {
                let handle =
                    unsafe { pinker_uniao_criar(7, 3, 24, 8, origem.as_ptr() as *const u8) };
                unsafe {
                    pinker_uniao_copiar_payload(
                        handle,
                        7,
                        3,
                        24,
                        8,
                        destino.as_mut_ptr() as *mut u8,
                    );
                }
            }

            assert_eq!(
                identidades_publicas_registradas(),
                antes_identidades,
                "oito construções e oito extrações não podem consumir identidade pública alguma"
            );
            let depois = orcamento_de_unioes();
            assert_eq!(
                depois.descriptors - antes_orcamento.descriptors,
                8,
                "cada construção cobra exatamente um descritor interno"
            );
            assert_eq!(
                depois.payload_bytes - antes_orcamento.payload_bytes,
                8 * 24,
                "cada construção cobra os bytes reais do payload multi-palavra"
            );
            assert_eq!(
                depois.metadata_bytes - antes_orcamento.metadata_bytes,
                8 * UNION_DESCRIPTOR_METADATA_BYTES
            );
            assert_eq!(
                destino, origem,
                "payload multi-palavra copiado integralmente"
            );

            // A cota pública continua respondendo normalmente depois disso.
            let ponteiro = pinker_publico_alocar(16);
            assert!(!ponteiro.is_null());
            assert_eq!(
                identidades_publicas_registradas(),
                antes_identidades + 1,
                "um `alocar` bem-sucedido consome exatamente uma identidade pública"
            );
            return;
        }

        let saida = filho_contabilidade_uniao(
            "tests::dominio_interno_de_uniao_nao_consome_identidade_publica",
        );
        assert!(
            saida.status.success(),
            "filho da contabilidade de união falhou: {}{}",
            String::from_utf8_lossy(&saida.stdout),
            String::from_utf8_lossy(&saida.stderr)
        );
    }

    /// A extração nativa escreve num destino do chamador (slot do frame), e não
    /// numa região pública: o ponteiro entregue nunca pertence ao registro de
    /// identidades públicas.
    #[test]
    fn extracao_nativa_escreve_em_destino_do_chamador() {
        let origem: [u64; 2] = [7, 9];
        let mut destino = [0_u64; 2];
        let handle = unsafe { pinker_uniao_criar(11, 1, 16, 8, origem.as_ptr() as *const u8) };
        unsafe {
            pinker_uniao_copiar_payload(handle, 11, 1, 16, 8, destino.as_mut_ptr() as *mut u8);
        }
        assert_eq!(destino, origem);
        let endereco = destino.as_ptr() as usize;
        let publico = memoria_publica()
            .lock()
            .map(|memoria| {
                memoria.alocacoes.iter().any(|alocacao| {
                    alocacao
                        .base
                        .checked_add(alocacao.tamanho)
                        .is_some_and(|fim| endereco >= alocacao.base && endereco < fim)
                })
            })
            .expect("registro público disponível");
        assert!(
            !publico,
            "o destino da extração não pode pertencer ao registro público"
        );
    }

    fn identidades_publicas_registradas() -> usize {
        memoria_publica()
            .lock()
            .map(|memoria| memoria.alocacoes.len())
            .expect("registro público disponível")
    }

    fn orcamento_de_unioes() -> UnionBudget {
        estado_unioes()
            .lock()
            .map(|estado| estado.orcamento)
            .expect("estado de uniões disponível")
    }

    fn filho_contabilidade_uniao(teste: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args(["--exact", teste, "--nocapture"])
            .env("PINKER_RT_TESTE_CONTABILIDADE_UNIAO", "1")
            .output()
            .expect("executar filho da contabilidade de união")
    }

    // @pinker-nav:end evidencia.runtime.cota-identidades-publicas

    // @pinker-nav:start evidencia.runtime.inicializacao-abi
    // @pinker-nav:domain inicializacao
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória do bootstrap e da ABI: `pinker_rt_iniciar` desabilita core dump antes do código Pinker, captura `argc`/`argv` e os devolve por `pinker_rt_argc`/`pinker_rt_argv`, e `pinker_rt_versao` reporta a versão corrente da ABI.
    #[test]
    fn iniciar_captura_argc_e_argv() {
        let argv: [*const u8; 2] = [b"pink\0".as_ptr(), std::ptr::null()];
        unsafe { pinker_rt_iniciar(1, argv.as_ptr()) };
        assert_eq!(pinker_rt_argc(), 1);
        assert_eq!(pinker_rt_argv(), argv.as_ptr());
    }

    #[test]
    fn versao_da_abi_atual() {
        assert_eq!(pinker_rt_versao(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn iniciar_desabilita_core_dump_mesmo_se_o_filho_herdar_soft_ilimitado() {
        #[repr(C)]
        struct RLimit {
            current: u64,
            maximum: u64,
        }
        extern "C" {
            fn getrlimit(resource: i32, limit: *mut RLimit) -> i32;
            fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
        }
        const RLIMIT_CORE: i32 = 4;
        const RLIMIT_CPU: i32 = 0;
        const RLIMIT_AS: i32 = 9;

        fn core_limit() -> (u64, u64) {
            let mut limit = RLimit {
                current: 0,
                maximum: 0,
            };
            assert_eq!(unsafe { getrlimit(RLIMIT_CORE, &mut limit) }, 0);
            (limit.current, limit.maximum)
        }

        if std::env::var_os("PINKER_RT_TESTE_CORE_FILHO").is_some() {
            let before = core_limit();
            let argv: [*const u8; 2] = [b"pink\0".as_ptr(), std::ptr::null()];
            unsafe { pinker_rt_iniciar(1, argv.as_ptr()) };
            let after = core_limit();
            assert_eq!(after.0, 0, "soft RLIMIT_CORE precisa ser zero");
            assert_eq!(
                after.1, before.1,
                "runtime preserva o hard limit do operador"
            );
            return;
        }

        use std::os::unix::process::CommandExt;
        let mut command =
            std::process::Command::new(std::env::current_exe().expect("binário de teste"));
        command
            .args([
                "--exact",
                "tests::iniciar_desabilita_core_dump_mesmo_se_o_filho_herdar_soft_ilimitado",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_CORE_FILHO", "1");
        unsafe {
            command.pre_exec(|| {
                let mut limit = RLimit {
                    current: 0,
                    maximum: 0,
                };
                if getrlimit(RLIMIT_CORE, &mut limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                limit.current = limit.maximum;
                if setrlimit(RLIMIT_CORE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for (resource, value) in [(RLIMIT_CPU, 15), (RLIMIT_AS, 1024 * 1024 * 1024)] {
                    let limit = RLimit {
                        current: value,
                        maximum: value,
                    };
                    if setrlimit(resource, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let output = command
            .output()
            .expect("executar filho com core habilitado");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // @pinker-nav:end evidencia.runtime.inicializacao-abi

    // @pinker-nav:start evidencia.runtime.sigpipe-disposicao
    // @pinker-nav:domain processos
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência do contrato de SIGPIPE do runtime: partindo de SIG_DFL num processo filho dedicado, pinker_rt_iniciar deixa o pai com SIG_IGN; um filho criado pelo construtor comum comando_saneado observa SIG_DFL antes da inicialização da linguagem, medido por construtor de .init_array do próprio binário de teste; e restaurar_disposicao_padrao devolve erro em vez de silenciar falha, sem tocar a disposição do processo.
    /// Sentinela distinta de qualquer disposição real; sinaliza que o
    /// construtor de `.init_array` não rodou.
    #[cfg(unix)]
    const SIGPIPE_NAO_OBSERVADO: usize = usize::MAX - 1;

    /// Disposição de `SIGPIPE` que este binário de teste herdou através de
    /// `exec`.
    ///
    /// A leitura precisa acontecer antes do `lang_start` da std, que instala
    /// `SIG_IGN` e mascararia a herança. Por isso o construtor abaixo é
    /// registrado em `.init_array`, durante a partida da libc.
    #[cfg(unix)]
    static SIGPIPE_HERDADO_PELO_TESTE: AtomicUsize = AtomicUsize::new(SIGPIPE_NAO_OBSERVADO);

    /// Lê a disposição herdada restaurando-a em seguida: `signal(2)` devolve a
    /// disposição anterior, então ler exige uma troca imediatamente desfeita.
    ///
    /// # Safety
    /// Roda em `.init_array`, antes de threads e antes da std; usa apenas
    /// `signal(2)`.
    #[cfg(unix)]
    unsafe extern "C" fn capturar_sigpipe_herdado_pelo_teste() {
        let anterior = signal(SINAL_SIGPIPE, SINAL_HANDLER_PADRAO);
        if anterior != SINAL_HANDLER_ERRO && anterior != SINAL_HANDLER_PADRAO {
            signal(SINAL_SIGPIPE, anterior);
        }
        SIGPIPE_HERDADO_PELO_TESTE.store(anterior, Ordering::SeqCst);
    }

    #[cfg(unix)]
    #[used]
    #[cfg_attr(target_os = "linux", link_section = ".init_array")]
    static CONSTRUTOR_SIGPIPE_DO_TESTE: unsafe extern "C" fn() =
        capturar_sigpipe_herdado_pelo_teste;

    /// Depois de `pinker_rt_iniciar`, o processo Pinker pai ignora `SIGPIPE`.
    ///
    /// A medida roda num processo filho dedicado porque a preparação é
    /// idempotente por `Once`: num processo que já a consumiu, a chamada seria
    /// um no-op e o teste ficaria vazio. O filho começa devolvendo `SIGPIPE` a
    /// `SIG_DFL`, reproduzindo o `main` nativo — que não passa pela
    /// inicialização da std — e só então chama a inicialização do runtime.
    #[cfg(unix)]
    #[test]
    fn pai_ignora_sigpipe_depois_de_iniciar() {
        if std::env::var_os("PINKER_RT_TESTE_SIGPIPE_PAI").is_some() {
            restaurar_disposicao_padrao(SINAL_SIGPIPE).expect("partir de SIG_DFL");
            let argv: [*const u8; 2] = [b"pink\0".as_ptr(), std::ptr::null()];
            // SAFETY: `argv` é um vetor válido terminado em nulo, vivo por toda
            // a chamada; o runtime apenas o armazena.
            unsafe { pinker_rt_iniciar(1, argv.as_ptr()) };
            // Ler a disposição exige trocá-la; trocar justamente por `SIG_IGN`
            // devolve o valor anterior sem alterar o estado esperado.
            // SAFETY: FFI pura com handler constante, sem ponteiros.
            let observada = unsafe { signal(SINAL_SIGPIPE, SINAL_HANDLER_IGNORAR) };
            assert_eq!(
                observada, SINAL_HANDLER_IGNORAR,
                "o pai Pinker precisa ignorar SIGPIPE depois de pinker_rt_iniciar"
            );
            return;
        }

        let saida = std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args([
                "--exact",
                "tests::pai_ignora_sigpipe_depois_de_iniciar",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_SIGPIPE_PAI", "1")
            .output()
            .expect("executar filho de teste");
        let relato = String::from_utf8_lossy(&saida.stdout).into_owned();
        assert!(
            saida.status.success(),
            "disposição do pai divergente:\n{relato}"
        );
        // Sem esta guarda, um filtro que não casasse com nenhum teste também
        // sairia com sucesso e o teste passaria sem medir nada.
        assert!(
            relato.contains("1 passed"),
            "o filho precisa ter executado exatamente o teste medido:\n{relato}"
        );
    }

    /// Todo filho criado pelo construtor comum observa `SIG_DFL`, mesmo com o
    /// pai ignorando `SIGPIPE`.
    ///
    /// Esta é a prova da autoridade comum: `comando_saneado` é o único ponto
    /// que constrói `Command` para as famílias de subprocesso, e a preparação
    /// está nele. A matriz de ponta a ponta cobre cada família observando o
    /// mesmo contrato pelo programa Pinker.
    #[cfg(unix)]
    #[test]
    fn filho_de_comando_saneado_observa_disposicao_padrao() {
        // `SIG_DFL` sai com código próprio, não com zero: um filtro que não
        // casasse com nenhum teste faria o binário sair com zero, e o teste
        // passaria sem medir nada.
        const OBSERVOU_PADRAO: i32 = 7;

        if std::env::var_os("PINKER_RT_TESTE_SIGPIPE_FILHO").is_some() {
            let codigo = match SIGPIPE_HERDADO_PELO_TESTE.load(Ordering::SeqCst) {
                SINAL_HANDLER_PADRAO => OBSERVOU_PADRAO,
                SINAL_HANDLER_IGNORAR => 1,
                SINAL_HANDLER_ERRO => 3,
                SIGPIPE_NAO_OBSERVADO => 4,
                _ => 2,
            };
            std::process::exit(codigo);
        }

        // O pai passa a ignorar SIGPIPE — é a disposição que sobreviveria ao
        // `exec` se o runtime não restaurasse a padrão no filho.
        preparar_disposicao_sinais();

        let mut comando = comando_saneado(std::env::current_exe().expect("binário de teste"));
        let saida = comando
            .args([
                "--exact",
                "tests::filho_de_comando_saneado_observa_disposicao_padrao",
                "--nocapture",
            ])
            .env("PINKER_RT_TESTE_SIGPIPE_FILHO", "1")
            .output()
            .expect("executar filho de teste");
        assert_eq!(
            saida.status.code(),
            Some(OBSERVOU_PADRAO),
            "o filho precisa observar SIG_DFL \
             (0=teste não rodou, 1=SIG_IGN, 2=handler, 3=erro, 4=sonda não rodou)"
        );
    }

    /// Falha ao preparar a disposição do filho vira erro, nunca silêncio.
    ///
    /// `SIGKILL` não aceita mudança de disposição, então serve de gatilho real
    /// para o caminho de erro sem exigir superfície pública nem variável de
    /// ambiente de produção.
    #[cfg(unix)]
    #[test]
    fn falha_ao_restaurar_disposicao_vira_erro() {
        const SINAL_SIGKILL: i32 = 9;

        let erro = restaurar_disposicao_padrao(SINAL_SIGKILL)
            .expect_err("SIGKILL não aceita mudança de disposição");
        assert!(
            erro.raw_os_error().is_some(),
            "a falha precisa carregar o erro do sistema: {erro}"
        );

        // A tentativa falha não pode ter mexido na disposição do processo.
        // SAFETY: FFI pura com handler constante, sem ponteiros.
        let observada = unsafe { signal(SINAL_SIGPIPE, SINAL_HANDLER_IGNORAR) };
        assert_eq!(
            observada, SINAL_HANDLER_IGNORAR,
            "a falha não pode alterar a disposição de SIGPIPE do processo"
        );
    }
    // @pinker-nav:end evidencia.runtime.sigpipe-disposicao

    // @pinker-nav:start evidencia.runtime.texto-verso
    // @pinker-nav:domain texto
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Helper `verso_de`, que monta blocos de verso em memória para toda a suíte interna, e evidência das operações de texto: `pinker_verso_tamanho` conta code points Unicode, `pinker_verso_juntar` concatena em bloco novo e `pinker_verso_igual` compara por conteúdo.
    fn verso_de(texto: &str) -> Vec<u8> {
        let mut bloco = Vec::with_capacity(texto.len() + 8);
        bloco.extend_from_slice(&(texto.len() as u64).to_ne_bytes());
        bloco.extend_from_slice(texto.as_bytes());
        bloco
    }

    #[test]
    fn verso_tamanho_conta_code_points_unicode() {
        let ascii = verso_de("rosa");
        let acentuado = verso_de("coração");
        unsafe {
            assert_eq!(pinker_verso_tamanho(ascii.as_ptr()), 4);
            // 7 caracteres, 9 bytes — espelha chars().count() do interpretador.
            assert_eq!(pinker_verso_tamanho(acentuado.as_ptr()), 7);
        }
    }

    #[test]
    fn verso_juntar_concatena_em_novo_bloco() {
        let a = verso_de("ola ");
        let b = verso_de("rosa");
        unsafe {
            let junto = pinker_verso_juntar(a.as_ptr(), b.as_ptr());
            assert!(!junto.is_null());
            assert_eq!(verso_bytes(junto), b"ola rosa");
            assert_eq!(pinker_verso_tamanho(junto), 8);
            pinker_liberar(junto);
        }
    }

    #[test]
    fn verso_igual_compara_conteudo() {
        let a = verso_de("pinker");
        let b = verso_de("pinker");
        let c = verso_de("rosa");
        unsafe {
            assert_eq!(pinker_verso_igual(a.as_ptr(), b.as_ptr()), 1);
            assert_eq!(pinker_verso_igual(a.as_ptr(), c.as_ptr()), 0);
        }
    }
    // @pinker-nav:end evidencia.runtime.texto-verso

    // @pinker-nav:start evidencia.runtime.listas-dinamicas
    // @pinker-nav:domain listas
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória das listas dinâmicas do runtime: anexar/obter/tamanho, crescimento além da capacidade inicial, `pinker_lista_definir` substituindo elemento, `pinker_lista_inserir` deslocando o sufixo e `pinker_lista_tirar_ultimo` removendo e devolvendo o topo.
    #[test]
    fn lista_anexar_obter_e_tamanho() {
        let l = pinker_lista_criar();
        assert!(!l.is_null());
        unsafe {
            assert_eq!(pinker_lista_tamanho(l), 0);
            pinker_lista_anexar(l, 7);
            pinker_lista_anexar(l, 21);
            assert_eq!(pinker_lista_tamanho(l), 2);
            assert_eq!(pinker_lista_obter(l, 0), 7);
            assert_eq!(pinker_lista_obter(l, 1), 21);
        }
    }

    #[test]
    fn lista_cresce_alem_da_capacidade_inicial() {
        let l = pinker_lista_criar();
        unsafe {
            for i in 0..100 {
                pinker_lista_anexar(l, i * 3);
            }
            assert_eq!(pinker_lista_tamanho(l), 100);
            for i in 0..100 {
                assert_eq!(pinker_lista_obter(l, i), i * 3);
            }
        }
    }

    #[test]
    fn lista_definir_substitui_elemento() {
        let l = pinker_lista_criar();
        unsafe {
            pinker_lista_anexar(l, 1);
            pinker_lista_anexar(l, 2);
            pinker_lista_definir(l, 1, 42);
            assert_eq!(pinker_lista_obter(l, 1), 42);
            assert_eq!(pinker_lista_tamanho(l), 2);
        }
    }

    #[test]
    fn lista_inserir_desloca_sufixo() {
        let l = pinker_lista_criar();
        unsafe {
            pinker_lista_anexar(l, 1);
            pinker_lista_anexar(l, 3);
            pinker_lista_inserir(l, 1, 2);
            assert_eq!(pinker_lista_tamanho(l), 3);
            assert_eq!(pinker_lista_obter(l, 0), 1);
            assert_eq!(pinker_lista_obter(l, 1), 2);
            assert_eq!(pinker_lista_obter(l, 2), 3);
            pinker_lista_inserir(l, 0, 0);
            assert_eq!(pinker_lista_obter(l, 0), 0);
            pinker_lista_inserir(l, 4, 4);
            assert_eq!(pinker_lista_obter(l, 4), 4);
        }
    }

    #[test]
    fn lista_tirar_ultimo_remove_e_devolve() {
        let l = pinker_lista_criar();
        unsafe {
            pinker_lista_anexar(l, 10);
            pinker_lista_anexar(l, 20);
            assert_eq!(pinker_lista_tirar_ultimo(l), 20);
            assert_eq!(pinker_lista_tamanho(l), 1);
            assert_eq!(pinker_lista_tirar_ultimo(l), 10);
            assert_eq!(pinker_lista_tamanho(l), 0);
        }
    }
    // @pinker-nav:end evidencia.runtime.listas-dinamicas

    // @pinker-nav:start evidencia.runtime.mapas-dinamicos
    // @pinker-nav:domain mapas
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória dos mapas dinâmicos do runtime: definição/obtenção/`tem`/tamanho com chave bombom, comparação por conteúdo com chave verso, remoção preservando a ordem e ausência silenciosa, e crescimento além da capacidade inicial.
    #[test]
    fn mapa_chave_bombom_definir_obter_tem_tamanho() {
        let m = pinker_mapa_criar_chave_bombom();
        unsafe {
            pinker_mapa_definir(m, 1, 10);
            pinker_mapa_definir(m, 2, 20);
            pinker_mapa_definir(m, 1, 11);
            assert_eq!(pinker_mapa_tamanho(m), 2);
            assert_eq!(pinker_mapa_obter(m, 1), 11);
            assert_eq!(pinker_mapa_obter(m, 2), 20);
            assert_eq!(pinker_mapa_tem(m, 2), 1);
            assert_eq!(pinker_mapa_tem(m, 3), 0);
        }
    }

    #[test]
    fn mapa_chave_verso_compara_por_conteudo() {
        let m = pinker_mapa_criar_chave_verso();
        let chave_a = verso_de("rosa");
        let chave_a_clone = verso_de("rosa");
        let chave_b = verso_de("pinker");
        unsafe {
            pinker_mapa_definir(m, chave_a.as_ptr() as u64, 7);
            // Ponteiro diferente, mesmo conteúdo: precisa achar a entrada.
            assert_eq!(pinker_mapa_tem(m, chave_a_clone.as_ptr() as u64), 1);
            assert_eq!(pinker_mapa_obter(m, chave_a_clone.as_ptr() as u64), 7);
            assert_eq!(pinker_mapa_tem(m, chave_b.as_ptr() as u64), 0);
            pinker_mapa_definir(m, chave_a_clone.as_ptr() as u64, 8);
            assert_eq!(pinker_mapa_tamanho(m), 1);
            assert_eq!(pinker_mapa_obter(m, chave_a.as_ptr() as u64), 8);
        }
    }

    #[test]
    fn mapa_remover_preserva_ordem_e_ausencia_e_silenciosa() {
        let m = pinker_mapa_criar_chave_bombom();
        unsafe {
            pinker_mapa_definir(m, 1, 10);
            pinker_mapa_definir(m, 2, 20);
            pinker_mapa_definir(m, 3, 30);
            pinker_mapa_remover(m, 2);
            assert_eq!(pinker_mapa_tamanho(m), 2);
            assert_eq!(pinker_mapa_tem(m, 2), 0);
            pinker_mapa_remover(m, 99);
            assert_eq!(pinker_mapa_tamanho(m), 2);
            let cursor = pinker_mapa_iterador_criar(m);
            assert_eq!(pinker_mapa_iterador_proxima(cursor), 1);
            assert_eq!(pinker_mapa_iterador_proxima(cursor), 3);
        }
    }

    #[test]
    fn mapa_cresce_alem_da_capacidade_inicial() {
        let m = pinker_mapa_criar_chave_bombom();
        unsafe {
            for i in 0..50 {
                pinker_mapa_definir(m, i, i * 2);
            }
            assert_eq!(pinker_mapa_tamanho(m), 50);
            for i in 0..50 {
                assert_eq!(pinker_mapa_obter(m, i), i * 2);
            }
        }
    }
    // @pinker-nav:end evidencia.runtime.mapas-dinamicos

    // @pinker-nav:start evidencia.runtime.leques-carga
    // @pinker-nav:domain leques
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória dos leques (variantes com carga) do runtime: criação com tag e leitura de cargas posicionais, aninhamento de leque dentro de leque habilitando recursão, e crescimento além da capacidade inicial.
    #[test]
    fn leque_criar_anexar_tag_e_carga() {
        unsafe {
            let l = pinker_leque_criar_0(2);
            let l = pinker_leque_anexar(l, 42);
            let l = pinker_leque_anexar(l, 7);
            assert_eq!(pinker_leque_tag(l), 2);
            assert_eq!(pinker_leque_carga(l, 2, 0), 42);
            assert_eq!(pinker_leque_carga(l, 2, 1), 7);
        }
    }

    #[test]
    fn leque_aninhado_habilita_recursao() {
        unsafe {
            // Expr.Lit(21) dentro de Expr.Dobro(Expr) — carga é outro leque.
            let lit = pinker_leque_criar_0(0);
            let lit = pinker_leque_anexar(lit, 21);
            let dobro = pinker_leque_criar_0(1);
            let dobro = pinker_leque_anexar(dobro, lit as u64);
            let interno = pinker_leque_carga(dobro, 1, 0) as *mut u8;
            assert_eq!(pinker_leque_tag(interno), 0);
            assert_eq!(pinker_leque_carga(interno, 0, 0), 21);
        }
    }

    #[test]
    fn leque_cresce_alem_da_capacidade_inicial() {
        unsafe {
            let mut l = pinker_leque_criar_0(9);
            for i in 0..10 {
                l = pinker_leque_anexar(l, i * 5);
            }
            for i in 0..10 {
                assert_eq!(pinker_leque_carga(l, 9, i), i * 5);
            }
        }
    }
    // @pinker-nav:end evidencia.runtime.leques-carga

    // @pinker-nav:start evidencia.runtime.json-familia
    // @pinker-nav:domain dados
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência interna da família JSON pela ABI nativa, sem passar por ELF: a leitura devolve `Resultado` pelo mesmo leque do código gerado e a carga de sucesso é o handle da raiz; o nesting é atravessado por handle até a folha; a serialização sai determinística por ordem de chave; dado externo malformado vira variante de erro em vez de abortar; e o recorte plano histórico preserva `u64::MAX` no parse e na emissão, sem cast para `i64`. É a prova de que os símbolos existem e funcionam no runtime, não apenas de que o backend os emite.
    #[test]
    fn parte_e1_json_resultado_e_nesting_pela_abi_nativa() {
        let texto = verso_alocar(r#"{"a":[{"b":-7}],"z":true}"#);
        let resultado = unsafe { pinker_json_ler_resultado(texto.cast_const()) };
        assert_eq!(unsafe { pinker_leque_tag(resultado) }, RESULTADO_TAG_OK);
        let raiz = unsafe { pinker_leque_carga(resultado, RESULTADO_TAG_OK, 0) };

        // objeto -> lista -> objeto, tudo pelo mesmo mecanismo por handle.
        let chave_a = verso_alocar("a");
        let lista = unsafe { pinker_json_objeto_obter(raiz, chave_a.cast_const()) };
        assert_eq!(pinker_json_lista_tamanho(lista), 1);
        let primeiro = pinker_json_lista_obter(lista, 0);
        let chave_b = verso_alocar("b");
        let folha = unsafe { pinker_json_objeto_obter(primeiro, chave_b.cast_const()) };
        assert_eq!(pinker_json_numero(folha), -7);

        // Serialização determinística por ordem de chave.
        let emitido = pinker_json_emitir(raiz);
        assert_eq!(
            unsafe { verso_str(emitido.cast_const()) },
            r#"{"a":[{"b":-7}],"z":true}"#
        );
        unsafe {
            pinker_liberar(texto);
            pinker_liberar(chave_a);
            pinker_liberar(chave_b);
            pinker_liberar(emitido);
        }
    }

    #[test]
    fn parte_e1_json_malformado_vira_valor_e_nao_aborta() {
        let texto = verso_alocar(r#"{"a":}"#);
        let resultado = unsafe { pinker_json_ler_resultado(texto.cast_const()) };
        assert_eq!(unsafe { pinker_leque_tag(resultado) }, RESULTADO_TAG_ERRO);
        unsafe { pinker_liberar(texto) };
    }

    /// O recorte plano histórico vai até `u64::MAX`, que o domínio adulto
    /// recusa. Se alguém unificar os dois domínios, esta evidência quebra.
    #[test]
    fn parte_e1_json_plano_preserva_u64_max_pela_abi_nativa() {
        let origem = r#"{"x":18446744073709551615}"#;
        let texto = verso_alocar(origem);
        let mapa = unsafe { pinker_json_plano_ler(texto.cast_const()) };
        let chave = verso_alocar("x");
        assert_eq!(
            unsafe { pinker_mapa_obter(mapa, chave as u64) },
            u64::MAX,
            "o recorte plano precisa preservar u64::MAX no parse"
        );
        let emitido = unsafe { pinker_json_plano_emitir(mapa) };
        let saida = unsafe { verso_str(emitido.cast_const()) };
        assert_eq!(
            saida, origem,
            "u64::MAX nao pode truncar nem trocar de sinal"
        );
        assert!(!saida.contains('-'), "sinal apareceu do nada");

        // O mesmo documento é recusado pelo dominio adulto, como valor.
        let adulto = unsafe { pinker_json_ler_resultado(texto.cast_const()) };
        assert_eq!(unsafe { pinker_leque_tag(adulto) }, RESULTADO_TAG_ERRO);
        unsafe {
            pinker_liberar(texto);
            pinker_liberar(chave);
            pinker_liberar(emitido);
        }
    }
    // @pinker-nav:end evidencia.runtime.json-familia

    // @pinker-nav:start evidencia.runtime.mapas-iterador-snapshot
    // @pinker-nav:domain mapas
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória do iterador de mapas: `pinker_mapa_iterador_criar` fixa um snapshot das chaves, de modo que definições e remoções posteriores não afetam a sequência devolvida por `pinker_mapa_iterador_proxima`; fecha fisicamente o módulo de testes internos do runtime.
    #[test]
    fn mapa_iterador_usa_snapshot_das_chaves() {
        let m = pinker_mapa_criar_chave_bombom();
        unsafe {
            pinker_mapa_definir(m, 1, 10);
            pinker_mapa_definir(m, 2, 20);
            let cursor = pinker_mapa_iterador_criar(m);
            // Mutação após o cursor não afeta o snapshot.
            pinker_mapa_definir(m, 3, 30);
            pinker_mapa_remover(m, 1);
            assert_eq!(pinker_mapa_iterador_proxima(cursor), 1);
            assert_eq!(pinker_mapa_iterador_proxima(cursor), 2);
        }
    }
    // @pinker-nav:end evidencia.runtime.mapas-iterador-snapshot

    // @pinker-nav:start evidencia.runtime.unioes-snapshot
    // @pinker-nav:domain unioes
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória do descritor de união estrutural: layout do bloco único com payload alinhado, snapshot independente da origem e das extrações, alinhamento de dezesseis honrado, recusa de layout fora dos limites documentados e, por processo filho, diagnóstico controlado para falha de alocação injetada só em teste e para handle desconhecido nunca dereferenciado.

    /// HR3: o layout do bloco único {cabeçalho, padding, payload} respeita o
    /// alinhamento pedido, cabe no bloco e usa aritmética checada.
    #[test]
    fn uniao_layout_de_alocacao_respeita_alinhamento_e_limites() {
        for (size, align) in [(1_u64, 1_u64), (9, 8), (16, 16), (24, 8), (4096, 16)] {
            let (offset, total, total_align) =
                union_allocation_layout(size, align).expect("layout finito");
            assert_eq!(offset % align, 0, "payload alinhado: {size}/{align}");
            assert!(offset >= std::mem::size_of::<PinkerUnionDescriptor>() as u64);
            assert!(total >= offset + size);
            assert_eq!(total % total_align, 0);
            assert!(total_align >= align);
        }
        assert!(union_allocation_layout(u64::MAX, 8).is_none(), "overflow");
    }

    /// HR3: o descritor guarda um snapshot integral e independente. Mudar a
    /// origem depois da criação não muda o payload observado, cada extração
    /// escreve num destino distinto e o descritor não é alterado.
    #[test]
    fn uniao_snapshot_e_independente_da_origem_e_das_extracoes() {
        let mut origem: [u8; 24] = [0; 24];
        for (indice, byte) in origem.iter_mut().enumerate() {
            *byte = indice as u8;
        }
        let handle = unsafe { pinker_uniao_criar(7, 3, 24, 8, origem.as_ptr()) };
        assert!(!handle.is_null());

        // A origem muda inteira depois da injeção.
        origem = [0xAA; 24];

        let mut primeiro: [u8; 24] = [0; 24];
        let mut segundo: [u8; 24] = [0; 24];
        unsafe {
            pinker_uniao_copiar_payload(handle, 7, 3, 24, 8, primeiro.as_mut_ptr());
            pinker_uniao_copiar_payload(handle, 7, 3, 24, 8, segundo.as_mut_ptr());
        }
        for indice in 0..24usize {
            assert_eq!(primeiro[indice], indice as u8, "snapshot preservado");
            assert_eq!(segundo[indice], indice as u8, "extrações iguais");
        }
        assert_eq!(origem[0], 0xAA, "a origem permanece independente");

        // Mudar o primeiro destino não contamina o descritor.
        primeiro[0] = 0xFF;
        let mut terceiro: [u8; 24] = [0; 24];
        unsafe { pinker_uniao_copiar_payload(handle, 7, 3, 24, 8, terceiro.as_mut_ptr()) };
        assert_eq!(terceiro[0], 0, "o snapshot não foi alterado pelo binding");

        assert_eq!(unsafe { pinker_uniao_tag(handle, 7) }, 3);
    }

    /// HR3: o alinhamento pedido é honrado pelo storage do payload.
    #[test]
    fn uniao_storage_respeita_alinhamento_de_dezesseis() {
        let origem: [u8; 16] = [1; 16];
        let handle = unsafe { pinker_uniao_criar(1, 0, 16, 16, origem.as_ptr()) };
        let descriptor = unsafe { &*(handle as *const PinkerUnionDescriptor) };
        let payload = unsafe { handle.add(descriptor.payload_offset as usize) };
        assert_eq!(payload as usize % 16, 0, "payload alinhado a 16");
    }

    /// HR3: layout fora dos limites documentados é recusado sem alocar.
    #[test]
    fn uniao_layout_invalido_e_reconhecido_antes_de_alocar() {
        assert!(!union_layout_valid(0, 8), "tamanho zero");
        assert!(
            !union_layout_valid(MAX_UNION_PAYLOAD_BYTES + 1, 8),
            "acima do limite"
        );
        assert!(!union_layout_valid(8, 0), "alinhamento zero");
        assert!(
            !union_layout_valid(8, 3),
            "alinhamento não potência de dois"
        );
        assert!(
            !union_layout_valid(8, MAX_UNION_PAYLOAD_ALIGN * 2),
            "alinhamento acima do limite"
        );
        assert!(union_layout_valid(MAX_UNION_PAYLOAD_BYTES, 16), "no limite");
    }

    #[cfg(unix)]
    fn filho_uniao(modo: &str, teste: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().expect("binário de teste"))
            .args(["--exact", teste, "--nocapture"])
            .env("PINKER_RT_TESTE_UNIAO_FILHO", modo)
            .output()
            .expect("executar filho de teste")
    }

    /// Limites minúsculos usados para provar cada fronteira sem alocar nada.
    const LIMITES_DE_TESTE: UnionBudgetLimits = UnionBudgetLimits {
        max_descriptors: 2,
        max_payload_bytes: 40,
        max_metadata_bytes: 2 * UNION_DESCRIPTOR_METADATA_BYTES,
    };

    /// HR3/GAP4: o último descritor permitido é aceito e o primeiro acima do
    /// teto é recusado, sem materializar um milhão de descritores.
    #[test]
    fn uniao_budget_fronteira_de_descritores() {
        let no_limite = UnionBudget {
            descriptors: LIMITES_DE_TESTE.max_descriptors - 1,
            payload_bytes: 0,
            metadata_bytes: 0,
        };
        let aceito = union_budget_reserve(no_limite, LIMITES_DE_TESTE, 8)
            .expect("o último descritor permitido é aceito");
        assert_eq!(aceito.descriptors, LIMITES_DE_TESTE.max_descriptors);

        let acima = UnionBudget {
            descriptors: LIMITES_DE_TESTE.max_descriptors,
            ..no_limite
        };
        assert_eq!(
            union_budget_reserve(acima, LIMITES_DE_TESTE, 8),
            Err(UnionBudgetError::Descriptors),
            "o primeiro descritor acima do limite é recusado"
        );
    }

    /// HR3/GAP4: o último byte de payload permitido é aceito e o primeiro acima
    /// do teto é recusado.
    #[test]
    fn uniao_budget_fronteira_de_bytes_de_payload() {
        let base = UnionBudget {
            descriptors: 0,
            payload_bytes: LIMITES_DE_TESTE.max_payload_bytes - 8,
            metadata_bytes: 0,
        };
        let aceito = union_budget_reserve(base, LIMITES_DE_TESTE, 8)
            .expect("o último byte permitido é aceito");
        assert_eq!(aceito.payload_bytes, LIMITES_DE_TESTE.max_payload_bytes);

        assert_eq!(
            union_budget_reserve(base, LIMITES_DE_TESTE, 9),
            Err(UnionBudgetError::PayloadBytes),
            "o primeiro byte acima do limite é recusado"
        );
    }

    /// HR3/GAP4: a última metadata permitida é aceita e a primeira acima do teto
    /// é recusada, mesmo com payload dentro do orçamento.
    #[test]
    fn uniao_budget_fronteira_de_metadata() {
        let base = UnionBudget {
            descriptors: 0,
            payload_bytes: 0,
            metadata_bytes: LIMITES_DE_TESTE.max_metadata_bytes - UNION_DESCRIPTOR_METADATA_BYTES,
        };
        let aceito = union_budget_reserve(base, LIMITES_DE_TESTE, 8)
            .expect("a última metadata permitida é aceita");
        assert_eq!(aceito.metadata_bytes, LIMITES_DE_TESTE.max_metadata_bytes);

        let acima = UnionBudget {
            metadata_bytes: LIMITES_DE_TESTE.max_metadata_bytes,
            ..base
        };
        assert_eq!(
            union_budget_reserve(acima, LIMITES_DE_TESTE, 8),
            Err(UnionBudgetError::MetadataBytes),
            "a primeira metadata acima do limite é recusada"
        );
    }

    /// HR3/GAP4: cada contador detecta overflow antes de qualquer comparação com
    /// o teto, e o overflow é diagnóstico, não pânico.
    #[test]
    fn uniao_budget_detecta_overflow_de_cada_contador() {
        let ilimitado = UnionBudgetLimits {
            max_descriptors: u64::MAX,
            max_payload_bytes: u64::MAX,
            max_metadata_bytes: u64::MAX,
        };
        assert_eq!(
            union_budget_reserve(
                UnionBudget {
                    descriptors: u64::MAX,
                    payload_bytes: 0,
                    metadata_bytes: 0,
                },
                ilimitado,
                8,
            ),
            Err(UnionBudgetError::DescriptorOverflow)
        );
        assert_eq!(
            union_budget_reserve(
                UnionBudget {
                    descriptors: 0,
                    payload_bytes: u64::MAX,
                    metadata_bytes: 0,
                },
                ilimitado,
                1,
            ),
            Err(UnionBudgetError::PayloadOverflow)
        );
        assert_eq!(
            union_budget_reserve(
                UnionBudget {
                    descriptors: 0,
                    payload_bytes: 0,
                    metadata_bytes: u64::MAX,
                },
                ilimitado,
                8,
            ),
            Err(UnionBudgetError::MetadataOverflow)
        );
    }

    /// HR3/GAP4: uma reserva recusada não altera o orçamento corrente — nem no
    /// contador que passou antes do que falhou.
    #[test]
    fn uniao_budget_recusa_e_atomica() {
        let antes = UnionBudget {
            descriptors: 0,
            payload_bytes: LIMITES_DE_TESTE.max_payload_bytes,
            metadata_bytes: 0,
        };
        let copia = antes;
        assert_eq!(
            union_budget_reserve(antes, LIMITES_DE_TESTE, 1),
            Err(UnionBudgetError::PayloadBytes),
            "a recusa vem do contador de bytes, depois de descritores passar"
        );
        assert_eq!(
            antes, copia,
            "o orçamento de entrada permanece byte a byte idêntico após a recusa"
        );

        // O runtime de produção usa os limites canônicos; a unidade é a mesma.
        assert_eq!(UNION_BUDGET_LIMITS.max_descriptors, MAX_UNION_DESCRIPTORS);
        assert_eq!(
            UNION_BUDGET_LIMITS.max_payload_bytes,
            MAX_UNION_TOTAL_PAYLOAD_BYTES
        );
        assert_eq!(
            UNION_BUDGET_LIMITS.max_metadata_bytes,
            MAX_UNION_METADATA_BYTES
        );
    }

    /// HR3/GAP3: o binding extraído tem storage próprio. Modificar o primeiro
    /// destino não altera o snapshot, a segunda extração recebe os bytes
    /// originais e os dois destinos ocupam endereços distintos.
    #[test]
    fn uniao_mutacao_do_binding_extraido_nao_altera_o_snapshot() {
        let origem: [u8; 32] = std::array::from_fn(|indice| (indice as u8).wrapping_mul(3));
        let handle = unsafe { pinker_uniao_criar(21, 1, 32, 8, origem.as_ptr()) };
        assert!(!handle.is_null());

        let mut a: [u8; 32] = [0; 32];
        unsafe { pinker_uniao_copiar_payload(handle, 21, 1, 32, 8, a.as_mut_ptr()) };
        assert_eq!(a, origem, "a primeira extração devolve o snapshot integral");

        // Mutação integral do binding extraído.
        a = [0xC3; 32];

        let mut b: [u8; 32] = [0; 32];
        unsafe { pinker_uniao_copiar_payload(handle, 21, 1, 32, 8, b.as_mut_ptr()) };
        assert_eq!(b, origem, "a segunda extração conserva os bytes originais");
        assert_ne!(
            a.as_ptr() as usize,
            b.as_ptr() as usize,
            "as duas extrações usam storages distintos"
        );
        assert_eq!(unsafe { pinker_uniao_tag(handle, 21) }, 1);
    }

    /// HR3: falha de alocação do payload produz diagnóstico controlado, não
    /// abort de alocador. A injeção é exclusivamente interna ao teste — não há
    /// variável de ambiente que altere a política documentada em execução.
    #[cfg(unix)]
    #[test]
    fn uniao_falha_de_alocacao_produz_diagnostico_controlado() {
        if std::env::var_os("PINKER_RT_TESTE_UNIAO_FILHO").is_some() {
            FALHA_ALOCACAO_UNIAO.store(true, std::sync::atomic::Ordering::SeqCst);
            let origem: [u8; 8] = [0; 8];
            unsafe { pinker_uniao_criar(1, 0, 8, 8, origem.as_ptr()) };
            unreachable!("a criação deveria ter terminado o processo");
        }
        let saida = filho_uniao(
            "alocacao",
            "tests::uniao_falha_de_alocacao_produz_diagnostico_controlado",
        );
        assert_eq!(saida.status.code(), Some(1), "saída controlada");
        let stderr = String::from_utf8_lossy(&saida.stderr);
        assert!(
            stderr.contains("alocação de descritor de união estrutural falhou"),
            "{stderr}"
        );
    }

    /// HR3: cada validação da extração é obrigatória e produz diagnóstico
    /// próprio. Um único teste percorre os cinco estados inválidos porque cada
    /// um termina o processo; o modo vem do ambiente apenas para selecionar o
    /// cenário no processo filho, nunca para alterar política.
    #[cfg(unix)]
    #[test]
    fn uniao_extracao_valida_uniao_tag_tamanho_alinhamento_e_destino() {
        if let Some(modo) = std::env::var_os("PINKER_RT_TESTE_UNIAO_FILHO") {
            let origem: [u8; 8] = [9; 8];
            let handle = unsafe { pinker_uniao_criar(11, 4, 8, 8, origem.as_ptr()) };
            let mut destino: [u8; 8] = [0; 8];
            let modo = modo.to_string_lossy().to_string();
            unsafe {
                match modo.as_str() {
                    "uniao" => {
                        pinker_uniao_copiar_payload(handle, 12, 4, 8, 8, destino.as_mut_ptr())
                    }
                    "tag" => pinker_uniao_copiar_payload(handle, 11, 5, 8, 8, destino.as_mut_ptr()),
                    "tamanho" => {
                        pinker_uniao_copiar_payload(handle, 11, 4, 4, 8, destino.as_mut_ptr())
                    }
                    "alinhamento" => {
                        pinker_uniao_copiar_payload(handle, 11, 4, 8, 4, destino.as_mut_ptr())
                    }
                    "destino" => {
                        pinker_uniao_copiar_payload(handle, 11, 4, 8, 8, std::ptr::null_mut())
                    }
                    "tag_identidade" => {
                        pinker_uniao_tag(handle, 12);
                    }
                    outro => panic!("modo desconhecido: {outro}"),
                }
            }
            unreachable!("a operação deveria ter terminado o processo");
        }
        for (modo, esperado) in [
            ("uniao", "identidade de união divergente"),
            ("tag", "tag incompatível"),
            ("tamanho", "tamanho divergente"),
            ("alinhamento", "alinhamento divergente"),
            ("destino", "destino nulo"),
            (
                "tag_identidade",
                "leitura de tag com identidade de união divergente",
            ),
        ] {
            let saida = filho_uniao(
                modo,
                "tests::uniao_extracao_valida_uniao_tag_tamanho_alinhamento_e_destino",
            );
            assert_eq!(
                saida.status.code(),
                Some(1),
                "modo {modo}: saída controlada"
            );
            let stderr = String::from_utf8_lossy(&saida.stderr);
            assert!(stderr.contains(esperado), "modo {modo}: {stderr}");
        }
    }

    /// HR3/GAP4: quando a alocação falha, nenhum handle é publicado. O filho
    /// imprimiria o handle em stdout se a criação tivesse devolvido; a saída
    /// vazia é a evidência de que nada foi exposto nem registrado.
    #[cfg(unix)]
    #[test]
    fn uniao_falha_de_alocacao_nao_publica_handle() {
        if std::env::var_os("PINKER_RT_TESTE_UNIAO_FILHO").is_some() {
            FALHA_ALOCACAO_UNIAO.store(true, std::sync::atomic::Ordering::SeqCst);
            let origem: [u8; 8] = [0; 8];
            let handle = unsafe { pinker_uniao_criar(31, 0, 8, 8, origem.as_ptr()) };
            println!("handle_publicado={}", handle as usize);
            unreachable!("a criação deveria ter terminado o processo");
        }
        let saida = filho_uniao(
            "alocacao_sem_handle",
            "tests::uniao_falha_de_alocacao_nao_publica_handle",
        );
        assert_eq!(saida.status.code(), Some(1), "saída controlada");
        let stdout = String::from_utf8_lossy(&saida.stdout);
        assert!(
            !stdout.contains("handle_publicado="),
            "nenhum handle pode ser publicado: {stdout}"
        );
        let stderr = String::from_utf8_lossy(&saida.stderr);
        assert!(
            stderr.contains("alocação de descritor de união estrutural falhou"),
            "{stderr}"
        );
    }

    /// HR3: um handle que não foi criado por este runtime nunca é
    /// dereferenciado.
    #[cfg(unix)]
    #[test]
    fn uniao_handle_desconhecido_nao_e_dereferenciado() {
        if std::env::var_os("PINKER_RT_TESTE_UNIAO_FILHO").is_some() {
            let mut destino: [u8; 8] = [0; 8];
            unsafe {
                pinker_uniao_copiar_payload(
                    0x1234_5678_9abc_def0_u64 as *mut u8,
                    1,
                    0,
                    8,
                    8,
                    destino.as_mut_ptr(),
                )
            };
            unreachable!("a extração deveria ter terminado o processo");
        }
        let saida = filho_uniao(
            "handle",
            "tests::uniao_handle_desconhecido_nao_e_dereferenciado",
        );
        assert_eq!(saida.status.code(), Some(1), "saída controlada");
        let stderr = String::from_utf8_lossy(&saida.stderr);
        assert!(stderr.contains("handle desconhecido"), "{stderr}");
    }
}
// @pinker-nav:end evidencia.runtime.unioes-snapshot
