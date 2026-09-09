//! Guardião estrutural da decomposição física do binário `pink`, no estado
//! cumulativo FINAL: MAIN-5+2+3 (#605) mais MAIN-4 (#638) mais MAIN-1 (#640).
//!
//! A #601 registrou que `src/main.rs` não tinha nenhum guardião estrutural por
//! caminho: perder uma região, duplicá-la, deixá-la no arquivo antigo ou não
//! incluir o módulo novo não quebrava teste nenhum. Este arquivo fecha esse
//! buraco e nada mais.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo das regiões. Afirma coisas mecânicas: o
//! conjunto de arquivos de `src/pink_cli/`, o wiring dos `mod` no entrypoint,
//! a igualdade de conjunto entre a partição declarada aqui e a camada `cli` do
//! catálogo, a presença única de cada região, que a decomposição não promoveu
//! visibilidade, que o escopo textual do `macro_rules! try_or_exit` continua o
//! que a #601 mediu — desde a MAIN-4 — e, desde a MAIN-1, que o roteamento de
//! `nav`, a varredura do catálogo, os códigos de saída e o wiring de
//! diagnóstico continuam sendo do entrypoint: o irmão implementa os
//! adaptadores, não vira uma segunda autoridade de navegação ou de projeção.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{pink_cli, PINK_CLI_ARQUIVOS};
use pinker_v0::nav::CodeCatalog;
use rust_source::codigo_executavel;

/// Regiões que a decomposição física moveu de `src/main.rs` para os irmãos.
/// As sete primeiras são da MAIN-5+2+3 (#605); as duas seguintes, da MAIN-4
/// (#638); as três últimas, da MAIN-1 (#640), a última unidade MAIN planejada
/// nesta sequência.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("cli.parsing.subcomandos", "cli_parsing.rs"),
    ("cli.parsing.roteamento", "cli_parsing.rs"),
    ("cli.doc.consulta", "doc_cli.rs"),
    ("cli.doc.sincronizacao", "doc_cli.rs"),
    ("cli.doc.mudancas", "doc_cli.rs"),
    ("cli.doc.verificacao", "doc_cli.rs"),
    ("cli.modulos.importacao", "modules.rs"),
    ("cli.analise.pipeline", "analysis_build.rs"),
    ("cli.build.nativo", "analysis_build.rs"),
    ("cli.nav.projecao", "nav_cli.rs"),
    ("cli.nav.consulta", "nav_cli.rs"),
    ("cli.nav.sincronizacao-verificacao", "nav_cli.rs"),
];

/// Regiões que continuam no entrypoint depois da última unidade do inventário.
/// `cli.execucao.entrada` é o `main`, o roteamento de modo de comando, a
/// varredura do catálogo e o `macro_rules! try_or_exit`; `cli.config.modelos` e
/// `cli.ajuda.usage` são o vocabulário do binário, que a §7 da #601 rejeitou
/// mover por custo de visibilidade desproporcional; `cli.execucao.editor-repl`
/// não pertence a unidade nenhuma do inventário.
const REGIOES_RETIDAS: &[&str] = &[
    "cli.config.modelos",
    "cli.ajuda.usage",
    "cli.execucao.entrada",
    "cli.execucao.editor-repl",
];

/// As três regiões da MAIN-1, a última unidade do inventário da #601. Elas são
/// um subconjunto declarado de [`REGIOES_MOVIDAS`]: deixá-las para trás no pai,
/// duplicá-las ou espalhá-las por outro irmão são desvios que um `assert` só
/// sobre a lista grande não nomearia.
const REGIOES_DA_MAIN_1: &[&str] = &[
    "cli.nav.projecao",
    "cli.nav.consulta",
    "cli.nav.sincronizacao-verificacao",
];

/// Irmão que a MAIN-1 criou.
const IRMAO_DA_MAIN_1: &str = "nav_cli.rs";

