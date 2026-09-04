//! #442/C1 — a autoridade declarativa das intrínsecas históricas é uma só.
//!
//! Antes da consolidação, existência, contrato de parâmetros, contrato de
//! retorno, política de aridade e roteamento de runtime eram enumerados
//! independentemente por sete camadas. Estes testes provam que a enumeração
//! passou a ser única e que reintroduzir uma cópia local é detectável.

use pinker_v0::intrinsics::identity::{
    intrinsic_from_public_spelling, CalleeIdentity, IntrinsicIdentity, HISTORICAL_CANONICAL_ALIASES,
};
use pinker_v0::intrinsics::registry::{self, ArityPolicy, RuntimeRouting, Signature};
use std::collections::BTreeSet;

/// Grafias históricas que cada fase ainda cita literalmente, e por quê.
///
/// A auditoria é o gate contra a regressão que motivou a consolidação: se uma
/// fase voltar a listar grafias históricas por conta própria, o conjunto muda e
/// o teste fica vermelho. Cada exceção abaixo é implementação de fase — corpo,
/// monomorfização ou efeito de pilha —, nunca binding declarativo.
const CITACOES_AUTORIZADAS: &[(&str, &str, &[&str])] = &[
    (
        "src/ir_validate.rs",
        "tipagem da variádica: modelo `verso` seguido de N valores; o mínimo vem do registry",
        &["formatar_verso"],
    ),
    (
        "src/cfg_ir_validate.rs",
        "tipagem da variádica, idem",
        &["formatar_verso"],
    ),
    ("src/instr_select_validate.rs", "nenhuma", &[]),
    (
        "src/abstract_machine_validate.rs",
        "efeito de pilha das duas operações de aridade não fixa",
        &["afirmar", "formatar_verso"],
    ),
    (
        "src/backend_s.rs",
        "empacotamento próprio de `formatar_verso` e a forma de ponteiro de `alocar`",
        &["alocar", "formatar_verso"],
    ),
];

fn fonte_sem_testes(caminho: &str) -> String {
    let bruto = std::fs::read_to_string(format!("{}/{caminho}", env!("CARGO_MANIFEST_DIR")))
        .expect("fonte da fase legível");
    match bruto.find("\n#[cfg(test)]") {
        Some(corte) => bruto[..corte].to_string(),
        None => bruto,
    }
}

fn grafias_citadas(fonte: &str) -> BTreeSet<&'static str> {
    registry::grafias()
        .filter(|grafia| fonte.contains(&format!("\"{grafia}\"")))
        .collect()
}

#[test]
fn nenhuma_fase_de_validacao_reintroduz_enumeracao_historica() {
    for (caminho, motivo, autorizadas) in CITACOES_AUTORIZADAS {
        let citadas = grafias_citadas(&fonte_sem_testes(caminho));
        let esperadas: BTreeSet<&str> = autorizadas.iter().copied().collect();
        assert_eq!(
            citadas, esperadas,
            "{caminho}: citação literal de grafia histórica fora do autorizado ({motivo})"
        );
    }
}

#[test]
fn nenhuma_fase_reconstroi_a_tabela_de_simbolos_de_runtime() {
    let backend = fonte_sem_testes("src/backend_s.rs");
    for entrada in registry::HISTORICAL {
        if let RuntimeRouting::Symbol(simbolo) = entrada.runtime {
            assert!(
                !backend.contains(&format!("\"{}\" => Some(\"{simbolo}\")", entrada.spelling)),
                "{}: símbolo de runtime voltou a ser decidido no backend",
                entrada.spelling
            );
        }
    }
}

#[test]
fn registry_e_identidade_enxergam_a_mesma_superficie() {
    let grafias: Vec<&str> = registry::grafias().collect();
    assert_eq!(grafias.len(), 131);
    for grafia in &grafias {
        assert!(registry::e_historica(grafia));
        assert!(
            matches!(
                intrinsic_from_public_spelling(grafia),
                Some(IntrinsicIdentity::Historical(_))
            ),
            "{grafia}"
        );
    }
    // Uma grafia de usuário não entra na autoridade por parecer com uma.
    assert!(!registry::e_historica("tamanho_verso_do_usuario"));
    assert_eq!(
        intrinsic_from_public_spelling("tamanho_verso_do_usuario"),
        None
    );
}

