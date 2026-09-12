//! U-02 / TC-02 — a especialização de operação de mapa tem UMA autoridade.
//!
//! A relação sob prova é finita e fechada:
//!
//! ```text
//! WHICH_MONOMORPHIC_EXECUTABLE_IDENTITY
//! =
//! SPECIALIZE(CONCRETE_MAP_CLASS, GENERIC_MAP_OPERATION)
//!
//! MAP_CLASSES          = 4
//! GENERIC_OPERATIONS   = 6   (criar definir obter tem tamanho remover)
//! SPECIALIZATION_CELLS = 24
//! ```
//!
//! No baseline ela tinha CINCO realizações independentes: as três tabelas
//! `(classe, operação) -> grafia` de `parser`, `semantic` e `ir`, as quatro
//! seleções de `tamanho` do desugaring de `para cada` e as quatro seleções de
//! `criar` do lowering. Esta suíte prova que sobrou uma.
//!
//! # O oráculo não é cópia da tabela
//!
//! Validar uma tabela com uma cópia manual da mesma tabela não é independência.
//! Os oráculos daqui são relações que já existiam e que ninguém escreveu para
//! esta prova:
//!
//! 1. **C1 / registry, por inversão estrutural.** Cada grafia monomórfica de
//!    mapa já declara em [`registry::HISTORICAL`] retorno e parâmetros. Disso
//!    sai a classe (o tipo do operando 0, ou do resultado em `criar`) e sai a
//!    operação (aridade mais forma do resultado). A inversão nunca lê o nome:
//!    ela reconstrói o par `(classe, operação)` do contrato e só então compara
//!    com a autoridade.
//! 2. **O corpo hospedado do interpretador.** Ele agrupa, por operação, as
//!    quatro grafias monomórficas que executam o mesmo corpo. Esse agrupamento é
//!    a relação lida ao contrário, escrita por outro dono e por outro motivo.
//! 3. **A projeção observável de cada fase.** Parser, semântica e IR são
//!    observados pelo que produzem — AST, aceitação/tipo, IR renderizada —, não
//!    pelo que a autoridade afirma.
//!
//! # Onde esta suíte para, e quem continua
//!
//! Estes oráculos fecham a reintrodução **divergente**: uma fase que escolhe a
//! célula errada muda o que produz, e a projeção a recusa. Eles NÃO fecham a
//! reintrodução **concordante** — a cópia local que responde exatamente o valor
//! canônico —, porque o comportamento dela é idêntico e nenhuma projeção a
//! distingue. O único detector dela aqui é o censo textual, e censo textual não
//! fecha classe nenhuma: basta o literal do alvo deixar de aparecer inteiro no
//! texto, como `concat!("mapa_", "verso_bombom_", "tamanho")`.
//!
//! ```text
//! TEXTUAL_CENSUS        = SUPPLEMENTAL_ONLY
//! OBSERVABLE_PROJECTION = DIVERGENCE_DETECTOR
//! METAMORPHIC_EXECUTION = TERMINAL_GUARANTEE
//! ```
//!
//! A garantia terminal é por EXECUÇÃO e vive em
//! `src/map_specialization/metamorphic_oracle.rs`: mutar a célula na autoridade
//! e exigir que a seleção de cada consumidor real acompanhe. Quem tem cópia
//! local mantém a resposta antiga e fica vermelho lá, não aqui.
//!
//! # O que esta suíte NÃO reivindica
//!
//! ```text
//! MAP_SPECIALIZATION_RELATION != PUBLIC_INTRINSIC_REGISTRY
//! MAP_SPECIALIZATION_RELATION != INTERNAL_OPERATION_CONTRACT
//! MAP_SPECIALIZATION_RELATION != RUNTIME_OR_INTERPRETER_BODY
//! MAP_SPECIALIZATION_RELATION != ABI_SYMBOL
//! CONCORDANT_DUPLICATE_ABSENCE != PROVED_BY_THIS_SUITE
//! ```

