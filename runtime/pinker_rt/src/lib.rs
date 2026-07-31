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

use std::alloc::{alloc, dealloc, Layout};
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
// @pinker-nav:summary Alocador manual e regiões públicas: pinker_alocar reserva bloco alinhado a 16 bytes; a superfície pública rejeita zero/overflow/falha, zera bytes, registra identidade/base/tamanho/vida e mantém blocos liberados em quarentena. Acesso, liberação e derivação de ponteiro validam proveniência, domínio, alinhamento, limites, use-after-free, double free e escapes mesmo quando o endereço derivado já não cai dentro da região.
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

#[derive(Clone, Copy)]
struct AlocacaoPublica {
    identidade: u64,
    base: usize,
    tamanho: usize,
    reservado: usize,
    viva: bool,
}

const PAGINA_PUBLICA: usize = 4096;
const MAX_IDENTIDADES_PUBLICAS: usize = 1_000_000;
const MAX_METADATA_PUBLICA_BYTES: usize =
    MAX_IDENTIDADES_PUBLICAS * std::mem::size_of::<AlocacaoPublica>();
const MAX_QUARENTENA_FISICA_BYTES: usize = 0;
const MAX_ESPACO_VIRTUAL_PUBLICO_BYTES: usize = 8 * 1024 * 1024 * 1024;

struct MemoriaPublica {
    arena_base: usize,
    proximo_offset: usize,
    proxima_identidade: u64,
    alocacoes: Vec<AlocacaoPublica>,
}

#[cfg(target_os = "linux")]
fn reservar_arena_publica() -> usize {
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
    const PROT_NONE: i32 = 0;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_NORESERVE: i32 = 0x4000;
    let base = unsafe {
        mmap(
            std::ptr::null_mut(),
            MAX_ESPACO_VIRTUAL_PUBLICO_BYTES,
            PROT_NONE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE,
            -1,
            0,
        )
    };
    if base as usize == usize::MAX {
        erro_memoria_publica("arena virtual pública indisponível");
    }
    base as usize
}

#[cfg(not(target_os = "linux"))]
fn reservar_arena_publica() -> usize {
    erro_memoria_publica("arena pública limitada indisponível neste target")
}

fn memoria_publica() -> &'static Mutex<MemoriaPublica> {
    static MEMORIA: OnceLock<Mutex<MemoriaPublica>> = OnceLock::new();
    MEMORIA.get_or_init(|| {
        Mutex::new(MemoriaPublica {
            arena_base: reservar_arena_publica(),
            proximo_offset: 0,
            proxima_identidade: 1,
            alocacoes: Vec::new(),
        })
    })
}

#[cfg(target_os = "linux")]
fn comprometer_paginas_publicas(base: usize, tamanho: usize) -> Result<(), ()> {
    use std::ffi::c_void;

    extern "C" {
        fn mprotect(address: *mut c_void, length: usize, protection: i32) -> i32;
    }
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    (unsafe { mprotect(base as *mut c_void, tamanho, PROT_READ | PROT_WRITE) } == 0)
        .then_some(())
        .ok_or(())
}

