//! Classificação única e terminal das representações de payload de união (HR3).
//!
//! Antes desta camada, `union_member_layout` convertia **qualquer** erro de
//! layout de um tipo não `nulo` em `(8, 8)`. Isso deixava metadata falsa
//! atravessar semântica, IR e validadores, e a incoerência só aparecia — quando
//! aparecia — na criação nativa do descritor. Aqui a classificação é exaustiva:
//! todo membro de união é `Scalar`, `OpaqueHandle` ou `Aggregate`, com tamanho e
//! alinhamento reais, ou é rejeitado com um diagnóstico estável.
//!
//! Esta é a **única** autoridade de classificação. Semântica, lowering,
//! registry, validadores de IR/CFG/seleção/máquina, interpretador, backend e a
//! ABI do runtime consomem o resultado desta função; nenhuma delas recalcula
//! layout com regras próprias.

use crate::ast::{StructDecl, Type};
use crate::layout;
use std::collections::HashMap;

// @pinker-nav:start uniao.payload.classificacao
// @pinker-nav:domain unioes
// @pinker-nav:layer layout
// @pinker-nav:summary Classificação exaustiva das representações de payload de união em escalar, handle opaco e agregado, com layout real, resolução transparente de apelidos em profundidade, limites explícitos de tamanho e alinhamento e diagnósticos estáveis para tipos sem representação conhecida; substitui integralmente o antigo fallback (8, 8).

/// Categoria operacional do payload de um membro de união.
///
/// A categoria decide **como** o valor é materializado na injeção e na
/// extração, e é decidida uma única vez, aqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnionPayloadRepresentation {
    /// Inteiro, lógico ou leque sem carga: largura real, copiado por valor.
    Scalar,
    /// Handle de uma palavra para um descritor pertencente a outro domínio
    /// (`verso`, listas, mapas, ponteiros, callables, objetos de trato,
    /// uniões já materializadas). A cópia é rasa **por contrato**.
    OpaqueHandle,
    /// Agregado com layout estático conhecido (`ninho`, array fixo e apelidos
    /// resolvidos deles). A cópia é integral, byte a byte.
    Aggregate,
}

impl UnionPayloadRepresentation {
    /// Nome estável usado em diagnósticos, artefatos e impressões de IR.
    pub fn name(self) -> &'static str {
        match self {
            UnionPayloadRepresentation::Scalar => "escalar",
            UnionPayloadRepresentation::OpaqueHandle => "handle",
            UnionPayloadRepresentation::Aggregate => "agregado",
        }
    }
}

/// Layout terminal de um payload de união: tamanho, alinhamento e categoria.
///
/// Os três campos viajam juntos porque separá-los tornaria representável um
/// estado inconsistente (por exemplo, um agregado de 24 bytes classificado como
/// handle de uma palavra).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnionPayloadLayout {
    pub size: u64,
    pub align: u64,
    pub representation: UnionPayloadRepresentation,
}

impl UnionPayloadLayout {
    /// Confere as invariantes que todo consumidor pode assumir depois desta
    /// camada. Os validadores repetem esta defesa em vez de confiar na origem.
    pub fn is_well_formed(&self) -> bool {
        self.size > 0
            && self.size <= MAX_UNION_PAYLOAD_BYTES
            && self.align > 0
            && self.align.is_power_of_two()
            && self.align <= MAX_UNION_PAYLOAD_ALIGN
            && match self.representation {
                UnionPayloadRepresentation::OpaqueHandle => {
                    self.size == layout::POINTER_SIZE && self.align == layout::POINTER_ALIGN
                }
                UnionPayloadRepresentation::Scalar => {
                    self.size <= layout::POINTER_SIZE && self.align <= layout::POINTER_ALIGN
                }
                UnionPayloadRepresentation::Aggregate => true,
            }
    }
}

