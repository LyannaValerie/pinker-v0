//! Autoridade única da taxonomia de entradas de filesystem — Parte C.
//!
//! A enumeração de um diretório devolve nomes. Classificar cada nome é uma
//! operação separada, e a classificação precisa de uma representação que o
//! programa consiga decidir: um leque, não um texto nem um código numérico.
//!
//! ```text
//! verso  -> serializa a taxonomia em texto e devolve a exaustividade ao usuário
//! bombom -> obriga o usuário a comparar números mágicos
//! leque  -> compõe com `encaixe`, e o compilador cobra exaustividade
//! ```
//!
//! `TipoEntrada` não tem carga. Um leque sem carga abaixa para o discriminante
//! imediato (ver `Item::Enum` na IR), então esta escolha **não** cria handle,
//! alocação nem família nova de recurso — o valor é uma palavra, como `bombom`.
//!
//! Este módulo é a única declaração do nome público do leque e dos nomes de
//! suas variantes dentro da crate do compilador. Parser e interpretador derivam
//! daqui; o runtime nativo espelha os discriminantes e a paridade é fixada por
//! evidência, pela mesma razão que `RESULTADO_TAG_OK` é espelhado lá.
//!
//! O que este módulo **não** faz:
//!
//! - não segue symlink. A classificação é sempre `symlink_metadata` (lstat):
//!   um symlink é `Symlink` mesmo quando aponta para diretório, e um symlink
//!   quebrado continua `Symlink` em vez de virar erro;
//! - não decide política de erro. Quem falha e como falha é da autoridade das
//!   superfícies falíveis;
//! - não cresce por analogia. `Outro` existe justamente para que FIFO, socket e
//!   device não pressionem a taxonomia a cada tipo novo do host.

// @pinker-nav:start filesystem.tipo-entrada.taxonomia
// @pinker-nav:domain filesystem
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única da taxonomia de entradas da Parte C: `TipoEntrada` nomeia as quatro classes observáveis sem seguir symlink, `VARIANTES` fixa a ordem de declaração — que é o discriminante lido pela IR — e `classificar` deriva a classe de um `FileType` obtido por `symlink_metadata`, testando `is_symlink` antes de arquivo e diretório para que o alvo nunca decida a classe. O nome público do leque e os nomes das variantes existem só aqui; o runtime nativo espelha os discriminantes e a paridade é fixada por evidência.

/// Nome público do leque predeclarado da taxonomia.
pub const LEQUE_TIPO_ENTRADA: &str = "TipoEntrada";

/// Classe de uma entrada de filesystem, observada sem seguir symlink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEntrada {
    /// Arquivo regular.
    Arquivo,
    /// Diretório.
    Diretorio,
    /// Link simbólico, independente do que o alvo seja ou de o alvo existir.
    Symlink,
    /// Qualquer outra entrada representável pelo host: FIFO, socket, device.
    Outro,
}

/// Variantes na ordem de declaração do leque.
///
/// A ordem **é** o discriminante: a IR usa o índice de declaração da variante.
/// Reordenar esta lista muda valores observáveis, e a evidência de paridade
/// quebra imediatamente se as duas pontas discordarem.
pub const VARIANTES: &[(&str, TipoEntrada)] = &[
    ("Arquivo", TipoEntrada::Arquivo),
    ("Diretorio", TipoEntrada::Diretorio),
    ("Symlink", TipoEntrada::Symlink),
    ("Outro", TipoEntrada::Outro),
];

impl TipoEntrada {
    /// Discriminante da variante: seu índice de declaração em [`VARIANTES`].
    pub fn discriminante(self) -> u64 {
        VARIANTES
            .iter()
            .position(|(_, variante)| *variante == self)
            .expect("toda variante de TipoEntrada está declarada em VARIANTES") as u64
    }

    /// Nome público da variante.
    pub fn nome(self) -> &'static str {
        VARIANTES
            .iter()
            .find(|(_, variante)| *variante == self)
            .map(|(nome, _)| *nome)
            .expect("toda variante de TipoEntrada está declarada em VARIANTES")
    }

    /// Classifica um `FileType` obtido por `symlink_metadata`.
    ///
    /// `is_symlink` é testado primeiro por contrato, não por precaução: passar
    /// aqui um `FileType` vindo de `metadata` (que segue o link) produziria a
    /// classe do alvo em vez da classe da entrada. A ordem documenta que a
    /// entrada, não o alvo, decide.
    pub fn classificar(tipo: std::fs::FileType) -> TipoEntrada {
        if tipo.is_symlink() {
            TipoEntrada::Symlink
        } else if tipo.is_file() {
            TipoEntrada::Arquivo
        } else if tipo.is_dir() {
            TipoEntrada::Diretorio
        } else {
            TipoEntrada::Outro
        }
    }
}

// @pinker-nav:end filesystem.tipo-entrada.taxonomia
