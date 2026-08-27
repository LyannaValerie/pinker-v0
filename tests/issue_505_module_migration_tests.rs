//! Gates estruturais da TOTAL_INTRINSIC_MODULE_MIGRATION (Issue #505).
//!
//! Este arquivo é a autoridade de teste da migração. Ele não repete o que
//! `intrinsic_authority` já prova sobre si mesma: prova as invariantes que
//! atravessam autoridades — que a enumeração central enxerga toda a superfície
//! pública, que cada identidade pública tem exatamente um módulo importável, e
//! que nenhuma grafia pública sobrevive como global.
//!
//! ```text
//! CENTRAL_PUBLIC_SPELLINGS == UNION_PUBLIC_SPELLINGS
//! CENTRAL_IDENTITIES       == UNION_IDENTITIES
//! ALL_PUBLIC_INTRINSICS    -> IMPORTABLE_MODULE_SURFACE
//! GLOBAL_PUBLIC_INTRINSIC  = 0
//! ```

mod common;

use common::parse;
use pinker_v0::familia_superficie::{self, FAMILIAS};
use pinker_v0::intrinsic_authority::{
    all_canonical_intrinsic_spellings, all_public_intrinsic_members,
    canonical_public_intrinsic_spelling, public_intrinsic_member, IntrinsicIdentity,
    PublicIntrinsicOrigin,
};
use pinker_v0::saida_processo::ACESSORES as ACESSORES_DE_PROCESSO;
use std::collections::{BTreeMap, BTreeSet};

/// Chave estável de identidade.
///
/// `IntrinsicIdentity` é `Copy + Eq` mas não ordenável, e cada variante carrega
/// ou uma grafia `'static` ou um enum `Copy`: a forma `Debug` é injetiva e
/// determinística, e serve como chave sem obrigar a autoridade a derivar `Ord`
/// só por causa de teste.
fn chave(identity: IntrinsicIdentity) -> String {
    format!("{identity:?}")
}

/// Nome do callee da única chamada em `principal`, já canonicalizado.
fn callee_de_principal(programa: &pinker_v0::ast::Program) -> Option<String> {
    use pinker_v0::ast::{ExprKind, Item, Stmt};
    for item in &programa.items {
        let Item::Function(function) = item else {
            continue;
        };
        if function.name != "principal" {
            continue;
        }
        for stmt in &function.body.stmts {
            let Stmt::Return(retorno) = stmt else {
                continue;
            };
            let Some(expr) = retorno.expr.as_ref() else {
                continue;
            };
            let ExprKind::Call(callee, _) = &expr.kind else {
                continue;
            };
            if let ExprKind::Ident(nome) = &callee.kind {
                return Some(nome.clone());
            }
        }
    }
    None
}

fn grafias_canonicas() -> BTreeSet<String> {
    all_canonical_intrinsic_spellings()
        .into_iter()
        .map(|entry| entry.spelling.to_string())
        .collect()
}

fn identidades_canonicas() -> BTreeSet<String> {
    all_canonical_intrinsic_spellings()
        .into_iter()
        .map(|entry| chave(entry.identity))
        .collect()
}

/// A união histórica: autoridade central mais a autoridade de processo que a
/// #505 encontrou fora dela. Depois da Stage 0 as duas precisam coincidir.
fn grafias_da_uniao() -> BTreeSet<String> {
    let mut uniao = grafias_canonicas();
    for acessor in ACESSORES_DE_PROCESSO {
        uniao.insert(acessor.to_string());
    }
    uniao
}

fn identidades_da_uniao() -> BTreeSet<String> {
    let mut uniao = identidades_canonicas();
    for acessor in ACESSORES_DE_PROCESSO {
        uniao.insert(chave(IntrinsicIdentity::ProcessAccessor(acessor)));
    }
    uniao
}

