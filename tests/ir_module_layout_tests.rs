//! Guardião estrutural da decomposição física do lowering AST → IR
//! (#621 e #624, unidades IR-1 e IR-2 do inventário da #601).
//!
//! `src/ir.rs` é a autoridade do lowering. A #621 desceu o `impl FunctionLowerer`
//! inteiro — as cinco regiões `ir.lowering.funcoes-blocos`,
//! `ir.lowering.comandos-controle`, `ir.lowering.expressoes-valores`,
//! `ir.lowering.bindings-escopos` e `ir.lowering.constantes` — para
//! `src/ir/lowering.rs`. A #624 desceu a montagem do contexto global e a
//! orquestração do programa — as cinco regiões
//! `ir.lowering.programa-orquestracao`, `ir.lowering.contexto-declaracoes`,
//! `ir.lowering.assinaturas-intrinsecos`, `ir.lowering.metodos-identidade` e
//! `ir.lowering.identidade-resolvida` — para `src/ir/context.rs`. Nenhuma das
//! duas divide a autoridade: o modelo da IR, as `struct
//! FunctionLowerer`/`LoweringContext` e todo o estado, a resolução de tipo e de
//! união (`resolve_type`, `resolve_union_ast_type`, `intern_union`), a
//! renderização textual e a conversão AST→`TypeIR` continuam no pai, e o pai
//! continua sendo um arquivo — não virou `mod.rs`.
//!
//! Este arquivo prova o estado CUMULATIVO das duas unidades, não só o do último
//! corte. As formas de cegueira silenciosa que ele fecha:
//!
//! 1. um oráculo textual que continuasse lendo só `src/ir.rs` seguiria verde e
//!    pararia de observar os irmãos — a OG-1 da #601. As duas consultas do
//!    lowering a `method_dispatch` desceram, uma para cada irmão, então a
//!    cegueira cairia justamente sobre C2; o consumo do registry declarativo de
//!    intrínsecas desceu junto, e com ele C1. Os censos de C2, de C5, da D6 e da
//!    Parte G leem `fonte_de_modulo::ir()`, e o teste abaixo prova que a lista
//!    lida por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ficar para trás no pai, ou — a
//!    forma nova que a segunda unidade cria — o corte podia arrastar código que
//!    não é dele, inclusive código que a IR-1 já tinha movido;
//! 3. o corte podia promover visibilidade ou mudar a superfície pública do
//!    módulo. A IR-1 expôs `new`, `lower_function` e `lower_const` como
//!    `pub(super)`; a IR-2 expôs `resolved_identity`, `intern_resolved_ast`,
//!    `repr_identity` e `internal_identity` pela mesma razão — são os símbolos
//!    que o pai ou o outro irmão chamam. `lower_program` e
//!    `lower_program_composto` já eram `pub` antes do move e continuam `pub`; o
//!    pai os reexporta para preservar `pinker_v0::ir::lower_program` e
//!    `pinker_v0::ir::lower_program_composto`, e essa é a única reexportação
//!    devida;
//! 4. o corte podia arrastar a validação da IR ou a fronteira de CFG para um
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

/// As regiões que cada unidade moveu, e o irmão onde passam a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    // IR-1 (#621): o `impl FunctionLowerer` inteiro.
    ("ir.lowering.funcoes-blocos", "lowering.rs"),
    ("ir.lowering.comandos-controle", "lowering.rs"),
    ("ir.lowering.expressoes-valores", "lowering.rs"),
    ("ir.lowering.bindings-escopos", "lowering.rs"),
    ("ir.lowering.constantes", "lowering.rs"),
    // IR-2 (#624): a orquestração do programa e a montagem do contexto.
    ("ir.lowering.programa-orquestracao", "context.rs"),
    ("ir.lowering.contexto-declaracoes", "context.rs"),
    ("ir.lowering.assinaturas-intrinsecos", "context.rs"),
    ("ir.lowering.metodos-identidade", "context.rs"),
    ("ir.lowering.identidade-resolvida", "context.rs"),
];

/// As regiões que os dois cortes deixaram onde estavam. `ir.renderizacao.textual`
/// é a vizinha imediata do span da IR-2 — a que vem depois — e é ela que ficaria
/// vermelha se o corte tivesse escorregado uma região para a frente;
/// `ir.tipos.identidade-resolvida` é a vizinha de trás. As outras duas são as
/// unidades IR-3 e IR-4 do mesmo inventário, que esta Task não executa.
const REGIOES_RETIDAS: &[&str] = &[
    "ir.modelo.representacao",
    "ir.tipos.identidade-resolvida",
    "ir.renderizacao.textual",
    "ir.tipos.conversao-ast",
];