/// Os dez símbolos que a #601 mediu como `pub(super)` da MAIN-1 — a lista
/// `exports` de `unit_costs.json`, nem um a mais. Eles são o cabo entre o
/// roteamento, que fica no entrypoint, e a implementação, que desceu.
const SIMBOLOS_DA_MAIN_1: &[&str] = &[
    "run_nav_projecao",
    "run_nav_mostrar",
    "run_nav_buscar",
    "run_nav_localizar",
    "run_nav_cobertura_diff",
    "run_nav_impacto",
    "run_nav_listar",
    "run_nav_mapa",
    "run_nav_sincronizar",
    "run_nav_verificar",
];

/// Símbolos que o move obrigou a expor ao entrypoint, um por dependência real.
const EXPOSICOES_NECESSARIAS: &[&str] = &[
    "base_dir_de",
    "carregar_e_projetar",
    "contexto_de_import",
    "load_doc_config",
    "parse_args",
    "run_analyze",
    "run_build",
    "run_doc",
    "run_nav_buscar",
    "run_nav_cobertura_diff",
    "run_nav_impacto",
    "run_nav_listar",
    "run_nav_localizar",
    "run_nav_mapa",
    "run_nav_mostrar",
    "run_nav_projecao",
    "run_nav_sincronizar",
    "run_nav_verificar",
    "write_atomic",
];

/// Irmão que a MAIN-4 criou e o único que pode usar `try_or_exit!`, porque é o
/// único declarado abaixo da definição da macro.
const IRMAO_DA_MAIN_4: &str = "analysis_build.rs";

/// Ocorrências de `try_or_exit!` que a #601 mediu dentro dos spans da MAIN-4.
/// A conta dela é lexical: das 29, 28 são chamadas e 1 é a menção ao nome da
/// macro dentro do `@pinker-nav:summary` da própria região `cli.analise.pipeline`
/// — a mesma sobrecontagem conservadora que a §3 da #601 declara para `path` e
/// `criar`. As duas contas ficam ancoradas aqui: nenhuma das 29 ficou para trás.
const CHAMADAS_DE_TRY_OR_EXIT_NA_MAIN_4: usize = 28;
const OCORRENCIAS_DE_TRY_OR_EXIT_NA_MAIN_4: usize = 29;

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
            "a região {chave} não pertence a nenhuma unidade executada e deveria continuar em src/main.rs"
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

/// A decomposição é física: ela não promove nada para fora do binário, e
/// nenhum irmão leva `macro_rules!` consigo — a macro é do entrypoint.
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
            "src/pink_cli/{nome} define macro_rules!, que é do entrypoint e não viaja com nenhuma unidade"
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

fn catalogo() -> CodeCatalog {
    let caminho = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/navigation.jsonl");
    CodeCatalog::load(&caminho).expect("catálogo de código versionado")
}

/// `DECLARED == REALITY` por igualdade de conjunto, não por presença. Uma lista
/// conferida só por presença é cega para a região nova que ninguém registrou e
/// para a região que mudou de arquivo por engano: as duas passariam. A camada
/// `cli` do catálogo é a realidade; a partição acima é a declaração.
#[test]
fn a_particao_declarada_e_exatamente_a_camada_cli_do_catalogo() {
    let catalogo = catalogo();
    let real: BTreeSet<(String, String)> = catalogo
        .regions
        .iter()
        .filter(|regiao| regiao.layer.as_deref() == Some("cli"))
        .map(|regiao| (regiao.key.clone(), regiao.file.clone()))
        .collect();
    let declarado: BTreeSet<(String, String)> = REGIOES_MOVIDAS
        .iter()
        .map(|(chave, arquivo)| ((*chave).to_string(), format!("src/pink_cli/{arquivo}")))
        .chain(
            REGIOES_RETIDAS
                .iter()
                .map(|chave| ((*chave).to_string(), "src/main.rs".to_string())),
        )
        .collect();
    assert_eq!(
        declarado, real,
        "a partição declarada neste guardião divergiu da camada cli do catálogo"
    );
}

