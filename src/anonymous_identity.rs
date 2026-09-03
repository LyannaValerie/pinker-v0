//! Identidade estreita e injetiva de callables anônimos.

use crate::source_origin::SourceOrigin;

// @pinker-nav:start identidades.anonima-callable
// @pinker-nav:domain identidade
// @pinker-nav:layer compilador
// @pinker-nav:summary Codifica a identidade estrutural e injetiva de callables anônimos a partir da proveniência canônica da fonte e do índice local do parser, renderizando integralmente os bytes sob o namespace sintético reservado. Desde a #567 há uma segunda forma, para a closure COPIADA numa materialização de corpo default de trato: ela carrega as duas proveniências — a da closure, que é a unidade onde o default foi escrito, e a da materialização, que é a unidade que escreveu o `impl` e conta o índice. As duas são necessárias e por razões distintas: sem a primeira a cópia se apresentaria como coisa do importador; sem a segunda dois importadores cunhariam o mesmo nome, porque índice local só é injetivo dentro de quem o conta. Nenhuma grafia de trato, alvo ou método participa.
const FORMAT_MAGIC: &[u8] = b"pinker-anonymous-callable-v1";
const MATERIALIZED_DEFAULT_FORMAT_MAGIC: &[u8] = b"pinker-materialized-default-closure-v1";
pub const ANONYMOUS_CALLABLE_PREFIX: &str = "__anon_carinho_";

fn hex_bytes(rendered: &str) -> Option<Vec<u8>> {
    if rendered.len() % 2 != 0 {
        return None;
    }
    rendered
        .as_bytes()
        .chunks(2)
        .map(|par| {
            let alto = char::from(par[0]).to_digit(16)?;
            let baixo = char::from(par[1]).to_digit(16)?;
            u8::try_from(alto * 16 + baixo).ok()
        })
        .collect()
}

fn push_len_prefixed(bytes: &mut Vec<u8>, payload: &[u8]) {
    bytes.extend_from_slice(
        &u64::try_from(payload.len())
            .expect("payload length fits in u64")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(payload);
}

fn push_origin(bytes: &mut Vec<u8>, origin: &SourceOrigin) {
    match origin {
        SourceOrigin::Root => bytes.push(0),
        SourceOrigin::Module(module_key) => {
            bytes.push(1);
            push_len_prefixed(bytes, module_key.as_bytes());
        }
        SourceOrigin::Builtin => bytes.push(2),
    }
}

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
    push_origin(&mut bytes, origin);
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

/// #567 — identidade de uma closure sintética COPIADA para uma materialização
/// de corpo default de trato.
///
/// A cópia tem duas proveniências, e perder qualquer uma delas é um defeito
/// diferente:
///
/// - a da CLOSURE, que é a unidade onde o corpo default foi escrito. Ela entra
///   inteira, pelos bytes que já a renderizam, e é o que impede que a cópia se
///   apresente como coisa da unidade importadora;
/// - a da MATERIALIZAÇÃO, que é a unidade que escreveu o `impl` e conta o
///   índice local. Ela entra porque o índice local só é injetivo dentro de quem
///   o conta: emprestar a proveniência da origem e contar o índice no
///   importador faria dois importadores cunharem o mesmo nome para
///   materializações diferentes, e a captura da primeira valeria para as duas.
///
/// Uma cópia por materialização é obrigatória: a captura de uma closure é
/// resolvida uma vez por nome, no primeiro ponto de criação, e o receiver do
/// default muda de tipo a cada alvo.
///
/// A codificação é integral e prefixada por comprimento nas duas pontas, então
/// nome igual continua provando entidade igual — a premissa que a deduplicação
/// por conteúdo usa. Nenhuma grafia de trato, alvo ou método participa: quem
/// decide identidade de relação continua sendo a autoridade canônica.
pub fn materialized_default_closure_name(
    origin_closure_name: &str,
    materializing_origin: &SourceOrigin,
    local_index: usize,
) -> String {
    let rendered = origin_closure_name
        .strip_prefix(ANONYMOUS_CALLABLE_PREFIX)
        .unwrap_or(origin_closure_name);
    // A origem entra pelos bytes que a renderizam. Quando a grafia recebida não
    // é a renderização canônica, os bytes crus dela servem igual: o que importa
    // aqui é ser injetivo, e nunca silenciar a diferença.
    let origin_bytes =
        hex_bytes(rendered).unwrap_or_else(|| origin_closure_name.as_bytes().to_vec());

    let mut bytes = Vec::new();
    bytes.extend_from_slice(MATERIALIZED_DEFAULT_FORMAT_MAGIC);
    push_len_prefixed(&mut bytes, &origin_bytes);
    push_origin(&mut bytes, materializing_origin);
    bytes.extend_from_slice(
        &u64::try_from(local_index)
            .expect("materialized default closure local index fits in u64")
            .to_be_bytes(),
    );
    format!("{}{}", ANONYMOUS_CALLABLE_PREFIX, full_hex(&bytes))
}
// @pinker-nav:end identidades.anonima-callable
