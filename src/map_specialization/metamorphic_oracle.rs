//! Prova por EXECUÇÃO de que cada consumidor real deriva a célula da autoridade
//! U-02, em vez de decidir por conta própria (#653, escalonamento A).
//!
//! # Por que esta camada existe
//!
//! A prova de nível B combinava dois detectores e cada um cobria metade da
//! pergunta. A projeção observável recusa a fase que escolhe a célula ERRADA. O
//! censo textual era o único detector da fase que escolhe a célula CERTA por
//! conta própria — e censo textual não fecha classe nenhuma: basta o literal do
//! alvo deixar de aparecer inteiro no texto. Uma revisão adversarial exibiu
//! exatamente isso, com o alvo montado por `concat!`, e o terminal continuou
//! verde.
//!
//! ```text
//! TEXTUAL_CENSUS        = SUPPLEMENTAL_ONLY
//! OBSERVABLE_PROJECTION = DIVERGENCE_DETECTOR
//! METAMORPHIC_EXECUTION = TERMINAL_GUARANTEE
//! ```
//!
//! # A propriedade provada
//!
//! ```text
//! canonical cell         = A
//! test-only mutation     = B
//!
//! real consumer executes
//!   correct derivation       -> consumer selects B
//!   local concordant copy    -> consumer still selects A   -> RED
//! ```
//!
//! Uma mutação contrafactual por SUBSTITUIÇÃO basta. Não existe direção
//! permissiva a provar: o domínio é fechado e funcional — `(classe, operação)`
//! endereça exatamente uma identidade —, e a Task não está tentando admitir
//! entrada nova. A obrigação é `AUTHORITY_CELL_MOVES -> CONSUMER_SELECTION_MOVES`.
//!
//! Isto **não** é o aparato de U-01 importado. Não há oráculo bidirecional, nem
//! testemunha expandida, nem token opaco de produção, e a prova não se
//! generaliza para outra autoridade.
//!
//! # A unidade de obrigação é a aresta REAL
//!
//! A obrigação não é `24 × 5` cego. É:
//!
//! ```text
//! REAL_CONSUMER_EDGE =
//!   (consumer, specialization_cell)
//!   where that consumer actually asks U-02 for that cell
//! ```
//!
//! O conjunto de arestas é **medido**, não declarado: para cada par da grade
//! `5 × 24`, a mutação da célula é instalada e a seleção do consumidor é
//! observada. Se ela se move para o alvo mutado, o par é uma aresta real e está
//! provado. Se nada muda no que aquele consumidor seleciona, o par é
//! `NOT_ASKED`. Nenhum par fica sem veredito, e nenhuma obrigação artificial é
//! inventada para célula que um consumidor não consulta.
//!
//! # Atribuição: cada testemunha isola um decisor
//!
//! O observável de um consumidor só serve como prova se, dentro dele, nenhum
//! OUTRO decisor puder mover a mesma grafia. As testemunhas são escolhidas para
//! isso, e a escolha é verificável no próprio observável canônico:
//!
//! ```text
//! D1 parser         testemunha DIRETA, observável = AST
//!                   `criar` nunca aparece: o parser a deixa genérica
//! D2 semantic       testemunha INDIRETA, observável = diagnóstico da checagem
//!                   sob contrato canônico aceita; sob mutação recusa NOMEANDO o alvo
//! D3 ir             testemunha INDIRETA, observável = IR de `func principal`
//!                   a criação mora em `faz`, fora da fatia: `criar` não aparece
//! D4 para-cada      testemunha de LAÇO sobre mapa vazio, observável = AST
//!                   nenhuma chamada direta de mapa: o único `tamanho` é do desugaring
//! D5 lowering criar testemunha de CRIAÇÃO isolada, observável = IR
//!                   nenhuma operação além de `criar` existe na fonte
//! ```
//!
//! A chave de busca é sempre a tabela CANÔNICA, nunca a célula instalada: o
//! contrafactual move a decisão do consumidor, não o vocabulário do oráculo.

