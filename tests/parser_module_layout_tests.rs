//! Guardião estrutural da decomposição física do `parser` (#602, unidade PAR-X).
//!
//! A #601 registrou que `src/parser.rs` não tinha nenhum guardião estrutural por
//! caminho: perder uma região, duplicá-la, deixá-la no arquivo antigo ou não
//! incluir o módulo novo não quebrava teste nenhum. Este arquivo fecha esse
//! buraco e nada mais.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo das regiões. Afirma quatro coisas mecânicas:
//! o conjunto de arquivos do módulo, o wiring dos `mod`, a presença única de
//! cada região cartografada, e que a decomposição não promoveu visibilidade.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use common::fonte_de_modulo::{parser, PARSER_ARQUIVOS};
use common::rust_source::codigo_executavel;

/// Regiões que a PAR-X moveu de `src/parser.rs` para os irmãos.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("parser.resultado.tentar-propagar", "resultado.rs"),
    ("parser.genericos.inferencia-local", "genericos.rs"),
    ("parser.genericos.substituicao-ast", "genericos.rs"),
    ("parser.callbacks.substituicao-estatica", "genericos.rs"),
    ("parser.callbacks.instanciacao-estatica", "genericos.rs"),
    ("parser.genericos.funcoes-instanciacao", "genericos.rs"),
    ("parser.genericos.leques-instanciacao", "genericos.rs"),
    ("parser.comandos.bloco", "comandos.rs"),
    ("parser.lacos.for-each", "lacos.rs"),
    ("parser.expressoes.precedencia", "expressoes.rs"),
    ("parser.expressoes.primarias", "expressoes.rs"),
    ("parser.expressoes.postfix", "expressoes.rs"),
    ("parser.texto.interpolacao", "expressoes.rs"),
];

/// Regiões que a PAR-X deixou onde estavam. `parser.importacoes.superficie-familia`
/// é a fronteira C6 da #600: a autoridade pré-loader do parser não se move.
const REGIOES_RETIDAS: &[&str] = &[
    "parser.fluxo.nucleo",
    "parser.programa.estrutura",
    "parser.tipos.gramatica",
    "parser.declaracoes.tipos",
    "parser.encaixe.expressao",
    "parser.closures.expressao",
    "parser.funcoes.declaracao",
    "parser.genericos.identidade-especializacao",
    "parser.importacoes.superficie-familia",
    "parser.genericos.leques-template",
    "parser.constantes.declaracao",
];

/// Símbolos que o move obrigou a expor ao módulo pai, um por dependência real.
const EXPOSICOES_NECESSARIAS: &[&str] = &[
    "callee_intrinseco",
    "infer_generic_call_type_args",
    "infer_local_expr_type",
    "instantiate_function_param_functions",
    "instantiate_generic_enums",
    "instantiate_generic_functions",
    "parse_block",
    "parse_expr",
    "parse_for_stmt_desugared",
    "parse_propagar_desugared",
    "parse_tentar_desugared",
    "push_value_param_scope",
    "register_value_type",
    "substitute_function_param_block",
    "substitute_type",
];

fn diretorio_do_modulo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/parser")
}

fn fonte(nome: &str) -> &'static str {
    PARSER_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo estrutural que lê o módulo por `fonte_de_modulo`.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_do_modulo())
        .expect("src/parser/ legível")
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
    let declarados: BTreeSet<String> = PARSER_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/parser/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate: o build fica verde e a
/// implementação some. É a sensitivity M1 da #602.
#[test]
fn mod_rs_inclui_todos_os_irmaos() {
    let codigo = codigo_executavel(fonte("mod.rs"));
    for (nome, _) in PARSER_ARQUIVOS {
        if *nome == "mod.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/parser/mod.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
    }
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. É a sensitivity M2 da #602.
#[test]
fn cada_regiao_cartografada_aparece_uma_vez_no_arquivo_certo() {
    let modulo = parser();
    for (chave, _) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
    }
    for (chave, arquivo) in REGIOES_MOVIDAS {
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/parser/{arquivo}"
        );
    }
    for chave in REGIOES_RETIDAS {
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte("mod.rs").contains(&marcador),
            "a região {chave} não é da PAR-X e deveria continuar em src/parser/mod.rs"
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
            "`{marcador}` deveria aparecer exatamente uma vez no módulo parser"
        );
    }
}

/// Cada símbolo exposto tem uma definição só. Uma implementação duplicada entre
/// pai e irmão passaria pelo marcador acima se viesse sem os comentários.
#[test]
fn cada_exposicao_necessaria_tem_uma_definicao_so() {
    let codigo = codigo_executavel(&parser());
    for simbolo in EXPOSICOES_NECESSARIAS {
        let definicao = format!("fn {simbolo}(");
        assert_eq!(
            codigo.matches(&definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo parser"
        );
        assert_eq!(
            codigo.matches(&format!("pub(super) fn {simbolo}(")).count(),
            1,
            "`{simbolo}` deveria ser exposto ao pai por `pub(super)`, e só por ele"
        );
    }
}

/// A decomposição é física: ela não promove nada para fora do módulo `parser`,
/// e o estado do `Parser` continua privado ao pai. É a sensitivity M3 da #602 —
/// remover uma aresta destas quebra a compilação — mais o controle de que
/// nenhuma delas virou promoção larga.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in PARSER_ARQUIVOS {
        let codigo = codigo_executavel(fonte);
        assert!(
            !codigo.contains("pub(crate)"),
            "src/parser/{nome} promoveu visibilidade a pub(crate)"
        );
        if *nome == "mod.rs" {
            continue;
        }
        assert!(
            !codigo.contains("pub fn") && !codigo.contains("pub struct"),
            "src/parser/{nome} passou a exportar superfície pública nova"
        );
    }
    let exposicoes = codigo_executavel(&parser())
        .matches("pub(super) fn ")
        .count();
    assert_eq!(
        exposicoes,
        EXPOSICOES_NECESSARIAS.len(),
        "o módulo parser expõe ao pai um número de símbolos diferente do justificado pelo move"
    );
}
