//! Guardião estrutural da decomposição física da checagem semântica
//! (#619, unidade SEM-1 do inventário da #601).
//!
//! `src/semantic.rs` é a autoridade da fase semântica. A #619 desce a região
//! `semantic.chamadas.despacho` inteira para `src/semantic/calls.rs` sem
//! dividir essa autoridade: o estado (`SemanticChecker`), a ordem das duas
//! passagens, os escopos, o sistema de tipos e as demais famílias continuam no
//! pai, e o pai continua sendo um arquivo — não virou `mod.rs`.
//!
//! O corte é o primeiro de `semantic.rs` e por isso paga a reescrita do
//! guardião C2, exatamente como o inventário da #601 previu. Ele cria formas
//! de cegueira silenciosa que este arquivo fecha, e nada mais:
//!
//! 1. um oráculo textual que continuasse lendo só `src/semantic.rs` seguiria
//!    verde e pararia de observar o irmão — a OG-1 da #601. O único ponto em
//!    que a semântica consulta `method_dispatch::select_impl_method` desceu
//!    junto, então a cegueira cairia justamente sobre C2. Os censos de C2, de
//!    C5, da #505, da #514 e da D6 passaram a ler
//!    `fonte_de_modulo::semantic()`, e o teste abaixo prova que a lista lida
//!    por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ou ficar para trás no pai;
//! 3. o corte podia promover visibilidade ou mudar a superfície pública do
//!    módulo. `check_call_expr` é o único símbolo que o pai chama e o único
//!    que passou de privado a `pub(super)`; nenhum item do corte era `pub`,
//!    então nenhuma reexportação é devida e nenhuma pode aparecer.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre a regra de despacho, que é de `src/method_dispatch.rs`.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{semantic, SEMANTIC_ARQUIVOS};
use rust_source::codigo_executavel;

/// A região que a SEM-1 moveu, e o irmão onde passa a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[("semantic.chamadas.despacho", "calls.rs")];

/// As regiões que o corte deixou onde estavam. As duas primeiras são as
/// vizinhas imediatas do span da SEM-1 — a que vem antes e a que vem depois —,
/// e são elas que ficariam vermelhas se o corte tivesse escorregado uma região
/// para qualquer lado.
const REGIOES_RETIDAS: &[&str] = &[
    "semantic.expressoes.verificacao",
    "semantic.modulos.validacao-local",
    "semantic.identificadores.namespace-produtor-de-simbolo",
    "semantic.importacoes.familias",
    "semantic.tipos.sistema",
    "semantic.escopos.variaveis",
    "semantic.programa.duas-passagens",
    "semantic.tratos.contratos",
    "semantic.funcoes.verificacao",
    "semantic.comandos.verificacao",
    "semantic.unioes.encaixe",
    "semantic.fluxo.retornos",
];

