//! Oráculo metamórfico do contrato de operações internas — G651-02.
//!
//! A pergunta terminal da #651 não é *“consigo reconhecer no texto todas as
//! formas que parecem uma segunda tabela?”*. Reconhecer forma é corrida
//! perdida: o fechamento dirigido do HEAD `f8683d87` passou 19 de 21
//! reimplementações locais pelo guard sintático mudando apenas macro,
//! aritmética, padrão, helper ou arquivo hospedeiro.
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
//! Um consumidor que pergunte à autoridade muda junto. Um consumidor cuja
//! decisão local SOMBREIA a autoridade — decide no lugar dela, e por isso pode
//! divergir dela — continua respondendo o valor ANTIGO, e o teste fica vermelho
//! — **independentemente de arquivo, posição, helper, macro, operador, alias ou
//! forma textual**, porque o oráculo nunca olha o texto.
//!
//! O fechamento dirigido de `acb6ac6` mostrou que o MECANISMO estava certo e a
//! COBERTURA não: com uma sentinela por fato, decisão local sobre outra
//! operação do mesmo fato escapava, e a lista manual de consumidores era ao
//! mesmo tempo universo e objeto auditado. As duas correções estruturam este
//! arquivo.
//!
//! **O domínio é a autoridade.** A prova terminal ITERA [`INTERNAL_OPERATIONS`]
//! × fatos e classifica cada par em `PROVADO` ou `ESTRUTURALMENTE NÃO
//! APLICÁVEL`. Nenhuma sentinela escolhida a dedo define o universo, e operação
//! nova na autoridade sem testemunha deixa a cobertura vermelha.
//!
//! **O universo de consumidores é descoberto, não declarado.** Duas fontes
//! independentes da escrituração respondem quem consulta a autoridade: a
//! própria autoridade anota, sob `cfg(test)`, o arquivo de quem a chama
//! ([`super::registro`]), e o inventário estático varre `src/**` atrás de quem
//! nomeia a API canônica. A primeira cobre o que as sondas executam; a segunda
//! cobre o compilador inteiro, inclusive fases fora do corpus. A escrituração
//! deste arquivo é conferida contra as duas e contra o piso histórico de oito
//! decisores: tirar um consumidor daqui deixa o teste vermelho, porque a
//! descoberta continua achando o consumidor real.
//!
//! Duas propriedades merecem ser ditas em voz alta.
//!
//! **Não há segunda tabela canônica dentro do teste.** Cada mutação nasce da
//! entrada canônica corrente por cópia e edição de UM fato; a cobertura declara
//! relações de teste — quem decide qual fato, para que forma de operação, com
//! qual sonda por testemunha — e nunca aridade, operandos ou resultado. Esses
//! valores continuam existindo só em [`INTERNAL_OPERATIONS`].
//!
//! **Fato de fase não vira autoridade compartilhada.** A declaração de quais
//! fatos cada decisor consulta é conferida nas duas direções contra o que a
//! execução mede sobre o domínio inteiro, e os controles negativos da suíte de
//! integração exigem que decisão local legítima — predicado de comparação de
//! tipo, texto de diagnóstico, corpo do interpretador, binding nativo no dono —
//! continue invisível para o oráculo.
//!
//! ## A prova anda nos dois sentidos
//!
//! Mutar só para ESTREITAR prova dependência de um lado só, e duas classes de
//! decisão local sobrevivem a essa metade:
//!
//! * a tabela local ADITIVA e concorde, que roda ao lado da autoridade e
//!   concorda com ela no estado canônico — o caminho da autoridade continua
//!   vivo, ainda move a resposta, e a duplicação fica coberta por ele;
//! * a decisão local numa célula que a fase não PERGUNTA — nada a obriga a
//!   acompanhar, e “não pergunta” passa por “não decide”.
//!
//! [`direcao_permissiva_por_aridade`] fecha as duas para F2. O artefato
//! EXPANDIDO — um operando a mais em cada chamada à operação — é ilegal sob a
//! autoridade canônica e legal sob a mutação de aridade, e aí a matriz tem
//! exigência dos dois lados: quem consulta precisa passar de RECUSA a ACEITE, e
//! quem tem carve-out precisa ACEITAR nos dois estados.
//!
//! Para F1/família, F3 e F4 a cirurgia é outra, porque a mutação não
//! acrescenta operando: ela TROCA a classe exigida de um operando ou de um
//! resultado. O artefato que ela admite é um sítio de chamada NOVO, construído
//! a partir do contrato MUTADO e recusado pelo canônico —
//! [`direcao_permissiva_por_fato`]. Trocar a classe X por Y retira X e admite
//! Y: a mutação não precisa ser superconjunto global da canônica para que a
//! testemunha seja nova.
//!
//! Para F1/pertinência a direção que falta é a entrada NOVA:
//! [`direcao_permissiva_por_pertinencia`] instala uma entrada SINTÉTICA,
//! viva só dentro da variante, e exige que quem deriva pertinência da
//! autoridade passe a admiti-la.
//!
//! **A descoberta atravessa repassador.** A instrumentação dinâmica anota um
//! quadro de pilha só, então uma função que devolve a resposta da autoridade
//! esconderia quem a chama. [`repassadores`] acha cada função cujo CORPO é a
//! consulta, e
//! [`nenhum_consumidor_alcanca_a_autoridade_por_indirecao`] exige que quem a
//! chame esteja escriturado. Entrada de fase não é repassador: um
//! `validate_program` também consulta a autoridade, mas devolve o veredito da
//! fase, não o fato.
//!
//! ## O domínio provado, e só ele
//!
//! ```text
//! AUTHORITY_PROOF =
//! BOUNDED_BIDIRECTIONAL_SEMANTIC_METAMORPHIC_EXECUTION
//! ```
//!
//! Uma suíte finita NÃO prova que nenhum trecho de Rust arbitrário possa
//! responder à mesma pergunta em algum lugar da árvore. Não é isso que está
//! escrito aqui. O que esta prova estabelece, e mede a cada execução, é um
//! domínio nomeado:
//!
//! * o **domínio compartilhado declarado** — as operações de
//!   [`INTERNAL_OPERATIONS`], iteradas, nunca amostradas;
//! * as **dimensões** de cada fato — pertinência, família, aridade, classe e
//!   discriminante de operando, classe e presença de resultado —, cada uma
//!   mutada por si, porque uma mutação que move duas dimensões pode esconder
//!   duplicação na que não foi observada;
//! * os **consumidores reais**, descobertos por três vias independentes da
//!   escrituração — registro dinâmico, inventário estático de `src/**` e
//!   inventário transitivo de repassadores — e conferidos contra o piso
//!   histórico de oito decisores;
//! * as **classes materiais de duplicação**: a decisão local que SOMBREIA a
//!   autoridade (decide no lugar dela) e a que roda ADITIVA e CONCORDE ao lado
//!   dela (concorda no estado canônico);
//! * testemunhas **restritivas** e **complementares** para cada obrigação, com
//!   a disposição de cada uma MEDIDA, nunca declarada.
//!
//! A obrigação direcional é por
//! `(operação, fato, dimensão, consumidor, representação, testemunha,
//! observável)`, e termina em exatamente uma destas classes:
//!
//! * **provada por admissão** — existe testemunha nova, ilegal sob a canônica e
//!   legal sob a mutada, e o consumidor a admite;
//! * **provada por projeção** — o observável do consumidor é uma resposta
//!   DERIVADA, não um veredito, e ela segue a autoridade nos dois estados e os
//!   distingue;
//! * **provada por remoção de rota** — a autoridade mutada não admite chamada
//!   alguma naquela fase, medido pela recusa, sob ela, da testemunha construída
//!   a partir dela; quem continuar ACEITANDO só pode estar decidindo por conta
//!   própria, e a matriz restritiva exige e mede essa mudança;
//! * **equivalência de representação** — tudo que a mutada admite, a canônica
//!   já admitia naquela fase: não existe caso novo a recusar;
//! * **estruturalmente não aplicável** — a variante descreve um contrato
//!   incoerente consigo mesmo, lido do tipo da própria autoridade.
//!
//! `direcao_permissiva_por_fato` exige zero obrigações fora dessas classes.
//! Nenhuma delas é atalho: “não consegui construir a testemunha” não classifica
//! nada, e deixa a obrigação SEM PROVA — vermelha.
//!
//! O que fica de fora, dito com todas as letras: o runtime e o interpretador
//! não entram em mutação contrafactual (a prova termina antes de exigir
//! execução de assinatura que produção nunca implementou); o binding ABI
//! continua sendo de `backend_s` e não é exigido da entrada sintética; e o
//! guard léxico da suíte de integração permanece `SUPPLEMENTAL_ONLY`.

use super::contract_seam::MutatedContract;
use super::registro::Gravacao;
use super::{
    InternalOperands, InternalOperation, InternalOperationFamily, InternalResult, MapOperandRole,
    INTERNAL_OPERATIONS, TERNARIA,
};
use crate::abstract_machine::{self, MachineProgram};
use crate::abstract_machine_validate;
use crate::ast::{Block, ElseBlock, Expr, ExprKind, Program, Stmt};
use crate::cfg_ir::{self, ProgramCfgIR};
use crate::cfg_ir_validate;
use crate::error::PinkerError;
use crate::instr_select::{self, SelectedProgram};
use crate::instr_select_validate;
use crate::intrinsics::identity::CalleeIdentity;
use crate::ir::MapKeyIR;
use crate::ir::{self, BlockIR, InstructionIR, ProgramIR, TypeIR, ValueIR};
use crate::ir_validate;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::semantic;

// ---------------------------------------------------------------------------
// Corpus de sondas
// ---------------------------------------------------------------------------
//
// Um único programa que materializasse as trinta e uma operações seria um
// monstro frágil. O corpus é plural de propósito; o que precisa ser provado — e
// é, em `toda_operacao_declarada_tem_testemunha` — é a COMPLETUDE dele contra a
// autoridade, não a economia de sondas.

/// Sonda principal: família genérica de mapa com chave `bombom`, leque com
/// carga imediata e `verso`, especialização `verso->bombom` e escolha ternária.
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

/// Sonda das classes de mapa que a principal não materializa: chave `verso`
/// genérica e as três especializações restantes.
const SONDA_MAPAS: &str = r#"
pacote main; trazer mapa.criar; trazer mapa.definir; trazer mapa.tamanho;

leque Cor { Rosa, Azul }

carinho principal() -> bombom {
    nova vl: mapa<verso, Cor> = criar();
    definir(vl, "a", Cor.Rosa);
    nova muda n: bombom = 0;
    para cada kv em vl { n = n + tamanho(vl); }

    nova vv: mapa<verso,verso> = criar();
    definir(vv, "a", "b");
    para cada k2 em vv { n = n + tamanho(vv); }

    nova bb: mapa<bombom,bombom> = criar();
    definir(bb, 1, 2);
    para cada k3 em bb { n = n + k3; }

    nova bv: mapa<bombom,verso> = criar();
    definir(bv, 1, "x");
    para cada k4 em bv { n = n + k4; }

    mimo n;
}
"#;

/// Sonda das cargas de leque com handle de lista.
const SONDA_LEQUES: &str = r#"
pacote main; trazer lista;

leque Pacote { Numeros(lista<bombom>), Textos(lista<verso>) }

carinho principal() -> bombom {
    nova ns: lista<bombom> = lista.bombom_criar();
    nova ts: lista<verso> = lista.verso_criar();
    nova a: Pacote = Pacote.Numeros(ns);
    nova b: Pacote = Pacote.Textos(ts);
    nova muda total: bombom = 0;
    encaixe a {
        caso Pacote.Numeros(x) { total = total + lista.bombom_tamanho(x); }
        caso Pacote.Textos(y) { total = total + lista.verso_tamanho(y); }
    }
    encaixe b {
        caso Pacote.Numeros(x2) { total = total + lista.bombom_tamanho(x2); }
        caso Pacote.Textos(y2) { total = total + lista.verso_tamanho(y2); }
    }
    mimo total;
}
"#;

/// Sonda da carga de handle opaco nominal: `SaidaProcesso`, da Parte D.
const SONDA_PROCESSO: &str = r#"
pacote main; trazer lista; trazer mapa; trazer processo;
apelido Res = Resultado<SaidaProcesso, verso>;

carinho principal() -> bombom {
    nova argumentos: lista<verso> = lista.verso_criar();
    nova ambiente: mapa<verso,verso> = mapa.verso_verso_criar();
    nova r: Res = processo.executar_estruturado("/bin/true", argumentos, "", ".", ambiente, LimiteTempo.Ate(1000));
    encaixe r {
        caso Res.Ok(saida) { nova copia: Res = Res.Ok(saida); mimo 0; }
        caso Res.Erro(erro) { falar(erro); mimo 1; }
    }
    mimo 2;
}
"#;

const CORPUS: &[(&str, &str)] = &[
    ("principal", SONDA),
    ("externa", SONDA_EXTERNA),
    ("mapas", SONDA_MAPAS),
    ("leques", SONDA_LEQUES),
    ("processo", SONDA_PROCESSO),
];

// ---------------------------------------------------------------------------
// Artefatos
// ---------------------------------------------------------------------------

/// Índice do estágio cujo artefato cada consumidor examina.
const AST: usize = 0;
const IR: usize = 1;
const CFG: usize = 2;
const SEL: usize = 3;
const MAQ: usize = 4;

/// Artefatos de uma sonda, construídos UMA vez sob a autoridade real.
///
/// Construir antes da mutação é o que torna a prova por consumidor: nenhum
/// consumidor herda a recusa de um anterior, e a redundância entre fases não
/// esconde uma reimplementação local numa fase só.
struct Sonda {
    nome: &'static str,
    programa: Program,
    ir: ProgramIR,
    cfg: ProgramCfgIR,
    selecionado: SelectedProgram,
    maquina: MachineProgram,
    /// Impressão `Debug` de cada estágio, para medir ALCANCE — se o artefato
    /// que o consumidor examina contém mesmo uma CHAMADA à operação.
    ///
    /// É medição do artefato, não declaração: nada aqui repete contrato.
    textos: [String; 5],
}

impl Sonda {
    fn construir(nome: &'static str, fonte: &str) -> Self {
        let mut lexer = Lexer::new(fonte);
        let tokens = lexer.tokenize().expect("sonda deve lexar");
        let programa = Parser::new(tokens).parse().expect("sonda deve analisar");
        semantic::check_program(&programa).expect("sonda deve passar na semântica");
        let ir = ir::lower_program(&programa).expect("sonda deve baixar para a IR");
        ir_validate::validate_program(&ir).expect("IR da sonda deve validar");
        let cfg = cfg_ir::lower_program(&ir).expect("sonda deve baixar para a CFG");
        cfg_ir_validate::validate_program(&cfg).expect("CFG da sonda deve validar");
        let selecionado = instr_select::lower_program(&cfg).expect("sonda deve selecionar");
        instr_select_validate::validate_program(&selecionado)
            .expect("seleção da sonda deve validar");
        let maquina =
            abstract_machine::lower_program(&selecionado).expect("sonda deve baixar à máquina");
        abstract_machine_validate::validate_program(&maquina)
            .expect("máquina da sonda deve validar");
        let textos = [
            format!("{programa:?}"),
            format!("{ir:?}"),
            format!("{cfg:?}"),
            format!("{selecionado:?}"),
            format!("{maquina:?}"),
        ];
        Self {
            nome,
            programa,
            ir,
            cfg,
            selecionado,
            maquina,
            textos,
        }
    }

    /// O artefato deste estágio materializa uma CHAMADA a esta grafia?
    ///
    /// A pergunta é de sítio de chamada, não de ocorrência do nome: a grafia
    /// também aparece como METADADO (`extract_intrinsic` da carga de leque), e
    /// metadado não é decisão de contrato a tomar. Na AST a chamada nomeia a
    /// grafia num identificador; das fases baixadas em diante ela vive no campo
    /// `callee`. A grafia é reservada na fronteira léxica e nenhum programa
    /// pode escrevê-la, então a ocorrência nessas duas formas é sempre a
    /// chamada transportada.
    fn materializa(&self, estagio: usize, spelling: &str) -> bool {
        let sitio = if estagio == AST {
            format!("Ident(\"{spelling}\")")
        } else {
            format!("callee: \"{spelling}\"")
        };
        self.textos[estagio].contains(&sitio)
    }

    fn testemunha(&self, spelling: &str) -> bool {
        (AST..=MAQ).any(|estagio| self.materializa(estagio, spelling))
    }
}

fn corpus() -> Vec<Sonda> {
    CORPUS
        .iter()
        .map(|(nome, fonte)| Sonda::construir(nome, fonte))
        .collect()
}

// ---------------------------------------------------------------------------
// Escrituração de consumidores
// ---------------------------------------------------------------------------

/// Um decisor que consulta a autoridade.
///
/// Esta lista é ESCRITURAÇÃO, não universo: quem existe é decidido pela
/// descoberta de [`super::registro`], e `descoberta_e_escrituracao_coincidem`
/// exige que as duas coincidam. Tirar uma linha daqui não esconde o consumidor.
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