/// A superfície pública inteira, nominal: `(módulo, membro, grafia canônica)`.
///
/// Tabela dourada. Um gate que só conta membros aceita troca silenciosa de
/// nome, de módulo ou de identidade; este aqui não aceita nenhuma das três.
const SUPERFICIE_ESPERADA: &[(&str, &str, &str)] = &[
    ("acaso", "criar", "aleatorio_criar"),
    ("acaso", "entre", "aleatorio_entre"),
    ("acaso", "proximo", "aleatorio_proximo"),
    ("ambiente", "variavel_ou", "ambiente_ou"),
    ("ambiente", "argumento", "argumento"),
    ("ambiente", "argumento_ou", "argumento_ou"),
    ("ambiente", "buscar_contexto", "buscar_contexto"),
    ("ambiente", "pedir_argumento", "pedir_argumento"),
    ("ambiente", "quantos_argumentos", "quantos_argumentos"),
    ("ambiente", "tem_argumento", "tem_argumento"),
    ("ambiente", "tem_chave", "tem_chave"),
    ("ambiente", "tem_flag", "tem_flag"),
    ("arquivo", "abrir", "abrir"),
    ("arquivo", "abrir_anexo", "abrir_anexo"),
    ("arquivo", "anexar_verso", "anexar_verso"),
    ("arquivo", "copiar", "copiar_arquivo"),
    ("arquivo", "criar", "criar_arquivo"),
    ("arquivo", "escrever_bombom", "escrever"),
    ("arquivo", "escrever_verso", "escrever_verso"),
    ("arquivo", "fechar", "fechar"),
    ("arquivo", "ler_bombom", "ler_arquivo"),
    ("arquivo", "ler_caminho_ou", "arquivo_ou"),
    ("arquivo", "ler_caminho_resultado", "ler_arquivo_resultado"),
    ("arquivo", "ler_caminho_verso", "ler_arquivo_verso"),
    ("arquivo", "ler_verso", "ler_verso_arquivo"),
    ("arquivo", "renomear", "renomear_arquivo"),
    ("arquivo", "truncar", "truncar_arquivo"),
    ("assertiva", "afirmar", "afirmar"),
    ("caminho", "existe", "caminho_existe"),
    ("caminho", "juntar", "juntar_caminho"),
    ("caminho", "e_arquivo", "e_arquivo"),
    ("caminho", "e_diretorio", "e_diretorio"),
    ("caminho", "arquivo_vazio", "e_vazio"),
    ("caminho", "tamanho_arquivo", "tamanho_arquivo"),
    ("caminho", "criar_diretorio", "criar_diretorio"),
    ("caminho", "remover_arquivo", "remover_arquivo"),
    ("caminho", "remover_diretorio", "remover_diretorio"),
    ("caminho", "diretorio_atual", "diretorio_atual"),
    ("caminho", "listar_diretorio", "listar_diretorio"),
    ("caminho", "tipo_de_entrada", "tipo_de_entrada"),
    ("caminho", "tamanho_de_entrada", "tamanho_de_entrada"),
    ("csv", "emitir_linha_bombom", "emitir_linha_csv_bombom"),
    ("csv", "ler_linha_bombom", "ler_linha_csv_bombom"),
    ("entrada", "ouvir", "ouvir"),
    ("entrada", "ouvir_verso", "ouvir_verso"),
    ("entrada", "ouvir_verso_ou", "ouvir_verso_ou"),
    ("integridade", "sha256_arquivo", "sha256_arquivo"),
    ("integridade", "sha256_verso", "sha256_verso"),
    ("json", "tipo", "json_tipo"),
    ("json", "como_verso", "json_verso"),
    ("json", "como_numero", "json_numero"),
    ("json", "como_logica", "json_logica"),
    ("json", "lista_obter", "json_lista_obter"),
    ("json", "lista_tamanho", "json_lista_tamanho"),
    ("json", "objeto_obter", "json_objeto_obter"),
    ("json", "objeto_tem", "json_objeto_tem"),
    ("json", "objeto_tamanho", "json_objeto_tamanho"),
    ("json", "objeto_chaves", "json_objeto_chaves"),
    ("json", "emitir", "emitir_json"),
    ("json", "emitir_plano_bombom", "emitir_json_plano_bombom"),
    ("json", "ler_plano_bombom", "ler_json_plano_bombom"),
    ("json", "ler_resultado", "ler_json_resultado"),
    ("lista", "criar", "lista_criar"),
    ("lista", "obter", "lista_obter"),
    ("lista", "definir", "lista_definir"),
    ("lista", "inserir", "lista_inserir"),
    ("lista", "anexar", "lista_anexar"),
    ("lista", "tamanho", "lista_tamanho"),
    ("lista", "tirar_ultimo", "lista_tirar_ultimo"),
    ("lista", "bombom_criar", "lista_bombom_criar"),
    ("lista", "bombom_obter", "lista_bombom_obter"),
    ("lista", "bombom_definir", "lista_bombom_definir"),
    ("lista", "bombom_inserir", "lista_bombom_inserir"),
    ("lista", "bombom_anexar", "lista_bombom_anexar"),
    ("lista", "bombom_tamanho", "lista_bombom_tamanho"),
    ("lista", "bombom_tirar_ultimo", "lista_bombom_tirar_ultimo"),
    ("lista", "verso_criar", "lista_verso_criar"),
    ("lista", "verso_obter", "lista_verso_obter"),
    ("lista", "verso_definir", "lista_verso_definir"),
    ("lista", "verso_inserir", "lista_verso_inserir"),
    ("lista", "verso_anexar", "lista_verso_anexar"),
    ("lista", "verso_tamanho", "lista_verso_tamanho"),
    ("lista", "verso_tirar_ultimo", "lista_verso_tirar_ultimo"),
    ("mapa", "definir", "mapa_definir"),
    ("mapa", "obter", "mapa_obter"),
    ("mapa", "remover", "mapa_remover"),
    ("mapa", "tamanho", "mapa_tamanho"),
    ("mapa", "tem", "mapa_tem"),
    ("mapa", "bombom_bombom_criar", "mapa_bombom_bombom_criar"),
    (
        "mapa",
        "bombom_bombom_definir",
        "mapa_bombom_bombom_definir",
    ),
    ("mapa", "bombom_bombom_obter", "mapa_bombom_bombom_obter"),
    (
        "mapa",
        "bombom_bombom_remover",
        "mapa_bombom_bombom_remover",
    ),
    (
        "mapa",
        "bombom_bombom_tamanho",
        "mapa_bombom_bombom_tamanho",
    ),
    ("mapa", "bombom_bombom_tem", "mapa_bombom_bombom_tem"),
    ("mapa", "bombom_verso_criar", "mapa_bombom_verso_criar"),
    ("mapa", "bombom_verso_definir", "mapa_bombom_verso_definir"),
    ("mapa", "bombom_verso_obter", "mapa_bombom_verso_obter"),
    ("mapa", "bombom_verso_remover", "mapa_bombom_verso_remover"),
    ("mapa", "bombom_verso_tamanho", "mapa_bombom_verso_tamanho"),
    ("mapa", "bombom_verso_tem", "mapa_bombom_verso_tem"),
    ("mapa", "verso_bombom_criar", "mapa_verso_bombom_criar"),
    ("mapa", "verso_bombom_definir", "mapa_verso_bombom_definir"),
    ("mapa", "verso_bombom_obter", "mapa_verso_bombom_obter"),
    ("mapa", "verso_bombom_remover", "mapa_verso_bombom_remover"),
    ("mapa", "verso_bombom_tamanho", "mapa_verso_bombom_tamanho"),
    ("mapa", "verso_bombom_tem", "mapa_verso_bombom_tem"),
    ("mapa", "verso_verso_criar", "mapa_verso_verso_criar"),
    ("mapa", "verso_verso_definir", "mapa_verso_verso_definir"),
    ("mapa", "verso_verso_obter", "mapa_verso_verso_obter"),
    ("mapa", "verso_verso_remover", "mapa_verso_verso_remover"),
    ("mapa", "verso_verso_tamanho", "mapa_verso_verso_tamanho"),
    ("mapa", "verso_verso_tem", "mapa_verso_verso_tem"),
    ("memoria", "alocar", "alocar"),
    ("memoria", "liberar", "liberar"),
    ("processo", "executar", "executar_processo"),
    (
        "processo",
        "executar_resultado",
        "executar_processo_resultado",
    ),
    (
        "processo",
        "executar_estruturado",
        "executar_processo_estruturado",
    ),
    ("processo", "executar_com_entrada", "executar_com_entrada"),
    ("processo", "capturar_stdout", "capturar_stdout"),
    ("processo", "capturar_stderr", "capturar_stderr"),
    ("processo", "pipeline_minimo", "pipeline_minimo"),
    ("processo", "codigo", "processo_codigo"),
    ("processo", "saida", "processo_saida"),
    ("processo", "erro", "processo_erro"),
    ("processo", "sair", "sair"),
    ("tempo", "unix", "tempo_unix"),
    ("tempo", "formatar_unix", "formatar_tempo_unix"),
    ("tempo", "dormir", "dormir"),
    ("texto", "aparar", "aparar_verso"),
    ("texto", "buscar", "buscar_verso"),
    ("texto", "comeca_com", "comeca_com"),
    ("texto", "termina_com", "termina_com"),
    ("texto", "contem", "contem_verso"),
    ("texto", "dividir_contar", "dividir_verso_contar"),
    ("texto", "dividir_em", "dividir_verso_em"),
    ("texto", "fatiar", "fatiar_verso"),
    ("texto", "formatar", "formatar_verso"),
    ("texto", "igual", "igual_verso"),
    ("texto", "indice", "indice_verso"),
    ("texto", "indice_em", "indice_verso_em"),
    ("texto", "juntar", "juntar_verso"),
    ("texto", "juntar_com", "juntar_verso_com"),
    ("texto", "maiusculo", "maiusculo_verso"),
    ("texto", "minusculo", "minusculo_verso"),
    ("texto", "nao_vazio", "nao_vazio_verso"),
    ("texto", "substituir", "substituir_verso"),
    ("texto", "tamanho", "tamanho_verso"),
    ("texto", "vazio", "vazio_verso"),
    ("texto", "bombom_para_verso", "bombom_para_verso"),
    ("texto", "verso_para_bombom", "verso_para_bombom"),
    (
        "texto",
        "verso_para_bombom_resultado",
        "verso_para_bombom_resultado",
    ),
];

