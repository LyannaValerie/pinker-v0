//! Guardião estrutural da decomposição física do interpretador (#608, unidade
//! INT-1 administrativa; #642, campanha INT-TESTS — a unidade `#601/INT-1`
//! original do inventário).
//!
//! A #601 registrou que `src/interpreter.rs` não tinha guardião estrutural por
//! caminho, e a #607 mediu o custo exato do corte: perder a inclusão do irmão,
//! duplicar `try_call_intrinsic`, deixar a implementação antiga no pai, alargar
//! visibilidade ou desfazer as duas âncoras cartográficas novas não quebrava
//! teste nenhum. Este arquivo fecha esse buraco e nada mais.
//!
//! A INT-TESTS acrescenta um irmão de natureza diferente: `tests.rs` é somente
//! teste. Ele traz duas formas próprias de dano silencioso. A primeira é o
//! inverso da que o irmão de produção corre: produção descer para dentro de um
//! arquivo que nenhum censo de autoridade observa, escondida atrás do
//! `#[cfg(test)]` do módulo. A segunda é a inclusão subir para o topo do pai —
//! os censos que cortam `src/interpreter.rs` no primeiro `#[cfg(test)]`
//! passariam a cortar o arquivo inteiro, continuariam verdes e parariam de
//! observar a produção.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre o conteúdo das regiões nem dos testes movidos. Afirma
//! coisas mecânicas: o conjunto de arquivos de `src/interpreter/`, o wiring do
//! `mod` no pai, a presença única de cada região cartografada no arquivo certo,
//! que o irmão de teste é só teste e entra só sob `#[cfg(test)]`, e que a
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

/// Regiões que a decomposição física moveu de `src/interpreter.rs` para um
/// irmão, e o irmão onde passam a morar. Dez da #608, junto com a porta
/// `try_call_intrinsic` inteira — `despacho-hospedado` é a âncora nova do
/// prefixo que antes vivia, falsamente, dentro de `acaso` —, e duas da
/// INT-TESTS (#642), dentro dos módulos de teste.
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
    // INT-TESTS (#642): as duas regiões cartografadas que viajam dentro dos
    // módulos `#[cfg(test)]`, na unidade `#601/INT-1` original do inventário.
    ("interpreter.unioes.contabilidade-dominios", "tests.rs"),
    ("evidencia.processos.saida-runtime-hospedado", "tests.rs"),
];

/// Regiões que continuam no pai. `interpreter.memoria.estado-enderecavel` é a
/// segunda âncora nova da #608: o bloco de memória endereçável que a
/// cartografia anterior atribuía a `acaso` continua fisicamente no pai, agora
/// com key própria. Todas as onze são de produção — depois da INT-TESTS o pai
/// não carrega mais região de teste nenhuma.
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
];

/// O único símbolo que o move obrigou a expor ao pai. A #607 mediu o custo:
/// um `pub(super)`, zero `pub(crate)` novo, zero campo promovido.
const EXPOSICOES_NECESSARIAS: &[&str] = &["try_call_intrinsic"];

/// Irmãos que carregam só teste. Eles entram no crate exclusivamente sob
/// `#[cfg(test)]` e nenhum censo de autoridade precisa observá-los.
const IRMAOS_SOMENTE_TESTE: &[&str] = &["tests.rs"];

/// Os itens `pub` que cada irmão pode ter, e por quê. A lista é exaustiva: o
/// corte físico não promove nada, nem para o crate nem para fora dele.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[
    // #608: nada público desceu; `try_call_intrinsic` é `pub(super)`, e a
    // visibilidade restrita é contada por [`EXPOSICOES_NECESSARIAS`].
    ("hosted_intrinsics.rs", &[]),
    // INT-TESTS (#642): a ponte que devolve o pai aos seis módulos movidos,
    // que continuam escritos com `use super::*`. `mod tests` é privado e
    // `#[cfg(test)]`: a ponte não amplia superfície nenhuma para fora do
    // módulo `interpreter`.
    ("tests.rs", &["pub use super::*;"]),
];

/// Os seis módulos `#[cfg(test)]` que a INT-TESTS moveu inteiros, sem
/// renomeação. Presença única no irmão, ausência no pai.
const MODULOS_DE_TESTE_MOVIDOS: &[&str] = &[
    "fase244_trait_runtime_tests",
    "d3_callable_lifetime_tests",
    "fase246_public_memory_tests",
    "hr3_union_budget_tests",
    "contabilidade_dominios_uniao_tests",
    "part_d_saida_processo_runtime_tests",
];

fn pub_autorizado(nome: &str) -> &'static [&'static str] {
    PUB_AUTORIZADO
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, itens)| *itens)
        .unwrap_or_else(|| panic!("{nome} entrou no módulo sem lista de `pub` autorizado"))
}

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
            "src/interpreter/hosted_intrinsics.rs".to_string(),
            "src/interpreter/tests.rs".to_string()
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
    // O irmão somente teste entra no crate só sob `#[cfg(test)]`. Sem o
    // atributo ele passaria a compilar em build de produção e a ponte
    // `pub use super::*` deixaria de ser test-only.
    let bruto = fonte("interpreter.rs");
    for nome in IRMAOS_SOMENTE_TESTE {
        let modulo = nome.trim_end_matches(".rs");
        assert_eq!(
            bruto
                .matches(&format!("#[cfg(test)]\nmod {modulo};"))
                .count(),
            1,
            "`mod {modulo};` deveria ser declarado exatamente uma vez e sob `#[cfg(test)]`"
        );
    }
}