use super::{
    monomorphic_public_spelling, CanonicalMapClass, GenericMapOperation, CANONICAL_MAP_CLASSES,
    GENERIC_MAP_OPERATIONS,
};
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::cell::RefCell;

// ---------------------------------------------------------------------------
// O seam contrafactual
// ---------------------------------------------------------------------------

thread_local! {
    /// A célula substituída, se houver. Uma por thread, instalada por guarda e
    /// desfeita no `Drop`: a prova não depende de ordem de execução e nada
    /// vaza para outro teste.
    static INSTALLED_CELL: RefCell<Option<(CanonicalMapClass, GenericMapOperation, &'static str)>> =
        const { RefCell::new(None) };
}

pub(super) fn installed_cell_target(
    class: CanonicalMapClass,
    operation: GenericMapOperation,
) -> Option<&'static str> {
    INSTALLED_CELL.with(|slot| {
        slot.borrow().and_then(|(instalada, op, alvo)| {
            (instalada == class && op == operation).then_some(alvo)
        })
    })
}

/// Instala uma célula mutada enquanto viver.
struct MutatedCell;

impl MutatedCell {
    fn install(
        class: CanonicalMapClass,
        operation: GenericMapOperation,
        target: &'static str,
    ) -> Self {
        INSTALLED_CELL.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(
                slot.is_none(),
                "mutação aninhada: a prova perderia o controle de qual célula está substituída"
            );
            *slot = Some((class, operation, target));
        });
        Self
    }
}

impl Drop for MutatedCell {
    fn drop(&mut self) {
        INSTALLED_CELL.with(|slot| *slot.borrow_mut() = None);
    }
}

// ---------------------------------------------------------------------------
// Testemunhas
// ---------------------------------------------------------------------------

/// A grafia Pinker de cada classe concreta, e os valores que a povoam.
struct ClassSource {
    class: CanonicalMapClass,
    map_type: &'static str,
    key: &'static str,
    value: &'static str,
    value_type: &'static str,
}

const CLASS_SOURCES: &[ClassSource] = &[
    ClassSource {
        class: CanonicalMapClass::VersoBombom,
        map_type: "mapa<verso,bombom>",
        key: "\"a\"",
        value: "1",
        value_type: "bombom",
    },
    ClassSource {
        class: CanonicalMapClass::VersoVerso,
        map_type: "mapa<verso,verso>",
        key: "\"a\"",
        value: "\"x\"",
        value_type: "verso",
    },
    ClassSource {
        class: CanonicalMapClass::BombomBombom,
        map_type: "mapa<bombom,bombom>",
        key: "7",
        value: "1",
        value_type: "bombom",
    },
    ClassSource {
        class: CanonicalMapClass::BombomVerso,
        map_type: "mapa<bombom,verso>",
        key: "7",
        value: "\"x\"",
        value_type: "verso",
    },
];

fn class_source(class: CanonicalMapClass) -> &'static ClassSource {
    CLASS_SOURCES
        .iter()
        .find(|caso| caso.class == class)
        .expect("classe canônica com fonte de testemunha")
}

/// Testemunha DIRETA: o mapa é um `Ident` de tipo declarado, e o parser
/// especializa as cinco operações que recebem o mapa. `criar` continua genérica
/// na AST.
fn direct_witness(class: CanonicalMapClass) -> String {
    let caso = class_source(class);
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
        tipo = caso.map_type,
        chave = caso.key,
        valor = caso.value,
        tipo_valor = caso.value_type,
    )
}