// ----------------------------------------------------------------------------
// STAGE 0 — reconciliação da autoridade central
// ----------------------------------------------------------------------------

/// `CENTRAL_PUBLIC_SPELLINGS == UNION_PUBLIC_SPELLINGS`.
///
/// O gate existe para impedir que uma segunda lista pública paralela volte a
/// crescer ao lado da enumeração central, como `saida_processo::ACESSORES`
/// crescera até a #505.
#[test]
fn autoridade_central_enxerga_toda_a_grafia_publica() {
    let central = grafias_canonicas();
    let uniao = grafias_da_uniao();
    let fora: Vec<_> = uniao.difference(&central).cloned().collect();
    assert!(
        fora.is_empty(),
        "grafia pública fora da autoridade central: {fora:?}"
    );
    assert_eq!(central, uniao);
}

/// `CENTRAL_IDENTITIES == UNION_IDENTITIES`.
#[test]
fn autoridade_central_enxerga_toda_a_identidade_publica() {
    let central = identidades_canonicas();
    let uniao = identidades_da_uniao();
    let fora: Vec<_> = uniao.difference(&central).cloned().collect();
    assert!(
        fora.is_empty(),
        "identidade pública fora da autoridade central: {fora:?}"
    );
    assert_eq!(central, uniao);
}

