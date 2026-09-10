//! Autoridade declarativa das **operações internas do compilador** (U-01).
//!
//! Fronteira semântica, declarada uma vez para que nenhuma fase precise
//! reconstruí-la:
//!
//! ```text
//! COMPILER_INTERNAL_CLASS      != INTERNAL_OPERATION_IDENTITY
//! INTERNAL_OPERATION_IDENTITY  != PUBLIC_INTRINSIC_IDENTITY
//! INTERNAL_OPERATION_IDENTITY  != ABI_SYMBOL
//! INTERNAL_OPERATION_IDENTITY  != RUNTIME_IMPLEMENTATION
//! INTERNAL_OPERATION_IDENTITY  != INTERPRETER_IMPLEMENTATION
//! ```
//!
//! - A **classe** do callee continua sendo de
//!   [`crate::intrinsics::identity::CalleeIdentity::CompilerInternal`], que por
//!   sua vez continua derivando de [`crate::native_symbol::is_compiler_generated`].
//!   Esta autoridade **não** classifica callee: ela só descreve o contrato
//!   estrutural das operações que a classe já admitiu.
//! - A **superfície pública** (C1) permanece inteiramente em
//!   [`crate::intrinsics::registry`] e [`crate::intrinsics::public_surface`].
//!   Nenhuma grafia desta tabela é pública, nenhuma grafia de lá entra aqui, e
//!   as duas tabelas nunca se consultam. A fronteira é física também: esta
//!   autoridade vive FORA de `src/intrinsics/`, cuja família a #588 fechou, e
//!   ao lado das demais autoridades de família — `valor_json`, `sha256`,
//!   `falha_operacional`, `enum_payload`, `native_symbol`.
//! - O **símbolo ABI** é uma projeção de backend declarada ao lado da operação,
//!   pela mesma disciplina de [`crate::valor_json::simbolo_runtime`]. O símbolo
//!   é consequência da identidade, nunca a identidade: nenhuma consulta desta
//!   autoridade aceita um símbolo `pinker_*` como chave.
//! - Os **corpos** — interpretador hospedado e `pinker_rt` — continuam com seus
//!   donos de fase. Esta autoridade é declarativa e não executa nada.
//! - A relação `(classe concreta de mapa, operação genérica) -> grafia
//!   monomórfica` **não** mora aqui: é objeto de U-02 e continua onde está.
//!
//! O fato que esta autoridade centraliza é o contrato estrutural — existência,
//! aridade, operandos, resultado e binding nativo — que antes era decidido
//! independentemente por `ir_validate`, `cfg_ir_validate`,
//! `abstract_machine_validate`, `instr_select_validate`, `ir::context`,
//! `ir::model` e `backend_s`, cada um com sua própria tabela literal.

use crate::ir::{MapKeyIR, TypeIR};

/// Família estrutural de uma operação interna.
///
/// Classifica a FORMA do contrato, não o domínio de dados: `MapaGenerica` é a
/// família cujo contrato depende do mapa recebido, e é por isso que ela existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalOperationFamily {
    /// Operações de leque com carga materializadas pelo desugaring de `encaixe`
    /// e pela construção de variantes.
    Leque,
    /// Operações de mapa com contrato fixo, uma por classe concreta de mapa.
    MapaMonomorfica,
    /// Operações de mapa cujo contrato depende do mapa recebido no operando 0.
    MapaGenerica,
    /// A escolha ternária que o lowering materializa.
    Ternaria,
}

/// Papel de um operando cujo tipo é relativo ao mapa recebido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapOperandRole {
    /// O próprio mapa. Sempre o operando 0.
    Receptor,
    /// A chave do mapa recebido.
    Chave,
    /// O valor do mapa recebido.
    Valor,
}

/// Contrato de operandos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalOperands {
    /// Tipos IR declarados, na ordem. A aridade é o tamanho da lista.
    Declarados(&'static [TypeIR]),
    /// Papéis relativos ao mapa recebido no operando 0. A aridade é o tamanho
    /// da lista; os tipos concretos são derivados pela fase a partir do mapa,
    /// com o predicado de comparação de cada fase.
    PapeisDeMapa(&'static [MapOperandRole]),
    /// `(condição lógica, ramo, ramo)`. Os dois ramos determinam o resultado.
    Ramificacao,
}

/// Contrato de resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalResult {
    /// Tipo IR declarado. `Nulo` significa operação sem valor.
    Declarado(TypeIR),
    /// Mapa novo com esta chave; o tipo do valor é livre.
    MapaNovoComChave(MapKeyIR),
    /// Tipo do valor do mapa recebido no operando 0.
    ValorDoMapaReceptor,
    /// Tipo dos ramos.
    TipoDosRamos,
}

