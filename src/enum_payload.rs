//! Classificação única e terminal das cargas de variante de `leque` (D1).
//!
//! Antes desta camada, três lugares diferentes decidiam **como** uma carga era
//! materializada, cada um por um `match` parcial sobre o tipo-fonte:
//!
//! ```text
//! parser  : Type::Verso(_) => "..._carga_v",  _ => "..._carga_b"
//! ir      : TypeIR::Verso  => "..._anexar_v", _ => "..._anexar_b"
//! semantic: Type::Bombom | Type::Verso | apelido-de-leque => aceito, resto => recusado
//! ```
//!
//! Os três ramos `_` eram indistinguíveis de "qualquer coisa é uma palavra
//! inteira", e por isso a única forma de a linguagem crescer sem perder
//! identidade era abandonar o `match` parcial. Este módulo é a **única**
//! autoridade: resolve o tipo declarado da carga em profundidade, classifica a
//! representação operacional e deriva daí — e só daí — o helper de runtime.
//!
//! O que a classificação **não** faz: decidir a identidade semântica. Essa
//! continua sendo [`crate::union_canon::canonical_type_key`] aplicada ao tipo
//! resolvido devolvido aqui. As duas dimensões viajam juntas em
//! [`EnumPayloadShape`] justamente porque `lista<bombom>`, `lista<Cor>` e
//! `lista<Token>` compartilham a mesma classe operacional e **não** são
//! intercambiáveis.

use crate::ast::Type;
use crate::union_canon;
use std::collections::{HashMap, HashSet};

// @pinker-nav:start leque.carga.classificacao
// @pinker-nav:domain leques
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única das cargas de variante de leque: `resolve_payload_type` resolve apelidos em profundidade (inclusive o elemento de `lista<E>` e cadeias de apelidos) sem criar identidade nominal nova, `classify_enum_payload` decide a classe operacional exaustivamente por variante de `Type` (discriminante imediato, `verso`, handle opaco de uma palavra ou recusa estável), e `EnumPayloadShape` transporta representação operacional, tipo resolvido, chave canônica de identidade e identidade do elemento da lista até parser, semântica, lowering, validadores, interpretador e backend nativo, que derivam o helper de runtime exclusivamente da classe — nunca de um `match` parcial local sobre o tipo-fonte.

/// Categoria operacional de uma carga de variante.
///
/// O `leque` continua sendo abaixado para o discriminante imediato (ou para o
/// handle do próprio leque, conforme o contrato já existente): as duas formas
/// cabem numa palavra inteira e compartilham helper. `verso` mantém o caminho
/// próprio de texto. Toda `lista<E>` é um handle opaco de uma palavra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnumPayloadClass {
    /// Inteiro ou discriminante imediato: `bombom` e `leque` concreto.
    ImmediateDiscriminant,
    /// `verso`: handle textual, com helper próprio desde a Fase 218.
    Verso,
    /// Handle opaco de uma palavra. A classe descreve somente a representação;
    /// `EnumPayloadShape::resolved` preserva qual família semântica ocupa essa
    /// palavra (lista ou handle builtin nominal).
    OpaqueWordHandle,
}

impl EnumPayloadClass {
    /// Nome estável usado em diagnósticos, artefatos e impressões de IR.
    pub fn name(self) -> &'static str {
        match self {
            EnumPayloadClass::ImmediateDiscriminant => "imediato",
            EnumPayloadClass::Verso => "verso",
            EnumPayloadClass::OpaqueWordHandle => "handle",
        }
    }
}

/// Descrição de carga de primeira classe.
///
/// Substitui o antigo par "`Vec<Type>` no parser, `Vec<TypeIR>` na IR": os
/// campos viajam juntos porque separá-los volta a tornar representável o estado
/// em que uma carga tem representação de uma palavra e nenhuma identidade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumPayloadShape {
    /// Classe operacional — decide o helper, nunca a validade do tipo.
    pub class: EnumPayloadClass,
    /// Tipo AST resolvido integralmente (apelidos transparentes, elemento de
    /// lista já nominalizado). É a fonte da identidade semântica.
    pub resolved: Type,
}

impl EnumPayloadShape {
    /// Identidade semântica canônica da carga, pelo contrato compartilhado.
    ///
    /// `lista<bombom>`, `lista<Cor>` e `lista<Token>` produzem três chaves
    /// distintas; `apelido N = lista<bombom>` produz a mesma chave de
    /// `lista<bombom>`.
    pub fn canonical_key(&self) -> String {
        union_canon::canonical_type_key(&self.resolved)
    }

