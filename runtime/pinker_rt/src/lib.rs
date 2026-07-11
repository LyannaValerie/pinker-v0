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
