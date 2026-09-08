//! Guardião estrutural cumulativo da decomposição física da checagem semântica
//! (#619, unidade SEM-1, #628, unidade SEM-2, e #634, unidade SEM-3, do
//! inventário da #601).
//!
//! `src/semantic.rs` é a autoridade da fase semântica. A #619 desce a região
//! `semantic.chamadas.despacho` inteira para `src/semantic/calls.rs`, a #628
//! desce a região `semantic.comandos.verificacao` inteira para
//! `src/semantic/statements.rs` e a #634 desce as três regiões contíguas
//! `semantic.unioes.encaixe`, `semantic.fluxo.retornos` e
//! `semantic.expressoes.verificacao` inteiras para
//! `src/semantic/expressions.rs`, sem dividir essa autoridade: o estado
//! (`SemanticChecker`), a ordem das duas passagens, os escopos, o sistema de
//! tipos e as demais famílias continuam no pai, e o pai continua sendo um
//! arquivo — não virou `mod.rs`.
//!
//! O guardião é cumulativo: ele prova o módulo inteiro a cada corte, não apenas
//! a unidade da vez. As listas abaixo são exaustivas e comparadas por igualdade
//! de conjunto com o disco, então um irmão novo que ninguém registrasse, ou uma
//! região que escorregasse de um arquivo para outro, fica vermelho aqui.
//!
//! O corte da SEM-1 foi o primeiro de `semantic.rs` e por isso pagou a
//! reescrita do guardião C2, exatamente como o inventário da #601 previu. Os
//! cortes criam formas de cegueira silenciosa que este arquivo fecha, e nada
//! mais:
//!
//! 1. um oráculo textual que continuasse lendo só `src/semantic.rs` seguiria
//!    verde e pararia de observar os irmãos — a OG-1 da #601. O único ponto em
//!    que a semântica consulta `method_dispatch::select_impl_method` desceu
//!    com a SEM-1, então a cegueira cairia justamente sobre C2. Os censos de
//!    C2, de C5, da #505, da #514 e da D6 passaram a ler
//!    `fonte_de_modulo::semantic()`, e o teste abaixo prova que a lista lida
//!    por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ou ficar para trás no pai;
//! 3. o corte podia promover visibilidade ou mudar a superfície pública do
//!    módulo. `check_call_expr` (SEM-1), `check_block` (SEM-2) e os sete
//!    símbolos da SEM-3 são os únicos que um chamador de fora do próprio corte
//!    invoca, e os únicos que passaram de privados a `pub(super)`; nenhum item
//!    dos cortes era `pub`, então nenhuma reexportação é devida e nenhuma pode
//!    aparecer;
//! 4. um corte podia atravessar autoridade que não é da fase. Nem a SEM-2 nem
//!    a SEM-3 atravessam nenhuma: `statements.rs` e `expressions.rs` não
//!    consultam `method_dispatch` (C2), não leem o registry declarativo de
//!    intrínsecas (C1), não reconstroem origem de default body por grafia
//!    (C5), não reabrem a conclusão da #600 (C6) e não decidem a política
//!    ainda aberta da #579. O teste abaixo cobra esse zero, e cobra que o
//!    módulo inteiro continue com uma consulta por decisão.
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

/// As regiões que os cortes moveram, e o irmão onde cada uma passa a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("semantic.chamadas.despacho", "calls.rs"),
    ("semantic.comandos.verificacao", "statements.rs"),
    ("semantic.unioes.encaixe", "expressions.rs"),
    ("semantic.fluxo.retornos", "expressions.rs"),
    ("semantic.expressoes.verificacao", "expressions.rs"),
];

/// As regiões que os cortes deixaram onde estavam. `semantic.funcoes.verificacao`
/// é a vizinha imediata anterior do span contíguo da SEM-3 e a anterior da
/// SEM-2, e `semantic.modulos.validacao-local` é a posterior da SEM-1 e da
/// SEM-3: são elas que ficariam vermelhas se um corte tivesse escorregado uma
/// região para qualquer lado.
///
/// A lista também é o que impede a SEM-4 (`tratos`) de vir junto por engano:
/// ela tem de continuar no pai.
const REGIOES_RETIDAS: &[&str] = &[
    "semantic.modulos.validacao-local",
    "semantic.identificadores.namespace-produtor-de-simbolo",
    "semantic.importacoes.familias",
    "semantic.tipos.sistema",
    "semantic.escopos.variaveis",
    "semantic.programa.duas-passagens",
    "semantic.tratos.contratos",
    "semantic.funcoes.verificacao",
];