/// Prova nominal das três grafias que a #505 encontrou em autoridade paralela.
///
/// Contagem não basta: um gate que só compara tamanhos aceita a troca de uma
/// grafia por outra. Aqui cada acessor é nomeado.
#[test]
fn os_tres_acessores_de_processo_tem_identidade_na_autoridade_central() {
    for acessor in ["processo_codigo", "processo_saida", "processo_erro"] {
        let entrada = canonical_public_intrinsic_spelling(acessor)
            .unwrap_or_else(|| panic!("acessor '{acessor}' ausente da autoridade central"));
        assert_eq!(entrada.spelling, acessor);
        assert_eq!(
            entrada.identity,
            IntrinsicIdentity::ProcessAccessor(acessor)
        );
        assert_eq!(entrada.origin, PublicIntrinsicOrigin::ProcessAccessor);
        assert_eq!(entrada.identity.canonical_public_spelling(), acessor);
    }
}

/// A autoridade de processo continua sendo a lista única dos três — a
/// integração não a duplicou nem a reescreveu.
#[test]
fn a_autoridade_de_processo_continua_com_os_tres_e_so_eles() {
    assert_eq!(
        ACESSORES_DE_PROCESSO,
        ["processo_codigo", "processo_saida", "processo_erro"]
    );
    for acessor in ACESSORES_DE_PROCESSO {
        assert!(pinker_v0::saida_processo::e_acessor(acessor));
    }
}

// ----------------------------------------------------------------------------
// A superfície modular
// ----------------------------------------------------------------------------