mod common;

use pinker_v0::internal_operations;
use pinker_v0::intrinsics::registry::{self, Signature};
use pinker_v0::ir::TypeIR;
use pinker_v0::map_specialization::{
    specialize, specialize_generic_spelling, specialize_spelling, CanonicalMapClass,
    GenericMapOperation, CANONICAL_MAP_CLASSES, GENERIC_MAP_OPERATIONS,
};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Modelo do domínio, independente da autoridade
// ---------------------------------------------------------------------------

/// O que `mapa<K,V>` significa para cada classe, na representação da IR.
///
/// São quatro linhas de fato de linguagem — o que a classe É —, não a relação de
/// 24 células sob prova.
fn chave_e_valor(class: CanonicalMapClass) -> (TypeIR, TypeIR) {
    match class {
        CanonicalMapClass::VersoBombom => (TypeIR::Verso, TypeIR::Bombom),
        CanonicalMapClass::VersoVerso => (TypeIR::Verso, TypeIR::Verso),
        CanonicalMapClass::BombomBombom => (TypeIR::Bombom, TypeIR::Bombom),
        CanonicalMapClass::BombomVerso => (TypeIR::Bombom, TypeIR::Verso),
    }
}

/// A categoria operacional da própria classe.
fn type_ir_da_classe(class: CanonicalMapClass) -> TypeIR {
    match class {
        CanonicalMapClass::VersoBombom => TypeIR::MapVersoBombom,
        CanonicalMapClass::VersoVerso => TypeIR::MapVersoVerso,
        CanonicalMapClass::BombomBombom => TypeIR::MapBombomBombom,
        CanonicalMapClass::BombomVerso => TypeIR::MapBombomVerso,
    }
}

fn classe_de_type_ir(ty: TypeIR) -> Option<CanonicalMapClass> {
    CANONICAL_MAP_CLASSES
        .iter()
        .copied()
        .find(|&class| type_ir_da_classe(class) == ty)
}

/// A operação que um contrato declarado descreve, deduzida da FORMA.
///
/// Nenhum nome é consultado: só aridade, tipo do resultado e os tipos que a
/// classe define. É isso que torna a inversão um oráculo e não um espelho.
fn operacao_do_contrato(
    class: CanonicalMapClass,
    params: &[TypeIR],
    ret: TypeIR,
) -> Option<GenericMapOperation> {
    let (chave, valor) = chave_e_valor(class);
    match (params.len(), ret) {
        (0, r) if r == type_ir_da_classe(class) => Some(GenericMapOperation::Criar),
        (1, TypeIR::Bombom) => Some(GenericMapOperation::Tamanho),
        (2, TypeIR::Logica) if params[1] == chave => Some(GenericMapOperation::Tem),
        (2, TypeIR::Nulo) if params[1] == chave => Some(GenericMapOperation::Remover),
        (2, r) if r == valor && params[1] == chave => Some(GenericMapOperation::Obter),
        (3, TypeIR::Nulo) if params[1] == chave && params[2] == valor => {
            Some(GenericMapOperation::Definir)
        }
        _ => None,
    }
}

/// A relação reconstruída de C1, sem olhar a autoridade nem os nomes.
///
/// Uma entrada de `registry` entra aqui quando o contrato dela É de operação
/// monomórfica de mapa: ou o operando 0 é uma classe concreta de mapa, ou o
/// resultado é uma classe concreta e não há operando.
fn relacao_invertida_de_c1() -> BTreeMap<(CanonicalMapClass, GenericMapOperation), &'static str> {
    let mut relacao = BTreeMap::new();
    for entry in registry::HISTORICAL {
        let Signature::Declared { ret, params, .. } = entry.signature else {
            continue;
        };
        let class = match params.first() {
            Some(&primeiro) => classe_de_type_ir(primeiro),
            None => classe_de_type_ir(ret),
        };
        let Some(class) = class else {
            continue;
        };
        let Some(operation) = operacao_do_contrato(class, params, ret) else {
            continue;
        };
        assert!(
            relacao.insert((class, operation), entry.spelling).is_none(),
            "C1 declara dois contratos para {class:?}/{operation:?}"
        );
    }
    relacao
}

