//! Autoridade única das superfícies falíveis — falha operacional como valor.
//!
//! Antes desta camada, uma falha operacional recuperável (arquivo ausente,
//! spawn impossível, texto externo malformado) terminava o programa nos dois
//! backends por caminhos diferentes:
//!
//! ```text
//! interpretador : runtime_err(&str) -> PinkerError::Runtime  -> encerra
//! nativo        : erro_fatal(&str)  -> eprintln + exit(1)     -> encerra
//! ```
//!
//! O programa não tinha como reagir: a causa existia apenas como texto no
//! stderr de um processo já morto.
//!
//! Este módulo é a **única** autoridade sobre quais intrínsecas devolvem a
//! falha como valor, qual especialização de `Resultado<T,E>` cada uma produz e
//! qual símbolo do runtime nativo a implementa. Parser, semântica, validadores,
//! interpretador e backend derivam tudo daqui — nunca de um `match` local por
//! nome de intrínseca espalhado por camada.
//!
//! O que este módulo **não** faz:
//!
//! - não define semântica de propagação. `tentar`/`propagar`/`propagar?`
//!   continuam sendo desugaring puro do parser sobre a forma do leque, cegos à
//!   expressão que produziu o valor. A origem da operação não pode definir
//!   propagação;
//! - não cria uma segunda representação de resultado. O valor devolvido é o
//!   `Resultado<T,E>` predeclarado (Fase 241), monomorfizado pelo mesmo caminho
//!   dos leques genéricos do usuário;
//! - não introduz taxonomia de erros. A carga de falha é `verso`, a mesma
//!   família que os dois backends já usam para descrever a causa;
//! - não converte falha interna em valor. Bug de programa e violação de
//!   invariante continuam fatais.

use crate::ast::Type;
use crate::token::Position;
use crate::token::Span;

// @pinker-nav:start falha.operacional.superficies
// @pinker-nav:domain erros
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única das superfícies falíveis da Parte B: `CargaResultado` classifica a carga de sucesso/falha em uma palavra (`bombom`) ou texto (`verso`), `SuperficieFalivel` liga o nome da intrínseca à especialização de `Resultado<T,E>` que ela devolve e ao símbolo do runtime nativo que a implementa, e `SUPERFICIES_FALIVEIS` é a lista fechada consultada por parser, semântica, validadores, interpretador e backend. As tags `TAG_OK`/`TAG_ERRO` espelham a ordem de declaração do leque predeclarado e são fixadas por teste; nenhuma camada redescobre esses fatos por conta própria.

/// Tag (discriminante) da variante de sucesso de `Resultado<T,E>`.
///
/// O discriminante é o índice de declaração da variante
/// (ver a construção de `enum_variants` na IR), e o template predeclarado
/// declara `Ok` antes de `Erro`. Esses dois fatos são do compilador, não do
/// usuário, mas o acoplamento é explícito e está fixado por teste: reordenar as
/// variantes do predeclarado quebra a suíte em vez de corromper valores em
/// silêncio.
pub const TAG_OK: u64 = 0;

/// Tag (discriminante) da variante de falha de `Resultado<T,E>`.
pub const TAG_ERRO: u64 = 1;

/// Nome do leque predeclarado especializado por estas superfícies.
pub const LEQUE_RESULTADO: &str = "Resultado";

/// Classe operacional de uma carga de `Resultado<T,E>`.
///
/// Deliberadamente mínima: a Parte B só precisa distinguir uma palavra inteira
/// de um texto possuído. Ampliar esta enumeração é decisão de outra frente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargaResultado {
    /// Valor imediato de uma palavra.
    Bombom,
    /// Texto possuído pelo valor que o transporta.
    Verso,
}

impl CargaResultado {
    /// Tipo AST correspondente, com o span do uso.
    pub fn tipo(self, span: Span) -> Type {
        match self {
            CargaResultado::Bombom => Type::Bombom(span),
            CargaResultado::Verso => Type::Verso(span),
        }
    }

    /// Chave usada na composição do nome monomórfico do leque.
    ///
    /// Espelha `generic_type_key` do parser para as duas variantes suportadas.
    pub fn chave(self) -> &'static str {
        match self {
            CargaResultado::Bombom => "bombom",
            CargaResultado::Verso => "verso",
        }
    }

    /// Decide se um tipo já checado satisfaz esta carga.
    ///
    /// Existe para que a semântica não redescubra por `matches!` local qual é o
    /// tipo aceito por cada superfície: a exigência viaja na autoridade.
    pub fn aceita(self, ty: &Type) -> bool {
        matches!(
            (self, ty),
            (CargaResultado::Bombom, Type::Bombom(_)) | (CargaResultado::Verso, Type::Verso(_))
        )
    }
}

/// Identidade operacional de uma superfície falível.
///
/// É o que o interpretador despacha. Sem isto, o dispatch hospedado teria de
/// reconhecer a superfície pelo **nome público**, duplicando em `interpreter.rs`
/// a autoridade lexical que este módulo existe para concentrar — o nome público
/// passaria a ser decidido em dois lugares.
///
/// A variante nomeia a operação, não a intrínseca: o nome público continua
/// declarado uma única vez, em [`SUPERFICIES_FALIVEIS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperacaoFalivel {
    /// Lê um arquivo inteiro a partir de um caminho.
    LerArquivoPorCaminho,
    /// Executa um processo e observa seu código de saída.
    ExecutarProcesso,
    /// Converte texto externo em número.
    ConverterVersoParaBombom,
}