/// Piso histórico: os oito decisores que o reinventário da #651 encontrou
/// repetindo F1–F4 antes da consolidação. A descoberta precisa reencontrar
/// todos. Um que tenha deixado legitimamente de consultar a autoridade é
/// matéria de disposição do Guia, não de edição silenciosa desta lista.
const PISO_HISTORICO: &[&str] = &[
    "src/abstract_machine_validate.rs",
    "src/backend_s/external_callconv.rs",
    "src/cfg_ir_validate.rs",
    "src/instr_select_validate.rs",
    "src/ir/context.rs",
    "src/ir/model.rs",
    "src/ir_validate.rs",
    "src/semantic/calls.rs",
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

    /// Arquivo que a descoberta anota quando este decisor consulta.
    fn arquivo(self) -> &'static str {
        match self {
            Self::Semantic => "src/semantic/calls.rs",
            Self::IrContext => "src/ir/context.rs",
            Self::IrModel => "src/ir/model.rs",
            Self::IrValidate => "src/ir_validate.rs",
            Self::CfgIrValidate => "src/cfg_ir_validate.rs",
            Self::InstrSelectValidate => "src/instr_select_validate.rs",
            Self::AbstractMachineValidate => "src/abstract_machine_validate.rs",
            Self::BackendSExternalCallconv => "src/backend_s/external_callconv.rs",
        }
    }

    /// Estágio cujo artefato este decisor examina. É o estágio que decide
    /// ALCANCE: um validador de CFG não vê operação que o lowering não emitiu.
    fn estagio(self) -> usize {
        match self {
            // O contexto de lowering resolve o retorno das chamadas que JÁ
            // estão na AST; a fase que ele alimenta é a mesma da semântica.
            Self::Semantic | Self::IrContext => AST,
            Self::IrModel | Self::IrValidate => IR,
            Self::CfgIrValidate => CFG,
            Self::InstrSelectValidate | Self::BackendSExternalCallconv => SEL,
            Self::AbstractMachineValidate => MAQ,
        }
    }

    /// Este decisor alcança uma chamada desta operação nesta sonda?
    fn alcanca(self, sonda: &Sonda, spelling: &str) -> bool {
        if self == Self::IrModel {
            // `ir/model` não é uma fase: é o predicado que as fases consultam,
            // cada uma com o callee da representação dela. O alcance dele é
            // qualquer estágio que materialize a chamada, não um estágio só.
            return sonda.testemunha(spelling);
        }
        sonda.materializa(self.estagio(), spelling)
    }

    /// A resposta observável deste consumidor sobre artefatos já construídos.
    ///
    /// Recusar-se a continuar TAMBÉM é resposta: `semantic/calls` trata a
    /// ausência de contrato como erro de programa do compilador, e o oráculo
    /// precisa ler isso como mudança, não como queda do teste.
    fn resposta(self, sonda: &Sonda, spelling: &str) -> String {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.resposta_direta(sonda, spelling)
        }))
        .unwrap_or_else(|_| "INTERROMPE".to_string())
    }

    fn resposta_direta(self, sonda: &Sonda, spelling: &str) -> String {
        match self {
            Self::Semantic => veredito(semantic::check_program(&sonda.programa)),
            Self::IrContext => match ir::lower_program(&sonda.programa) {
                Ok(baixado) => format!("IR#{:016x}", impressao(&format!("{baixado:?}"))),
                Err(erro) => format!("RECUSA: {erro}"),
            },
            // `ir/model` não valida artefato: expõe um predicado puro sobre a
            // grafia. A resposta observável dele é o predicado na operação sob
            // prova — por isso a grafia chega aqui.
            Self::IrModel => format!("generica={}", ir::is_generic_map_intrinsic(spelling)),
            Self::IrValidate => veredito(ir_validate::validate_program(&sonda.ir)),
            Self::CfgIrValidate => veredito(cfg_ir_validate::validate_program(&sonda.cfg)),
            Self::InstrSelectValidate => {
                veredito(instr_select_validate::validate_program(&sonda.selecionado))
            }
            Self::AbstractMachineValidate => {
                veredito(abstract_machine_validate::validate_program(&sonda.maquina))
            }
            Self::BackendSExternalCallconv => veredito(
                crate::backend_s::emit_external_toolchain_subset(&sonda.selecionado),
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
// Fatos e mutações derivadas do contrato canônico
// ---------------------------------------------------------------------------

/// Fato compartilhado do contrato estrutural.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fato {
    /// F1 — a operação existe na autoridade.
    Existencia,
    /// F1 — a família estrutural da operação.
    Familia,
    /// F2 — quantos operandos a operação exige.
    Aridade,
    /// F3 — o contrato estrutural dos operandos.
    Operandos,
    /// F4 — o contrato estrutural do resultado.
    Resultado,
}

/// Os fatos cuja duplicação ADITIVA e concorde é material, e que por isso
/// precisam de prova na direção permissiva além da restritiva.
///
/// F2/aridade tem prova própria em [`direcao_permissiva_por_aridade`], pelo
/// artefato EXPANDIDO, que é a forma mais forte: ela não constrói sítio novo,
/// alarga TODOS os sítios que a sonda já materializa.
const FATOS_PERMISSIVOS: &[Fato] = &[Fato::Familia, Fato::Operandos, Fato::Resultado];

const FATOS: &[Fato] = &[
    Fato::Existencia,
    Fato::Familia,
    Fato::Aridade,
    Fato::Operandos,
    Fato::Resultado,
];

impl Fato {
    fn nome(self) -> &'static str {
        match self {
            Self::Existencia => "F1/existência",
            Self::Familia => "F1/família",
            Self::Aridade => "F2/aridade",
            Self::Operandos => "F3/operandos",
            Self::Resultado => "F4/resultado",
        }
    }
}

fn vazar<T: 'static>(valores: Vec<T>) -> &'static [T] {
    Box::leak(valores.into_boxed_slice())
}

/// A outra classe escalar de transporte, para mudar um operando de classe.
fn outra_classe(tipo: TypeIR) -> TypeIR {
    if tipo == TypeIR::Bombom {
        TypeIR::Verso
    } else {
        TypeIR::Bombom
    }
}

/// A dimensão do fato que uma variante move.
///
/// Um fato pode ter mais de uma dimensão, e uma mutação só pode esconder
/// duplicação: trocar `Declarado(logica)` por `Declarado(nulo)` move ao mesmo
/// tempo a CLASSE e a PRESENÇA do resultado, e um consumidor que decida a
/// classe localmente continua acompanhando pela presença. Cada dimensão é
/// mutada por si.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dimensao {
    /// F1 — a operação deixa de existir.
    Pertinencia,
    /// F1 — a família passa a outra variante válida.
    VarianteDeFamilia,
    /// F2 — um operando a mais.
    OperandoAMais,
    /// F3 — a classe declarada de um operando, que era esta.
    ClasseDeOperando(TypeIR),
    /// F3 — os operandos deixam de ser relativos ao mapa recebido.
    DiscriminanteDeOperandos,
    /// F4 — a classe (ou a chave) do resultado.
    ClasseDoResultado,
    /// F4 — a operação passa a produzir valor, ou deixa de produzir.
    PresencaDoResultado,
}

impl Dimensao {
    fn nome(self) -> &'static str {
        match self {
            Self::Pertinencia => "pertinência",
            Self::VarianteDeFamilia => "variante de família",
            Self::OperandoAMais => "um operando a mais",
            Self::ClasseDeOperando(_) => "classe de operando",
            Self::DiscriminanteDeOperandos => "discriminante de operandos",
            Self::ClasseDoResultado => "classe do resultado",
            Self::PresencaDoResultado => "presença do resultado",
        }
    }
}

/// Uma variante mutada do contrato canônico para um par (operação, fato).
struct Variante {
    dimensao: Dimensao,
    tabela: Vec<InternalOperation>,
}

/// A classe de pilha desta classe IR é definida?
///
/// A máquina abstrata projeta operando e resultado em classe de pilha e trata
/// o que não sabe projetar como compatível com tudo. Mutação que entra ou sai
/// desse `desconhecido` não pode falsificar nada nessa fase — é limite de
/// resolução da camada, não decisão local escondida.
fn projetavel_na_pilha(tipo: TypeIR) -> bool {
    matches!(
        tipo,
        TypeIR::Bombom | TypeIR::Logica | TypeIR::Verso | TypeIR::ListBombom | TypeIR::ListVerso
    )
}

/// A outra chave de mapa, para mover o resultado de uma criação.
fn outra_chave(chave: MapKeyIR) -> MapKeyIR {
    match chave {
        MapKeyIR::Bombom => MapKeyIR::Verso,
        MapKeyIR::Verso => MapKeyIR::Bombom,
    }
}

/// As variantes mutadas de um par (operação, fato).
///
/// Cada variante NASCE da entrada canônica corrente — cópia, edição de UMA
/// dimensão do fato — e nunca de uma reconstrução manual do contrato esperado.
/// `Err` classifica o par como estruturalmente não aplicável, com a razão.
///
/// Um fato pode ter mais de uma variante porque uma mutação só pode esconder
/// duplicação: trocar `Declarado(logica)` por `Declarado(nulo)` move ao mesmo
/// tempo a CLASSE e a PRESENÇA do resultado, e um consumidor que decida a
/// classe localmente continua acompanhando pela presença. Cada dimensão é
/// mutada por si.
fn variantes(spelling: &str, fato: Fato) -> Result<Vec<Variante>, &'static str> {
    let canonica = INTERNAL_OPERATIONS
        .iter()
        .find(|operacao| operacao.spelling == spelling)
        .expect("operação do domínio canônico");

    let editar = |aplicar: &dyn Fn(&mut InternalOperation)| {
        let mut operacoes = INTERNAL_OPERATIONS.to_vec();
        let indice = operacoes
            .iter()
            .position(|operacao| operacao.spelling == spelling)
            .expect("operação do domínio canônico");
        aplicar(&mut operacoes[indice]);
        operacoes
    };

    let mut variantes: Vec<Variante> = Vec::new();
    match fato {
        Fato::Existencia => {
            let mut operacoes = INTERNAL_OPERATIONS.to_vec();
            operacoes.retain(|operacao| operacao.spelling != spelling);
            variantes.push(Variante {
                dimensao: Dimensao::Pertinencia,
                tabela: operacoes,
            });
        }
        Fato::Familia => {
            // Troca por outra variante válida e diferente, escolhida para que a
            // troca seja observável: o que distingue `MapaGenerica` das demais
            // é justamente o contrato depender do mapa recebido.
            let destino = if canonica.family == InternalOperationFamily::MapaGenerica {
                InternalOperationFamily::MapaMonomorfica
            } else {
                InternalOperationFamily::MapaGenerica
            };
            variantes.push(Variante {
                dimensao: Dimensao::VarianteDeFamilia,
                tabela: editar(&|operacao| operacao.family = destino),
            });
        }
        Fato::Aridade => {
            let operandos = match canonica.operands {
                InternalOperands::Declarados(params) => {
                    // Repete o ÚLTIMO operando declarado, pelo mesmo motivo dos
                    // papéis de mapa: acrescentar uma CLASSE nova moveria junto
                    // o contrato de operando, e a mutação de aridade deixaria de
                    // ser de aridade só. Repetir também é o que torna o artefato
                    // EXPANDIDO construtível — duplicar o último argumento passa
                    // a conformar exatamente a este contrato.
                    let mut mutados = params.to_vec();
                    mutados.push(*params.last().unwrap_or(&TypeIR::Bombom));
                    InternalOperands::Declarados(vazar(mutados))
                }
                InternalOperands::PapeisDeMapa(papeis) => {
                    // Repete o ÚLTIMO papel já presente. Empurrar um papel novo
                    // moveria junto o discriminante — `papeis.contains(&Chave)`
                    // desvia as fases para outro ramo do `match`, e a mutação de
                    // aridade deixaria de ser de aridade só, mascarando
                    // duplicação local no ramo que nem chega a ser alcançado.
                    let mut mutados = papeis.to_vec();
                    mutados.push(*papeis.last().unwrap_or(&MapOperandRole::Receptor));
                    InternalOperands::PapeisDeMapa(vazar(mutados))
                }
                // A ramificação não carrega lista de operandos: a aridade dela
                // é a própria forma. Passar a três+1 declarados é a mutação de
                // aridade disponível sem inventar representação nova.
                InternalOperands::Ramificacao => InternalOperands::Declarados(vazar(vec![
                    TypeIR::Logica,
                    TypeIR::Bombom,
                    TypeIR::Bombom,
                    TypeIR::Bombom,
                ])),
            };
            variantes.push(Variante {
                dimensao: Dimensao::OperandoAMais,
                tabela: editar(&|operacao| operacao.operands = operandos),
            });
        }
        Fato::Operandos => match canonica.operands {
            InternalOperands::Declarados([]) | InternalOperands::PapeisDeMapa([]) => {
                return Err("operação sem operandos: não há contrato de operando a mutar")
            }
            InternalOperands::Ramificacao => {
                return Err("ramificação não declara tipo de operando: os ramos são livres")
            }
            InternalOperands::Declarados(params) => {
                let primeiro = {
                    let mut mutados = params.to_vec();
                    mutados[0] = outra_classe(mutados[0]);
                    InternalOperands::Declarados(vazar(mutados))
                };
                variantes.push(Variante {
                    dimensao: Dimensao::ClasseDeOperando(params[0]),
                    tabela: editar(&|operacao| operacao.operands = primeiro),
                });
                if params.len() > 1 {
                    let ultimo = {
                        let mut mutados = params.to_vec();
                        let fim = mutados.len() - 1;
                        mutados[fim] = outra_classe(mutados[fim]);
                        InternalOperands::Declarados(vazar(mutados))
                    };
                    variantes.push(Variante {
                        dimensao: Dimensao::ClasseDeOperando(params[params.len() - 1]),
                        tabela: editar(&|operacao| operacao.operands = ultimo),
                    });
                }
            }
            InternalOperands::PapeisDeMapa(papeis) => {
                // Discriminante diferente com a MESMA aridade: os operandos
                // deixam de ser relativos ao mapa recebido.
                let declarados =
                    InternalOperands::Declarados(vazar(vec![TypeIR::Bombom; papeis.len()]));
                variantes.push(Variante {
                    dimensao: Dimensao::DiscriminanteDeOperandos,
                    tabela: editar(&|operacao| operacao.operands = declarados),
                });
            }
        },
        Fato::Resultado => match canonica.result {
            // O resultado é o tipo dos ramos: a autoridade declara aqui que NÃO
            // fixa resultado, e cada fase o deriva do call site. É a contraparte
            // de `Ramificacao` no lado do resultado, e a medição confirma:
            // nenhum consumidor consulta `result` fora dos caminhos guardados
            // por contrato relativo ao mapa.
            InternalResult::TipoDosRamos => {
                return Err("resultado é o tipo dos ramos: a autoridade não fixa resultado")
            }
            // Operação sem valor: a única dimensão que existe é a presença.
            InternalResult::Declarado(TypeIR::Nulo) => variantes.push(Variante {
                dimensao: Dimensao::PresencaDoResultado,
                tabela: editar(&|operacao| {
                    operacao.result = InternalResult::Declarado(TypeIR::Bombom)
                }),
            }),
            _ => {
                let classe = match canonica.result {
                    InternalResult::Declarado(tipo) => {
                        InternalResult::Declarado(outra_classe(tipo))
                    }
                    InternalResult::MapaNovoComChave(chave) => {
                        InternalResult::MapaNovoComChave(outra_chave(chave))
                    }
                    // O valor do mapa recebido não tem classe fixa para trocar;
                    // passar a “mapa novo com chave” é o resultado de outro
                    // discriminante que nenhum valor de mapa pode coincidir.
                    _ => InternalResult::MapaNovoComChave(MapKeyIR::Bombom),
                };
                variantes.push(Variante {
                    dimensao: Dimensao::ClasseDoResultado,
                    tabela: editar(&|operacao| operacao.result = classe),
                });
                variantes.push(Variante {
                    dimensao: Dimensao::PresencaDoResultado,
                    tabela: editar(&|operacao| {
                        operacao.result = InternalResult::Declarado(TypeIR::Nulo)
                    }),
                });
            }
        },
    }
    Ok(variantes)
}

// ---------------------------------------------------------------------------
// Superfície de decisão: quem decide qual fato, para que forma de operação
// ---------------------------------------------------------------------------

/// Fatos que cada decisor consulta na autoridade.
///
/// Isto é relação de teste, não contrato: nenhuma linha diz aridade, operando
/// ou resultado de operação alguma. A declaração é conferida nas duas direções
/// por `declaracao_de_fatos_por_consumidor_e_exata`, contra o que a execução
/// metamórfica mede sobre o domínio inteiro — linha vazia ou faltante fica
/// vermelha.
const FATOS_POR_CONSUMIDOR: &[(Consumidor, &[Fato])] = &[
    (
        Consumidor::Semantic,
        &[
            Fato::Existencia,
            Fato::Aridade,
            Fato::Operandos,
            Fato::Resultado,
        ],
    ),
    (Consumidor::IrContext, &[Fato::Existencia, Fato::Resultado]),
    (Consumidor::IrModel, &[Fato::Existencia, Fato::Familia]),
    (
        Consumidor::IrValidate,
        &[
            Fato::Existencia,
            Fato::Familia,
            Fato::Aridade,
            Fato::Operandos,
            Fato::Resultado,
        ],
    ),
    (
        Consumidor::CfgIrValidate,
        &[
            Fato::Existencia,
            Fato::Familia,
            Fato::Aridade,
            Fato::Operandos,
            Fato::Resultado,
        ],
    ),
    (
        Consumidor::InstrSelectValidate,
        &[Fato::Existencia, Fato::Familia, Fato::Resultado],
    ),
    (
        Consumidor::AbstractMachineValidate,
        &[
            Fato::Existencia,
            Fato::Familia,
            Fato::Aridade,
            Fato::Operandos,
            Fato::Resultado,
        ],
    ),
    (
        Consumidor::BackendSExternalCallconv,
        &[Fato::Existencia, Fato::Aridade],
    ),
];