/// A superfície pública é exatamente a tabela dourada.
#[test]
fn a_superficie_publica_e_nominalmente_a_esperada() {
    let observada: Vec<(String, String, String)> = all_public_intrinsic_members()
        .into_iter()
        .map(|membro| {
            (
                membro.module.to_string(),
                membro.member.to_string(),
                membro.identity.canonical_public_spelling().to_string(),
            )
        })
        .collect();
    let esperada: Vec<(String, String, String)> = SUPERFICIE_ESPERADA
        .iter()
        .map(|(modulo, membro, canonica)| {
            (modulo.to_string(), membro.to_string(), canonica.to_string())
        })
        .collect();
    assert_eq!(observada, esperada);
}

/// Os quinze módulos aceitos pela revisão taxonômica, e nenhum outro.
#[test]
fn os_modulos_sao_exatamente_os_quinze_aceitos() {
    assert_eq!(
        FAMILIAS,
        [
            "acaso",
            "ambiente",
            "arquivo",
            "assertiva",
            "caminho",
            "csv",
            "entrada",
            "integridade",
            "json",
            "lista",
            "mapa",
            "memoria",
            "processo",
            "tempo",
            "texto",
        ]
    );
    let modulos_da_superficie: BTreeSet<&str> = all_public_intrinsic_members()
        .into_iter()
        .map(|membro| membro.module)
        .collect();
    let declarados: BTreeSet<&str> = FAMILIAS.iter().copied().collect();
    assert_eq!(
        modulos_da_superficie, declarados,
        "módulo declarado sem membro, ou membro em módulo não declarado"
    );
}

/// `EVERY_PUBLIC_INTRINSIC_HAS_IMPORTABLE_MODULE = TRUE`.
#[test]
fn toda_identidade_publica_tem_modulo_importavel() {
    let com_modulo: BTreeSet<String> = all_public_intrinsic_members()
        .into_iter()
        .map(|membro| chave(membro.identity))
        .collect();
    let sem_modulo: Vec<_> = identidades_canonicas()
        .difference(&com_modulo)
        .cloned()
        .collect();
    assert!(
        sem_modulo.is_empty(),
        "identidade pública sem módulo importável: {sem_modulo:?}"
    );
}

/// `DUPLICATED_MODULE_MEMBERSHIP = 0`: uma identidade, um módulo, um membro.
#[test]
fn nenhuma_identidade_pertence_a_dois_modulos() {
    let mut por_identidade: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for membro in all_public_intrinsic_members() {
        por_identidade
            .entry(chave(membro.identity))
            .or_default()
            .push((membro.module.to_string(), membro.member.to_string()));
    }
    let duplicadas: Vec<_> = por_identidade
        .iter()
        .filter(|(_, lugares)| lugares.len() > 1)
        .collect();
    assert!(
        duplicadas.is_empty(),
        "identidade com mais de uma autoridade modular: {duplicadas:?}"
    );
}

/// Dentro de um módulo, a grafia do membro é única.
#[test]
fn nenhum_modulo_exporta_membro_duplicado() {
    let mut vistos: BTreeSet<(&str, &str)> = BTreeSet::new();
    for membro in all_public_intrinsic_members() {
        assert!(
            vistos.insert((membro.module, membro.member)),
            "membro duplicado: {}.{}",
            membro.module,
            membro.member
        );
    }
}

/// `UNCLASSIFIED_PUBLIC_INTRINSICS = 0` e nenhum membro inventa identidade.
#[test]
fn a_superficie_modular_e_bijetiva_com_a_identidade_publica() {
    let membros = all_public_intrinsic_members();
    let identidades_de_membro: BTreeSet<String> = membros
        .iter()
        .map(|membro| chave(membro.identity))
        .collect();
    let canonicas = identidades_canonicas();
    assert_eq!(
        membros.len(),
        identidades_de_membro.len(),
        "membros e identidades precisam estar em bijeção"
    );
    assert_eq!(identidades_de_membro, canonicas);
}