// ---------------------------------------------------------------------------
// Limites centrais
//
// O repositório já possui um limite canônico para metadata de memória pública
// (`MAX_IDENTIDADES_PUBLICAS`), mas nenhum limite aplicável a agregados de
// união. Os valores abaixo são escolhidos explicitamente e documentados em
// `docs/union_types.md`; são finitos, independentes do profile de compilação e
// revalidados no runtime nativo e no interpretador.
// ---------------------------------------------------------------------------

/// Teto por payload. Uma página é o maior agregado que a fase suporta copiar
/// integralmente para o storage imutável do descritor sem abrir política de
/// alocação nova.
pub const MAX_UNION_PAYLOAD_BYTES: u64 = 4096;

/// Teto de alinhamento. `layout_of_type` nunca produz alinhamento maior que o
/// de ponteiro nesta fase; o teto de 16 acompanha o alinhamento de pilha exigido
/// pela SysV e deixa margem sem exigir realinhamento dinâmico de frame.
pub const MAX_UNION_PAYLOAD_ALIGN: u64 = 16;

/// Teto de descritores vivos, espelhando a ordem de grandeza já adotada para
/// identidades de memória pública.
pub const MAX_UNION_DESCRIPTORS: u64 = 1_000_000;

/// Teto agregado de bytes de snapshot. Impede crescimento ilimitado mesmo com
/// descritores individualmente pequenos.
pub const MAX_UNION_TOTAL_PAYLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// Bytes de metadata por descritor contabilizados contra o orçamento: os oito
/// campos `u64` do cabeçalho do descritor nativo.
pub const UNION_DESCRIPTOR_METADATA_BYTES: u64 = 8 * 8;

/// Teto de metadata de descritores, derivado do teto de descritores.
pub const MAX_UNION_METADATA_BYTES: u64 = MAX_UNION_DESCRIPTORS * UNION_DESCRIPTOR_METADATA_BYTES;

// ---------------------------------------------------------------------------
// Domínio interno de binding de extração
//
// Extrair um payload agregado materializa uma cópia nova, distinta do snapshot
// imutável do descritor. Esse storage **não é uma identidade pública**: não vem
// de `alocar`, não é aceito por `liberar` e não reduz a cota vitalícia de
// identidades públicas. O backend nativo o realiza como slot do frame
// (`leaq -offset(%rbp)`), reaproveitado a cada passagem pelo mesmo ponto de
// extração; o interpretador não possui frame de máquina e o realiza numa arena
// interna própria, monotônica enquanto não existir contrato de desalocação para
// uniões.
//
// Os dois tetos abaixo são, por isso, **exclusivos do interpretador**: o slot do
// frame nativo já está reservado quando a extração acontece e não é cobrado de
// orçamento nenhum, então não existe cota nem diagnóstico de binding do lado
// nativo. Não confundir com os tetos de descritores, bytes de payload e
// metadata acima, esses sim aplicados nos dois back-ends. O que os dois
// compartilham é o contrato de contabilidade — extrair não consome identidade
// pública —, não a realização do storage.
// ---------------------------------------------------------------------------

/// Teto de regiões de binding de extração vivas na arena do interpretador.
///
/// Sem contraparte nativa: ver a nota do bloco acima.
pub const MAX_UNION_BINDING_REGIONS: u64 = 1_000_000;

/// Teto agregado de bytes materializados para bindings de extração na arena do
/// interpretador.
///
/// Sem contraparte nativa: ver a nota do bloco acima.
pub const MAX_UNION_BINDING_BYTES: u64 = 256 * 1024 * 1024;

/// Motivo estável pelo qual um tipo não pode ser membro de união.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionPayloadRejection {
    /// O layout estático do tipo não é conhecido (ou não existe).
    UnknownLayout(String),
    /// O tipo não possui representação de payload definida nesta fase.
    UnknownRepresentation(String),
    /// O layout é conhecido, mas o tamanho é zero ou excede o teto.
    Size(String),
    /// O alinhamento é zero, não é potência de dois ou excede o teto.
    Align(String),
}