/// Testemunha INDIRETA: o mapa vem de uma chamada, então o parser não pode
/// especializar e a decisão cai para a semântica e para a IR. `faz` NÃO chama
/// operação nenhuma sobre um `Ident`, para que nenhuma grafia da fatia de
/// `principal` venha do parser.
fn indirect_witness(class: CanonicalMapClass) -> String {
    let caso = class_source(class);
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho faz() -> {tipo} {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   mimo m;\n\
         }}\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   mapa.definir(faz(), {chave}, {valor});\n\
         \x20   talvez !mapa.tem(faz(), {chave}) {{ mimo 1; }}\n\
         \x20   nova v: {tipo_valor} = mapa.obter(faz(), {chave});\n\
         \x20   mapa.definir(faz(), {chave}, v);\n\
         \x20   mapa.remover(faz(), {chave});\n\
         \x20   talvez mapa.tamanho(faz()) != 0 {{ mimo 2; }}\n\
         \x20   mimo 0;\n\
         }}\n",
        tipo = caso.map_type,
        chave = caso.key,
        valor = caso.value,
        tipo_valor = caso.value_type,
    )
}

/// Testemunha de LAÇO: `para cada` sobre mapa vazio. Nenhuma chamada direta de
/// operação de mapa existe na fonte, então o único `tamanho` da AST é o que o
/// desugaring materializa.
fn para_cada_witness(class: CanonicalMapClass) -> String {
    let caso = class_source(class);
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   nova muda n: bombom = 0;\n\
         \x20   para cada c em m {{\n\
         \x20       n = n + 1;\n\
         \x20   }}\n\
         \x20   mimo n;\n\
         }}\n",
        tipo = caso.map_type,
    )
}

/// Testemunha de CRIAÇÃO isolada: a única operação de mapa da fonte é `criar`.
fn criar_witness(class: CanonicalMapClass) -> String {
    let caso = class_source(class);
    format!(
        "pacote main;\n\
         trazer mapa;\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   nova m: {tipo} = mapa.criar();\n\
         \x20   mimo 0;\n\
         }}\n",
        tipo = caso.map_type,
    )
}

// ---------------------------------------------------------------------------
// Observáveis
// ---------------------------------------------------------------------------

fn parse(source: &str) -> crate::ast::Program {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("testemunha lexa");
    Parser::new(tokens).parse().expect("testemunha parseia")
}

fn ast_text(source: &str) -> String {
    crate::printer::render_program(&parse(source))
}

/// Diagnóstico da checagem semântica, vazio quando ela aceita.
///
/// É o observável mais próximo da decisão que a semântica tem: ela não emite a
/// grafia escolhida em lugar nenhum quando aceita, e quando recusa a mensagem
/// NOMEIA a grafia que ela selecionou.
fn semantic_text(source: &str) -> String {
    match crate::semantic::check_program(&parse(source)) {
        Ok(()) => String::new(),
        Err(erro) => format!("{erro:?}"),
    }
}

/// IR da testemunha, sem passar pela checagem semântica.
///
/// A checagem é deliberadamente evitada: sob mutação ela recusa o programa —
/// isso é a aresta de D2 — e recusar cedo esconderia a decisão da IR. Quando o
/// lowering não consegue resolver o alvo mutado, o texto do erro também NOMEIA a
/// grafia que a IR selecionou, e serve igualmente como observável.
fn ir_text(source: &str) -> String {
    match crate::ir::lower_program(&parse(source)) {
        Ok(programa) => crate::ir::render_program(&programa),
        Err(erro) => format!("{erro:?}"),
    }
}