/// As definições que os cortes moveram inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão, e nenhuma deixada para trás no pai.
const DEFINICOES_MOVIDAS: &[(&str, &str)] = &[
    ("fn check_trait_object_method_call(", "calls.rs"),
    ("fn resolve_impl_method(", "calls.rs"),
    ("fn resolve_qualified_impl_method(", "calls.rs"),
    ("fn generic_map_monomorphic_callee(", "calls.rs"),
    ("fn check_named_function_call(", "calls.rs"),
    ("fn check_call_expr(", "calls.rs"),
    ("fn check_block(", "statements.rs"),
    ("fn check_union_match(", "expressions.rs"),
    ("fn check_if_as_nested_branch(", "expressions.rs"),
    ("fn check_return_stmt(", "expressions.rs"),
    ("fn block_returns(", "expressions.rs"),
    ("fn if_returns(", "expressions.rs"),
    ("fn enum_match_returns(", "expressions.rs"),
    ("fn union_match_returns(", "expressions.rs"),
    ("fn check_value_expr(", "expressions.rs"),
    ("fn function_result_type(", "expressions.rs"),
    ("fn check_expr(", "expressions.rs"),
    ("fn check_pointer_arithmetic(", "expressions.rs"),
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["calls.rs", "expressions.rs", "statements.rs"];

/// Os itens `pub` que cada irmão pode ter. A lista é exaustiva e vazia para
/// todo irmão: nenhum item dos cortes era `pub` antes do move, então nenhum
/// pode ser depois.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[
    ("calls.rs", &[]),
    ("expressions.rs", &[]),
    ("statements.rs", &[]),
];

/// A visibilidade restrita que cada irmão pode ter, exaustiva.
///
/// É o custo Rust inteiro de cada unidade, medido pelo inventário da #601 e
/// reconfirmado contra a main de cada corte, sem nenhum `pub(crate)`:
/// `check_call_expr` é o único símbolo da SEM-1 chamado de fora do corte — de
/// `semantic.expressoes.verificacao`, hoje em `expressions.rs` —, `check_block`
/// é o único da SEM-2 — chamado pelo pai nas famílias de funções e de leques e
/// pelo irmão de expressões nas de uniões e de fluxo — e os sete da SEM-3 são
/// os que o pai e os irmãos chamam: `check_union_match`,
/// `check_if_as_nested_branch` e `check_return_stmt` a partir dos comandos,
/// `block_returns` a partir da alcançabilidade de retorno das famílias de
/// funções e de tratos, `check_value_expr` a partir de quase toda a fase,
/// `function_result_type` a partir do despacho de chamadas e `check_expr` a
/// partir da expressão-comando. Os outros quatro símbolos da SEM-3
/// (`if_returns`, `enum_match_returns`, `union_match_returns` e
/// `check_pointer_arithmetic`) só têm chamador dentro do próprio corte e
/// continuam privados.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[
    ("calls.rs", &["pub(super) fn check_call_expr("]),
    (
        "expressions.rs",
        &[
            "pub(super) fn check_union_match(",
            "pub(super) fn check_if_as_nested_branch(",
            "pub(super) fn check_return_stmt(",
            "pub(super) fn block_returns(",
            "pub(super) fn check_value_expr(",
            "pub(super) fn function_result_type(",
            "pub(super) fn check_expr(",
        ],
    ),
    ("statements.rs", &["pub(super) fn check_block("]),
];

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
            "a região {chave} não é de nenhum corte e deveria continuar em src/semantic.rs"
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
        "a superfície pública de semantic mudou; SEM-1, SEM-2 e SEM-3 são decomposição \
         física e não mudam API pública"
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

/// Nem a SEM-2 nem a SEM-3 atravessam autoridade nenhuma, e os irmãos que elas
/// criam têm de continuar assim.
///
/// `semantic.comandos.verificacao` (SEM-2) e as três regiões da SEM-3 —
/// `semantic.unioes.encaixe`, `semantic.fluxo.retornos` e
/// `semantic.expressoes.verificacao` — não consultam `method_dispatch` (C2),
/// não leem o registry declarativo de intrínsecas (C1), não reconstroem origem
/// de default body por grafia (C5), não reabrem a conclusão arquitetural da
/// #600 (C6) e não decidem a política de alcance ainda aberta da #579. Um zero
/// medido é o que separa "não toquei" de "toquei sem perceber": qualquer uma
/// dessas grafias aparecendo nesses irmãos seria autoridade nova nascendo num
/// corte que se declara físico.
///
/// `calls.rs` fica de fora desta lista de propósito: é o irmão que consome C2,
/// e o teste dele é o `o_irmao_consome_c2_e_nao_cria_uma_segunda_autoridade`.
#[test]
fn os_irmaos_sem_autoridade_nao_atravessam_autoridade_nenhuma() {
    for nome in ["statements.rs", "expressions.rs"] {
        let irmao = codigo_executavel(fonte(nome));
        for grafia in [
            "select_impl_method",
            "select_representative",
            "NivelDeDespacho",
            "nivel_de_despacho",
            "PorUnidadeImportada",
            "method_dispatch",
            "method_index",
            "intrinsics::",
            "__impl_",
        ] {
            assert_eq!(
                irmao.matches(grafia).count(),
                0,
                "src/semantic/{nome} passou a usar `{grafia}`; o corte não atravessa \
                 autoridade nenhuma"
            );
        }
    }
}