impl UnionPayloadRejection {
    /// Código estável do diagnóstico semântico correspondente.
    pub fn code(&self) -> &'static str {
        match self {
            UnionPayloadRejection::UnknownLayout(_) => "E-SEMANTIC-UNION-PAYLOAD-LAYOUT",
            UnionPayloadRejection::UnknownRepresentation(_) => {
                "E-SEMANTIC-UNION-PAYLOAD-REPRESENTATION"
            }
            UnionPayloadRejection::Size(_) => "E-SEMANTIC-UNION-PAYLOAD-SIZE",
            UnionPayloadRejection::Align(_) => "E-SEMANTIC-UNION-PAYLOAD-ALIGN",
        }
    }

    fn detail(&self) -> &str {
        match self {
            UnionPayloadRejection::UnknownLayout(detail)
            | UnionPayloadRejection::UnknownRepresentation(detail)
            | UnionPayloadRejection::Size(detail)
            | UnionPayloadRejection::Align(detail) => detail,
        }
    }

    /// Mensagem completa, com o código estável como prefixo.
    pub fn message(&self) -> String {
        format!("{}: {}", self.code(), self.detail())
    }
}

/// Classifica o payload de um membro de união.
///
/// Apelidos são transparentes **em profundidade**: a categoria e o layout
/// correspondem sempre ao alvo resolvido. A identidade semântica do membro
/// (`ResolvedTypeId`) continua sendo decidida fora daqui e não é afetada.
pub fn classify_union_payload(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
) -> Result<UnionPayloadLayout, UnionPayloadRejection> {
    let representation = classify_representation(ty, aliases, structs, &mut Vec::new())?;
    // Handles opacos têm tamanho e alinhamento **por definição da categoria**,
    // e não por consulta de layout: `verso`, listas e mapas são ponteiros para
    // descritores de outro domínio e não possuem layout estático de conteúdo.
    // Isto não é o antigo fallback: a categoria foi decidida por enumeração
    // explícita, e um tipo fora dela nunca chega aqui como handle.
    let candidate = match representation {
        UnionPayloadRepresentation::OpaqueHandle => UnionPayloadLayout {
            size: layout::POINTER_SIZE,
            align: layout::POINTER_ALIGN,
            representation,
        },
        UnionPayloadRepresentation::Scalar | UnionPayloadRepresentation::Aggregate => {
            let layout = layout::layout_of_type(ty, aliases, structs).map_err(|msg| {
                UnionPayloadRejection::UnknownLayout(format!("{}: {msg}", ty.name()))
            })?;
            UnionPayloadLayout {
                size: layout.size,
                align: layout.align,
                representation,
            }
        }
    };
    check_limits(&candidate, ty)?;
    Ok(candidate)
}

fn check_limits(candidate: &UnionPayloadLayout, ty: &Type) -> Result<(), UnionPayloadRejection> {
    if candidate.size == 0 {
        return Err(UnionPayloadRejection::Size(format!(
            "o membro '{}' tem tamanho zero e não pode ser payload de união",
            ty.name()
        )));
    }
    if candidate.size > MAX_UNION_PAYLOAD_BYTES {
        return Err(UnionPayloadRejection::Size(format!(
            "o membro '{}' ocupa {} bytes e excede o limite de {MAX_UNION_PAYLOAD_BYTES} bytes por \
             payload de união",
            ty.name(),
            candidate.size
        )));
    }
    if candidate.align == 0 || !candidate.align.is_power_of_two() {
        return Err(UnionPayloadRejection::Align(format!(
            "o membro '{}' declara alinhamento {} que não é potência de dois",
            ty.name(),
            candidate.align
        )));
    }
    if candidate.align > MAX_UNION_PAYLOAD_ALIGN {
        return Err(UnionPayloadRejection::Align(format!(
            "o membro '{}' exige alinhamento {} acima do limite suportado de \
             {MAX_UNION_PAYLOAD_ALIGN}",
            ty.name(),
            candidate.align
        )));
    }
    if !candidate.is_well_formed() {
        return Err(UnionPayloadRejection::UnknownRepresentation(format!(
            "o membro '{}' produziu classificação '{}' incoerente com tamanho {} e alinhamento {}",
            ty.name(),
            candidate.representation.name(),
            candidate.size,
            candidate.align
        )));
    }
    Ok(())
}