// ---------------------------------------------------------------------------
// Leitura de fonte que PRESERVA literais
// ---------------------------------------------------------------------------

/// Remove apenas comentários, preservando literais de texto.
///
/// `rust_source::codigo_executavel` remove também os literais, e é a leitura
/// certa para "esta camada ainda constrói X?". Aqui a pergunta é a oposta —
/// "esta camada ainda NOMEIA a grafia do alvo?" — e a grafia vive exatamente
/// dentro de um literal. Ler o código executável tornaria o censo vazio: a
/// evidência teria sido apagada antes de ser observada.
///
/// Limite declarado: identificador produzido por macro não aparece no texto.
fn codigo_com_literais(fonte: &str) -> String {
    let bytes: Vec<char> = fonte.chars().collect();
    let mut saida = String::with_capacity(fonte.len());
    let mut i = 0usize;
    let mut bloco = 0usize;
    while i < bytes.len() {
        let dois: String = bytes[i..(i + 2).min(bytes.len())].iter().collect();
        if bloco > 0 {
            if dois == "/*" {
                bloco += 1;
                i += 2;
            } else if dois == "*/" {
                bloco -= 1;
                i += 2;
            } else {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        if dois == "/*" {
            bloco = 1;
            i += 2;
            saida.push(' ');
            continue;
        }
        if dois == "//" {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        // Literal cru `r"..."` / `r#"..."#`: preservado inteiro, porque `//`
        // dentro dele não é comentário.
        if bytes[i] == 'r' {
            let mut cerquilhas = 0usize;
            while i + 1 + cerquilhas < bytes.len() && bytes[i + 1 + cerquilhas] == '#' {
                cerquilhas += 1;
            }
            if bytes.get(i + 1 + cerquilhas) == Some(&'"') {
                let fecho: String = std::iter::once('"')
                    .chain(std::iter::repeat('#').take(cerquilhas))
                    .collect();
                let inicio = i;
                i += 2 + cerquilhas;
                while i < bytes.len() {
                    let janela: String = bytes[i..(i + fecho.len()).min(bytes.len())]
                        .iter()
                        .collect();
                    if janela == fecho {
                        i += fecho.len();
                        break;
                    }
                    i += 1;
                }
                saida.extend(&bytes[inicio..i.min(bytes.len())]);
                continue;
            }
        }
        if bytes[i] == '"' {
            let inicio = i;
            i += 1;
            while i < bytes.len() && bytes[i] != '"' {
                i += if bytes[i] == '\\' { 2 } else { 1 };
            }
            i = (i + 1).min(bytes.len());
            saida.extend(&bytes[inicio..i]);
            continue;
        }
        saida.push(bytes[i]);
        i += 1;
    }
    saida
}

// ---------------------------------------------------------------------------
// 1. Domínio: exaustivo, finito e classificado
// ---------------------------------------------------------------------------

#[test]
fn o_dominio_tem_quatro_classes_seis_operacoes_e_vinte_e_quatro_celulas() {
    assert_eq!(CANONICAL_MAP_CLASSES.len(), 4);
    assert_eq!(GENERIC_MAP_OPERATIONS.len(), 6);

    let classes: BTreeSet<_> = CANONICAL_MAP_CLASSES.iter().copied().collect();
    assert_eq!(classes.len(), 4, "classe repetida na enumeração estável");
    let operacoes: BTreeSet<_> = GENERIC_MAP_OPERATIONS.iter().copied().collect();
    assert_eq!(
        operacoes.len(),
        6,
        "operação repetida na enumeração estável"
    );

    // Toda célula é DEFINED: nenhuma combinação do domínio fica sem identidade,
    // e nenhuma identidade é reutilizada por duas células.
    let mut alvos = BTreeSet::new();
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            let alvo = specialize_spelling(class, operation);
            assert!(
                alvos.insert(alvo),
                "{class:?}/{operation:?} reaproveita o alvo '{alvo}' de outra célula"
            );
        }
    }
    assert_eq!(alvos.len(), 24, "o domínio deixou de ter 24 células");
}

// ---------------------------------------------------------------------------
// 2. Oráculo 1: a relação é a de C1, invertida estruturalmente
// ---------------------------------------------------------------------------

#[test]
fn a_relacao_canonica_e_exatamente_a_inversao_estrutural_de_c1() {
    let de_c1 = relacao_invertida_de_c1();
    assert_eq!(
        de_c1.len(),
        24,
        "C1 deixou de declarar 24 contratos monomórficos de mapa"
    );

    for (&(class, operation), &esperado) in &de_c1 {
        assert_eq!(
            specialize_spelling(class, operation),
            esperado,
            "a autoridade discorda de C1 em {class:?}/{operation:?}"
        );
    }

    // E no sentido inverso: a autoridade não inventa célula que C1 não declara.
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            assert!(
                de_c1.contains_key(&(class, operation)),
                "a autoridade declara {class:?}/{operation:?}, que C1 não declara"
            );
        }
    }
}