/// Fatos que cada decisor NÃO consulta, com a razão.
///
/// Esta lista é a contraparte obrigatória de [`FATOS_POR_CONSUMIDOR`]:
/// `carve_outs_cobrem_toda_a_matriz` exige que as duas juntas cubram as
/// quarenta células (oito decisores x cinco fatos) exatamente uma vez. Sem ela,
/// “não consulta” entraria calado na prova, e calado é como lacuna vira
/// isenção.
///
/// Uma célula aqui NÃO afirma que a fase é incapaz de decidir o fato: afirma
/// que ela não o pergunta à autoridade, e nomeia o motivo. Onde a fase decide o
/// fato na representação DELA, isso está dito com todas as letras.
const CARVE_OUTS: &[(Consumidor, Fato, &str)] = &[
    (
        Consumidor::Semantic,
        Fato::Familia,
        "a semântica roteia operação interna por grafia, não por família",
    ),
    (
        Consumidor::IrContext,
        Fato::Familia,
        "o contexto de lowering registra assinatura, não roteia por família",
    ),
    (
        Consumidor::IrContext,
        Fato::Aridade,
        "o contexto registra `(retorno, params)`; a aridade do call site é checada pelos validadores",
    ),
    (
        Consumidor::IrContext,
        Fato::Operandos,
        "o laço descarta os parâmetros: esta camada só precisa do retorno",
    ),
    (
        Consumidor::IrModel,
        Fato::Aridade,
        "`ir/model` expõe um predicado de família e nada mais",
    ),
    (Consumidor::IrModel, Fato::Operandos, "idem"),
    (Consumidor::IrModel, Fato::Resultado, "idem"),
    (
        Consumidor::InstrSelectValidate,
        Fato::Aridade,
        "a seleção desvia da checagem de assinatura quando o callee é a pseudo-chamada de \
         ramificação, e para as demais compara retorno, não aridade",
    ),
    (
        Consumidor::InstrSelectValidate,
        Fato::Operandos,
        "os parâmetros são descartados por esta fase; só o retorno entra na tabela",
    ),
    (
        Consumidor::BackendSExternalCallconv,
        Fato::Familia,
        "o emissor do subset montável classifica a pseudo-chamada pela grafia reservada",
    ),
    (
        Consumidor::BackendSExternalCallconv,
        Fato::Operandos,
        "o emissor não tipa operando de operação interna: o subset montável só tem a ramificação",
    ),
    (
        Consumidor::BackendSExternalCallconv,
        Fato::Resultado,
        "idem: o resultado da ramificação é o tipo dos ramos, que a autoridade não fixa",
    ),
];

fn consulta(consumidor: Consumidor, fato: Fato) -> bool {
    FATOS_POR_CONSUMIDOR
        .iter()
        .find(|(declarado, _)| *declarado == consumidor)
        .map(|(_, fatos)| fatos.contains(&fato))
        .expect("consumidor sem linha de fatos")
}

