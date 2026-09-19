//! Controles da governança versionada do uso supervisionado da Trama (#705 / T3).
//!
//! POT/LPT: AUTHORITY #705 #690 #671 #674
//! POT/LPT: INVARIANT BOOTSTRAP_ONLY != WHOLE_TASK_COMPLIANCE
//! POT/LPT: INVARIANT rc=0 != RELEVANCE_PROVEN
//! POT/LPT: INVARIANT AGENT_QUERY_TRANSLATION != PRODUCT_MULTILINGUAL_RETRIEVAL
//! POT/LPT: MUST NOT implement #702 #703 #704
//!
//! Metade dos controles lê a regra versionada em `AGENTS.md`: apagar a regra
//! deixa o controle vermelho. A outra metade observa o produto corrente, para
//! que a regra não possa ser satisfeita por texto enquanto a superfície de
//! recuperação muda por baixo.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn agents() -> String {
    fs::read_to_string(root().join("AGENTS.md")).expect("AGENTS.md legível")
}

/// A seção versionada do uso supervisionado, isolada das demais políticas.
fn supervised_section() -> String {
    let text = agents();
    let start = text
        .find("## Uso supervisionado da Trama durante a Task")
        .expect("AGENTS.md precisa da seção de uso supervisionado da Trama");
    let rest = &text[start..];
    let end = rest[1..]
        .find("\n## ")
        .map(|offset| offset + 2)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

fn requires(section: &str, needles: &[&str], control: &str) {
    for needle in needles {
        assert!(
            section.contains(needle),
            "{control}: a regra versionada perdeu '{needle}'"
        );
    }
}

fn pink(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root())
        .output()
        .expect("executar pink")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("pink termina com código")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Saída combinada: o produto escolhe o fluxo, o controle observa a mensagem.
fn saida(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// O1 — os seis pontos de controle existem e P0 sozinho não satisfaz a Task.
#[test]
fn o1_bootstrap_only_nao_satisfaz_a_task_inteira() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "CHECKPOINT P0 PREP",
            "CHECKPOINT P1 BEFORE_BROAD_SEARCH",
            "CHECKPOINT P2 AFTER_SCOPE_DISCOVERY",
            "CHECKPOINT P3 STRUCTURAL_BLOCKER_RESUME",
            "CHECKPOINT P4 FINAL_CANDIDATE_REVIEW",
            "CHECKPOINT P5 FINALIZATION",
            "P0_ONLY != WHOLE_TASK_COMPLIANCE",
        ],
        "O1",
    );
}

/// O2 — busca ampla antes de P1 é caminho não supervisionado, não atalho.
#[test]
fn o2_busca_ampla_antes_de_p1_e_nao_supervisionada() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "most relevant Trama route before any broad repository search",
            "hide a broad search behind a vague",
            "every operation performed outside the supervised route",
        ],
        "O2",
    );
}

/// O3 — transição material de escopo exige requery ou revalidação explícita.
#[test]
fn o3_transicao_de_escopo_exige_requery_ou_revalidacao() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "MATERIAL_SCOPE_TRANSITION WHEN any",
            "initial understanding -> subsystem identified",
            "investigation -> implementation target chosen",
            "new unexpected subsystem enters the diff",
            "structural blocker changes the hypothesis",
            "candidate fix broadens the fileset",
            "final diff differs materially from the initial scope",
            "-> requery Trama",
            "OR explicitly revalidate the prior evidence",
            "claim that a query about subsystem A covers a newly discovered subsystem B",
        ],
        "O3",
    );
    // A proibição de cerimônia não pode virar desculpa para não reconsultar.
    assert!(
        section.contains("repeat an identical query ceremonially when scope, catalog/source state"),
        "O3: a exceção de não-cerimônia precisa continuar condicionada ao escopo"
    );
}

/// O4 — consulta tardia não certifica retroativamente a busca anterior.
#[test]
fn o4_consulta_tardia_nao_certifica_busca_anterior() {
    let section = supervised_section();
    requires(
        &section,
        &["LATE_QUERY != RETROACTIVE_PROOF_OF_EARLIER_COMPLIANCE"],
        "O4",
    );
}