/// Import seletivo resolve exatamente a identidade do PAR pedido.
///
/// Grafia de membro não é única entre módulos — `criar`, `obter`, `tamanho`,
/// `definir` e `juntar` existem em mais de um — e há membro cuja grafia é a
/// grafia canônica de outra identidade, como `json.lista_obter`. Nenhuma das
/// duas coisas é ambiguidade enquanto a resolução for feita pelo par; este
/// gate percorre a superfície inteira e confere, no AST canonicalizado, que
/// ela é.
///
/// O esperado vem da TABELA DOURADA, não do registro. Uma injeção de
/// sensibilidade mostrou que consultar `membro.identity` aqui tornava o teste
/// auto-consistente: repontar `acaso.criar` para `criar_arquivo` mudava as
/// duas pontas da comparação ao mesmo tempo, e o gate seguia verde.
#[test]
fn import_seletivo_resolve_exatamente_a_identidade_do_par() {
    for (modulo, membro, esperado) in SUPERFICIE_ESPERADA {
        let fonte = format!(
            "pacote main;\ntrazer {modulo}.{membro};\ncarinho principal() -> bombom {{ mimo {membro}(); }}\n"
        );
        let programa =
            parse(&fonte).unwrap_or_else(|erro| panic!("{modulo}.{membro} não parseia: {erro:?}"));
        let observado = callee_de_principal(&programa)
            .unwrap_or_else(|| panic!("{modulo}.{membro} não produziu chamada"));
        assert_eq!(
            observado, *esperado,
            "trazer {modulo}.{membro} canonicalizou para a identidade errada"
        );
    }
}

/// A forma qualificada resolve para a mesma identidade que a seletiva.
#[test]
fn forma_qualificada_resolve_a_mesma_identidade_que_a_seletiva() {
    for (modulo, membro, esperado) in SUPERFICIE_ESPERADA {
        let fonte = format!(
            "pacote main;\ntrazer {modulo};\ncarinho principal() -> bombom {{ mimo {modulo}.{membro}(); }}\n"
        );
        let programa = parse(&fonte)
            .unwrap_or_else(|erro| panic!("{modulo}.{membro} qualificada não parseia: {erro:?}"));
        let observado = callee_de_principal(&programa)
            .unwrap_or_else(|| panic!("{modulo}.{membro} qualificada não produziu chamada"));
        assert_eq!(observado, *esperado);
    }
}

/// Nenhum módulo prefixa a si mesmo.
///
/// O modo de falha que a revisão adversarial da #505 existe para pegar é o
/// módulo que troca `foo_bar_baz` global por `foo.foo_bar_baz` — mesma feiura,
/// um andar a mais. Aqui isso é mecânico: a grafia do membro não pode conter o
/// nome do próprio módulo.
///
/// O inverso não é proibido e não deveria ser: `arquivo.ler_caminho_ou` cita
/// `caminho` porque a operação fala de caminho, e `caminho.tamanho_arquivo`
/// cita `arquivo` porque o sujeito é um arquivo. O que a regra recusa é a
/// redundância com o próprio módulo, que o import já diz.
#[test]
fn nenhum_modulo_apenas_prefixa_os_nomes_antigos() {
    let mut ofensores = Vec::new();
    for membro in all_public_intrinsic_members() {
        if membro.member.contains(membro.module) {
            ofensores.push(format!("{}.{}", membro.module, membro.member));
        }
    }
    assert!(
        ofensores.is_empty(),
        "membro que repete o nome do próprio módulo: {ofensores:?}"
    );
}

/// `sair` pertence ao módulo `processo`. Transferência taxonômica da #505.
///
/// `sistema` era um cluster candidato da revisão taxonômica, nunca um módulo
/// em código — no baseline `sair` era global, e não membro de `sistema`.
/// Afirmar a ausência de `sistema` em `FAMILIAS` seria vácuo; o que tem
/// conteúdo é `sair` resolver por `processo` e por mais ninguém.
#[test]
fn sair_pertence_ao_modulo_processo() {
    let membro = public_intrinsic_member("processo", "sair").expect("processo.sair");
    assert_eq!(membro.identity, IntrinsicIdentity::Historical("sair"));
    for modulo in FAMILIAS {
        if *modulo == "processo" {
            continue;
        }
        assert!(
            public_intrinsic_member(modulo, "sair").is_none(),
            "`sair` não pode existir também em `{modulo}`"
        );
    }
}

/// Os três acessores são membros de `processo`, com grafia adulta.
#[test]
fn os_acessores_de_processo_sao_membros_do_modulo_processo() {
    for (membro, canonica) in [
        ("codigo", "processo_codigo"),
        ("saida", "processo_saida"),
        ("erro", "processo_erro"),
    ] {
        let entrada =
            public_intrinsic_member("processo", membro).expect("acessor exportado por processo");
        assert_eq!(entrada.identity.canonical_public_spelling(), canonica);
    }
}

