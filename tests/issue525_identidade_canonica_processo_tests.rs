//! Evidência da #525: seis grafias públicas, três identidades canônicas.
//!
//! A Founder aprovou em #525 as três unificações levantadas pela revisão
//! taxonômica de #505. O que esta suíte protege não é a existência das grafias
//! — elas já eram públicas no baseline — e sim a propriedade estrutural de que
//! cada par responde por **uma** identidade intrínseca, com a grafia adulta
//! como grafia canônica.
//!
//! ```text
//! LEGACY_PUBLIC_SPELLING != DISTINCT_CANONICAL_IDENTITY
//! MULTIPLE_PUBLIC_SPELLINGS -> ONE_CANONICAL_INTRINSIC_IDENTITY
//! ```
//!
//! A gramática de argv continua sendo dita por `runtime/pinker_argv_contract`,
//! e a paridade observável interpretador/nativo das seis grafias continua
//! sendo provada por `issue492_argumento_nomeado_paridade_tests`. Aqui se prova
//! a autoridade central e o comportamento hospedado que a consome.

mod common;

use pinker_v0::intrinsic_authority::{
    all_canonical_intrinsic_spellings, canonical_alias_target, canonical_public_intrinsic_spelling,
    declaration_conflict_policy, intrinsic_from_public_spelling, DeclarationConflictPolicy,
    IntrinsicIdentity, PublicIntrinsicOrigin, HISTORICAL_CANONICAL_ALIASES,
};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, interpreter, ir, ir_validate, semantic,
};
use std::collections::{BTreeMap, BTreeSet};

/// Os três pares aprovados pela Founder, na forma `(alias, grafia adulta)`.
/// Construtor de fonte Pinker parametrizada por grafia.
type FonteDeCaso = fn(&str) -> String;

const PARES: [(&str, &str); 3] = [
    ("tem_argumento_nomeado", "tem_chave"),
    ("argumento_nomeado_ou", "pedir_argumento"),
    ("argumento_nomeado_ou_ambiente_ou", "buscar_contexto"),
];

/// Censo do baseline efetivo `1f2d6c55`, recalculado pela própria autoridade.
///
/// Contado sobre `all_canonical_intrinsic_spellings()`. No baseline desta Issue o
/// domínio-união da #505 somava três acessores de `saida_processo` que a
/// enumeração central não enxergava.
const PUBLIC_SPELLINGS_BEFORE: usize = 163;
const CANONICAL_IDENTITIES_BEFORE: usize = 151;
const EXPECTED_IDENTITY_DELTA: usize = 3;

/// Grafias que a Stage 0 da #505 **acrescentou** à enumeração central.
///
/// A #525 não as tocou e continua não as tocando: elas já eram públicas por
/// `saida_processo::ACESSORES`, e o que mudou foi a autoridade que as enumera.
/// A constante existe para que este censo continue exato — somar sem nomear
/// aceitaria qualquer três grafias novas.
const STAGE0_ACESSORES_INTEGRADOS: [&str; 3] =
    ["processo_codigo", "processo_saida", "processo_erro"];

fn identidade(spelling: &str) -> IntrinsicIdentity {
    intrinsic_from_public_spelling(spelling)
        .unwrap_or_else(|| panic!("grafia pública desconhecida pela autoridade: {spelling}"))
}

/// Agrupa grafias públicas por identidade canônica, só onde N grafias > 1.
fn grafias_por_identidade_compartilhada() -> Vec<(String, Vec<&'static str>)> {
    let mut agrupadas: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for entry in all_canonical_intrinsic_spellings() {
        agrupadas
            .entry(format!("{:?}", entry.identity))
            .or_default()
            .push(entry.spelling);
    }
    agrupadas
        .into_iter()
        .filter(|(_, spellings)| spellings.len() > 1)
        .collect()
}

// ---------------------------------------------------------------------------
// Os três pares
// ---------------------------------------------------------------------------

#[test]
fn cada_par_resolve_para_uma_identidade_so() {
    for (alias, adulta) in PARES {
        assert_eq!(
            identidade(alias),
            identidade(adulta),
            "{alias} e {adulta} deveriam compartilhar identidade canônica"
        );
    }
}