/// Decide a categoria operacional, resolvendo apelidos em profundidade.
///
/// O `match` é exaustivo por variante de `Type` de propósito: acrescentar um
/// tipo novo à linguagem passa a exigir uma decisão explícita de representação
/// de payload, em vez de cair silenciosamente num ramo genérico.
fn classify_representation(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
    resolving_aliases: &mut Vec<String>,
) -> Result<UnionPayloadRepresentation, UnionPayloadRejection> {
    match ty {
        // Escalares: largura real, copiados por valor. `Enum` abaixa para o
        // discriminante `bombom` e preserva a identidade nominal fora daqui.
        Type::Bombom(_)
        | Type::U8(_)
        | Type::U16(_)
        | Type::U32(_)
        | Type::U64(_)
        | Type::I8(_)
        | Type::I16(_)
        | Type::I32(_)
        | Type::I64(_)
        | Type::Logica(_)
        | Type::Enum { .. } => Ok(UnionPayloadRepresentation::Scalar),

        // Handles opacos de uma palavra. Cada categoria está enumerada: nenhum
        // erro desconhecido é convertido em handle.
        Type::Verso(_)
        | Type::ListBombom(_)
        | Type::ListVerso(_)
        | Type::ListEnum { .. }
        | Type::MapVersoBombom(_)
        | Type::MapVersoVerso(_)
        | Type::MapBombomBombom(_)
        | Type::MapBombomVerso(_)
        | Type::Map { .. }
        | Type::Pointer { .. }
        | Type::Function { .. }
        | Type::OpaqueHandle { .. }
        | Type::Union { .. } => Ok(UnionPayloadRepresentation::OpaqueHandle),

        // Agregados com layout estático. Os componentes são classificados para
        // que um campo sem representação conhecida rejeite o agregado inteiro
        // em vez de ser absorvido pelo tamanho total.
        Type::FixedArray { element, .. } => {
            classify_representation(element, aliases, structs, resolving_aliases)?;
            Ok(UnionPayloadRepresentation::Aggregate)
        }
        Type::Struct { name, .. } => {
            if !structs.contains_key(name) {
                return Err(UnionPayloadRejection::UnknownLayout(format!(
                    "o ninho '{name}' não existe e não pode ser payload de união"
                )));
            }
            Ok(UnionPayloadRepresentation::Aggregate)
        }

        // Apelidos são transparentes em profundidade: a categoria é a do alvo.
        Type::Alias { name, span } => {
            if structs.contains_key(name) {
                return Ok(UnionPayloadRepresentation::Aggregate);
            }
            if resolving_aliases.iter().any(|entry| entry == name) {
                return Err(UnionPayloadRejection::UnknownLayout(format!(
                    "apelido de tipo recursivo detectado em '{name}'"
                )));
            }
            let target = aliases.get(name).ok_or_else(|| {
                UnionPayloadRejection::UnknownLayout(format!(
                    "o tipo '{name}' não existe e não pode ser payload de união"
                ))
            })?;
            let _ = span;
            resolving_aliases.push(name.clone());
            let representation =
                classify_representation(target, aliases, structs, resolving_aliases);
            resolving_aliases.pop();
            representation
        }

        Type::Applied { name, .. } => Err(UnionPayloadRejection::UnknownRepresentation(format!(
            "o tipo genérico aplicado '{name}' não foi monomorfizado e não possui representação \
             de payload de união"
        ))),
        Type::Nulo(_) => Err(UnionPayloadRejection::UnknownRepresentation(
            "o tipo 'nulo' não possui representação de payload de união".to_string(),
        )),
    }
}
// @pinker-nav:end uniao.payload.classificacao
