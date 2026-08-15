//! Nomes públicos e assinaturas da superfície SHA-256 — Parte E2.
//!
//! Mesma disciplina de [`crate::valor_json`]: os nomes e as assinaturas são
//! declarados **uma vez**, e parser, semântica, IR, validadores, interpretador e
//! backend derivam daqui em vez de repetir a tabela por camada — que é como uma
//! camada acaba discordando das outras sem que ninguém perceba.
//!
//! O núcleo criptográfico não está aqui: vive em `pinker_sha256_contract`, puro
//! e compartilhado com o runtime nativo. Aqui só moram os nomes da linguagem.
//!
//! A superfície falível de arquivo não está aqui: nome, símbolo e cargas vivem
//! em [`crate::falha_operacional`], autoridade das superfícies que devolvem
//! `Resultado<T,E>`. Um nome, uma autoridade — e um teste da Parte B cobra que
//! o nome público de uma superfície falível exista só naquele arquivo.

// @pinker-nav:start sha256.superficie.nomes
// @pinker-nav:domain integridade
// @pinker-nav:layer semantica
// @pinker-nav:summary Nomes públicos da superfície SHA-256 adulta da Parte E2 declarados num único lugar: `sha256_verso` é a intrínseca pura sobre os bytes UTF-8 de um `verso`, e a intrínseca falível de arquivo vive em `falha_operacional` porque devolve `Resultado`. `assinatura_ir` publica a assinatura operacional uma vez para todas as camadas de pipeline, e `simbolo_runtime` liga o nome ao símbolo `pinker_*` do runtime nativo, de modo que o backend consulte a autoridade em vez de manter uma cópia da lista.

/// Nomes públicos das intrínsecas **não falíveis** da superfície SHA-256.
///
/// A superfície de arquivo não está aqui: ela devolve `Resultado<verso,verso>`
/// e pertence a [`crate::falha_operacional`].
pub mod intrinsecas {
    /// `sha256_verso(verso) -> verso`.
    ///
    /// Contrato: `SHA256(verso) = SHA256(UTF8_BYTES(verso))`. Sem normalização
    /// Unicode, sem conversão por codepoint, sem round-trip por outra
    /// codificação. Dado já em memória não pode falhar, logo não devolve
    /// `Resultado`.
    pub const VERSO: &str = "sha256_verso";
}

/// Símbolo do runtime nativo de [`intrinsecas::VERSO`].
pub const SIMBOLO_VERSO: &str = "pinker_sha256_verso";

/// Comprimento do digest na forma canônica pública.
///
/// 64 caracteres hexadecimais minúsculos, sem prefixo e sem separador. É a
/// mesma forma que o repositório já grava em manifests e a mesma que
/// `sha256sum(1)` emite, o que torna a comparação uma igualdade de `verso`.
pub const DIGEST_CARACTERES: usize = 64;

/// Todas as intrínsecas não falíveis da família, em ordem estável.
pub const ACESSORES: [&str; 1] = [intrinsecas::VERSO];

/// Verdadeiro para qualquer intrínseca não falível da família.
pub fn e_acessor(nome: &str) -> bool {
    ACESSORES.contains(&nome)
}

/// Assinatura operacional de um acessor: retorno e parâmetros, na ordem.
///
/// Declarada **uma vez**. IR, validação de IR, validação de CFG, validação de
/// seleção e validação da máquina abstrata derivam desta função.
pub fn assinatura_ir(nome: &str) -> Option<(crate::ir::TypeIR, Vec<crate::ir::TypeIR>)> {
    use crate::ir::TypeIR;
    match nome {
        intrinsecas::VERSO => Some((TypeIR::Verso, vec![TypeIR::Verso])),
        _ => None,
    }
}

/// Símbolo `pinker_*` do runtime nativo que implementa o nome, quando houver.
///
/// O backend consulta esta função em vez de manter uma cópia da lista.
pub fn simbolo_runtime(nome: &str) -> Option<&'static str> {
    match nome {
        intrinsecas::VERSO => Some(SIMBOLO_VERSO),
        _ => None,
    }
}

// @pinker-nav:end sha256.superficie.nomes
