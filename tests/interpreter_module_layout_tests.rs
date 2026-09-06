//! Guardião estrutural da decomposição física do interpretador (#608, unidade
//! INT-1).
//!
//! A #601 registrou que `src/interpreter.rs` não tinha guardião estrutural por
//! caminho, e a #607 mediu o custo exato do corte: perder a inclusão do irmão,
//! duplicar `try_call_intrinsic`, deixar a implementação antiga no pai, alargar
//! visibilidade ou desfazer as duas âncoras cartográficas novas não quebrava
//! teste nenhum. Este arquivo fecha esse buraco e nada mais.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo das regiões. Afirma quatro coisas mecânicas: o
//! conjunto de arquivos de `src/interpreter/`, o wiring do `mod` no pai, a
//! presença única de cada região cartografada no arquivo certo, e que a
//! decomposição não promoveu visibilidade.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{interpreter, INTERPRETER_ARQUIVOS};
use rust_source::codigo_executavel;

/// Regiões que a INT-1 moveu de `src/interpreter.rs` para o irmão, junto com a
/// porta `try_call_intrinsic` inteira. `despacho-hospedado` é a âncora nova do
/// prefixo que antes vivia, falsamente, dentro de `acaso`.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    (
        "interpreter.intrinsecos.despacho-hospedado",
        "hosted_intrinsics.rs",
    ),
    ("interpreter.intrinsecos.acaso", "hosted_intrinsics.rs"),
    ("interpreter.intrinsecos.listas", "hosted_intrinsics.rs"),
    (
        "interpreter.intrinsecos.mapas-verso-bombom",
        "hosted_intrinsics.rs",
    ),
    ("interpreter.intrinsecos.leques", "hosted_intrinsics.rs"),
    (
        "interpreter.intrinsecos.io-arquivo-texto",
        "hosted_intrinsics.rs",
    ),
    (
        "interpreter.intrinsecos.falha-operacional",
        "hosted_intrinsics.rs",
    ),
    (
        "interpreter.intrinsecos.tempo-processos-ambiente",
        "hosted_intrinsics.rs",
    ),
    (
        "interpreter.intrinsecos.conversoes-numero-texto",
        "hosted_intrinsics.rs",
    ),
    (
        "interpreter.intrinsecos.mapas-tipados",
        "hosted_intrinsics.rs",
    ),
];

/// Regiões que a INT-1 deixou onde estavam. `interpreter.memoria.estado-
/// enderecavel` é a segunda âncora nova: o bloco de memória endereçável que a
/// cartografia anterior atribuía a `acaso` continua fisicamente no pai, agora
/// com key própria.
const REGIOES_RETIDAS: &[&str] = &[
    "interpreter.modelo.valores-estado",
    "interpreter.execucao.programa-globais",
    "interpreter.execucao.funcoes-fluxo",
    "interpreter.execucao.instrucoes-pilha",
    "interpreter.falha-operacional.construcao",
    "interpreter.memoria.estado-enderecavel",
    "interpreter.hospedeiro.servicos-auxiliares",
    "interpreter.execucao.valores-tipos",
    "interpreter.diagnostico.stack-trace",
    "interpreter.unioes.contabilidade-dominios",
    "evidencia.processos.saida-runtime-hospedado",
];

/// O único símbolo que o move obrigou a expor ao pai. A #607 mediu o custo:
/// um `pub(super)`, zero `pub(crate)` novo, zero campo promovido.
const EXPOSICOES_NECESSARIAS: &[&str] = &["try_call_intrinsic"];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/interpreter")
}

fn fonte(nome: &str) -> &'static str {
    INTERPRETER_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo estrutural que lê o interpretador por `fonte_de_modulo`.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/interpreter/ legível")
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
    let declarados: BTreeSet<String> = INTERPRETER_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "interpreter.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/interpreter/ divergiu da lista lida pelos oráculos estruturais"
    );
    assert_eq!(
        fonte_de_modulo::interpreter_caminhos(),
        vec![
            "src/interpreter.rs".to_string(),
            "src/interpreter/hosted_intrinsics.rs".to_string()
        ],
        "os censos que leem do disco deixaram de cobrir o módulo inteiro"
    );
}

