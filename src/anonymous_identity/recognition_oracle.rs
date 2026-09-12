//! Prova por EXECUÇÃO de que cada consumidor real deriva o reconhecimento da
//! autoridade U-05, em vez de decidir por conta própria (#655).
//!
//! # Por que esta camada existe
//!
//! O censo textual pega a regressão barata e nada além disso. Uma revisão
//! adversarial da #655 exibiu o limite exato: trocado o consumidor real de
//! `parser::expressoes` por `arg_name.starts_with(&["__anon_", "carinho_"]
//! .concat())`, a decisão local voltou, o censo não viu e o terminal continuou
//! verde.
//!
//! ```text
//! TEXTUAL_CENSUS        = SUPPLEMENTAL_ONLY
//! COUNTERFACTUAL_SPELLING = TERMINAL_GUARANTEE
//! ```
//!
//! # A propriedade provada
//!
//! ```text
//! canonical namespace spelling = A
//! test-only spelling           = B
//!
//! real consumer
//!   derives from the authority  -> follows the rename, observável idêntico
//!   keeps a local copy          -> still answers for A          -> RED
//! ```
//!
//! O domínio desta unidade é uma pergunta booleana sobre UMA constante que a
//! cunhagem e o reconhecimento compartilham. Então o contrafactual mínimo não é
//! uma tabela de células nem um oráculo bidirecional: é renomear o namespace e
//! exigir que o observável canônico não mude. Quem cunha passa a escrever `B`;
//! quem reconhece por conta própria continua procurando `A` e deixa de ver a
//! closure que ele mesmo acabou de receber.
//!
//! Isto **não** é o aparato de U-01/U-02 importado. Não há célula mutada, nem
//! testemunha por aresta, nem token opaco: uma substituição de grafia basta,
//! porque a autoridade inteira cabe numa string.
//!
//! # O oráculo não é o helper
//!
//! Nenhuma testemunha pergunta `is_anonymous_callable_name`. Cada uma observa o
//! que a fase PRODUZ — IR renderizada ou o texto do diagnóstico — e compara as
//! duas execuções depois de apagar a grafia do namespace dos dois lados. Um
//! consumidor que não acompanha produz observável diferente, e a diferença
//! sobrevive à normalização justamente porque não é a grafia que mudou.
//!
//! # Fronteira medida
//!
//! A cobertura é MEDIDA, não declarada: cada consumidor real recebeu, um a um,
//! uma cópia local escondida em `["__anon_", "carinho_"].concat()`, e o veredito
//! foi observado.
//!
//! ```text
//! consumidores reais de reconhecimento              = 16
//! detectados por prova de execução                  = 15
//!   destes, por estas testemunhas                   = 12
//!   destes, pelas testemunhas de `module_resolve`   =  2
//!   destes, pela testemunha do `pink_cli`           =  1
//! inalcançável a partir da fonte                    =  1
//! ```
//!
//! As duas testemunhas de `module_resolve` moram lá porque aqueles consumidores
//! vivem no caminho de projeção do grafo de módulos: a obrigação é a mesma, mas o
//! observável de ponta a ponta exige o carregador.
//!
//! A colheita de closures de default do `pink_cli` mora no módulo do binário, e
//! o `#[cfg(test)]` da biblioteca não a alcançava. Alcança agora, sem redesenho
//! de produção: `src/u05_cli_recognition_tests.rs` compila o MESMO arquivo
//! físico `src/pink_cli/modules.rs` dentro da biblioteca, sob `#[cfg(test)]` e
//! com o contexto de nomes que `main.rs` lhe dá, de modo que
//! `pinker_v0::anonymous_identity` resolva para ESTA autoridade e para ESTE
//! contrafactual. A entrada é a superfície existente `contexto_de_import` e o
//! observável é o pool real de templates. Medida a cópia local escondida ali, o
//! pool contrafactual chega vazio e a testemunha fica vermelha.
//!
//! O inalcançável é o disjunto anônimo da recusa de endereço cru em
//! `semantic::expressions`: `&IDENT` só carrega grafia de fonte, e a fronteira
//! léxica recusa o namespace reservado antes — nas duas posições, a de declaração
//! e a de `&`, como
//! `tests/issue_655_u05_anonymous_recognition_authority_tests.rs` fixa. Ele segue
//! derivando da autoridade; não sustenta comportamento.

use std::cell::Cell;

use crate::{ir, semantic};

