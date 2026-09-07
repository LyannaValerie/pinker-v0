//! Guardião estrutural da decomposição física do lowering AST → IR
//! (#621, unidade IR-1 do inventário da #601).
//!
//! `src/ir.rs` é a autoridade do lowering. A #621 desce o `impl FunctionLowerer`
//! inteiro — as cinco regiões `ir.lowering.funcoes-blocos`,
//! `ir.lowering.comandos-controle`, `ir.lowering.expressoes-valores`,
//! `ir.lowering.bindings-escopos` e `ir.lowering.constantes` — para
//! `src/ir/lowering.rs` sem dividir essa autoridade: o modelo da IR, as
//! `struct FunctionLowerer`/`LoweringContext` e todo o estado, a orquestração
//! do programa, a internação de identidade resolvida e a renderização textual
//! continuam no pai, e o pai continua sendo um arquivo — não virou `mod.rs`.
//!
//! O corte é o primeiro de `ir.rs` e por isso paga a reescrita dos oráculos que
//! liam o monólito por caminho fixo, exatamente como o inventário da #601
//! previu. Ele cria formas de cegueira silenciosa que este arquivo fecha, e
//! nada mais:
//!
//! 1. um oráculo textual que continuasse lendo só `src/ir.rs` seguiria verde e
//!    pararia de observar o irmão — a OG-1 da #601. O único ponto em que o
//!    lowering consulta `method_dispatch::select_impl_method` desceu junto,
//!    então a cegueira cairia justamente sobre C2. Os censos de C2, de C5, da
//!    D6 e da Parte G passaram a ler `fonte_de_modulo::ir()`, e o teste abaixo
//!    prova que a lista lida por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ou ficar para trás no pai;
//! 3. o corte podia promover visibilidade ou mudar a superfície pública do
//!    módulo. `new`, `lower_function` e `lower_const` são os três símbolos que o
//!    pai chama e os únicos que passaram de privados a `pub(super)`; nenhum item
//!    do corte era `pub`, então nenhuma reexportação é devida e nenhuma pode
//!    aparecer;
//! 4. o corte podia arrastar a validação da IR ou a fronteira de CFG para o
//!    irmão. Nenhuma das duas desceu: `src/ir_validate.rs` e `src/cfg_ir.rs`
//!    continuam donos do que sempre foram.
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

use fonte_de_modulo::{ir, IR_ARQUIVOS};
use rust_source::codigo_executavel;

/// As regiões que a IR-1 moveu, e o irmão onde passam a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    ("ir.lowering.funcoes-blocos", "lowering.rs"),
    ("ir.lowering.comandos-controle", "lowering.rs"),
    ("ir.lowering.expressoes-valores", "lowering.rs"),
    ("ir.lowering.bindings-escopos", "lowering.rs"),
    ("ir.lowering.constantes", "lowering.rs"),
];

/// As regiões que o corte deixou onde estavam. `ir.lowering.identidade-resolvida`
/// e `ir.renderizacao.textual` são as vizinhas imediatas do span da IR-1 — a que
/// vem antes e a que vem depois —, e são elas que ficariam vermelhas se o corte
/// tivesse escorregado uma região para qualquer lado. As outras são as unidades
/// IR-2, IR-3 e IR-4 do mesmo inventário, que esta Task não executa.
const REGIOES_RETIDAS: &[&str] = &[
    "ir.modelo.representacao",
    "ir.tipos.identidade-resolvida",
    "ir.lowering.programa-orquestracao",
    "ir.lowering.contexto-declaracoes",
    "ir.lowering.assinaturas-intrinsecos",
    "ir.lowering.metodos-identidade",
    "ir.lowering.identidade-resolvida",
    "ir.renderizacao.textual",
    "ir.tipos.conversao-ast",
];

