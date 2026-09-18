use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

// POT/LPT: AUTHORITY #698
// POT/LPT: INVARIANT HISTORICAL_INTERVAL_IS_FINITE — a cobertura obrigatória
// vale para os merges de PR alcançáveis a partir de `CUTOVER` e maiores que
// `BASELINE_PR`. Nenhum merge posterior ao cutover deve manifesto ou exceção.
// POT/LPT: MUST NOT estender o intervalo histórico para PRs novos.

/// Marco documental: o próprio PR #330 é exclusivo.
const BASELINE_PR: u64 = 330;
/// Fim do intervalo histórico: merge do PR #410.
const CUTOVER: &str = "1df2a6afff423bc7564e7322880e24af683f6089";

/// Código do diagnóstico de clone raso.
///
/// A reconstrução histórica percorre merges alcançáveis a partir do cutover. Em
/// clone raso o enxerto corta esse alcance e o gate falhava dizendo que uma
/// exceção "não pertence ao histórico alcançável" — uma mensagem que sugere
/// corrupção histórica quando a causa é apenas profundidade de clone. A CI já
/// usa `fetch-depth: 0`; o problema era só o diagnóstico local.
const CODIGO_CLONE_RASO: &str = "E-CHANGE-HISTORY-SHALLOW-CLONE";

/// Detecta clone raso.
///
/// `git rev-parse --is-shallow-repository` é a consulta canônica. O arquivo
/// `.git/shallow` é o fallback para versões de git anteriores a ela; um `git`
/// indisponível não é tratado como "completo", porque isso enfraqueceria a
/// verificação — nesse caso a falha original continua valendo.
fn repositorio_e_raso(diretorio: &Path) -> bool {
    let consulta = Command::new("git")
        .arg("-C")
        .arg(diretorio)
        .args(["rev-parse", "--is-shallow-repository"])
        .output();
    if let Ok(saida) = consulta {
        if saida.status.success() {
            let resposta = String::from_utf8_lossy(&saida.stdout);
            return resposta.trim() == "true";
        }
    }
    diretorio.join(".git").join("shallow").exists()
}

/// Diagnóstico acionável quando o histórico local está incompleto.
///
/// Devolve `None` em clone completo — o gate segue exatamente como antes.
fn diagnostico_de_historico_incompleto(diretorio: &Path) -> Option<String> {
    repositorio_e_raso(diretorio).then(|| {
        format!(
            "{CODIGO_CLONE_RASO}\n\
             a reconstrução histórica exige o histórico completo;\n\
             execute `git fetch --unshallow` e repita a validação"
        )
    })
}

/// Falha cedo, e com a causa certa, quando o clone é raso.
///
/// A verificação não é enfraquecida: o teste continua falhando: o que muda é o
/// diagnóstico. `--unshallow` nunca é executado automaticamente.
fn exigir_historico_completo() {
    if let Some(diagnostico) = diagnostico_de_historico_incompleto(Path::new(".")) {
        panic!("{diagnostico}");
    }
}

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

