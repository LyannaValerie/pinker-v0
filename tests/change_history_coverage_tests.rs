use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

const BASELINE_PR: u64 = 330;
const CUTOVER: &str = "1df2a6afff423bc7564e7322880e24af683f6089";

#[derive(Debug)]
struct Exception {
    merge_sha: String,
    reason_code: String,
}

fn exceptions() -> BTreeMap<u64, Exception> {
    let text = fs::read_to_string(".pinker/changes/historical-exceptions-v1.yaml")
        .expect("ler exceções históricas");
    assert!(text.contains("schema: 1"));
    assert!(text.contains("baseline_pr: 330"));
    assert!(text.contains(&format!("cutover_merge_sha: {CUTOVER}")));

    let mut parsed = BTreeMap::new();
    let mut current_pr = None;
    let mut merge_sha = None;
    let mut reason_code = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - pr: ") {
            if let Some(pr) = current_pr.take() {
                let old = parsed.insert(
                    pr,
                    Exception {
                        merge_sha: merge_sha.take().expect("merge_sha da exceção"),
                        reason_code: reason_code.take().expect("reason_code da exceção"),
                    },
                );
                assert!(old.is_none(), "exceção duplicada para PR #{pr}");
            }
            current_pr = Some(value.parse::<u64>().expect("número de PR"));
        } else if let Some(value) = line.strip_prefix("    merge_sha: ") {
            merge_sha = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("    reason_code: ") {
            reason_code = Some(value.to_owned());
        }
    }
    if let Some(pr) = current_pr {
        let old = parsed.insert(
            pr,
            Exception {
                merge_sha: merge_sha.expect("merge_sha da exceção final"),
                reason_code: reason_code.expect("reason_code da exceção final"),
            },
        );
        assert!(old.is_none(), "exceção duplicada para PR #{pr}");
    }
    parsed
}

fn reachable_merges() -> BTreeMap<u64, String> {
    let output = Command::new("git")
        .args(["log", "--merges", "--format=%H%x09%s", CUTOVER])
        .output()
        .expect("executar git log local");
    assert!(output.status.success(), "git log deve funcionar sem rede");
    let text = String::from_utf8(output.stdout).expect("git log UTF-8");
    text.lines()
        .filter_map(|line| {
            let (sha, subject) = line.split_once('\t')?;
            let rest = subject.strip_prefix("Merge pull request #")?;
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let pr = rest[..digits].parse::<u64>().ok()?;
            (pr > BASELINE_PR).then(|| (pr, sha.to_owned()))
        })
        .collect()
}

fn manifest_prs() -> BTreeSet<u64> {
    fs::read_dir(".pinker/changes")
        .expect("listar manifestos")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let number = name.strip_prefix("pr-")?.strip_suffix(".yaml")?;
            let pr = number.parse::<u64>().ok()?;
            let text = fs::read_to_string(entry.path()).ok()?;
            let declared = format!("  number: {pr}");
            assert!(text.contains("schema: 1"), "schema ausente em {name}");
            assert!(
                text.contains(&declared),
                "número interno divergente em {name}"
            );
            Some(pr)
        })
        .collect()
}

#[test]
fn workflows_disponibilizam_historico_local_completo() {
    for workflow in [".github/workflows/ci.yml", ".github/workflows/trama.yml"] {
        let text = fs::read_to_string(workflow).expect("ler workflow");
        assert!(
            text.contains("fetch-depth: 0"),
            "{workflow} precisa materializar o histórico para o gate local sem rede"
        );
    }
}

#[test]
fn todo_merge_pos_baseline_ate_o_cutover_tem_manifesto_ou_excecao() {
    assert!(Path::new(".pinker/doc.toml").exists());
    let merges = reachable_merges();
    let manifests = manifest_prs();
    let exceptions = exceptions();
    let allowed_reasons = [
        "missing_original_structured_block",
        "ambiguous_change_kind",
        "insufficient_canonical_evidence",
    ];

    for (&pr, sha) in &merges {
        let has_manifest = manifests.contains(&pr);
        let exception = exceptions.get(&pr);
        assert!(
            has_manifest ^ exception.is_some(),
            "PR #{pr} precisa de exatamente uma forma de cobertura"
        );
        if let Some(exception) = exception {
            assert_eq!(
                &exception.merge_sha, sha,
                "merge SHA divergente para PR #{pr}"
            );
            assert!(
                allowed_reasons.contains(&exception.reason_code.as_str()),
                "reason_code inválido para PR #{pr}"
            );
        }
    }

    for &pr in exceptions.keys() {
        assert!(
            merges.contains_key(&pr),
            "exceção para PR #{pr} não pertence ao histórico alcançável no cutover"
        );
        assert!(
            !manifests.contains(&pr),
            "PR #{pr} não pode ter manifesto e exceção"
        );
    }
}

#[test]
fn excecoes_sao_proibidas_depois_do_cutover() {
    let output = Command::new("git")
        .args([
            "log",
            "--merges",
            "--format=%H%x09%s",
            &format!("{CUTOVER}..HEAD"),
        ])
        .output()
        .expect("executar git log local");
    assert!(output.status.success());
    let later = String::from_utf8(output.stdout).expect("git log UTF-8");
    for pr in exceptions().keys() {
        assert!(
            !later
                .lines()
                .any(|line| line.contains(&format!("Merge pull request #{pr} "))),
            "exceção histórica posterior ao cutover para PR #{pr}"
        );
    }
}
