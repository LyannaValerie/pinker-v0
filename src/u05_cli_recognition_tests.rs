//! U-05 — prova durável, por EXECUÇÃO, de que o consumidor de reconhecimento
//! do `pink_cli` deriva a decisão da autoridade canônica.
//!
//! # Por que este adaptador existe
//!
//! O consumidor `colher_closures_de_default` mora em `src/pink_cli/modules.rs`,
//! que `src/main.rs` incorpora por `#[path]`. O oráculo contrafactual da
//! biblioteca (`src/anonymous_identity/recognition_oracle.rs`) não alcançava o
//! binário, e a cobertura da #655 ficava com um consumidor alcançável sem prova
//! durável: uma cópia local escondida podia voltar ali e o terminal seguia
//! verde, porque o censo textual é SUPLEMENTAR por desenho.
//!
//! Este módulo compila o MESMO arquivo físico dentro da crate de biblioteca,
//! exclusivamente sob `#[cfg(test)]`, fornecendo o contexto de nomes que
//! `modules.rs` recebe hoje de `main.rs` — inclusive `use crate as pinker_v0`,
//! de modo que `pinker_v0::anonymous_identity` resolva para a MESMA autoridade
//! e para a MESMA instância thread-local do contrafactual. Não há cópia,
//! reimplementação nem segunda política: o que executa é o consumidor real.
//!
//! ```text
//! SAME_PHYSICAL_SOURCE_FILE   = TRUE
//! SAME_LIBRARY_AUTHORITIES    = TRUE
//! SAME_COUNTERFACTUAL_STATE   = TRUE
//! ```
//!
//! # O que é provado
//!
//! A entrada é a superfície existente `modules::contexto_de_import`, que já
//! chega causalmente ao coletor privado. O observável é o pool real
//! `ContextoDeImport::closures_de_default_importadas`.
//!
//! ```text
//! consumidor deriva da autoridade -> pool acompanha a renomeação
//! consumidor com cópia local      -> pool perde a closure -> RED
//! ```
//!
//! O gabarito vem da fixture conhecida — um trato com UM corpo default que
//! contém UMA closure, e uma função ordinária de controle negativo — e nunca do
//! reconhecedor auditado. A normalização apaga SÓ a grafia do namespace; nem
//! ausência, nem cardinalidade, nem conteúdo estrutural são normalizados.
//!
//! Esta prova é do código-fonte real do módulo do CLI dentro do harness da
//! biblioteca; ela não substitui
//! `tests/issue_517_imported_trait_impl_tests.rs::
//! default_importado_com_closure_sintetica_compoe_pela_origem`, que continua
//! provando a ligação e o comportamento do processo `pink`.

use crate as pinker_v0;

use crate::anonymous_identity::{
    AutoridadeContrafactual, ANONYMOUS_CALLABLE_PREFIX, PREFIXO_CONTRAFACTUAL,
};
use crate::error::PinkerError;
use crate::generic_identity::GenericOrigin;
use crate::lexer::Lexer;
use crate::module_graph::ModuleGraph;
use crate::module_resolve;
use crate::parser::{ContextoDeImport, Parser};
use crate::source_map::{SourceId, SourceMap};
use crate::source_origin::SourceOrigin;
use crate::token::{Span, Token};
use crate::{ast, semantic};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// O arquivo REAL do consumidor, incorporado como `main.rs` o incorpora. O
// `allow` é local a esta segunda inclusão: o harness exerce `contexto_de_import`
// e Rust compila o módulo inteiro, então o resto da unidade MAIN-3 fica sem
// chamador AQUI — e só aqui.
#[allow(dead_code)]
#[path = "pink_cli/modules.rs"]
mod modules;

/// Unidade declarante: UM trato com corpo default que contém UMA closure, e uma
/// função ordinária de topo que serve de controle negativo.
const MODULO_DECLARANTE: &str = "\
pacote m655_cli;

carinho apoio655() -> bombom { mimo 7; }