#[cfg(not(target_os = "linux"))]
fn comprometer_paginas_publicas(_base: usize, _tamanho: usize) -> Result<(), ()> {
    Err(())
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

fn intervalo_publico_contido(
    inicio_regiao: usize,
    tamanho_regiao: usize,
    inicio_acesso: usize,
    largura_acesso: usize,
) -> Result<bool, ()> {
    let fim_regiao = inicio_regiao.checked_add(tamanho_regiao).ok_or(())?;
    let fim_acesso = inicio_acesso.checked_add(largura_acesso).ok_or(())?;
    Ok(inicio_acesso >= inicio_regiao && fim_acesso <= fim_regiao)
}

/// Fase 246: entrada pública de `alocar`. Diferentemente de
/// `pinker_alocar`, rejeita zero, valida toda conversão, zera os bytes visíveis e
/// registra ownership para que `liberar` possa validar a origem.
#[no_mangle]
pub extern "C" fn pinker_publico_alocar(tamanho: u64) -> *mut u8 {
    debug_assert_eq!(
        MAX_METADATA_PUBLICA_BYTES,
        MAX_IDENTIDADES_PUBLICAS * std::mem::size_of::<AlocacaoPublica>()
    );
    debug_assert_eq!(MAX_QUARENTENA_FISICA_BYTES, 0);
    if tamanho == 0 {
        erro_memoria_publica("'alocar' rejeita tamanho zero");
    }
    let tamanho_usize = usize::try_from(tamanho)
        .unwrap_or_else(|_| erro_memoria_publica("'alocar' excede a largura da plataforma"));
    if tamanho_usize > (isize::MAX as usize).saturating_sub(CABECALHO) {
        erro_memoria_publica("'alocar' excede o maior bloco representável pela plataforma");
    }
    let reservado = tamanho_usize
        .checked_add(PAGINA_PUBLICA - 1)
        .map(|valor| valor & !(PAGINA_PUBLICA - 1))
        .unwrap_or_else(|| erro_memoria_publica("overflow ao alinhar alocação pública"));
    let mut memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    if memoria.alocacoes.len() >= MAX_IDENTIDADES_PUBLICAS {
        erro_memoria_publica("limite de identidades públicas esgotado");
    }
    memoria
        .alocacoes
        .try_reserve(1)
        .unwrap_or_else(|_| erro_memoria_publica("metadata pública de alocações esgotada"));
    let identidade = memoria.proxima_identidade;
    memoria.proxima_identidade = identidade
        .checked_add(1)
        .unwrap_or_else(|| erro_memoria_publica("identidade pública de alocação esgotada"));
    let fim = memoria
        .proximo_offset
        .checked_add(reservado)
        .filter(|fim| *fim <= MAX_ESPACO_VIRTUAL_PUBLICO_BYTES)
        .unwrap_or_else(|| erro_memoria_publica("espaço virtual público esgotado"));
    let base = memoria
        .arena_base
        .checked_add(memoria.proximo_offset)
        .unwrap_or_else(|| erro_memoria_publica("overflow na arena pública"));
    comprometer_paginas_publicas(base, reservado)
        .unwrap_or_else(|_| erro_memoria_publica("'alocar' falhou ao comprometer memória"));
    let ponteiro = base as *mut u8;
    unsafe {
        ponteiro.write_bytes(0, tamanho_usize);
    }
    memoria.proximo_offset = fim;
    memoria.alocacoes.push(AlocacaoPublica {
        identidade,
        base,
        tamanho: tamanho_usize,
        reservado,
        viva: true,
    });
    ponteiro
}

/// Fase 246: entrada pública de `liberar`. Somente o ponteiro-base de uma
/// alocação pública viva é aceito; ponteiros internos e double free falham
/// deterministicamente sem tocar no allocator interno.
///
/// # Safety
///
/// `ponteiro` deve ser exatamente o endereço-base retornado por
/// `pinker_publico_alocar`. O registro interno valida essa origem antes de
/// marcar a geração como liberada; o bloco físico permanece em quarentena.
#[no_mangle]
pub unsafe extern "C" fn pinker_publico_liberar(ponteiro: *mut u8) {
    if ponteiro.is_null() {
        erro_memoria_publica("'liberar' rejeita ponteiro nulo");
    }
    let mut memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    if let Some(indice) = indice_base_publica_mais_recente(&memoria.alocacoes, ponteiro as usize) {
        let alocacao = &mut memoria.alocacoes[indice];
        if !alocacao.viva {
            erro_memoria_publica("E-RUNTIME-MEM-DOUBLE-FREE: 'liberar' detectou double free");
        }
        debug_assert!(alocacao.identidade > 0);
        debug_assert!(alocacao.tamanho > 0);
        descomprometer_paginas_publicas(alocacao.base, alocacao.reservado)
            .unwrap_or_else(|_| erro_memoria_publica("falha ao descomprometer memória pública"));
        alocacao.viva = false;
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
    MetadadosDeRegiaoInvalidos,
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
            Self::MetadadosDeRegiaoInvalidos => {
                Some("E-RUNTIME-MEM-ADDRESS-OVERFLOW: metadados de região pública inválidos")
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
/// intervalo, existência da região, estado vivo, alinhamento e, por fim,
/// contenção — distinguindo interior que cruza o limite de acesso que começa
/// antes da base.
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
        let Some(fim) = alocacao.base.checked_add(alocacao.tamanho) else {
            return false;
        };
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
    let Ok(contido) = intervalo_publico_contido(alocacao.base, alocacao.tamanho, endereco, largura)
    else {
        return VeredictoAcesso::MetadadosDeRegiaoInvalidos;
    };
    if contido {
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
/// O back-end nativo emite esta chamada para todo acesso através de ponteiro de
/// proveniência pública **ou fabricada** (`<inteiro> virar seta<T>`). Endereço
/// nunca registrado é recusado aqui, com diagnóstico estável, em vez de virar
/// escrita em memória real e SIGSEGV.
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
        let fim = alocacao.base.saturating_add(alocacao.tamanho);
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
    let fim = alocacao
        .base
        .checked_add(alocacao.tamanho)
        .unwrap_or_else(|| {
            erro_memoria_publica(
                "E-RUNTIME-MEM-ADDRESS-OVERFLOW: metadados de região pública inválidos",
            )
        });
    if derivado < alocacao.base || derivado > fim {
        erro_memoria_publica(
            "E-RUNTIME-MEM-OUT-OF-BOUNDS: derivação fora dos limites da alocação pública",
        );
    }
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
// @pinker-nav:summary Núcleo de formatar_verso (placeholders `{}` na ordem, com erro_fatal em contagem ou placeholder malformado) e as variantes pinker_formatar_verso_0..8 geradas pela macro formatar_wrappers!, cada uma com aridade fixa (0 a 8 argumentos) — não há variante para aridade maior.
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

macro_rules! formatar_wrappers {
    ($(($nome:ident, $($arg:ident),*)),* $(,)?) => {
        $(
            /// # Safety
            /// Todos os ponteiros devem apontar para blocos de verso válidos.
            #[no_mangle]
            pub unsafe extern "C" fn $nome(modelo: *const u8, $($arg: *const u8),*) -> *mut u8 {
                formatar_verso_nucleo(modelo, &[$($arg),*])
            }
        )*
    };
}

/// # Safety
/// `modelo` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_formatar_verso_0(modelo: *const u8) -> *mut u8 {
    formatar_verso_nucleo(modelo, &[])
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
// @pinker-nav:summary Impressão uniforme de falar: bombom/logica/verso/espaço/newline passam pelo mesmo writer com write_all+flush; a disposição de SIGPIPE é estabelecida em pinker_rt_iniciar (e reafirmada por Once nas entradas de I/O) para que pipe fechado retorne erro em qualquer ordem de execução, e toda falha de stdout termina pelo diagnóstico controlado de erro_fatal.
#[cfg(unix)]
const SINAL_SIGPIPE: i32 = 13;
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
/// Sobre herança: `SIG_IGN` sobrevive a `exec`, mas os filhos não são afetados
/// — ver `comando_saneado` para a medição e a decisão de não instalar um
/// `pre_exec` próprio.
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
use std::io::{Read as _, Seek as _, Write as _};
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
// @pinker-nav:summary Execução de subprocessos sem shell implícito: basenames são resolvidos somente na PATH fixa /usr/local/bin:/usr/bin:/bin, caminhos com slash são usados literalmente, e todas as famílias recebem a PATH saneada; a disposição de SIGPIPE do pai não é reafirmada aqui (pertence a pinker_rt_iniciar) e os filhos observam SIG_DFL porque std::process::Command restaura a disposição antes do exec — medido pela matriz de R5, não presumido; executar_com_entrada escreve stdin numa thread concorrente à espera, fecha o pipe e agrega erros sem deixar writer órfão.
const PATH_PROCESSOS: &str = "/usr/local/bin:/usr/bin:/bin";

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
/// que em princípio faria um programa externo disparado pela Pinker herdar uma
/// disposição não padrão. Isso **não** acontece porque `std::process::Command`
/// restaura `SIGPIPE` para `SIG_DFL` no caminho pré-`exec`, antes de qualquer
/// closure de `pre_exec` do chamador. Foi medido, não presumido: a matriz de
/// R5 inclui uma célula (`sigpipe-disposicao`) em que o filho lê a disposição
/// herdada num construtor de `.init_array` — antes do `lang_start` da std, que
/// instalaria `SIG_IGN` e mascararia a medida — e exige `SIG_DFL` nos dois
/// back-ends.
///
/// Por isso o runtime **não** instala um `pre_exec` próprio: seria código
/// `unsafe` redundante que nenhum teste conseguiria distinguir do
/// comportamento já garantido. A célula da matriz é o guardião do contrato: se
/// a std deixar de restaurar, ela falha.
fn comando_saneado(resolvido: std::path::PathBuf) -> std::process::Command {
    let mut processo = std::process::Command::new(resolvido);
    processo.env("PATH", PATH_PROCESSOS);
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

// @pinker-nav:start evidencia.runtime.memoria-alocador
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Abertura do módulo de testes internos do runtime nativo e evidência em memória do alocador: alinhamento e usabilidade do bloco devolvido por `pinker_alocar`, não sobreposição entre alocações independentes, alocação de zero bytes e tolerância a `pinker_liberar` sobre ponteiro nulo.
#[cfg(test)]
mod tests {
    use super::*;

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

    // @pinker-nav:end evidencia.runtime.memoria-alocador

    // @pinker-nav:start evidencia.runtime.validacao-acesso-publico
    // @pinker-nav:domain memoria
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Matriz do veredicto de acesso à memória pública (hotfix pós-PR #411, item V4) sobre a unidade pura `classificar_acesso_publico`: endereços não registrados (4096 não mapeado, nulo, pilha, dado estático, função, alocação interna do runtime, mapeamento estrangeiro válido) recusados como E-RUNTIME-MEM-UNKNOWN-ACCESS; região liberada como use-after-free; base viva, interior, primeiro e último byte válidos permitidos; um byte após a região e acesso multibyte cruzando o limite recusados; e a matriz de larguras 1/2/4/8 com os alinhamentos correspondentes, idêntica para load e store porque ambos compartilham o mesmo predicado.
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
            VeredictoAcesso::MetadadosDeRegiaoInvalidos,
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
    // @pinker-nav:end evidencia.runtime.cota-identidades-publicas

    // @pinker-nav:start evidencia.runtime.inicializacao-abi
    // @pinker-nav:domain inicializacao
    // @pinker-nav:layer evidencia
    // @pinker-nav:summary Evidência em memória do bootstrap e da ABI: `pinker_rt_iniciar` captura `argc`/`argv` e os devolve por `pinker_rt_argc`/`pinker_rt_argv`, e `pinker_rt_versao` reporta a versão corrente da ABI.
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
    // @pinker-nav:end evidencia.runtime.inicializacao-abi

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