/// Uma operação interna e todo o seu contrato estrutural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalOperation {
    /// Grafia com que a operação viaja entre as fases. É transporte, não
    /// identidade de fonte: `native_symbol` recusa este namespace na fronteira
    /// léxica, então nenhum programa pode escrevê-la.
    pub spelling: &'static str,
    pub family: InternalOperationFamily,
    pub operands: InternalOperands,
    pub result: InternalResult,
    /// Projeção de backend: símbolo do runtime nativo que implementa a
    /// operação. `None` quando o lowering a resolve sem chamada de runtime.
    pub runtime_symbol: Option<&'static str>,
}

impl InternalOperation {
    /// Número de operandos exigido em qualquer fase.
    pub fn arity(&self) -> usize {
        match self.operands {
            InternalOperands::Declarados(params) => params.len(),
            InternalOperands::PapeisDeMapa(roles) => roles.len(),
            InternalOperands::Ramificacao => 3,
        }
    }

    /// Tipos IR dos operandos, quando o contrato é fixo.
    pub fn declared_params(&self) -> Option<&'static [TypeIR]> {
        match self.operands {
            InternalOperands::Declarados(params) => Some(params),
            InternalOperands::PapeisDeMapa(_) | InternalOperands::Ramificacao => None,
        }
    }

    /// Tipo IR do resultado, quando o contrato é fixo.
    pub fn declared_ret(&self) -> Option<TypeIR> {
        match self.result {
            InternalResult::Declarado(ret) => Some(ret),
            InternalResult::MapaNovoComChave(_)
            | InternalResult::ValorDoMapaReceptor
            | InternalResult::TipoDosRamos => None,
        }
    }

    /// Retorno e parâmetros, quando os dois são fixos.
    ///
    /// É esta a forma que as cinco tabelas de assinatura consumiam por literal.
    pub fn assinatura_ir(&self) -> Option<(TypeIR, &'static [TypeIR])> {
        match (self.declared_ret(), self.declared_params()) {
            (Some(ret), Some(params)) => Some((ret, params)),
            _ => None,
        }
    }

    /// A operação produz valor?
    ///
    /// `Declarado(Nulo)` é o único contrato sem valor.
    pub fn returns_value(&self) -> bool {
        !matches!(self.result, InternalResult::Declarado(TypeIR::Nulo))
    }
}

