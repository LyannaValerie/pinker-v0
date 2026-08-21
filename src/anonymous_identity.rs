//! Identidade estreita e injetiva de callables anônimos.

use crate::source_origin::SourceOrigin;

// @pinker-nav:start identidades.anonima-callable
// @pinker-nav:domain identidade
// @pinker-nav:layer compilador
// @pinker-nav:summary Codifica a identidade estrutural e injetiva de callables anônimos a partir da proveniência canônica da fonte e do índice local do parser, renderizando integralmente os bytes sob o namespace sintético reservado.
const FORMAT_MAGIC: &[u8] = b"pinker-anonymous-callable-v1";
pub const ANONYMOUS_CALLABLE_PREFIX: &str = "__anon_carinho_";

fn full_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(char::from(HEX[usize::from(byte >> 4)]));
        rendered.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    rendered
}

/// Bytes estruturais da identidade disponível no ponto em que o parser
/// materializa uma closure. A renderização é integral: não é digest e não
/// perde informação.
pub fn anonymous_callable_identity_bytes(origin: &SourceOrigin, local_index: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    match origin {
        SourceOrigin::Root => bytes.push(0),
        SourceOrigin::Module(module_key) => {
            bytes.push(1);
            let module_bytes = module_key.as_bytes();
            bytes.extend_from_slice(
                &u64::try_from(module_bytes.len())
                    .expect("module key length fits in u64")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(module_bytes);
        }
        SourceOrigin::Builtin => bytes.push(2),
    }
    bytes.extend_from_slice(
        &u64::try_from(local_index)
            .expect("anonymous callable local index fits in u64")
            .to_be_bytes(),
    );
    bytes
}

/// Nome reservado, determinístico e seguro para o assembler vigente.
pub fn anonymous_callable_name(origin: &SourceOrigin, local_index: usize) -> String {
    format!(
        "{}{}",
        ANONYMOUS_CALLABLE_PREFIX,
        full_hex(&anonymous_callable_identity_bytes(origin, local_index))
    )
}
// @pinker-nav:end identidades.anonima-callable