/// O5 — `rc=0` com resultados não prova relevância, e o produto não anuncia
/// suficiência nenhuma. A consulta preservada por #671 volta com sucesso e sete
/// regiões, nenhuma delas referente ao termo consultado.
#[test]
fn o5_rc0_com_resultados_nao_prova_relevancia() {
    let out = pink(&["nav", "buscar", "xyzzy-nao-existe", "--json"]);
    assert_eq!(code(&out), 0, "a busca precisa continuar sendo sucesso");
    let text = stdout(&out);
    assert!(
        !text.contains("\"total_results\":0"),
        "o controle exige um conjunto não vazio"
    );
    // O cabeçalho ecoa a consulta; o corpo é que não pode referi-la.
    let corpo = text
        .split_once("\"results\":[")
        .expect("a resposta precisa trazer o conjunto de resultados")
        .1;
    assert!(
        !corpo.contains("xyzzy"),
        "nenhum resultado deveria referir o termo consultado"
    );
    for inventado in [
        "\"sufficient\"",
        "\"sufficiency\"",
        "\"relevant\"",
        "\"threshold\"",
    ] {
        assert!(
            !text.contains(inventado),
            "nav buscar passou a anunciar suficiência com {inventado}: isso é #704"
        );
    }

    let section = supervised_section();
    requires(
        &section,
        &[
            "rc=0 != relevance proven",
            "result_count > 0 != answer sufficient",
            "MUST NOT record PASS solely because",
            "the command exited 0",
            "the result list was nonempty",
            "the score was positive",
        ],
        "O5",
    );
}

/// O6 — candidato selecionado é inspecionado, ou a insuficiência e o fallback
/// ficam registrados. Não existe terceira saída.
#[test]
fn o6_candidato_inspecionado_ou_fallback_registrado() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "MUST at a material checkpoint",
            "inspect the selected candidate",
            "AND record the supporting evidence",
            "OR record TRAMA_INSUFFICIENT and a bounded fallback",
            "QUERY_EXECUTED != ANSWER_PROVEN",
        ],
        "O6",
    );
}

/// O7 — identidade explícita vazia não é ausência do símbolo: `binary_commit`
/// não tem chave cadastrada e mesmo assim é uma declaração real da fonte.
#[test]
fn o7_localizar_sem_identidade_explicita_nao_e_ausencia() {
    let out = pink(&["nav", "localizar", "binary_commit", "--json"]);
    assert_eq!(code(&out), 0);
    let text = stdout(&out);
    assert!(
        text.contains("\"candidates\":[]"),
        "o controle depende de um símbolo sem identidade explícita"
    );
    assert!(
        text.contains("\"classification\":\"EXTRACTED_CANDIDATE\""),
        "a fonte corrente declara binary_commit"
    );
    assert!(
        text.contains("\"path\":\"src/tooling.rs\""),
        "a declaração real precisa continuar localizável"
    );

    requires(
        &supervised_section(),
        &[
            "NAV_LOCALIZAR_EMPTY != SYMBOL_ABSENT",
            "treat an empty localizar as symbol nonexistence",
        ],
        "O7",
    );
}

/// O8 — fallback continua legítimo e continua carregando propósito, tentativa,
/// limitação e escopo delimitado.
#[test]
fn o8_fallback_preserva_proposito_limitacao_e_escopo() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "FALLBACK when Trama is insufficient",
            "MUST record purpose",
            "MUST record the exact Trama attempt",
            "MUST record the Trama result",
            "MUST record the limitation",
            "MUST record the fallback tool",
            "MUST record the bounded fallback scope",
            "MUST record the evidence recovered",
        ],
        "O8",
    );
}

/// O9 — RED esperado não é blocker operacional, e uma sonda deliberada de
/// no-answer devolve `rc=4` declarado, não falha de harness.
#[test]
fn o9_red_esperado_e_no_answer_nao_sao_blocker() {
    let out = pink(&["doc", "rota", "zzzz-nonexistent-intent-xyzzy"]);
    assert_eq!(
        code(&out),
        4,
        "a sonda de no-answer precisa devolver o código declarado de ausência"
    );
    assert!(
        saida(&out).contains("Nenhuma rota encontrada"),
        "a ausência precisa continuar explicada ao agente"
    );

    requires(
        &supervised_section(),
        &[
            "MUST NOT classify as a structural blocker by default",
            "an expected mutant RED",
            "a negative test failure expected by the control",
            "rc=4 from a deliberate no-answer probe",
        ],
        "O9",
    );
}

/// O10 — a reconciliação final também vale para Task sem PR.
#[test]
fn o10_finalizacao_vale_para_task_sem_pr() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "CHECKPOINT P5 FINALIZATION",
            "applies to a Task without PR as well",
            "rejected candidates, fallbacks, limitations, coverage gaps",
        ],
        "O10",
    );
}

/// O11 — o que a máquina observa e o que o agente atesta continuam separados, e
/// a lacuna de cartografia é declarada em vez de decorada.
#[test]
fn o11_observado_e_atestado_permanecem_separados() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "EVIDENCE CLASS",
            "OBSERVED",
            "AGENT_ATTESTED",
            "OBSERVATION != COMPREHENSION",
            "encode agent interpretation as machine-proven truth",
            "TRAMA_COVERAGE_GAP = TRUE",
            "MUST NOT invent an arbitrary marker merely to get green",
        ],
        "O11",
    );
}

