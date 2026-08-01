//! Continuação do hotfix pós-PR #411 — item V4: a proveniência de um ponteiro
//! não pode depender da **forma** da chamada que o devolveu.
//!
//! A revisão humana do head `3725118` apontou que o cast `virar seta<T>`
//! promovia a `Fabricated` toda origem `Unclassified`, mesmo quando a origem já
//! era tipada como ponteiro. Corrigir só isso revelou uma assimetria
//! preexistente: `SelectedInstr::Call` com retorno ponteiro era classificada
//! `Public`, mas `CallIndirect`, `CallRaw` e `TraitCall` caíam em
//! `Unclassified`. O cast compensava a lacuna por acidente — ao remover a
//! promoção, o acesso pelas formas indiretas passava a descer cru.
//!
//! Medido em ELF real no head `3725118`, com `liberar` seguido de acesso:
//!
//! | forma    | sem cast   | com cast |
//! |----------|------------|----------|
//! | direta   | exit 1 UAF | exit 1 UAF |
//! | indireta | **exit 139 (SIGSEGV)** | exit 1 UAF |
//! | crua     | **exit 139 (SIGSEGV)** | exit 1 UAF |
//! | trato    | **exit 139 (SIGSEGV)** | exit 1 UAF |
//!
//! O caso **sem cast** é o que prova onde está a correção: ela é da
//! classificação da chamada, não do tratamento posterior do cast.
//!
//! O que continua fora do subconjunto atual da linguagem, e por isso não tem
//! reprodução de ponta a ponta: `seta<seta<T>>` (recusada com "seta de seta
//! ainda não é suportada nesta fase"), `seta virar u64` (recusada pela
//! semântica) e carga de união com ponteiro. Sem essas três, **nenhum ponteiro
//! pode ser carregado de memória**, e a classe `Unclassified` tipada como
//! ponteiro só existe como unidade — sua evidência está em
//! `src/backend_s.rs`, módulo `tests_proveniencia_de_ponteiro`.

mod common;

use common::render_backend_s_external_subset_nativo;
use common::ControlledCommand as Command;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// As quatro formas de chamada que devolvem valor no IR selecionado, com o
/// trecho que obtém uma região pública por cada uma delas.
const FORMAS: [(&str, &str, &str); 4] = [
    (
        "direta",
        "",
        "    nova p: seta<u8> = fabricar();\n",
    ),
    (
        "indireta",
        "",
        "    nova f: carinho() -> seta<u8> = fabricar;\n    nova p: seta<u8> = f();\n",
    ),
    (
        "crua",
        "",
        "    nova fp: seta<carinho() -> seta<u8> > = &fabricar;\n    nova p: seta<u8> = fp();\n",
    ),
    (
        "trato",
        "trato Fonte {\n    carinho regiao(valor: si) -> seta<u8>;\n}\n\nimpl Fonte para bombom {\n    carinho regiao(valor: bombom) -> seta<u8> {\n        mimo alocar(8);\n    }\n}\n",
        "    nova objeto: trato<Fonte> = 1 virar trato<Fonte>;\n    nova p: seta<u8> = objeto.regiao();\n",
    ),
];

/// Monta um programa que obtém a região pela forma pedida e a acessa.
///
/// `com_cast` insere um `virar seta<u32>` entre a obtenção e o acesso, para
/// separar o efeito da classificação da chamada do efeito do cast.
fn programa(forma: &str, com_cast: bool, store: bool) -> String {
    let (_, prelúdio, obtencao) = FORMAS
        .iter()
        .find(|(nome, _, _)| *nome == forma)
        .expect("forma de chamada conhecida");
    let fabricar = if forma == "trato" {
        String::new()
    } else {
        "carinho fabricar() -> seta<u8> {\n    mimo alocar(8);\n}\n".to_string()
    };
    let (cast, alvo) = if com_cast {
        ("    nova q: seta<u32> = p virar seta<u32>;\n", "q")
    } else {
        ("", "p")
    };
    let acesso = if store {
        format!("    *{alvo} = 7;\n")
    } else {
        format!("    falar(*{alvo});\n")
    };
    format!(
        "pacote main;\n\n{prelúdio}{fabricar}\ncarinho principal() -> bombom {{\n{obtencao}{cast}{acesso}    mimo 0;\n}}\n"
    )
}