    /// Identidade concreta do elemento, quando a carga é uma lista.
    ///
    /// `lista<Cor>` devolve o tipo nominal `Cor`; `lista<bombom>` devolve
    /// `bombom`; uma carga que não é lista devolve `None`.
    pub fn element_type(&self) -> Option<Type> {
        match &self.resolved {
            Type::ListBombom(span) => Some(Type::Bombom(*span)),
            Type::ListVerso(span) => Some(Type::Verso(*span)),
            Type::ListEnum { element, span } => Some(Type::Enum {
                name: element.clone(),
                span: *span,
            }),
            _ => None,
        }
    }

    /// Nome da intrínseca interna que **anexa** esta carga na construção.
    ///
    /// A escolha deriva da classe; dentro da classe de handle, o sufixo reflete
    /// a representação operacional exigida pela assinatura tipada da intrínseca
    /// (`lista<bombom>` e `lista<Leque>` compartilham `ListBombom`). Todos os
    /// quatro nomes colapsam no mesmo símbolo de runtime nativo.
    pub fn anexar_intrinsic(&self) -> &'static str {
        match self.class {
            EnumPayloadClass::ImmediateDiscriminant => ANEXAR_IMEDIATO,
            EnumPayloadClass::Verso => ANEXAR_VERSO,
            EnumPayloadClass::OpaqueWordHandle => match &self.resolved {
                Type::ListVerso(_) => ANEXAR_LISTA_VERSO,
                Type::ListBombom(_) | Type::ListEnum { .. } => ANEXAR_LISTA_BOMBOM,
                // Handles opacos nominais que não são listas compartilham o
                // par de intrínsecas internas. O nome histórico da constante
                // vem da Parte D; a função é geral, e a Parte E1 é a segunda
                // família a usá-la — sem helper novo por família.
                Type::OpaqueHandle { name, .. }
                    if name == crate::saida_processo::TIPO_SAIDA_PROCESSO
                        || name == crate::valor_json::TIPO_VALOR_JSON =>
                {
                    ANEXAR_SAIDA_PROCESSO
                }
                _ => unreachable!("classe de handle sem família semântica válida"),
            },
        }
    }

    /// Nome da intrínseca interna que **extrai** esta carga no `encaixe`.
    pub fn carga_intrinsic(&self) -> &'static str {
        match self.class {
            EnumPayloadClass::ImmediateDiscriminant => CARGA_IMEDIATO,
            EnumPayloadClass::Verso => CARGA_VERSO,
            EnumPayloadClass::OpaqueWordHandle => match &self.resolved {
                Type::ListVerso(_) => CARGA_LISTA_VERSO,
                Type::ListBombom(_) | Type::ListEnum { .. } => CARGA_LISTA_BOMBOM,
                // Handles opacos nominais que não são listas compartilham o
                // par de intrínsecas internas. O nome histórico da constante
                // vem da Parte D; a função é geral, e a Parte E1 é a segunda
                // família a usá-la — sem helper novo por família.
                Type::OpaqueHandle { name, .. }
                    if name == crate::saida_processo::TIPO_SAIDA_PROCESSO
                        || name == crate::valor_json::TIPO_VALOR_JSON =>
                {
                    CARGA_SAIDA_PROCESSO
                }
                _ => unreachable!("classe de handle sem família semântica válida"),
            },
        }
    }
}

/// Intrínseca de anexo para cargas imediatas (`bombom`, leque).
pub const ANEXAR_IMEDIATO: &str = "__pinker_internal_leque_anexar_b";
/// Intrínseca de anexo para cargas `verso`.
pub const ANEXAR_VERSO: &str = "__pinker_internal_leque_anexar_v";
/// Intrínseca de anexo para handles de `lista<bombom>` e `lista<Leque>`.
pub const ANEXAR_LISTA_BOMBOM: &str = "__pinker_internal_leque_anexar_lista_b";
/// Intrínseca de anexo para handles de `lista<verso>`.
pub const ANEXAR_LISTA_VERSO: &str = "__pinker_internal_leque_anexar_lista_v";
/// Intrínseca de anexo para handles opacos nominais que não são listas.
pub const ANEXAR_SAIDA_PROCESSO: &str = "__pinker_internal_leque_anexar_saida_processo";