/// Sem o `mod`, o irmão não entra no crate; sem o `use`, a porta não é a mesma
/// que `exec_instr` chama. É a sensitivity M1 da #608.
#[test]
fn o_pai_inclui_e_importa_o_irmao() {
    let codigo = codigo_executavel(fonte("interpreter.rs"));
    for (nome, _) in INTERPRETER_ARQUIVOS {
        if *nome == "interpreter.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/interpreter.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
    }
    assert_eq!(
        codigo
            .matches("use hosted_intrinsics::try_call_intrinsic;")
            .count(),
        1,
        "src/interpreter.rs deveria importar a porta do irmão exatamente uma vez"
    );
    // O pai continua sendo um arquivo: a #608 decidiu explicitamente não
    // convertê-lo em `src/interpreter/mod.rs`.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. É a sensitivity M2 da #608, e são as
/// M5/M6 para as duas âncoras novas.
#[test]
fn cada_regiao_cartografada_aparece_uma_vez_no_arquivo_certo() {
    let modulo = interpreter();
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
            "a região {chave} deveria morar em src/interpreter/{arquivo}"
        );
    }
    for chave in REGIOES_RETIDAS {
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte("interpreter.rs").contains(&marcador),
            "a região {chave} não é da INT-1 e deveria continuar em src/interpreter.rs"
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
            "`{marcador}` deveria aparecer exatamente uma vez no módulo interpreter"
        );
    }
}

/// Cada símbolo exposto tem uma definição só. Uma implementação duplicada entre
/// pai e irmão passaria pelo marcador acima se viesse sem os comentários.
/// É a sensitivity M2 da #608 no sentido do símbolo, e a M3 no do qualificador.
#[test]
fn cada_exposicao_necessaria_tem_uma_definicao_so() {
    let codigo = codigo_executavel(&interpreter());
    let pai = codigo_executavel(fonte("interpreter.rs"));
    for simbolo in EXPOSICOES_NECESSARIAS {
        let definicao = format!("fn {simbolo}(");
        assert_eq!(
            codigo.matches(&definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo interpreter"
        );
        assert_eq!(
            codigo.matches(&format!("pub(super) fn {simbolo}(")).count(),
            1,
            "`{simbolo}` deveria ser exposto ao pai por `pub(super)`, e só por ele"
        );
        assert_eq!(
            pai.matches(&definicao).count(),
            0,
            "a implementação antiga de `{simbolo}` ficou para trás em src/interpreter.rs"
        );
    }
}

/// A decomposição é física: ela não promove nada para fora do interpretador e
/// não promove campo nenhum. É a sensitivity M4 da #608, mais o controle de que
/// o único `pub(super)` do irmão é o justificado pelo move.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in INTERPRETER_ARQUIVOS {
        if *nome == "interpreter.rs" {
            continue;
        }
        let codigo = codigo_executavel(fonte);
        assert!(
            !codigo.contains("pub(crate)"),
            "src/interpreter/{nome} promoveu visibilidade a pub(crate)"
        );
        assert_eq!(
            codigo.matches("pub ").count(),
            0,
            "src/interpreter/{nome} passou a exportar superfície pública nova"
        );
        assert_eq!(
            codigo.matches("pub(").count(),
            codigo.matches("pub(super)").count(),
            "src/interpreter/{nome} usa visibilidade restrita que não é pub(super)"
        );
        assert!(
            !codigo.contains("macro_rules!"),
            "src/interpreter/{nome} levou macro por escopo textual, que a #607 mediu como ausente do corte"
        );
    }
    let exposicoes = codigo_executavel(&interpreter())
        .matches("pub(super) ")
        .count();
    assert_eq!(
        exposicoes,
        EXPOSICOES_NECESSARIAS.len(),
        "o módulo interpreter expõe ao pai um número de símbolos diferente do justificado pelo move"
    );
}
