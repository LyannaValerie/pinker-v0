//! Autoridade dos **nomes públicos** e das **assinaturas** da superfície JSON
//! adulta — Parte E1.
//!
//! O modelo, a interpretação de texto externo e a serialização determinística
//! não moram aqui: moram em [`pinker_json_contract`], que o runtime nativo
//! também usa. A separação é o que garante paridade por construção em vez de
//! paridade por disciplina.
//!
//! ```text
//! pinker_json_contract : gramática, árvore, ordem, domínio numérico
//! valor_json           : nomes públicos, tipos do compilador, assinaturas
//! ```
//!
//! # O que o handle é
//!
//! Mesma categoria de `lista`, `mapa`, `callable` e
//! [`crate::saida_processo::SaidaProcesso`]: uma palavra, identidade
//! monotônica, nunca reutilizada, sem recurso de sistema operacional por trás.
//! O que é novo é a arena ser **recursiva** — um nó referencia handles da mesma
//! tabela —, e é isso que torna o nesting geral em vez de uma família de
//! formatos.
//!
//! ```text
//! JSON_VALUE_IDENTITY != JSON_SOURCE_TEXT != PINKER_RUNTIME_REPRESENTATION
//! ```

pub use pinker_json_contract::{
    interpretar, serializar, NoJson, PoliticaValorJson, TabelaJson, TipoJson, LIMITE_PROFUNDIDADE,
};

// @pinker-nav:start json.identidade.nomes-publicos
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Nomes públicos da família JSON declarados uma única vez: o tipo do valor é handle opaco nominal reservado pelo runtime, o leque de classificação fixa a ordem de declaração que é o discriminante lido pela IR, e as duas identidades entram em `runtime_identity` — a reserva não vem do recipiente acidental em que cada uma é materializada.
/// Nome público do tipo do valor JSON.
///
/// Identidade **produzida pelo runtime**, como `SaidaProcesso`: o valor por trás
/// do nome é fabricado pela implementação, então o usuário não pode redeclarar.
/// A recusa vem de `runtime_identity`, não de um `match` local no parser.
pub const TIPO_VALOR_JSON: &str = "ValorJson";

/// Nome público do leque que classifica um valor JSON.
pub const LEQUE_TIPO_JSON: &str = "TipoJson";

/// Variantes de [`LEQUE_TIPO_JSON`], em ordem de declaração.
///
/// A ordem **é** o discriminante lido pela IR, exatamente como em
/// [`crate::tipo_entrada::VARIANTES`]. A tabela vive no contrato puro para que
/// o runtime nativo espelhe os mesmos discriminantes sem redeclará-los.
pub const VARIANTES: [&str; 6] = pinker_json_contract::VARIANTES;
// @pinker-nav:end json.identidade.nomes-publicos

// @pinker-nav:start json.superficie.nomes
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Nomes públicos da superfície JSON adulta declarados em um único lugar: a intrínseca falível de leitura vive em `falha_operacional`, e `intrinsecas` reúne emissão, classificação e acessores de verso, número, lógica, lista e objeto. `e_acessor` reconhece o conjunto por uma declaração só, para que nenhuma camada repita a lista de nomes.
/// Nomes públicos das intrínsecas não falíveis da superfície JSON.
///
/// A leitura falível não está aqui: ela pertence a
/// [`crate::falha_operacional`], que é a autoridade das superfícies que
/// devolvem `Resultado<T,E>`. Um nome, uma autoridade.
pub mod intrinsecas {
    /// `emitir_json(ValorJson) -> verso`, serialização determinística.
    pub const EMITIR: &str = "emitir_json";
    /// `json_tipo(ValorJson) -> TipoJson`.
    pub const TIPO: &str = "json_tipo";
    /// `json_verso(ValorJson) -> verso`.
    pub const VERSO: &str = "json_verso";
    /// `json_numero(ValorJson) -> i64`.
    pub const NUMERO: &str = "json_numero";
    /// `json_logica(ValorJson) -> logica`.
    pub const LOGICA: &str = "json_logica";
    /// `json_lista_tamanho(ValorJson) -> bombom`.
    pub const LISTA_TAMANHO: &str = "json_lista_tamanho";
    /// `json_lista_obter(ValorJson, bombom) -> ValorJson`.
    pub const LISTA_OBTER: &str = "json_lista_obter";
    /// `json_objeto_tamanho(ValorJson) -> bombom`.
    pub const OBJETO_TAMANHO: &str = "json_objeto_tamanho";
    /// `json_objeto_tem(ValorJson, verso) -> logica`.
    pub const OBJETO_TEM: &str = "json_objeto_tem";
    /// `json_objeto_obter(ValorJson, verso) -> ValorJson`.
    pub const OBJETO_OBTER: &str = "json_objeto_obter";
    /// `json_objeto_chaves(ValorJson) -> lista<verso>`, em ordem de chave.
    pub const OBJETO_CHAVES: &str = "json_objeto_chaves";
}