/// Uma superfície que devolve falha operacional recuperável como valor.
#[derive(Debug, Clone, Copy)]
pub struct SuperficieFalivel {
    /// Nome público da intrínseca.
    ///
    /// Declarado aqui e **somente** aqui dentro da crate do compilador. Nenhuma
    /// camada reconhece a superfície por este texto: quem despacha usa
    /// [`SuperficieFalivel::operacao`].
    pub intrinseca: &'static str,
    /// Identidade operacional despachada pelo interpretador.
    pub operacao: OperacaoFalivel,
    /// Tipo do único argumento aceito.
    ///
    /// A Parte B só abre superfícies de um argumento; a exigência mora aqui em
    /// vez de ser reafirmada por cada camada. Ampliar assinaturas é decisão de
    /// outra frente, não desta.
    pub argumento: CargaResultado,
    /// Carga da variante `Ok`.
    pub sucesso: CargaResultado,
    /// Carga da variante `Erro`. Hoje sempre `verso`.
    pub falha: CargaResultado,
    /// Símbolo do runtime nativo que implementa a superfície.
    pub simbolo_runtime: &'static str,
    /// Superfície histórica correspondente, preservada sem alteração.
    ///
    /// Registrada para que a compatibilidade seja um fato consultável e não uma
    /// promessa de texto: a superfície histórica continua existindo, com a
    /// mesma assinatura e o mesmo comportamento de aborto.
    pub historica: &'static str,
}

impl SuperficieFalivel {
    /// Nome monomórfico do leque devolvido, p. ex.
    /// `__gen_leque_Resultado_verso_verso`.
    ///
    /// Composto exatamente como `Parser::generic_enum_name` compõe o nome de
    /// qualquer especialização escrita pelo usuário.
    pub fn leque_monomorfico(&self) -> String {
        format!(
            "__gen_leque_{}_{}_{}",
            LEQUE_RESULTADO,
            self.sucesso.chave(),
            self.falha.chave()
        )
    }

    /// Argumentos de tipo da especialização, na ordem `<T, E>`.
    pub fn argumentos_de_tipo(&self, span: Span) -> Vec<Type> {
        vec![self.sucesso.tipo(span), self.falha.tipo(span)]
    }

    /// Tipo devolvido pela intrínseca, do ponto de vista da semântica.
    pub fn tipo_de_retorno(&self, span: Span) -> Type {
        Type::Enum {
            name: self.leque_monomorfico(),
            span,
        }
    }
}

/// Span sintético usado quando a materialização do tipo não vem de uma posição
/// de fonte do usuário. Mesma convenção do template predeclarado: posição 0:0
/// nunca finge uma localização real.
pub fn span_sintetico() -> Span {
    Span::single(Position::new(0, 0))
}

/// Lista fechada das superfícies falíveis.
///
/// Três domínios independentes atravessam o mesmo fluxo de erro: filesystem,
/// processo/spawn e parsing em tempo de execução. A independência é o ponto —
/// uma única operação falível não provaria que o mecanismo é geral.
pub const SUPERFICIES_FALIVEIS: &[SuperficieFalivel] = &[
    // Filesystem: leitura de arquivo inteiro por caminho.
    // Não abre handle visível ao usuário, logo uma falha não pode deixar
    // recurso parcialmente vivo nem sem dono.
    SuperficieFalivel {
        intrinseca: "ler_arquivo_resultado",
        operacao: OperacaoFalivel::LerArquivoPorCaminho,
        argumento: CargaResultado::Verso,
        sucesso: CargaResultado::Verso,
        falha: CargaResultado::Verso,
        simbolo_runtime: "pinker_arquivo_ler_caminho_resultado",
        historica: "ler_arquivo_verso",
    },
    // Processo: spawn de um executável.
    // A falha recuperável é a impossibilidade de executar (ausente, sem
    // permissão). O código de saída do filho é valor de sucesso, não falha.
    SuperficieFalivel {
        intrinseca: "executar_processo_resultado",
        operacao: OperacaoFalivel::ExecutarProcesso,
        argumento: CargaResultado::Verso,
        sucesso: CargaResultado::Bombom,
        falha: CargaResultado::Verso,
        simbolo_runtime: "pinker_processo_executar_resultado",
        historica: "executar_processo",
    },
    // Parsing em tempo de execução: texto externo para número.
    // Domínio independente dos dois anteriores e independente de JSON.
    SuperficieFalivel {
        intrinseca: "verso_para_bombom_resultado",
        operacao: OperacaoFalivel::ConverterVersoParaBombom,
        argumento: CargaResultado::Verso,
        sucesso: CargaResultado::Bombom,
        falha: CargaResultado::Verso,
        simbolo_runtime: "pinker_verso_para_bombom_resultado",
        historica: "verso_para_bombom",
    },
];

/// Resolve uma superfície falível pelo nome da intrínseca.
pub fn superficie(nome: &str) -> Option<&'static SuperficieFalivel> {
    SUPERFICIES_FALIVEIS
        .iter()
        .find(|superficie| superficie.intrinseca == nome)
}

/// Nomes das intrínsecas falíveis, para os validadores de IR.
pub fn nomes() -> impl Iterator<Item = &'static str> {
    SUPERFICIES_FALIVEIS
        .iter()
        .map(|superficie| superficie.intrinseca)
}

// @pinker-nav:end falha.operacional.superficies
