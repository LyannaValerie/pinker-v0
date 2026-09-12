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
//! - O **símbolo ABI** NÃO mora aqui. No baseline a relação `operação interna ->
//!   símbolo do runtime` tinha um decisor só, `backend_s`, e a #651 só autoriza
//!   consolidar o binding quando várias fases repetem a MESMA relação. Que o
//!   backend também enumere operações internas não torna o binding uma decisão
//!   duplicada:
//!
//!   ```text
//!   OPERATION EXISTS
//!   !=
//!   THIS OPERATION BINDS TO THIS ABI SYMBOL
//!   ```
//!
//!   Recolher autoridade única para uma tabela nova porque ela caberia bem ali
//!   é o oposto do que TC-01 autoriza. O símbolo continua com `backend_s`.
//! - Os **corpos** — interpretador hospedado e `pinker_rt` — continuam com seus
//!   donos de fase. Esta autoridade é declarativa e não executa nada.
//! - A relação `(classe concreta de mapa, operação genérica) -> grafia
//!   monomórfica` **não** mora aqui: desde U-02 ela tem autoridade própria em
//!   [`crate::map_specialization`], cujo codomínio é a superfície pública C1.
//!   Nenhuma grafia desta tabela é alvo dela, e o fallback adulto do mapa
//!   genérico — `TypeIR::Map { key, .. }` — continua sendo respondido por estas
//!   operações internas, não por especialização monomórfica.
//!
//! O fato que esta autoridade centraliza é o contrato estrutural — existência,
//! aridade, operandos e resultado — que antes era decidido
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
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_criar_chave_verso",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[]),
        result: InternalResult::MapaNovoComChave(MapKeyIR::Verso),
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
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_obter",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::ValorDoMapaReceptor,
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_tem",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::Declarado(TypeIR::Logica),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_remover",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[
            MapOperandRole::Receptor,
            MapOperandRole::Chave,
        ]),
        result: InternalResult::Declarado(TypeIR::Nulo),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_tamanho",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[MapOperandRole::Receptor]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_criar",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::PapeisDeMapa(&[MapOperandRole::Receptor]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    // O cursor já é `bombom`: estas duas são da família genérica porque o
    // desugaring genérico as emite, mas o contrato delas é fixo.
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_proxima_chave_bombom",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_iterador_proxima_chave_verso",
        family: InternalOperationFamily::MapaGenerica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
    },
    // -- Mapa monomórfico (uma operação por classe concreta) ----------------
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_bombom_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapVersoBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_verso_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapVersoVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_verso_verso_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_bombom_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapBombomBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_verso_iterador_criar",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::MapBombomVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave",
        family: InternalOperationFamily::MapaMonomorfica,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    // -- Leque com carga ----------------------------------------------------
    InternalOperation {
        spelling: "__pinker_internal_leque_criar_0",
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: "__pinker_internal_leque_tag",
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    // As cinco formas de anexo e as cinco de carga existem porque o tipo do
    // operando/resultado difere; todas colapsam nos mesmos dois símbolos do
    // runtime, que movem uma palavra sem interpretar o conteúdo.
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_IMEDIATO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Verso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_LISTA_BOMBOM,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::ListBombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_LISTA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::ListVerso]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::ANEXAR_SAIDA_PROCESSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::OpaqueWordHandle]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_IMEDIATO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Verso),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_LISTA_BOMBOM,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::ListBombom),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_LISTA_VERSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::ListVerso),
    },
    InternalOperation {
        spelling: crate::enum_payload::CARGA_SAIDA_PROCESSO,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::OpaqueWordHandle),
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
    },
];

/// Grafia da escolha ternária. Declarada aqui porque é a autoridade que
/// descreve o contrato dela; a reserva do nome continua em `native_symbol`.
pub const TERNARIA: &str = "__ternario";

/// Contrato estrutural da grafia, quando ela é uma operação interna.
#[cfg_attr(test, track_caller)]
pub fn entrada(spelling: &str) -> Option<&'static InternalOperation> {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    contract_table()
        .iter()
        .find(|operation| operation.spelling == spelling)
}

/// A grafia é uma operação interna declarada?
#[cfg_attr(test, track_caller)]
pub fn e_operacao_interna(spelling: &str) -> bool {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    entrada(spelling).is_some()
}

/// A grafia é a escolha ternária?
#[cfg_attr(test, track_caller)]
pub fn e_ternaria(spelling: &str) -> bool {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    spelling == TERNARIA
}

/// A grafia é uma operação de mapa cujo contrato depende do mapa recebido?
///
/// É a pergunta que `ir::model::is_generic_map_intrinsic` respondia por lista
/// literal e que os três validadores consumiam.
#[cfg_attr(test, track_caller)]
pub fn e_operacao_generica_de_mapa(spelling: &str) -> bool {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    entrada(spelling)
        .is_some_and(|operation| operation.family == InternalOperationFamily::MapaGenerica)
}

/// Aridade declarada da operação interna.
#[cfg_attr(test, track_caller)]
pub fn aridade(spelling: &str) -> Option<usize> {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    entrada(spelling).map(InternalOperation::arity)
}

/// Retorno e parâmetros IR da operação, quando o contrato é fixo.
#[cfg_attr(test, track_caller)]
pub fn assinatura_ir(spelling: &str) -> Option<(TypeIR, &'static [TypeIR])> {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    entrada(spelling).and_then(InternalOperation::assinatura_ir)
}

/// Todas as operações com contrato fixo, para as tabelas de assinatura das
/// fases que precisam de `(ret, params)` completos.
#[cfg_attr(test, track_caller)]
pub fn assinaturas_declaradas(
) -> impl Iterator<Item = (&'static str, TypeIR, &'static [TypeIR])> + Clone {
    #[cfg(test)]
    registro::registrar(std::panic::Location::caller());
    contract_table().iter().filter_map(|operation| {
        operation
            .assinatura_ir()
            .map(|(ret, params)| (operation.spelling, ret, params))
    })
}