#[test]
fn todo_alvo_pertence_a_superficie_historica_e_nenhum_e_operacao_interna() {
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            let alvo = specialize_spelling(class, operation);
            assert!(
                registry::e_historica(alvo),
                "{class:?}/{operation:?} aponta para '{alvo}', fora do registry histórico"
            );
            assert!(
                !internal_operations::e_operacao_interna(alvo),
                "U-01 absorvida: {class:?}/{operation:?} resolve para a operação interna '{alvo}'"
            );
            // A identidade estruturada e a grafia nunca se separam.
            assert_eq!(
                specialize(class, operation).canonical_public_spelling(),
                alvo
            );
        }
    }

    // E a fronteira no outro sentido: nenhuma operação interna virou grafia
    // pública por causa desta consolidação.
    for operation in internal_operations::INTERNAL_OPERATIONS {
        assert!(
            !registry::e_historica(operation.spelling),
            "grafia interna '{}' entrou na superfície histórica",
            operation.spelling
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Oráculo 2: o agrupamento dos corpos hospedados concorda com a relação
// ---------------------------------------------------------------------------

/// Conteúdo balanceado de cada `matches!(...)` da fonte.
fn grupos_matches(fonte: &str) -> Vec<String> {
    let mut grupos = Vec::new();
    let marcador = "matches!(";
    let bytes: Vec<char> = fonte.chars().collect();
    let mut inicio = 0usize;
    while let Some(offset) = fonte[inicio..].find(marcador) {
        let abre = inicio + offset + marcador.len() - 1;
        let mut indice = fonte[..abre].chars().count();
        let mut profundidade = 0usize;
        let comeco = indice;
        while indice < bytes.len() {
            match bytes[indice] {
                '(' => profundidade += 1,
                ')' => {
                    profundidade -= 1;
                    if profundidade == 0 {
                        break;
                    }
                }
                _ => {}
            }
            indice += 1;
        }
        grupos.push(bytes[comeco..indice.min(bytes.len())].iter().collect());
        inicio = inicio + offset + marcador.len();
    }
    grupos
}

fn grafias_monomorficas_de_mapa(trecho: &str) -> BTreeSet<&'static str> {
    let mut encontradas = BTreeSet::new();
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            let alvo = specialize_spelling(class, operation);
            // `_criar` não é substring de outra grafia, e as demais terminam a
            // grafia: comparar por aspas evita casar prefixo.
            if trecho.contains(&format!("\"{alvo}\"")) {
                encontradas.insert(alvo);
            }
        }
    }
    encontradas
}

