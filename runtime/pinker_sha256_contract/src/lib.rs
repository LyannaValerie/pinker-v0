//! Contrato puro do SHA-256 da Pinker — Parte E2.
//!
//! Este crate existe pela mesma razão única e verificável do
//! `pinker_json_contract`: **uma** implementação.
//!
//! O interpretador vive no crate do compilador e o runtime nativo vive em
//! `pinker_rt`, que não pode depender do compilador. Duas implementações do
//! mesmo algoritmo divergiriam, e um digest divergente é indetectável a olho
//! nu — é só mais uma sequência de 64 caracteres plausíveis.
//!
//! ```text
//! ONE_ALGORITHM -> ONE_IMPLEMENTATION -> PARITY_BY_CONSTRUCTION
//! ```
//!
//! Aqui não há nome público da linguagem, tipo do compilador, ABI nem I/O: só
//! o núcleo de compressão e a forma canônica do digest. Os nomes públicos e as
//! assinaturas vivem em `sha256`, no compilador; a leitura de arquivo pertence
//! a cada backend, porque I/O não é contrato puro.
//!
//! O núcleo é incremental **por dentro** para que hashear um arquivo não exija
//! materializar o conteúdo inteiro na memória. Isso é detalhe de implementação:
//! nenhuma API incremental é exposta à linguagem.
//!
//! ```text
//! INTERNAL_STREAMING DOES_NOT_REQUIRE PUBLIC_HASH_CONTEXT
//! ```

// @pinker-nav:start sha256.contrato.nucleo
// @pinker-nav:domain integridade
// @pinker-nav:layer contrato
// @pinker-nav:summary Autoridade única do SHA-256 da Pinker (Parte E2): as constantes K e o estado inicial de FIPS 180-4, o compressor de bloco de 64 bytes, o acumulador incremental `Sha256` (atualizar/finalizar) que mantém apenas um bloco parcial em memória, e `sha256_hex`, a forma canônica de 64 caracteres hexadecimais minúsculos. Compartilhado pelo compilador e pelo runtime nativo para que o digest seja idêntico nos dois backends por construção; não contém I/O, nome público da linguagem nem ABI.

/// Constantes de round do SHA-256 (FIPS 180-4): as 64 primeiras raízes cúbicas
/// primas, em ponto fixo de 32 bits.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Estado inicial do SHA-256 (FIPS 180-4): as 8 primeiras raízes quadradas
/// primas, em ponto fixo de 32 bits.
const ESTADO_INICIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Tamanho do bloco de compressão do SHA-256, em bytes.
const BLOCO: usize = 64;

/// Acumulador incremental de SHA-256.
///
/// Mantém em memória apenas o estado de 8 palavras e **um** bloco parcial de 64
/// bytes, independentemente do volume total já absorvido. É isso que permite
/// hashear um arquivo grande sem carregá-lo inteiro.
///
/// Não é superfície pública da linguagem: existe para que o compilador e o
/// runtime nativo compartilhem o mesmo núcleo.
#[derive(Clone)]
pub struct Sha256 {
    estado: [u32; 8],
    parcial: [u8; BLOCO],
    preenchidos: usize,
    total_bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::novo()
    }
}

impl Sha256 {
    /// Acumulador vazio, no estado inicial de FIPS 180-4.
    pub fn novo() -> Self {
        Self {
            estado: ESTADO_INICIAL,
            parcial: [0u8; BLOCO],
            preenchidos: 0,
            total_bytes: 0,
        }
    }

    /// Absorve mais bytes.
    ///
    /// O resultado depende apenas da concatenação dos bytes absorvidos, nunca
    /// de como eles foram fatiados entre chamadas — propriedade fixada por
    /// teste, porque é exatamente ela que o caminho de arquivo em streaming
    /// depende para concordar com o caminho de uma tacada só.
    pub fn atualizar(&mut self, dados: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(dados.len() as u64);
        let mut resto = dados;

        // Completa o bloco parcial pendente, se houver.
        if self.preenchidos > 0 {
            let falta = BLOCO - self.preenchidos;
            let leva = falta.min(resto.len());
            self.parcial[self.preenchidos..self.preenchidos + leva].copy_from_slice(&resto[..leva]);
            self.preenchidos += leva;
            resto = &resto[leva..];
            if self.preenchidos == BLOCO {
                let bloco = self.parcial;
                comprimir(&mut self.estado, &bloco);
                self.preenchidos = 0;
            }
        }

        // Consome blocos completos direto da entrada, sem copiar.
        let mut pedacos = resto.chunks_exact(BLOCO);
        for bloco in &mut pedacos {
            let bloco: &[u8; BLOCO] = bloco.try_into().expect("chunks_exact devolve 64 bytes");
            comprimir(&mut self.estado, bloco);
        }

        // Guarda a sobra para a próxima chamada.
        let sobra = pedacos.remainder();
        if !sobra.is_empty() {
            self.parcial[..sobra.len()].copy_from_slice(sobra);
            self.preenchidos = sobra.len();
        }
    }