// ---------------------------------------------------------------------------
// Costura metamórfica — apenas sob `cfg(test)`.
//
// Toda consulta desta autoridade passa por `contract_table()`. Em produção a
// função É a tabela: `#[cfg(not(test))]` devolve `INTERNAL_OPERATIONS` e o
// otimizador não vê indireção alguma. Nenhum dado de produção muda de forma,
// nenhum `trait` novo atravessa a Pinker e nenhuma fase precisa ser genérica.
//
// Sob `cfg(test)`, a mesma função consulta antes uma variante MUTADA do
// contrato, instalada por teste e por thread. É essa costura que torna a
// pergunta terminal de G651-02 executável:
//
// ```text
// MUTATE(CANONICAL_FACT) -> ALL_RELEVANT_CONSUMERS_OBSERVE_MUTATION
// ```
//
// Um consumidor que SOMBREIE a autoridade — decida no lugar dela — continua
// respondendo o valor ANTIGO sob a mutação, e o oráculo de
// `metamorphic_oracle` fica vermelho sem precisar reconhecer a forma sintática
// da decisão local. Um que decida ADITIVAMENTE ao lado dela, concordando no
// estado canônico, não aparece nessa metade: é a mutação que ADMITE um caso
// novo — e a testemunha que só ela aceita — que o denuncia. O oráculo prova as
// duas direções, e o domínio exato de cada uma está declarado no cabeçalho
// dele.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
#[inline]
fn contract_table() -> &'static [InternalOperation] {
    INTERNAL_OPERATIONS
}

#[cfg(test)]
use contract_seam::contract_table;

#[cfg(test)]
pub(crate) mod contract_seam {
    use super::{InternalOperation, INTERNAL_OPERATIONS};
    use std::cell::Cell;

    thread_local! {
        /// Contrato mutado vigente nesta thread de teste, quando existe.
        ///
        /// Ser `thread_local` é o que permite que `cargo test` rode as provas
        /// metamórficas em paralelo sem que uma mutação vaze para outra.
        static MUTATED: Cell<Option<&'static [InternalOperation]>> = const { Cell::new(None) };
    }

    /// A tabela que TODA consulta da autoridade enxerga agora.
    pub(crate) fn contract_table() -> &'static [InternalOperation] {
        MUTATED.with(Cell::get).unwrap_or(INTERNAL_OPERATIONS)
    }

    /// Instala uma variante mutada do contrato canônico até ser descartada.
    ///
    /// O escopo é o do valor devolvido: ao sair, a autoridade real volta a
    /// valer. Nenhum teste precisa desfazer a mutação à mão.
    #[must_use = "a mutação vale enquanto o guarda existir"]
    pub(crate) struct MutatedContract;

    impl MutatedContract {
        pub(crate) fn install(operations: Vec<InternalOperation>) -> Self {
            let mutated: &'static [InternalOperation] = Box::leak(operations.into_boxed_slice());
            MUTATED.with(|cell| cell.set(Some(mutated)));
            Self
        }
    }

    impl Drop for MutatedContract {
        fn drop(&mut self) {
            MUTATED.with(|cell| cell.set(None));
        }
    }
}

// ---------------------------------------------------------------------------
// Registro de consulta — descoberta independente de consumidores, só sob
// `cfg(test)`.
//
// A pergunta “quem decide F1–F4?” não pode ser respondida pela mesma lista que
// o oráculo audita: foi essa circularidade que o fechamento dirigido de
// `acb6ac6` derrubou, removendo um consumidor real da lista manual sem que
// nada ficasse vermelho.
//
// Aqui a resposta vem da própria autoridade: toda consulta pública anota o
// ARQUIVO de quem perguntou. `#[cfg_attr(test, track_caller)]` mantém produção
// intacta — fora de teste nem o atributo nem a anotação existem. Consultas
// desta autoridade a si mesma são descartadas, para que o registro contenha só
// consumidores de verdade.
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod registro {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::panic::Location;

    thread_local! {
        /// Arquivos que consultaram a autoridade enquanto há gravação aberta.
        static CONSULTAS: RefCell<Option<BTreeSet<&'static str>>> = const { RefCell::new(None) };
    }

    /// Anota o arquivo do consumidor, quando há gravação aberta nesta thread.
    pub(crate) fn registrar(local: &'static Location<'static>) {
        let arquivo = local.file();
        if arquivo.starts_with("src/internal_operations") {
            return;
        }
        CONSULTAS.with(|celula| {
            if let Some(consultas) = celula.borrow_mut().as_mut() {
                consultas.insert(arquivo);
            }
        });
    }

    /// Gravação aberta: enquanto viver, as consultas desta thread são anotadas.
    #[must_use = "a gravação vale enquanto o guarda existir"]
    pub(crate) struct Gravacao;

    impl Gravacao {
        pub(crate) fn abrir() -> Self {
            CONSULTAS.with(|celula| *celula.borrow_mut() = Some(BTreeSet::new()));
            Self
        }

        /// Fecha a gravação e devolve os arquivos observados, em ordem estável.
        pub(crate) fn fechar(self) -> Vec<&'static str> {
            CONSULTAS
                .with(|celula| celula.borrow_mut().take())
                .unwrap_or_default()
                .into_iter()
                .collect()
        }
    }

    impl Drop for Gravacao {
        fn drop(&mut self) {
            CONSULTAS.with(|celula| *celula.borrow_mut() = None);
        }
    }
}

#[cfg(test)]
mod metamorphic_oracle;