/// O12 — entrada inválida e chave ausente nunca viram PASS.
#[test]
fn o12_entrada_invalida_nunca_vira_pass() {
    let invalida = pink(&["nav", "localizar", "binary_commit", "--limite", "3"]);
    assert_eq!(
        code(&invalida),
        2,
        "uma flag que não pertence ao subcomando é uso inválido"
    );

    let ausente = pink(&["nav", "mostrar", "nao.existe.chave.alguma"]);
    assert_ne!(code(&ausente), 0, "chave ausente não é sucesso");
    assert_eq!(code(&ausente), 4, "chave ausente é ausência declarada");
}

/// O13 — o procedimento de consulta conceitual é inglês canônico, e traduzir a
/// consulta do agente não vira alegação de produto multilíngue.
#[test]
fn o13_consulta_conceitual_em_ingles_nao_e_produto_multilingue() {
    let section = supervised_section();
    requires(
        &section,
        &[
            "a conceptual Trama query SHOULD use canonical English",
            "MUST record AGENT_QUERY_TRANSLATION",
            "AGENT_QUERY_TRANSLATION != PRODUCT_MULTILINGUAL_RETRIEVAL",
            "Portuguese aliases",
            "automatic translation",
            "stemming",
            "synonym expansion",
        ],
        "O13",
    );
}

/// O14 — o literal factual continua literal: o símbolo real é localizado pela
/// grafia da fonte, e a tradução do mesmo conceito não encontra nada. O produto
/// não traduz por baixo.
#[test]
fn o14_literal_factual_nao_e_traduzido() {
    let literal = pink(&["nav", "localizar", "binary_commit", "--json"]);
    assert_eq!(code(&literal), 0);
    assert!(stdout(&literal).contains("\"name\":\"binary_commit\""));

    let traduzido = pink(&["nav", "localizar", "commit_binario", "--json"]);
    assert_eq!(
        code(&traduzido),
        4,
        "a tradução do conceito não pode resolver para o literal da fonte"
    );
    let text = stdout(&traduzido);
    assert!(text.contains("\"candidates\":[]"), "identidade explícita");
    assert!(
        text.contains("\"extracted_candidates\":[]"),
        "candidato extraído"
    );
    assert!(
        text.contains("\"textual_occurrences\":[]"),
        "ocorrência textual"
    );

    requires(
        &supervised_section(),
        &["a factual literal target MUST stay literal"],
        "O14",
    );
}

/// O15 — #702, #703 e #704 continuam ausentes do produto: não há filtro de
/// intenção, não há camada de tradução e não há limiar de relevância.
#[test]
fn o15_findings_diferidos_continuam_nao_implementados() {
    // #702 — nenhuma superfície pública de intenção/filtro por camada.
    let intencao = pink(&["nav", "buscar", "--intencao", "implementation", "type"]);
    assert_eq!(code(&intencao), 2, "#702 não está autorizado");

    let ajuda = saida(&pink(&["nav"]));
    for proibido in ["--intencao", "--intent", "--limiar", "--threshold"] {
        assert!(
            !ajuda.contains(proibido),
            "a superfície pública de nav expôs '{proibido}'"
        );
    }

    // #704 — a tabela de códigos de saída continua a mesma.
    for esperado in [
        "0 sucesso",
        "2 uso inválido",
        "3 catálogo ausente/inválido",
        "4 sem resultado",
        "5 fonte/âncora ou drift",
        "6 harness",
        "7 política",
    ] {
        assert!(
            ajuda.contains(esperado),
            "a tabela de códigos de saída de nav perdeu '{esperado}'"
        );
    }

    // #703 — a regra versionada continua negando a derivação multilíngue.
    requires(
        &supervised_section(),
        &["MUST NOT derive from this rule", "a relevance threshold"],
        "O15",
    );
}

/// O16 — o arquivo histórico materializado continua 13/13 INTACT.
#[test]
fn o16_arquivo_historico_permanece_13_de_13_intacto() {
    let out = pink(&["nav", "projecao", "verificar"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    let text = stdout(&out);
    assert!(
        text.contains("verificar: INTACT"),
        "o arquivo deixou de ser íntegro"
    );
    let intactos = text
        .lines()
        .filter(|line| line.starts_with(".pinker/archive/") && line.contains(" INTACT "))
        .count();
    assert_eq!(intactos, 13, "o acervo histórico mudou de tamanho");
}

/// A fronteira da Rosa continua versionada: T3 não autoriza um segundo harness.
#[test]
fn fronteira_rosa_permanece_versionada() {
    requires(
        &supervised_section(),
        &[
            "MUST NOT create",
            "a persistent agent coordinator",
            "a cross-provider journal architecture",
            "a second Task lifecycle authority",
            "a quota manager",
            "a Rosa substitute",
            "a second agent harness",
            "STRONGER_AUTOMATION -> #594",
        ],
        "ROSA",
    );
}