/// Intrínseca de extração para cargas imediatas (`bombom`, leque).
pub const CARGA_IMEDIATO: &str = "__pinker_internal_leque_carga_b";
/// Intrínseca de extração para cargas `verso`.
pub const CARGA_VERSO: &str = "__pinker_internal_leque_carga_v";
/// Intrínseca de extração para handles de `lista<bombom>` e `lista<Leque>`.
pub const CARGA_LISTA_BOMBOM: &str = "__pinker_internal_leque_carga_lista_b";
/// Intrínseca de extração para handles de `lista<verso>`.
pub const CARGA_LISTA_VERSO: &str = "__pinker_internal_leque_carga_lista_v";
/// Intrínseca de extração para handles opacos nominais que não são listas.
pub const CARGA_SAIDA_PROCESSO: &str = "__pinker_internal_leque_carga_saida_processo";

/// Todas as intrínsecas de extração de carga, em ordem estável.
pub const CARGA_INTRINSICS: [&str; 5] = [
    CARGA_IMEDIATO,
    CARGA_VERSO,
    CARGA_LISTA_BOMBOM,
    CARGA_LISTA_VERSO,
    CARGA_SAIDA_PROCESSO,
];

/// Todas as intrínsecas de anexo de carga, em ordem estável.
pub const ANEXAR_INTRINSICS: [&str; 5] = [
    ANEXAR_IMEDIATO,
    ANEXAR_VERSO,
    ANEXAR_LISTA_BOMBOM,
    ANEXAR_LISTA_VERSO,
    ANEXAR_SAIDA_PROCESSO,
];

/// Verdadeiro para qualquer intrínseca interna de extração de carga.
pub fn is_carga_intrinsic(name: &str) -> bool {
    CARGA_INTRINSICS.contains(&name)
}

/// Verdadeiro para qualquer intrínseca interna de anexo de carga.
pub fn is_anexar_intrinsic(name: &str) -> bool {
    ANEXAR_INTRINSICS.contains(&name)
}

/// Motivo estável pelo qual um tipo não pode ser carga de variante nesta fase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumPayloadRejection {
    /// O nome não existe, ou o apelido é recursivo, ou o genérico não foi
    /// monomorfizado: a identidade não pôde ser resolvida.
    Unresolved(String),
    /// O tipo resolveu, mas não pertence ao contrato de cargas desta fase.
    Unsupported(String),
}

impl EnumPayloadRejection {
    /// Código estável do diagnóstico semântico correspondente.
    pub fn code(&self) -> &'static str {
        match self {
            EnumPayloadRejection::Unresolved(_) => "E-SEMANTIC-ENUM-PAYLOAD-UNRESOLVED",
            EnumPayloadRejection::Unsupported(_) => "E-SEMANTIC-ENUM-PAYLOAD-UNSUPPORTED",
        }
    }

    fn detail(&self) -> &str {
        match self {
            EnumPayloadRejection::Unresolved(detail)
            | EnumPayloadRejection::Unsupported(detail) => detail,
        }
    }

    /// Mensagem completa, com o código estável como prefixo.
    pub fn message(&self) -> String {
        format!("{}: {}", self.code(), self.detail())
    }
}

/// Descrição textual, estável e fiel do contrato aceito nesta fase.
///
/// Substitui a antiga enumeração `'bombom', 'verso' ou um leque declarado`, que
/// deixou de ser fiel quando as listas entraram no contrato.
pub const CONTRATO_CARGAS: &str =
    "'bombom', 'verso', um leque declarado, 'lista<bombom>', 'lista<verso>', \
     'lista<Leque>', o handle builtin nominal 'SaidaProcesso' ou um apelido \
     transparente de qualquer uma dessas formas";

/// Resolve integralmente o tipo declarado de uma carga.
///
/// Apelidos são transparentes **em profundidade** e nunca criam identidade
/// nominal nova: `apelido CorAlias = Cor; apelido ListaCor = lista<CorAlias>;
/// apelido ListaCor2 = ListaCor;` converge para `lista<Cor>`.
pub fn resolve_payload_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
) -> Result<Type, EnumPayloadRejection> {
    resolve_named(ty, aliases, enums, structs, &mut Vec::new())
}