trato ComClosure655 {
    carinho calcular(valor: bombom) -> bombom {
        nova f: carinho() -> bombom = carinho () -> bombom { mimo apoio655(); };
        mimo valor + f();
    }
}
";

/// Raiz que traz o trato pela forma seletiva — o suficiente para
/// `contexto_de_import` ler o módulo vizinho e colher o pool.
const RAIZ: &str = "\
pacote main;
trazer m655_cli.ComClosure655;

impl ComClosure655 para bombom {}

carinho principal() -> bombom {
    nova x: bombom = 10;
    mimo x.calcular();
}
";

/// Nome do módulo vizinho, tal como a raiz o escreve.
const MODULO: &str = "m655_cli";

/// Função ordinária da unidade declarante. Controle negativo: ela é item de
/// topo do MESMO programa que o coletor percorre e não pode entrar no pool.
const FUNCAO_ORDINARIA: &str = "apoio655";

/// Trato que a raiz traz.
const TRATO: &str = "ComClosure655";

/// Um corpo default, uma closure.
const TEMPLATES_ESPERADOS: usize = 1;

/// Diretório exclusivo da execução, removido no `Drop`.
struct DiretorioDaFixture {
    caminho: PathBuf,
}

impl Drop for DiretorioDaFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.caminho);
    }
}

fn fixture(rotulo: &str) -> DiretorioDaFixture {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("relógio monotônico desde a época")
        .as_nanos();
    let caminho = std::env::temp_dir().join(format!(
        "pinker_u05_cli_{rotulo}_{}_{agora}",
        std::process::id()
    ));
    fs::create_dir_all(&caminho).expect("criar diretório da fixture");
    fs::write(
        caminho.join(format!("{MODULO}.pink")),
        MODULO_DECLARANTE.as_bytes(),
    )
    .expect("escrever módulo declarante");
    DiretorioDaFixture { caminho }
}

/// Fatos observados de UMA execução completa de `contexto_de_import`.
struct FatosDoContexto {
    import_incompleto: bool,
    trato_presente: bool,
    cardinalidade_do_pool: usize,
    funcao_ordinaria_no_pool: bool,
    /// Toda chave do pool foi cunhada sob a grafia VIGENTE nesta execução. É o
    /// que prova que a autoridade realmente se moveu, e não que o contrafactual
    /// passou despercebido.
    chaves_sob_a_grafia_vigente: bool,
    /// Chaves do pool com a grafia do namespace apagada.
    chaves_normalizadas: Vec<String>,
    /// Renderização estrutural de cada template, com a grafia apagada.
    templates_normalizados: Vec<String>,
}

/// Apaga SÓ a grafia variável do namespace. O que sobrevive é decisão.
fn sem_a_grafia(texto: &str, prefixo: &str) -> String {
    texto.replace(prefixo, "<ANON>")
}

/// Reconstrói TUDO — leitura do arquivo, tokenização, parse do vizinho e
/// montagem do contexto — e passa causalmente pelo consumidor real.
fn executar(base_dir: &Path, prefixo: &str) -> FatosDoContexto {
    let tokens = Lexer::new(RAIZ)
        .tokenize()
        .expect("a raiz da fixture tem de tokenizar");
    let contexto: ContextoDeImport = modules::contexto_de_import(&tokens, base_dir);

    let mut chaves: Vec<String> = contexto
        .closures_de_default_importadas
        .keys()
        .map(|chave| sem_a_grafia(chave, prefixo))
        .collect();
    chaves.sort();

    let mut templates: Vec<(String, String)> = contexto
        .closures_de_default_importadas
        .iter()
        .map(|(chave, decl)| {
            (
                sem_a_grafia(chave, prefixo),
                sem_a_grafia(&format!("{decl:?}"), prefixo),
            )
        })
        .collect();
    templates.sort();

    FatosDoContexto {
        import_incompleto: contexto.import_incompleto,
        trato_presente: contexto.tratos_importados.contains_key(TRATO),
        cardinalidade_do_pool: contexto.closures_de_default_importadas.len(),
        funcao_ordinaria_no_pool: contexto
            .closures_de_default_importadas
            .contains_key(FUNCAO_ORDINARIA),
        chaves_sob_a_grafia_vigente: contexto
            .closures_de_default_importadas
            .keys()
            .all(|chave| chave.starts_with(prefixo)),
        chaves_normalizadas: chaves,
        templates_normalizados: templates.into_iter().map(|(_, decl)| decl).collect(),
    }
}