#[test]
fn o_agrupamento_dos_corpos_hospedados_concorda_com_a_relacao() {
    let interpretador = common::fonte_de_modulo::interpreter();
    let despacho = codigo_com_literais(&interpretador);

    // As cinco operações que o interpretador hospeda com um corpo por operação.
    // `criar` fica fora: o corpo dela não é despachado por grafia, e sim pelo
    // wrapper de handle que a própria fase escolhe.
    let hospedadas = [
        (
            GenericMapOperation::Definir,
            "__pinker_internal_mapa_definir",
        ),
        (GenericMapOperation::Obter, "__pinker_internal_mapa_obter"),
        (GenericMapOperation::Tem, "__pinker_internal_mapa_tem"),
        (
            GenericMapOperation::Tamanho,
            "__pinker_internal_mapa_tamanho",
        ),
        (
            GenericMapOperation::Remover,
            "__pinker_internal_mapa_remover",
        ),
    ];

    let grupos = grupos_matches(&despacho);
    for (operation, interna) in hospedadas {
        let esperado: BTreeSet<&'static str> = CANONICAL_MAP_CLASSES
            .iter()
            .map(|&class| specialize_spelling(class, operation))
            .collect();
        let grupo = grupos
            .iter()
            .find(|grupo| grupo.contains(&format!("\"{interna}\"")))
            .unwrap_or_else(|| panic!("corpo hospedado de '{interna}' desapareceu"));
        assert_eq!(
            grafias_monomorficas_de_mapa(grupo),
            esperado,
            "o corpo hospedado de '{interna}' deixou de agrupar exatamente a coluna \
             {operation:?} da relação"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Nenhuma fase decide a relação localmente
// ---------------------------------------------------------------------------

/// As fases que consomem a relação, cada uma pelo módulo inteiro.
fn fases_consumidoras() -> Vec<(&'static str, String)> {
    vec![
        ("parser", common::fonte_de_modulo::parser()),
        ("semantic", common::fonte_de_modulo::semantic()),
        ("ir", common::fonte_de_modulo::ir()),
    ]
}

#[test]
fn nenhuma_fase_nomeia_uma_identidade_monomorfica_de_mapa() {
    // Guard SUPLEMENTAR. Ele recusa a reintrodução escrita de forma direta, que
    // é a que aparece em revisão e em merge acidental, e é barato. O que ele NÃO
    // faz é fechar a classe: um literal montado por fragmento passa por ele
    // intacto. Quem fecha é a prova por execução em
    // `src/map_specialization/metamorphic_oracle.rs`; se este censo for lido
    // como fechamento, a promessa fica maior que a medida.
    for (nome, fonte) in fases_consumidoras() {
        let executavel = codigo_com_literais(&fonte);
        for &class in CANONICAL_MAP_CLASSES {
            for &operation in GENERIC_MAP_OPERATIONS {
                let alvo = specialize_spelling(class, operation);
                assert!(
                    !executavel.contains(alvo),
                    "{nome} voltou a nomear '{alvo}': a relação tem decisor local outra vez"
                );
            }
        }
    }
}

#[test]
fn as_fases_consultam_a_autoridade_e_so_traduzem_representacao() {
    for (nome, fonte) in fases_consumidoras() {
        let executavel = common::rust_source::codigo_executavel(&fonte);
        assert!(
            executavel.contains("map_specialization"),
            "{nome} deixou de consultar a autoridade de especialização"
        );
        // O adapter de representação é legítimo e permanece local: ele responde
        // QUAL classe é, nunca qual identidade a operação endereça.
        assert!(
            executavel.contains("canonical_map_class"),
            "{nome} perdeu o adapter de representação para a classe canônica"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Projeção observável de cada fase
// ---------------------------------------------------------------------------

struct ClasseFonte {
    class: CanonicalMapClass,
    tipo: &'static str,
    chave: &'static str,
    valor: &'static str,
    tipo_valor: &'static str,
    tipo_valor_errado: &'static str,
}

const FONTES: &[ClasseFonte] = &[
    ClasseFonte {
        class: CanonicalMapClass::VersoBombom,
        tipo: "mapa<verso,bombom>",
        chave: "\"a\"",
        valor: "1",
        tipo_valor: "bombom",
        tipo_valor_errado: "verso",
    },
    ClasseFonte {
        class: CanonicalMapClass::VersoVerso,
        tipo: "mapa<verso,verso>",
        chave: "\"a\"",
        valor: "\"x\"",
        tipo_valor: "verso",
        tipo_valor_errado: "bombom",
    },
    ClasseFonte {
        class: CanonicalMapClass::BombomBombom,
        tipo: "mapa<bombom,bombom>",
        chave: "7",
        valor: "1",
        tipo_valor: "bombom",
        tipo_valor_errado: "verso",
    },
    ClasseFonte {
        class: CanonicalMapClass::BombomVerso,
        tipo: "mapa<bombom,verso>",
        chave: "7",
        valor: "\"x\"",
        tipo_valor: "verso",
        tipo_valor_errado: "bombom",
    },
];

/// Forma direta: o mapa é um `Ident` de tipo declarado, e o parser especializa.
fn fonte_direta(caso: &ClasseFonte) -> String {
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   mapa.definir(m, {chave}, {valor});\n\
         \x20   talvez !mapa.tem(m, {chave}) {{ mimo 1; }}\n\
         \x20   nova v: {tipo_valor} = mapa.obter(m, {chave});\n\
         \x20   mapa.definir(m, {chave}, v);\n\
         \x20   mapa.remover(m, {chave});\n\
         \x20   mimo mapa.tamanho(m);\n\
         }}\n",
        tipo = caso.tipo,
        chave = caso.chave,
        valor = caso.valor,
        tipo_valor = caso.tipo_valor,
    )
}

/// Forma indireta: o mapa vem de uma chamada, o parser não pode especializar, e
/// a decisão cai para a semântica e para a IR.
fn fonte_indireta(caso: &ClasseFonte, tipo_do_valor_lido: &str) -> String {
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho faz() -> {tipo} {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   mapa.definir(m, {chave}, {valor});\n\
         \x20   mimo m;\n\
         }}\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   mapa.definir(faz(), {chave}, {valor});\n\
         \x20   talvez !mapa.tem(faz(), {chave}) {{ mimo 1; }}\n\
         \x20   nova v: {lido} = mapa.obter(faz(), {chave});\n\
         \x20   mapa.definir(faz(), {chave}, v);\n\
         \x20   mapa.remover(faz(), {chave});\n\
         \x20   talvez mapa.tamanho(faz()) != 1 {{ mimo 2; }}\n\
         \x20   mimo 0;\n\
         }}\n",
        tipo = caso.tipo,
        chave = caso.chave,
        valor = caso.valor,
        lido = tipo_do_valor_lido,
    )
}