#[test]
fn identidade_de_cada_par_tem_a_grafia_adulta_como_canonica() {
    for (alias, adulta) in PARES {
        assert_eq!(
            identidade(alias).canonical_public_spelling(),
            adulta,
            "grafia canônica de {alias} deveria ser {adulta}"
        );
        assert_eq!(
            identidade(adulta).canonical_public_spelling(),
            adulta,
            "grafia adulta {adulta} deveria ser sua própria grafia canônica"
        );
    }
}

#[test]
fn alias_declara_a_grafia_adulta_na_autoridade_central() {
    for (alias, adulta) in PARES {
        assert_eq!(canonical_alias_target(alias), Some(adulta));
        assert_eq!(
            canonical_alias_target(adulta),
            None,
            "{adulta} não é alias de ninguém"
        );
    }
}

#[test]
fn as_seis_grafias_continuam_publicas_e_com_a_propria_grafia_preservada() {
    for (alias, adulta) in PARES {
        for spelling in [alias, adulta] {
            let entry = canonical_public_intrinsic_spelling(spelling)
                .unwrap_or_else(|| panic!("{spelling} deixou de ser pública"));
            assert_eq!(
                entry.spelling, spelling,
                "a entrada pública precisa preservar a grafia consultada"
            );
            assert_eq!(entry.origin, PublicIntrinsicOrigin::Historical);
        }
    }
}

// ---------------------------------------------------------------------------
// Censo: exatamente três identidades desaparecem, nada mais se move
// ---------------------------------------------------------------------------

#[test]
fn nenhuma_grafia_publica_desaparece() {
    let spellings = all_canonical_intrinsic_spellings();
    let esperado = PUBLIC_SPELLINGS_BEFORE + STAGE0_ACESSORES_INTEGRADOS.len();
    assert_eq!(spellings.len(), esperado);
    let unicas: BTreeSet<_> = spellings.iter().map(|entry| entry.spelling).collect();
    assert_eq!(
        unicas.len(),
        esperado,
        "grafias públicas precisam continuar unívocas"
    );
    // O delta é exatamente a integração da Stage 0, nomeada e não apenas
    // contada: três grafias quaisquer não passam por aqui.
    for acessor in STAGE0_ACESSORES_INTEGRADOS {
        assert!(
            unicas.contains(&acessor),
            "{acessor} deveria ter entrado na enumeração central pela Stage 0 da #505"
        );
    }
}

#[test]
fn exatamente_tres_identidades_desaparecem() {
    let identidades: BTreeSet<String> = all_canonical_intrinsic_spellings()
        .iter()
        .map(|entry| format!("{:?}", entry.identity))
        .collect();
    assert_eq!(
        identidades.len(),
        CANONICAL_IDENTITIES_BEFORE - EXPECTED_IDENTITY_DELTA + STAGE0_ACESSORES_INTEGRADOS.len(),
        "IDENTITY_DELTA da #525 deveria continuar sendo exatamente -{EXPECTED_IDENTITY_DELTA}, \
         somado apenas às identidades que a Stage 0 da #505 integrou"
    );
    for acessor in STAGE0_ACESSORES_INTEGRADOS {
        assert!(
            identidades.contains(&format!("ProcessAccessor({acessor:?})")),
            "{acessor} deveria ter identidade própria na autoridade central"
        );
    }
}