/// Escopo de `macro_rules!` é textual, não de item: nenhum `pub(super)` o
/// alcança. A #601 mediu `try_or_exit!` definido em `src/main.rs` com 29 usos,
/// todos dentro dos spans da MAIN-4. O move só é legítimo se a definição
/// continuar única e no entrypoint, se os 29 usos continuarem inteiros num
/// irmão só, e se o `mod` desse irmão estiver ABAIXO da definição — acima, o
/// irmão não enxerga a macro e o build para.
#[test]
fn o_escopo_textual_de_try_or_exit_e_o_que_a_601_mediu() {
    let entrypoint = codigo_executavel(fonte("main.rs"));

    assert_eq!(
        entrypoint.matches("macro_rules! try_or_exit").count(),
        1,
        "a definição de try_or_exit! deveria continuar única em src/main.rs"
    );
    let definicao = entrypoint
        .find("macro_rules! try_or_exit")
        .expect("definição de try_or_exit! no entrypoint");
    let modulo = IRMAO_DA_MAIN_4.trim_end_matches(".rs");
    let declaracao = entrypoint
        .find(&format!("mod {modulo};"))
        .expect("o entrypoint declara o irmão da MAIN-4");
    assert!(
        definicao < declaracao,
        "`mod {modulo};` precisa vir depois de `macro_rules! try_or_exit`: acima da definição o irmão não enxerga a macro"
    );

    assert_eq!(
        entrypoint.matches("try_or_exit!(").count(),
        0,
        "nenhum uso de try_or_exit! deveria ter ficado no entrypoint"
    );
    let irmao = codigo_executavel(fonte(IRMAO_DA_MAIN_4));
    assert_eq!(
        irmao.matches("try_or_exit!(").count(),
        CHAMADAS_DE_TRY_OR_EXIT_NA_MAIN_4,
        "src/pink_cli/{IRMAO_DA_MAIN_4} deveria conter as {CHAMADAS_DE_TRY_OR_EXIT_NA_MAIN_4} chamadas da MAIN-4"
    );
    assert_eq!(
        fonte(IRMAO_DA_MAIN_4).matches("try_or_exit!").count(),
        OCORRENCIAS_DE_TRY_OR_EXIT_NA_MAIN_4,
        "src/pink_cli/{IRMAO_DA_MAIN_4} deveria conter as {OCORRENCIAS_DE_TRY_OR_EXIT_NA_MAIN_4} ocorrências que a #601 mediu"
    );

    for (nome, fonte_irmao) in PINK_CLI_ARQUIVOS {
        if *nome == "main.rs" || *nome == IRMAO_DA_MAIN_4 {
            continue;
        }
        assert_eq!(
            codigo_executavel(fonte_irmao)
                .matches("try_or_exit!")
                .count(),
            0,
            "src/pink_cli/{nome} é declarado acima da definição da macro e não pode usá-la"
        );
    }
}

