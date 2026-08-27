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

/// O delta que a #525 introduziu, expresso sem depender de um total.
///
/// O censo absoluto desta Issue foi medido quando a superfície pública ainda
/// era uma lista plana de grafias. A #505 separou os dois namespaces — grafia
/// canônica endereça identidade, `(módulo, membro)` endereça superfície
/// pública —, então um total congelado aqui passaria a medir a #505 em vez da
/// #525. O invariante da #525 é relativo e sobrevive à separação: **três**
/// grafias canônicas a mais do que identidades, e exatamente as três
/// aprovadas pela Founder.
const EXPECTED_IDENTITY_DELTA: usize = 3;

/// Grafias que a Stage 0 da #505 acrescentou à enumeração central.
///
/// A #525 não as tocou: elas já eram públicas por `saida_processo::ACESSORES`,
/// e o que mudou foi a autoridade que as enumera.
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
    // O total absoluto do namespace de identidade depois da #505: 130
    // históricas + 9 falíveis + 11 acessores JSON + 1 SHA-256 + 3 acessores de
    // processo, sem interseção. Sem ele, «nenhuma grafia desaparece» não
    // detecta um desaparecimento arbitrário — só a perda de unicidade.
    assert_eq!(spellings.len(), 154);
    let unicas: BTreeSet<_> = spellings.iter().map(|entry| entry.spelling).collect();
    assert_eq!(
        unicas.len(),
        spellings.len(),
        "grafias canônicas precisam continuar unívocas"
    );
    for (alias, adulta) in PARES {
        for spelling in [alias, adulta] {
            assert!(
                unicas.contains(&spelling),
                "{spelling} deixou de ser conhecida pela autoridade"
            );
        }
    }
    for acessor in STAGE0_ACESSORES_INTEGRADOS {
        assert!(
            unicas.contains(&acessor),
            "{acessor} deveria ter entrado na enumeração central pela Stage 0 da #505"
        );
    }
}

