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
//! Um consumidor que pergunte à autoridade muda junto. Um consumidor com
//! decisão local independente continua respondendo o valor ANTIGO, e o teste
//! fica vermelho — **independentemente de arquivo, posição, helper, macro,
//! operador, alias ou forma textual**, porque o oráculo nunca olha o texto.
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
//! **O universo de consumidores é descoberto, não declarado.** A própria
//! autoridade anota, sob `cfg(test)`, o arquivo de quem a consulta
//! ([`super::registro`]). A escrituração deste arquivo é conferida contra essa
//! descoberta e contra o piso histórico de oito decisores: tirar um consumidor
//! daqui deixa o teste vermelho, porque a descoberta continua achando o
//! consumidor real.
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

use super::contract_seam::MutatedContract;
use super::registro::Gravacao;
use super::{
    InternalOperands, InternalOperation, InternalOperationFamily, InternalResult, MapOperandRole,
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
use crate::ir::MapKeyIR;
use crate::ir::{self, ProgramIR, TypeIR};
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
                    let mut mutados = params.to_vec();
                    mutados.push(TypeIR::Bombom);
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
    (Consumidor::Semantic, &[Fato::Existencia, Fato::Aridade]),
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
        Consumidor::Semantic,
        Fato::Operandos,
        "CARVE-OUT DECLARADA POR TC-01: a semântica decide a CLASSE do operando na representação \
         `Type` desta fase, que não é `TypeIR`. F3 tem dono único na representação IR; o espelho \
         de `Type` em `semantic/calls` é pré-existente ao baseline e não foi consolidado",
    ),
    (
        Consumidor::Semantic,
        Fato::Resultado,
        "CARVE-OUT DECLARADA POR TC-01: mesma razão de F3 — o resultado devolvido pela semântica é \
         um `Type` da fase, não o `TypeIR` da autoridade",
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