/// A #640 executa a MAIN-1, a última unidade do inventário. As três regiões
/// `cli.nav.*` moram no irmão dela, não sobraram no pai e não vazaram para
/// nenhum outro irmão — nem uma cópia, que é o modo silencioso de "mover".
#[test]
fn a_main_1_mora_no_irmao_e_nao_no_entrypoint() {
    for chave in REGIOES_DA_MAIN_1 {
        assert!(
            REGIOES_MOVIDAS
                .iter()
                .any(|(movida, arquivo)| movida == chave && *arquivo == IRMAO_DA_MAIN_1),
            "{chave} é da MAIN-1 e deveria estar declarada como movida para {IRMAO_DA_MAIN_1}"
        );
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(IRMAO_DA_MAIN_1).contains(&marcador),
            "a região {chave} é da MAIN-1 e deveria morar em src/pink_cli/{IRMAO_DA_MAIN_1}"
        );
        for (nome, fonte_arquivo) in PINK_CLI_ARQUIVOS {
            if *nome == IRMAO_DA_MAIN_1 {
                continue;
            }
            assert!(
                !fonte_arquivo.contains(&marcador),
                "{nome} ainda contém {chave}: a MAIN-1 ficou duplicada ou foi deixada para trás"
            );
        }
    }

    // A implementação também não pode ter ficado no pai sem os marcadores.
    let entrypoint = codigo_executavel(fonte("main.rs"));
    let irmao = codigo_executavel(fonte(IRMAO_DA_MAIN_1));
    for simbolo in SIMBOLOS_DA_MAIN_1 {
        let definicao = format!("fn {simbolo}(");
        assert_eq!(
            entrypoint.matches(&definicao).count(),
            0,
            "`{definicao}` é da MAIN-1 e não deveria continuar definida em src/main.rs"
        );
        assert_eq!(
            irmao.matches(&definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição em src/pink_cli/{IRMAO_DA_MAIN_1}"
        );
    }
    assert_eq!(
        SIMBOLOS_DA_MAIN_1.len(),
        irmao.matches("pub(super) fn ").count(),
        "src/pink_cli/{IRMAO_DA_MAIN_1} expõe ao entrypoint um número de símbolos diferente dos dez que a #601 mediu"
    );
}

/// O move é físico e não cria autoridade nova. Quem decide qual `NavSub` roda
/// continua sendo `run_nav`, no entrypoint; o irmão só implementa o adaptador
/// de cada uma. Um roteamento que descesse junto tornaria o filho a segunda
/// autoridade de navegação.
#[test]
fn o_roteamento_de_nav_continua_no_entrypoint() {
    let entrypoint = codigo_executavel(fonte("main.rs"));
    let binario = codigo_executavel(&pink_cli());

    assert_eq!(
        entrypoint.matches("fn run_nav(").count(),
        1,
        "`run_nav` deveria continuar definido em src/main.rs"
    );
    assert_eq!(
        binario.matches("fn run_nav(").count(),
        1,
        "`run_nav` deveria existir uma vez só no binário inteiro"
    );
    for despacho in [
        "NavSub::Mostrar",
        "NavSub::Buscar",
        "NavSub::Localizar",
        "NavSub::CoberturaDiff",
        "NavSub::Impacto",
        "NavSub::Listar",
        "NavSub::Mapa",
        "NavSub::Sincronizar",
        "NavSub::Verificar",
        "NavSub::Projecao",
    ] {
        assert_eq!(
            entrypoint.matches(despacho).count(),
            1,
            "`{despacho}` deveria continuar sendo roteado exatamente uma vez em src/main.rs"
        );
        assert_eq!(
            codigo_executavel(fonte(IRMAO_DA_MAIN_1))
                .matches(despacho)
                .count(),
            0,
            "`{despacho}` desceu para o irmão: o roteamento de nav é do entrypoint"
        );
    }
}