/// Este decisor decide este fato PARA ESTA OPERAÇÃO?
///
/// Saber quais fatos o decisor consulta não basta: a mesma consulta é DECISIVA
/// para umas formas de operação e inerte para outras, e exigir mudança onde a
/// resposta não pode mudar inventaria aresta em vez de medir. Cada condição é
/// lida da entrada canônica corrente; nenhuma linha aqui repete aridade,
/// operando ou resultado.
///
/// O padrão que atravessa quase todas as linhas é o mesmo: contrato relativo ao
/// mapa recebido tem caminho dedicado em cada fase, e é a existência desse
/// desvio — não uma escolha do teste — que separa as formas.
fn decide(
    consumidor: Consumidor,
    fato: Fato,
    dimensao: Dimensao,
    operacao: &InternalOperation,
) -> bool {
    if !consulta(consumidor, fato) {
        return false;
    }
    let generica = operacao.family == InternalOperationFamily::MapaGenerica;
    // Família genérica é rótulo de QUEM emite; o que faz a fase desviar é o
    // contrato DEPENDER do mapa recebido. As duas operações de avanço de cursor
    // são genéricas por emissão e de contrato fixo, e por isso trocar a família
    // delas não move validador nenhum.
    let contrato_relativo_ao_mapa = generica
        && (matches!(operacao.operands, InternalOperands::PapeisDeMapa(_))
            || matches!(
                operacao.result,
                InternalResult::MapaNovoComChave(_) | InternalResult::ValorDoMapaReceptor
            ));
    match (consumidor, fato) {
        // A rotulação “é operação genérica de mapa?” só desvia decisão para as
        // operações cujo contrato é mesmo relativo ao mapa: trocar a família de
        // uma que não é faz a fase entrar no ramo genérico e aceitar assim
        // mesmo. O fato continua coberto em todo o domínio por `ir/model`, que
        // acompanha as trinta e uma.
        (
            Consumidor::IrValidate
            | Consumidor::CfgIrValidate
            | Consumidor::InstrSelectValidate
            | Consumidor::AbstractMachineValidate,
            Fato::Familia,
        ) => contrato_relativo_ao_mapa,
        // `ir/model` É o predicado de família: tirar da autoridade uma operação
        // que já não era genérica não pode mover a resposta dele.
        (Consumidor::IrModel, Fato::Existencia) => generica,
        // O contexto de lowering registra `(retorno, params)` das operações de
        // contrato fixo; a família genérica tem resolução dedicada e não passa
        // por essa tabela.
        (Consumidor::IrContext, _) => !generica && operacao.assinatura_ir().is_some(),
        // A seleção reconhece a pseudo-chamada de ramificação pela grafia
        // reservada, não pela tabela: retirá-la da autoridade não move a fase.
        (Consumidor::InstrSelectValidate, Fato::Existencia) => {
            !matches!(operacao.operands, InternalOperands::Ramificacao)
        }
        // A máquina projeta operando e resultado em classe de valor de pilha, e
        // a família genérica resolve aridade por família, sem olhar operando.
        // O que ela não sabe projetar entra como compatível com tudo: mutação
        // nessa faixa não pode falsificar nada aqui.
        (Consumidor::AbstractMachineValidate, Fato::Operandos) => match dimensao {
            Dimensao::ClasseDeOperando(tipo) => !generica && projetavel_na_pilha(tipo),
            _ => !generica,
        },
        (Consumidor::AbstractMachineValidate, Fato::Resultado) => match dimensao {
            Dimensao::ClasseDoResultado => matches!(
                operacao.result,
                InternalResult::Declarado(tipo) if projetavel_na_pilha(tipo)
            ),
            _ => true,
        },
        // A semântica compara a CLASSE declarada do operando onde a classe
        // declarada é a classe do valor. Na família de leque a pergunta da fase
        // é outra e não é expressável em `TypeIR`: “este valor é um leque COM
        // CARGA?”. Todo leque é `bombom` na IR, então a classe declarada não
        // responde essa pergunta, e não há fato compartilhado a duplicar.
        (Consumidor::Semantic, Fato::Operandos) => {
            operacao.family != InternalOperationFamily::Leque
        }
        // Nos extratores de carga a semântica responde o TIPO DA CARGA
        // declarado no leque do usuário, decidido por `enum_payload` e pela
        // declaração do leque — uma pergunta que a autoridade não faz e não
        // poderia responder, porque ela declara a classe de TRANSPORTE. A
        // resposta da fase é mais fina que a classe declarada, não uma segunda
        // cópia dela.
        (Consumidor::Semantic, Fato::Resultado) => {
            !crate::enum_payload::is_carga_intrinsic(operacao.spelling)
        }
        // O emissor do subset montável só classifica a pseudo-chamada que
        // sobrevive à seleção; nenhuma outra operação interna chega a ele como
        // decisão de contrato.
        (Consumidor::BackendSExternalCallconv, _) => {
            matches!(operacao.operands, InternalOperands::Ramificacao)
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Medição
// ---------------------------------------------------------------------------

struct Aresta {
    consumidor: Consumidor,
    sonda: &'static str,
    dimensao: Dimensao,
    /// O decisor decide este fato para esta operação E alcança a chamada.
    decisoria: bool,
    acompanhou: bool,
}

/// Mede, para um par (operação, fato), o que cada consumidor responde antes e
/// depois da mutação, em cada sonda que materializa a operação.
fn medir(
    corpus: &[Sonda],
    operacao: &InternalOperation,
    fato: Fato,
) -> Result<Vec<Aresta>, &'static str> {
    let spelling = operacao.spelling;
    let variantes = variantes(spelling, fato)?;
    let mut arestas = Vec::new();
    for sonda in corpus {
        if !sonda.testemunha(spelling) {
            continue;
        }
        let antes: Vec<String> = CONSUMIDORES
            .iter()
            .map(|consumidor| consumidor.resposta(sonda, spelling))
            .collect();
        for variante in &variantes {
            let depois: Vec<String> = {
                let _mutado = MutatedContract::install(variante.tabela.clone());
                CONSUMIDORES
                    .iter()
                    .map(|consumidor| consumidor.resposta(sonda, spelling))
                    .collect()
            };
            for (indice, consumidor) in CONSUMIDORES.iter().enumerate() {
                arestas.push(Aresta {
                    consumidor: *consumidor,
                    sonda: sonda.nome,
                    dimensao: variante.dimensao,
                    decisoria: consumidor.alcanca(sonda, spelling)
                        && decide(*consumidor, fato, variante.dimensao, operacao),
                    acompanhou: antes[indice] != depois[indice],
                });
            }
        }
    }
    Ok(arestas)
}

// ---------------------------------------------------------------------------
// Direção permissiva: artefato EXPANDIDO
// ---------------------------------------------------------------------------
//
// A direção restritiva prova dependência quando a autoridade fica mais
// ESTREITA: o artefato canônico deixa de ser legal e quem deriva passa a
// recusar. Ela não vê duas classes de decisão local:
//
//   * a tabela local ADITIVA e concorde, que roda ao lado da autoridade e
//     concorda com ela no estado canônico — o caminho da autoridade continua
//     vivo e ainda move a resposta, então a duplicação fica coberta;
//   * a decisão local numa célula que a fase não pergunta — nada a obriga a
//     acompanhar, e “não pergunta” passa por “não decide”.
//
// As duas só aparecem na direção oposta: a autoridade fica mais LARGA e o
// artefato passa a ser legal. Quem deriva precisa passar a ACEITAR; quem tem
// tabela local própria continua recusando, e a divergência denuncia a tabela.
//
// O artefato expandido é a mesma sonda com o ÚLTIMO operando repetido em cada
// chamada à operação sob prova — exatamente o contrato que a mutação de aridade
// declara. Ele nasce da IR canônica por edição, e as fases seguintes são
// baixadas a partir dele SOB a autoridade mutada, porque é ela que o admite.

/// Artefatos de uma sonda com um operando a mais em cada chamada à operação.
struct Expandida {
    ir: ProgramIR,
    cfg: ProgramCfgIR,
    selecionado: SelectedProgram,
    maquina: MachineProgram,
    /// Quantos sítios de chamada à operação a CFG deste artefato contém.
    ///
    /// O desugaring de `encaixe` e o de `tentar` SINTETIZAM chamadas direto na
    /// CFG: um artefato cuja IR tem só a testemunha pode chegar à CFG com
    /// sítios canônicos de volta, e aí a recusa que a fase devolve é a do sítio
    /// velho, não a da testemunha. Contar é o que separa as duas.
    sitios_cfg: usize,
}

/// Visita toda chamada a `spelling` hospedada neste valor.
///
/// Uma travessia só, duas perguntas: a expansão de aridade edita o nó; a
/// testemunha permissiva o LÊ para nascer de um sítio real. Manter as duas na
/// mesma varredura é o que garante que elas enxerguem exatamente as mesmas
/// formas da IR.
fn visitar_chamadas(valor: &mut ValueIR, spelling: &str, acao: &mut dyn FnMut(&mut ValueIR)) {
    match valor {
        ValueIR::Call { callee, args, .. } => {
            let alvo = callee == spelling;
            for argumento in args.iter_mut() {
                visitar_chamadas(argumento, spelling, acao);
            }
            if alvo {
                acao(valor);
            }
        }
        ValueIR::Unary { operand, .. } => visitar_chamadas(operand, spelling, acao),
        ValueIR::Deref { ptr, .. } => visitar_chamadas(ptr, spelling, acao),
        ValueIR::Binary { lhs, rhs, .. } => {
            visitar_chamadas(lhs, spelling, acao);
            visitar_chamadas(rhs, spelling, acao);
        }
        ValueIR::PointerOffset {
            pointer, offset, ..
        } => {
            visitar_chamadas(pointer, spelling, acao);
            visitar_chamadas(offset, spelling, acao);
        }
        ValueIR::TraitCall { object, args, .. } => {
            visitar_chamadas(object, spelling, acao);
            for argumento in args.iter_mut() {
                visitar_chamadas(argumento, spelling, acao);
            }
        }
        ValueIR::CallIndirect { callee, args, .. } | ValueIR::CallRaw { callee, args, .. } => {
            visitar_chamadas(callee, spelling, acao);
            for argumento in args.iter_mut() {
                visitar_chamadas(argumento, spelling, acao);
            }
        }
        ValueIR::FieldAccess { base, .. } => visitar_chamadas(base, spelling, acao),
        ValueIR::Index { base, index, .. } => {
            visitar_chamadas(base, spelling, acao);
            visitar_chamadas(index, spelling, acao);
        }
        ValueIR::Cast { value, .. }
        | ValueIR::UnionInject { value, .. }
        | ValueIR::UnionTag { value, .. }
        | ValueIR::UnionExtract { value, .. } => visitar_chamadas(value, spelling, acao),
        // As folhas e as formas que não hospedam valor. A COMPLETUDE desta
        // varredura não é presumida: `expandir` conta os sítios de chamada no
        // texto do artefato e exige que todos tenham sido expandidos.
        _ => {}
    }
}

fn visitar_bloco(bloco: &mut BlockIR, spelling: &str, acao: &mut dyn FnMut(&mut ValueIR)) {
    for instrucao in &mut bloco.instructions {
        visitar_instrucao(instrucao, spelling, acao);
    }
}

/// Os valores que esta instrução hospeda DIRETAMENTE, sem entrar em sub-bloco.
///
/// Separar os dois níveis é o que permite achar o bloco MAIS INTERNO que
/// hospeda uma chamada: é lá, e só lá, que os argumentos dela estão garantidos
/// em escopo para uma testemunha nova.
fn valores_da_instrucao(instrucao: &mut InstructionIR) -> Vec<&mut ValueIR> {
    match instrucao {
        InstructionIR::Let { value, .. }
        | InstructionIR::Assign { value, .. }
        | InstructionIR::Expr { value, .. } => vec![value],
        InstructionIR::Return {
            value: Some(valor), ..
        } => vec![valor],
        InstructionIR::StoreIndirect { ptr, value, .. } => vec![ptr, value],
        InstructionIR::StoreFieldIndirect { base, value, .. } => vec![base, value],
        InstructionIR::StoreIndexed {
            base, index, value, ..
        } => vec![base, index, value],
        InstructionIR::If { condition, .. } | InstructionIR::While { condition, .. } => {
            vec![condition]
        }
        InstructionIR::EnumMatch(encaixe) => vec![&mut encaixe.scrutinee],
        InstructionIR::UnionMatch(encaixe) => vec![&mut encaixe.scrutinee],
        // `Falar`, `InlineAsm`, `Break` e `Continue` não hospedam chamada a
        // operação interna nas sondas; a contagem de `expandir` prova isso a
        // cada execução.
        _ => Vec::new(),
    }
}

/// Os blocos aninhados desta instrução.
fn sub_blocos(instrucao: &mut InstructionIR) -> Vec<&mut BlockIR> {
    match instrucao {
        InstructionIR::If {
            then_block,
            else_block,
            ..
        } => match else_block {
            Some(senao) => vec![then_block, senao],
            None => vec![then_block],
        },
        InstructionIR::While { body_block, .. } => vec![body_block],
        InstructionIR::EnumMatch(encaixe) => {
            let mut blocos: Vec<&mut BlockIR> = encaixe
                .arms
                .iter_mut()
                .map(|braco| &mut braco.body)
                .collect();
            if let Some(senao) = &mut encaixe.otherwise {
                blocos.push(senao);
            }
            blocos
        }
        InstructionIR::UnionMatch(encaixe) => encaixe
            .arms
            .iter_mut()
            .map(|braco| &mut braco.body)
            .collect(),
        _ => Vec::new(),
    }
}

fn visitar_instrucao(
    instrucao: &mut InstructionIR,
    spelling: &str,
    acao: &mut dyn FnMut(&mut ValueIR),
) {
    for valor in valores_da_instrucao(instrucao) {
        visitar_chamadas(valor, spelling, acao);
    }
    for bloco in sub_blocos(instrucao) {
        visitar_bloco(bloco, spelling, acao);
    }
}

/// Quantos sítios de chamada a esta grafia o texto deste estágio materializa.
fn sitios_de_chamada(texto: &str, spelling: &str) -> usize {
    texto.matches(&format!("callee: \"{spelling}\"")).count()
}

/// Constrói o artefato expandido, ou diz por que ele não é construtível.
///
/// As fases seguintes são baixadas SOB a autoridade mutada: é ela que admite o
/// operando a mais, e é dela que o artefato precisa ser um programa legal para
/// que a pergunta permissiva faça sentido.
fn expandir(
    sonda: &Sonda,
    spelling: &str,
    tabela: &[InternalOperation],
) -> Result<Expandida, String> {
    let esperadas = sitios_de_chamada(&sonda.textos[IR], spelling);
    if esperadas == 0 {
        return Err("a IR desta sonda não materializa chamada a esta operação".to_string());
    }
    let mut ir = sonda.ir.clone();
    let mut expandidas = 0usize;
    // Repete o último operando de cada chamada — é o que a mutação de aridade
    // declara. Sem operando algum, o operando a mais declarado é `bombom`, e o
    // literal inteiro é o valor dessa classe.
    {
        let mut repetir_ultimo = |chamada: &mut ValueIR| {
            if let ValueIR::Call { args, .. } = chamada {
                match args.last().cloned() {
                    Some(ultimo) => args.push(ultimo),
                    None => args.push(ValueIR::Int(0)),
                }
                expandidas += 1;
            }
        };
        for funcao in &mut ir.functions {
            visitar_bloco(&mut funcao.entry, spelling, &mut repetir_ultimo);
        }
    }
    if expandidas != esperadas {
        return Err(format!(
            "expansão incompleta: {esperadas} sítios de chamada na IR, {expandidas} expandidos — \
             a varredura não alcança alguma forma que hospeda a chamada"
        ));
    }
    let _mutado = MutatedContract::install(tabela.to_vec());
    let cfg = cfg_ir::lower_program(&ir).map_err(|erro| format!("CFG do expandido: {erro}"))?;
    // O artefato expandido só é pergunta legítima se a expansão SOBREVIVER à
    // descida. Onde a própria CFG SINTETIZA chamadas novas à operação — o
    // desugaring de `encaixe` emite leitura de tag e de carga direto na CFG —
    // essas chamadas nascem com a forma que o produtor escolhe, e o artefato
    // fica misto: parte expandida, parte não. Misto não prova nada, e decidir
    // QUANDO emitir é da fase produtora, não fato compartilhado.
    let sitios_cfg = sitios_de_chamada(&format!("{cfg:?}"), spelling);
    if sitios_cfg != esperadas {
        return Err(format!(
            "a CFG sintetiza chamadas a esta operação: {esperadas} sítios na IR, {sitios_cfg} na \
             CFG — o artefato expandido não é construtível de ponta a ponta"
        ));
    }
    let selecionado = instr_select::lower_program(&cfg)
        .map_err(|erro| format!("seleção do expandido: {erro}"))?;
    let maquina = abstract_machine::lower_program(&selecionado)
        .map_err(|erro| format!("máquina do expandido: {erro}"))?;
    Ok(Expandida {
        ir,
        cfg,
        selecionado,
        maquina,
        sitios_cfg,
    })
}

/// Expansão CIRÚRGICA por estágio.
///
/// Onde a descida não constrói o artefato expandido de ponta a ponta, a
/// pergunta permissiva ainda existe para o validador daquele estágio: o
/// contrato que ele confere é o do artefato que ele recebe. A aridade da
/// ramificação é a forma dela, e por isso a CFG recusa um `__ternario` de
/// quatro operandos antes de qualquer contrato — mas quem valida a CFG e a
/// seleção continua tendo de responder se a aridade que exige vem da autoridade
/// ou de uma tabela própria.
///
/// A cirurgia repete o último operando de cada chamada à operação, no artefato
/// daquele estágio, e conta os sítios para não presumir completude.
fn expandir_cfg(cfg: &ProgramCfgIR, spelling: &str) -> Option<ProgramCfgIR> {
    let esperadas = sitios_de_chamada(&format!("{cfg:?}"), spelling);
    if esperadas == 0 {
        return None;
    }
    let mut expandido = cfg.clone();
    let mut expandidas = 0usize;
    for funcao in &mut expandido.functions {
        for bloco in &mut funcao.blocks {
            for instrucao in &mut bloco.instructions {
                if let cfg_ir::InstructionCfgIR::Call { callee, args, .. } = instrucao {
                    if callee == spelling {
                        match args.last().cloned() {
                            Some(ultimo) => args.push(ultimo),
                            None => return None,
                        }
                        expandidas += 1;
                    }
                }
            }
        }
    }
    (expandidas == esperadas).then_some(expandido)
}

fn expandir_sel(selecionado: &SelectedProgram, spelling: &str) -> Option<SelectedProgram> {
    let esperadas = sitios_de_chamada(&format!("{selecionado:?}"), spelling);
    if esperadas == 0 {
        return None;
    }
    let mut expandido = selecionado.clone();
    let mut expandidas = 0usize;
    for funcao in &mut expandido.functions {
        for bloco in &mut funcao.blocks {
            for instrucao in &mut bloco.instructions {
                let alvo = match instrucao {
                    instr_select::SelectedInstr::Call { callee, args, .. }
                    | instr_select::SelectedInstr::CallVoid { callee, args, .. }
                        if callee == spelling =>
                    {
                        Some(args)
                    }
                    _ => None,
                };
                if let Some(args) = alvo {
                    match args.last().cloned() {
                        Some(ultimo) => args.push(ultimo),
                        None => return None,
                    }
                    expandidas += 1;
                }
            }
        }
    }
    (expandidas == esperadas).then_some(expandido)
}

impl Consumidor {
    /// A resposta deste consumidor sobre o artefato EXPANDIDO por cirurgia no
    /// estágio que ele examina, quando a descida não constrói o artefato
    /// inteiro.
    fn resposta_expandida_no_estagio(self, sonda: &Sonda, spelling: &str) -> Option<String> {
        let responder = |resposta: String| Some(resposta);
        match self {
            Self::CfgIrValidate => {
                let cfg = expandir_cfg(&sonda.cfg, spelling)?;
                responder(veredito(cfg_ir_validate::validate_program(&cfg)))
            }
            Self::InstrSelectValidate => {
                let selecionado = expandir_sel(&sonda.selecionado, spelling)?;
                responder(veredito(instr_select_validate::validate_program(
                    &selecionado,
                )))
            }
            Self::BackendSExternalCallconv => {
                let selecionado = expandir_sel(&sonda.selecionado, spelling)?;
                responder(veredito(crate::backend_s::emit_external_toolchain_subset(
                    &selecionado,
                )))
            }
            // A IR e a máquina não recebem cirurgia: a IR expandida já é o
            // caminho principal, e mexer em `argc` na máquina sem mexer nos
            // empilhamentos produziria recusa por pilha, não por contrato.
            _ => None,
        }
    }

    /// A resposta deste consumidor sobre o artefato EXPANDIDO.
    ///
    /// `None` quando o consumidor não examina artefato onde a expansão existe:
    /// a semântica e o contexto de lowering leem a AST, onde a chamada interna
    /// ainda não tem a forma expandida, e `ir/model` é um predicado sobre a
    /// grafia, que operando nenhum move.
    fn resposta_expandida(self, expandida: &Expandida) -> Option<String> {
        let resposta = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match self {
            Self::Semantic | Self::IrContext | Self::IrModel => None,
            Self::IrValidate => Some(veredito(ir_validate::validate_program(&expandida.ir))),
            Self::CfgIrValidate => {
                Some(veredito(cfg_ir_validate::validate_program(&expandida.cfg)))
            }
            Self::InstrSelectValidate => Some(veredito(instr_select_validate::validate_program(
                &expandida.selecionado,
            ))),
            Self::AbstractMachineValidate => Some(veredito(
                abstract_machine_validate::validate_program(&expandida.maquina),
            )),
            Self::BackendSExternalCallconv => Some(veredito(
                crate::backend_s::emit_external_toolchain_subset(&expandida.selecionado),
            )),
        }));
        match resposta {
            Ok(resposta) => resposta,
            Err(_) => Some("INTERROMPE".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Direção permissiva geral: testemunha NOVA, conforme ao contrato mutado
// ---------------------------------------------------------------------------
//
// A expansão de aridade prova a direção permissiva para F2 mexendo nos sítios
// que a sonda já materializa. Para F1, F3 e F4 a cirurgia é outra: a mutação
// não acrescenta operando, TROCA a classe exigida de um operando ou de um
// resultado, e o artefato que ela admite é um sítio de chamada NOVO — um que o
// contrato canônico RECUSA e o mutado ACEITA.
//
// ```text
//                A0 (canônica)      A1 (mutada)
//   testemunha   RECUSA             ACEITA
// ```
//
// É esse quadrante que a direção restritiva não alcança. Uma cópia local
// ADITIVA e concorde — a fase pergunta à autoridade E confere de novo a regra
// antiga por conta própria — continua RECUSANDO sob A1, e a divergência
// denuncia a cópia. Trocar a classe X por Y retira X e admite Y: a mutação não
// precisa ser superconjunto global de A0 para que a testemunha seja nova.
//
// A testemunha nasce do contrato MUTADO, que por sua vez nasceu da entrada
// canônica corrente por cópia e edição de uma dimensão. Nenhuma classe é
// escrita aqui: `args_da_variante` e `resultado_da_variante` LEEM a variante.

/// O sítio de chamada que serve de base para a testemunha.
struct Sitio {
    args: Vec<ValueIR>,
    ret_type: TypeIR,
    identidade: crate::intrinsics::identity::CalleeIdentity,
}

/// Um literal desta classe, quando a classe é literalizável.
///
/// Só as classes escalares de transporte têm literal na IR; um mapa, uma lista
/// ou um handle opaco precisam de um valor já em escopo, e por isso a
/// testemunha reaproveita o argumento do sítio base quando a classe não muda.
fn literal(tipo: TypeIR) -> Option<ValueIR> {
    match tipo {
        TypeIR::Bombom => Some(ValueIR::Int(0)),
        TypeIR::Verso => Some(ValueIR::String("testemunha".to_string())),
        TypeIR::Logica => Some(ValueIR::Bool(true)),
        _ => None,
    }
}

/// Os argumentos que o contrato MUTADO exige, a partir do sítio base.
///
/// Onde a variante declara a MESMA classe que a canônica naquela posição, o
/// argumento do sítio base é reaproveitado: ele já conforma, e reaproveitar é o
/// que mantém a testemunha a um passo do programa real. Onde a classe MUDOU, é
/// preciso um valor da classe nova — e é só aí que a testemunha difere.
fn args_da_variante(
    canonica: &InternalOperation,
    mutada: &InternalOperation,
    base: &[ValueIR],
) -> Option<Vec<ValueIR>> {
    match mutada.operands {
        InternalOperands::Declarados(novos) => {
            let velhos = canonica.declared_params();
            novos
                .iter()
                .enumerate()
                .map(
                    |(indice, classe)| match velhos.and_then(|v| v.get(indice)) {
                        Some(velho) if velho == classe => base.get(indice).cloned(),
                        _ => literal(*classe),
                    },
                )
                .collect()
        }
        // Papéis e ramificação não declaram classe: a testemunha nova, nessas
        // formas, é a do resultado, e os operandos do sítio base continuam
        // valendo.
        InternalOperands::PapeisDeMapa(_) | InternalOperands::Ramificacao => {
            (base.len() == mutada.arity()).then(|| base.to_vec())
        }
    }
}

/// A classe de resultado que o contrato MUTADO declara, na forma concreta que a
/// IR anota no sítio de chamada.
///
/// `ret_canonico` é o resultado que o sítio base carrega. Ele entra só onde a
/// autoridade declara resultado RELATIVO — valor do mapa recebido, tipo dos
/// ramos —, porque aí a classe concreta é derivada pela fase e a testemunha não
/// pode inventá-la.
fn resultado_da_variante(mutada: &InternalOperation, ret_canonico: TypeIR) -> Option<TypeIR> {
    match mutada.result {
        InternalResult::Declarado(tipo) => Some(tipo),
        InternalResult::MapaNovoComChave(chave) => {
            let valor = match ret_canonico {
                TypeIR::Map { value, .. } => value,
                _ => crate::ir::MapValueIR::Bombom,
            };
            Some(TypeIR::Map {
                key: chave,
                value: valor,
            })
        }
        InternalResult::ValorDoMapaReceptor | InternalResult::TipoDosRamos => Some(ret_canonico),
    }
}

/// Caminho até um bloco aninhado: para cada nível, a instrução e o sub-bloco.
type CaminhoDeBloco = Vec<(usize, usize)>;

/// O bloco MAIS INTERNO que hospeda uma chamada DIRETA à operação.
///
/// É lá, e só lá, que os argumentos do sítio estão garantidos em escopo para
/// uma testemunha nova: um binding de bloco aninhado não é legível de fora.
fn bloco_do_primeiro_sitio(bloco: &mut BlockIR, spelling: &str) -> Option<CaminhoDeBloco> {
    for (indice, instrucao) in bloco.instructions.iter_mut().enumerate() {
        for (sub, aninhado) in sub_blocos(instrucao).into_iter().enumerate() {
            if let Some(mut caminho) = bloco_do_primeiro_sitio(aninhado, spelling) {
                caminho.insert(0, (indice, sub));
                return Some(caminho);
            }
        }
    }
    let hospeda = bloco.instructions.iter_mut().any(|instrucao| {
        let mut achou = false;
        for valor in valores_da_instrucao(instrucao) {
            visitar_chamadas(valor, spelling, &mut |_| achou = true);
        }
        achou
    });
    hospeda.then(CaminhoDeBloco::new)
}

/// Navega o caminho até o bloco.
fn bloco_em<'a>(raiz: &'a mut BlockIR, caminho: &CaminhoDeBloco) -> &'a mut BlockIR {
    let mut atual = raiz;
    for (indice, sub) in caminho {
        atual = sub_blocos(&mut atual.instructions[*indice])
            .into_iter()
            .nth(*sub)
            .expect("sub-bloco do caminho");
    }
    atual
}

/// O primeiro sítio de chamada à operação, para servir de base à testemunha.
fn primeiro_sitio(funcao: &mut crate::ir::FunctionIR, spelling: &str) -> Option<Sitio> {
    let mut sitio = None;
    visitar_bloco(&mut funcao.entry, spelling, &mut |chamada| {
        if sitio.is_none() {
            if let ValueIR::Call {
                args,
                ret_type,
                identidade,
                ..
            } = chamada
            {
                sitio = Some(Sitio {
                    args: args.clone(),
                    ret_type: *ret_type,
                    identidade: *identidade,
                });
            }
        }
    });
    sitio
}

/// Apaga da IR toda chamada à operação, preservando a forma do programa.
///
/// É o passo que torna a testemunha a ÚNICA pergunta sobre esta operação no
/// artefato. Sem ele, os sítios canônicos — legais sob a autoridade real e
/// ilegais sob a mutada — recusariam o programa inteiro sob `A1`, e o oráculo
/// leria a recusa do sítio velho como se fosse a da testemunha.
///
/// A neutralização só SUBSTITUI, nunca remove: índice de instrução estável é o
/// que permite inserir a testemunha no caminho medido antes.
fn neutralizar_sitios(
    funcao: &mut crate::ir::FunctionIR,
    spelling: &str,
) -> Result<(), &'static str> {
    // Um sítio cuja classe de retorno não tem literal só pode ser neutralizado
    // se ninguém ler o slot que ele alimenta: aí o slot inteiro passa a
    // `bombom` e a chamada vira um literal dessa classe.
    let mut sem_literal: Vec<String> = Vec::new();
    lets_da_operacao(&mut funcao.entry, spelling, &mut sem_literal);
    let texto = format!("{funcao:?}");
    for slot in &sem_literal {
        if texto.contains(&format!("Local(\"{slot}\")")) {
            return Err(
                "o sítio canônico devolve classe sem literal para um slot que o programa lê: \
                 a testemunha não seria a única pergunta sobre a operação neste artefato",
            );
        }
    }
    for local in funcao.locals.iter_mut() {
        if sem_literal.contains(&local.slot) {
            local.ty = TypeIR::Bombom;
            local.resolved = None;
        }
    }
    visitar_bloco(&mut funcao.entry, spelling, &mut |chamada| {
        if let ValueIR::Call { ret_type, .. } = chamada {
            // Sem literal da classe, o valor vira `bombom`: ou o slot já foi
            // retipado acima, ou a instrução é um `Expr` que descarta o valor.
            *chamada = literal(*ret_type).unwrap_or(ValueIR::Int(0));
        }
    });
    Ok(())
}

/// Slots dos `Let` cujo valor INTEIRO é uma chamada à operação sem literal de
/// retorno.
fn lets_da_operacao(bloco: &mut BlockIR, spelling: &str, achados: &mut Vec<String>) {
    for instrucao in &mut bloco.instructions {
        if let InstructionIR::Let {
            slot,
            value: ValueIR::Call {
                callee, ret_type, ..
            },
            ..
        } = instrucao
        {
            if callee == spelling && literal(*ret_type).is_none() {
                achados.push(slot.clone());
            }
        }
        for aninhado in sub_blocos(instrucao) {
            lets_da_operacao(aninhado, spelling, achados);
        }
    }
}

fn span_sintetico() -> crate::token::Span {
    crate::token::Span::new(
        crate::token::Position::new(1, 1),
        crate::token::Position::new(1, 1),
    )
}

/// Slot da testemunha. Nome impossível na fonte, como todo temporário de fase.
const TESTEMUNHA_SLOT: &str = "%u01testemunha#0";

/// A instrução que hospeda a testemunha, e o local que ela declara.
fn instrucao_da_testemunha(
    spelling: &str,
    args: Vec<ValueIR>,
    ret: TypeIR,
    identidade: crate::intrinsics::identity::CalleeIdentity,
) -> (InstructionIR, Option<crate::ir::LocalIR>) {
    let chamada = ValueIR::Call {
        callee: spelling.to_string(),
        args,
        ret_type: ret,
        identidade,
    };
    if ret == TypeIR::Nulo {
        return (
            InstructionIR::Expr {
                value: chamada,
                span: span_sintetico(),
            },
            None,
        );
    }
    (
        InstructionIR::Let {
            slot: TESTEMUNHA_SLOT.to_string(),
            value: chamada,
            span: span_sintetico(),
        },
        Some(crate::ir::LocalIR {
            source_name: TESTEMUNHA_SLOT.to_string(),
            slot: TESTEMUNHA_SLOT.to_string(),
            ty: ret,
            resolved: None,
            is_mut: false,
        }),
    )
}

/// Insere a instrução no fim do bloco, antes de um terminador.
fn inserir_no_fim(bloco: &mut BlockIR, instrucao: InstructionIR) {
    let posicao = bloco
        .instructions
        .iter()
        .position(|existente| {
            matches!(
                existente,
                InstructionIR::Return { .. }
                    | InstructionIR::Break { .. }
                    | InstructionIR::Continue { .. }
            )
        })
        .unwrap_or(bloco.instructions.len());
    bloco.instructions.insert(posicao, instrucao);
}

/// A testemunha permissiva desta variante, na IR, com as fases seguintes
/// baixadas SOB a autoridade mutada.
///
/// O artefato é UM: as duas leituras — sob a canônica e sob a mutada —
/// perguntam ao mesmo programa. É a autoridade que muda, nunca o artefato.
fn testemunha_ir(
    sonda: &Sonda,
    canonica: &InternalOperation,
    variante: &Variante,
) -> Result<Expandida, String> {
    let spelling = canonica.spelling;
    let mutada = variante
        .tabela
        .iter()
        .find(|operacao| operacao.spelling == spelling)
        .ok_or_else(|| "a variante retira a operação: não há sítio a construir".to_string())?;
    let mut ir = sonda.ir.clone();
    let mut inseriu: Option<usize> = None;
    for (indice, funcao) in ir.functions.iter_mut().enumerate() {
        let Some(caminho) = bloco_do_primeiro_sitio(&mut funcao.entry, spelling) else {
            continue;
        };
        let sitio = primeiro_sitio(funcao, spelling).expect("sítio achado pelo caminho");
        let args = args_da_variante(canonica, mutada, &sitio.args)
            .ok_or_else(|| "os operandos do contrato mutado não são construtíveis".to_string())?;
        let ret = resultado_da_variante(mutada, sitio.ret_type)
            .ok_or_else(|| "o resultado do contrato mutado não é construtível".to_string())?;
        neutralizar_sitios(funcao, spelling).map_err(str::to_string)?;
        let (instrucao, local) = instrucao_da_testemunha(spelling, args, ret, sitio.identidade);
        inserir_no_fim(bloco_em(&mut funcao.entry, &caminho), instrucao);
        if let Some(local) = local {
            funcao.locals.push(local);
        }
        inseriu = Some(indice);
        break;
    }
    let Some(hospedeira) = inseriu else {
        return Err("nenhum sítio de chamada na IR desta sonda".to_string());
    };
    // As demais funções da sonda também podem hospedar sítios canônicos, e um
    // só deles recusaria o programa inteiro sob a autoridade mutada.
    for (indice, funcao) in ir.functions.iter_mut().enumerate() {
        if indice != hospedeira {
            neutralizar_sitios(funcao, spelling).map_err(str::to_string)?;
        }
    }
    let _mutado = MutatedContract::install(variante.tabela.clone());
    let cfg = cfg_ir::lower_program(&ir).map_err(|erro| format!("CFG da testemunha: {erro}"))?;
    let selecionado = instr_select::lower_program(&cfg)
        .map_err(|erro| format!("seleção da testemunha: {erro}"))?;
    let maquina = abstract_machine::lower_program(&selecionado)
        .map_err(|erro| format!("máquina da testemunha: {erro}"))?;
    let sitios_cfg = sitios_de_chamada(&format!("{cfg:?}"), spelling);
    Ok(Expandida {
        ir,
        cfg,
        selecionado,
        maquina,
        sitios_cfg,
    })
}

/// A testemunha permissiva MÍNIMA: um programa cuja única instrução é a chamada.
///
/// A sonda real é a primeira escolha, porque ela é o programa que o compilador
/// de verdade produz. Mas duas coisas a inviabilizam: a CFG SINTETIZA chamadas
/// que a IR não tem — o desugaring de `encaixe` e o de `tentar` emitem leitura
/// de tag e de carga direto lá —, e um sítio canônico cuja classe de retorno
/// não tem literal não é neutralizável. Nos dois casos o artefato deixaria de
/// ter a testemunha como única pergunta sobre a operação.
///
/// O programa mínimo não tem esse problema porque não tem mais nada. Ele
/// reaproveita as tabelas de identidade da sonda — tipos resolvidos, uniões,
/// variantes de leque — e troca o corpo da função de entrada pela testemunha.
/// Só existe onde todos os operandos do contrato mutado são literalizáveis: é a
/// forma de dizer que a testemunha não inventa valor que a representação não
/// tem.
fn testemunha_minima(
    sonda: &Sonda,
    canonica: &InternalOperation,
    variante: &Variante,
) -> Result<Expandida, String> {
    let spelling = canonica.spelling;
    let mutada = variante
        .tabela
        .iter()
        .find(|operacao| operacao.spelling == spelling)
        .ok_or_else(|| "a variante retira a operação: não há sítio a construir".to_string())?;
    let params = mutada
        .declared_params()
        .ok_or_else(|| "o contrato mutado declara papéis, não classes: sem literal".to_string())?;
    let args: Option<Vec<ValueIR>> = params.iter().map(|classe| literal(*classe)).collect();
    let args = args.ok_or_else(|| {
        "algum operando do contrato mutado é de classe sem literal na IR".to_string()
    })?;
    let ret = match mutada.result {
        InternalResult::Declarado(tipo) => tipo,
        _ => {
            return Err(
                "o contrato mutado declara resultado relativo: sem sítio mínimo".to_string(),
            )
        }
    };
    let mut ir = sonda.ir.clone();
    let indice = ir
        .functions
        .iter()
        .position(|funcao| funcao.params.is_empty() && funcao.ret_type == TypeIR::Bombom)
        .ok_or_else(|| "a sonda não tem função de entrada sem parâmetros".to_string())?;
    let mut entrada = ir.functions[indice].clone();
    let (instrucao, local) =
        instrucao_da_testemunha(spelling, args, ret, CalleeIdentity::CompilerInternal);
    entrada.locals = local.into_iter().collect();
    entrada.entry.instructions = vec![
        instrucao,
        InstructionIR::Return {
            value: Some(ValueIR::Int(0)),
            span: span_sintetico(),
        },
    ];
    ir.functions = vec![entrada];
    let _mutado = MutatedContract::install(variante.tabela.clone());
    let cfg = cfg_ir::lower_program(&ir).map_err(|erro| format!("CFG da testemunha: {erro}"))?;
    let selecionado = instr_select::lower_program(&cfg)
        .map_err(|erro| format!("seleção da testemunha: {erro}"))?;
    let maquina = abstract_machine::lower_program(&selecionado)
        .map_err(|erro| format!("máquina da testemunha: {erro}"))?;
    let sitios_cfg = sitios_de_chamada(&format!("{cfg:?}"), spelling);
    Ok(Expandida {
        ir,
        cfg,
        selecionado,
        maquina,
        sitios_cfg,
    })
}

// ---------------------------------------------------------------------------
// A testemunha permissiva na AST
// ---------------------------------------------------------------------------
//
// A semântica e o contexto de lowering examinam a AST, e é lá que a decisão
// deles acontece: os desugarings de `encaixe`, de `tentar` e de `para cada` já
// materializaram a chamada interna quando a checagem semântica roda. Uma
// testemunha só de IR nunca chega a eles — e foi exatamente em `semantic/calls`
// que a duplicação de F3/F4 do `B5` viveu até o sexto HEAD. Sem esta metade a
// prova permissiva deixaria de fora o consumidor mais importante.
//
// A cirurgia é a mesma da IR: neutralizar todo sítio canônico, para que a
// testemunha seja a ÚNICA pergunta sobre a operação, e inserir um sítio novo
// que conforma ao contrato MUTADO.

/// As expressões que esta instrução hospeda DIRETAMENTE, sem entrar em bloco.
fn expressoes_do_stmt(stmt: &mut Stmt) -> Vec<&mut Expr> {
    match stmt {
        Stmt::Let(declaracao) => vec![&mut declaracao.init],
        Stmt::Return(retorno) => retorno.expr.iter_mut().collect(),
        Stmt::Assign(atribuicao) => vec![&mut atribuicao.expr],
        // A cadeia `senão se` é UM `Stmt::If` com outro `IfStmt` dentro do
        // `else`: parar no primeiro deixaria de fora toda condição a partir da
        // segunda — e é lá que o desugaring de `tentar` põe as leituras de tag.
        Stmt::If(condicional) => {
            let mut condicoes = Vec::new();
            let mut atual = condicional;
            loop {
                condicoes.push(&mut atual.condition);
                match &mut atual.else_branch {
                    Some(ElseBlock::If(aninhado)) => atual = aninhado,
                    Some(ElseBlock::Block(_)) | None => break,
                }
            }
            condicoes
        }
        Stmt::While(laco) => vec![&mut laco.condition],
        Stmt::Falar(falar) => falar.args.iter_mut().collect(),
        Stmt::EnumMatch(encaixe) => vec![&mut encaixe.scrutinee],
        Stmt::UnionMatch(encaixe) => vec![&mut encaixe.scrutinee],
        Stmt::Expr(expressao) => vec![expressao],
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::InlineAsm(_) => Vec::new(),
    }
}

/// Os blocos aninhados desta instrução.
fn blocos_do_stmt(stmt: &mut Stmt) -> Vec<&mut Block> {
    match stmt {
        Stmt::If(condicional) => {
            let mut blocos = Vec::new();
            let mut atual = condicional;
            loop {
                let crate::ast::IfStmt {
                    then_branch,
                    else_branch,
                    ..
                } = atual;
                blocos.push(then_branch);
                match else_branch {
                    Some(ElseBlock::Block(bloco)) => {
                        blocos.push(bloco);
                        break;
                    }
                    Some(ElseBlock::If(aninhado)) => atual = aninhado,
                    None => break,
                }
            }
            blocos
        }
        Stmt::While(laco) => vec![&mut laco.body],
        Stmt::EnumMatch(encaixe) => {
            let mut blocos: Vec<&mut Block> = encaixe
                .arms
                .iter_mut()
                .map(|braco| &mut braco.body)
                .collect();
            if let Some(senao) = &mut encaixe.otherwise {
                blocos.push(senao);
            }
            blocos
        }
        Stmt::UnionMatch(encaixe) => encaixe
            .arms
            .iter_mut()
            .map(|braco| &mut braco.body)
            .collect(),
        _ => Vec::new(),
    }
}

/// Visita toda chamada a `spelling` hospedada nesta expressão.
fn visitar_chamadas_ast(expr: &mut Expr, spelling: &str, acao: &mut dyn FnMut(&mut Expr)) {
    let alvo = matches!(
        &expr.kind,
        ExprKind::Call(callee, _)
            if matches!(&callee.kind, ExprKind::Ident(nome) if nome == spelling)
    );
    match &mut expr.kind {
        ExprKind::Call(callee, args) => {
            visitar_chamadas_ast(callee, spelling, acao);
            for argumento in args.iter_mut() {
                visitar_chamadas_ast(argumento, spelling, acao);
            }
        }
        ExprKind::Binary(lhs, _, rhs) => {
            visitar_chamadas_ast(lhs, spelling, acao);
            visitar_chamadas_ast(rhs, spelling, acao);
        }
        ExprKind::Unary(_, operando)
        | ExprKind::AddressOf(operando)
        | ExprKind::InternalMapIterCreate(operando)
        | ExprKind::InternalMapIterNextKey(operando)
        | ExprKind::Cast { expr: operando, .. } => visitar_chamadas_ast(operando, spelling, acao),
        ExprKind::FieldAccess { base, .. } => visitar_chamadas_ast(base, spelling, acao),
        ExprKind::Index { base, index } => {
            visitar_chamadas_ast(base, spelling, acao);
            visitar_chamadas_ast(index, spelling, acao);
        }
        _ => {}
    }
    if alvo {
        acao(expr);
    }
}

fn visitar_bloco_ast(bloco: &mut Block, spelling: &str, acao: &mut dyn FnMut(&mut Expr)) {
    for stmt in &mut bloco.stmts {
        for expressao in expressoes_do_stmt(stmt) {
            visitar_chamadas_ast(expressao, spelling, acao);
        }
        for aninhado in blocos_do_stmt(stmt) {
            visitar_bloco_ast(aninhado, spelling, acao);
        }
    }
}

/// O bloco MAIS INTERNO que hospeda uma chamada DIRETA à operação.
fn bloco_do_primeiro_sitio_ast(bloco: &mut Block, spelling: &str) -> Option<CaminhoDeBloco> {
    for (indice, stmt) in bloco.stmts.iter_mut().enumerate() {
        for (sub, aninhado) in blocos_do_stmt(stmt).into_iter().enumerate() {
            if let Some(mut caminho) = bloco_do_primeiro_sitio_ast(aninhado, spelling) {
                caminho.insert(0, (indice, sub));
                return Some(caminho);
            }
        }
    }
    let hospeda = bloco.stmts.iter_mut().any(|stmt| {
        let mut achou = false;
        for expressao in expressoes_do_stmt(stmt) {
            visitar_chamadas_ast(expressao, spelling, &mut |_| achou = true);
        }
        achou
    });
    hospeda.then(CaminhoDeBloco::new)
}

fn bloco_ast_em<'a>(raiz: &'a mut Block, caminho: &CaminhoDeBloco) -> &'a mut Block {
    let mut atual = raiz;
    for (indice, sub) in caminho {
        atual = blocos_do_stmt(&mut atual.stmts[*indice])
            .into_iter()
            .nth(*sub)
            .expect("sub-bloco do caminho");
    }
    atual
}

