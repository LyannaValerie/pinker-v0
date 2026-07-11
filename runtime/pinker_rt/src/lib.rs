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

fn layout_para(tamanho_total: usize) -> Layout {
    Layout::from_size_align(tamanho_total, ALINHAMENTO)
        .expect("layout de alocação inválido no runtime pinker_rt")
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
    let layout = layout_para(total);
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
    dealloc(base, layout_para(total));
}

// ---------------------------------------------------------------------------
// Verso dinâmico (Fase 215/B4)
//
// Representação nativa de `verso`: ponteiro único para um bloco
// `[tamanho_em_bytes: u64][bytes utf-8...]`. Literais estáticos em `.rodata`
// e versos de heap compartilham o mesmo layout, então todas as operações
// abaixo funcionam uniformemente sobre qualquer valor de verso.
// ---------------------------------------------------------------------------

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
// `falar` nativo (Fase 215/B4) — espelha byte a byte as instruções de máquina
// do interpretador: PrintIntInline, PrintBoolInline, PrintStrValueInline,
// PrintSpace e PrintNewline. O flush acontece na quebra de linha (LineWriter).
// ---------------------------------------------------------------------------

/// Imprime um `bombom` decimal sem quebra de linha.
#[no_mangle]
pub extern "C" fn pinker_falar_pedaco_bombom(valor: u64) {
    print!("{}", valor);
}

/// Imprime uma `logica` como `verdade`/`falso` sem quebra de linha.
#[no_mangle]
pub extern "C" fn pinker_falar_pedaco_logica(valor: u64) {
    print!("{}", if valor != 0 { "verdade" } else { "falso" });
}

/// Imprime os bytes de um verso sem quebra de linha.
///
/// # Safety
/// `v` deve apontar para um bloco de verso válido.
#[no_mangle]
pub unsafe extern "C" fn pinker_falar_pedaco_verso(v: *const u8) {
    use std::io::Write;
    let bytes = verso_bytes(v);
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = lock.write_all(bytes);
}

/// Separador entre argumentos de `falar` (espaço simples).
#[no_mangle]
pub extern "C" fn pinker_falar_espaco() {
    print!(" ");
}

/// Fim de um `falar` (quebra de linha; o LineWriter da std faz o flush).
#[no_mangle]
pub extern "C" fn pinker_falar_fim() {
    println!();
}

// ---------------------------------------------------------------------------
// Listas nativas (Fase 216/B5)
//
// Uma lista é um ponteiro para um header fixo `[len: u64][cap: u64][dados:
// *mut u64]`; os elementos são palavras de 8 bytes (o valor de `bombom`, o
// ponteiro de um `verso` ou o valor/handle de um leque), então a mesma
// implementação serve `lista<bombom>`, `lista<verso>` e `lista<Leque>`.
// O header nunca muda de endereço; o crescimento realoca apenas `dados`.
// ---------------------------------------------------------------------------

const LISTA_CAP_INICIAL: u64 = 8;

fn erro_fatal(msg: &str) -> ! {
    eprintln!("Erro de Execução (pinker_rt): {}", msg);
    std::process::exit(1)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