/// Autoridade de navegação e de projeção não se duplica. As autoridades reais
/// são da biblioteca; o binário só as adapta, e cada porta de entrada continua
/// tendo um chamador só. A varredura do repositório (`scan_code`) fica no
/// entrypoint, porque é dela que o `main` depende para os dois lados.
#[test]
fn a_autoridade_de_nav_e_projecao_nao_foi_duplicada() {
    let entrypoint = codigo_executavel(fonte("main.rs"));
    let irmao = codigo_executavel(fonte(IRMAO_DA_MAIN_1));
    let binario = codigo_executavel(&pink_cli());

    assert_eq!(
        entrypoint.matches("fn scan_code(").count(),
        1,
        "`scan_code` deveria continuar definido em src/main.rs"
    );
    assert_eq!(
        binario.matches("fn scan_code(").count(),
        1,
        "`scan_code` deveria existir uma vez só no binário inteiro"
    );
    assert_eq!(
        entrypoint.matches("nav::CodeIndex::scan_repo(").count(),
        1,
        "a varredura do repositório deveria continuar sendo chamada só do entrypoint"
    );
    assert_eq!(
        irmao.matches("nav::CodeIndex::scan_repo(").count(),
        0,
        "src/pink_cli/{IRMAO_DA_MAIN_1} passou a varrer o repositório por conta própria"
    );

    for autoridade in [
        "nav::verify_repository(",
        "nav_projection_lifecycle::plan_prepare(",
        "nav_projection_lifecycle::plan_accept(",
        "nav_projection_lifecycle::apply_prepare(",
        "nav_projection_lifecycle::apply_accept(",
        "symbol_index::locate(",
        "diff_coverage::analyze(",
    ] {
        assert_eq!(
            binario.matches(autoridade).count(),
            irmao.matches(autoridade).count(),
            "`{autoridade}` deveria ser consumida só pelo adaptador da MAIN-1"
        );
        assert!(
            irmao.contains(autoridade),
            "`{autoridade}` deveria continuar sendo consumida por src/pink_cli/{IRMAO_DA_MAIN_1}"
        );
    }
}

/// Os códigos de saída e o wiring de diagnóstico são o vocabulário do binário:
/// a §7 da #601 rejeitou mover `cli.config.modelos`, e nenhum irmão pode
/// redefinir um `EXIT_*` por conta própria. O irmão os usa por `use super::*`,
/// não os declara.
#[test]
fn os_codigos_de_saida_continuam_declarados_uma_vez_no_entrypoint() {
    let entrypoint = codigo_executavel(fonte("main.rs"));
    for codigo in [
        "EXIT_OK",
        "EXIT_FAILURE",
        "EXIT_USAGE",
        "EXIT_CATALOG",
        "EXIT_NORESULT",
        "EXIT_SOURCE",
        "EXIT_HARNESS",
        "EXIT_POLICY",
        "EXIT_STALE",
    ] {
        let declaracao = format!("const {codigo}: i32 =");
        assert_eq!(
            entrypoint.matches(&declaracao).count(),
            1,
            "`{declaracao}` deveria continuar declarado exatamente uma vez em src/main.rs"
        );
        for (nome, fonte_irmao) in PINK_CLI_ARQUIVOS {
            if *nome == "main.rs" {
                continue;
            }
            assert_eq!(
                codigo_executavel(fonte_irmao).matches(&declaracao).count(),
                0,
                "src/pink_cli/{nome} redeclarou `{codigo}`"
            );
        }
    }
}

/// O move é físico: ele não transfere autoridade. Quem escolhe o modo de
/// comando continua sendo o `main` do entrypoint; o irmão só implementa. Um
/// despacho que migrasse para o filho o tornaria a nova autoridade de pipeline.
#[test]
fn o_despacho_de_modo_de_comando_continua_no_entrypoint() {
    let entrypoint = codigo_executavel(fonte("main.rs"));
    let binario = codigo_executavel(&pink_cli());
    for despacho in [
        "CliCommand::Analyze(config) => run_analyze(config)",
        "CliCommand::Build(config) => run_build(config)",
        "CliCommand::Nav(config) => std::process::exit(run_nav(config))",
    ] {
        assert_eq!(
            entrypoint.matches(despacho).count(),
            1,
            "`{despacho}` deveria continuar exatamente uma vez em src/main.rs"
        );
        assert_eq!(
            binario.matches(despacho).count(),
            1,
            "`{despacho}` deveria existir uma vez só no binário inteiro"
        );
    }
    assert_eq!(
        entrypoint.matches("fn main(").count(),
        1,
        "o entrypoint deveria continuar dono de `fn main`"
    );
    assert_eq!(
        binario.matches("fn main(").count(),
        1,
        "`fn main` deveria existir uma vez só no binário inteiro"
    );
}