#[test]
fn politica_de_alias_historico_continua_congelada() {
    assert_eq!(HISTORICAL_CANONICAL_ALIASES.len(), 3);
    for (alias, adulta) in HISTORICAL_CANONICAL_ALIASES {
        let alias_entry = registry::entrada(alias).expect("alias no registry");
        let adulta_entry = registry::entrada(adulta).expect("grafia adulta no registry");
        assert_eq!(alias_entry.signature, adulta_entry.signature);
        assert_eq!(alias_entry.runtime, adulta_entry.runtime);
        assert_eq!(
            registry::simbolo_runtime(alias),
            registry::simbolo_runtime(adulta)
        );
    }
    // Nenhum alias novo entra por descuido: a relação é exatamente esta.
    let aliases: BTreeSet<&str> = HISTORICAL_CANONICAL_ALIASES
        .iter()
        .map(|(alias, _)| *alias)
        .collect();
    assert_eq!(
        aliases,
        [
            "argumento_nomeado_ou",
            "argumento_nomeado_ou_ambiente_ou",
            "tem_argumento_nomeado"
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn simbolo_declarado_existe_no_runtime_nativo() {
    let runtime = include_str!("../runtime/pinker_rt/src/lib.rs");
    for entrada in registry::HISTORICAL {
        match entrada.runtime {
            RuntimeRouting::Symbol(simbolo) => assert!(
                runtime.contains(&format!("fn {simbolo}(")),
                "{}: símbolo {simbolo} ausente do runtime nativo",
                entrada.spelling
            ),
            RuntimeRouting::ByArity { .. } => {
                let aridades =
                    registry::aridades_aceitas(entrada.spelling).expect("recorte declarado");
                for argc in aridades {
                    let simbolo = registry::simbolo_runtime_por_aridade(entrada.spelling, *argc)
                        .expect("aridade dentro do recorte tem símbolo");
                    assert!(
                        runtime.contains(&format!("fn {simbolo}(")),
                        "{}: símbolo {simbolo} ausente do runtime nativo",
                        entrada.spelling
                    );
                }
            }
            RuntimeRouting::NotRouted => {}
        }
    }
}

#[test]
fn contrato_declarado_cobre_toda_a_superficie_das_fases_de_assinatura() {
    let declaradas: BTreeSet<&str> = registry::HISTORICAL
        .iter()
        .filter(|entrada| entrada.assinatura_ir().is_some())
        .map(|entrada| entrada.spelling)
        .collect();
    let genericas: BTreeSet<&str> = registry::HISTORICAL
        .iter()
        .filter(|entrada| entrada.signature == Signature::GenericMonomorphized)
        .map(|entrada| entrada.spelling)
        .collect();
    assert_eq!(declaradas.len(), 118);
    assert_eq!(genericas.len(), 13);
    assert!(declaradas.is_disjoint(&genericas));
    for grafia in &genericas {
        assert!(
            grafia.starts_with("lista_") || grafia.starts_with("mapa_"),
            "{grafia}: só as coleções genéricas são monomorfizadas antes das assinaturas"
        );
    }
}

#[test]
fn aridade_exata_derivada_do_contrato_de_parametros() {
    for entrada in registry::HISTORICAL {
        let Signature::Declared { arity, params, .. } = entrada.signature else {
            continue;
        };
        if arity == ArityPolicy::Exact {
            assert!(
                !registry::aridade_no_recorte(entrada.spelling, params.len()),
                "{}: aridade exata não abre recorte",
                entrada.spelling
            );
        }
    }
}

#[test]
fn identidade_do_callee_continua_decidindo_quem_e_intrinseca() {
    // A grafia sozinha nunca prova que a chamada é intrínseca: a decisão é da
    // identidade resolvida, e o registry não a substitui.
    assert!(CalleeIdentity::User.is_user());
    assert!(!CalleeIdentity::User.dispatches_as_builtin());
    let intrinseca = intrinsic_from_public_spelling("tamanho_verso").expect("grafia histórica");
    assert!(CalleeIdentity::Intrinsic(intrinseca).dispatches_as_builtin());
    assert_eq!(
        CalleeIdentity::Intrinsic(intrinseca).canonical_spelling(),
        Some("tamanho_verso")
    );
    assert_eq!(CalleeIdentity::User.canonical_spelling(), None);
}

/// Caminhos de módulo que C1-C fechou fisicamente, e a família que os substitui.
///
/// A varredura ignora este próprio arquivo: ele precisa citar os nomes antigos
/// para poder recusá-los em toda parte.
const CAMINHOS_FECHADOS: &[(&str, &str)] = &[
    ("crate::intrinsic_authority", "crate::intrinsics::identity"),
    (
        "pinker_v0::intrinsic_authority",
        "pinker_v0::intrinsics::identity",
    ),
    (
        "crate::familia_superficie",
        "crate::intrinsics::public_surface",
    ),
    (
        "pinker_v0::familia_superficie",
        "pinker_v0::intrinsics::public_surface",
    ),
];

fn raiz() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Todos os `.rs` versionados de `src/` e `tests/`, exceto este arquivo.
fn fontes_rust() -> Vec<(String, String)> {
    fn coletar(dir: &std::path::Path, saida: &mut Vec<std::path::PathBuf>) {
        for entrada in std::fs::read_dir(dir).expect("diretório de fontes legível") {
            let caminho = entrada.expect("entrada legível").path();
            if caminho.is_dir() {
                coletar(&caminho, saida);
            } else if caminho.extension().is_some_and(|ext| ext == "rs") {
                saida.push(caminho);
            }
        }
    }

    let raiz = raiz();
    let mut caminhos = Vec::new();
    coletar(&raiz.join("src"), &mut caminhos);
    coletar(&raiz.join("tests"), &mut caminhos);
    caminhos
        .into_iter()
        .filter(|caminho| !caminho.ends_with("c1_intrinsic_registry_tests.rs"))
        .map(|caminho| {
            let relativo = caminho
                .strip_prefix(&raiz)
                .expect("fonte dentro do repositório")
                .to_string_lossy()
                .into_owned();
            let texto = std::fs::read_to_string(&caminho).expect("fonte legível");
            (relativo, texto)
        })
        .collect()
}

#[test]
fn a_familia_fisica_das_intrinsecas_esta_fechada() {
    let raiz = raiz();

    for antigo in ["src/intrinsic_authority.rs", "src/familia_superficie.rs"] {
        assert!(
            !raiz.join(antigo).exists(),
            "{antigo} voltou a existir na raiz de src/"
        );
    }

    let mut membros: Vec<String> = std::fs::read_dir(raiz.join("src/intrinsics"))
        .expect("família das intrínsecas presente")
        .map(|entrada| {
            entrada
                .expect("entrada legível")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    membros.sort();

    assert_eq!(
        membros,
        vec![
            "identity.rs".to_string(),
            "mod.rs".to_string(),
            "public_surface.rs".to_string(),
            "registry.rs".to_string(),
        ],
        "a família física das intrínsecas mudou de composição"
    );
}

/// Remove comentários e literais de texto, para que a varredura veja só código.
///
/// Sem isto o oráculo mente nos dois sentidos: um comentário interposto
/// (`pub mod /* compat */ antigo`) esconderia o stub, e um comentário que
/// mencione `mod antigo` acusaria stub onde não há.
fn codigo_sem_comentarios_nem_textos(fonte: &str) -> String {
    let bytes: Vec<char> = fonte.chars().collect();
    let mut saida = String::with_capacity(fonte.len());
    let mut i = 0;
    let mut profundidade_de_bloco = 0usize;
    while i < bytes.len() {
        let dois: String = bytes[i..(i + 2).min(bytes.len())].iter().collect();
        if profundidade_de_bloco > 0 {
            if dois == "/*" {
                profundidade_de_bloco += 1;
                i += 2;
            } else if dois == "*/" {
                profundidade_de_bloco -= 1;
                i += 2;
            } else {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        if dois == "/*" {
            profundidade_de_bloco = 1;
            i += 2;
            saida.push(' ');
            continue;
        }
        if dois == "//" {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        // Literal de texto cru: `r"..."`, `r#"..."#`, com o mesmo número de `#`.
        if bytes[i] == 'r' {
            let mut cerquilhas = 0;
            while i + 1 + cerquilhas < bytes.len() && bytes[i + 1 + cerquilhas] == '#' {
                cerquilhas += 1;
            }
            if bytes.get(i + 1 + cerquilhas) == Some(&'"') {
                let fecho: String = std::iter::once('"')
                    .chain(std::iter::repeat('#').take(cerquilhas))
                    .collect();
                i += 2 + cerquilhas;
                while i < bytes.len() {
                    let janela: String = bytes[i..(i + fecho.len()).min(bytes.len())]
                        .iter()
                        .collect();
                    if janela == fecho {
                        i += fecho.len();
                        break;
                    }
                    i += 1;
                }
                saida.push(' ');
                continue;
            }
        }
        if bytes[i] == '"' {
            i += 1;
            while i < bytes.len() && bytes[i] != '"' {
                i += if bytes[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            saida.push(' ');
            continue;
        }
        // Literal de caractere: `'x'` ou `'\x'`. Um tempo de vida (`'a`) não fecha
        // aspa e segue como código — apagá-lo esconderia o identificador seguinte.
        if bytes[i] == '\'' {
            let escapado = bytes.get(i + 1) == Some(&'\\');
            let fim = if escapado { i + 3 } else { i + 2 };
            if bytes.get(fim) == Some(&'\'') {
                i = fim + 1;
                saida.push(' ');
                continue;
            }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    saida
}

/// Pares `(palavra-chave, identificador)` do código, ignorando espaço e pontuação.
///
/// A varredura é sintática de propósito: `mod x;`, `pub mod x {`, `use ... as x;`,
/// `r#x` e qualquer quebra de linha ou comentário entre eles produzem o mesmo par.
///
/// Limite declarado: identificador produzido por expansão de macro não aparece
/// no texto e escapa desta varredura. Nenhum oráculo textual o alcança.
fn pares_de_identificadores(codigo: &str) -> Vec<(String, String)> {
    let sem_raw = codigo.replace("r#", "");
    let palavras: Vec<String> = sem_raw
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|palavra| !palavra.is_empty())
        .map(str::to_string)
        .collect();
    palavras
        .windows(2)
        .map(|par| (par[0].clone(), par[1].clone()))
        .collect()
}

#[test]
fn nenhum_stub_de_compatibilidade_preserva_o_caminho_antigo() {
    for (caminho, texto) in fontes_rust() {
        if !caminho.starts_with("src/") {
            continue;
        }
        let pares = pares_de_identificadores(&codigo_sem_comentarios_nem_textos(&texto));
        for modulo in ["intrinsic_authority", "familia_superficie"] {
            let par = |chave: &str| (chave.to_string(), modulo.to_string());
            assert!(
                !pares.contains(&par("mod")),
                "{caminho} ainda declara o módulo `{modulo}` (arquivo ou inline)"
            );
            assert!(
                !pares.contains(&par("as")),
                "{caminho} reexporta a família nova sob o nome antigo `{modulo}`"
            );
        }
    }

    let mod_familia =
        std::fs::read_to_string(raiz().join("src/intrinsics/mod.rs")).expect("mod.rs legível");
    for modulo in ["intrinsic_authority", "familia_superficie"] {
        assert!(
            !mod_familia.contains(modulo),
            "src/intrinsics/mod.rs ainda nomeia `{modulo}` como caminho vivo"
        );
    }
}

#[test]
fn os_consumidores_apontam_para_a_familia_nova() {
    let fontes = fontes_rust();
    let mut novos = 0usize;

    for (caminho, texto) in &fontes {
        for (antigo, novo) in CAMINHOS_FECHADOS {
            assert!(
                !texto.contains(antigo),
                "{caminho} ainda importa pelo caminho fechado `{antigo}`; use `{novo}`"
            );
            novos += texto.matches(novo).count();
        }
    }

    assert!(
        novos > 0,
        "nenhum consumidor cita a família nova: a varredura perdeu o alvo"
    );
}

#[test]
fn identidade_e_superficie_publica_continuam_autoridade_unica() {
    let fontes = fontes_rust();

    for (autoridade, marcador) in [
        (
            "src/intrinsics/identity.rs",
            "pub const HISTORICAL_CANONICAL_ALIASES",
        ),
        ("src/intrinsics/public_surface.rs", "pub const FAMILIAS"),
    ] {
        let donos: Vec<&str> = fontes
            .iter()
            .filter(|(caminho, texto)| caminho.starts_with("src/") && texto.contains(marcador))
            .map(|(caminho, _)| caminho.as_str())
            .collect();
        assert_eq!(
            donos,
            vec![autoridade],
            "`{marcador}` deixou de ter dono físico único"
        );
    }
}
