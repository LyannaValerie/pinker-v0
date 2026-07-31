//! Hotfix pós-PR #411 — item V4: a validação nativa de acesso não pode ignorar
//! endereço fabricado a partir de inteiro.
//!
//! A linguagem permite `<inteiro> virar seta<T>`. A análise de proveniência do
//! back-end classificava esse ponteiro como "não público" e **omitia** a chamada
//! a `pinker_publico_validar_acesso`; o acesso descia cru e o processo morria por
//! SIGSEGV (exit 139), enquanto o interpretador — cuja memória é uma tabela —
//! diagnosticava o mesmo programa com exit 1.
//!
//! A correção classifica a proveniência em três classes (pública, interna,
//! fabricada) e valida as duas primeiras exceto a interna: memória pública e
//! endereço fabricado passam pelo validador, o ambiente de closure continua
//! isento porque não é memória pública e não poderia ser confrontado com o
//! registro público.
//!
//! O limite residual está documentado em `MANUAL.md`, seção "Memória
//! explícita": o
//! interpretador tem um espaço de endereços sintético, no qual inteiros
//! pequenos podem coincidir com globais, então os dois back-ends concordam em
//! *recusar deterministicamente* endereços não registrados, não em *quais*
//! endereços fabricados são válidos.

mod common;

use common::render_backend_s_external_subset_nativo;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tipos escalares endereçáveis, com largura e alinhamento operacionais.
const TIPOS: [(&str, u64); 10] = [
    ("u8", 1),
    ("i8", 1),
    ("logica", 1),
    ("u16", 2),
    ("i16", 2),
    ("u32", 4),
    ("i32", 4),
    ("u64", 8),
    ("i64", 8),
    ("bombom", 8),
];

fn valor_para(tipo: &str) -> &'static str {
    if tipo == "logica" {
        "verdade"
    } else {
        "1"
    }
}

fn programa_store(tipo: &str) -> String {
    format!(
        "pacote main;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova cru: seta<bombom> = 4096 virar seta<bombom>;\n\
         \x20   nova alvo: seta<{tipo}> = cru virar seta<{tipo}>;\n\
         \x20   *alvo = {valor};\n\
         \x20   mimo 0;\n\
         }}\n",
        valor = valor_para(tipo)
    )
}

fn programa_load(tipo: &str) -> String {
    format!(
        "pacote main;\n\
         carinho principal() -> bombom {{\n\
         \x20   nova cru: seta<bombom> = 4096 virar seta<bombom>;\n\
         \x20   nova alvo: seta<{tipo}> = cru virar seta<{tipo}>;\n\
         \x20   falar(*alvo);\n\
         \x20   mimo 0;\n\
         }}\n"
    )
}

/// Conta as chamadas ao validador e confere os metadados passados em `%rsi`
/// (largura) e `%rdx` (alinhamento).
fn exigir_validacao_emitida(rotulo: &str, assembly: &str, largura: u64) {
    assert!(
        assembly.contains("call pinker_publico_validar_acesso"),
        "{rotulo}: acesso por ponteiro fabricado sem chamada ao validador\n{assembly}"
    );
    assert!(
        assembly.contains(&format!("movq ${largura}, %rsi")),
        "{rotulo}: largura {largura} não chegou ao validador\n{assembly}"
    );
    assert!(
        assembly.contains(&format!("movq ${largura}, %rdx")),
        "{rotulo}: alinhamento {largura} não chegou ao validador\n{assembly}"
    );
}

// @pinker-nav:start evidencia.hotfix.v4-ponteiro-fabricado
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência do item V4: a emissão nativa chama pinker_publico_validar_acesso para load e store através de ponteiro fabricado por conversão de inteiro, nos dez tipos escalares endereçáveis e nas larguras 1/2/4/8; o ambiente de closure continua isento por ser domínio interno; e os exemplos de endereço fabricado, nulo e round-trip por inteiro falham deterministicamente com o mesmo exit nos dois back-ends, sem SIGSEGV.
#[test]
fn v4_emite_validacao_para_load_e_store_por_ponteiro_fabricado() {
    for (tipo, largura) in TIPOS {
        let store = render_backend_s_external_subset_nativo(&programa_store(tipo))
            .unwrap_or_else(|erro| panic!("assembly de store para {tipo}: {erro:?}"));
        exigir_validacao_emitida(&format!("store {tipo}"), &store, largura);

        let load = render_backend_s_external_subset_nativo(&programa_load(tipo))
            .unwrap_or_else(|erro| panic!("assembly de load para {tipo}: {erro:?}"));
        exigir_validacao_emitida(&format!("load {tipo}"), &load, largura);
    }
}