/// Largura e alinhamento operacionais do acesso emitido pelo programa acima.
fn largura_do_acesso(com_cast: bool) -> u64 {
    if com_cast {
        4
    } else {
        1
    }
}

// @pinker-nav:start evidencia.hotfix.v4-chamadas-ponteiro
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da simetria das formas de chamada na proveniência de ponteiro: para chamada direta, indireta, por endereço cru de código e de trato, o ponteiro devolvido é classificado como memória pública e o acesso emite pinker_publico_validar_acesso em load e em store, com e sem cast ponteiro→ponteiro; os doze exemplos de região válida, uso após liberar e uso após liberar com cast concordam em exit e stdout entre interpretador e ELF nativo, com diagnóstico E-RUNTIME-MEM-USE-AFTER-FREE e sem nenhum término por sinal de memória.
#[test]
fn toda_forma_de_chamada_emite_validacao_em_load_e_store() {
    for (forma, _, _) in FORMAS {
        for com_cast in [false, true] {
            for store in [false, true] {
                let fonte = programa(forma, com_cast, store);
                let assembly = render_backend_s_external_subset_nativo(&fonte)
                    .unwrap_or_else(|erro| panic!("assembly de {forma} (cast={com_cast}, store={store}): {erro:?}\n{fonte}"));
                let largura = largura_do_acesso(com_cast);
                assert!(
                    assembly.contains("call pinker_publico_validar_acesso"),
                    "{forma} (cast={com_cast}, store={store}): acesso por ponteiro de chamada sem validação\n{fonte}\n{assembly}"
                );
                assert!(
                    assembly.contains(&format!("movq ${largura}, %rsi")),
                    "{forma} (cast={com_cast}, store={store}): largura {largura} não chegou ao validador\n{assembly}"
                );
                assert!(
                    assembly.contains(&format!("movq ${largura}, %rdx")),
                    "{forma} (cast={com_cast}, store={store}): alinhamento {largura} não chegou ao validador\n{assembly}"
                );
            }
        }
    }
}

/// Load e store compartilham a decisão: o mesmo programa, mudando só o sentido
/// do acesso, precisa emitir a mesma quantidade de validações.
#[test]
fn load_e_store_nao_divergem_na_decisao() {
    for (forma, _, _) in FORMAS {
        for com_cast in [false, true] {
            let load = render_backend_s_external_subset_nativo(&programa(forma, com_cast, false))
                .expect("assembly de load");
            let store = render_backend_s_external_subset_nativo(&programa(forma, com_cast, true))
                .expect("assembly de store");
            assert_eq!(
                load.matches("call pinker_publico_validar_acesso").count(),
                store.matches("call pinker_publico_validar_acesso").count(),
                "{forma} (cast={com_cast}): load e store divergiram na validação"
            );
        }
    }
}

/// A documentação normativa precisa descrever o que o código faz — nem menos,
/// nem mais.
///
/// Duas regressões documentais concretas já aconteceram e este teste as barra:
/// voltar a falar em "três classes" de proveniência quando o código tem quatro,
/// e restaurar a garantia universal "nenhum programa Pinker deve terminar por
/// sinal de memória", que `Unclassified` não sustenta.
#[test]
fn documentacao_normativa_descreve_as_quatro_classes_sem_garantia_universal() {
    for caminho in ["MANUAL.md", "docs/expandir.md"] {
        let texto = std::fs::read_to_string(caminho).expect("ler documentação normativa");
        for classe in ["Public", "Internal", "Fabricated", "Unclassified"] {
            assert!(
                texto.contains(classe),
                "{caminho}: a classe de proveniência `{classe}` precisa estar descrita"
            );
        }
        for formulacao in [
            "três classes",
            "tres classes",
            "tri-estado",
            "três domínios",
            "Nenhum programa Pinker deve terminar por",
        ] {
            assert!(
                !texto.contains(formulacao),
                "{caminho}: formulação obsoleta ou mais ampla que o código: {formulacao:?}"
            );
        }
    }

    let manual = std::fs::read_to_string("MANUAL.md").expect("MANUAL.md");
    assert!(
        manual.contains("não compartilham implementação de validação"),
        "MANUAL.md precisa dizer que interpretador e nativo têm implementações distintas"
    );
    assert!(
        manual.contains("Unclassified` permanece fora da validação pública"),
        "MANUAL.md precisa registrar o limite da classe não classificada"
    );
}