    /// Aplica o padding de FIPS 180-4 e devolve as 8 palavras do digest.
    ///
    /// O padding é `0x80`, zeros até faltarem 8 bytes para fechar o bloco, e o
    /// comprimento total **em bits** big-endian. Quando não cabem os 8 bytes de
    /// comprimento no bloco corrente, um bloco extra é emitido — é esse o caso
    /// de borda em 55/56 bytes coberto por teste.
    pub fn finalizar(mut self) -> [u32; 8] {
        let bits = self.total_bytes.wrapping_mul(8);

        self.absorver_padding(0x80);
        while self.preenchidos != BLOCO - 8 {
            self.absorver_padding(0x00);
        }
        for byte in bits.to_be_bytes() {
            self.absorver_padding(byte);
        }
        debug_assert_eq!(self.preenchidos, 0, "o padding fecha exatamente um bloco");

        self.estado
    }

    /// Digest na forma canônica: 64 caracteres hexadecimais minúsculos.
    pub fn finalizar_hex(self) -> String {
        hex_minusculo(self.finalizar())
    }

    /// Empurra um byte de padding, comprimindo quando o bloco fecha.
    ///
    /// Deliberadamente não usa [`Sha256::atualizar`]: o padding não pode entrar
    /// na contagem de bytes que ele mesmo codifica.
    fn absorver_padding(&mut self, byte: u8) {
        self.parcial[self.preenchidos] = byte;
        self.preenchidos += 1;
        if self.preenchidos == BLOCO {
            let bloco = self.parcial;
            comprimir(&mut self.estado, &bloco);
            self.preenchidos = 0;
        }
    }
}

/// Compressão de um bloco de 64 bytes sobre o estado corrente.
fn comprimir(estado: &mut [u32; 8], bloco: &[u8; BLOCO]) {
    let mut w = [0u32; 64];
    for (i, palavra) in w.iter_mut().take(16).enumerate() {
        let o = i * 4;
        *palavra = u32::from_be_bytes([bloco[o], bloco[o + 1], bloco[o + 2], bloco[o + 3]]);
    }
    for i in 16..64 {
        let a = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let b = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(a)
            .wrapping_add(w[i - 7])
            .wrapping_add(b);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *estado;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    for (slot, valor) in estado.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(valor);
    }
}

/// Forma canônica do digest: 64 caracteres hexadecimais **minúsculos**, sem
/// prefixo e sem separador.
///
/// Escrito à mão em vez de por `format!` para não depender de formatação
/// configurável: o alfabeto é fixo aqui, não herdado.
fn hex_minusculo(estado: [u32; 8]) -> String {
    const DIGITOS: &[u8; 16] = b"0123456789abcdef";
    let mut saida = String::with_capacity(64);
    for palavra in estado {
        for byte in palavra.to_be_bytes() {
            saida.push(DIGITOS[(byte >> 4) as usize] as char);
            saida.push(DIGITOS[(byte & 0x0f) as usize] as char);
        }
    }
    saida
}

/// SHA-256 de uma sequência de bytes, na forma canônica de 64 caracteres
/// hexadecimais minúsculos.
///
/// É a entrada de uma tacada só, para quem já tem todos os bytes em memória.
/// Quem lê de um arquivo deve usar [`Sha256`] incrementalmente.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut acumulador = Sha256::novo();
    acumulador.atualizar(bytes);
    acumulador.finalizar_hex()
}

// @pinker-nav:end sha256.contrato.nucleo

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn vetores_oficiais_fips_180_4() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn fatiamento_nao_altera_o_digest() {
        // A propriedade de que o caminho em streaming depende: o digest é
        // função da concatenação, nunca do tamanho dos pedaços.
        let dados: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let referencia = sha256_hex(&dados);
        for pedaco in [1usize, 7, 63, 64, 65, 127, 128, 999] {
            let mut acumulador = Sha256::novo();
            for parte in dados.chunks(pedaco) {
                acumulador.atualizar(parte);
            }
            assert_eq!(
                acumulador.finalizar_hex(),
                referencia,
                "fatiamento em {pedaco} bytes divergiu"
            );
        }
    }

    #[test]
    fn bordas_de_padding() {
        // 55 cabe com o comprimento no mesmo bloco; 56 força um bloco extra.
        for tamanho in [0usize, 1, 54, 55, 56, 57, 63, 64, 65, 119, 120, 128] {
            let dados = vec![0x61u8; tamanho];
            let mut acumulador = Sha256::novo();
            acumulador.atualizar(&dados);
            let incremental = acumulador.finalizar_hex();
            assert_eq!(incremental, sha256_hex(&dados), "tamanho {tamanho}");
            assert_eq!(incremental.len(), 64);
        }
    }

    #[test]
    fn forma_canonica_do_digest() {
        let digest = sha256_hex(b"qualquer coisa");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert!(!digest.starts_with("0x"));
    }

    #[test]
    fn bytes_crus_incluindo_nul_sao_preservados() {
        // Nenhum byte é especial para o núcleo: NUL e UTF-8 inválido entram.
        assert_ne!(sha256_hex(b"a\0b"), sha256_hex(b"ab"));
        assert_ne!(sha256_hex(&[0xff, 0xfe]), sha256_hex(b""));
    }
}