/// O ambiente de closure é domínio interno do runtime: não está no registro
/// público e validá-lo rejeitaria um acesso legítimo. A isenção é deliberada e
/// precisa continuar valendo depois da correção.
///
/// A prova é operacional, não textual: se o acesso ao ambiente passasse pelo
/// validador, o runtime não encontraria região pública registrada para aquele
/// endereço e o ELF terminaria com `E-RUNTIME-MEM-UNKNOWN-ACCESS`. Exigir que o
/// binário nativo continue terminando com sucesso e com o mesmo stdout do
/// interpretador é o que distingue a isenção preservada de uma isenção perdida.
#[test]
fn v4_preserva_isencao_do_ponteiro_interno_de_closure() {
    const EXEMPLO_CLOSURE: &str = "examples/fase246_closure_captura_ponteiro_valido.pink";

    let codigo = std::fs::read_to_string(EXEMPLO_CLOSURE).expect("exemplo de closure com ponteiro");
    let assembly =
        render_backend_s_external_subset_nativo(&codigo).expect("assembly do exemplo de closure");
    assert!(
        assembly.contains("call pinker_publico_alocar"),
        "o exemplo precisa exercitar memória pública\n{assembly}"
    );

    let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", EXEMPLO_CLOSURE])
        .output()
        .expect("interpretador");
    assert_eq!(
        interpretado.status.code(),
        Some(0),
        "closure com ponteiro público deveria continuar válida: {}",
        String::from_utf8_lossy(&interpretado.stderr)
    );

    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let executavel = compilar(EXEMPLO_CLOSURE, &runtime_lib);
    let nativo = Command::new(&executavel).output().expect("ELF nativo");
    assert_eq!(
        nativo.status.code(),
        Some(0),
        "o ambiente de closure não pode passar a ser validado como memória pública: {}",
        String::from_utf8_lossy(&nativo.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&nativo.stderr).contains("E-RUNTIME-MEM-UNKNOWN-ACCESS"),
        "isenção do domínio interno perdida"
    );
    assert_eq!(
        String::from_utf8_lossy(&interpretado.stdout),
        String::from_utf8_lossy(&nativo.stdout),
        "paridade de stdout do exemplo de closure"
    );
    let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
}

/// Limite residual medido, não presumido.
///
/// A análise classifica como `Unclassified` o ponteiro que chega por um caminho
/// que ela não percorre — por exemplo, carregado de memória. Esses acessos
/// continuam sem validação, exatamente como antes do hotfix. Tratá-los como
/// exigentes foi testado e **quebra** as closures das Fases 243/244: parte dos
/// ponteiros de ambiente chega por caminhos não classificados, e validá-los
/// contra o registro público rejeita acesso legítimo. Fechar essa classe exige
/// uma análise de domínio de verdade, com contrato próprio, não um ajuste do
/// predicado.
///
/// Este teste guarda a fronteira: os exemplos de closure precisam continuar
/// executando nos dois back-ends, e é isso que impede que a classe
/// `Unclassified` seja "fechada" por engano numa mudança futura.
#[test]
fn v4_documenta_limite_da_classe_nao_classificada() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    for exemplo in [
        "examples/fase243_closure_captura_imutavel_valido.pink",
        "examples/fase246_closure_captura_ponteiro_valido.pink",
    ] {
        if !std::path::Path::new(exemplo).exists() {
            continue;
        }
        let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", exemplo])
            .output()
            .expect("interpretador");
        let executavel = compilar(exemplo, &runtime_lib);
        let nativo = Command::new(&executavel).output().expect("ELF nativo");
        // O exit code é o do próprio programa; o que se exige é paridade e
        // ausência de diagnóstico de memória pública.
        assert_eq!(
            nativo.status.code(),
            interpretado.status.code(),
            "{exemplo}: closure divergiu entre back-ends: {}",
            String::from_utf8_lossy(&nativo.stderr)
        );
        assert!(
            !String::from_utf8_lossy(&nativo.stderr).contains("E-RUNTIME-MEM"),
            "{exemplo}: closure passou a ser barrada pelo validador público: {}",
            String::from_utf8_lossy(&nativo.stderr)
        );
        let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
    }
}