#[test]
fn nenhuma_identidade_nao_relacionada_muda() {
    // Os doze grupos N:1 que já existiam no baseline, mais os três da #525.
    // Qualquer deriva em identidade não relacionada — uma quarta unificação,
    // um alias de família recolapsado, uma grafia adulta trocada — muda esta
    // tabela antes de chegar a qualquer consumidor de fase.
    let esperado: Vec<(&str, Vec<&str>)> = vec![
        ("Fallible(HashArquivo)", vec!["sha256", "sha256_arquivo"]),
        (
            "Fallible(LerArquivoPorCaminho)",
            vec!["ler_arquivo_resultado", "ler_caminho_resultado"],
        ),
        (
            "Historical(\"arquivo_ou\")",
            vec!["arquivo_ou", "ler_caminho_ou"],
        ),
        (
            "Historical(\"buscar_contexto\")",
            vec!["argumento_nomeado_ou_ambiente_ou", "buscar_contexto"],
        ),
        (
            "Historical(\"copiar_arquivo\")",
            vec!["copiar", "copiar_arquivo"],
        ),
        (
            "Historical(\"criar_arquivo\")",
            vec!["criar", "criar_arquivo"],
        ),
        ("Historical(\"e_vazio\")", vec!["arquivo_vazio", "e_vazio"]),
        (
            "Historical(\"escrever\")",
            vec!["escrever", "escrever_bombom"],
        ),
        (
            "Historical(\"ler_arquivo\")",
            vec!["ler_arquivo", "ler_bombom"],
        ),
        (
            "Historical(\"ler_arquivo_verso\")",
            vec!["ler_arquivo_verso", "ler_caminho_verso"],
        ),
        (
            "Historical(\"ler_verso_arquivo\")",
            vec!["ler_verso", "ler_verso_arquivo"],
        ),
        (
            "Historical(\"pedir_argumento\")",
            vec!["argumento_nomeado_ou", "pedir_argumento"],
        ),
        (
            "Historical(\"renomear_arquivo\")",
            vec!["renomear", "renomear_arquivo"],
        ),
        (
            "Historical(\"tem_chave\")",
            vec!["tem_argumento_nomeado", "tem_chave"],
        ),
        (
            "Historical(\"truncar_arquivo\")",
            vec!["truncar", "truncar_arquivo"],
        ),
    ];

    let observado = grafias_por_identidade_compartilhada();
    let observado: Vec<(&str, Vec<&str>)> = observado
        .iter()
        .map(|(identity, spellings)| (identity.as_str(), spellings.clone()))
        .collect();
    assert_eq!(observado, esperado);
}

#[test]
fn nenhuma_quarta_equivalencia_e_criada() {
    assert_eq!(HISTORICAL_CANONICAL_ALIASES.len(), 3);
    let alvos: BTreeSet<&str> = HISTORICAL_CANONICAL_ALIASES
        .iter()
        .map(|(_, canonical)| *canonical)
        .collect();
    assert_eq!(
        alvos,
        BTreeSet::from(["tem_chave", "pedir_argumento", "buscar_contexto"])
    );
    let aliases: BTreeSet<&str> = HISTORICAL_CANONICAL_ALIASES
        .iter()
        .map(|(alias, _)| *alias)
        .collect();
    assert_eq!(
        aliases,
        BTreeSet::from([
            "tem_argumento_nomeado",
            "argumento_nomeado_ou",
            "argumento_nomeado_ou_ambiente_ou",
        ])
    );
}

// ---------------------------------------------------------------------------
// Política de conflito de declaração
// ---------------------------------------------------------------------------

/// Afirma o contrato de política para as seis grafias.
///
/// `declaration_conflict_policy` hoje ignora o argumento e devolve sempre
/// `DeclarationIsRejected`, então este teste sozinho não distingue uma grafia
/// intrínseca de qualquer outra coisa. O que ele prova é o passo anterior: as
/// seis continuam sendo **encontradas** pela autoridade, que é a condição para
/// a política ser consultada. A prova de que a recusa realmente alcança as seis
/// é `redeclarar_qualquer_uma_das_seis_grafias_continua_recusado`, que atravessa
/// `semantic::check_program` de ponta a ponta.
#[test]
fn politica_de_conflito_vale_para_as_seis_grafias() {
    for (alias, adulta) in PARES {
        for spelling in [alias, adulta] {
            let entry = canonical_public_intrinsic_spelling(spelling).expect("grafia pública");
            assert_eq!(
                declaration_conflict_policy(entry),
                DeclarationConflictPolicy::DeclarationIsRejected
            );
        }
    }
}