/// Grafia contrafactual do namespace. Reservada como as outras formas do
/// compilador, para que a renomeação não vire, sem querer, um teste sobre
/// namespace não reservado.
pub(crate) const PREFIXO_CONTRAFACTUAL: &str = "__prova_anon_";

thread_local! {
    static CONTRAFACTUAL: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn contrafactual_ativo() -> bool {
    CONTRAFACTUAL.with(Cell::get)
}

/// Instala a grafia contrafactual na thread corrente e a desfaz no `Drop`.
pub(crate) struct AutoridadeContrafactual(());

impl AutoridadeContrafactual {
    pub(crate) fn instalar() -> Self {
        CONTRAFACTUAL.with(|ativo| {
            assert!(!ativo.get(), "contrafactual já instalado nesta thread");
            ativo.set(true);
        });
        Self(())
    }
}

impl Drop for AutoridadeContrafactual {
    fn drop(&mut self) {
        CONTRAFACTUAL.with(|ativo| ativo.set(false));
    }
}

/// Observável canônico de uma fonte: a IR renderizada, ou o texto do erro da
/// primeira fase que recusar.
fn observavel(fonte: &str) -> String {
    let tokens = match crate::lexer::Lexer::new(fonte).tokenize() {
        Ok(tokens) => tokens,
        Err(erro) => return format!("LEXER {erro:?}"),
    };
    let programa = match crate::parser::Parser::new(tokens).parse() {
        Ok(programa) => programa,
        Err(erro) => return format!("PARSER {erro:?}"),
    };
    if let Err(erro) = semantic::check_program(&programa) {
        return format!("SEMANTIC {erro:?}");
    }
    match ir::lower_program(&programa) {
        Ok(programa_ir) => format!("IR {}", ir::render_program(&programa_ir)),
        Err(erro) => format!("IR {erro:?}"),
    }
}

/// Apaga a grafia do namespace dos dois lados. O que sobrar é o que a fase
/// decidiu, e é isso que tem de coincidir.
fn sem_a_grafia(observavel: &str, prefixo: &str) -> String {
    observavel.replace(prefixo, "<ANON>")
}

/// Executa a fonte sob a autoridade canônica e sob a contrafactual, e exige que
/// as duas fases produzam a mesma decisão.
fn autoridade_move_o_consumidor(rotulo: &str, fonte: &str) {
    let canonico = observavel(fonte);
    assert!(
        canonico.contains(super::ANONYMOUS_CALLABLE_PREFIX),
        "{rotulo}: a fonte tem de exercer o namespace anônimo; observável:\n{canonico}"
    );

    let contrafactual = {
        let _guarda = AutoridadeContrafactual::instalar();
        assert_eq!(super::anonymous_callable_prefix(), PREFIXO_CONTRAFACTUAL);
        observavel(fonte)
    };
    assert_eq!(
        super::anonymous_callable_prefix(),
        super::ANONYMOUS_CALLABLE_PREFIX,
        "{rotulo}: o contrafactual tem de ser desfeito no Drop"
    );

    assert!(
        contrafactual.contains(PREFIXO_CONTRAFACTUAL),
        "{rotulo}: a cunhagem tem de seguir a autoridade; observável:\n{contrafactual}"
    );
    assert_eq!(
        sem_a_grafia(&canonico, super::ANONYMOUS_CALLABLE_PREFIX),
        sem_a_grafia(&contrafactual, PREFIXO_CONTRAFACTUAL),
        "{rotulo}: algum consumidor manteve a resposta da grafia antiga"
    );
}

/// Captura transitiva por closure aninhada — `ast::expand_transitive_free_idents`
/// é quem decide expandir em vez de capturar.
const CAPTURA_ANINHADA: &str = "\
pacote main;

carinho principal() -> bombom {
    nova base: bombom = 4;
    nova externa: carinho(bombom) -> bombom = carinho(a: bombom) -> bombom {
        nova interna: carinho(bombom) -> bombom = carinho(b: bombom) -> bombom {
            mimo base + b;
        };
        mimo interna(a);
    };
    mimo externa(1);
}
";

/// Closure usada como VALOR — a resolução tardia de `semantic::expressions` e
/// de `ir::lowering` só acontece para quem reconhece o nome sintético.
const CLOSURE_COMO_VALOR: &str = "\
pacote main;

carinho principal() -> bombom {
    nova fator: bombom = 3;
    nova dobrar: carinho(bombom) -> bombom = carinho(x: bombom) -> bombom {
        mimo x * fator;
    };
    mimo dobrar(7);
}
";

/// Chamada imediata — a closure nunca é resolvida como valor, e as duas
/// varreduras de fim de passagem (`semantic` e `ir::context`) são as que a
/// alcançam.
const CHAMADA_IMEDIATA: &str = "\
pacote main;

carinho principal() -> bombom {
    nova valor: bombom = carinho(v: bombom) -> bombom {
        mimo v + 1;
    }(41);
    mimo valor;
}
";

/// `carinho` anônimo passado DIRETO como argumento — `parser::expressoes`
/// decide especializar estaticamente.
const ARGUMENTO_DIRETO: &str = "\
pacote main;

carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
    mimo f(x);
}