/// Todas as operações internas, em ordem estável por família.
///
/// A tabela é exaustiva por construção: `LAW-01` prova que o conjunto declarado
/// aqui é exatamente o conjunto de grafias internas usadas por `src/**`.
pub const INTERNAL_OPERATIONS: &[InternalOperation] = &[
    // -- Mapa genérico (contrato relativo ao mapa recebido) -----------------
    InternalOperation {
        spelling: "__pinker_internal_mapa_criar_chave_bombom",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[]),
        result: InternalResult::MapaNovoComChave(MapKeyIR::Bombom),
        runtime_symbol: Some("pinker_mapa_criar_chave_bombom"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_criar_chave_verso",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[]),
        result: InternalResult::MapaNovoComChave(MapKeyIR::Verso),
        runtime_symbol: Some("pinker_mapa_criar_chave_verso"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_definir",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
            MapOperandRole::Valor,
        ]),
        result: InternalResult::Declarado(TypeIR::Nulo),
        runtime_symbol: Some("pinker_mapa_definir"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_obter",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::ValorDoMapaReceptor,
        runtime_symbol: Some("pinker_mapa_obter"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_tem",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::Declarado(TypeIR::Logica),
        runtime_symbol: Some("pinker_mapa_tem"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_remover",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::Declarado(TypeIR::Nulo),
        runtime_symbol: Some("pinker_mapa_remover"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_tamanho",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[MapOperandRole::Receptor]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_tamanho"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_criar",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[MapOperandRole::Receptor]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_criar"),
    },
    // O cursor já é `bombom`: estas duas são da família genérica porque o
    // desugaring genérico as emite, mas o contrato delas é fixo.
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_proxima_chave_bombom",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_proxima_chave_verso",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    // -- Mapa monomórfico (uma operação por classe concreta) ----------------
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_bombom_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapVersoBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_criar"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_verso_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapVersoVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_criar"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_verso_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_bombom_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapBombomBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_criar"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_verso_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapBombomVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_criar"),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_mapa_iterador_proxima"),
    },
    // -- Leque com carga ----------------------------------------------------
    InternalOperation {
        spelling: "__pinker_internal_leque_criar_0",
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_criar_0"),
    },
    InternalOperation {
        spelling: "__pinker_internal_leque_tag",
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_tag"),
    },
    // As cinco formas de anexo e as cinco de carga existem porque o tipo do
    // operando/resultado difere; todas colapsam nos mesmos dois símbolos do
    // runtime, que movem uma palavra sem interpretar o conteúdo.
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_IMEDIATO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_anexar"),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Verso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_anexar"),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_LISTA_BOMBOM,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::ListBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_anexar"),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_LISTA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::ListVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_anexar"),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_SAIDA_PROCESSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::OpaqueWordHandle]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_anexar"),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_IMEDIATO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
        runtime_symbol: Some("pinker_leque_carga"),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
        runtime_symbol: Some("pinker_leque_carga"),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_LISTA_BOMBOM,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::ListBombom),
        runtime_symbol: Some("pinker_leque_carga"),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_LISTA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::ListVerso),
        runtime_symbol: Some("pinker_leque_carga"),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_SAIDA_PROCESSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::OpaqueWordHandle),
        runtime_symbol: Some("pinker_leque_carga"),
    },
    // -- Escolha ternária ---------------------------------------------------
    //
    // Prova de que esta autoridade não é "o prefixo `__pinker_internal_`": a
    // escolha ternária é uma operação interna com contrato próprio e grafia de
    // outra forma reservada.
    InternalOperation {
        spelling: TERNARIA,
        family: InternalOperationFamily::Ternaria,
        operands: InternalOperands::Ramificacao,
        result: InternalResult::TipoDosRamos,
        // O lowering materializa a escolha em fluxo de controle; não há
        // chamada de runtime.
        runtime_symbol: None,
    },
];

/// Grafia da escolha ternária. Declarada aqui porque é a autoridade que
/// descreve o contrato dela; a reserva do nome continua em `native_symbol`.
pub const TERNARIA: &str = "__ternario";

/// Contrato estrutural da grafia, quando ela é uma operação interna.
pub fn entrada(spelling: &str) -> Option<&'static InternalOperation> {
    INTERNAL_OPERATIONS
        .iter()
        .find(|operation| operation.spelling == spelling)
}

/// A grafia é uma operação interna declarada?
pub fn e_operacao_interna(spelling: &str) -> bool {
    entrada(spelling).is_some()
}

/// A grafia é a escolha ternária?
pub fn e_ternaria(spelling: &str) -> bool {
    spelling == TERNARIA
}

/// A grafia é uma operação de mapa cujo contrato depende do mapa recebido?
///
/// É a pergunta que `ir::model::is_generic_map_intrinsic` respondia por lista
/// literal e que os três validadores consumiam.
pub fn e_operacao_generica_de_mapa(spelling: &str) -> bool {
    entrada(spelling)
        .is_some_and(|operation| operation.family == InternalOperationFamily::MapaGenerica)
}

/// Aridade declarada da operação interna.
pub fn aridade(spelling: &str) -> Option<usize> {
    entrada(spelling).map(InternalOperation::arity)
}

/// Retorno e parâmetros IR da operação, quando o contrato é fixo.
pub fn assinatura_ir(spelling: &str) -> Option<(TypeIR, &'static [TypeIR])> {
    entrada(spelling).and_then(InternalOperation::assinatura_ir)
}

/// Símbolo do runtime nativo que implementa a operação.
///
/// A consulta é sempre pela identidade interna. O símbolo nunca é chave.
pub fn simbolo_runtime(spelling: &str) -> Option<&'static str> {
    entrada(spelling).and_then(|operation| operation.runtime_symbol)
}

/// Todas as operações com contrato fixo, para as tabelas de assinatura das
/// fases que precisam de `(ret, params)` completos.
pub fn assinaturas_declaradas(
) -> impl Iterator<Item = (&'static str, TypeIR, &'static [TypeIR])> + Clone {
    INTERNAL_OPERATIONS.iter().filter_map(|operation| {
        operation
            .assinatura_ir()
            .map(|(ret, params)| (operation.spelling, ret, params))
    })
}