/// As definições que cada unidade moveu inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão certo, e nenhuma deixada para trás no pai.
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
    ("fn from_program_composto(", "context.rs"),
    ("fn intern_resolved_ast(", "context.rs"),
    ("fn internal_identity(", "context.rs"),
    ("fn lower_program(", "context.rs"),
    ("fn lower_program_composto(", "context.rs"),
    ("fn register_impl_methods(", "context.rs"),
    ("fn repr_identity(", "context.rs"),
    ("fn resolved_identity(", "context.rs"),
    ("fn seal_declared_signature_identities(", "context.rs"),
    ("fn seal_enum_variant_metadata(", "context.rs"),
];

/// As definições que os cortes NÃO moveram e que continuam no pai.
///
/// `resolve_type`, `resolve_union_ast_type` e `intern_union` são métodos do
/// mesmo `impl LoweringContext` cuja maior parte desceu com a IR-2, e não estão
/// em nenhuma das cinco regiões da unidade: arrastá-las junto seria mover código
/// que não é do corte. `builtin_sig` e `builtin_nominal_sig` são os helpers de
/// assinatura que a região `ir.lowering.assinaturas-intrinsecos` chama; uma cópia
/// no irmão seria censo local de intrínseca, que é o que C1 proíbe. `line` e
/// `render_program` ancoram a fronteira com a IR-4, que esta Task não executa.
const DEFINICOES_RETIDAS: &[&str] = &[
    "fn builtin_nominal_sig(",
    "fn builtin_sig(",
    "fn intern_union(",
    "fn line(",
    "fn render_program(",
    "fn resolve_type(",
    "fn resolve_union_ast_type(",
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["context.rs", "lowering.rs"];

/// Os itens `pub` que cada irmão pode ter, exaustivo.
///
/// Nenhum item da IR-1 era `pub`. Os dois da IR-2 já eram `pub` no pai antes do
/// move — são a entrada pública do lowering — e continuam `pub` no irmão, com o
/// pai reexportando os dois caminhos.
const PUB_AUTORIZADO: &[(&str, &[&str])] = &[
    (
        "context.rs",
        &["pub fn lower_program(", "pub fn lower_program_composto("],
    ),
    ("lowering.rs", &[]),
];

/// A visibilidade restrita que cada irmão pode ter, exaustiva.
///
/// É o custo Rust inteiro das duas unidades. Para a IR-1 o inventário da #601
/// previu `4 pub(super)` e o baseline desmentiu um: as únicas ocorrências de
/// `resolve_closure` fora do corte eram comentários. Para a IR-2 previu
/// `4 pub(super)` e os quatro se confirmaram, exatamente os quatro `exports` que
/// o `unit_costs.json` nomeia. Nenhum `pub(crate)` novo em nenhuma das duas.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[
    (
        "context.rs",
        &[
            "pub(super) fn resolved_identity(",
            "pub(super) fn intern_resolved_ast(",
            "pub(super) fn repr_identity(",
            "pub(super) fn internal_identity(",
        ],
    ),
    (
        "lowering.rs",
        &[
            "pub(super) fn new(",
            "pub(super) fn lower_function(",
            "pub(super) fn lower_const(",
        ],
    ),
];

/// A reexportação que o pai deve — e a única que pode existir no módulo.
const REEXPORTACAO_DEVIDA: &str = "pub use context::{lower_program, lower_program_composto};";

/// A superfície pública do módulo, congelada item a item.
///
/// É o contrato `PUBLIC_PATHS_BEFORE == AFTER` da #621 e da #624 na forma que um
/// teste consegue observar. A #621 não movia nenhum item público; a #624 move
/// dois, e por isso a lista continua idêntica somente porque a reexportação
/// devolve os dois caminhos ao pai.
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
            "a região {chave} não é de nenhuma das duas unidades e deveria continuar em src/ir.rs"
        );
    }

    // Presença não basta: a lista precisa ser o conjunto EXATO do que ficou. Só
    // com igualdade uma região arrastada em silêncio — ou uma sobra de uma IR-3
    // ou IR-4 executada pela metade — fica vermelha aqui, e não apenas na
    // cartografia. É a mesma disciplina de
    // `o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem`.
    let no_pai: BTreeSet<&str> = regioes_declaradas(pai()).collect();
    let retidas: BTreeSet<&str> = REGIOES_RETIDAS.iter().copied().collect();
    assert_eq!(
        no_pai, retidas,
        "o conjunto de regiões que ficaram em src/ir.rs divergiu do declarado"
    );
    for (nome, _) in IR_ARQUIVOS {
        if *nome == "ir.rs" {
            continue;
        }
        let no_irmao: BTreeSet<&str> = regioes_declaradas(fonte(nome)).collect();
        let esperadas: BTreeSet<&str> = REGIOES_MOVIDAS
            .iter()
            .filter(|(_, arquivo)| arquivo == nome)
            .map(|(chave, _)| *chave)
            .collect();
        assert_eq!(
            no_irmao, esperadas,
            "o conjunto de regiões de src/ir/{nome} divergiu do declarado"
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

    // O outro lado do mesmo contrato: o corte não pode arrastar código que não
    // é dele. Cada definição retida tem uma implementação só, e ela está no pai.
    for definicao in DEFINICOES_RETIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo ir"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            1,
            "`{definicao}` não é de nenhuma das duas unidades e deveria continuar em src/ir.rs"
        );
    }
}