/// Superfícies planas históricas, preservadas com o domínio `u64` original.
///
/// Continuam existindo com a mesma assinatura e o mesmo comportamento de
/// aborto. O que mudou é que passaram a ter dono nativo e a compartilhar a
/// autoridade gramatical — não o domínio numérico, que permanece `u64`.
pub mod plano {
    /// `ler_json_plano_bombom(verso) -> mapa<verso,bombom>`.
    pub const LER: &str = "ler_json_plano_bombom";
    /// `emitir_json_plano_bombom(mapa<verso,bombom>) -> verso`.
    pub const EMITIR: &str = "emitir_json_plano_bombom";
    /// Símbolo nativo de [`LER`].
    pub const SIMBOLO_LER: &str = "pinker_json_plano_ler";
    /// Símbolo nativo de [`EMITIR`].
    pub const SIMBOLO_EMITIR: &str = "pinker_json_plano_emitir";
}

/// Todas as intrínsecas não falíveis, em ordem estável.
pub const ACESSORES: [&str; 11] = [
    intrinsecas::EMITIR,
    intrinsecas::TIPO,
    intrinsecas::VERSO,
    intrinsecas::NUMERO,
    intrinsecas::LOGICA,
    intrinsecas::LISTA_TAMANHO,
    intrinsecas::LISTA_OBTER,
    intrinsecas::OBJETO_TAMANHO,
    intrinsecas::OBJETO_TEM,
    intrinsecas::OBJETO_OBTER,
    intrinsecas::OBJETO_CHAVES,
];

/// Verdadeiro para qualquer intrínseca não falível da família.
pub fn e_acessor(nome: &str) -> bool {
    ACESSORES.contains(&nome)
}

/// Assinatura operacional de um acessor: retorno e parâmetros, na ordem.
///
/// Declarada **uma vez**. IR, validação de IR, validação de CFG, validação de
/// seleção e validação da máquina abstrata derivam desta função em vez de
/// repetir a tabela por camada — que é como uma camada acaba discordando das
/// outras sem que ninguém perceba.
pub fn assinatura_ir(nome: &str) -> Option<(crate::ir::TypeIR, Vec<crate::ir::TypeIR>)> {
    use crate::ir::TypeIR;
    let handle = TypeIR::OpaqueWordHandle;
    let assinatura = match nome {
        intrinsecas::EMITIR | intrinsecas::VERSO => (TypeIR::Verso, vec![handle]),
        // O leque sem carga abaixa para o discriminante, uma palavra.
        intrinsecas::TIPO => (TypeIR::Bombom, vec![handle]),
        intrinsecas::NUMERO => (TypeIR::I64, vec![handle]),
        intrinsecas::LOGICA => (TypeIR::Logica, vec![handle]),
        intrinsecas::LISTA_TAMANHO | intrinsecas::OBJETO_TAMANHO => (TypeIR::Bombom, vec![handle]),
        intrinsecas::LISTA_OBTER => (handle, vec![handle, TypeIR::Bombom]),
        intrinsecas::OBJETO_TEM => (TypeIR::Logica, vec![handle, TypeIR::Verso]),
        intrinsecas::OBJETO_OBTER => (handle, vec![handle, TypeIR::Verso]),
        intrinsecas::OBJETO_CHAVES => (TypeIR::ListVerso, vec![handle]),
        _ => return None,
    };
    Some(assinatura)
}