fn compilar(exemplo: &str, runtime_lib: &std::path::Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_hf412_chamadas_{nanos}"));
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", runtime_lib)
        .output()
        .expect("invocar pink build --nativo");
    assert!(
        build.status.success(),
        "build nativo de {exemplo} falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nome = std::path::Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo");
    out_dir.join(nome)
}

/// Região devolvida por cada forma de chamada é memória pública de verdade: o
/// acesso válido continua válido nos dois back-ends, com o mesmo stdout.
///
/// Sem este caso a simetria poderia ser "corrigida" recusando tudo.
#[test]
fn regiao_devolvida_por_qualquer_forma_de_chamada_permanece_acessivel() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    for (forma, _, _) in FORMAS {
        let exemplo = format!("examples/hotfix_v4_chamada_{forma}_regiao_valida_valido.pink");
        let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", &exemplo])
            .output()
            .expect("interpretador");
        assert_eq!(
            interpretado.status.code(),
            Some(0),
            "{exemplo}: acesso legítimo recusado no interpretador: {}",
            String::from_utf8_lossy(&interpretado.stderr)
        );
        let executavel = compilar(&exemplo, &runtime_lib);
        let nativo = Command::new(&executavel).output().expect("ELF nativo");
        assert_eq!(
            nativo.status.code(),
            Some(0),
            "{exemplo}: acesso legítimo recusado no nativo: {}",
            String::from_utf8_lossy(&nativo.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&interpretado.stdout),
            String::from_utf8_lossy(&nativo.stdout),
            "{exemplo}: paridade de stdout"
        );
        let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
    }
}

/// O núcleo da regressão: `liberar` seguido de acesso precisa diagnosticar em
/// **toda** forma de chamada, com e sem cast, nos dois back-ends, e nunca
/// terminar por sinal.
///
/// A variante `sem cast` é obrigatória: no head `3725118` ela terminava por
/// SIGSEGV nas três formas indiretas.
#[test]
fn uso_apos_liberar_diagnostica_em_toda_forma_de_chamada() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    for (forma, _, _) in FORMAS {
        for sufixo in ["uso_apos_liberar", "cast_uso_apos_liberar"] {
            let exemplo = format!("examples/hotfix_v4_chamada_{forma}_{sufixo}_invalido.pink");
            let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
                .args(["--run", &exemplo])
                .output()
                .expect("interpretador");
            assert_eq!(
                interpretado.status.code(),
                Some(1),
                "{exemplo}: interpretador deveria diagnosticar"
            );
            assert!(
                String::from_utf8_lossy(&interpretado.stderr)
                    .contains("E-RUNTIME-MEM-USE-AFTER-FREE"),
                "{exemplo}: diagnóstico interpretado inesperado: {}",
                String::from_utf8_lossy(&interpretado.stderr)
            );

            let executavel = compilar(&exemplo, &runtime_lib);
            let nativo = Command::new(&executavel).output().expect("ELF nativo");
            assert!(
                nativo.status.code().is_some(),
                "{exemplo}: nativo terminou por sinal — SIGSEGV/SIGBUS não podem escapar"
            );
            assert_eq!(
                nativo.status.code(),
                Some(1),
                "{exemplo}: nativo deveria diagnosticar (stderr: {})",
                String::from_utf8_lossy(&nativo.stderr)
            );
            assert!(
                String::from_utf8_lossy(&nativo.stderr).contains("E-RUNTIME-MEM-USE-AFTER-FREE"),
                "{exemplo}: diagnóstico nativo inesperado: {}",
                String::from_utf8_lossy(&nativo.stderr)
            );
            let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
        }
    }
}
// @pinker-nav:end evidencia.hotfix.v4-chamadas-ponteiro