/// As cinco operações que recebem o mapa no argumento 0.
fn operacoes_com_receptor() -> Vec<GenericMapOperation> {
    GENERIC_MAP_OPERATIONS
        .iter()
        .copied()
        .filter(|operation| operation.recebe_mapa_no_argumento_zero())
        .collect()
}

#[test]
fn projecao_do_parser_concorda_com_a_autoridade() {
    // O parser especializa as cinco operações que recebem o mapa. `criar` é
    // NOT_APPLICABLE nesta fase por razão causal: não há mapa no call site, e a
    // classe só existe depois de o tipo anotado ser resolvido.
    for caso in FONTES {
        let ast = common::render_ast(&fonte_direta(caso)).expect("parse da forma direta");
        for operation in operacoes_com_receptor() {
            let alvo = specialize_spelling(caso.class, operation);
            assert!(
                ast.contains(alvo),
                "AST de {:?} não trouxe '{alvo}' para {operation:?}",
                caso.class
            );
        }
        // A grafia genérica de `criar` sobrevive ao parser, e nenhuma grafia de
        // outra classe aparece.
        assert!(
            ast.contains("mapa_criar"),
            "{:?}: criar especializado antes do tempo",
            caso.class
        );
        for outra in CANONICAL_MAP_CLASSES
            .iter()
            .copied()
            .filter(|&c| c != caso.class)
        {
            for &operation in GENERIC_MAP_OPERATIONS {
                let alvo = specialize_spelling(outra, operation);
                assert!(
                    !ast.contains(alvo),
                    "AST de {:?} vazou '{alvo}', que é de {outra:?}",
                    caso.class
                );
            }
        }
    }
}

