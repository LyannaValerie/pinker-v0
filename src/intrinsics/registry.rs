//! Autoridade declarativa única do binding das intrínsecas históricas.
//!
//! # O que esta tabela decide
//!
//! Para cada grafia histórica, o binding declarativo — existência, contrato de
//! parâmetros, contrato de retorno e roteamento de runtime — nasce **aqui** e é
//! consumido pelas fases. Antes da consolidação C1 o mesmo fato existia sete
//! vezes: `semantic`, `ir`, `ir_validate`, `cfg_ir_validate`,
//! `instr_select_validate`, `abstract_machine_validate` e `backend_s`
//! enumeravam cada um a sua cópia, e nada impedia que discordassem.
//!
//! ```text
//! REGISTRY                       = SOURCE OF DECLARATIVE TRUTH
//! SEMANTIC/IR/BACKEND/VALIDATORS = CONSUMERS OR DERIVED VIEWS
//! ```
//!
//! # O que esta tabela NÃO decide
//!
//! Identidade continua em [`crate::intrinsics::identity`]: quem responde SE uma
//! chamada é intrínseca é a `CalleeIdentity`, nunca o texto. Superfície pública
//! `(módulo, membro)` continua em [`crate::intrinsics::public_surface`]. E os
//! **corpos** continuam com seus donos: o interpretador hospeda a execução e o
//! `pinker_rt` implementa o símbolo nativo.
//!
//! ```text
//! HOSTED_IMPLEMENTATION != DECLARATIVE_BINDING
//! NATIVE_IMPLEMENTATION != DECLARATIVE_BINDING
//! ```
//!
//! O modelo segue os precedentes adultos já existentes no repositório —
//! [`crate::falha_operacional`], [`crate::valor_json`], [`crate::sha256`] e
//! [`crate::saida_processo`] —, que declaram nome, assinatura e símbolo numa
//! autoridade só e são consultados pelas fases.

use crate::ir::TypeIR;

/// Política de aridade de uma grafia histórica.
///
/// A realidade não é uniforme, e forçá-la a `params.len()` seria inventar um
/// contrato que a linguagem não tem: cinco grafias escolhem o símbolo do
/// runtime pela aridade do call site e `formatar_verso` é variádica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArityPolicy {
    /// Exatamente o tamanho do contrato de parâmetros.
    Exact,
    /// Recorte fechado de aridades aceitas, na ordem declarada.
    Subset(&'static [usize]),
    /// Aridade variável com mínimo: o contrato de parâmetros descreve só o
    /// prefixo fixo, e as fases decidem os argumentos restantes.
    AtLeast(usize),
}

/// Contrato de assinatura de uma grafia histórica.
///
/// A variante genérica existe porque a realidade tem formas polimórficas: as
/// grafias `lista_*`/`mapa_*` sem sufixo de elemento são monomorfizadas pela IR
/// antes das tabelas de assinatura, e por isso nunca aparecem nelas. Forçá-las
/// a uma assinatura fixa seria inventar um contrato que a linguagem não tem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    /// Aridade exata e tipos declarados, iguais em todas as fases de assinatura.
    Declared {
        ret: TypeIR,
        params: &'static [TypeIR],
        arity: ArityPolicy,
        /// Posições em que a máquina abstrata aceita qualquer tipo de pilha.
        ///
        /// Dívida histórica preservada exatamente: dez entradas relaxam o
        /// handle de lista do argumento 0 para `Unknown`, enquanto duas
        /// entradas equivalentes exigem o tipo derivado. Registrar o desvio
        /// aqui mantém o comportamento idêntico e torna a divergência
        /// auditável num lugar só, em vez de invisível em oitocentas linhas de
        /// tabela manual.
        machine_relaxed_params: &'static [usize],
    },
    /// Forma genérica reescrita pela IR antes das fases de assinatura.
    GenericMonomorphized,
}