/// A ordem de fase não mudou: o pai continua sendo quem chama a verificação de
/// comandos, e o irmão continua sendo só o corpo — inclusive a própria
/// recursão, que desceu junto porque é interna à região.
///
/// As sete chamadas de fora da própria região são as de sempre, e a SEM-3 só
/// mudou de que arquivo três delas partem: duas na verificação de corpos de
/// topo e duas no `encaixe` de leque continuam no pai; uma no `encaixe` de
/// união e duas no ramo `talvez`/`senão` passaram a partir de
/// `expressions.rs`, porque foram essas regiões que desceram. Se uma delas
/// migrasse de arquivo, ou se aparecesse uma oitava, a fronteira que torna
/// `pub(super)` suficiente teria mudado.
#[test]
fn o_pai_continua_chamando_a_verificacao_de_comandos() {
    let bruto = pai();
    let codigo_do_pai = codigo_executavel(bruto);
    let irmao = codigo_executavel(fonte("statements.rs"));
    let expressoes = codigo_executavel(fonte("expressions.rs"));
    assert_eq!(
        codigo_do_pai.matches("self.check_block(").count(),
        4,
        "src/semantic.rs deveria chamar `check_block` exatamente quatro vezes"
    );
    assert_eq!(
        expressoes.matches("self.check_block(").count(),
        3,
        "src/semantic/expressions.rs deveria chamar `check_block` exatamente três vezes"
    );
    assert_eq!(
        codigo_do_pai.matches("self.check_block(").count()
            + expressoes.matches("self.check_block(").count(),
        7,
        "as sete chamadas de fora da região de comandos deixaram de ser sete"
    );
    assert_eq!(
        irmao.matches("self.check_block(").count(),
        3,
        "src/semantic/statements.rs deveria conter só as três recursões da própria região"
    );

    // Duas dessas chamadas moram na verificação de corpos de topo, entre os
    // marcadores da região que a hospeda: é o ponto em que a fase entra nos
    // comandos, e um deslocamento de fase mudaria este vizinho.
    let inicio = bruto
        .find("// @pinker-nav:start semantic.funcoes.verificacao")
        .expect("a família de funções continua no pai");
    let fim = bruto
        .find("// @pinker-nav:end semantic.funcoes.verificacao")
        .expect("a família de funções continua fechada no pai");
    assert!(
        inicio < fim,
        "os marcadores da família de funções inverteram"
    );
    assert_eq!(
        bruto[inicio..fim].matches("self.check_block(").count(),
        2,
        "a entrada da verificação de corpos de topo nos comandos saiu da família de funções"
    );
}

/// A ordem de fase não mudou: quem chama o despacho de chamadas continua sendo
/// a verificação de expressões, e `calls.rs` continua sendo só o corpo. É a
/// fronteira que torna `pub(super)` suficiente — e a sensitivity M9 da #619
/// perturba exatamente o que este teste ancora.
///
/// A SEM-3 não mudou a fase, mudou o arquivo: a verificação de expressões
/// desceu para `expressions.rs` e levou a chamada consigo. O pai deixou de
/// chamar, e é isso que este teste passa a exigir — um `check_call_expr`
/// reaparecendo no pai seria a chamada duplicada ou a região voltando.
#[test]
fn a_verificacao_de_expressoes_continua_chamando_o_despacho_de_chamadas() {
    let pai = codigo_executavel(pai());
    let expressoes = codigo_executavel(fonte("expressions.rs"));
    assert_eq!(
        pai.matches("self.check_call_expr(").count(),
        0,
        "src/semantic.rs voltou a chamar `check_call_expr`; a chamada desceu com a SEM-3"
    );
    assert_eq!(
        expressoes.matches("self.check_call_expr(").count(),
        1,
        "src/semantic/expressions.rs deveria chamar `check_call_expr` exatamente uma vez"
    );
    // A única chamada mora na verificação de expressões, depois do início do
    // despacho que a hospeda: um deslocamento de fase mudaria este vizinho.
    let chamada = expressoes
        .find("self.check_call_expr(")
        .expect("chamada presente no irmão de expressões");
    let regiao = expressoes
        .find("ExprKind::Call")
        .expect("o despacho de expressões mora no irmão");
    assert!(
        regiao < chamada,
        "a chamada de `check_call_expr` saiu do despacho de expressões"
    );
}
