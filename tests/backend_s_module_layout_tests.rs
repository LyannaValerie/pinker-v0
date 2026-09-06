//! Guardião estrutural da decomposição física do backend montável (#610,
//! unidade BS-3).
//!
//! A #601 mediu que `src/backend_s.rs` é lido por caminho fixo por vários
//! oráculos e que a primeira unidade do arquivo paga a reescrita desses
//! guardiões. Eles não leem o arquivo do mesmo jeito. Três censos de produção
//! — C1, superfície de família e isolamento de símbolo de ABI — cortam no
//! primeiro `#[cfg(test)]`; dois censos de arquivo inteiro, D6 e o da #522,
//! foram reapontados para o módulo composto; e D7 e a cartografia extraem
//! corpos de função específicos, que não saíram do pai. O corte BS-3 move só
//! os dois módulos de teste, e nenhum deles perde produção.
//!
//! O que o corte cria de novo é uma forma de cegueira silenciosa, e ela é dos
//! três primeiros: se a declaração `mod tests;` subir para o topo do pai, eles
//! passam a cortar o arquivo inteiro, continuam verdes e param de observar a
//! produção. Este arquivo fecha esse buraco e nada mais.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo dos testes movidos.

#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{backend_s, BACKEND_S_ARQUIVOS};
use pinker_v0::intrinsics::registry::{self, RuntimeRouting};
use rust_source::codigo_executavel;

/// As duas regiões que a BS-3 moveu, e o irmão onde passam a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("evidencia.backend-s.proveniencia-de-ponteiro", "tests.rs"),
    ("evidencia.backend-s.selecao-de-rota-nativa", "tests.rs"),
];

/// Os dois módulos de teste que viajaram inteiros, sem renomeação.
const MODULOS_MOVIDOS: &[&str] = &[
    "tests_proveniencia_de_ponteiro",
    "tests_selecao_de_rota_nativa",
];

/// Amostra de regiões que a BS-3 deixou onde estavam. Não é a lista completa
/// do arquivo: é o controle de que o corte não arrastou vizinhança.
const REGIOES_RETIDAS: &[&str] = &[
    "backend-s.renderizacao.abi-textual-componentes",
    "backend-s.runtime.intrinsecas-por-aridade",
    "backend-s.runtime.simbolos-intrinsecas",
    "backend-s.lowering.chamadas-sysv",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/backend_s")
}

fn fonte(nome: &str) -> &'static str {
    BACKEND_S_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo que lê o backend por `fonte_de_modulo`.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/backend_s/ legível")
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
    let declarados: BTreeSet<String> = BACKEND_S_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "backend_s.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/backend_s/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate e os vinte testes movidos deixam de
/// existir sem que nada fique vermelho. É a sensitivity M1 da #610.
#[test]
fn o_pai_inclui_o_irmao() {
    let codigo = codigo_executavel(fonte("backend_s.rs"));
    for (nome, _) in BACKEND_S_ARQUIVOS {
        if *nome == "backend_s.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/backend_s.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
    }
    // O pai continua sendo um arquivo: a #610 manteve a forma que a #608
    // decidiu para o interpretador.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608/#610"
    );
}

/// O oráculo desta unidade.
///
/// `tests/c1_intrinsic_registry_tests.rs`, `tests/part_g_familia_superficie_
/// tests.rs` e `tests/issue497_abi_symbol_isolation_tests.rs` leem
/// `src/backend_s.rs` cortando no primeiro `#[cfg(test)]`. Antes da BS-3 o
/// corte caía no fim do arquivo e eles viam a produção inteira. Só continua
/// assim enquanto a declaração do irmão for a última coisa do pai: subi-la
/// para o topo deixaria os três verdes e cegos.
#[test]
fn o_corte_dos_oraculos_no_primeiro_cfg_test_ainda_ve_a_producao_inteira() {
    let pai = fonte("backend_s.rs");
    let corte = pai
        .find("\n#[cfg(test)]")
        .expect("o pai declara o irmão sob #[cfg(test)]");
    let depois: String = pai[corte..]
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        depois, "#[cfg(test)] mod tests;",
        "depois do primeiro `#[cfg(test)]` o pai passou a ter conteúdo que os \
         oráculos que cortam ali deixam de observar"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem módulo
/// deixado para trás no arquivo antigo. São as sensitivities M2 e M3 da #610.
#[test]
fn cada_regiao_e_cada_modulo_movido_aparece_uma_vez_no_arquivo_certo() {
    let modulo = backend_s();
    for (chave, arquivo) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/backend_s/{arquivo}"
        );
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte("backend_s.rs").contains(&marcador),
            "a região {chave} não é da BS-3 e deveria continuar em src/backend_s.rs"
        );
    }

    let codigo = codigo_executavel(&modulo);
    let pai = codigo_executavel(fonte("backend_s.rs"));
    for nome in MODULOS_MOVIDOS {
        let declaracao = format!("mod {nome} {{");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "`{declaracao}` deveria ter exatamente uma definição no módulo backend_s"
        );
        assert_eq!(
            pai.matches(&declaracao).count(),
            0,
            "o módulo `{nome}` ficou para trás em src/backend_s.rs"
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
            "`{marcador}` deveria aparecer exatamente uma vez no módulo backend_s"
        );
    }
}

/// A decomposição é física: não promove nada para fora do backend. A única
/// reexportação do irmão é a ponte que devolve o pai aos módulos movidos, que
/// continuam escritos com `use super::*`; ela vive dentro de um `mod tests`
/// privado e `#[cfg(test)]`, e por isso não é superfície de crate nenhuma.
/// É a sensitivity M4 da #610.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in BACKEND_S_ARQUIVOS {
        if *nome == "backend_s.rs" {
            continue;
        }
        let codigo = codigo_executavel(fonte);
        assert!(
            !codigo.contains("pub(crate)"),
            "src/backend_s/{nome} promoveu visibilidade a pub(crate)"
        );
        assert_eq!(
            codigo.matches("pub(").count(),
            0,
            "src/backend_s/{nome} passou a usar visibilidade restrita, que o corte não previa"
        );
        assert_eq!(
            codigo.matches("pub ").count(),
            1,
            "src/backend_s/{nome} deveria ter exatamente a ponte `pub use super::*;`"
        );
        assert_eq!(
            codigo.matches("pub use super::*;").count(),
            1,
            "a única reexportação do irmão deveria ser a ponte para o pai"
        );
    }
}

/// C1 continua fora do backend, inclusive no irmão.
///
/// `tests/c1_intrinsic_registry_tests.rs` faz este censo sobre
/// `fonte_sem_testes("src/backend_s.rs")`, que por construção nunca observou os
/// módulos de teste. Agora que eles são um arquivo próprio, o censo passa a ter
/// onde ser feito: uma tabela local de grafia -> símbolo de runtime plantada no
/// irmão fica vermelha aqui, pelo mesmo motivo. É a sensitivity M5 da #610.
///
/// A fonte é lida crua, como C1 a lê: o braço procurado É um literal de texto,
/// e `codigo_executavel` apagaria justamente a grafia que decide.
#[test]
fn o_irmao_nao_reconstroi_a_tabela_de_simbolos_de_runtime() {
    let codigo = fonte("tests.rs");
    for entrada in registry::HISTORICAL {
        if let RuntimeRouting::Symbol(simbolo) = entrada.runtime {
            assert!(
                !codigo.contains(&format!("\"{}\" => Some(\"{simbolo}\")", entrada.spelling)),
                "{}: símbolo de runtime passou a ser decidido em src/backend_s/tests.rs",
                entrada.spelling
            );
        }
    }
}