/// As definições que a IR-1 moveu inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão, e nenhuma deixada para trás no pai.
///
/// `new` aparece com a assinatura inteira porque o pai tem um `new` próprio, de
/// `TypeRefIR`: um oráculo que procurasse só `fn new(` não distinguiria os dois
/// e ficaria verde com a implementação duplicada.
const DEFINICOES_MOVIDAS: &[(&str, &str)] = &[
    ("fn allocate_binding(", "lowering.rs"),
    ("fn callable_metadata_for_expr(", "lowering.rs"),
    ("fn callable_metadata_for_value(", "lowering.rs"),
    ("fn callable_metadata_from_return_type(", "lowering.rs"),
    ("fn callable_ret_identity(", "lowering.rs"),
    ("fn concrete_snapshot_size(", "lowering.rs"),
    ("fn ensure_fnref_wrapper(", "lowering.rs"),
    ("fn function_value_identity(", "lowering.rs"),
    ("fn impl_receiver_key(", "lowering.rs"),
    ("fn lower_block(", "lowering.rs"),
    ("fn lower_break(", "lowering.rs"),
    ("fn lower_closure_function(", "lowering.rs"),
    ("fn lower_const(", "lowering.rs"),
    ("fn lower_continue(", "lowering.rs"),
    ("fn lower_enum_match(", "lowering.rs"),
    ("fn lower_enum_pattern(", "lowering.rs"),
    ("fn lower_falar(", "lowering.rs"),
    ("fn lower_function(", "lowering.rs"),
    ("fn lower_if(", "lowering.rs"),
    ("fn lower_inline_asm(", "lowering.rs"),
    ("fn lower_let(", "lowering.rs"),
    ("fn lower_return(", "lowering.rs"),
    ("fn lower_stmt(", "lowering.rs"),
    ("fn lower_trait_call(", "lowering.rs"),
    ("fn lower_union_match(", "lowering.rs"),
    ("fn lower_value(", "lowering.rs"),
    ("fn lower_while(", "lowering.rs"),
    ("fn new(context: &'a LoweringContext)", "lowering.rs"),
    ("fn next_block_label(", "lowering.rs"),
    ("fn nominal_name_of_value(", "lowering.rs"),
    ("fn pointee_identity_of(", "lowering.rs"),
    ("fn pointer_element_layout(", "lowering.rs"),
    ("fn pointer_pointee_for_expr(", "lowering.rs"),
    ("fn pop_scope(", "lowering.rs"),
    ("fn push_scope(", "lowering.rs"),
    ("fn raw_function_metadata_for_expr(", "lowering.rs"),
    ("fn raw_function_metadata_for_value(", "lowering.rs"),
    ("fn raw_ret_identity(", "lowering.rs"),
    ("fn resolve_binding(", "lowering.rs"),
    ("fn resolve_closure(", "lowering.rs"),
    ("fn resolve_existing_binding(", "lowering.rs"),
    ("fn resolve_impl_method(", "lowering.rs"),
    ("fn resolve_qualified_impl_method(", "lowering.rs"),
    ("fn resolve_trait_impl_symbol(", "lowering.rs"),
    ("fn trait_object_name_for_expr(", "lowering.rs"),
    ("fn trait_vtable(", "lowering.rs"),
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["lowering.rs"];

/// Os itens `pub` que cada irmão pode ter. A lista é exaustiva e vazia: nenhum
/// item do corte era `pub` antes do move, então nenhum pode ser depois.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[("lowering.rs", &[])];

/// A visibilidade restrita que cada irmão pode ter, exaustiva.
///
/// É o custo Rust inteiro da unidade. O inventário da #601 previu `4
/// pub(super)` e nenhum `pub(crate)`; o baseline atual desmentiu um dos quatro:
/// as únicas ocorrências de `resolve_closure` fora do corte são comentários, e
/// comentário não é chamada. Três exposições bastam — `new` e `lower_function`
/// porque `lower_program`/`lower_program_composto` constroem o lowerer, e
/// `lower_const` porque a orquestração despacha constantes.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[(
    "lowering.rs",
    &[
        "pub(super) fn new(",
        "pub(super) fn lower_function(",
        "pub(super) fn lower_const(",
    ],
)];