// ----------------------------------------------------------------------------
// Remoção da superfície global
// ----------------------------------------------------------------------------

/// Fonte que chama `grafia` a seco, sem nenhum `trazer`.
///
/// A fonte NÃO é mínima de propósito. Uma fonte sem nenhuma ligação não
/// consegue observar a classe de escape em que uma ligação de OUTRO escopo
/// reabre a superfície global — foi exatamente assim que uma revisão
/// adversarial encontrou o parser cedendo a um censo léxico do arquivo
/// inteiro. Aqui o arquivo liga o mesmo nome como parâmetro e como local numa
/// função vizinha, e a chamada em `principal` continua tendo de ser recusada:
/// nenhuma dessas ligações está visível ali.
fn fonte_bare(grafia: &str) -> String {
    format!(
        "pacote main;\n\
         carinho vizinha({grafia}: verso) -> verso {{\n\
         \x20   nova {grafia}_local: verso = {grafia};\n\
         \x20   mimo {grafia}_local;\n\
         }}\n\
         carinho principal() -> bombom {{ mimo {grafia}(); }}\n"
    )
}

/// A mesma chamada, num arquivo sem ligação nenhuma.
///
/// Mantida ao lado da anterior para separar as duas causas de recusa: se só
/// esta ficasse verde, a recusa dependeria da ausência de ligações no arquivo.
fn fonte_bare_sem_ligacoes(grafia: &str) -> String {
    format!("pacote main;\ncarinho principal() -> bombom {{ mimo {grafia}(); }}\n")
}

/// `GLOBAL_PUBLIC_INTRINSIC_BY_DESIGN = 0` e `..._BY_HISTORY = 0`.
///
/// A prova é comportamental, e não uma lista que se diz vazia: cada grafia
/// pública — canônica ou de membro — é escrita a seco e precisa ser recusada
/// por não estar no escopo. Uma intrínseca global sobrevivente compila aqui.
#[test]
fn nenhuma_grafia_publica_e_chamavel_sem_import() {
    let mut candidatas: BTreeSet<String> = grafias_canonicas();
    for membro in all_public_intrinsic_members() {
        candidatas.insert(membro.member.to_string());
    }
    assert!(candidatas.len() >= 154);
    let mut sobreviventes: Vec<String> = Vec::new();
    for grafia in &candidatas {
        for fonte in [fonte_bare(grafia), fonte_bare_sem_ligacoes(grafia)] {
            match parse(&fonte) {
                Ok(_) => sobreviventes.push(grafia.clone()),
                Err(erro) => {
                    let msg = format!("{erro:?}");
                    assert!(
                        msg.contains("não está no escopo"),
                        "grafia '{grafia}' recusada por outro motivo: {msg}"
                    );
                }
            }
        }
    }
    assert!(
        sobreviventes.is_empty(),
        "intrínseca global sobrevivente: {sobreviventes:?}"
    );
}

/// Ligação de outro escopo não reabre a superfície global.
///
/// Controle positivo do gate acima, com o oráculo do lado do comportamento e
/// não do parser: se a recusa cedesse a uma ligação não visível, este programa
/// compilaria e leria o argv de verdade.
#[test]
fn ligacao_em_outro_escopo_nao_reabre_a_superficie_global() {
    let fonte = "pacote main;\n\
                 carinho rotular(prefixo: verso) -> verso {\n\
                 \x20   nova argumento: verso = prefixo;\n\
                 \x20   mimo argumento;\n\
                 }\n\
                 carinho principal() -> bombom {\n\
                 \x20   falar(rotular(\"x\"));\n\
                 \x20   falar(argumento(0));\n\
                 \x20   mimo 0;\n\
                 }\n";
    let erro = parse(fonte).expect_err("ligação de outra função não pode reabrir o global");
    assert!(
        format!("{erro:?}").contains("não está no escopo"),
        "{erro:?}"
    );
}

