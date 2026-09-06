//! Guardião estrutural da decomposição física do backend montável (#610,
//! unidade BS-3; #612, unidade BS-2; #615, unidade BS-1).
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
//! A BS-2 acrescentou a forma seguinte da mesma cegueira, e ela é maior:
//! `render_abi.rs` é produção, não teste. Ler só o pai deixou de ser ler a
//! produção, então os três censos passaram a ler
//! `fonte_de_modulo::backend_s_producao()` — o módulo inteiro, cada arquivo
//! cortado no seu próprio primeiro `#[cfg(test)]`. O que este guardião
//! acrescenta é o que aquele repoint não alcança sozinho: que nenhum irmão de
//! produção esconda produção atrás de um `#[cfg(test)]`, que as funções
//! movidas existam uma vez só e no arquivo certo, que a única `pub` do irmão
//! seja a que já era `pub` antes do move, e que o caminho público
//! `pinker_v0::backend_s::render_program` continue existindo.
//!
//! A BS-1 acrescentou a terceira forma: o irmão que ela cria precisa devolver
//! ao pai a função que ele chama nas duas entradas públicas do caminho
//! montável, e essa é a primeira visibilidade restrita do módulo. Um `pub(`
//! sem lista deixaria de ser corte físico e viraria promoção livre, então a
//! lista autorizada abaixo passou a cobrir também a visibilidade restrita, item
//! a item: `pub(crate)` continua proibido em qualquer irmão, e qualquer
//! `pub(super)` além do declarado fica vermelho.
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

/// As regiões que a decomposição física já moveu, e o irmão onde passam a
/// morar. Duas da BS-3 (#610), três da BS-2 (#612), sete da BS-1 (#615).
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("evidencia.backend-s.proveniencia-de-ponteiro", "tests.rs"),
    ("evidencia.backend-s.selecao-de-rota-nativa", "tests.rs"),
    (
        "backend-s.renderizacao.abi-textual-programa",
        "render_abi.rs",
    ),
    (
        "backend-s.renderizacao.abi-textual-instrucoes",
        "render_abi.rs",
    ),
    (
        "backend-s.renderizacao.abi-textual-componentes",
        "render_abi.rs",
    ),
    ("backend-s.lowering.globais-rodata", "external_callconv.rs"),
    ("backend-s.lowering.funcoes-frames", "external_callconv.rs"),
    (
        "backend-s.lowering.blocos-terminadores",
        "external_callconv.rs",
    ),
    (
        "backend-s.lowering.operacoes-memoria",
        "external_callconv.rs",
    ),
    ("backend-s.lowering.chamadas-sysv", "external_callconv.rs"),
    (
        "backend-s.lowering.objetos-trato-nativos",
        "external_callconv.rs",
    ),
    ("backend-s.lowering.falar-runtime", "external_callconv.rs"),
];

/// As funções que a decomposição moveu inteiras, e o irmão onde passam a
/// morar. Treze da BS-2 (#612), uma da BS-1 (#615). Uma definição, no irmão, e
/// nenhuma deixada para trás no pai.
const FUNCOES_MOVIDAS: &[(&str, &str)] = &[
    ("render_program", "render_abi.rs"),
    ("render_instruction", "render_abi.rs"),
    ("render_terminator", "render_abi.rs"),
    ("render_unary", "render_abi.rs"),
    ("render_binop", "render_abi.rs"),
    ("render_operand", "render_abi.rs"),
    ("render_temp", "render_abi.rs"),
    ("render_slot", "render_abi.rs"),
    ("join_or_empty", "render_abi.rs"),
    ("render_abi_params", "render_abi.rs"),
    ("render_abi_return", "render_abi.rs"),
    ("render_call_site", "render_abi.rs"),
    ("render_abi_call_args", "render_abi.rs"),
    ("extract_external_callconv_program", "external_callconv.rs"),
];

/// Irmãos que carregam produção, não teste. São eles que os censos de
/// autoridade precisam continuar observando depois da BS-2.
const IRMAOS_DE_PRODUCAO: &[&str] = &["external_callconv.rs", "render_abi.rs"];