/// A superfície pública do módulo, congelada item a item.
///
/// É o contrato `PUBLIC_PATHS_BEFORE == AFTER` da #621 na forma que um teste
/// consegue observar. O corte não contém nenhum item público, então esta lista
/// é exatamente a de antes do move.
const API_PUBLICA_CONGELADA: &[&str] = &[
    "BinaryOpIR",
    "BindingIR",
    "BlockIR",
    "ConstIR",
    "EnumMatchArmIR",
    "EnumMatchIR",
    "EnumPatternIR",
    "EnumPatternPayloadIR",
    "EnumPayloadMetaIR",
    "EnumVariantMetaIR",
    "FalarArgIR",
    "FunctionIR",
    "InlineAsmOperandIR",
    "InstructionIR",
    "LocalIR",
    "MapKeyIR",
    "MapValueIR",
    "NominalTypeKindIR",
    "ProgramIR",
    "ResolvedSignatureIR",
    "ResolvedTypeIR",
    "ResolvedTypeId",
    "ResolvedTypeParts",
    "ResolvedTypeTable",
    "ScalarTypeIR",
    "TypeIR",
    "TypeRefIR",
    "UnaryOpIR",
    "UnionMatchArmIR",
    "UnionMatchIR",
    "UnionMemberIR",
    "UnionTypeIR",
    "UnionTypeId",
    "ValueIR",
    "lower_program",
    "lower_program_composto",
    "render_program",
    "validate_resolved_type_reference",
    "validate_resolved_type_table",
    "validate_union_match_coverage",
    "validate_union_member_identity",
    "validate_union_member_reference",
    "validate_union_reference",
    "validate_union_registry",
    "validate_union_registry_identities",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ir")
}

fn fonte(nome: &str) -> &'static str {
    IR_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

fn pai() -> &'static str {
    fonte("ir.rs")
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo que lê o módulo por `fonte_de_modulo` — e o de C2 é um deles.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/ir/ legível")
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
    let declarados: BTreeSet<String> = IR_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "ir.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/ir/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate e o que desceu deixa de existir sem
/// que nada fique vermelho. É a sensitivity M1 da #621.
#[test]
fn o_pai_inclui_o_irmao() {
    let codigo = codigo_executavel(pai());
    for (nome, _) in IR_ARQUIVOS {
        if *nome == "ir.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/ir.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
        // O filho é detalhe físico, não caminho público. `pub mod lowering;`
        // criaria `pinker_v0::ir::lowering::*` sem apagar nenhum dos itens
        // congelados — ampliação de superfície que o censo de itens não veria
        // sozinho.
        assert_eq!(
            codigo.matches(&format!("pub {declaracao}")).count(),
            0,
            "src/ir.rs tornou o irmão `{modulo}` um caminho público"
        );
    }
    // O pai continua sendo um arquivo: a #621 mantém a forma que a #608 decidiu
    // e a #610, a #612, a #615, a #617 e a #619 mantiveram. Convertê-lo em
    // `mod.rs` moveria o campo `file` das nove regiões que ficaram e faria as
    // projeções FROZEN pararem com E-SNAP-PATH-ALTERADO.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. São as sensitivities M2 e M3 da #621.
#[test]
fn cada_regiao_e_cada_definicao_movida_aparece_uma_vez_no_arquivo_certo() {
    let modulo = ir();
    for (chave, arquivo) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/ir/{arquivo}"
        );
        assert!(
            !pai().contains(&marcador),
            "a região {chave} ficou para trás em src/ir.rs"
        );
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            pai().contains(&marcador),
            "a região {chave} não é da IR-1 e deveria continuar em src/ir.rs"
        );
    }

    let codigo = codigo_executavel(&modulo);
    let codigo_do_pai = codigo_executavel(pai());
    for (definicao, arquivo) in DEFINICOES_MOVIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo ir"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            0,
            "a implementação de `{definicao}` ficou para trás em src/ir.rs"
        );
        assert_eq!(
            codigo_executavel(fonte(arquivo)).matches(definicao).count(),
            1,
            "`{definicao}` deveria morar em src/ir/{arquivo}"
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
            "`{marcador}` deveria aparecer exatamente uma vez no módulo ir"
        );
    }
}