/// A ligação PENDENTE do próprio statement continua cedendo.
///
/// `nova f: carinho(...) = carinho(...) { f(...) }` cita o nome no mesmo
/// statement que o liga. Recusar aqui tiraria da semântica um programa que
/// sempre foi dela — e é a única janela em que a recusa cede fora de
/// `escopos_locais`.
#[test]
fn ligacao_pendente_do_proprio_statement_cede() {
    let fonte = "pacote main;\n\
                 carinho principal() -> bombom {\n\
                 \x20   nova tamanho: carinho(verso) -> bombom = carinho(s: verso) -> bombom {\n\
                 \x20       mimo tamanho(s);\n\
                 \x20   };\n\
                 \x20   mimo 0;\n\
                 }\n";
    parse(fonte).expect("o parser não pode recusar a auto-referência pendente");
}

/// A única grafia builtin chamável sem import é `mapa_criar`, e ela é nomeada.
///
/// A revisão adversarial da #505 encontrou que `mapa_criar` — o construtor
/// genérico de mapa — continua chamável a seco, enquanto `lista.criar` virou
/// membro. A assimetria é real e PREEXISTE a esta Issue: `mapa_criar` está em
/// `GRAFIAS_BUILTIN_NAO_PUBLICAS` desde a #507, a revisão taxonômica a
/// classificou como `INTERNAL_ONLY_NOT_PUBLIC_INTRINSIC`, e por isso ela nunca
/// entrou nas 151 identidades públicas.
///
/// O que esta Issue pode fazer sem invadir a taxonomia aceita é impedir que a
/// exceção CRESÇA em silêncio. Toda grafia que `semantic` despacha como
/// builtin é ou pública — e então recusada a seco — ou exatamente
/// `mapa_criar`. Uma segunda exceção quebra aqui.
#[test]
fn a_excecao_builtin_nao_publica_e_exatamente_uma_e_tem_nome() {
    const NAO_CHAMAVEIS: &[&str] = &["si", "trato"];
    let fonte = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/semantic.rs"),
    )
    .expect("ler src/semantic.rs");

    let mut excecoes: BTreeSet<String> = BTreeSet::new();
    for pedaco in fonte.split("name == \"").skip(1) {
        let Some(fim) = pedaco.find('"') else {
            continue;
        };
        let grafia = &pedaco[..fim];
        if grafia.starts_with("__") || NAO_CHAMAVEIS.contains(&grafia) {
            continue;
        }
        if !grafia
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        if canonical_public_intrinsic_spelling(grafia).is_some() {
            continue;
        }
        if parse(&fonte_bare_sem_ligacoes(grafia)).is_ok() {
            excecoes.insert(grafia.to_string());
        }
    }
    assert_eq!(
        excecoes,
        BTreeSet::from(["mapa_criar".to_string()]),
        "a exceção builtin não pública mudou de tamanho ou de nome"
    );
}

/// A recusa cede para quem declarou o nome — a pressão global morreu junto
/// com a superfície global.
#[test]
fn homonimo_local_sem_import_nao_e_recusado() {
    let fonte = "pacote main;\n\
                 carinho tamanho_verso(x: verso) -> bombom { mimo 7; }\n\
                 carinho principal() -> bombom { mimo tamanho_verso(\"oi\"); }\n";
    parse(fonte).expect("declaração homônima sem import é legítima depois da #505");
}

/// As duas formas de import habilitam a chamada, e só elas.
#[test]
fn as_duas_formas_de_import_habilitam_a_chamada() {
    let seletiva = "pacote main;\n\
                    trazer texto.tamanho;\n\
                    carinho principal() -> bombom { mimo tamanho(\"oi\"); }\n";
    parse(seletiva).expect("forma seletiva");
    let qualificada = "pacote main;\n\
                       trazer texto;\n\
                       carinho principal() -> bombom { mimo texto.tamanho(\"oi\"); }\n";
    parse(qualificada).expect("forma qualificada");
}

/// Import seletivo resolve para a identidade do par pedido, e não para a de
/// um homônimo de outro módulo.
#[test]
fn import_seletivo_resolve_a_identidade_do_modulo_pedido() {
    for (modulo, esperado) in [
        ("acaso", "aleatorio_criar"),
        ("arquivo", "criar_arquivo"),
        ("lista", "lista_criar"),
    ] {
        let resolvida = familia_superficie::resolver(modulo, "criar")
            .unwrap_or_else(|| panic!("{modulo}.criar"));
        assert_eq!(
            resolvida, esperado,
            "{modulo}.criar precisa resolver para a identidade do próprio módulo"
        );
    }
}