/// As definições que a SEM-1 moveu inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão, e nenhuma deixada para trás no pai.
const DEFINICOES_MOVIDAS: &[(&str, &str)] = &[
    ("fn check_trait_object_method_call(", "calls.rs"),
    ("fn resolve_impl_method(", "calls.rs"),
    ("fn resolve_qualified_impl_method(", "calls.rs"),
    ("fn generic_map_monomorphic_callee(", "calls.rs"),
    ("fn check_named_function_call(", "calls.rs"),
    ("fn check_call_expr(", "calls.rs"),
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["calls.rs"];

/// Os itens `pub` que cada irmão pode ter. A lista é exaustiva e vazia: nenhum
/// item do corte era `pub` antes do move, então nenhum pode ser depois.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[("calls.rs", &[])];

/// A visibilidade restrita que cada irmão pode ter, exaustiva.
///
/// É o custo Rust inteiro da unidade, medido pelo inventário da #601 como
/// `1 pub(super)` e nenhum `pub(crate)`: `check_call_expr` é o único símbolo do
/// corte que o pai chama — de `semantic.expressoes.verificacao` —, e por isso o
/// único que precisa ser visível para ele.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] =
    &[("calls.rs", &["pub(super) fn check_call_expr("])];

/// A superfície pública do módulo, congelada item a item.
///
/// É o contrato `PUBLIC_PATHS_BEFORE == AFTER` da #619 na forma que um teste
/// consegue observar. O corte não contém nenhum item público, então esta lista
/// é exatamente a de antes do move.
const API_PUBLICA_CONGELADA: &[&str] = &[
    "SemanticChecker",
    "check_module_unit",
    "check_program",
    "check_program_composto",
    "validar_namespace_pinker_owned",
    "validate_builtin_family_import",
    "validate_family_import_collision",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/semantic")
}

fn fonte(nome: &str) -> &'static str {
    SEMANTIC_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

fn pai() -> &'static str {
    fonte("semantic.rs")
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo que lê o módulo por `fonte_de_modulo` — e o de C2 é um deles.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/semantic/ legível")
        .map(|entrada| entrada.expect("entrada de diretório").path())
        .filter(|caminho| caminho.extension().is_some_and(|ext| ext == "rs"))
        .map(|caminho| {
            caminho
                .file_name()
                .expect("nome de arquivo")
                .to_str()
                .expect("utf-8")
                .to_string()
        })
        .collect();
    let declarados: BTreeSet<String> = SEMANTIC_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "semantic.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/semantic/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate e o que desceu deixa de existir sem
/// que nada fique vermelho. É a sensitivity M1 da #619.
#[test]
fn o_pai_inclui_o_irmao() {
    let codigo = codigo_executavel(pai());
    for (nome, _) in SEMANTIC_ARQUIVOS {
        if *nome == "semantic.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/semantic.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
        // O filho é detalhe físico, não caminho público. `pub mod calls;`
        // criaria `pinker_v0::semantic::calls::*` sem apagar nenhum dos itens
        // congelados — ampliação de superfície que o censo de itens não veria
        // sozinho.
        assert_eq!(
            codigo.matches(&format!("pub {declaracao}")).count(),
            0,
            "src/semantic.rs tornou o irmão `{modulo}` um caminho público"
        );
    }
    // O pai continua sendo um arquivo: a #619 mantém a forma que a #608
    // decidiu e a #610, a #612, a #615 e a #617 mantiveram. Convertê-lo em
    // `mod.rs` moveria o campo `file` das treze regiões do arquivo e faria as
    // treze projeções FROZEN pararem com E-SNAP-PATH-ALTERADO.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. São as sensitivities M2 e M3 da #619.
#[test]
fn cada_regiao_e_cada_definicao_movida_aparece_uma_vez_no_arquivo_certo() {
    let modulo = semantic();
    for (chave, arquivo) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/semantic/{arquivo}"
        );
        assert!(
            !pai().contains(&marcador),
            "a região {chave} ficou para trás em src/semantic.rs"
        );
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            pai().contains(&marcador),
            "a região {chave} não é da SEM-1 e deveria continuar em src/semantic.rs"
        );
    }

    let codigo = codigo_executavel(&modulo);
    let codigo_do_pai = codigo_executavel(pai());
    for (definicao, arquivo) in DEFINICOES_MOVIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo semantic"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            0,
            "a implementação de `{definicao}` ficou para trás em src/semantic.rs"
        );
        assert_eq!(
            codigo_executavel(fonte(arquivo)).matches(definicao).count(),
            1,
            "`{definicao}` deveria morar em src/semantic/{arquivo}"
        );
    }
}

fn conferir_regiao_unica(modulo: &str, chave: &str) {
    for marcador in [
        format!("// @pinker-nav:start {chave}"),
        format!("// @pinker-nav:end {chave}"),
    ] {
        assert_eq!(
            modulo.matches(&marcador).count(),
            1,
            "`{marcador}` deveria aparecer exatamente uma vez no módulo semantic"
        );
    }
}

/// A decomposição é física: não promove nada. Cada irmão tem duas listas
/// exaustivas — o que pode ser `pub` e o que pode ter visibilidade restrita —,
/// e nada mais. É a sensitivity M4 da #619.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in SEMANTIC_ARQUIVOS {
        if *nome == "semantic.rs" {
            continue;
        }
        let codigo = codigo_executavel(fonte);
        let autorizados = itens_autorizados(PUB_AUTORIZADO, nome, "`pub`");
        assert_eq!(
            codigo.matches("pub ").count(),
            autorizados.len(),
            "src/semantic/{nome} tem mais itens `pub` do que o corte previa"
        );
        let restritos = itens_autorizados(PUB_RESTRITO_AUTORIZADO, nome, "visibilidade restrita");
        assert_eq!(
            codigo.matches("pub(").count(),
            restritos.len(),
            "src/semantic/{nome} tem mais visibilidade restrita do que o corte previa"
        );
        assert_eq!(
            codigo.matches("pub(crate)").count(),
            0,
            "src/semantic/{nome} criou visibilidade de crate; o corte previa zero"
        );
        for item in autorizados.iter().chain(restritos.iter()) {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/semantic/{nome} deveria conter `{item}` exatamente uma vez"
            );
        }
    }
}

/// A lista exaustiva de um irmão, ou o panic que recusa um irmão não
/// declarado: um arquivo novo não entra no módulo sem dizer o que expõe.
fn itens_autorizados(
    lista: &'static [(&'static str, &'static [&'static str])],
    nome: &str,
    classe: &str,
) -> &'static [&'static str] {
    lista
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, itens)| *itens)
        .unwrap_or_else(|| {
            panic!("src/semantic/{nome} não declarou que itens de {classe} o corte previa")
        })
}

