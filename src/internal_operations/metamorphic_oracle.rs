//! Oráculo metamórfico do contrato de operações internas — G651-02.
//!
//! A pergunta terminal da #651 não é *“consigo reconhecer no texto todas as
//! formas que parecem uma segunda tabela?”*. Reconhecer forma é corrida
//! perdida, e o fechamento dirigido do HEAD `f8683d87` provou isso: 19 de 21
//! reimplementações locais do MESMO contrato passaram pelo guard sintático
//! mudando apenas macro, aritmética, padrão, helper ou arquivo hospedeiro.
//!
//! A pergunta que este módulo responde é operacional:
//!
//! ```text
//! MUTATE(CANONICAL_FACT)
//! ->
//! ALL_RELEVANT_CONSUMERS_OBSERVE_MUTATION
//! ```
//!
//! O procedimento, para cada fato compartilhado:
//!
//! 1. construir os artefatos de cada fase UMA vez, sob a autoridade real;
//! 2. ler a resposta observável de cada consumidor;
//! 3. instalar uma variante MUTADA do contrato canônico (a costura de
//!    [`super::contract_seam`], viva só sob `cfg(test)`);
//! 4. reler a mesma resposta dos mesmos artefatos.
//!
//! Um consumidor que pergunte à autoridade muda junto. Um consumidor com
//! decisão local independente continua respondendo o valor ANTIGO, e o teste
//! fica vermelho — **independentemente de arquivo, posição, helper, macro,
//! operador, alias ou forma textual**, porque o oráculo nunca olha o texto.
//!
//! ```text
//! CANONICAL_FACT changes
//! LOCAL_DUPLICATE remains old
//! -> METAMORPHIC_ORACLE = RED
//! ```
//!
//! Duas propriedades deste desenho merecem ser ditas em voz alta.
//!
//! **Não há segunda tabela canônica dentro do teste.** Cada mutação é derivada
//! da autoridade real por cópia e edição de UM campo; o oráculo declara o
//! *delta* esperado (quais consumidores devem acompanhar), nunca o catálogo.
//! Acrescentar, remover ou reescrever uma operação na autoridade não exige
//! tocar neste arquivo.
//!
//! **Fato de fase não vira autoridade compartilhada.** Cada prova também exige
//! que os consumidores fora da aresta real NÃO mudem. Predicado de comparação
//! de tipo da fase, texto de diagnóstico, decisão do produtor sobre QUANDO
//! emitir, corpo do interpretador e binding nativo no dono legítimo continuam
//! locais por construção: o oráculo só observa o resultado da pergunta
//! compartilhada.

use super::contract_seam::MutatedContract;
use super::{
    InternalOperands, InternalOperation, InternalOperationFamily, InternalResult,
    INTERNAL_OPERATIONS, TERNARIA,
};
use crate::abstract_machine::{self, MachineProgram};
use crate::abstract_machine_validate;
use crate::ast::Program;
use crate::cfg_ir::{self, ProgramCfgIR};
use crate::cfg_ir_validate;
use crate::error::PinkerError;
use crate::instr_select::{self, SelectedProgram};
use crate::instr_select_validate;
use crate::ir::{self, ProgramIR, TypeIR};
use crate::ir_validate;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::semantic;

// ---------------------------------------------------------------------------
// Sondas
// ---------------------------------------------------------------------------

/// Sonda principal: exercita a família genérica de mapa, o leque com carga e a
/// escolha ternária no MESMO programa, para que todos os consumidores de fase
/// vejam operação interna de verdade.
const SONDA: &str = r#"
pacote main; trazer mapa.criar; trazer mapa.definir; trazer mapa.obter; trazer mapa.remover; trazer mapa.tamanho; trazer mapa.tem;

leque Escolha { Vazio, Numero(bombom) }
leque Resultado { Ok(bombom), Erro(verso) }

carinho validar(a: bombom, ok: logica) -> Resultado {
    talvez ok {
        mimo Resultado.Ok(a);
    }
    mimo Resultado.Erro("falha validada");
}

carinho principal() -> bombom {
    nova m: mapa<bombom, Escolha> = criar();
    definir(m, 1, Escolha.Numero(41));
    nova presente: logica = tem(m, 1);
    nova escolhido: Escolha = obter(m, 1);
    nova muda soma: bombom = 0;
    encaixe escolhido {
        caso Escolha.Numero(n) { soma = soma + n; }
        caso Escolha.Vazio { soma = soma + 1; }
    }
    nova muda ordem: bombom = 0;
    para cada chave em m { ordem = ordem + chave; }
    remover(m, 1);

    nova vb: mapa<verso,bombom> = criar();
    definir(vb, "um", 1);
    nova muda parcial: bombom = 0;
    para cada k em vb { parcial = parcial + obter(vb, k); }

    nova bom: Resultado = validar(42, verdade);
    tentar bom {
        sucesso Resultado.Ok(valor) { soma = soma + valor; }
        falha Resultado.Erro(msg) { falar(msg); }
    }

    nova ajuste: bombom = presente ? 1 : 0;
    mimo tamanho(m) + ordem + soma + ajuste + parcial;
}
"#;