#[test]
fn projecao_da_semantica_concorda_com_a_autoridade() {
    // Na forma indireta o parser não especializa, então quem escolhe a célula é
    // a semântica. A escolha é observável pelo contrato que ela passa a exigir:
    // o resultado de `obter` é o valor DA CLASSE. Uma célula trocada mudaria
    // esse tipo, e é isso que o par aceitar/recusar mede.
    for caso in FONTES {
        common::parse_and_check(&fonte_indireta(caso, caso.tipo_valor))
            .unwrap_or_else(|erro| panic!("{:?}: forma indireta recusada: {erro:?}", caso.class));

        let erro = common::parse_and_check(&fonte_indireta(caso, caso.tipo_valor_errado))
            .expect_err("o valor de outra classe deveria ser recusado");
        let texto = format!("{erro:?}");
        assert!(
            texto.contains(caso.tipo_valor),
            "{:?}: o diagnóstico não menciona o valor esperado da classe: {texto}",
            caso.class
        );
    }
}

#[test]
fn projecao_da_ir_concorda_com_a_autoridade() {
    // A IR é a única fase que cobre as 24 células: ela especializa `criar` pela
    // anotação do destino e as outras cinco pelo mapa recebido.
    for caso in FONTES {
        for fonte in [fonte_direta(caso), fonte_indireta(caso, caso.tipo_valor)] {
            let ir = common::render_ir(&fonte).expect("lowering do caso de mapa");
            for &operation in GENERIC_MAP_OPERATIONS {
                let alvo = specialize_spelling(caso.class, operation);
                assert!(
                    ir.contains(alvo),
                    "IR de {:?} não trouxe '{alvo}' para {operation:?}",
                    caso.class
                );
            }
            // Nenhuma grafia genérica sobrevive ao lowering.
            for &operation in GENERIC_MAP_OPERATIONS {
                let generica = operation.generic_public_spelling();
                assert!(
                    !ir.contains(&format!("{generica}(")),
                    "IR de {:?} manteve a grafia genérica '{generica}'",
                    caso.class
                );
            }
            for outra in CANONICAL_MAP_CLASSES
                .iter()
                .copied()
                .filter(|&c| c != caso.class)
            {
                for &operation in GENERIC_MAP_OPERATIONS {
                    let alvo = specialize_spelling(outra, operation);
                    assert!(
                        !ir.contains(alvo),
                        "IR de {:?} vazou '{alvo}', que é de {outra:?}",
                        caso.class
                    );
                }
            }
        }
    }
}

/// Forma `para cada`: o desugaring emite o `tamanho` DA CLASSE.
fn fonte_para_cada(caso: &ClasseFonte) -> String {
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   mapa.definir(m, {chave}, {valor});\n\
         \x20   nova muda n: bombom = 0;\n\
         \x20   para cada c em m {{\n\
         \x20       n = n + 1;\n\
         \x20   }}\n\
         \x20   mimo n - 1;\n\
         }}\n",
        tipo = caso.tipo,
        chave = caso.chave,
        valor = caso.valor,
    )
}