/// Um literal desta classe na AST, quando a classe é literalizável.
fn literal_ast(tipo: TypeIR, span: crate::token::Span) -> Option<Expr> {
    let kind = match tipo {
        TypeIR::Bombom => ExprKind::IntLit(0),
        TypeIR::Verso => ExprKind::StringLit("testemunha".to_string()),
        TypeIR::Logica => ExprKind::BoolLit(true),
        _ => return None,
    };
    Some(Expr { kind, span })
}

/// A classe na representação `Type` desta fase, quando ela existe.
///
/// É correspondência de REPRESENTAÇÃO, e mora no teste pela mesma razão que
/// mora na fase: a testemunha precisa ser escrita na linguagem da AST. QUAL
/// classe a operação exige continua vindo só da autoridade.
fn tipo_ast(tipo: TypeIR, span: crate::token::Span) -> Option<crate::ast::Type> {
    use crate::ast::Type;
    Some(match tipo {
        TypeIR::Bombom => Type::Bombom(span),
        TypeIR::Verso => Type::Verso(span),
        TypeIR::Logica => Type::Logica(span),
        _ => return None,
    })
}

/// Os argumentos que o contrato MUTADO exige, na AST, a partir do sítio base.
fn args_ast_da_variante(
    canonica: &InternalOperation,
    mutada: &InternalOperation,
    base: &[Expr],
    span: crate::token::Span,
) -> Option<Vec<Expr>> {
    match mutada.operands {
        InternalOperands::Declarados(novos) => {
            let velhos = canonica.declared_params();
            novos
                .iter()
                .enumerate()
                .map(
                    |(indice, classe)| match velhos.and_then(|v| v.get(indice)) {
                        Some(velho) if velho == classe => base.get(indice).cloned(),
                        _ => literal_ast(*classe, span),
                    },
                )
                .collect()
        }
        InternalOperands::PapeisDeMapa(_) | InternalOperands::Ramificacao => {
            (base.len() == mutada.arity()).then(|| base.to_vec())
        }
    }
}

/// Nome do slot da testemunha na AST. Impossível na fonte, como os temporários
/// que os próprios desugarings fabricam.
const TESTEMUNHA_NOME: &str = "__u01_testemunha";

/// A testemunha permissiva desta variante, na AST.
fn testemunha_ast(
    sonda: &Sonda,
    canonica: &InternalOperation,
    variante: &Variante,
) -> Result<Program, String> {
    use crate::ast::{Item, LetStmt};
    let spelling = canonica.spelling;
    let mutada = variante
        .tabela
        .iter()
        .find(|operacao| operacao.spelling == spelling)
        .ok_or_else(|| "a variante retira a operação: não há sítio a construir".to_string())?;
    let InternalResult::Declarado(ret) = mutada.result else {
        return Err("o contrato mutado declara resultado relativo: sem sítio na AST".to_string());
    };
    let InternalResult::Declarado(canonico) = canonica.result else {
        return Err("o contrato canônico declara resultado relativo: sem sítio na AST".to_string());
    };
    let mut programa = sonda.programa.clone();
    let mut alvo: Option<(usize, CaminhoDeBloco, Vec<Expr>, crate::token::Span)> = None;
    for (indice, item) in programa.items.iter_mut().enumerate() {
        let Item::Function(funcao) = item else {
            continue;
        };
        let Some(caminho) = bloco_do_primeiro_sitio_ast(&mut funcao.body, spelling) else {
            continue;
        };
        let mut base: Option<Vec<Expr>> = None;
        visitar_bloco_ast(&mut funcao.body, spelling, &mut |chamada| {
            if base.is_none() {
                if let ExprKind::Call(_, args) = &chamada.kind {
                    base = Some(args.clone());
                }
            }
        });
        alvo = Some((
            indice,
            caminho,
            base.expect("sítio achado pelo caminho"),
            funcao.span,
        ));
        break;
    }
    let Some((indice, caminho, base, span)) = alvo else {
        return Err("nenhum sítio de chamada na AST desta sonda".to_string());
    };
    let args = args_ast_da_variante(canonica, mutada, &base, span)
        .ok_or_else(|| "os operandos do contrato mutado não são construtíveis".to_string())?;
    let tipo = tipo_ast(ret, span)
        .ok_or_else(|| "a classe do resultado mutado não é escrevível na AST".to_string())?;
    let neutro = literal_ast(canonico, span).ok_or_else(|| {
        "a classe do resultado canônico não tem literal: o sítio canônico não é neutralizável"
            .to_string()
    })?;
    for item in &mut programa.items {
        if let Item::Function(funcao) = item {
            visitar_bloco_ast(&mut funcao.body, spelling, &mut |chamada| {
                *chamada = neutro.clone()
            });
        }
    }
    let Item::Function(funcao) = &mut programa.items[indice] else {
        unreachable!("o alvo foi achado numa função")
    };
    let chamada = Expr {
        kind: ExprKind::Call(
            Box::new(Expr {
                kind: ExprKind::Ident(spelling.to_string()),
                span,
            }),
            args,
        ),
        span,
    };
    let testemunha = Stmt::Let(LetStmt {
        name: TESTEMUNHA_NOME.to_string(),
        is_mut: false,
        ty: Some(tipo),
        init: chamada,
        span,
    });
    let bloco = bloco_ast_em(&mut funcao.body, &caminho);
    let posicao = bloco
        .stmts
        .iter()
        .position(|stmt| matches!(stmt, Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_)))
        .unwrap_or(bloco.stmts.len());
    bloco.stmts.insert(posicao, testemunha);
    Ok(programa)
}

/// Um artefato-testemunha, na representação que o consumidor examina.
enum Artefato {
    /// Árvore de sintaxe: a representação da semântica e do contexto de
    /// lowering.
    Arvore(Program),
    /// IR e as fases baixadas a partir dela sob a autoridade mutada.
    Baixado(Expandida),
}