/// Os itens `pub` que cada irmão pode ter, e por quê. A lista é exaustiva: o
/// corte físico não promove nada.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[
    // BS-3: ponte que devolve o pai aos módulos movidos, dentro de um
    // `mod tests` privado e `#[cfg(test)]`.
    ("tests.rs", &["pub use super::*;"]),
    // BS-2: `render_program` já era `pub` em `src/backend_s.rs` antes do move,
    // e o pai a reexporta para preservar o caminho público.
    ("render_abi.rs", &["pub fn render_program"]),
    // BS-1: nada público desceu; a função movida era privada ao módulo e
    // continua sendo, com visibilidade restrita listada logo abaixo.
    ("external_callconv.rs", &[]),
];

/// A visibilidade restrita que cada irmão pode ter, e por quê. Também
/// exaustiva, e por isso separada de [`PUB_AUTORIZADO`]: `pub(super)` não é
/// superfície pública, mas também não é o privado que o corte tinha antes.
///
/// `pub(crate)` não aparece aqui e não pode aparecer: promover um item para o
/// crate inteiro é mudança de autoridade, não decomposição física.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[
    ("tests.rs", &[]),
    ("render_abi.rs", &[]),
    // BS-1: `extract_external_callconv_program` era privada ao módulo
    // `backend_s` e é chamada pelas duas entradas públicas que ficaram no pai
    // (`emit_external_toolchain_subset` e `..._nativo`). `pub(super)` é o
    // mínimo que devolve ao pai a função que ele já chamava.
    (
        "external_callconv.rs",
        &["pub(super) fn extract_external_callconv_program"],
    ),
];

/// Os dois módulos de teste que viajaram inteiros, sem renomeação.
const MODULOS_MOVIDOS: &[&str] = &[
    "tests_proveniencia_de_ponteiro",
    "tests_selecao_de_rota_nativa",
];

/// Amostra de regiões que os cortes deixaram onde estavam. Não é a lista
/// completa do arquivo: é o controle de que o corte não arrastou vizinhança.
/// As duas primeiras são exatamente as vizinhas imediatas do span da BS-1 —
/// a que vem antes e a que vem depois —, e são elas que ficariam vermelhas se
/// o corte tivesse escorregado uma região para qualquer lado.
const REGIOES_RETIDAS: &[&str] = &[
    "backend-s.abi.registradores-argumentos",
    "backend-s.renderizacao.callconv-programa",
    "backend-s.dados.strings-rodata",
    "backend-s.runtime.intrinsecas-por-aridade",
    "backend-s.runtime.simbolos-intrinsecas",
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
/// tests.rs` e `tests/issue497_abi_symbol_isolation_tests.rs` cortam cada
/// arquivo do módulo no seu primeiro `#[cfg(test)]`. No pai o corte só cai no
/// fim do arquivo enquanto a declaração `mod tests;` for a última coisa dele:
/// subi-la para o topo deixaria os três verdes e cegos para toda a produção
/// que ficou no pai. A declaração `mod render_abi;` é produção e vive acima do
/// corte, junto do resto.
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
    for (nome, arquivo) in FUNCOES_MOVIDAS {
        let definicao = format!("fn {nome}(");
        assert_eq!(
            codigo.matches(&definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo backend_s"
        );
        assert_eq!(
            pai.matches(&definicao).count(),
            0,
            "a implementação de `{nome}` ficou para trás em src/backend_s.rs"
        );
        assert!(
            codigo_executavel(fonte(arquivo))
                .matches(&definicao)
                .count()
                == 1,
            "`{nome}` deveria morar em src/backend_s/{arquivo}"
        );
    }
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

/// A decomposição é física: não promove nada para fora do backend. Cada irmão
/// tem duas listas exaustivas — o que pode ser `pub` e o que pode ter
/// visibilidade restrita —, e nada mais: nem `pub(crate)`, nem um `pub(super)`
/// a mais, nem uma segunda reexportação. É a sensitivity M4 da #610, da #612 e
/// da #615.
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
        let autorizados = itens_autorizados(PUB_AUTORIZADO, nome, "`pub`");
        assert_eq!(
            codigo.matches("pub ").count(),
            autorizados.len(),
            "src/backend_s/{nome} tem mais itens `pub` do que o corte previa"
        );
        let restritos = itens_autorizados(PUB_RESTRITO_AUTORIZADO, nome, "visibilidade restrita");
        assert_eq!(
            codigo.matches("pub(").count(),
            restritos.len(),
            "src/backend_s/{nome} tem mais visibilidade restrita do que o corte previa"
        );
        for item in autorizados.iter().chain(restritos.iter()) {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/backend_s/{nome} deveria conter `{item}` exatamente uma vez"
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
            panic!("src/backend_s/{nome} não declarou que itens de {classe} o corte previa")
        })
}