fn resolve_named(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    resolving: &mut Vec<String>,
) -> Result<Type, EnumPayloadRejection> {
    match ty {
        Type::Alias { name, span } => {
            if enums.contains(name) {
                return Ok(Type::Enum {
                    name: name.clone(),
                    span: *span,
                });
            }
            if structs.contains(name) {
                return Ok(Type::Struct {
                    name: name.clone(),
                    span: *span,
                });
            }
            if resolving.iter().any(|entry| entry == name) {
                return Err(EnumPayloadRejection::Unresolved(format!(
                    "apelido de tipo recursivo detectado em '{name}'"
                )));
            }
            let Some(target) = aliases.get(name) else {
                return Err(EnumPayloadRejection::Unresolved(format!(
                    "o tipo '{name}' não existe"
                )));
            };
            resolving.push(name.clone());
            let resolved = resolve_named(target, aliases, enums, structs, resolving);
            resolving.pop();
            Ok(resolved?.with_span(*span))
        }
        // O elemento de `lista<E>` é resolvido pelo mesmo caminho: `lista<Alias>`
        // não é uma lista de um tipo nominal chamado `Alias`, é a lista do alvo.
        Type::ListEnum { element, span } => {
            let element_ty = resolve_named(
                &Type::Alias {
                    name: element.clone(),
                    span: *span,
                },
                aliases,
                enums,
                structs,
                resolving,
            )?;
            match element_ty {
                Type::Bombom(_) => Ok(Type::ListBombom(*span)),
                Type::Verso(_) => Ok(Type::ListVerso(*span)),
                Type::Enum { name, .. } => Ok(Type::ListEnum {
                    element: name,
                    span: *span,
                }),
                other => Err(EnumPayloadRejection::Unsupported(format!(
                    "'lista<{}>' não é um tipo de lista operacional desta fase",
                    other.name()
                ))),
            }
        }
        other => Ok(other.clone()),
    }
}

/// Classifica a carga de uma variante de `leque`.
///
/// Devolve a descrição de primeira classe — classe operacional **e** tipo
/// resolvido — ou um motivo estável de recusa. O `match` é exaustivo por
/// variante de `Type` de propósito: acrescentar um tipo novo à linguagem passa
/// a exigir uma decisão explícita de carga, em vez de cair num ramo genérico.
pub fn classify_enum_payload(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
) -> Result<EnumPayloadShape, EnumPayloadRejection> {
    let resolved = resolve_payload_type(ty, aliases, enums, structs)?;
    let class = match &resolved {
        // Discriminante imediato: cabe numa palavra e é copiado por valor.
        Type::Bombom(_) => EnumPayloadClass::ImmediateDiscriminant,
        Type::Enum { name, .. } => {
            if !enums.contains(name) {
                return Err(EnumPayloadRejection::Unresolved(format!(
                    "o leque '{name}' não foi declarado"
                )));
            }
            EnumPayloadClass::ImmediateDiscriminant
        }

        // `verso` mantém o caminho próprio de texto.
        Type::Verso(_) => EnumPayloadClass::Verso,

        // Todas as listas são handles opacos de uma palavra. `lista<verso>`
        // **não** é um `verso` e `lista<Leque>` **não** perde o elemento: as
        // três formas compartilham a classe e diferem na identidade.
        Type::ListBombom(_) | Type::ListVerso(_) | Type::OpaqueHandle { .. } => {
            EnumPayloadClass::OpaqueWordHandle
        }
        Type::ListEnum { element, .. } => {
            if !enums.contains(element) {
                return Err(EnumPayloadRejection::Unresolved(format!(
                    "o leque '{element}' usado como elemento de lista não foi declarado"
                )));
            }
            EnumPayloadClass::OpaqueWordHandle
        }

        // Fora do contrato desta fase, com o motivo nomeado em cada caso.
        Type::Applied { name, .. } => {
            return Err(EnumPayloadRejection::Unresolved(format!(
                "o tipo genérico aplicado '{name}' não foi monomorfizado"
            )))
        }
        Type::Alias { name, .. } => {
            return Err(EnumPayloadRejection::Unresolved(format!(
                "o tipo '{name}' não existe"
            )))
        }
        other @ (Type::U8(_)
        | Type::U16(_)
        | Type::U32(_)
        | Type::U64(_)
        | Type::I8(_)
        | Type::I16(_)
        | Type::I32(_)
        | Type::I64(_)
        | Type::Logica(_)
        | Type::MapVersoBombom(_)
        | Type::MapVersoVerso(_)
        | Type::MapBombomBombom(_)
        | Type::MapBombomVerso(_)
        | Type::Map { .. }
        | Type::FixedArray { .. }
        | Type::Pointer { .. }
        | Type::Function { .. }
        | Type::Struct { .. }
        | Type::Union { .. }
        | Type::Nulo(_)) => {
            return Err(EnumPayloadRejection::Unsupported(format!(
                "o tipo '{}' não é aceito como carga de variante nesta fase",
                other.name()
            )))
        }
    };
    Ok(EnumPayloadShape { class, resolved })
}
// @pinker-nav:end leque.carga.classificacao
