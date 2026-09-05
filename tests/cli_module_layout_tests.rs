//! Guardião estrutural da decomposição física do binário `pink` (#605,
//! unidade MAIN-5+2+3).
//!
//! A #601 registrou que `src/main.rs` não tinha nenhum guardião estrutural por
//! caminho: perder uma região, duplicá-la, deixá-la no arquivo antigo ou não
//! incluir o módulo novo não quebrava teste nenhum. Este arquivo fecha esse
//! buraco e nada mais.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo das regiões. Afirma quatro coisas mecânicas:
//! o conjunto de arquivos de `src/pink_cli/`, o wiring dos `mod` no entrypoint,
//! a presença única de cada região cartografada, e que a decomposição não
//! promoveu visibilidade.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{pink_cli, PINK_CLI_ARQUIVOS};
use rust_source::codigo_executavel;

/// Regiões que a MAIN-5+2+3 moveu de `src/main.rs` para os irmãos.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("cli.parsing.subcomandos", "cli_parsing.rs"),
    ("cli.parsing.roteamento", "cli_parsing.rs"),
    ("cli.doc.consulta", "doc_cli.rs"),
    ("cli.doc.sincronizacao", "doc_cli.rs"),
    ("cli.doc.mudancas", "doc_cli.rs"),
    ("cli.doc.verificacao", "doc_cli.rs"),
    ("cli.modulos.importacao", "modules.rs"),
];

/// Regiões que a MAIN-5+2+3 deixou onde estavam. `cli.execucao.entrada` é o
/// `main` e o `macro_rules! try_or_exit`; `cli.analise.pipeline` e
/// `cli.build.nativo` são a orquestração do pipeline, que não se move.
const REGIOES_RETIDAS: &[&str] = &[
    "cli.config.modelos",
    "cli.ajuda.usage",
    "cli.execucao.entrada",
    "cli.nav.projecao",
    "cli.nav.consulta",
    "cli.nav.sincronizacao-verificacao",
    "cli.execucao.editor-repl",
    "cli.analise.pipeline",
    "cli.build.nativo",
];

/// Símbolos que o move obrigou a expor ao entrypoint, um por dependência real.
const EXPOSICOES_NECESSARIAS: &[&str] = &[
    "base_dir_de",
    "carregar_e_projetar",
    "contexto_de_import",
    "load_doc_config",
    "parse_args",
    "run_doc",
    "write_atomic",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pink_cli")
}

fn fonte(nome: &str) -> &'static str {
    PINK_CLI_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo estrutural que lê o binário por `fonte_de_modulo`.
#[test]
fn o_conjunto_de_arquivos_do_binario_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/pink_cli/ legível")
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
    let declarados: BTreeSet<String> = PINK_CLI_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "main.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/pink_cli/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate: o build fica verde e a
/// implementação some. É a sensitivity M1 da #605.
#[test]
fn o_entrypoint_inclui_todos_os_irmaos() {
    let codigo = codigo_executavel(fonte("main.rs"));
    let bruto = fonte("main.rs");
    for (nome, _) in PINK_CLI_ARQUIVOS {
        if *nome == "main.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/main.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
        // `src/main.rs` é a raiz do crate binário e continua sendo um arquivo,
        // não um `mod.rs`: os irmãos moram em `src/pink_cli/` por `#[path]`.
        let caminho = format!("#[path = \"pink_cli/{nome}\"]");
        assert_eq!(
            bruto.matches(&caminho).count(),
            1,
            "src/main.rs deveria apontar `{caminho}` exatamente uma vez"
        );
    }
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. É a sensitivity M2 da #605.
#[test]
fn cada_regiao_cartografada_aparece_uma_vez_no_arquivo_certo() {
    let binario = pink_cli();
    for (chave, _) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&binario, chave);
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&binario, chave);
    }
    for (chave, arquivo) in REGIOES_MOVIDAS {
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/pink_cli/{arquivo}"
        );
    }
    for chave in REGIOES_RETIDAS {
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte("main.rs").contains(&marcador),
            "a região {chave} não é da MAIN-5+2+3 e deveria continuar em src/main.rs"
        );
    }
}

fn conferir_regiao_unica(binario: &str, chave: &str) {
    for marcador in [
        format!("// @pinker-nav:start {chave}"),
        format!("// @pinker-nav:end {chave}"),
    ] {
        assert_eq!(
            binario.matches(&marcador).count(),
            1,
            "`{marcador}` deveria aparecer exatamente uma vez no binário pink"
        );
    }
}

/// Cada símbolo exposto tem uma definição só. Uma implementação duplicada entre
/// entrypoint e irmão passaria pelo marcador acima se viesse sem os comentários.
#[test]
fn cada_exposicao_necessaria_tem_uma_definicao_so() {
    let codigo = codigo_executavel(&pink_cli());
    for simbolo in EXPOSICOES_NECESSARIAS {
        let definicao = format!("fn {simbolo}(");
        assert_eq!(
            codigo.matches(&definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no binário pink"
        );
        assert_eq!(
            codigo.matches(&format!("pub(super) fn {simbolo}(")).count(),
            1,
            "`{simbolo}` deveria ser exposto ao entrypoint por `pub(super)`, e só por ele"
        );
    }
}

/// A decomposição é física: ela não promove nada para fora do binário, e o
/// `macro_rules! try_or_exit` continua no entrypoint, junto com quem o usa.
/// É a sensitivity M3 da #605 — remover uma aresta destas quebra a compilação —
/// mais o controle de que nenhuma delas virou promoção larga.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in PINK_CLI_ARQUIVOS {
        let codigo = codigo_executavel(fonte);
        assert!(
            !codigo.contains("pub(crate)"),
            "src/pink_cli/{nome} promoveu visibilidade a pub(crate)"
        );
        if *nome == "main.rs" {
            continue;
        }
        assert_eq!(
            codigo.matches("pub ").count(),
            0,
            "src/pink_cli/{nome} passou a exportar superfície pública nova"
        );
        assert_eq!(
            codigo.matches("pub(").count(),
            codigo.matches("pub(super)").count(),
            "src/pink_cli/{nome} usa visibilidade restrita que não é pub(super)"
        );
        assert!(
            !codigo.contains("macro_rules!"),
            "src/pink_cli/{nome} levou macro por escopo textual, que a #601 mediu como dependência da MAIN-4"
        );
    }
    let exposicoes = codigo_executavel(&pink_cli())
        .matches("pub(super) fn ")
        .count();
    assert_eq!(
        exposicoes,
        EXPOSICOES_NECESSARIAS.len(),
        "o binário pink expõe ao entrypoint um número de símbolos diferente do justificado pelo move"
    );
}