/// As chaves de região declaradas por uma fonte, na ordem em que aparecem.
fn regioes_declaradas(fonte: &'static str) -> impl Iterator<Item = &'static str> {
    fonte.lines().filter_map(|linha| {
        linha
            .trim_start()
            .strip_prefix("// @pinker-nav:start ")
            .map(str::trim)
    })
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
    // A IR-2 desceu dois itens que já eram `pub`. Preservar
    // `pinker_v0::ir::lower_program` e `pinker_v0::ir::lower_program_composto`
    // exige exatamente uma reexportação mecânica, e ela é a única que pode
    // existir: uma segunda abriria caminho novo sem mudar a contagem acima.
    assert_eq!(
        codigo.matches("pub use ").count(),
        1,
        "o módulo mudou de quantidade de reexportações; só a da entrada pública do lowering é devida"
    );
    assert_eq!(
        codigo_executavel(pai())
            .matches(REEXPORTACAO_DEVIDA)
            .count(),
        1,
        "src/ir.rs deveria reexportar a entrada pública do lowering exatamente uma vez"
    );
    for item in ["pub fn lower_program(", "pub fn lower_program_composto("] {
        assert_eq!(
            codigo_executavel(fonte("context.rs")).matches(item).count(),
            1,
            "`{item}` deveria continuar público em src/ir/context.rs"
        );
    }
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

/// C2 continua com uma dona só, e nenhum dos irmãos virou a segunda.
///
/// Os dois cortes movem o consumo, nunca a regra: `src/method_dispatch.rs`
/// continua decidindo alcance, precedência, desempate e representante. A #621
/// desceu `select_impl_method` para `lowering.rs` e a #624 desceu
/// `select_representative` para `context.rs`, cada uma dentro da região que a
/// contém. As duas consultas da fase continuam sendo uma cada, agora em dois
/// arquivos irmãos, e nenhuma sobrou no pai.
#[test]
fn o_modulo_consome_c2_e_nao_cria_uma_segunda_autoridade() {
    let contexto = codigo_executavel(fonte("context.rs"));
    let lowering = codigo_executavel(fonte("lowering.rs"));
    let pai = codigo_executavel(pai());
    let modulo = codigo_executavel(&ir());

    // O vocabulário da precedência e a pergunta de alcance continuam fora da
    // fase — nos irmãos inclusive, que é onde a tentação nasce.
    for termo in [
        "NivelDeDespacho",
        "nivel_de_despacho",
        "PorUnidadeImportada",
    ] {
        for (nome, codigo) in [("context.rs", &contexto), ("lowering.rs", &lowering)] {
            assert_eq!(
                codigo.matches(termo).count(),
                0,
                "src/ir/{nome} voltou a aplicar `{termo}` por conta própria"
            );
        }
        assert_eq!(
            pai.matches(termo).count(),
            0,
            "src/ir.rs voltou a aplicar `{termo}` por conta própria"
        );
    }

    // Uma consulta por decisão, cada uma no irmão que a região levou.
    let esperado = [
        ("select_impl_method(", "lowering.rs", &lowering, &contexto),
        ("select_representative(", "context.rs", &contexto, &lowering),
    ];
    for (decisao, dono, codigo_do_dono, codigo_do_outro) in esperado {
        assert_eq!(
            codigo_do_dono.matches(decisao).count(),
            1,
            "src/ir/{dono} deveria consultar `{decisao}` exatamente uma vez"
        );
        assert_eq!(
            codigo_do_outro.matches(decisao).count(),
            0,
            "o outro irmão ganhou uma cópia de `{decisao}`, duplicando a autoridade"
        );
        assert_eq!(
            pai.matches(decisao).count(),
            0,
            "src/ir.rs voltou a consultar `{decisao}` por conta própria"
        );
        // O módulo inteiro continua com exatamente uma consulta por decisão: os
        // cortes não puderam nem duplicar nem apagar nenhuma delas.
        assert_eq!(
            modulo.matches(decisao).count(),
            1,
            "o módulo ir deveria consultar `{decisao}` exatamente uma vez"
        );
    }
}

/// C5 continua estruturada e C1 continua com dona única, também nos irmãos.
///
/// `lowering.rs` é o maior pedaço de lowering que existe e `context.rs` é onde
/// as assinaturas de intrínseca são declaradas: são os dois lugares em que
/// reconstruir a origem de um corpo default pela grafia do nome sintético, ou
/// repetir o censo de assinaturas de intrínseca, custaria menos linhas do que
/// consultar a autoridade.
#[test]
fn os_irmaos_nao_reconstroem_c5_nem_duplicam_c1() {
    let irmaos = [
        ("context.rs", codigo_executavel(fonte("context.rs"))),
        ("lowering.rs", codigo_executavel(fonte("lowering.rs"))),
    ];
    for (nome, codigo) in &irmaos {
        for termo in [
            "__impl_",
            "__trait_default_check_",
            "trait_default_body",
            "TraitDefaultBody",
        ] {
            assert_eq!(
                codigo.matches(termo).count(),
                0,
                "src/ir/{nome} voltou a decidir origem de corpo default por `{termo}`"
            );
        }
    }

    // C1: a região `ir.lowering.assinaturas-intrinsecos` desceu com a IR-2, e
    // com ela a única leitura do registry declarativo. Ela continua sendo uma
    // leitura só, e continua sendo leitura: os helpers de assinatura
    // (`builtin_sig`, `builtin_nominal_sig`) ficaram no pai, e uma cópia deles
    // no irmão seria censo local — ver DEFINICOES_RETIDAS.
    let contexto = codigo_executavel(fonte("context.rs"));
    let lowering = codigo_executavel(fonte("lowering.rs"));
    assert_eq!(
        contexto.matches("intrinsics::registry").count(),
        1,
        "src/ir/context.rs deveria consultar o registry declarativo exatamente uma vez"
    );
    assert_eq!(
        lowering.matches("intrinsics::registry").count(),
        0,
        "src/ir/lowering.rs passou a manter censo próprio de intrínseca"
    );
    assert_eq!(
        codigo_executavel(pai())
            .matches("intrinsics::registry")
            .count(),
        0,
        "src/ir.rs voltou a consultar o registry por conta própria"
    );
    for termo in ["builtin_sig", "builtin_nominal_sig"] {
        assert_eq!(
            lowering.matches(termo).count(),
            0,
            "src/ir/lowering.rs passou a manter censo próprio de intrínseca por `{termo}`"
        );
    }
}

/// A validação da IR e a fronteira de CFG não desceram com nenhum dos cortes.
///
/// `src/ir_validate.rs` e `src/cfg_ir.rs` continuam donos do que sempre foram;
/// os irmãos constroem `InstructionIR` estruturada e param aí.
#[test]
fn os_irmaos_nao_absorveram_validacao_de_ir_nem_fronteira_de_cfg() {
    for nome in IRMAOS_DE_PRODUCAO {
        let irmao = codigo_executavel(fonte(nome));
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
                "src/ir/{nome} passou a executar `{termo}`, que é de outra autoridade"
            );
        }
    }
}