/// Símbolo do runtime nativo que implementa cada acessor.
///
/// Declarado ao lado do nome público e da assinatura, e não num `match` solto
/// dentro do backend: a ausência de uma entrada aqui foi **exatamente** o
/// defeito histórico da família JSON — o backend tratava a intrínseca como
/// função Pinker do usuário, não a encontrava e recusava o programa que o
/// interpretador aceitava.
pub fn simbolo_runtime(nome: &str) -> Option<&'static str> {
    let simbolo = match nome {
        // As duas históricas: sem dono nativo, elas eram recusadas pelo backend
        // como "call para função inexistente" enquanto o interpretador as
        // aceitava. Esta entrada é o fecho dessa divergência.
        plano::LER => plano::SIMBOLO_LER,
        plano::EMITIR => plano::SIMBOLO_EMITIR,
        intrinsecas::EMITIR => "pinker_json_emitir",
        intrinsecas::TIPO => "pinker_json_tipo",
        intrinsecas::VERSO => "pinker_json_verso",
        intrinsecas::NUMERO => "pinker_json_numero",
        intrinsecas::LOGICA => "pinker_json_logica",
        intrinsecas::LISTA_TAMANHO => "pinker_json_lista_tamanho",
        intrinsecas::LISTA_OBTER => "pinker_json_lista_obter",
        intrinsecas::OBJETO_TAMANHO => "pinker_json_objeto_tamanho",
        intrinsecas::OBJETO_TEM => "pinker_json_objeto_tem",
        intrinsecas::OBJETO_OBTER => "pinker_json_objeto_obter",
        intrinsecas::OBJETO_CHAVES => "pinker_json_objeto_chaves",
        _ => return None,
    };
    Some(simbolo)
}
// @pinker-nav:end json.superficie.nomes

// @pinker-nav:start evidencia.json.nomes-e-assinaturas
// @pinker-nav:domain dados
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência de que o conjunto de acessores é reconhecido por uma única declaração, de que a intrínseca falível pertence a outra autoridade e de que toda intrínseca declarada possui assinatura operacional — nenhuma camada pode registrar um nome sem tipo.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acessores_sao_reconhecidos_por_uma_unica_declaracao() {
        for nome in ACESSORES {
            assert!(e_acessor(nome));
        }
        assert!(
            !e_acessor(crate::falha_operacional::LER_JSON_RESULTADO),
            "a falível pertence a `falha_operacional`"
        );
        assert!(!e_acessor("ler_json_plano_bombom"));
    }

    /// Nome declarado sem assinatura seria um nome que alguma camada registra
    /// e outra não sabe tipar.
    #[test]
    fn todo_acessor_possui_assinatura() {
        for nome in ACESSORES {
            assert!(
                assinatura_ir(nome).is_some(),
                "acessor sem assinatura: {nome}"
            );
        }
        assert!(assinatura_ir("ler_json_plano_bombom").is_none());
    }

    /// Acessor sem símbolo nativo é a forma exata do defeito histórico da
    /// família JSON: o interpretador aceita e o backend recusa.
    ///
    /// As duas superfícies planas entram aqui de propósito — a ausência delas
    /// **era** o defeito. Um nome que não pertence à família continua sem
    /// símbolo, para que o teste não passe por mapear tudo.
    #[test]
    fn toda_superficie_json_possui_simbolo_nativo() {
        for nome in ACESSORES {
            assert!(
                simbolo_runtime(nome).is_some(),
                "acessor sem símbolo nativo: {nome}"
            );
        }
        for nome in [plano::LER, plano::EMITIR] {
            assert!(
                simbolo_runtime(nome).is_some(),
                "superfície plana histórica sem símbolo nativo: {nome}"
            );
        }
        assert!(simbolo_runtime("mapa_verso_bombom_criar").is_none());
    }

    /// A reserva das duas identidades não é opcional: sem ela o usuário poderia
    /// redeclarar o nome e reinterpretar valores produzidos pelo runtime.
    #[test]
    fn identidades_sao_reservadas_pelo_runtime() {
        use crate::runtime_identity::{runtime_reserved_identity, RuntimeSemanticKind};
        assert_eq!(
            runtime_reserved_identity(TIPO_VALOR_JSON).map(|id| id.kind),
            Some(RuntimeSemanticKind::OpaqueWordHandle)
        );
        assert_eq!(
            runtime_reserved_identity(LEQUE_TIPO_JSON).map(|id| id.kind),
            Some(RuntimeSemanticKind::PlainEnum)
        );
    }
}
// @pinker-nav:end evidencia.json.nomes-e-assinaturas