/// Sonda do subset externo montável (Fase 214): a pseudo-chamada ternária
/// sobrevive à seleção e é o emissor de `backend_s` que a consome.
const SONDA_EXTERNA: &str = r#"
pacote main;
carinho principal() -> bombom {
    nova a: bombom = 5;
    nova r: bombom = a > 3 ? 42 : 7;
    mimo r;
}
"#;

// ---------------------------------------------------------------------------
// Sentinelas
// ---------------------------------------------------------------------------

/// F1 — operação de contrato relativo ao mapa recebido.
const SENTINELA_FAMILIA: &str = "__pinker_internal_mapa_tem";
/// F3 — construção de leque vazio, com operandos declarados.
const SENTINELA_OPERANDOS: &str = "__pinker_internal_leque_criar_0";
/// F1/pertinência e F4 — leitura da etiqueta do leque: a operação que o
/// desugaring de `tentar` deixa como chamada no AST e cujo retorno o lowering
/// de `ir/context` resolve pela tabela de assinaturas.
const SENTINELA_RESULTADO: &str = "__pinker_internal_leque_tag";

/// Operandos mutados de [`SENTINELA_OPERANDOS`]: a classe do operando passa de
/// `bombom` para `verso`.
const OPERANDOS_MUTADOS: &[TypeIR] = &[TypeIR::Verso];

/// Operandos mutados da ternária: quatro, em vez de três.
const RAMIFICACAO_MUTADA: &[TypeIR] = &[
    TypeIR::Logica,
    TypeIR::Bombom,
    TypeIR::Bombom,
    TypeIR::Bombom,
];

// ---------------------------------------------------------------------------
// Mutações
// ---------------------------------------------------------------------------

/// Mutação pontual do contrato canônico. Cada variante muda UM fato.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutacao {
    /// F1 — a família estrutural da operação sentinela.
    FamiliaDaOperacao,
    /// F1 — a pertinência: a operação sentinela deixa de existir.
    PertinenciaDaOperacao,
    /// F2 — a aridade da escolha ternária: 3 -> 4.
    AridadeDaTernaria,
    /// F3 — a classe do primeiro operando da carga de leque.
    ContratoDeOperando,
    /// F4 — o tipo de resultado da construção de leque.
    ContratoDeResultado,
}

impl Mutacao {
    fn nome(self) -> &'static str {
        match self {
            Self::FamiliaDaOperacao => "F1/família",
            Self::PertinenciaDaOperacao => "F1/pertinência",
            Self::AridadeDaTernaria => "F2/aridade",
            Self::ContratoDeOperando => "F3/operandos",
            Self::ContratoDeResultado => "F4/resultado",
        }
    }

    /// A tabela mutada, derivada da canônica por cópia e edição de um campo.
    fn tabela(self) -> Vec<InternalOperation> {
        let mut operacoes = INTERNAL_OPERATIONS.to_vec();
        match self {
            Self::FamiliaDaOperacao => editar(&mut operacoes, SENTINELA_FAMILIA, |operacao| {
                operacao.family = InternalOperationFamily::MapaMonomorfica;
            }),
            Self::PertinenciaDaOperacao => {
                let antes = operacoes.len();
                operacoes.retain(|operacao| operacao.spelling != SENTINELA_RESULTADO);
                assert_eq!(
                    operacoes.len() + 1,
                    antes,
                    "sentinela '{SENTINELA_RESULTADO}' saiu da autoridade"
                );
            }
            Self::AridadeDaTernaria => editar(&mut operacoes, TERNARIA, |operacao| {
                operacao.operands = InternalOperands::Declarados(RAMIFICACAO_MUTADA);
            }),
            Self::ContratoDeOperando => editar(&mut operacoes, SENTINELA_OPERANDOS, |operacao| {
                operacao.operands = InternalOperands::Declarados(OPERANDOS_MUTADOS);
            }),
            Self::ContratoDeResultado => editar(&mut operacoes, SENTINELA_RESULTADO, |operacao| {
                operacao.result = InternalResult::Declarado(TypeIR::Verso);
            }),
        }
        operacoes
    }
}