impl Consumidor {
    /// A resposta deste consumidor à testemunha, ou `None` quando ele não
    /// examina a representação em que ela existe.
    fn resposta_na_testemunha(self, artefato: &Artefato) -> Option<String> {
        match artefato {
            Artefato::Arvore(programa) => {
                let resposta =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match self {
                        Self::Semantic => Some(veredito(semantic::check_program(programa))),
                        Self::IrContext => Some(match ir::lower_program(programa) {
                            Ok(_) => "ACEITA".to_string(),
                            Err(erro) => format!("RECUSA: {erro}"),
                        }),
                        _ => None,
                    }));
                match resposta {
                    Ok(resposta) => resposta,
                    Err(_) => Some("INTERROMPE".to_string()),
                }
            }
            Artefato::Baixado(expandida) => {
                // O artefato daquele estágio está CONTAMINADO quando a descida
                // sintetiza sítios canônicos de volta: ler a recusa do sítio
                // velho como se fosse a da testemunha é o erro que o §6 da
                // disposição proíbe.
                let depois_da_ir = matches!(
                    self,
                    Self::CfgIrValidate
                        | Self::InstrSelectValidate
                        | Self::AbstractMachineValidate
                        | Self::BackendSExternalCallconv
                );
                if depois_da_ir && expandida.sitios_cfg != 1 {
                    return None;
                }
                self.resposta_expandida(expandida)
            }
        }
    }
}

/// A variante é coerente consigo mesma?
///
/// Um contrato que declara resultado RELATIVO ao mapa recebido — valor do mapa,
/// mapa novo com chave — e operandos que já NÃO são papéis de mapa não descreve
/// operação alguma: o resultado aponta para um operando que deixou de existir.
///
/// A incoerência é do CONTRATO, lida do tipo da própria autoridade, e não uma
/// observação sobre consumidor nenhum. Onde ela aparece, a direção permissiva é
/// estruturalmente inexistente: não há chamada que a variante admita, em fase
/// alguma, porque a variante não descreve chamada admissível.
fn variante_coerente(mutada: &InternalOperation) -> bool {
    let resultado_relativo_ao_mapa = matches!(
        mutada.result,
        InternalResult::ValorDoMapaReceptor | InternalResult::MapaNovoComChave(_)
    );
    let operandos_relativos_ao_mapa = matches!(mutada.operands, InternalOperands::PapeisDeMapa(_));
    !resultado_relativo_ao_mapa || operandos_relativos_ao_mapa
}

/// A PROJEÇÃO que um consumidor deriva da autoridade.
///
/// Nem todo consumidor responde com aceite ou recusa. `ir/model` É o predicado
/// de família — a resposta dele é a própria classificação —, e o contexto de
/// lowering ANOTA no sítio de chamada a classe de resultado que a autoridade
/// declara. Para esses dois, exigir uma testemunha “ilegal sob A0 e legal sob
/// A1” seria exigir o quadrante errado: eles não recusam, eles PROJETAM.
///
/// O observável certo é o da disposição §10 — *actual inferred type / result /
/// projection* —, e ele é direcional nos dois sentidos de uma vez: se a fase
/// tiver tabela local, a projeção fica no valor ANTIGO sob a mutação, seja ela
/// mais estreita ou mais larga.
///
/// O valor esperado em cada estado vem da entrada daquele estado — a canônica
/// corrente e a variante derivada dela. Nenhuma classe é escrita aqui.
fn projecao(
    consumidor: Consumidor,
    sonda: &Sonda,
    spelling: &str,
    entrada: Option<&InternalOperation>,
) -> Option<(String, String)> {
    match consumidor {
        Consumidor::IrModel => {
            let medido = format!("generica={}", ir::is_generic_map_intrinsic(spelling));
            let esperado = format!(
                "generica={}",
                entrada.is_some_and(
                    |operacao| operacao.family == InternalOperationFamily::MapaGenerica
                )
            );
            Some((medido, esperado))
        }
        Consumidor::IrContext => {
            let baixado = ir::lower_program(&sonda.programa).ok()?;
            let mut anotados: Vec<TypeIR> = Vec::new();
            let mut copia = baixado;
            for funcao in &mut copia.functions {
                visitar_bloco(&mut funcao.entry, spelling, &mut |chamada| {
                    if let ValueIR::Call { ret_type, .. } = chamada {
                        if !anotados.contains(ret_type) {
                            anotados.push(*ret_type);
                        }
                    }
                });
            }
            if anotados.is_empty() {
                return None;
            }
            let medido = format!("{anotados:?}");
            let declarado = entrada.and_then(|operacao| operacao.declared_ret())?;
            let esperado = format!("{:?}", vec![declarado]);
            Some((medido, esperado))
        }
        _ => None,
    }
}

/// A obrigação por projeção: o que a fase projeta, nos dois estados, contra o
/// que a autoridade de cada estado declara.
fn obrigacao_por_projecao(
    consumidor: Consumidor,
    sonda: &Sonda,
    canonica: &InternalOperation,
    variante: &Variante,
) -> Option<(bool, bool)> {
    let spelling = canonica.spelling;
    let mutada = variante
        .tabela
        .iter()
        .find(|operacao| operacao.spelling == spelling);
    let (medido_a0, esperado_a0) = projecao(consumidor, sonda, spelling, Some(canonica))?;
    let (medido_a1, esperado_a1) = {
        let _mutado = MutatedContract::install(variante.tabela.clone());
        projecao(consumidor, sonda, spelling, mutada)?
    };
    // A prova exige as duas metades: a projeção precisa SEGUIR a autoridade nos
    // dois estados, e os dois estados precisam ser distinguíveis. Uma projeção
    // que não muda não prova derivação alguma.
    Some((
        medido_a0 == esperado_a0 && medido_a1 == esperado_a1,
        medido_a0 != medido_a1,
    ))
}