/// A decomposição é física: não promove nada. Cada irmão tem duas listas
/// exaustivas — o que pode ser `pub` e o que pode ter visibilidade restrita —,
/// e nada mais. É a sensitivity M4 da #621.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in IR_ARQUIVOS {
        if *nome == "ir.rs" {
            continue;
        }
        let codigo = codigo_executavel(fonte);
        let autorizados = itens_autorizados(PUB_AUTORIZADO, nome, "`pub`");
        assert_eq!(
            codigo.matches("pub ").count(),
            autorizados.len(),
            "src/ir/{nome} tem mais itens `pub` do que o corte previa"
        );
        let restritos = itens_autorizados(PUB_RESTRITO_AUTORIZADO, nome, "visibilidade restrita");
        assert_eq!(
            codigo.matches("pub(").count(),
            restritos.len(),
            "src/ir/{nome} tem mais visibilidade restrita do que o corte previa"
        );
        assert_eq!(
            codigo.matches("pub(crate)").count(),
            0,
            "src/ir/{nome} criou visibilidade de crate; o corte previa zero"
        );
        for item in autorizados.iter().chain(restritos.iter()) {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/ir/{nome} deveria conter `{item}` exatamente uma vez"
            );
        }
    }
}

/// O pai também não promoveu: a decomposição não pode pagar o layout físico com
/// visibilidade de crate no arquivo que ficou.
///
/// O laço acima pula `ir.rs` — ele mede o custo do irmão — e por isso não
/// observaria um `pub(crate)` novo do lado do pai. Os três que existem são os
/// mesmos de antes do corte, e nenhum deles é da IR-1: `MapKeyIR::type_ir` e
/// `MapValueIR::type_ir` na região `ir.modelo.representacao`, e
/// `is_generic_map_intrinsic` entre elas.
#[test]
fn o_pai_tambem_nao_ganhou_visibilidade_de_crate() {
    let codigo = codigo_executavel(pai());
    assert_eq!(
        codigo.matches("pub(crate)").count(),
        3,
        "src/ir.rs mudou de quantidade de `pub(crate)`; a #621 não promove visibilidade"
    );
    assert_eq!(
        codigo.matches("pub(crate) fn type_ir(").count(),
        2,
        "os dois `type_ir` de chave e valor de mapa deixaram de ser `pub(crate)` do pai"
    );
    assert_eq!(
        codigo
            .matches("pub(crate) fn is_generic_map_intrinsic(")
            .count(),
        1,
        "`is_generic_map_intrinsic` deixou de ser o terceiro `pub(crate)` do pai"
    );
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
            panic!("src/ir/{nome} não declarou que itens de {classe} o corte previa")
        })
}

/// A superfície pública do módulo não mudou de tamanho nem de conteúdo.
#[test]
fn a_superficie_publica_do_modulo_e_exatamente_a_congelada() {
    let modulo = ir();
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
        "a superfície pública de ir mudou; a #621 é decomposição física e não muda API pública"
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
            "src/ir/{nome} é produção e não pode cortar o censo com um `#[cfg(test)]`"
        );
    }
}

/// C2 continua com uma dona só, e o irmão não virou a segunda.
///
/// A #621 move o consumo, nunca a regra: `src/method_dispatch.rs` continua
/// decidindo alcance, precedência, desempate e representante. O irmão constrói
/// candidatos e traduz o veredito — e é só isso que pode haver nele. As duas
/// consultas do lowering continuam sendo uma cada, agora em arquivos diferentes
/// do mesmo módulo. É a sensitivity M7 da #621.
#[test]
fn o_irmao_consome_c2_e_nao_cria_uma_segunda_autoridade() {
    let irmao = codigo_executavel(fonte("lowering.rs"));
    let pai = codigo_executavel(pai());
    let modulo = codigo_executavel(&ir());

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
            "src/ir/lowering.rs voltou a aplicar `{termo}` por conta própria"
        );
    }

    // O consumo desceu inteiro para o irmão: uma consulta lá, nenhuma no pai.
    assert_eq!(
        irmao.matches("select_impl_method(").count(),
        1,
        "src/ir/lowering.rs deveria consultar `select_impl_method` exatamente uma vez"
    );
    assert_eq!(
        pai.matches("select_impl_method(").count(),
        0,
        "src/ir.rs voltou a consultar `select_impl_method` por conta própria"
    );
    // `select_representative` é da região `ir.lowering.metodos-identidade`, que
    // a IR-1 não move: ela continua no pai, e o irmão não pode ganhar uma cópia.
    assert_eq!(
        pai.matches("select_representative(").count(),
        1,
        "src/ir.rs deveria continuar consultando `select_representative` uma vez"
    );
    assert_eq!(
        irmao.matches("select_representative(").count(),
        0,
        "src/ir/lowering.rs passou a escolher representante, duplicando a autoridade"
    );

    // O módulo inteiro continua com exatamente uma consulta por decisão: o
    // corte não pôde nem duplicar nem apagar nenhuma delas.
    for decisao in ["select_impl_method", "select_representative"] {
        assert_eq!(
            modulo.matches(&format!("{decisao}(")).count(),
            1,
            "o módulo ir deveria consultar `{decisao}` exatamente uma vez"
        );
    }
}