/// A fatia de uma função na IR renderizada.
fn ir_function_slice(ir: &str, name: &str) -> String {
    let abre = format!("  func {name} ");
    let Some(inicio) = ir.find(&abre) else {
        return String::new();
    };
    let resto = &ir[inicio + abre.len()..];
    match resto.find("\n  func ") {
        Some(fim) => resto[..fim].to_string(),
        None => resto.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Consumidores
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Consumer {
    D1Parser,
    D2Semantic,
    D3Ir,
    D4ParaCada,
    D5LoweringCriar,
}

const CONSUMERS: &[Consumer] = &[
    Consumer::D1Parser,
    Consumer::D2Semantic,
    Consumer::D3Ir,
    Consumer::D4ParaCada,
    Consumer::D5LoweringCriar,
];

impl Consumer {
    fn realization(self) -> &'static str {
        match self {
            Self::D1Parser => "src/parser/mod.rs CollectionKind::generic_map_callee",
            Self::D2Semantic => "src/semantic/calls.rs generic_map_monomorphic_callee",
            Self::D3Ir => "src/ir.rs generic_map_monomorphic_callee",
            Self::D4ParaCada => "src/parser/lacos.rs desugaring de `para cada`",
            Self::D5LoweringCriar => "src/ir/lowering.rs lowering de `mapa_criar`",
        }
    }

    /// O texto em que a seleção deste consumidor é observável.
    fn observable(self, class: CanonicalMapClass) -> String {
        match self {
            Self::D1Parser => ast_text(&direct_witness(class)),
            Self::D2Semantic => semantic_text(&indirect_witness(class)),
            Self::D3Ir => ir_function_slice(&ir_text(&indirect_witness(class)), "principal"),
            Self::D4ParaCada => ast_text(&para_cada_witness(class)),
            Self::D5LoweringCriar => ir_text(&criar_witness(class)),
        }
    }
}

/// A classe cuja grafia para `operation` aparece no observável.
///
/// A busca usa a tabela CANÔNICA como vocabulário: o contrafactual move a
/// decisão do consumidor, não as chaves do oráculo. Duas classes presentes ao
/// mesmo tempo significam que a testemunha não isola um decisor, e isso é falha
/// da prova, não resultado.
fn selected_class(observable: &str, operation: GenericMapOperation) -> Option<CanonicalMapClass> {
    let encontradas: Vec<CanonicalMapClass> = CANONICAL_MAP_CLASSES
        .iter()
        .copied()
        .filter(|&candidata| observable.contains(monomorphic_public_spelling(candidata, operation)))
        .collect();
    assert!(
        encontradas.len() <= 1,
        "testemunha ambígua para {operation:?}: {encontradas:?} coexistem no observável"
    );
    encontradas.first().copied()
}

/// A classe seguinte no ciclo. O alvo mutado é a MESMA operação em outra classe
/// concreta, que é sempre grafia histórica válida de C1 — nunca um alvo
/// inexistente, que faria a recusa vir do mutante e não da decisão.
fn next_class(class: CanonicalMapClass) -> CanonicalMapClass {
    let indice = CANONICAL_MAP_CLASSES
        .iter()
        .position(|&candidata| candidata == class)
        .expect("classe canônica enumerada");
    CANONICAL_MAP_CLASSES[(indice + 1) % CANONICAL_MAP_CLASSES.len()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeVerdict {
    /// O consumidor consulta esta célula, e a seleção dele acompanhou a mutação.
    RealEdgeProved,
    /// Mutar a célula não muda nada do que este consumidor seleciona.
    NotAsked,
}

/// Mede um par `(consumidor, célula)` e devolve o veredito.
fn measure(
    consumer: Consumer,
    class: CanonicalMapClass,
    operation: GenericMapOperation,
) -> EdgeVerdict {
    let canonical_target = monomorphic_public_spelling(class, operation);
    let mutated_class = next_class(class);
    let mutated_target = monomorphic_public_spelling(mutated_class, operation);
    assert_ne!(canonical_target, mutated_target);
    assert!(
        crate::intrinsics::registry::e_historica(mutated_target),
        "o alvo mutado precisa ser grafia histórica de C1, não um alvo inexistente"
    );

    let antes = selected_class(&consumer.observable(class), operation);

    let depois = {
        let _mutacao = MutatedCell::install(class, operation, mutated_target);
        selected_class(&consumer.observable(class), operation)
    };

    // A mutação não vaza: o guarda restaurou a célula.
    assert_eq!(
        installed_cell_target(class, operation),
        None,
        "a célula mutada sobreviveu ao guarda"
    );

    if depois == Some(mutated_class) {
        // Aresta real. Sob contrato canônico o consumidor tem de estar
        // selecionando a célula canônica — ou, no caso da semântica, aceitando
        // o programa sem nomear grafia nenhuma.
        match consumer {
            Consumer::D2Semantic => assert_eq!(
                antes, None,
                "{:?}/{class:?}/{operation:?}: a semântica deveria ACEITAR sob contrato canônico",
                consumer
            ),
            _ => assert_eq!(
                antes,
                Some(class),
                "{:?}/{class:?}/{operation:?}: sob contrato canônico o consumidor não selecionou a célula canônica",
                consumer
            ),
        }
        EdgeVerdict::RealEdgeProved
    } else {
        assert_eq!(
            antes, depois,
            "{:?}/{class:?}/{operation:?}: a mutação mexeu no observável sem levá-lo ao alvo mutado — \
             nem aresta provada, nem célula não consultada",
            consumer
        );
        EdgeVerdict::NotAsked
    }
}

// ---------------------------------------------------------------------------
// A prova
// ---------------------------------------------------------------------------

/// Toda a grade `consumidor × célula`, medida e classificada.
fn grid() -> Vec<(
    Consumer,
    CanonicalMapClass,
    GenericMapOperation,
    EdgeVerdict,
)> {
    let mut medidas = Vec::new();
    for &consumer in CONSUMERS {
        for &class in CANONICAL_MAP_CLASSES {
            for &operation in GENERIC_MAP_OPERATIONS {
                medidas.push((
                    consumer,
                    class,
                    operation,
                    measure(consumer, class, operation),
                ));
            }
        }
    }
    medidas
}

#[test]
fn a_grade_inteira_tem_veredito_e_nenhuma_aresta_real_fica_sem_prova() {
    let medidas = grid();
    assert_eq!(
        medidas.len(),
        CONSUMERS.len() * CANONICAL_MAP_CLASSES.len() * GENERIC_MAP_OPERATIONS.len(),
        "a grade deixou de cobrir consumidor × célula"
    );

    // Cada par tem exatamente um veredito, e `measure` já falha alto em
    // qualquer terceiro estado. Logo não existe par sem prova nem skip sem
    // explicação: o veredito NotAsked é medido, não declarado.
    let provadas = medidas
        .iter()
        .filter(|(_, _, _, v)| *v == EdgeVerdict::RealEdgeProved)
        .count();
    let nao_consultadas = medidas.len() - provadas;

    assert!(provadas > 0, "nenhuma aresta real medida");
    assert_eq!(
        provadas + nao_consultadas,
        medidas.len(),
        "par sem veredito na grade"
    );

    // O piso factual do reinventário: 20 + 20 + 20 + 4 + 4.
    let por_consumidor = |consumer: Consumer| {
        medidas
            .iter()
            .filter(|(c, _, _, v)| *c == consumer && *v == EdgeVerdict::RealEdgeProved)
            .count()
    };
    assert_eq!(por_consumidor(Consumer::D1Parser), 20, "D1");
    assert_eq!(por_consumidor(Consumer::D2Semantic), 20, "D2");
    assert_eq!(por_consumidor(Consumer::D3Ir), 20, "D3");
    assert_eq!(por_consumidor(Consumer::D4ParaCada), 4, "D4");
    assert_eq!(por_consumidor(Consumer::D5LoweringCriar), 4, "D5");
    assert_eq!(provadas, 68, "REAL_CONSUMER_EDGES");
}

#[test]
fn as_celulas_nao_consultadas_sao_exatamente_as_operacoes_fora_de_cada_realizacao() {
    // A fotografia inversa: cada `NotAsked` medido cai numa razão causal
    // declarada. Um skip que não caia em nenhuma delas reprova aqui, e é isso
    // que torna `UNEXPLAINED_SKIPS = 0` uma medida em vez de uma promessa.
    for (consumer, class, operation, veredito) in grid() {
        if veredito != EdgeVerdict::NotAsked {
            continue;
        }
        let esperado = match consumer {
            // Não há mapa no call site: a classe de `criar` vem da anotação do
            // destino e só existe depois de o tipo ser resolvido.
            Consumer::D1Parser => operation == GenericMapOperation::Criar,
            // A semântica apenas RECONHECE a criação genérica; quem a
            // especializa é o lowering.
            Consumer::D2Semantic => operation == GenericMapOperation::Criar,
            // `criar` não passa pela tabela de callee da IR: ela é
            // materializada pelo braço de `let` do lowering, que é D5.
            Consumer::D3Ir => operation == GenericMapOperation::Criar,
            // O desugaring de `para cada` materializa só `tamanho`.
            Consumer::D4ParaCada => operation != GenericMapOperation::Tamanho,
            // O braço de `let` do lowering materializa só `criar`.
            Consumer::D5LoweringCriar => operation != GenericMapOperation::Criar,
        };
        assert!(
            esperado,
            "skip sem razão causal declarada: {consumer:?} não consulta {class:?}/{operation:?}"
        );
    }
}

#[test]
fn a_mutacao_e_isolada_por_thread_e_desfeita_pelo_guarda() {
    let class = CanonicalMapClass::VersoBombom;
    let operation = GenericMapOperation::Obter;
    let canonica = monomorphic_public_spelling(class, operation);
    let mutada = monomorphic_public_spelling(next_class(class), operation);

    assert_eq!(super::specialize_spelling(class, operation), canonica);
    {
        let _mutacao = MutatedCell::install(class, operation, mutada);
        assert_eq!(super::specialize_spelling(class, operation), mutada);
        // Só a célula instalada muda; as outras 23 continuam canônicas.
        for &outra_classe in CANONICAL_MAP_CLASSES {
            for &outra_op in GENERIC_MAP_OPERATIONS {
                if outra_classe == class && outra_op == operation {
                    continue;
                }
                assert_eq!(
                    super::specialize_spelling(outra_classe, outra_op),
                    monomorphic_public_spelling(outra_classe, outra_op),
                    "a mutação vazou para {outra_classe:?}/{outra_op:?}"
                );
            }
        }
        // Outra thread não vê a substituição desta.
        let vista_em_outra_thread =
            std::thread::spawn(move || super::specialize_spelling(class, operation))
                .join()
                .expect("thread de leitura");
        assert_eq!(
            vista_em_outra_thread, canonica,
            "a mutação atravessou a thread"
        );
    }
    assert_eq!(super::specialize_spelling(class, operation), canonica);
}

#[test]
fn a_producao_nao_tem_o_seam() {
    // O caminho de produção é a tabela, sem substituição possível. Aqui o
    // `cfg(test)` está ativo por construção, então o que se mede é o contrato do
    // seam: sem célula instalada, `cell_target` É `monomorphic_public_spelling`.
    for &class in CANONICAL_MAP_CLASSES {
        for &operation in GENERIC_MAP_OPERATIONS {
            assert_eq!(installed_cell_target(class, operation), None);
            assert_eq!(
                super::cell_target(class, operation),
                monomorphic_public_spelling(class, operation)
            );
        }
    }
}

#[test]
fn o_relatorio_da_grade_nomeia_cada_realizacao() {
    // Serve ao relatório terminal: a contagem por realização, com o caminho
    // real de cada uma, para que a aresta não vire número anônimo.
    let medidas = grid();
    for &consumer in CONSUMERS {
        let provadas = medidas
            .iter()
            .filter(|(c, _, _, v)| *c == consumer && *v == EdgeVerdict::RealEdgeProved)
            .count();
        assert!(
            !consumer.realization().is_empty(),
            "realização sem caminho declarado"
        );
        assert!(
            provadas > 0,
            "{consumer:?} ({}) não tem aresta real provada",
            consumer.realization()
        );
    }
}