/// A ordem de fase não mudou: quem monta o contexto, quem constrói o lowerer e
/// quem despacha constantes continuam sendo o mesmo código, na mesma ordem.
///
/// Antes da IR-2 a orquestração morava no pai; agora mora em `context.rs`, e é
/// dali que ela chama os `pub(super)` do outro irmão. Este teste é o que a
/// sensitivity M10 da #624 perturba: mudar a fase de lugar, ou inverter a
/// montagem do contexto e o despacho de constantes, fica vermelho aqui.
#[test]
fn o_modulo_continua_orquestrando_o_lowering_na_mesma_ordem() {
    let contexto = codigo_executavel(fonte("context.rs"));
    let pai = codigo_executavel(pai());
    assert_eq!(
        contexto
            .matches("FunctionLowerer::new(&context).lower_function(")
            .count(),
        2,
        "src/ir/context.rs deveria construir o lowerer e abaixar a função nas duas entradas"
    );
    assert_eq!(
        contexto
            .matches("lower_const(const_decl, &context)")
            .count(),
        1,
        "src/ir/context.rs deveria despachar constantes exatamente uma vez"
    );
    for chamada in [
        "FunctionLowerer::new(&context).lower_function(",
        "lower_const(const_decl, &context)",
        "LoweringContext::from_program_composto(",
    ] {
        assert_eq!(
            pai.matches(chamada).count(),
            0,
            "`{chamada}` ficou para trás em src/ir.rs"
        );
    }
    // As duas chamadas moram na orquestração do programa, depois da montagem do
    // contexto: um deslocamento de fase mudaria este vizinho.
    let montagem = contexto
        .find("LoweringContext::from_program_composto(")
        .expect("a montagem do contexto continua no módulo");
    let despacho = contexto
        .find("lower_const(const_decl, &context)")
        .expect("o despacho de constantes continua no módulo");
    assert!(
        montagem < despacho,
        "o despacho de constantes passou a acontecer antes da montagem do contexto"
    );
}