#[test]
fn redeclarar_qualquer_uma_das_seis_grafias_continua_recusado() {
    for (alias, adulta) in PARES {
        for spelling in [alias, adulta] {
            let fonte = format!(
                "pacote main;\n\
                 carinho {spelling}() -> bombom {{ mimo 0; }}\n\
                 carinho principal() -> bombom {{ mimo 0; }}\n"
            );
            let ast = common::parse(&fonte).expect("parse");
            let erro = semantic::check_program(&ast)
                .expect_err("redeclaração de intrínseca deve falhar")
                .to_string();
            assert!(
                erro.contains("superfície intrínseca Pinker"),
                "{spelling}: {erro}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Comportamento hospedado: o alias continua chegando ao lugar certo
// ---------------------------------------------------------------------------

/// Executa `principal` no interpretador com o argv dado e devolve o status.
fn executar(source: &str, argv: &[&str]) -> i32 {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let ir = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    let args: Vec<String> = argv.iter().map(|arg| (*arg).to_string()).collect();
    interpreter::run_program_with_args(&machine, &args)
        .expect("interpretar")
        .exit_status
        .expect("status de saída")
}

/// `tem_chave`/`tem_argumento_nomeado`: 7 quando a chave tem valor, 3 quando não.
fn fonte_tem(spelling: &str) -> String {
    format!(
        "pacote main;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova muda r: bombom = 3;\n\
         \x20   talvez {spelling}(\"--chave\") {{ r = 7; }}\n\
         \x20   mimo r;\n\
         }}\n"
    )
}

/// `pedir_argumento`/`argumento_nomeado_ou`: 7 no valor de CLI, 3 no padrão.
fn fonte_pedir(spelling: &str) -> String {
    format!(
        "pacote main;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova muda r: bombom = 3;\n\
         \x20   talvez igual_verso({spelling}(\"--chave\", \"PADRAO\"), \"achado\") {{ r = 7; }}\n\
         \x20   mimo r;\n\
         }}\n"
    )
}

/// `buscar_contexto`/`argumento_nomeado_ou_ambiente_ou`: idem, com chave de ambiente.
fn fonte_buscar(spelling: &str) -> String {
    format!(
        "pacote main;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova muda r: bombom = 3;\n\
         \x20   talvez igual_verso({spelling}(\"--chave\", \"PINKER_525_ENV\", \"PADRAO\"), \"achado\") {{ r = 7; }}\n\
         \x20   mimo r;\n\
         }}\n"
    )
}

#[test]
fn alias_e_grafia_adulta_observam_o_mesmo_argv() {
    // `le_ambiente` marca o caso cuja resposta na ausência da chave de CLI
    // depende do ambiente do host — ver a nota sobre o oráculo, abaixo.
    let casos: [(FonteDeCaso, &str, &str, bool); 3] = [
        (fonte_tem, "tem_argumento_nomeado", "tem_chave", false),
        (
            fonte_pedir,
            "argumento_nomeado_ou",
            "pedir_argumento",
            false,
        ),
        (
            fonte_buscar,
            "argumento_nomeado_ou_ambiente_ou",
            "buscar_contexto",
            true,
        ),
    ];

    // Oráculo positivo: `--chave` com valor precisa ser observada nas duas
    // formas de escrita; chave ausente cai no ramo negativo. Um alias que
    // parasse de chegar ao acessor certo mudaria o status, não só a igualdade.
    //
    // `cli_decide` marca as linhas em que o valor vem da CLI e portanto vence
    // qualquer ambiente. Nas outras duas, `buscar_contexto` consulta
    // `PINKER_525_ENV` no ambiente real do host: exportá-la mudaria a resposta.
    // Para esse caso só afirmamos a igualdade alias/adulta, que vale sob
    // qualquer ambiente. A precedência CLI > ambiente > padrão já é provada com
    // ambiente controlado em `issue492_argumento_nomeado_paridade_tests`.
    let matriz: [(&[&str], i32, bool); 4] = [
        (&["--chave", "achado"], 7, true),
        (&["--chave=achado"], 7, true),
        (&["--outra", "achado"], 3, false),
        (&[], 3, false),
    ];

    for (fonte, alias, adulta, le_ambiente) in casos {
        for (argv, esperado, cli_decide) in matriz {
            let com_alias = executar(&fonte(alias), argv);
            let com_adulta = executar(&fonte(adulta), argv);
            assert_eq!(
                com_alias, com_adulta,
                "{alias} e {adulta} divergiram para argv {argv:?}"
            );
            if cli_decide || !le_ambiente {
                assert_eq!(
                    com_adulta, esperado,
                    "{adulta} respondeu {com_adulta} para argv {argv:?}"
                );
            }
        }
    }
}