/// Como o backend nativo encontra o símbolo do runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRouting {
    /// Símbolo fixo, independente da aridade do call site.
    Symbol(&'static str),
    /// O símbolo varia com o número de argumentos: `prefixo` concatenado à
    /// aridade do call site, dentro do [`ArityPolicy::Subset`] declarado.
    ByArity { prefixo: &'static str },
    /// Sem símbolo próprio nesta autoridade: forma genérica monomorfizada,
    /// exclusão de stdin do subset montável, empacotamento próprio
    /// (`formatar_verso`) ou símbolo declarado por outra autoridade de família
    /// (o recorte plano de [`crate::valor_json`]).
    NotRouted,
}

/// Quem valida a chamada na fase semântica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticContract {
    /// Aridade e tipos vêm do registry; a checagem é a genérica.
    Declared,
    /// Contrato próprio: aridade variável, tipagem polimórfica ou restrição
    /// semântica que não cabe em `(params, ret)`. A fase mantém o corpo; o
    /// registry mantém a existência.
    PhaseSpecific,
}

/// Uma grafia histórica e todo o seu binding declarativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoricalIntrinsic {
    pub spelling: &'static str,
    pub signature: Signature,
    pub runtime: RuntimeRouting,
    pub semantic: SemanticContract,
}

impl HistoricalIntrinsic {
    /// Retorno e parâmetros, quando a grafia tem contrato declarado.
    pub fn assinatura_ir(&self) -> Option<(TypeIR, &'static [TypeIR])> {
        match self.signature {
            Signature::Declared { ret, params, .. } => Some((ret, params)),
            Signature::GenericMonomorphized => None,
        }
    }

    /// Posições relaxadas para a máquina abstrata.
    pub fn machine_relaxed_params(&self) -> &'static [usize] {
        match self.signature {
            Signature::Declared {
                machine_relaxed_params,
                ..
            } => machine_relaxed_params,
            Signature::GenericMonomorphized => &[],
        }
    }
}