fn compilar(exemplo: &str, runtime_lib: &std::path::Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_hf412_v4_{nanos}"));
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

#[test]
fn v4_endereco_fabricado_falha_igual_nos_dois_back_ends() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    for (exemplo, diagnostico_nativo) in [
        (
            "examples/hotfix_v4_endereco_fabricado_store_invalido.pink",
            "E-RUNTIME-MEM-UNKNOWN-ACCESS",
        ),
        (
            "examples/hotfix_v4_endereco_fabricado_load_invalido.pink",
            "E-RUNTIME-MEM-UNKNOWN-ACCESS",
        ),
        (
            "examples/hotfix_v4_endereco_nulo_store_invalido.pink",
            "E-RUNTIME-MEM-UNKNOWN-ACCESS",
        ),
        (
            "examples/hotfix_v4_endereco_nulo_load_invalido.pink",
            "E-RUNTIME-MEM-UNKNOWN-ACCESS",
        ),
        (
            "examples/hotfix_v4_fabricado_indireto_invalido.pink",
            "E-RUNTIME-MEM-UNKNOWN-DERIVATION",
        ),
    ] {
        let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", exemplo])
            .output()
            .expect("interpretador");
        let executavel = compilar(exemplo, &runtime_lib);
        let nativo = Command::new(&executavel).output().expect("ELF nativo");

        assert_eq!(
            interpretado.status.code(),
            Some(1),
            "{exemplo}: interpretador deveria diagnosticar"
        );
        assert_eq!(
            nativo.status.code(),
            Some(1),
            "{exemplo}: nativo deveria diagnosticar em vez de morrer por sinal (stderr: {})",
            String::from_utf8_lossy(&nativo.stderr)
        );
        assert!(
            String::from_utf8_lossy(&nativo.stderr).contains(diagnostico_nativo),
            "{exemplo}: diagnóstico nativo inesperado: {}",
            String::from_utf8_lossy(&nativo.stderr)
        );
        assert!(
            String::from_utf8_lossy(&interpretado.stderr).contains("endereço inválido"),
            "{exemplo}: diagnóstico interpretado inesperado: {}",
            String::from_utf8_lossy(&interpretado.stderr)
        );
        let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
    }
}

/// Antes do hotfix, o mesmo programa terminava por SIGSEGV. A garantia mínima
/// é: nenhum programa Pinker pode derrubar o processo nativo por sinal de
/// memória.
#[test]
fn v4_nenhum_endereco_fabricado_termina_por_sinal() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    for exemplo in [
        "examples/hotfix_v4_endereco_fabricado_store_invalido.pink",
        "examples/hotfix_v4_endereco_fabricado_load_invalido.pink",
        "examples/hotfix_v4_endereco_nulo_store_invalido.pink",
        "examples/hotfix_v4_endereco_nulo_load_invalido.pink",
        "examples/hotfix_v4_fabricado_indireto_invalido.pink",
        // Exemplo histórico da Fase 71: fabrica um ponteiro a partir de um
        // inteiro pequeno. No interpretador o endereço coincide com uma global
        // e o programa é válido; no nativo não existe esse espaço sintético.
        // O contrato aqui é apenas o mínimo: falhar sem sinal.
        "examples/fase71_cast_memoria_valido.pink",
    ] {
        let executavel = compilar(exemplo, &runtime_lib);
        let nativo = Command::new(&executavel).output().expect("ELF nativo");
        assert!(
            nativo.status.code().is_some(),
            "{exemplo}: nativo terminou por sinal (SIGSEGV/SIGBUS não podem escapar)"
        );
        let _ = std::fs::remove_dir_all(executavel.parent().expect("diretório do build"));
    }
}
// @pinker-nav:end evidencia.hotfix.v4-ponteiro-fabricado