/// Todas as testemunhas permissivas desta variante, com a origem de cada uma.
///
/// A sonda real vem primeiro, porque ela é o programa que o compilador de
/// verdade produz. O programa mínimo entra como complemento — não como
/// substituto — para as formas que a sonda não consegue isolar.
fn testemunhas(
    corpus: &[Sonda],
    canonica: &InternalOperation,
    variante: &Variante,
) -> Vec<(&'static str, Result<Artefato, String>)> {
    let mut todas: Vec<(&'static str, Result<Artefato, String>)> = Vec::new();
    for sonda in corpus {
        if !sonda.testemunha(canonica.spelling) {
            continue;
        }
        todas.push((
            sonda.nome,
            testemunha_ir(sonda, canonica, variante).map(Artefato::Baixado),
        ));
        todas.push((
            sonda.nome,
            testemunha_ast(sonda, canonica, variante).map(Artefato::Arvore),
        ));
    }
    if let Some(primeira) = corpus.first() {
        todas.push((
            "mínima",
            testemunha_minima(primeira, canonica, variante).map(Artefato::Baixado),
        ));
    }
    todas
}

/// A resposta do consumidor à testemunha, sob a autoridade canônica e sob a
/// mutada.
///
/// `None` quando o consumidor não examina a representação em que a testemunha
/// existe, ou quando aquele artefato está contaminado por sítios canônicos.
fn leitura(
    consumidor: Consumidor,
    artefato: &Artefato,
    variante: &Variante,
) -> Option<(String, String)> {
    let canonica = consumidor.resposta_na_testemunha(artefato)?;
    let mutada = {
        let _mutado = MutatedContract::install(variante.tabela.clone());
        consumidor.resposta_na_testemunha(artefato)?
    };
    Some((canonica, mutada))
}

/// A resposta é um ACEITE?
///
/// Recusar-se a continuar também é resposta, e interromper também: as duas
/// contam como não-aceite, e nenhuma delas conta como prova.
fn aceitou(resposta: &str) -> bool {
    resposta == "ACEITA"
}

// ---------------------------------------------------------------------------
// As provas
// ---------------------------------------------------------------------------

/// Despejo bruto da matriz medida, para o registro de evidência da Task.
///
/// Não é gate: sem `ORACULO_MEDIR` no ambiente o teste não mede nada. Existe
/// porque os contadores do relatório terminal precisam sair de execução, não de
/// leitura de código.
#[test]
fn medicao_bruta() {
    if std::env::var_os("ORACULO_MEDIR").is_none() {
        return;
    }
    let corpus = corpus();
    for operacao in INTERNAL_OPERATIONS {
        for fato in FATOS {
            match medir(&corpus, operacao, *fato) {
                Err(razao) => println!("NA {} {} :: {razao}", operacao.spelling, fato.nome()),
                Ok(arestas) => {
                    for aresta in arestas {
                        println!(
                            "M {} {} [{}] {} {} decisoria={} acompanhou={}",
                            operacao.spelling,
                            fato.nome(),
                            aresta.dimensao.nome(),
                            aresta.sonda,
                            aresta.consumidor.nome(),
                            aresta.decisoria,
                            aresta.acompanhou
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn toda_operacao_declarada_tem_testemunha() {
    let corpus = corpus();
    let sem_testemunha: Vec<&str> = INTERNAL_OPERATIONS
        .iter()
        .filter(|operacao| {
            !corpus
                .iter()
                .any(|sonda| sonda.testemunha(operacao.spelling))
        })
        .map(|operacao| operacao.spelling)
        .collect();
    assert!(
        sem_testemunha.is_empty(),
        "operações declaradas sem sonda que as materialize: {sem_testemunha:?}"
    );
}

/// Arquivos de `src/**` que nomeiam a API canônica da autoridade.
///
/// É a descoberta do §11-B: pergunta sintática CONTROLADA — “quem consulta a
/// API canônica?” — e não a pergunta abandonada “quem escreveu qualquer Rust
/// semanticamente equivalente?”. Ela cobre o compilador inteiro, e não só as
/// fases que o corpus de sondas executa: um decisor novo no interpretador, num
/// backend ou em qualquer arquivo fora do pipeline das sondas aparece aqui.
fn inventario_estatico_de_consumidores() -> Vec<String> {
    fn varrer(dir: &std::path::Path, achados: &mut Vec<String>) {
        for entrada in std::fs::read_dir(dir).expect("ler diretório de fontes") {
            let caminho = entrada.expect("entrada de diretório").path();
            if caminho.is_dir() {
                varrer(&caminho, achados);
                continue;
            }
            if !caminho.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let relativo = caminho
                .strip_prefix(raiz())
                .expect("caminho dentro do repositório")
                .to_string_lossy()
                .replace('\\', "/");
            // A própria autoridade e o oráculo não são consumidores dela.
            if relativo.starts_with("src/internal_operations") {
                continue;
            }
            let texto = std::fs::read_to_string(&caminho).expect("ler fonte");
            if texto.contains("internal_operations::") {
                achados.push(relativo);
            }
        }
    }
    let mut achados = Vec::new();
    varrer(&raiz().join("src"), &mut achados);
    achados.sort();
    achados
}

/// Todos os arquivos `.rs` de `src/**` que não são a própria autoridade.
fn fontes_do_compilador() -> Vec<(String, String)> {
    fn varrer(dir: &std::path::Path, achados: &mut Vec<(String, String)>) {
        for entrada in std::fs::read_dir(dir).expect("ler diretório de fontes") {
            let caminho = entrada.expect("entrada de diretório").path();
            if caminho.is_dir() {
                varrer(&caminho, achados);
                continue;
            }
            if !caminho.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let relativo = caminho
                .strip_prefix(raiz())
                .expect("caminho dentro do repositório")
                .to_string_lossy()
                .replace('\\', "/");
            if relativo.starts_with("src/internal_operations") {
                continue;
            }
            let texto = std::fs::read_to_string(&caminho).expect("ler fonte");
            achados.push((relativo, texto));
        }
    }
    let mut achados = Vec::new();
    varrer(&raiz().join("src"), &mut achados);
    achados.sort();
    achados
}

/// Funções que REPASSAM a resposta da autoridade.
///
/// É a resposta a `B6`. A instrumentação dinâmica anota UM quadro de pilha — o
/// de quem chamou a autoridade — então um repassador declarado num arquivo já
/// escriturado esconde o consumidor real, que só nomeia o repassador. O
/// inventário estático tinha o mesmo ponto cego, porque perguntava apenas quem
/// nomeia `internal_operations::`.
///
/// Repassador é uma função cujo CORPO é a consulta: uma única menção à API
/// canônica, num corpo curto. Isso separa o repassador da entrada de fase — um
/// `validate_program` também consulta a autoridade, mas o que ele devolve é o
/// veredito da fase, não o fato. Quem chama uma entrada de fase está rodando o
/// compilador; quem chama um repassador está lendo a autoridade por outra
/// porta, e é isso que a descoberta precisa enxergar.
fn repassadores() -> Vec<(String, String)> {
    /// O corpo da função que começa em `fn`, por casamento de chaves.
    fn corpo(texto: &str, declaracao: usize) -> Option<&str> {
        let abre = declaracao + texto[declaracao..].find('{')?;
        let mut profundidade = 0usize;
        for (deslocamento, caractere) in texto[abre..].char_indices() {
            match caractere {
                '{' => profundidade += 1,
                '}' => {
                    profundidade -= 1;
                    if profundidade == 0 {
                        return Some(&texto[abre..abre + deslocamento + 1]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    let mut encontrados: Vec<(String, String)> = Vec::new();
    for (arquivo, texto) in fontes_do_compilador() {
        let mut inicio = 0usize;
        while let Some(posicao) = texto[inicio..].find("fn ") {
            let declaracao = inicio + posicao;
            inicio = declaracao + 3;
            let resto = &texto[declaracao + 3..];
            let fim = resto
                .find(|caractere: char| !caractere.is_alphanumeric() && caractere != '_')
                .unwrap_or(resto.len());
            let nome = resto[..fim].to_string();
            if nome.is_empty() {
                continue;
            }
            let Some(corpo) = corpo(texto.as_str(), declaracao) else {
                continue;
            };
            // Corpo curto com uma consulta só: a função É a consulta.
            if corpo.matches("internal_operations::").count() != 1 || corpo.len() > 240 {
                continue;
            }
            if !encontrados
                .iter()
                .any(|(existente, dono)| *existente == nome && *dono == arquivo)
            {
                encontrados.push((nome, arquivo.clone()));
            }
        }
    }
    encontrados.sort();
    encontrados
}

/// Arquivos que alcançam a autoridade por um repassador declarado noutro
/// arquivo.
fn consumidores_por_indirecao() -> Vec<(String, String)> {
    let repassadores = repassadores();
    let mut achados: Vec<(String, String)> = Vec::new();
    for (arquivo, texto) in fontes_do_compilador() {
        for (nome, dono) in &repassadores {
            if *dono == arquivo {
                continue;
            }
            // Consumo é CHAMADA. Reexportar o repassador não consome a
            // autoridade; quem consome é quem o chama.
            if texto.contains(&format!("{nome}(")) {
                achados.push((arquivo.clone(), nome.clone()));
            }
        }
    }
    achados.sort();
    achados
}

fn raiz() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn inventario_estatico_descoberta_e_escrituracao_coincidem() {
    let inventario = inventario_estatico_de_consumidores();
    let escriturados: Vec<&str> = CONSUMIDORES
        .iter()
        .map(|consumidor| consumidor.arquivo())
        .collect();

    let fora_da_escrituracao: Vec<&String> = inventario
        .iter()
        .filter(|arquivo| !escriturados.contains(&arquivo.as_str()))
        .collect();
    assert!(
        fora_da_escrituracao.is_empty(),
        "arquivos que consultam a API canônica sem escrituração no oráculo: {fora_da_escrituracao:?}"
    );

    let sem_inventario: Vec<&str> = escriturados
        .iter()
        .copied()
        .filter(|arquivo| !inventario.iter().any(|achado| achado == arquivo))
        .collect();
    assert!(
        sem_inventario.is_empty(),
        "escriturados que não nomeiam a API canônica em `src/**`: {sem_inventario:?}"
    );

    let fora_do_piso: Vec<&String> = inventario
        .iter()
        .filter(|arquivo| !PISO_HISTORICO.contains(&arquivo.as_str()))
        .collect();
    assert!(
        fora_do_piso.is_empty(),
        "consumidores novos, fora do piso histórico, sem explicação: {fora_do_piso:?}"
    );

    let piso_ausente: Vec<&str> = PISO_HISTORICO
        .iter()
        .copied()
        .filter(|arquivo| !inventario.iter().any(|achado| achado == arquivo))
        .collect();
    assert!(
        piso_ausente.is_empty(),
        "decisores do piso histórico que deixaram de consultar a autoridade \
         (matéria de disposição do Guia, não de edição desta lista): {piso_ausente:?}"
    );
    println!(
        "STATIC_INVENTORY_CONSUMERS = {}\n\
         STATIC_INVENTORY_OUTSIDE_FLOOR = {}",
        inventario.len(),
        fora_do_piso.len()
    );
}

/// Nenhum consumidor alcança a autoridade por indireção sem escrituração.
///
/// Fecha `B6`: a atribuição por quadro de chamada deixa de ser o limite da
/// descoberta. Quem nomeia um repassador declarado noutro arquivo é consumidor,
/// e precisa estar escriturado como qualquer outro.
#[test]
fn nenhum_consumidor_alcanca_a_autoridade_por_indirecao() {
    let escriturados: Vec<&str> = CONSUMIDORES
        .iter()
        .map(|consumidor| consumidor.arquivo())
        .collect();
    // Piso: a varredura precisa achar repassadores. Uma lista vazia deixaria a
    // checagem verde por vacuidade, que é a forma que `M-A`/`M-B` tinham.
    let repassadores = repassadores();
    assert!(
        repassadores.len() >= 3,
        "a varredura de repassadores encolheu para {}: a checagem de indireção ficaria vácua",
        repassadores.len()
    );
    let fora: Vec<String> = consumidores_por_indirecao()
        .into_iter()
        .filter(|(arquivo, _)| !escriturados.contains(&arquivo.as_str()))
        .map(|(arquivo, repassador)| format!("{arquivo} -> {repassador}"))
        .collect();
    assert!(
        fora.is_empty(),
        "arquivos que alcançam a autoridade por repassador sem escrituração no oráculo: {fora:?}"
    );
}

#[test]
fn carve_outs_cobrem_toda_a_matriz() {
    let mut faltando = Vec::new();
    let mut duplicadas = Vec::new();
    for consumidor in CONSUMIDORES {
        for fato in FATOS {
            let consultado = consulta(*consumidor, *fato);
            let isentado = CARVE_OUTS
                .iter()
                .any(|(c, f, _)| c == consumidor && f == fato);
            match (consultado, isentado) {
                (false, false) => faltando.push((consumidor.nome(), fato.nome())),
                (true, true) => duplicadas.push((consumidor.nome(), fato.nome())),
                _ => {}
            }
        }
    }
    assert!(
        faltando.is_empty(),
        "células sem consulta declarada e sem carve-out com razão: {faltando:?}"
    );
    assert!(
        duplicadas.is_empty(),
        "células que declaram consulta E carve-out ao mesmo tempo: {duplicadas:?}"
    );
    assert!(
        CARVE_OUTS.iter().all(|(_, _, razao)| !razao.is_empty()),
        "carve-out sem razão escrita"
    );
}

#[test]
fn descoberta_e_escrituracao_coincidem() {
    let gravacao = Gravacao::abrir();
    // A descoberta cobre exatamente o que o oráculo observa: construir o corpus
    // e ler a resposta de cada consumidor, sob a autoridade real.
    let corpus = corpus();
    for sonda in &corpus {
        for consumidor in CONSUMIDORES {
            let _ = consumidor.resposta(sonda, TERNARIA);
        }
    }
    let descobertos = gravacao.fechar();

    let escriturados: Vec<&str> = CONSUMIDORES
        .iter()
        .map(|consumidor| consumidor.arquivo())
        .collect();

    let sem_escrituracao: Vec<&str> = descobertos
        .iter()
        .copied()
        .filter(|arquivo| !escriturados.contains(arquivo))
        .collect();
    assert!(
        sem_escrituracao.is_empty(),
        "consumidores descobertos sem escrituração no oráculo: {sem_escrituracao:?}"
    );

    let sem_descoberta: Vec<&str> = escriturados
        .iter()
        .copied()
        .filter(|arquivo| !descobertos.contains(arquivo))
        .collect();
    assert!(
        sem_descoberta.is_empty(),
        "escriturados que a descoberta não encontrou consultando a autoridade: {sem_descoberta:?}"
    );

    let piso_ausente: Vec<&str> = PISO_HISTORICO
        .iter()
        .copied()
        .filter(|arquivo| !descobertos.contains(arquivo))
        .collect();
    assert!(
        piso_ausente.is_empty(),
        "decisores do piso histórico que deixaram de consultar a autoridade \
         (matéria de disposição do Guia, não de edição desta lista): {piso_ausente:?}"
    );

    let novos: Vec<&str> = descobertos
        .iter()
        .copied()
        .filter(|arquivo| !PISO_HISTORICO.contains(arquivo))
        .collect();
    assert!(
        novos.is_empty(),
        "consumidores novos, fora do piso histórico, sem explicação: {novos:?}"
    );
    println!(
        "DISCOVERED_CONSUMERS = {}\n\
         HISTORICAL_CONSUMERS_EXPECTED = {}\n\
         HISTORICAL_CONSUMERS_MISSING = {}\n\
         UNEXPLAINED_NEW_CONSUMERS = {}",
        descobertos.len(),
        PISO_HISTORICO.len(),
        piso_ausente.len(),
        novos.len()
    );
}

#[test]
fn cobertura_exaustiva_do_dominio_por_fato() {
    let corpus = corpus();
    let mut provados = 0usize;
    let mut nao_aplicaveis = 0usize;
    let mut arestas_decisorias: Vec<(&str, &str, &str)> = Vec::new();
    let mut arestas_provadas: Vec<(&str, &str, &str)> = Vec::new();
    let mut divergencias: Vec<String> = Vec::new();

    for operacao in INTERNAL_OPERATIONS {
        for fato in FATOS {
            let arestas = match medir(&corpus, operacao, *fato) {
                Err(_) => {
                    nao_aplicaveis += 1;
                    continue;
                }
                Ok(arestas) => arestas,
            };
            let mut decisorias_do_par = 0usize;
            for aresta in &arestas {
                if aresta.decisoria {
                    decisorias_do_par += 1;
                    let chave = (operacao.spelling, fato.nome(), aresta.consumidor.nome());
                    if !arestas_decisorias.contains(&chave) {
                        arestas_decisorias.push(chave);
                    }
                    if aresta.acompanhou && !arestas_provadas.contains(&chave) {
                        arestas_provadas.push(chave);
                    }
                }
                if aresta.decisoria && !aresta.acompanhou {
                    divergencias.push(format!(
                        "{} / {} [{}] / sonda {}: {} decide o fato para esta operação, alcança a chamada e NÃO acompanhou a mutação — decisão local sobreviveu",
                        operacao.spelling,
                        fato.nome(),
                        aresta.dimensao.nome(),
                        aresta.sonda,
                        aresta.consumidor.nome()
                    ));
                }
            }
            if decisorias_do_par == 0 {
                // Piso por par: um par só é PROVADO se ALGUM decisor foi
                // obrigado a acompanhar e acompanhou. Sem este piso, encolher a
                // superfície de decisão — por engano ou por deriva do formato
                // `Debug` que mede alcance — apagaria arestas em silêncio e o
                // relatório continuaria dizendo `UNPROVED_EDGES = 0`.
                divergencias.push(format!(
                    "{} / {}: nenhuma aresta decisória — o fato não chega a decisor algum nas sondas",
                    operacao.spelling,
                    fato.nome()
                ));
                continue;
            }
            provados += 1;
        }
    }

    // Piso por célula declarada: cada decisor precisa ter aresta decisória
    // provada em cada fato que declara consultar. É o que impede que uma coluna
    // inteira da matriz — um estágio que deixou de ser medido, um consumidor que
    // saiu do alcance — evapore com a suíte verde.
    for consumidor in CONSUMIDORES {
        for fato in FATOS {
            if !consulta(*consumidor, *fato) {
                continue;
            }
            let arestas = arestas_provadas
                .iter()
                .filter(|(_, nome_fato, nome_consumidor)| {
                    *nome_fato == fato.nome() && *nome_consumidor == consumidor.nome()
                })
                .count();
            if arestas == 0 {
                divergencias.push(format!(
                    "{} declara consultar {} e não tem uma só aresta decisória provada no domínio",
                    consumidor.nome(),
                    fato.nome()
                ));
            }
        }
    }

    assert!(
        divergencias.is_empty(),
        "cobertura metamórfica vermelha ({} divergências):\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );
    assert_eq!(
        provados + nao_aplicaveis,
        INTERNAL_OPERATIONS.len() * FATOS.len(),
        "par (operação, fato) sem classificação"
    );
    println!(
        "DECLARED_OPERATION_COUNT = {}\n\
         OPERATION_FACT_PAIR_COUNT = {}\n\
         PROVED_PAIR_COUNT = {provados}\n\
         STRUCTURALLY_NOT_APPLICABLE_COUNT = {nao_aplicaveis}\n\
         UNCLASSIFIED_PAIR_COUNT = 0\n\
         DISCOVERED_EDGES = {}\n\
         PROVED_EDGES = {}\n\
         UNPROVED_EDGES = {}",
        INTERNAL_OPERATIONS.len(),
        INTERNAL_OPERATIONS.len() * FATOS.len(),
        arestas_decisorias.len(),
        arestas_provadas.len(),
        arestas_decisorias.len() - arestas_provadas.len()
    );
}

/// A prova na direção PERMISSIVA: a autoridade passa a admitir, e quem deriva
/// passa a aceitar.
///
/// Para cada operação, o artefato expandido — um operando a mais em cada
/// chamada — é ilegal sob a autoridade canônica e legal sob a mutação de
/// aridade. Duas exigências, uma para cada lado da matriz:
///
/// * **célula consultada**: sob a canônica o consumidor precisa RECUSAR, e sob
///   a mutação precisa ACEITAR. Uma tabela local ADITIVA que concorde com a
///   autoridade continua recusando, e é exatamente isso que a direção
///   restritiva não consegue ver.
/// * **célula com carve-out**: o consumidor precisa ACEITAR nos dois estados.
///   É a prova que faltava de que ele NÃO tem caminho independente para o fato
///   — “não pergunta” deixa de ser lido como “não decide”.
///
/// Os pisos impedem que a prova encolha em silêncio, pela mesma razão que
/// [`cobertura_exaustiva_do_dominio_por_fato`] tem os dela: uma auditoria sem
/// piso pode perder arestas sem ficar vermelha.
#[test]
fn direcao_permissiva_por_aridade() {
    let corpus = corpus();
    let mut divergencias: Vec<String> = Vec::new();
    let mut derivacoes: Vec<(&str, &str)> = Vec::new();
    let mut ausencias: Vec<(&str, &str)> = Vec::new();
    let mut nao_construtiveis: Vec<String> = Vec::new();
    let mut operacoes_provadas: Vec<&str> = Vec::new();

    for operacao in INTERNAL_OPERATIONS {
        let variantes = match variantes(operacao.spelling, Fato::Aridade) {
            Ok(variantes) => variantes,
            Err(razao) => {
                nao_construtiveis.push(format!("{} :: {razao}", operacao.spelling));
                continue;
            }
        };
        for sonda in &corpus {
            if !sonda.testemunha(operacao.spelling) {
                continue;
            }
            for variante in &variantes {
                // Caminho principal: artefato expandido de ponta a ponta, legal
                // sob a autoridade mutada. Onde a descida não o constrói, cada
                // validador ainda pode ser perguntado pelo artefato do estágio
                // que ELE examina, por cirurgia.
                let expandida = match expandir(sonda, operacao.spelling, &variante.tabela) {
                    Ok(expandida) => Some(expandida),
                    Err(razao) => {
                        nao_construtiveis
                            .push(format!("{} / {} :: {razao}", operacao.spelling, sonda.nome));
                        None
                    }
                };
                for consumidor in CONSUMIDORES {
                    if !consumidor.alcanca(sonda, operacao.spelling) {
                        continue;
                    }
                    let ler = |consumidor: &Consumidor| match &expandida {
                        Some(expandida) => consumidor.resposta_expandida(expandida),
                        None => consumidor.resposta_expandida_no_estagio(sonda, operacao.spelling),
                    };
                    let Some(sob_canonica) = ler(consumidor) else {
                        continue;
                    };
                    let sob_mutacao = {
                        let _mutado = MutatedContract::install(variante.tabela.clone());
                        ler(consumidor).expect("mesma disponibilidade de resposta nos dois estados")
                    };
                    let aceita_canonica = sob_canonica == "ACEITA";
                    let aceita_mutada = sob_mutacao == "ACEITA";
                    if consulta(*consumidor, Fato::Aridade) {
                        if !decide(*consumidor, Fato::Aridade, variante.dimensao, operacao) {
                            continue;
                        }
                        if aceita_canonica || !aceita_mutada {
                            divergencias.push(format!(
                                "{} / {}: {} consulta a aridade e não acompanhou a EXPANSÃO da autoridade (canônica={sob_canonica}, mutada={sob_mutacao}) — decisão local concorde sobreviveu",
                                operacao.spelling,
                                sonda.nome,
                                consumidor.nome()
                            ));
                        } else {
                            derivacoes.push((operacao.spelling, consumidor.nome()));
                            if !operacoes_provadas.contains(&operacao.spelling) {
                                operacoes_provadas.push(operacao.spelling);
                            }
                        }
                    } else if !aceita_canonica || !aceita_mutada {
                        divergencias.push(format!(
                            "{} / {}: {} tem carve-out de aridade e RECUSA o artefato expandido (canônica={sob_canonica}, mutada={sob_mutacao}) — tem caminho independente para o fato que não pergunta",
                            operacao.spelling,
                            sonda.nome,
                            consumidor.nome()
                        ));
                    } else {
                        ausencias.push((operacao.spelling, consumidor.nome()));
                    }
                }
            }
        }
    }

    if std::env::var_os("ORACULO_MEDIR").is_some() {
        println!(
            "PERMISSIVA derivacoes={} ausencias={} operacoes={}",
            derivacoes.len(),
            ausencias.len(),
            operacoes_provadas.len()
        );
        for razao in &nao_construtiveis {
            println!("PERMISSIVA_NAO_CONSTRUTIVEL {razao}");
        }
    }

    assert!(
        divergencias.is_empty(),
        "direção permissiva vermelha ({} divergências):\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );

    // Piso por decisor: todo consumidor que examina artefato e declara consultar
    // a aridade precisa ter ao menos uma derivação provada na direção
    // permissiva. Sem isso, tirar um decisor do alcance apagaria a prova dele
    // sem um teste cair.
    for consumidor in CONSUMIDORES {
        if !consulta(*consumidor, Fato::Aridade) {
            continue;
        }
        if matches!(
            consumidor,
            Consumidor::Semantic | Consumidor::IrContext | Consumidor::IrModel
        ) {
            // Não examinam artefato onde a expansão existe: a AST ainda não tem
            // a chamada interna na forma expandida, e `ir/model` é predicado
            // sobre a grafia. A direção restritiva os cobre.
            continue;
        }
        // A aridade da ramificação É a forma dela: um `__ternario` com um
        // operando a mais não é um ramo, é outra coisa, e a CFG o recusa pela
        // forma antes de qualquer contrato. Onde TODAS as células decisórias de
        // aridade do consumidor são de ramificação, a pergunta permissiva não
        // chega a existir, e exigir derivação dele seria exigir o impossível.
        let tem_celula_construtivel = INTERNAL_OPERATIONS.iter().any(|operacao| {
            !matches!(operacao.operands, InternalOperands::Ramificacao)
                && decide(
                    *consumidor,
                    Fato::Aridade,
                    Dimensao::OperandoAMais,
                    operacao,
                )
        });
        if !tem_celula_construtivel {
            continue;
        }
        assert!(
            derivacoes.iter().any(|(_, nome)| *nome == consumidor.nome()),
            "{} declara consultar a aridade e não tem uma só derivação provada na direção permissiva",
            consumidor.nome()
        );
    }

    // Piso por domínio: TODA operação declarada precisa ter derivação provada
    // na direção permissiva. É o mesmo invariante da direção restritiva — o
    // domínio é a autoridade — e é o que impede que a prova permissiva volte a
    // ser por amostra.
    let sem_derivacao: Vec<&str> = INTERNAL_OPERATIONS
        .iter()
        .map(|operacao| operacao.spelling)
        .filter(|spelling| !operacoes_provadas.contains(spelling))
        .collect();
    assert!(
        sem_derivacao.is_empty(),
        "operações sem derivação provada na direção permissiva: {sem_derivacao:?}"
    );

    // Pisos globais: a prova precisa exibir as DUAS metades. Os números são
    // piso, não expectativa: existem para que uma queda apareça, e a medição
    // corrente passa deles com folga.
    assert!(
        derivacoes.len() >= 100 && ausencias.len() >= 35,
        "a direção permissiva precisa provar derivação E ausência: derivações={}, ausências={}",
        derivacoes.len(),
        ausencias.len()
    );
}

/// Disposição de uma obrigação direcional.
///
/// São as quatro classes que a disposição do Guia admite, e nenhuma delas é
/// atalho: `EquivalenciaDeRepresentacao` e `NaoAplicavel` exigem razão CAUSAL
/// MEDIDA, nunca “não consegui construir a testemunha”.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disposicao {
    /// Existe testemunha nova — ilegal sob a canônica, legal sob a mutada — e o
    /// consumidor a ADMITE. É o quadrante que a direção restritiva não alcança.
    ProvadaPorAdmissao,
    /// A autoridade mutada não admite chamada ALGUMA nesta fase: a testemunha
    /// construída a partir do contrato mutado é recusada sob ele, e aceita sob
    /// a canônica. A rota da fase desaparece com a mutação, então um consumidor
    /// que continue ACEITANDO só pode estar decidindo por conta própria — e a
    /// matriz restritiva exige e mede essa mudança.
    ProvadaPorRemocaoDeRota,
    /// A fase não distingue os dois estados do contrato: toda testemunha que a
    /// autoridade mutada admite, a canônica já admitia. Não existe caso novo, e
    /// por isso nenhuma cópia local concorde poderia recusá-lo.
    EquivalenciaDeRepresentacao,
    /// O observável do consumidor é uma PROJEÇÃO, não um veredito: ele deriva
    /// a resposta da autoridade em vez de aceitar ou recusar um artefato. A
    /// prova é a projeção seguir a autoridade nos DOIS estados e distinguir um
    /// do outro — direcional nos dois sentidos de uma vez.
    ProvadaPorProjecao,
    /// A decisão compartilhada não existe naquela variante: o contrato mutado é
    /// incoerente consigo mesmo, e por isso não admite chamada alguma em fase
    /// alguma. A razão é lida do tipo da autoridade, não do comportamento do
    /// consumidor.
    EstruturalmenteNaoAplicavel,
    /// Existe decisão material e não há prova suficiente.
    NaoProvada,
}

impl Disposicao {
    fn nome(self) -> &'static str {
        match self {
            Self::ProvadaPorAdmissao => "PROVADA/admissão",
            Self::ProvadaPorRemocaoDeRota => "PROVADA/remoção de rota",
            Self::ProvadaPorProjecao => "PROVADA/projeção",
            Self::EquivalenciaDeRepresentacao => "EQUIVALÊNCIA DE REPRESENTAÇÃO",
            Self::EstruturalmenteNaoAplicavel => "ESTRUTURALMENTE NÃO APLICÁVEL",
            Self::NaoProvada => "NÃO PROVADA",
        }
    }
}

/// A obrigação direcional de uma célula da matriz.
///
/// A unidade é `(operação, fato, dimensão, consumidor)`, com a representação, a
/// testemunha, o observável e os dois resultados medidos. Nenhum campo repete
/// valor canônico: aridade, classe de operando e classe de resultado continuam
/// existindo só em [`INTERNAL_OPERATIONS`], e chegam aqui pela variante, que
/// nasceu da entrada canônica corrente.
struct Obrigacao {
    operacao: &'static str,
    fato: Fato,
    dimensao: &'static str,
    consumidor: Consumidor,
    /// Por origem de testemunha: `(aceita sob a canônica, aceita sob a mutada)`.
    leituras: Vec<(&'static str, bool, bool)>,
    /// `(a projeção seguiu a autoridade nos dois estados, os dois estados são
    /// distinguíveis)`, quando o observável do consumidor é uma projeção.
    projecao: Option<(bool, bool)>,
    /// A variante descreve um contrato coerente consigo mesmo?
    coerente: bool,
    razoes: Vec<String>,
}

impl Obrigacao {
    fn admite(&self) -> bool {
        self.leituras
            .iter()
            .any(|(_, canonica, mutada)| !canonica && *mutada)
    }

    fn remove_rota(&self) -> bool {
        self.leituras
            .iter()
            .any(|(_, canonica, mutada)| *canonica && !mutada)
    }

    fn sem_caso_novo(&self) -> bool {
        !self.leituras.is_empty()
            && self
                .leituras
                .iter()
                .all(|(_, canonica, mutada)| *canonica && *mutada)
    }
}

/// A prova na direção PERMISSIVA, fato a fato e operação a operação.
///
/// [`direcao_permissiva_por_aridade`] prova F2 pelo artefato EXPANDIDO — um
/// operando a mais em cada chamada. Para F1/família, F3 e F4 a cirurgia é
/// outra, porque a mutação não acrescenta operando: ela TROCA a classe exigida,
/// e o artefato que a mutação admite é um sítio de chamada NOVO.
///
/// ```text
///                A0 (canônica)      A1 (mutada)
///   testemunha   RECUSA             ACEITA
/// ```
///
/// Uma cópia local ADITIVA e concorde — a fase pergunta à autoridade E confere
/// de novo, por conta própria, a regra antiga — continua RECUSANDO sob `A1`, e a
/// divergência a denuncia. É esta metade que faltava para F1, F3 e F4.
///
/// Onde a fase não tem o quadrante — porque a autoridade mutada não admite
/// chamada alguma nela, ou porque tudo que a mutada admite a canônica já
/// admitia — a razão é MEDIDA, não presumida, e a disposição diz qual das duas
/// é.
#[test]
fn direcao_permissiva_por_fato() {
    let corpus = corpus();
    let mut obrigacoes: Vec<Obrigacao> = Vec::new();
    let mut divergencias: Vec<String> = Vec::new();
    let mut nao_aplicaveis: Vec<String> = Vec::new();

    for operacao in INTERNAL_OPERATIONS {
        for fato in FATOS_PERMISSIVOS {
            let lista = match variantes(operacao.spelling, *fato) {
                Ok(lista) => lista,
                Err(razao) => {
                    nao_aplicaveis.push(format!(
                        "{} / {} :: {razao}",
                        operacao.spelling,
                        fato.nome()
                    ));
                    continue;
                }
            };
            for variante in &lista {
                let construidas = testemunhas(&corpus, operacao, variante);
                for consumidor in CONSUMIDORES {
                    // A obrigação só existe onde o consumidor ALCANÇA a
                    // operação: é o mesmo filtro da matriz restritiva, e é o
                    // que impede inventar aresta onde a fase nunca decide. O
                    // contexto de lowering registra assinatura de TODA operação
                    // de contrato fixo, mas quem constrói a cadeia de variante
                    // é o produtor, e essa chamada não chega à AST que ele
                    // examina.
                    if !corpus
                        .iter()
                        .any(|sonda| consumidor.alcanca(sonda, operacao.spelling))
                    {
                        continue;
                    }
                    let mut obrigacao = Obrigacao {
                        operacao: operacao.spelling,
                        fato: *fato,
                        dimensao: variante.dimensao.nome(),
                        consumidor: *consumidor,
                        leituras: Vec::new(),
                        coerente: variante
                            .tabela
                            .iter()
                            .find(|entrada| entrada.spelling == operacao.spelling)
                            .is_some_and(variante_coerente),
                        projecao: corpus.iter().find_map(|sonda| {
                            obrigacao_por_projecao(*consumidor, sonda, operacao, variante)
                        }),
                        razoes: Vec::new(),
                    };
                    for (origem, testemunha) in &construidas {
                        match testemunha {
                            Err(razao) => obrigacao.razoes.push(format!("{origem}: {razao}")),
                            Ok(testemunha) => {
                                if let Some((canonica, mutada)) =
                                    leitura(*consumidor, testemunha, variante)
                                {
                                    obrigacao.leituras.push((
                                        origem,
                                        aceitou(&canonica),
                                        aceitou(&mutada),
                                    ));
                                }
                            }
                        }
                    }
                    if obrigacao.leituras.is_empty() && obrigacao.projecao.is_none() {
                        continue;
                    }
                    if !consulta(*consumidor, *fato) {
                        // Célula com carve-out: a prova exigida é a de AUSÊNCIA
                        // de caminho independente. Quem não pergunta o fato não
                        // pode recusar uma testemunha que só viola esse fato.
                        for (origem, canonica, mutada) in &obrigacao.leituras {
                            if !canonica || !mutada {
                                divergencias.push(format!(
                                    "{} / {} [{}] / {origem}: {} tem carve-out do fato e RECUSA a testemunha (canônica={canonica}, mutada={mutada}) — tem caminho independente para o fato que não pergunta",
                                    operacao.spelling,
                                    fato.nome(),
                                    variante.dimensao.nome(),
                                    consumidor.nome()
                                ));
                            }
                        }
                        continue;
                    }
                    if !decide(*consumidor, *fato, variante.dimensao, operacao) {
                        continue;
                    }
                    obrigacoes.push(obrigacao);
                }
            }
        }
    }

    let mut contagem = [0usize; 6];
    let mut nao_provadas: Vec<String> = Vec::new();
    for obrigacao in &obrigacoes {
        let disposicao = if obrigacao.projecao == Some((true, true)) {
            Disposicao::ProvadaPorProjecao
        } else if obrigacao.admite() {
            Disposicao::ProvadaPorAdmissao
        } else if obrigacao.remove_rota() {
            Disposicao::ProvadaPorRemocaoDeRota
        } else if obrigacao.sem_caso_novo() {
            Disposicao::EquivalenciaDeRepresentacao
        } else if !obrigacao.coerente {
            Disposicao::EstruturalmenteNaoAplicavel
        } else {
            Disposicao::NaoProvada
        };
        contagem[match disposicao {
            Disposicao::ProvadaPorAdmissao => 0,
            Disposicao::ProvadaPorRemocaoDeRota => 1,
            Disposicao::ProvadaPorProjecao => 2,
            Disposicao::EquivalenciaDeRepresentacao => 3,
            Disposicao::EstruturalmenteNaoAplicavel => 4,
            Disposicao::NaoProvada => 5,
        }] += 1;
        if disposicao == Disposicao::NaoProvada {
            nao_provadas.push(format!(
                "{} / {} [{}] / {}: {} — leituras {:?}, projeção {:?}, razões {:?}",
                obrigacao.operacao,
                obrigacao.fato.nome(),
                obrigacao.dimensao,
                obrigacao.consumidor.nome(),
                disposicao.nome(),
                obrigacao.leituras,
                obrigacao.projecao,
                obrigacao.razoes
            ));
        }
    }

    assert!(
        divergencias.is_empty(),
        "direção permissiva vermelha ({} divergências):\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );
    assert!(
        nao_provadas.is_empty(),
        "obrigações direcionais sem prova ({}):\n  {}",
        nao_provadas.len(),
        nao_provadas.join("\n  ")
    );

    // Piso por célula declarada: nenhuma célula `(decisor, fato)` pode ficar de
    // pé SÓ por equivalência de representação. Equivalência é leitura honesta
    // onde a fase de fato não distingue os dois estados, mas uma coluna inteira
    // sustentada por ela seria a prova encolhendo em silêncio — que é o que os
    // pisos existem para impedir.
    for consumidor in CONSUMIDORES {
        for fato in FATOS_PERMISSIVOS {
            if !consulta(*consumidor, *fato) {
                continue;
            }
            let da_celula: Vec<&Obrigacao> = obrigacoes
                .iter()
                .filter(|obrigacao| obrigacao.consumidor == *consumidor && obrigacao.fato == *fato)
                .collect();
            if da_celula.is_empty() {
                continue;
            }
            assert!(
                da_celula.iter().any(|obrigacao| obrigacao.admite()
                    || obrigacao.projecao == Some((true, true))
                    || obrigacao.remove_rota()),
                "{} declara consultar {} e toda obrigação dele no domínio está de pé só por equivalência de representação",
                consumidor.nome(),
                fato.nome()
            );
        }
    }

    // Piso global do quadrante NOVO: a prova permissiva precisa exibir
    // admissão em quantidade, não uma amostra. O número é piso, não
    // expectativa: existe para que uma queda apareça, e a medição corrente
    // passa dele com folga.
    assert!(
        contagem[0] >= 200,
        "a direção permissiva precisa provar ADMISSÃO em escala: {} obrigações provadas por admissão",
        contagem[0]
    );

    println!(
        "DIRECTIONAL_OBLIGATIONS_TOTAL = {}\n\
         DIRECTIONAL_PROVED_BY_ADMISSION = {}\n\
         DIRECTIONAL_PROVED_BY_ROUTE_REMOVAL = {}\n\
         DIRECTIONAL_PROVED_BY_PROJECTION = {}\n\
         DIRECTIONAL_REPRESENTATION_EQUIVALENCE = {}\n\
         DIRECTIONAL_STRUCTURALLY_NOT_APPLICABLE = {}\n\
         DIRECTIONAL_UNPROVED = {}\n\
         DIRECTIONAL_NOT_APPLICABLE_PAIRS = {}",
        obrigacoes.len(),
        contagem[0],
        contagem[1],
        contagem[2],
        contagem[3],
        contagem[4],
        contagem[5],
        nao_aplicaveis.len(),
    );
    if std::env::var_os("ORACULO_MEDIR").is_some() {
        for razao in &nao_aplicaveis {
            println!("DIRECTIONAL_NA {razao}");
        }
        for obrigacao in &obrigacoes {
            println!(
                "DIRECTIONAL {} {} [{}] {} :: {:?}",
                obrigacao.operacao,
                obrigacao.fato.nome(),
                obrigacao.dimensao,
                obrigacao.consumidor.nome(),
                obrigacao.leituras
            );
        }
    }
}

/// Grafia da entrada SINTÉTICA de pertinência. Existe só dentro de uma variante
/// instalada por teste: nunca está em [`INTERNAL_OPERATIONS`], nunca chega à
/// superfície pública, nunca ganha binding de runtime e nunca muda ABI.
const SINTETICA: &str = "__pinker_internal_u01_pertinencia_sintetica";

/// A entrada sintética: contrato mínimo, na forma que qualquer fase sabe ler.
///
/// Ela não é uma operação nova do compilador — é a pergunta “esta grafia está
/// DECLARADA?” feita na direção que a remoção não alcança. Nenhum corpo de
/// runtime, nenhum binding nativo e nenhum lowering especializado são exigidos
/// dela: §11 da disposição separa `SYNTHETIC_OPERATION_EXISTS` de
/// `EVERY_PHASE_MUST_IMPLEMENT_ARBITRARY_NEW_OPERATION`.
fn entrada_sintetica() -> InternalOperation {
    InternalOperation {
        spelling: SINTETICA,
        family: InternalOperationFamily::Leque,
        operands: InternalOperands::Declarados(&[TypeIR::Bombom]),
        result: InternalResult::Declarado(TypeIR::Bombom),
    }
}

/// F1/pertinência na direção PERMISSIVA: a autoridade passa a DECLARAR, e quem
/// deriva passa a admitir.
///
/// A mutação restritiva de pertinência RETIRA a operação, e prova dependência
/// na direção da remoção. Ela não vê a lista local ADITIVA: um decisor que
/// pergunte à autoridade E mantenha a própria lista continua respondendo o
/// mesmo quando a autoridade perde uma entrada que a lista dele ainda tem.
///
/// A direção que falta é a entrada NOVA. Um decisor que derive a pertinência da
/// autoridade admite a grafia sintética; um que a conjugue com lista própria
/// continua recusando, e a divergência o denuncia.
///
/// A exigência vale só para os decisores cuja pergunta compartilhada é mesmo
/// *esta grafia é operação interna declarada?* — medido, não declarado: são os
/// que RECUSAM a testemunha sob a autoridade real. Quem não a recusa não
/// consulta pertinência para ela, e exigir admissão dele seria exigir
/// implementação de fase para uma operação fictícia.
#[test]
fn direcao_permissiva_por_pertinencia() {
    let corpus = corpus();
    let sintetica = entrada_sintetica();
    assert!(
        !INTERNAL_OPERATIONS
            .iter()
            .any(|operacao| operacao.spelling == SINTETICA),
        "a entrada sintética não pode existir na autoridade de produção"
    );
    assert!(
        crate::intrinsics::registry::HISTORICAL
            .iter()
            .all(|entrada| entrada.spelling != SINTETICA),
        "a entrada sintética não pode encostar na superfície pública"
    );
    let mut aditiva = INTERNAL_OPERATIONS.to_vec();
    aditiva.push(sintetica);
    let variante = Variante {
        dimensao: Dimensao::Pertinencia,
        tabela: aditiva,
    };
    let sonda = corpus.first().expect("corpus não vazio");
    let testemunha = testemunha_minima(sonda, &sintetica, &variante)
        .map(Artefato::Baixado)
        .expect("a testemunha mínima da entrada sintética precisa ser construtível");

    let mut derivaram: Vec<&'static str> = Vec::new();
    let mut indiferentes: Vec<String> = Vec::new();
    let mut divergencias: Vec<String> = Vec::new();
    for consumidor in CONSUMIDORES {
        if *consumidor == Consumidor::BackendSExternalCallconv {
            // O emissor do subset montável precisa do SÍMBOLO do runtime, e o
            // símbolo não mora nesta autoridade: `NATIVE_BINDING_OWNER` é
            // `src/backend_s.rs`, e G651-01 fechou exatamente essa fronteira.
            //
            // ```text
            // OPERATION EXISTS != THIS OPERATION BINDS TO THIS ABI SYMBOL
            // ```
            //
            // Exigir que ele admita uma grafia fictícia seria exigir binding
            // nativo para ela — o que a §11 da disposição proíbe e o que
            // reabriria G651-01 pelo avesso. A carve-out dele já diz que ele
            // classifica a pseudo-chamada pela grafia reservada, não pela
            // tabela.
            indiferentes.push(format!(
                "{}: exige binding nativo, que a autoridade não declara",
                consumidor.nome()
            ));
            continue;
        }
        let Some((canonica, mutada)) = leitura(*consumidor, &testemunha, &variante) else {
            continue;
        };
        if aceitou(&canonica) {
            // Não recusa a grafia desconhecida: a pergunta compartilhada dele
            // não é pertinência desta operação. Registrado, não exigido.
            indiferentes.push(format!("{}: {canonica}", consumidor.nome()));
            continue;
        }
        if aceitou(&mutada) {
            derivaram.push(consumidor.nome());
        } else {
            divergencias.push(format!(
                "{}: recusa a grafia sob a autoridade que a DECLARA (canônica={canonica}, mutada={mutada}) — a pertinência dele não vem da autoridade",
                consumidor.nome()
            ));
        }
    }

    assert!(
        divergencias.is_empty(),
        "F1/pertinência permissiva vermelha ({} divergências):\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );
    assert!(
        derivaram.len() >= 4,
        "a pertinência precisa ser derivada por todos os validadores de contrato: derivaram {derivaram:?}, indiferentes {indiferentes:?}"
    );
    println!(
        "F1_EXISTENCE_PERMISSIVE_DERIVED = {derivaram:?}\n\
         F1_EXISTENCE_PERMISSIVE_INDIFFERENT = {indiferentes:?}"
    );
}

#[test]
fn declaracao_de_fatos_por_consumidor_e_exata() {
    let corpus = corpus();
    let mut observado: Vec<(Consumidor, Fato)> = Vec::new();
    for operacao in INTERNAL_OPERATIONS {
        for fato in FATOS {
            let Ok(arestas) = medir(&corpus, operacao, *fato) else {
                continue;
            };
            for aresta in arestas {
                if aresta.acompanhou && !observado.contains(&(aresta.consumidor, *fato)) {
                    observado.push((aresta.consumidor, *fato));
                }
            }
        }
    }

    let mut divergencias = Vec::new();
    for consumidor in CONSUMIDORES {
        for fato in FATOS {
            let declarado = consulta(*consumidor, *fato);
            let medido = observado.contains(&(*consumidor, *fato));
            if declarado && !medido {
                divergencias.push(format!(
                    "{} declara consultar {} e nunca acompanhou uma mutação desse fato",
                    consumidor.nome(),
                    fato.nome()
                ));
            }
            if !declarado && medido {
                divergencias.push(format!(
                    "{} acompanhou {} sem declarar consulta — fato de fase tratado como decisão compartilhada",
                    consumidor.nome(),
                    fato.nome()
                ));
            }
        }
    }
    assert!(
        divergencias.is_empty(),
        "declaração de fatos por consumidor divergente do medido:\n  {}",
        divergencias.join("\n  ")
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
        let mut mutacao = variantes(TERNARIA, Fato::Aridade).expect("aplicável");
        let _mutado = MutatedContract::install(mutacao.remove(0).tabela);
        assert_ne!(super::aridade(TERNARIA), antes);
    }
    assert_eq!(super::aridade(TERNARIA), antes);
}