/// A superfície pública do módulo não mudou de tamanho nem de conteúdo.
#[test]
fn a_superficie_publica_do_modulo_e_exatamente_a_congelada() {
    let modulo = semantic();
    let observados: BTreeSet<&str> = modulo
        .lines()
        .filter_map(|linha| linha.strip_prefix("pub "))
        .filter_map(|resto| {
            for palavra in ["fn ", "const ", "struct ", "enum ", "trait ", "type "] {
                if let Some(nome) = resto.strip_prefix(palavra) {
                    return Some(
                        nome.split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                            .unwrap_or(""),
                    );
                }
            }
            None
        })
        .collect();
    let congelados: BTreeSet<&str> = API_PUBLICA_CONGELADA.iter().copied().collect();
    assert_eq!(
        observados, congelados,
        "a superfície pública de semantic mudou; a #619 é decomposição física e não muda \
         API pública"
    );

    // Itens não são a única forma de caminho público: um `pub mod` ou um
    // `pub use` acrescentaria caminhos sem mudar a contagem acima. O corte não
    // move nenhum item público, então não há reexportação devida.
    let codigo = codigo_executavel(&modulo);
    assert_eq!(
        codigo.matches("pub mod ").count(),
        0,
        "o módulo passou a expor um submódulo público; o corte é físico e o irmão é privado"
    );
    assert_eq!(
        codigo.matches("pub use ").count(),
        0,
        "o módulo passou a reexportar; nenhum item do corte era público e nada é devido"
    );
}

/// Um irmão de produção que ganhasse um `#[cfg(test)]` esconderia tudo o que
/// viesse depois de qualquer censo que corte ali — e os censos de C1, C2 e C5
/// cortam ali.
#[test]
fn irmao_de_producao_nao_esconde_producao_atras_de_cfg_test() {
    for nome in IRMAOS_DE_PRODUCAO {
        assert_eq!(
            codigo_executavel(fonte(nome))
                .matches("#[cfg(test)]")
                .count(),
            0,
            "src/semantic/{nome} é produção e não pode cortar o censo com um `#[cfg(test)]`"
        );
    }
}

/// C2 continua com uma dona só, e o irmão não virou a segunda.
///
/// A #619 move o consumo, nunca a regra: `src/method_dispatch.rs` continua
/// decidindo alcance, precedência, desempate e representante. O irmão constrói
/// candidatos e traduz o veredito — e é só isso que pode haver nele. As duas
/// consultas da fase semântica continuam sendo uma cada, agora em arquivos
/// diferentes do mesmo módulo. São as sensitivities M6 e M7 da #619.
#[test]
fn o_irmao_consome_c2_e_nao_cria_uma_segunda_autoridade() {
    let irmao = codigo_executavel(fonte("calls.rs"));
    let pai = codigo_executavel(pai());
    let modulo = codigo_executavel(&semantic());

    // O vocabulário da precedência e a pergunta de alcance continuam fora da
    // fase — no irmão inclusive, que é onde a tentação nasce.
    for termo in [
        "NivelDeDespacho",
        "nivel_de_despacho",
        "PorUnidadeImportada",
    ] {
        assert_eq!(
            irmao.matches(termo).count(),
            0,
            "src/semantic/calls.rs voltou a aplicar `{termo}` por conta própria"
        );
    }

    // O consumo desceu inteiro para o irmão: uma consulta lá, nenhuma no pai.
    assert_eq!(
        irmao.matches("select_impl_method(").count(),
        1,
        "src/semantic/calls.rs deveria consultar `select_impl_method` exatamente uma vez"
    );
    assert_eq!(
        pai.matches("select_impl_method(").count(),
        0,
        "src/semantic.rs voltou a consultar `select_impl_method` por conta própria"
    );
    // `select_representative` é da região `semantic.tratos.contratos`, que a
    // SEM-1 não move: ela continua no pai, e o irmão não pode ganhar uma cópia.
    assert_eq!(
        pai.matches("select_representative(").count(),
        1,
        "src/semantic.rs deveria continuar consultando `select_representative` uma vez"
    );
    assert_eq!(
        irmao.matches("select_representative(").count(),
        0,
        "src/semantic/calls.rs passou a escolher representante, duplicando a autoridade"
    );

    // O módulo inteiro continua com exatamente uma consulta por decisão: o
    // corte não pôde nem duplicar nem apagar nenhuma delas.
    for decisao in ["select_impl_method", "select_representative"] {
        assert_eq!(
            modulo.matches(&format!("{decisao}(")).count(),
            1,
            "o módulo semantic deveria consultar `{decisao}` exatamente uma vez"
        );
    }
}

/// A ordem de fase não mudou: o pai continua sendo quem chama o despacho de
/// chamadas, de dentro da verificação de expressões, e o irmão continua sendo
/// só o corpo. É a fronteira que torna `pub(super)` suficiente — e a
/// sensitivity M9 da #619 perturba exatamente o que este teste ancora.
#[test]
fn o_pai_continua_chamando_o_despacho_de_chamadas() {
    let pai = codigo_executavel(pai());
    assert_eq!(
        pai.matches("self.check_call_expr(").count(),
        1,
        "src/semantic.rs deveria chamar `check_call_expr` exatamente uma vez"
    );
    // A única chamada mora na verificação de expressões, antes do fim da
    // região que a hospeda: um deslocamento de fase mudaria este vizinho.
    let chamada = pai
        .find("self.check_call_expr(")
        .expect("chamada presente no pai");
    let regiao = pai
        .find("ExprKind::Call")
        .expect("o despacho de expressões continua no pai");
    assert!(
        regiao < chamada,
        "a chamada de `check_call_expr` saiu do despacho de expressões do pai"
    );
}