fn editar(
    operacoes: &mut [InternalOperation],
    spelling: &str,
    aplicar: impl FnOnce(&mut InternalOperation),
) {
    let operacao = operacoes
        .iter_mut()
        .find(|operacao| operacao.spelling == spelling)
        .unwrap_or_else(|| panic!("sentinela '{spelling}' saiu da autoridade"));
    aplicar(operacao);
}

// ---------------------------------------------------------------------------
// Consumidores
// ---------------------------------------------------------------------------

/// Os oito decisores que o reinventário da #651 encontrou repetindo F1–F4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Consumidor {
    Semantic,
    IrContext,
    IrModel,
    IrValidate,
    CfgIrValidate,
    InstrSelectValidate,
    AbstractMachineValidate,
    BackendSExternalCallconv,
}

const CONSUMIDORES: &[Consumidor] = &[
    Consumidor::Semantic,
    Consumidor::IrContext,
    Consumidor::IrModel,
    Consumidor::IrValidate,
    Consumidor::CfgIrValidate,
    Consumidor::InstrSelectValidate,
    Consumidor::AbstractMachineValidate,
    Consumidor::BackendSExternalCallconv,
];

impl Consumidor {
    fn nome(self) -> &'static str {
        match self {
            Self::Semantic => "semantic/calls",
            Self::IrContext => "ir/context",
            Self::IrModel => "ir/model",
            Self::IrValidate => "ir_validate",
            Self::CfgIrValidate => "cfg_ir_validate",
            Self::InstrSelectValidate => "instr_select_validate",
            Self::AbstractMachineValidate => "abstract_machine_validate",
            Self::BackendSExternalCallconv => "backend_s/external_callconv",
        }
    }

    /// A resposta observável deste consumidor sobre artefatos já construídos.
    ///
    /// Recusar-se a continuar TAMBÉM é resposta: `semantic/calls` trata a
    /// ausência de contrato como erro de programa do compilador, e o oráculo
    /// precisa ler isso como mudança, não como queda do teste.
    fn resposta(self, artefatos: &Artefatos) -> String {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resposta_direta(artefatos)
        }))
        .unwrap_or_else(|_| "INTERROMPE".to_string())
    }

    fn resposta_direta(self, artefatos: &Artefatos) -> String {
        match self {
            Self::Semantic => veredito(semantic::check_program(&artefatos.programa)),
            Self::IrContext => match ir::lower_program(&artefatos.programa) {
                Ok(baixado) => format!("IR#{:016x}", impressao(&format!("{baixado:?}"))),
                Err(erro) => format!("RECUSA: {erro}"),
            },
            Self::IrModel => format!(
                "generica={}",
                ir::is_generic_map_intrinsic(SENTINELA_FAMILIA)
            ),
            Self::IrValidate => veredito(ir_validate::validate_program(&artefatos.ir)),
            Self::CfgIrValidate => veredito(cfg_ir_validate::validate_program(&artefatos.cfg)),
            Self::InstrSelectValidate => veredito(instr_select_validate::validate_program(
                &artefatos.selecionado,
            )),
            Self::AbstractMachineValidate => veredito(abstract_machine_validate::validate_program(
                &artefatos.maquina,
            )),
            Self::BackendSExternalCallconv => veredito(
                crate::backend_s::emit_external_toolchain_subset(&artefatos.selecionado_externo),
            ),
        }
    }
}

fn veredito<T>(resultado: Result<T, PinkerError>) -> String {
    match resultado {
        Ok(_) => "ACEITA".to_string(),
        Err(erro) => format!("RECUSA: {erro}"),
    }
}