/// Três censos de autoridade cortam `src/interpreter.rs` no primeiro
/// `#[cfg(test)]` — entre eles o da superfície de família, que desce no módulo
/// inteiro por `interpreter_caminhos`. Enquanto a declaração do irmão de teste
/// for a última coisa do pai, o corte cai no fim e eles continuam vendo a
/// produção inteira. Se ela subir para o topo, eles cortam o arquivo inteiro,
/// continuam verdes e param de observar tudo: é a forma silenciosa OG-1 que a
/// #601 registrou, e o motivo de este teste existir.
#[test]
fn o_corte_dos_oraculos_no_primeiro_cfg_test_ainda_ve_a_producao_inteira() {
    let pai = fonte("interpreter.rs");
    let corte = pai
        .find("\n#[cfg(test)]")
        .expect("o pai declara o irmão de teste sob #[cfg(test)]");
    let depois = pai[corte + 1..].trim_end();
    assert_eq!(
        depois, "#[cfg(test)]\nmod tests;",
        "depois do primeiro `#[cfg(test)]` o pai passou a ter conteúdo que os \
         censos de autoridade deixariam de observar"
    );
    for chave in REGIOES_RETIDAS {
        assert!(
            pai[..corte].contains(&format!("// @pinker-nav:start {chave}")),
            "a região de produção {chave} caiu depois do corte dos censos"
        );
    }
}

/// O irmão somente teste é só teste. Produção que descesse para dentro dele
/// ficaria escondida atrás do `#[cfg(test)]` do módulo: compilaria, passaria, e
/// nenhum censo de autoridade a observaria.
///
/// O oráculo é o código executável do arquivo fora de qualquer bloco: sobra
/// exatamente a ponte e os cabeçalhos dos módulos de teste. Um item de
/// produção no topo aparece aqui; um escondido dentro de um `mod` de teste não
/// aparece, e é por isso que o corpo dos módulos continua sendo problema da
/// matriz comportamental, não deste arquivo.
#[test]
fn o_irmao_somente_teste_nao_carrega_producao() {
    let mut esperado = vec!["pub use super::*;".to_string()];
    for modulo in MODULOS_DE_TESTE_MOVIDOS {
        esperado.push(format!("#[cfg(test)] mod {modulo}"));
    }
    for nome in IRMAOS_SOMENTE_TESTE {
        assert_eq!(
            topo_fora_de_blocos(fonte(nome)),
            esperado.join(" "),
            "src/interpreter/{nome} ganhou item de topo que não é a ponte nem módulo de teste"
        );
    }
}

/// O código executável de `fonte` fora de qualquer par de chaves, com espaços
/// normalizados. Comentário e literal já saem em [`codigo_executavel`], então
/// uma linha de fonte Pinker dentro de um literal de teste não conta como item
/// de topo.
fn topo_fora_de_blocos(fonte: &str) -> String {
    let mut profundidade = 0usize;
    let mut topo = String::new();
    for caractere in codigo_executavel(fonte).chars() {
        match caractere {
            '{' => profundidade += 1,
            '}' => profundidade = profundidade.saturating_sub(1),
            _ if profundidade == 0 => topo.push(caractere),
            _ => {}
        }
    }
    topo.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cada módulo movido existe uma vez, no irmão, e nenhum ficou para trás nem
/// foi duplicado no pai.
#[test]
fn cada_modulo_de_teste_movido_aparece_uma_vez_no_irmao() {
    let pai = fonte("interpreter.rs");
    let filho = fonte("tests.rs");
    for modulo in MODULOS_DE_TESTE_MOVIDOS {
        let declaracao = format!("mod {modulo} {{");
        assert_eq!(
            filho.matches(&declaracao).count(),
            1,
            "`{declaracao}` deveria aparecer exatamente uma vez em src/interpreter/tests.rs"
        );
        assert_eq!(
            pai.matches(&declaracao).count(),
            0,
            "`{declaracao}` ficou para trás em src/interpreter.rs"
        );
    }
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
/// o único `pub(super)` do módulo é o justificado pelo move, e de que cada
/// `pub` de irmão é exatamente um item da lista autorizada — a ponte test-only
/// da INT-TESTS é o único que existe, e ela vive dentro de um `mod tests`
/// privado e `#[cfg(test)]`.
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
        let autorizados = pub_autorizado(nome);
        assert_eq!(
            codigo.matches("pub ").count(),
            autorizados.len(),
            "src/interpreter/{nome} passou a exportar superfície pública nova"
        );
        for item in autorizados {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/interpreter/{nome} deveria conter `{item}` exatamente uma vez"
            );
        }
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