/// Monta um repositório mínimo com dois commits e devolve seu caminho.
fn repositorio_sintetico(nome: &str) -> std::path::PathBuf {
    let raiz = std::env::temp_dir().join(format!(
        "pinker-clone-raso-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("tempo do sistema")
            .as_nanos()
    ));
    let origem = raiz.join("origem");
    fs::create_dir_all(&origem).expect("criar diretório de origem");

    let git = |args: &[&str]| {
        let saida = Command::new("git")
            .arg("-C")
            .arg(&origem)
            .args(args)
            .output()
            .expect("executar git");
        assert!(
            saida.status.success(),
            "git {args:?} falhou: {}",
            String::from_utf8_lossy(&saida.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "pinker-teste"]);
    git(&["config", "user.email", "pinker-teste@example.invalid"]);
    for indice in 0..2 {
        fs::write(origem.join("arquivo.txt"), format!("conteudo {indice}\n"))
            .expect("gravar arquivo");
        git(&["add", "arquivo.txt"]);
        git(&["commit", "--quiet", "-m", &format!("commit {indice}")]);
    }
    raiz
}

// @pinker-nav:start evidencia.hotfix.clone-raso-diagnostico
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência do diagnóstico de clone raso do gate de cobertura histórica (hotfix pós-PR #411): um clone com profundidade 1 é detectado e recebe E-CHANGE-HISTORY-SHALLOW-CLONE com a instrução de `git fetch --unshallow`, enquanto clone completo — o repositório sintético de origem e o próprio workspace — não muda de comportamento e segue sem diagnóstico.
#[test]
fn clone_raso_recebe_diagnostico_especifico() {
    let raiz = repositorio_sintetico("raso");
    let origem = raiz.join("origem");
    let destino = raiz.join("raso");
    let clone = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1"])
        .arg(format!("file://{}", origem.display()))
        .arg(&destino)
        .output()
        .expect("executar git clone");
    assert!(
        clone.status.success(),
        "clone raso falhou: {}",
        String::from_utf8_lossy(&clone.stderr)
    );

    assert!(
        repositorio_e_raso(&destino),
        "clone com --depth 1 precisa ser detectado como raso"
    );
    let diagnostico = diagnostico_de_historico_incompleto(&destino)
        .expect("clone raso precisa produzir diagnóstico");
    assert!(diagnostico.contains(CODIGO_CLONE_RASO), "{diagnostico}");
    assert!(
        diagnostico.contains("git fetch --unshallow"),
        "o diagnóstico precisa dizer o que fazer: {diagnostico}"
    );
    assert!(
        !diagnostico.contains("não pertence ao histórico alcançável"),
        "o diagnóstico de clone raso não pode sugerir corrupção histórica: {diagnostico}"
    );

    let _ = fs::remove_dir_all(&raiz);
}

#[test]
fn clone_completo_nao_muda_de_comportamento() {
    // O próprio workspace, onde a suíte roda, precisa estar completo — é a
    // garantia de que o gate real não foi enfraquecido. Passa pelo mesmo
    // caminho dos gates históricos para que um workspace raso receba o
    // diagnóstico acionável em vez de uma asserção obscura.
    exigir_historico_completo();

    let raiz = repositorio_sintetico("completo");
    let origem = raiz.join("origem");
    let destino = raiz.join("completo");
    let clone = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(format!("file://{}", origem.display()))
        .arg(&destino)
        .output()
        .expect("executar git clone");
    assert!(clone.status.success());

    for completo in [&origem, &destino] {
        assert!(
            !repositorio_e_raso(completo),
            "{} não deveria ser classificado como raso",
            completo.display()
        );
        assert!(
            diagnostico_de_historico_incompleto(completo).is_none(),
            "clone completo não pode receber diagnóstico de clone raso"
        );
    }

    let _ = fs::remove_dir_all(&raiz);
}
// @pinker-nav:end evidencia.hotfix.clone-raso-diagnostico

/// O gate histórico roda sem rede; todo workflow que exista precisa materializar
/// o histórico local. Um workflow inexistente não é um portão reprovado.
#[test]
fn workflows_existentes_disponibilizam_historico_local_completo() {
    let mut checked = 0usize;
    for entry in fs::read_dir(".github/workflows")
        .expect("diretório de workflows presente")
        .flatten()
    {
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml") | Some("yaml")
        ) {
            continue;
        }
        checked += 1;
        let text = fs::read_to_string(&path).expect("ler workflow");
        assert!(
            text.contains("fetch-depth: 0"),
            "{} precisa materializar o histórico para o gate local sem rede",
            path.display()
        );
    }
    assert!(checked >= 1, "deve haver ao menos um workflow permanente");
}

#[test]
fn todo_merge_pos_baseline_ate_o_cutover_tem_manifesto_ou_excecao() {
    exigir_historico_completo();
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
    exigir_historico_completo();
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

/// Controle causal de #698 (M3/M7): todo merge posterior ao cutover fica fora da
/// obrigação de cobertura. Um PR novo sem bloco `pinker-change` não produz
/// manifesto, não produz exceção e não reprova o gate histórico.
#[test]
fn merges_posteriores_ao_cutover_nao_devem_cobertura() {
    exigir_historico_completo();
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
    let text = String::from_utf8(output.stdout).expect("git log UTF-8");

    let posteriores: Vec<u64> = text
        .lines()
        .filter_map(|line| {
            let (_, subject) = line.split_once('\t')?;
            let rest = subject.strip_prefix("Merge pull request #")?;
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            rest[..digits].parse::<u64>().ok()
        })
        .collect();
    assert!(
        !posteriores.is_empty(),
        "o intervalo histórico precisa ser estritamente menor que a main atual"
    );

    let cobertos = reachable_merges();
    for pr in &posteriores {
        assert!(
            !cobertos.contains_key(pr),
            "PR #{pr} é posterior ao cutover e não pertence ao intervalo histórico"
        );
    }

    // Nenhum backfill retroativo é exigido: o acervo cobre o intervalo finito,
    // não a main inteira.
    let manifests = manifest_prs();
    assert!(
        manifests.len() < posteriores.len() + cobertos.len(),
        "o acervo não pode cobrir um-para-um todos os merges da main"
    );
}

/// Controle causal de #698 (M1): dentro do intervalo finito preservado, cada PR
/// continua coberto por exatamente uma forma aceita.
#[test]
fn intervalo_historico_preservado_e_finito_e_completo() {
    exigir_historico_completo();
    let merges = reachable_merges();
    let manifests = manifest_prs();
    let exceptions = exceptions();

    assert!(
        !merges.is_empty(),
        "o intervalo histórico não pode ser vazio"
    );
    let maior = merges.keys().max().copied().expect("maior PR do intervalo");
    assert!(
        maior <= 410,
        "o intervalo histórico terminou no merge do PR #410, veio #{maior}"
    );

    for &pr in merges.keys() {
        assert!(
            manifests.contains(&pr) ^ exceptions.contains_key(&pr),
            "PR #{pr} precisa de exatamente uma forma de cobertura"
        );
    }
}