/// FNV-1a de 64 bits: compacta a IR baixada numa resposta comparável.
fn impressao(texto: &str) -> u64 {
    texto.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

// ---------------------------------------------------------------------------
// Artefatos
// ---------------------------------------------------------------------------

/// Artefatos de cada fase, construídos UMA vez sob a autoridade real.
///
/// Construir antes da mutação é o que torna a prova por consumidor: nenhum
/// consumidor herda a recusa de um anterior, e a redundância entre fases não
/// esconde uma reimplementação local numa fase só.
struct Artefatos {
    programa: Program,
    ir: ProgramIR,
    cfg: ProgramCfgIR,
    selecionado: SelectedProgram,
    maquina: MachineProgram,
    selecionado_externo: SelectedProgram,
}

fn analisar(fonte: &str) -> Program {
    let mut lexer = Lexer::new(fonte);
    let tokens = lexer.tokenize().expect("sonda deve lexar");
    Parser::new(tokens).parse().expect("sonda deve analisar")
}

fn ate_selecao(fonte: &str) -> (Program, ProgramIR, ProgramCfgIR, SelectedProgram) {
    let programa = analisar(fonte);
    semantic::check_program(&programa).expect("sonda deve passar na semântica");
    let ir = ir::lower_program(&programa).expect("sonda deve baixar para a IR");
    ir_validate::validate_program(&ir).expect("IR da sonda deve validar");
    let cfg = cfg_ir::lower_program(&ir).expect("sonda deve baixar para a CFG");
    cfg_ir_validate::validate_program(&cfg).expect("CFG da sonda deve validar");
    let selecionado = instr_select::lower_program(&cfg).expect("sonda deve selecionar");
    instr_select_validate::validate_program(&selecionado).expect("seleção da sonda deve validar");
    (programa, ir, cfg, selecionado)
}

fn artefatos() -> Artefatos {
    let (programa, ir, cfg, selecionado) = ate_selecao(SONDA);
    let maquina =
        abstract_machine::lower_program(&selecionado).expect("sonda deve baixar à máquina");
    abstract_machine_validate::validate_program(&maquina).expect("máquina da sonda deve validar");
    let (_, _, _, selecionado_externo) = ate_selecao(SONDA_EXTERNA);
    crate::backend_s::emit_external_toolchain_subset(&selecionado_externo)
        .expect("sonda externa deve ser emitida pelo subset montável");
    Artefatos {
        programa,
        ir,
        cfg,
        selecionado,
        maquina,
        selecionado_externo,
    }
}

// ---------------------------------------------------------------------------
// O oráculo
// ---------------------------------------------------------------------------

/// Executa a prova metamórfica de uma mutação.
///
/// `arestas` é o delta esperado — os consumidores que respondem a este fato. O
/// teste falha nas duas direções: se um consumidor esperado NÃO acompanhar a
/// autoridade (decisão local sobrevivendo à mutação) e se um consumidor fora da
/// aresta acompanhar (fato de fase confundido com decisão compartilhada).
fn provar(mutacao: Mutacao, arestas: &[Consumidor]) {
    let artefatos = artefatos();
    let antes: Vec<String> = CONSUMIDORES
        .iter()
        .map(|consumidor| consumidor.resposta(&artefatos))
        .collect();
    let depois: Vec<String> = {
        let _mutado = MutatedContract::install(mutacao.tabela());
        CONSUMIDORES
            .iter()
            .map(|consumidor| consumidor.resposta(&artefatos))
            .collect()
    };

    let mut divergencias = Vec::new();
    for (indice, consumidor) in CONSUMIDORES.iter().enumerate() {
        let acompanhou = antes[indice] != depois[indice];
        let esperado = arestas.contains(consumidor);
        if acompanhou == esperado {
            continue;
        }
        divergencias.push(if esperado {
            format!(
                "{}: NÃO acompanhou a mutação {} — decisão local sobreviveu (antes={:?}, depois={:?})",
                consumidor.nome(),
                mutacao.nome(),
                antes[indice],
                depois[indice]
            )
        } else {
            format!(
                "{}: acompanhou a mutação {} sem ser aresta declarada (antes={:?}, depois={:?})",
                consumidor.nome(),
                mutacao.nome(),
                antes[indice],
                depois[indice]
            )
        });
    }

    assert!(
        divergencias.is_empty(),
        "oráculo metamórfico vermelho em {}:\n  {}",
        mutacao.nome(),
        divergencias.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// A matriz factual
// ---------------------------------------------------------------------------

/// Quais consumidores respondem a cada fato, medido — não presumido.
///
/// A matriz declara o DELTA esperado, nunca o catálogo: nenhuma linha repete
/// aridade, operando, resultado ou grafia de operação alguma. Acrescentar ou
/// reescrever uma operação na autoridade não exige tocar aqui.
///
/// Uma ausência é afirmação de fato, não lacuna. `instr_select_validate` não
/// responde a F2 porque a seleção desvia da checagem de assinatura quando o
/// callee é a ternária; `backend_s/external_callconv` só responde a F2 porque
/// o emissor do subset montável só pergunta pela aridade dela;
/// `ir/model` só responde a F1 porque a única pergunta que ele faz é a família.
const MATRIZ: &[(Mutacao, &[Consumidor])] = &[
    (
        Mutacao::FamiliaDaOperacao,
        &[
            Consumidor::IrModel,
            Consumidor::IrValidate,
            Consumidor::CfgIrValidate,
            Consumidor::InstrSelectValidate,
            Consumidor::AbstractMachineValidate,
        ],
    ),
    (
        Mutacao::PertinenciaDaOperacao,
        &[
            Consumidor::Semantic,
            Consumidor::IrContext,
            Consumidor::IrValidate,
            Consumidor::CfgIrValidate,
            Consumidor::InstrSelectValidate,
            Consumidor::AbstractMachineValidate,
        ],
    ),
    (
        Mutacao::AridadeDaTernaria,
        &[
            Consumidor::Semantic,
            Consumidor::IrValidate,
            Consumidor::CfgIrValidate,
            Consumidor::AbstractMachineValidate,
            Consumidor::BackendSExternalCallconv,
        ],
    ),
    (
        Mutacao::ContratoDeOperando,
        &[
            Consumidor::IrValidate,
            Consumidor::CfgIrValidate,
            Consumidor::AbstractMachineValidate,
        ],
    ),
    (
        Mutacao::ContratoDeResultado,
        &[
            Consumidor::IrContext,
            Consumidor::IrValidate,
            Consumidor::CfgIrValidate,
            Consumidor::InstrSelectValidate,
            Consumidor::AbstractMachineValidate,
        ],
    ),
];

fn arestas(mutacao: Mutacao) -> &'static [Consumidor] {
    MATRIZ
        .iter()
        .find(|(declarada, _)| *declarada == mutacao)
        .map(|(_, arestas)| *arestas)
        .expect("mutação sem linha na matriz")
}

// ---------------------------------------------------------------------------
// As provas
// ---------------------------------------------------------------------------

#[test]
fn f1_familia_da_operacao_chega_a_todo_consumidor_de_familia() {
    provar(
        Mutacao::FamiliaDaOperacao,
        arestas(Mutacao::FamiliaDaOperacao),
    );
}

#[test]
fn f1_pertinencia_da_operacao_chega_a_todo_consumidor_de_existencia() {
    provar(
        Mutacao::PertinenciaDaOperacao,
        arestas(Mutacao::PertinenciaDaOperacao),
    );
}

#[test]
fn f2_aridade_chega_a_todo_consumidor_de_aridade() {
    provar(
        Mutacao::AridadeDaTernaria,
        arestas(Mutacao::AridadeDaTernaria),
    );
}

#[test]
fn f3_contrato_de_operando_chega_a_todo_consumidor_de_operando() {
    provar(
        Mutacao::ContratoDeOperando,
        arestas(Mutacao::ContratoDeOperando),
    )
}

#[test]
fn f4_contrato_de_resultado_chega_a_todo_consumidor_de_resultado() {
    provar(
        Mutacao::ContratoDeResultado,
        arestas(Mutacao::ContratoDeResultado),
    );
}

#[test]
fn todo_consumidor_da_autoridade_tem_ao_menos_uma_aresta_provada() {
    // Resposta à pergunta 7 da revisão dirigida: nenhum dos oito decisores que
    // o reinventário encontrou repetindo F1–F4 fica sem edge coberto.
    let descobertos: Vec<&'static str> = CONSUMIDORES
        .iter()
        .filter(|consumidor| {
            !MATRIZ
                .iter()
                .any(|(_, arestas)| arestas.contains(consumidor))
        })
        .map(|consumidor| consumidor.nome())
        .collect();
    assert!(
        descobertos.is_empty(),
        "consumidores da autoridade sem aresta metamórfica: {descobertos:?}"
    );
}

#[test]
fn sem_mutacao_instalada_a_autoridade_e_a_tabela_canonica() {
    // A costura não pode alterar a resposta de produção. Sem guarda instalado,
    // toda consulta enxerga exatamente `INTERNAL_OPERATIONS`.
    for operacao in INTERNAL_OPERATIONS {
        assert_eq!(super::entrada(operacao.spelling), Some(operacao));
    }
    let declaradas: Vec<&str> = super::assinaturas_declaradas()
        .map(|(spelling, _, _)| spelling)
        .collect();
    let esperadas: Vec<&str> = INTERNAL_OPERATIONS
        .iter()
        .filter(|operacao| operacao.assinatura_ir().is_some())
        .map(|operacao| operacao.spelling)
        .collect();
    assert_eq!(declaradas, esperadas);
}

#[test]
fn a_mutacao_e_local_a_thread_e_termina_com_o_guarda() {
    let antes = super::aridade(TERNARIA);
    {
        let _mutado = MutatedContract::install(Mutacao::AridadeDaTernaria.tabela());
        assert_ne!(super::aridade(TERNARIA), antes);
    }
    assert_eq!(super::aridade(TERNARIA), antes);
}