/// Todas as grafias históricas, em ordem estável.
///
/// A ordem é a mesma da lista que esta tabela substituiu, para que a
/// enumeração da superfície pública não mude de ordem por causa da
/// consolidação.
pub const HISTORICAL: &[HistoricalIntrinsic] = &[
    HistoricalIntrinsic {
        spelling: "abrir",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_abrir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "abrir_anexo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_abrir_anexo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "afirmar",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Subset(&[1, 2]),
            params: &[TypeIR::Logica],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::ByArity {
            prefixo: "pinker_afirmar_",
        },
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "aleatorio_criar",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_aleatorio_criar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "aleatorio_entre",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_aleatorio_entre"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "aleatorio_proximo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_aleatorio_proximo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "alocar",
        signature: Signature::Declared {
            ret: TypeIR::Pointer { is_volatile: false },
            arity: ArityPolicy::Exact,
            params: &[TypeIR::U64],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_publico_alocar"),
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "ambiente_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_ou"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "anexar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_anexar_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "aparar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_aparar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "argumento",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_argumento"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "argumento_nomeado_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_pedir_argumento"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "argumento_nomeado_ou_ambiente_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_buscar_contexto"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "argumento_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_argumento_ou"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "arquivo_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_ou"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "bombom_para_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_bombom_para_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "buscar_contexto",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_buscar_contexto"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "buscar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_buscar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "caminho_existe",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_existe"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "capturar_stderr",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Subset(&[1, 2]),
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::ByArity {
            prefixo: "pinker_processo_capturar_stderr_",
        },
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "capturar_stdout",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Subset(&[1, 2]),
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::ByArity {
            prefixo: "pinker_processo_capturar_stdout_",
        },
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "comeca_com",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_comeca_com"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "contem_verso",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_contem"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "copiar_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_copiar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "criar_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_criar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "criar_diretorio",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_criar_diretorio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "diretorio_atual",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_diretorio_atual"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "dividir_verso_contar",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_dividir_contar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "dividir_verso_em",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_dividir_em"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "dormir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_dormir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "e_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_e_arquivo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "e_diretorio",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_e_diretorio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "e_vazio",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_e_vazio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "emitir_json_plano_bombom",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "emitir_linha_csv_bombom",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_emitir_linha_csv_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "escrever",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_escrever_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "escrever_verso",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_escrever_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "executar_com_entrada",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Subset(&[2, 3]),
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::ByArity {
            prefixo: "pinker_processo_com_entrada_",
        },
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "executar_processo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Subset(&[1, 2]),
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::ByArity {
            prefixo: "pinker_processo_executar_",
        },
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "fatiar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_fatiar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "fechar",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_fechar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "formatar_tempo_unix",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_formatar_tempo_unix"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "formatar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::AtLeast(2),
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "igual_verso",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_igual"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "indice_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_indice"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "indice_verso_em",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_indice_em"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "juntar_caminho",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_juntar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "juntar_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_juntar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "juntar_verso_com",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_juntar_com"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ler_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_ler_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ler_arquivo_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_ler_caminho_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ler_json_plano_bombom",
        signature: Signature::Declared {
            ret: TypeIR::MapVersoBombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ler_linha_csv_bombom",
        signature: Signature::Declared {
            ret: TypeIR::ListBombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ler_linha_csv_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ler_verso_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_ler_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "liberar",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Pointer { is_volatile: false }],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_publico_liberar"),
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_anexar",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_anexar",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom, TypeIR::Bombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_anexar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_criar",
        signature: Signature::Declared {
            ret: TypeIR::ListBombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_criar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_inserir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_inserir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_obter",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom, TypeIR::Bombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_bombom_tirar_ultimo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListBombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_tirar_ultimo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_criar",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_definir",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_inserir",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_obter",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_tamanho",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_tirar_ultimo",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_anexar",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso, TypeIR::Verso],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_anexar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_criar",
        signature: Signature::Declared {
            ret: TypeIR::ListVerso,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_criar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_inserir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_inserir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_obter",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso, TypeIR::Bombom],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "lista_verso_tirar_ultimo",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::ListVerso],
            machine_relaxed_params: &[0],
        },
        runtime: RuntimeRouting::Symbol("pinker_lista_tirar_ultimo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "maiusculo_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_maiusculo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_criar",
        signature: Signature::Declared {
            ret: TypeIR::MapBombomBombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_criar_chave_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomBombom, TypeIR::Bombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_obter",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomBombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_remover",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomBombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_remover"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomBombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_bombom_tem",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomBombom, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tem"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_criar",
        signature: Signature::Declared {
            ret: TypeIR::MapBombomVerso,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_criar_chave_bombom"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomVerso, TypeIR::Bombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_obter",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomVerso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_remover",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomVerso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_remover"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomVerso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_bombom_verso_tem",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapBombomVerso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tem"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_criar",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_definir",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_obter",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_remover",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_tamanho",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_tem",
        signature: Signature::GenericMonomorphized,
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::PhaseSpecific,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_criar",
        signature: Signature::Declared {
            ret: TypeIR::MapVersoBombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_criar_chave_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom, TypeIR::Verso, TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_obter",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_remover",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_remover"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_bombom_tem",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoBombom, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tem"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_criar",
        signature: Signature::Declared {
            ret: TypeIR::MapVersoVerso,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_criar_chave_verso"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_definir",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoVerso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_definir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_obter",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoVerso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_obter"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_remover",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoVerso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_remover"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_tamanho",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoVerso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "mapa_verso_verso_tem",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::MapVersoVerso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_mapa_tem"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "minusculo_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_minusculo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "nao_vazio_verso",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_nao_vazio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ouvir",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ouvir_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "ouvir_verso_ou",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::NotRouted,
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "pedir_argumento",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_pedir_argumento"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "pipeline_minimo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_processo_pipeline"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "quantos_argumentos",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_quantos_argumentos"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "remover_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_remover_arquivo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "remover_diretorio",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_remover_diretorio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "renomear_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_renomear"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "sair",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_sair"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "substituir_verso",
        signature: Signature::Declared {
            ret: TypeIR::Verso,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_substituir"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tamanho_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_caminho_tamanho_arquivo"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tamanho_verso",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_tamanho"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tem_argumento",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_tem_argumento"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tem_argumento_nomeado",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_tem_chave"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tem_chave",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_tem_chave"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tem_flag",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_ambiente_tem_flag"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "tempo_unix",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_tempo_unix"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "termina_com",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso, TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_termina_com"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "truncar_arquivo",
        signature: Signature::Declared {
            ret: TypeIR::Nulo,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Bombom],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_arquivo_truncar"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "vazio_verso",
        signature: Signature::Declared {
            ret: TypeIR::Logica,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_vazio"),
        semantic: SemanticContract::Declared,
    },
    HistoricalIntrinsic {
        spelling: "verso_para_bombom",
        signature: Signature::Declared {
            ret: TypeIR::Bombom,
            arity: ArityPolicy::Exact,
            params: &[TypeIR::Verso],
            machine_relaxed_params: &[],
        },
        runtime: RuntimeRouting::Symbol("pinker_verso_para_bombom"),
        semantic: SemanticContract::Declared,
    },
];

/// Entrada da grafia histórica, quando existe.
pub fn entrada(spelling: &str) -> Option<&'static HistoricalIntrinsic> {
    HISTORICAL.iter().find(|entry| entry.spelling == spelling)
}

/// A grafia pertence à superfície histórica?
pub fn e_historica(spelling: &str) -> bool {
    entrada(spelling).is_some()
}

/// Grafias históricas, em ordem estável.
pub fn grafias() -> impl Iterator<Item = &'static str> + Clone {
    HISTORICAL.iter().map(|entry| entry.spelling)
}

/// Assinatura operacional de uma grafia histórica: retorno e parâmetros.
///
/// Declarada **uma vez**. IR, validação de IR, validação de CFG, validação de
/// seleção e validação da máquina abstrata derivam desta função em vez de
/// repetir a tabela por camada — que é como uma camada acaba discordando das
/// outras sem que ninguém perceba.
pub fn assinatura_ir(spelling: &str) -> Option<(TypeIR, &'static [TypeIR])> {
    entrada(spelling).and_then(|entry| entry.assinatura_ir())
}

/// Símbolo fixo do runtime nativo, quando o roteamento não depende da aridade.
pub fn simbolo_runtime(spelling: &str) -> Option<&'static str> {
    match entrada(spelling)?.runtime {
        RuntimeRouting::Symbol(simbolo) => Some(simbolo),
        RuntimeRouting::ByArity { .. } | RuntimeRouting::NotRouted => None,
    }
}

/// O símbolo do runtime desta grafia é escolhido pela aridade do call site?
pub fn roteia_por_aridade(spelling: &str) -> bool {
    matches!(
        entrada(spelling).map(|entry| entry.runtime),
        Some(RuntimeRouting::ByArity { .. })
    )
}

/// Aridades aceitas pela grafia, quando o recorte é declarado.
///
/// `None` significa que a grafia não tem recorte próprio — a aridade dela é o
/// tamanho do contrato de parâmetros, como em qualquer outra.
pub fn aridades_aceitas(spelling: &str) -> Option<&'static [usize]> {
    match entrada(spelling)?.signature {
        Signature::Declared {
            arity: ArityPolicy::Subset(aridades),
            ..
        } => Some(aridades),
        _ => None,
    }
}

/// Aridade mínima de uma grafia variádica.
///
/// `None` para toda grafia que não é variádica — o mínimo dela é o tamanho do
/// contrato de parâmetros, e não há política própria para consultar.
pub fn aridade_minima(spelling: &str) -> Option<usize> {
    match entrada(spelling)?.signature {
        Signature::Declared {
            arity: ArityPolicy::AtLeast(minimo),
            ..
        } => Some(minimo),
        _ => None,
    }
}

/// A aridade observada está dentro do recorte declarado desta grafia?
///
/// Falso também quando a grafia não declara recorte: quem pergunta isso está
/// decidindo relaxação de aridade, e ausência de recorte não relaxa nada.
pub fn aridade_no_recorte(spelling: &str, argc: usize) -> bool {
    aridades_aceitas(spelling).is_some_and(|aridades| aridades.contains(&argc))
}

/// Símbolo do runtime para a aridade observada no call site.
///
/// `None` quando a grafia não roteia por aridade ou quando a aridade está fora
/// do recorte declarado — que são respostas diferentes para o backend, e por
/// isso ele consulta [`roteia_por_aridade`] antes.
pub fn simbolo_runtime_por_aridade(spelling: &str, argc: usize) -> Option<String> {
    match entrada(spelling)?.runtime {
        RuntimeRouting::ByArity { prefixo } => aridades_aceitas(spelling)
            .is_some_and(|aridades| aridades.contains(&argc))
            .then(|| format!("{prefixo}{argc}")),
        RuntimeRouting::Symbol(_) | RuntimeRouting::NotRouted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intrinsics::identity::{
        intrinsic_from_public_spelling, IntrinsicIdentity, HISTORICAL_CANONICAL_ALIASES,
    };
    use std::collections::BTreeSet;

    const ROTEADAS_POR_ARIDADE: [&str; 5] = [
        "afirmar",
        "capturar_stderr",
        "capturar_stdout",
        "executar_com_entrada",
        "executar_processo",
    ];

    const GENERICAS: [&str; 13] = [
        "lista_anexar",
        "lista_criar",
        "lista_definir",
        "lista_inserir",
        "lista_obter",
        "lista_tamanho",
        "lista_tirar_ultimo",
        "mapa_criar",
        "mapa_definir",
        "mapa_obter",
        "mapa_remover",
        "mapa_tamanho",
        "mapa_tem",
    ];

    #[test]
    fn toda_grafia_historica_tem_exatamente_uma_entrada() {
        let grafias: Vec<&str> = grafias().collect();
        assert_eq!(grafias.len(), 131, "a superfície histórica tem 131 grafias");
        let unicas: BTreeSet<&str> = grafias.iter().copied().collect();
        assert_eq!(unicas.len(), grafias.len(), "grafia duplicada no registry");
    }

    #[test]
    fn ordem_do_registry_e_estavel() {
        let grafias: Vec<&str> = grafias().collect();
        let mut ordenadas = grafias.clone();
        ordenadas.sort_unstable();
        assert_eq!(
            grafias, ordenadas,
            "a enumeração da superfície pública depende desta ordem"
        );
    }

    #[test]
    fn toda_entrada_e_reconhecida_como_intrinseca_historica() {
        for spelling in grafias() {
            match intrinsic_from_public_spelling(spelling) {
                Some(IntrinsicIdentity::Historical(_)) => {}
                outra => panic!("{spelling} não é identidade histórica: {outra:?}"),
            }
        }
    }

    #[test]
    fn contrato_declarado_esta_completo() {
        for entry in HISTORICAL {
            let Signature::Declared {
                params,
                machine_relaxed_params,
                ..
            } = entry.signature
            else {
                continue;
            };
            for posicao in machine_relaxed_params {
                assert!(
                    *posicao < params.len(),
                    "{}: relaxamento fora da aridade",
                    entry.spelling
                );
                assert!(
                    matches!(params[*posicao], TypeIR::ListBombom | TypeIR::ListVerso),
                    "{}: só handle de lista carrega o relaxamento histórico",
                    entry.spelling
                );
            }
        }
    }

    #[test]
    fn formas_genericas_nao_declaram_assinatura() {
        let genericas: BTreeSet<&str> = HISTORICAL
            .iter()
            .filter(|entry| entry.signature == Signature::GenericMonomorphized)
            .map(|entry| entry.spelling)
            .collect();
        assert_eq!(genericas, GENERICAS.into_iter().collect());
        for spelling in genericas {
            assert!(assinatura_ir(spelling).is_none());
        }
    }

    #[test]
    fn roteamento_por_aridade_e_o_recorte_conhecido() {
        let por_aridade: BTreeSet<&str> = HISTORICAL
            .iter()
            .filter(|entry| matches!(entry.runtime, RuntimeRouting::ByArity { .. }))
            .map(|entry| entry.spelling)
            .collect();
        assert_eq!(por_aridade, ROTEADAS_POR_ARIDADE.into_iter().collect());
        for spelling in por_aridade {
            assert!(
                simbolo_runtime(spelling).is_none(),
                "{spelling}: símbolo por aridade não é fixo"
            );
            assert!(roteia_por_aridade(spelling));
        }
    }

    #[test]
    fn politica_de_aridade_e_coerente_com_o_roteamento() {
        for entry in HISTORICAL {
            let Signature::Declared { arity, params, .. } = entry.signature else {
                continue;
            };
            match arity {
                ArityPolicy::Exact => {
                    assert!(
                        !matches!(entry.runtime, RuntimeRouting::ByArity { .. }),
                        "{}: aridade exata não escolhe símbolo por aridade",
                        entry.spelling
                    );
                    assert_eq!(aridades_aceitas(entry.spelling), None);
                    assert_eq!(aridade_minima(entry.spelling), None);
                }
                ArityPolicy::Subset(aridades) => {
                    assert!(
                        matches!(entry.runtime, RuntimeRouting::ByArity { .. }),
                        "{}: recorte de aridade só existe para roteamento por aridade",
                        entry.spelling
                    );
                    assert!(!aridades.is_empty(), "{}: recorte vazio", entry.spelling);
                    assert!(
                        aridades.windows(2).all(|par| par[0] < par[1]),
                        "{}: recorte fora de ordem",
                        entry.spelling
                    );
                    for argc in aridades {
                        assert!(aridade_no_recorte(entry.spelling, *argc));
                        assert!(simbolo_runtime_por_aridade(entry.spelling, *argc).is_some());
                    }
                    let fora = aridades.iter().max().expect("recorte não vazio") + 1;
                    assert!(!aridade_no_recorte(entry.spelling, fora));
                    assert_eq!(simbolo_runtime_por_aridade(entry.spelling, fora), None);
                }
                ArityPolicy::AtLeast(minimo) => {
                    assert!(
                        params.len() <= minimo,
                        "{}: o prefixo fixo não pode exceder o mínimo",
                        entry.spelling
                    );
                    assert_eq!(aridade_minima(entry.spelling), Some(minimo));
                }
            }
        }
    }

    #[test]
    fn variadica_declarada_e_apenas_formatar_verso() {
        let variadicas: BTreeSet<&str> = HISTORICAL
            .iter()
            .filter(|entry| {
                matches!(
                    entry.signature,
                    Signature::Declared {
                        arity: ArityPolicy::AtLeast(_),
                        ..
                    }
                )
            })
            .map(|entry| entry.spelling)
            .collect();
        assert_eq!(variadicas, ["formatar_verso"].into_iter().collect());
        assert_eq!(aridade_minima("formatar_verso"), Some(2));
    }

    #[test]
    fn alias_historico_compartilha_o_binding_da_grafia_adulta() {
        for (alias, adulta) in HISTORICAL_CANONICAL_ALIASES {
            let alias_entry = entrada(alias).expect("alias registrado");
            let adulta_entry = entrada(adulta).expect("grafia adulta registrada");
            assert_eq!(
                alias_entry.signature, adulta_entry.signature,
                "{alias}: alias com assinatura própria"
            );
            assert_eq!(
                alias_entry.runtime, adulta_entry.runtime,
                "{alias}: alias com símbolo próprio"
            );
            assert_eq!(
                alias_entry.semantic, adulta_entry.semantic,
                "{alias}: alias com contrato semântico próprio"
            );
        }
    }

    #[test]
    fn grafia_desconhecida_nao_e_historica() {
        assert!(!e_historica("carinho_do_usuario"));
        assert!(entrada("carinho_do_usuario").is_none());
        assert!(assinatura_ir("carinho_do_usuario").is_none());
        assert!(simbolo_runtime("carinho_do_usuario").is_none());
    }
}