/// `render_program` já era `pub` antes da BS-2, e `pinker_v0::backend_s::
/// render_program` é caminho público da lib. O move desce a definição um nível;
/// sem a reexportação do pai o caminho sumiria — remoção de API pública
/// disfarçada de decomposição física. A coerção abaixo é estática: se a
/// reexportação ou a assinatura mudarem, isto não compila.
const _CAMINHO_PUBLICO_PRESERVADO: fn(&pinker_v0::backend_text::BackendTextProgram) -> String =
    pinker_v0::backend_s::render_program;

#[test]
fn o_pai_reexporta_a_unica_funcao_publica_que_desceu() {
    let pai = codigo_executavel(fonte("backend_s.rs"));
    assert_eq!(
        pai.matches("pub use render_abi::render_program;").count(),
        1,
        "src/backend_s.rs deveria reexportar `render_abi::render_program` exatamente uma vez"
    );
    assert_eq!(
        pai.matches("pub fn ").count(),
        3,
        "as três entradas públicas que ficaram no pai são `emit_from_selected`, \
         `emit_external_toolchain_subset` e `emit_external_toolchain_subset_nativo`"
    );
}

/// Um irmão de produção que ganhasse um `#[cfg(test)]` esconderia tudo o que
/// viesse depois dos três censos que cortam ali — a mesma cegueira de subir
/// `mod tests;` no pai, um arquivo adiante.
#[test]
fn irmao_de_producao_nao_esconde_producao_atras_de_cfg_test() {
    for nome in IRMAOS_DE_PRODUCAO {
        assert_eq!(
            codigo_executavel(fonte(nome))
                .matches("#[cfg(test)]")
                .count(),
            0,
            "src/backend_s/{nome} é produção e não pode cortar o censo com um `#[cfg(test)]`"
        );
    }
}

/// O censo de produção do módulo tem que conter a produção de todo arquivo do
/// módulo. É o que faz um irmão novo entrar automaticamente em C1, na
/// superfície de família e no isolamento de símbolo de ABI, em vez de nascer
/// invisível.
#[test]
fn o_censo_de_producao_cobre_todo_arquivo_do_modulo() {
    let producao = fonte_de_modulo::backend_s_producao();
    for (nome, fonte) in BACKEND_S_ARQUIVOS {
        let esperada = fonte
            .split_once("\n#[cfg(test)]")
            .map_or(*fonte, |(antes, _)| antes);
        assert!(
            producao.contains(esperada),
            "a produção de {nome} ficou fora do censo de produção do módulo"
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
fn nenhum_irmao_reconstroi_a_tabela_de_simbolos_de_runtime() {
    for (nome, codigo) in BACKEND_S_ARQUIVOS {
        if *nome == "backend_s.rs" {
            continue;
        }
        for entrada in registry::HISTORICAL {
            if let RuntimeRouting::Symbol(simbolo) = entrada.runtime {
                assert!(
                    !codigo.contains(&format!("\"{}\" => Some(\"{simbolo}\")", entrada.spelling)),
                    "{}: símbolo de runtime passou a ser decidido em src/backend_s/{nome}",
                    entrada.spelling
                );
            }
        }
    }
}