/// C5 continua estruturada e C1 continua com dona única, também no irmão.
///
/// O irmão é o maior pedaço de lowering que existe: é nele que reconstruir a
/// origem de um corpo default pela grafia do nome sintético, ou repetir o censo
/// de assinaturas de intrínseca, custaria menos linhas do que consultar a
/// autoridade. São as sensitivities M8 e M9 da #621.
#[test]
fn o_irmao_nao_reconstroi_c5_nem_duplica_c1() {
    let irmao = codigo_executavel(fonte("lowering.rs"));
    for termo in [
        "__impl_",
        "__trait_default_check_",
        "trait_default_body",
        "TraitDefaultBody",
    ] {
        assert_eq!(
            irmao.matches(termo).count(),
            0,
            "src/ir/lowering.rs voltou a decidir origem de corpo default por `{termo}`"
        );
    }
    for termo in ["intrinsics::registry", "builtin_sig", "builtin_nominal_sig"] {
        assert_eq!(
            irmao.matches(termo).count(),
            0,
            "src/ir/lowering.rs passou a manter censo próprio de intrínseca por `{termo}`"
        );
    }
}

/// A validação da IR e a fronteira de CFG não desceram com o corte.
///
/// `src/ir_validate.rs` e `src/cfg_ir.rs` continuam donos do que sempre foram;
/// o irmão constrói `InstructionIR` estruturada e para aí. É a sensitivity M11
/// da #621.
#[test]
fn o_irmao_nao_absorveu_validacao_de_ir_nem_fronteira_de_cfg() {
    let irmao = codigo_executavel(fonte("lowering.rs"));
    for termo in [
        "ir_validate",
        "cfg_ir",
        "validate_program",
        "BasicBlock",
        "CfgProgram",
    ] {
        assert_eq!(
            irmao.matches(termo).count(),
            0,
            "src/ir/lowering.rs passou a executar `{termo}`, que é de outra autoridade"
        );
    }
}

/// A ordem de fase não mudou: o pai continua sendo quem constrói o lowerer e
/// quem despacha constantes, e o irmão continua sendo só o corpo. É a fronteira
/// que torna `pub(super)` suficiente — e a sensitivity M10 da #621 perturba
/// exatamente o que este teste ancora.
#[test]
fn o_pai_continua_orquestrando_o_lowering() {
    let pai = codigo_executavel(pai());
    assert_eq!(
        pai.matches("FunctionLowerer::new(&context).lower_function(")
            .count(),
        2,
        "src/ir.rs deveria construir o lowerer e abaixar a função nas duas entradas"
    );
    assert_eq!(
        pai.matches("lower_const(const_decl, &context)").count(),
        1,
        "src/ir.rs deveria despachar constantes exatamente uma vez"
    );
    // As duas chamadas moram na orquestração do programa, depois da montagem do
    // contexto: um deslocamento de fase mudaria este vizinho.
    let contexto = pai
        .find("LoweringContext::from_program_composto(")
        .expect("a montagem do contexto continua no pai");
    let despacho = pai
        .find("lower_const(const_decl, &context)")
        .expect("o despacho de constantes continua no pai");
    assert!(
        contexto < despacho,
        "o despacho de constantes passou a acontecer antes da montagem do contexto"
    );
}