#[test]
fn exatamente_tres_identidades_desaparecem() {
    let spellings = all_canonical_intrinsic_spellings();
    let identidades: BTreeSet<String> = spellings
        .iter()
        .map(|entry| format!("{:?}", entry.identity))
        .collect();
    assert_eq!(
        spellings.len() - identidades.len(),
        EXPECTED_IDENTITY_DELTA,
        "IDENTITY_DELTA da #525 deveria continuar sendo exatamente -{EXPECTED_IDENTITY_DELTA}"
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
    // No baseline desta Issue havia quinze grupos N:1: doze vinham de alias de
    // família — `criar`/`criar_arquivo`, `ler_bombom`/`ler_arquivo`, ... — e
    // três eram as unificações da Founder.
    //
    // A #505 não desfez nenhum dos doze: ela os tirou deste namespace. Onde
    // antes duas grafias planas endereçavam uma identidade, agora uma grafia
    // canônica a endereça e um par `(módulo, membro)` a expõe, e cada lado
    // tem uma grafia só. Quem prova essa metade é a tabela dourada da #505.
    //
    // O que precisa continuar aqui é o que a #525 decidiu: as três — e apenas
    // as três — unificações da Founder, em que DUAS grafias canônicas ainda
    // respondem por UMA identidade. Uma quarta unificação, uma grafia adulta
    // trocada ou um alias recolapsado muda esta tabela antes de chegar a
    // qualquer consumidor de fase.
    let esperado: Vec<(&str, Vec<&str>)> = vec![
        (
            "Historical(\"buscar_contexto\")",
            vec!["argumento_nomeado_ou_ambiente_ou", "buscar_contexto"],
        ),
        (
            "Historical(\"pedir_argumento\")",
            vec!["argumento_nomeado_ou", "pedir_argumento"],
        ),
        (
            "Historical(\"tem_chave\")",
            vec!["tem_argumento_nomeado", "tem_chave"],
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

/// As seis grafias continuam reservadas para declaração.
///
/// A #505 removeu a superfície GLOBAL, não a reserva: as seis continuam sendo
/// a chave pela qual `semantic`, `ir`, `interpreter` e `backend_s` despacham a
/// intrínseca depois da canonicalização. Aceitar a declaração sem reservar a
/// grafia trocaria esta recusa explícita por sombreamento silencioso.
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

/// O membro de módulo, esse sim, deixou de ocupar o namespace de quem não o traz.
///
/// `PUBLIC_INTRINSIC_GLOBAL_BY_HISTORY = 0` tem esta consequência direta: a
/// proibição que a PR #507 exercia sobre o namespace inteiro não pode
/// sobreviver por acidente à superfície global que a justificava. O membro
/// `ambiente.tem_chave` é homônimo da grafia adulta, então a fronteira é
/// visível num caso só: sem import, quem declara `tem_chave`... continua
/// recusado, porque a grafia é canônica. A liberdade real aparece em membro
/// cuja grafia NÃO é canônica, como `texto.tamanho`.
#[test]
fn membro_nao_trazido_nao_ocupa_o_namespace_local() {
    let fonte = "pacote main;\n\
                 carinho tamanho(x: verso) -> bombom { mimo 42; }\n\
                 carinho principal() -> bombom { mimo tamanho(\"oi\"); }\n";
    let ast = common::parse(fonte).expect("declaração homônima de membro sem import");
    semantic::check_program(&ast).expect("semantic aceita o homônimo");
    assert_eq!(
        executar(fonte, &[]),
        42,
        "a função do usuário precisa vencer"
    );

    let com_import = "pacote main;\n\
                      trazer texto.tamanho;\n\
                      carinho tamanho(x: verso) -> bombom { mimo 42; }\n\
                      carinho principal() -> bombom { mimo 0; }\n";
    let ast = common::parse(com_import).expect("parse");
    semantic::check_program(&ast).expect_err("com o import, a colisão é real");
}

/// As três grafias legadas deixaram de ser chamáveis, sem deixar de endereçar
/// a identidade adulta.
///
/// Esta é a disposição de compatibilidade da #505 para os três aliases:
/// `REMOVE_NOW` na superfície pública, identidade preservada. Elas não voltam
/// como identidade própria — que é o que a #525 e a #505 proíbem — e também
/// não voltam como segundo membro do módulo, o que recriaria exatamente a
/// multiplicidade de grafias públicas que esta campanha removeu.
#[test]
fn o_alias_legado_nao_e_chamavel_mas_continua_endereçando_a_identidade_adulta() {
    for (alias, adulta) in PARES {
        assert_eq!(identidade(alias), identidade(adulta));
        assert!(
            pinker_v0::familia_superficie::modulos_que_exportam(alias).is_empty(),
            "{alias} não pode voltar como membro de módulo"
        );
        let fonte =
            format!("pacote main;\ncarinho principal() -> bombom {{ mimo {alias}(\"--x\"); }}\n");
        let erro = common::parse(&fonte).expect_err("alias legado não é chamável");
        assert!(
            format!("{erro:?}").contains("não está no escopo"),
            "{alias}: {erro:?}"
        );
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
        "pacote main; trazer ambiente.{spelling};\n\
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
        "pacote main; trazer ambiente.{spelling}; trazer texto.igual;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova muda r: bombom = 3;\n\
         \x20   talvez igual({spelling}(\"--chave\", \"PADRAO\"), \"achado\") {{ r = 7; }}\n\
         \x20   mimo r;\n\
         }}\n"
    )
}

/// `buscar_contexto`/`argumento_nomeado_ou_ambiente_ou`: idem, com chave de ambiente.
fn fonte_buscar(spelling: &str) -> String {
    format!(
        "pacote main; trazer ambiente.{spelling}; trazer texto.igual;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova muda r: bombom = 3;\n\
         \x20   talvez igual({spelling}(\"--chave\", \"PINKER_525_ENV\", \"PADRAO\"), \"achado\") {{ r = 7; }}\n\
         \x20   mimo r;\n\
         }}\n"
    )
}

/// A grafia adulta observa o argv pelo membro do módulo que a expõe.
///
/// O invariante que a #525 estabeleceu era «alias e grafia adulta observam o
/// MESMO argv», e ele nascia de haver duas grafias chamáveis. A #505 deixou
/// uma só: a superfície pública de cada identidade é um par
/// `(módulo, membro)`. O que continua verificável — e é o que de fato
/// importava — é que a identidade unificada chega ao acessor certo, com o
/// mesmo oráculo de status.
///
/// A metade que falava do alias não foi apagada: ela virou
/// `o_alias_legado_nao_e_chamavel_mas_continua_endereçando_a_identidade_adulta`,
/// que prova a relação N:1 na autoridade em vez de na chamada.
#[test]
fn a_identidade_unificada_observa_o_argv_pelo_membro_do_modulo() {
    // `le_ambiente` marca o caso cuja resposta na ausência da chave de CLI
    // depende do ambiente do host — ver a nota sobre o oráculo, abaixo.
    let casos: [(FonteDeCaso, &str, bool); 3] = [
        (fonte_tem, "tem_chave", false),
        (fonte_pedir, "pedir_argumento", false),
        (fonte_buscar, "buscar_contexto", true),
    ];

    // Oráculo positivo: `--chave` com valor precisa ser observada; chave
    // ausente cai no ramo negativo. Um membro que parasse de chegar ao acessor
    // certo mudaria o status, e não apenas uma igualdade entre duas grafias.
    //
    // `cli_decide` marca as linhas em que o valor vem da CLI e portanto vence
    // qualquer ambiente. Nas outras duas, `buscar_contexto` consulta
    // `PINKER_525_ENV` no ambiente real do host: exportá-la mudaria a resposta,
    // e por isso ali só se afirma o que vale sob qualquer ambiente. A
    // precedência CLI > ambiente > padrão já é provada com ambiente controlado
    // em `issue492_argumento_nomeado_paridade_tests`.
    let matriz: [(&[&str], i32, bool); 4] = [
        (&["--chave", "achado"], 7, true),
        (&["--chave=achado"], 7, true),
        (&["--outra", "achado"], 3, false),
        (&[], 3, false),
    ];

    for (fonte, adulta, le_ambiente) in casos {
        for (argv, esperado, cli_decide) in matriz {
            let observado = executar(&fonte(adulta), argv);
            if cli_decide || !le_ambiente {
                assert_eq!(
                    observado, esperado,
                    "{adulta} respondeu {observado} para argv {argv:?}"
                );
            }
        }
    }
}