/// Pré-condições que impedem falso verde: leitura, parse e import têm de ter
/// funcionado, e o pool tem de ser o da fixture — não um vazio parecido com um
/// vazio.
fn exigir_fixture_discriminante(rotulo: &str, fatos: &FatosDoContexto) {
    assert!(
        !fatos.import_incompleto,
        "{rotulo}: a leitura do módulo vizinho falhou; sem isso o pool vazio não prova nada"
    );
    assert!(
        fatos.trato_presente,
        "{rotulo}: o trato `{TRATO}` tem de chegar pelo import"
    );
    assert_eq!(
        fatos.cardinalidade_do_pool, TEMPLATES_ESPERADOS,
        "{rotulo}: a fixture declara {TEMPLATES_ESPERADOS} closure no corpo default; \
         chaves observadas: {:?}",
        fatos.chaves_normalizadas
    );
    assert!(
        !fatos.funcao_ordinaria_no_pool,
        "{rotulo}: `{FUNCAO_ORDINARIA}` é função ordinária e não pode entrar no pool"
    );
    assert!(
        fatos.chaves_sob_a_grafia_vigente,
        "{rotulo}: a cunhagem tem de seguir a autoridade vigente; chaves: {:?}",
        fatos.chaves_normalizadas
    );
    let template = &fatos.templates_normalizados[0];
    assert!(
        template.contains(FUNCAO_ORDINARIA),
        "{rotulo}: o template tem de ser o corpo que chama `{FUNCAO_ORDINARIA}`: {template}"
    );
}

/// A testemunha durável: o consumidor real do CLI acompanha a autoridade
/// canônica de reconhecimento.
///
/// Se ele mantiver uma cópia local da regra — ainda que escondida em `concat`,
/// fatiamento ou helper —, sob a grafia contrafactual ele deixa de reconhecer a
/// closure que a cunhagem acabou de produzir, e o pool chega vazio.
#[test]
fn colheita_de_closures_de_default_do_cli_acompanha_a_autoridade() {
    let canonica = fixture("canonica");
    let fatos_canonicos = executar(&canonica.caminho, ANONYMOUS_CALLABLE_PREFIX);
    exigir_fixture_discriminante("canônica", &fatos_canonicos);

    // Nada da execução canônica é reaproveitado: outro diretório, outra leitura,
    // outro parse, outro contexto.
    let contrafactual = fixture("contrafactual");
    let fatos_contrafactuais = {
        let _guarda = AutoridadeContrafactual::instalar();
        executar(&contrafactual.caminho, PREFIXO_CONTRAFACTUAL)
    };

    let restaurado = crate::anonymous_identity::anonymous_callable_name(&SourceOrigin::Root, 0);
    assert!(
        restaurado.contains(ANONYMOUS_CALLABLE_PREFIX)
            && !restaurado.contains(PREFIXO_CONTRAFACTUAL),
        "o contrafactual tem de ser desfeito no Drop; grafia corrente: {restaurado}"
    );

    exigir_fixture_discriminante("contrafactual", &fatos_contrafactuais);
    assert_eq!(
        fatos_canonicos.chaves_normalizadas, fatos_contrafactuais.chaves_normalizadas,
        "o consumidor do CLI manteve a resposta da grafia antiga"
    );
    assert_eq!(
        fatos_canonicos.templates_normalizados, fatos_contrafactuais.templates_normalizados,
        "o template colhido mudou de forma além da grafia do namespace"
    );
}