carinho principal() -> bombom {
    mimo aplicar(carinho(v: bombom) -> bombom {
        mimo v * 2;
    }, 21);
}
";

/// `nova nome: carinho(...) = <anônimo>` — o alias de parser da Fase 238.
const ALIAS_DE_FUNCAO_LOCAL: &str = "\
pacote main;

carinho principal() -> bombom {
    nova somar: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom {
        mimo v + 2;
    };
    mimo somar(40);
}
";

/// Corpo default de `trato` com closure — `parser::mod` reúne os templates e
/// materializa uma cópia por relação, com as duas proveniências da #567.
const DEFAULT_DE_TRATO_COM_CLOSURE: &str = "\
pacote main;

carinho apoio() -> bombom { mimo 3; }

trato Marca {
    carinho marcar(valor: si) -> bombom {
        nova base: bombom = 5;
        nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom {
            mimo apoio() + base + v;
        };
        mimo f(2);
    }
}

impl Marca para bombom {}

carinho principal() -> bombom {
    nova x: bombom = 10;
    mimo x.marcar();
}
";

/// Diagnóstico de closure — `semantic::function_name_for_diagnostic` decide
/// esconder a identidade gerada atrás de `<anônima>`.
const DIAGNOSTICO_DE_CLOSURE: &str = "\
pacote main;

carinho principal() -> bombom {
    nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom {
        nova z: bombom = v;
    };
    mimo f(1);
}
";

#[test]
fn captura_transitiva_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("captura aninhada", CAPTURA_ANINHADA);
}

#[test]
fn closure_como_valor_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("closure como valor", CLOSURE_COMO_VALOR);
}

#[test]
fn chamada_imediata_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("chamada imediata", CHAMADA_IMEDIATA);
}

#[test]
fn argumento_direto_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("argumento direto", ARGUMENTO_DIRETO);
}

#[test]
fn alias_de_funcao_local_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("alias de função local", ALIAS_DE_FUNCAO_LOCAL);
}

#[test]
fn default_de_trato_com_closure_acompanha_a_autoridade() {
    autoridade_move_o_consumidor("default de trato", DEFAULT_DE_TRATO_COM_CLOSURE);
}

/// O diagnóstico é observável de erro, então a testemunha compara o texto
/// recusado e exige que `<anônima>` continue substituindo a identidade gerada.
#[test]
fn diagnostico_de_closure_acompanha_a_autoridade() {
    let canonico = observavel(DIAGNOSTICO_DE_CLOSURE);
    assert!(
        canonico.starts_with("SEMANTIC") && canonico.contains("<anônima>"),
        "a fonte tem de recusar nomeando a closure de forma anônima: {canonico}"
    );
    let contrafactual = {
        let _guarda = AutoridadeContrafactual::instalar();
        observavel(DIAGNOSTICO_DE_CLOSURE)
    };
    assert_eq!(
        canonico, contrafactual,
        "o diagnóstico manteve a resposta da grafia antiga"
    );
}

/// A grafia contrafactual é reservada como qualquer outra forma que o
/// compilador materializa: sem isso a renomeação testaria também a fronteira de
/// namespace, e uma falha não distinguiria as duas causas.
#[test]
fn a_grafia_contrafactual_e_identidade_gerada() {
    let _guarda = AutoridadeContrafactual::instalar();
    let nome = super::anonymous_callable_name(&crate::source_origin::SourceOrigin::Root, 0);
    assert!(nome.starts_with(PREFIXO_CONTRAFACTUAL));
    assert!(crate::native_symbol::is_compiler_generated(&nome));
}
