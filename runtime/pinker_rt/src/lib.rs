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
/// # Safety
/// `argv` deve ser o vetor de argumentos recebido pelo `main` C; o runtime
/// apenas o armazena para consulta posterior.
#[no_mangle]
pub unsafe extern "C" fn pinker_rt_iniciar(argc: i64, argv: *const *const u8) {
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
    if let Some(indice) =
        indice_base_publica_mais_recente(&memoria.alocacoes, ponteiro as usize)
    {
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
    if largura == 0 || alinhamento == 0 || !alinhamento.is_power_of_two() {
        erro_memoria_publica("metadados inválidos de acesso à memória pública");
    }
    let fim_acesso = endereco.checked_add(largura).unwrap_or_else(|| {
        erro_memoria_publica("E-RUNTIME-MEM-ADDRESS-OVERFLOW: overflow no acesso à memória pública")
    });
    let memoria = memoria_publica()
        .lock()
        .unwrap_or_else(|_| erro_memoria_publica("registro público de alocações indisponível"));
    let candidata = memoria.alocacoes.iter().rev().find(|alocacao| {
        let Some(fim) = alocacao.base.checked_add(alocacao.tamanho) else {
            return false;
        };
        (endereco >= alocacao.base && endereco <= fim)
            || (endereco < alocacao.base && fim_acesso > alocacao.base)
    });
    let Some(alocacao) = candidata else {
        erro_memoria_publica(
            "E-RUNTIME-MEM-UNKNOWN-ACCESS: acesso sem região pública registrada",
        );
    };
    if !alocacao.viva {
        erro_memoria_publica(
            "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública",
        );
    }
    if endereco % alinhamento != 0 {
        erro_memoria_publica("E-RUNTIME-MEM-MISALIGNED: acesso desalinhado à memória pública");
    }
    let contido = intervalo_publico_contido(alocacao.base, alocacao.tamanho, endereco, largura)
        .unwrap_or_else(|_| {
            erro_memoria_publica(
                "E-RUNTIME-MEM-ADDRESS-OVERFLOW: metadados de região pública inválidos",
            )
        });
    if !contido {
        if endereco >= alocacao.base {
            erro_memoria_publica(
                "E-RUNTIME-MEM-CROSS-BOUNDARY: acesso multibyte cruza o limite da alocação pública",
            );
        }
        erro_memoria_publica(
            "E-RUNTIME-MEM-OUT-OF-BOUNDS: acesso fora dos limites da alocação pública",
        );
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
// @pinker-nav:summary Impressão uniforme de falar: bombom/logica/verso/espaço/newline passam pelo mesmo writer com write_all+flush; SIGPIPE é ignorado em Unix para que pipe fechado retorne erro, e toda falha de stdout termina pelo diagnóstico controlado de erro_fatal.
#[cfg(unix)]
fn preparar_stdout() {
    static PREPARAR: Once = Once::new();
    PREPARAR.call_once(|| unsafe {
        extern "C" {
            fn signal(signal: i32, handler: usize) -> usize;
        }
        const SIGPIPE: i32 = 13;
        const SIG_IGN: usize = 1;
        signal(SIGPIPE, SIG_IGN);
    });
}

#[cfg(not(unix))]
fn preparar_stdout() {}

fn escrever_stdout(bytes: &[u8]) {
    use std::io::Write as _;

    preparar_stdout();
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
// @pinker-nav:summary Descritor imutável de união estrutural com identidade internada, tag determinística, layout validado e snapshot de uma palavra; criação e leitura usam uma ABI interna separada da memória pública.
#[repr(C)]
struct PinkerUnionDescriptor {
    union_type_id: u64,
    tag: u64,
    payload_size: u64,
    payload_align: u64,
    payload_word: u64,
}

fn union_layout_valid(size: u64, align: u64) -> bool {
    size > 0
        && size <= std::mem::size_of::<u64>() as u64
        && align > 0
        && align.is_power_of_two()
        && align <= std::mem::align_of::<u64>() as u64
}

/// Cria um snapshot imutável para um valor de união.
#[no_mangle]
pub extern "C" fn pinker_uniao_criar(
    union_type_id: u64,
    tag: u64,
    payload_size: u64,
    payload_align: u64,
    payload_word: u64,
) -> *mut u8 {
    if !union_layout_valid(payload_size, payload_align) {
        erro_fatal("layout inválido ao criar descritor de união estrutural");
    }
    Box::into_raw(Box::new(PinkerUnionDescriptor {
        union_type_id,
        tag,
        payload_size,
        payload_align,
        payload_word,
    })) as *mut u8
}

unsafe fn union_descriptor(handle: *mut u8) -> &'static PinkerUnionDescriptor {
    if handle.is_null() {
        erro_fatal("handle nulo de união estrutural");
    }
    &*(handle as *const PinkerUnionDescriptor)
}

/// Obtém a tag determinística de uma união.
///
/// # Safety
/// `handle` deve ter sido devolvido por `pinker_uniao_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_tag(handle: *mut u8) -> u64 {
    union_descriptor(handle).tag
}

unsafe fn union_payload(handle: *mut u8, expected_tag: u64) -> u64 {
    let descriptor = union_descriptor(handle);
    if descriptor.tag != expected_tag {
        erro_fatal("extração de união com tag incompatível");
    }
    if !union_layout_valid(descriptor.payload_size, descriptor.payload_align) {
        erro_fatal("descritor de união contém layout inválido");
    }
    descriptor.payload_word
}

/// Obtém uma carga escalar validando a tag.
///
/// # Safety
/// `handle` deve ter sido devolvido por `pinker_uniao_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_payload_b(handle: *mut u8, expected_tag: u64) -> u64 {
    union_payload(handle, expected_tag)
}

/// Obtém uma carga-handle validando a tag.
///
/// # Safety
/// `handle` deve ter sido devolvido por `pinker_uniao_criar`.
#[no_mangle]
pub unsafe extern "C" fn pinker_uniao_payload_v(handle: *mut u8, expected_tag: u64) -> *mut u8 {
    union_payload(handle, expected_tag) as *mut u8
}
// @pinker-nav:end runtime.unioes.descritor

// ---------------------------------------------------------------------------
// Arquivo, caminho, tempo e acaso nativos (Fase 220/B9)
//
// O modelo de arquivo espelha o interpretador: handles apontam para entradas
// em memória (`caminho` + `conteudo` + flag de anexo) e toda escrita persiste
// imediatamente no disco; handles fechados produzem erro distinto. O gerador
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
        .unwrap_or_else(|err| erro_fatal(&format!("falha ao reposicionar arquivo em '{nome}': {err}")));
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
// @pinker-nav:summary Execução de subprocessos sem shell implícito: basenames são resolvidos somente na PATH fixa /usr/local/bin:/usr/bin:/bin, caminhos com slash são usados literalmente, e todas as famílias recebem a PATH saneada; executar_com_entrada escreve stdin numa thread concorrente à espera, fecha o pipe e agrega erros sem deixar writer órfão.
const PATH_PROCESSOS: &str = "/usr/local/bin:/usr/bin:/bin";

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
    let resolvido =
        comando_resolvido(nome, comando).unwrap_or_else(|err| erro_fatal(err.as_str()));
    let mut processo = std::process::Command::new(resolvido);
    processo.env("PATH", PATH_PROCESSOS);
    processo
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
    let mut processo = std::process::Command::new(resolvido);
    processo.env("PATH", PATH_PROCESSOS);
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
    u64::try_from(codigo)
        .map_err(|_| "código de saída inválido em 'executar_com_entrada': valor negativo".to_string())
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
    let mut produtor = produtor_comando
        .spawn()
        .unwrap_or_else(|err| {
            erro_fatal(&format!(
                "falha ao executar processo produtor em 'pipeline_minimo': {err}"
            ))
        });
    let Some(saida_produtor) = produtor.stdout.take() else {
        erro_fatal("stdout indisponível em 'pipeline_minimo': produtor sem pipe configurado");
    };
    let mut consumidor_comando = novo_processo("pipeline_minimo", consumidor_nome);
    consumidor_comando.stdin(std::process::Stdio::from(saida_produtor));
    let mut consumidor = consumidor_comando
        .spawn()
        .unwrap_or_else(|err| {
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

        let path = std::env::temp_dir().join(format!(
            "pinker-rt-processo-{nome}-{}",
            std::process::id()
        ));
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
        let erro =
            processo_com_entrada_resultado(nao_leitor.to_str().unwrap(), &entrada, None).unwrap_err();
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

        let raiz = std::env::temp_dir().join(format!(
            "pinker-rt-descritores-{}",
            std::process::id()
        ));
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

        let raiz = std::env::temp_dir().join(format!(
            "pinker-rt-create-new-{}",
            std::process::id()
        ));
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
}
// @pinker-nav:end evidencia.runtime.mapas-iterador-snapshot