#[test]
fn projecao_do_desugaring_de_para_cada_concorda_com_a_autoridade() {
    // O desugaring de `para cada` era a QUARTA realização da relação: quatro
    // ramos por classe, cada um nomeando o próprio `mapa_<classe>_tamanho`. Ele
    // agora consulta a autoridade, e a projeção continua observável na AST —
    // um ramo que voltasse a escolher a célula errada trocaria a grafia aqui.
    for caso in FONTES {
        let ast = common::render_ast(&fonte_para_cada(caso)).expect("parse do `para cada`");
        let esperado = specialize_spelling(caso.class, GenericMapOperation::Tamanho);
        assert!(
            ast.contains(esperado),
            "`para cada` sobre {:?} não emitiu '{esperado}'",
            caso.class
        );
        for outra in CANONICAL_MAP_CLASSES
            .iter()
            .copied()
            .filter(|&c| c != caso.class)
        {
            let alheio = specialize_spelling(outra, GenericMapOperation::Tamanho);
            assert!(
                !ast.contains(alheio),
                "`para cada` sobre {:?} emitiu '{alheio}', que é de {outra:?}",
                caso.class
            );
        }
        // E o laço continua executando: a consolidação não mudou o desugaring.
        common::parse_and_check(&fonte_para_cada(caso)).expect("`para cada` recusado");
    }
}

// ---------------------------------------------------------------------------
// 6. Controles negativos de fronteira
// ---------------------------------------------------------------------------

#[test]
fn o_mapa_generico_adulto_nao_e_especializado() {
    // D6: o mapa genérico com valor de leque não tem identidade monomórfica
    // pública. Ele é materializado por operação interna, e a autoridade de
    // especialização não responde por ele. Este controle é o que impede a
    // consolidação de crescer para fora do seu domínio.
    let fonte = "pacote main;\n\
                 trazer mapa;\n\
                 \n\
                 leque Escolha { Vazio, Numero(bombom) }\n\
                 \n\
                 carinho principal() -> bombom {\n\
                 \x20   nova m: mapa<bombom,Escolha> = mapa.criar();\n\
                 \x20   mapa.definir(m, 1, Escolha.Numero(7));\n\
                 \x20   mimo mapa.tamanho(m);\n\
                 }\n";
    let ir = common::render_ir(fonte).expect("lowering do mapa genérico adulto");
    assert!(
        ir.contains("__pinker_internal_mapa_definir"),
        "o mapa genérico adulto deixou de usar operação interna: {ir}"
    );
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            let alvo = specialize_spelling(class, operation);
            assert!(
                !ir.contains(alvo),
                "o mapa genérico adulto foi especializado para '{alvo}'"
            );
        }
    }
}

#[test]
fn a_autoridade_nao_responde_por_grafia_que_nao_e_operacao_de_mapa() {
    for &class in CANONICAL_MAP_CLASSES {
        for grafia in [
            "lista_obter",
            "lista_tamanho",
            "tamanho_verso",
            "mapa_iterador_criar",
            "__pinker_internal_mapa_definir",
            // A própria forma monomórfica não é entrada da relação: o lado
            // esquerdo é sempre a grafia GENÉRICA.
            "mapa_verso_bombom_obter",
        ] {
            assert_eq!(
                specialize_generic_spelling(class, grafia),
                None,
                "'{grafia}' foi aceita como operação genérica de mapa em {class:?}"
            );
        }
    }
}

#[test]
fn a_relacao_nao_e_derivada_de_concatenacao_de_texto() {
    // Se a autoridade compusesse o alvo por texto, qualquer grafia com a forma
    // certa produziria um alvo plausível e inexistente. Ela não compõe: o lado
    // direito vem de C1, e só as seis operações declaradas têm célula.
    assert_eq!(
        GenericMapOperation::from_generic_public_spelling("mapa_inventada"),
        None
    );
    assert_eq!(
        specialize_generic_spelling(CanonicalMapClass::VersoBombom, "mapa_inventada"),
        None
    );
}
