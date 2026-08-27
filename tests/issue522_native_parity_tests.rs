mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
struct Observacao {
    codigo: Option<i32>,
    stdout: String,
    falha: Option<String>,
}

fn nucleo_da_falha(stderr: &str) -> Option<String> {
    for linha in stderr.lines() {
        if let Some((_, nucleo)) = linha.split_once("[runtime::erro] ") {
            return Some(nucleo.trim().to_string());
        }
        if let Some(nucleo) = linha.strip_prefix("Erro de Execução (pinker_rt): ") {
            return Some(nucleo.trim().to_string());
        }
    }
    (!stderr.trim().is_empty()).then(|| format!("SEM_NUCLEO_RECONHECIVEL: {stderr}"))
}

fn observar(saida: &Output) -> Observacao {
    let stderr = String::from_utf8_lossy(&saida.stderr);
    assert!(!stderr.contains("panicked"), "pânico no backend: {stderr}");
    Observacao {
        codigo: saida.status.code(),
        stdout: String::from_utf8_lossy(&saida.stdout).into_owned(),
        falha: nucleo_da_falha(&stderr),
    }
}

fn escrever_fonte(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte #522");
    caminho
}

fn compilar_nativo(
    dir: &NativeArtifactDir,
    fonte: &Path,
    runtime_lib: &Path,
    caso: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(fonte)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(caso)
        .timeout(Duration::from_secs(120))
        .output()
        .expect("compilar fonte #522")
}

struct Sujeito {
    _dir: NativeArtifactDir,
    fonte: PathBuf,
    binario: PathBuf,
}

impl Sujeito {
    fn novo(nome: &str, fonte: &str, runtime_lib: &Path) -> Self {
        let dir = NativeArtifactDir::create().expect("diretório nativo #522");
        let fonte = escrever_fonte(&dir, nome, fonte);
        let compilacao = compilar_nativo(&dir, &fonte, runtime_lib, nome);
        assert!(
            compilacao.status.success(),
            "{nome}: build nativo falhou: {}",
            String::from_utf8_lossy(&compilacao.stderr)
        );
        let binario = dir.path().join(nome);
        Self {
            _dir: dir,
            fonte,
            binario,
        }
    }

    fn paridade(&self, caso: &str) -> Observacao {
        let interpretado = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--run")
            .arg(&self.fonte)
            .logical_case(&format!("{caso}-interpretado"))
            .timeout(Duration::from_secs(60))
            .output()
            .expect("executar interpretador #522");
        let nativo = Command::new(&self.binario)
            .logical_case(&format!("{caso}-nativo"))
            .timeout(Duration::from_secs(60))
            .output()
            .expect("executar ELF #522");
        let interpretado = observar(&interpretado);
        let nativo = observar(&nativo);
        assert_eq!(interpretado, nativo, "paridade divergente em {caso}");
        interpretado
    }
}

fn runtime_lib() -> Option<PathBuf> {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return None;
    };
    Some(runtime_lib)
}

#[test]
fn afirmar_preserva_sucesso_falha_mensagem_e_as_duas_aridades() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };

    let sucesso = Sujeito::novo(
        "issue522_afirmar_sucesso",
        r#"
pacote main; trazer assertiva.afirmar;
carinho principal() -> bombom {
    nova mensagem: verso = "preservada";
    afirmar(verdade);
    afirmar(verdade, mensagem);
    falar(mensagem);
    mimo 0;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        sucesso.paridade("afirmar-sucesso"),
        Observacao {
            codigo: Some(0),
            stdout: "preservada\n".to_string(),
            falha: None,
        }
    );

    let falha_com_mensagem = Sujeito::novo(
        "issue522_afirmar_falha_mensagem",
        r#"
pacote main; trazer assertiva.afirmar;
carinho principal() -> bombom {
    afirmar(falso, "contrato 522");
    mimo 99;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        falha_com_mensagem.paridade("afirmar-falha-mensagem"),
        Observacao {
            codigo: Some(1),
            stdout: String::new(),
            falha: Some("afirmação falhou: contrato 522".to_string()),
        }
    );

    let falha_sem_mensagem = Sujeito::novo(
        "issue522_afirmar_falha_simples",
        r#"
pacote main; trazer assertiva.afirmar;
carinho principal() -> bombom {
    afirmar(falso);
    mimo 99;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        falha_sem_mensagem.paridade("afirmar-falha-simples"),
        Observacao {
            codigo: Some(1),
            stdout: String::new(),
            falha: Some("afirmação falhou".to_string()),
        }
    );
}

#[test]
fn dormir_preserva_milisegundos_sem_tolerancia_fragil_de_relogio() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    let sujeito = Sujeito::novo(
        "issue522_dormir",
        r#"
pacote main;
carinho principal() -> bombom {
    falar("antes");
    dormir(0);
    dormir(1);
    falar("depois");
    mimo 0;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        sujeito.paridade("dormir-zero-pequeno"),
        Observacao {
            codigo: Some(0),
            stdout: "antes\ndepois\n".to_string(),
            falha: None,
        }
    );
}

#[test]
fn emitir_csv_preserva_vazio_um_multiplos_separador_e_u64() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    let sujeito = Sujeito::novo(
        "issue522_csv_emitir",
        r#"
pacote main;
carinho principal() -> bombom {
    nova vazia: lista<bombom> = lista_bombom_criar();
    falar(emitir_linha_csv_bombom(vazia, ","));

    nova uma: lista<bombom> = lista_bombom_criar();
    lista_bombom_anexar(uma, 7);
    falar(emitir_linha_csv_bombom(uma, ";"));

    lista_bombom_anexar(uma, 11);
    lista_bombom_anexar(uma, 18446744073709551615);
    falar(emitir_linha_csv_bombom(uma, ";"));
    mimo 0;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        sujeito.paridade("csv-emitir-validos"),
        Observacao {
            codigo: Some(0),
            stdout: "\n7\n7;11;18446744073709551615\n".to_string(),
            falha: None,
        }
    );

    let separador_vazio = Sujeito::novo(
        "issue522_csv_emitir_sep_vazio",
        r#"
pacote main; trazer csv.emitir_linha_bombom; trazer lista.bombom_criar;
carinho principal() -> bombom {
    nova itens: lista<bombom> = bombom_criar();
    falar(emitir_linha_bombom(itens, ""));
    mimo 0;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        separador_vazio.paridade("csv-emitir-separador-vazio").falha,
        Some("intrínseca 'emitir_linha_csv_bombom' não aceita separador vazio".to_string())
    );

    // Separador de mais de um caractere e separadores fora do recorte mínimo de
    // CSV. Cada caso confere o núcleo de falha nos dois motores.
    for (nome, literal, nucleo) in [
        (
            "multi",
            r#"", ""#,
            "intrínseca 'emitir_linha_csv_bombom' exige separador de 1 caractere",
        ),
        (
            "aspas",
            r#""\"""#,
            "intrínseca 'emitir_linha_csv_bombom' rejeita separador fora do recorte mínimo de CSV",
        ),
        (
            "newline",
            r#""\n""#,
            "intrínseca 'emitir_linha_csv_bombom' rejeita separador fora do recorte mínimo de CSV",
        ),
    ] {
        let fonte = format!(
            r#"
pacote main; trazer csv.emitir_linha_bombom; trazer lista.bombom_anexar; trazer lista.bombom_criar;
carinho principal() -> bombom {{
    nova itens: lista<bombom> = bombom_criar();
    bombom_anexar(itens, 1);
    falar(emitir_linha_bombom(itens, {literal}));
    mimo 0;
}}
"#
        );
        let sujeito = Sujeito::novo(
            &format!("issue522_csv_emitir_sep_{nome}"),
            &fonte,
            &runtime_lib,
        );
        assert_eq!(
            sujeito
                .paridade(&format!("csv-emitir-separador-{nome}"))
                .falha,
            Some(nucleo.to_string()),
            "separador {nome}"
        );
    }
}

#[test]
fn ler_csv_preserva_split_parse_falhas_e_lista_resultante() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    let valido = Sujeito::novo(
        "issue522_csv_ler_valido",
        r#"
pacote main; trazer csv.emitir_linha_bombom; trazer csv.ler_linha_bombom; trazer lista.bombom_obter; trazer lista.bombom_tamanho;
carinho principal() -> bombom {
    nova itens: lista<bombom> = ler_linha_bombom("7;11;13", ";");
    falar(bombom_tamanho(itens));
    falar(bombom_obter(itens, 0));
    falar(bombom_obter(itens, 1));
    falar(bombom_obter(itens, 2));
    falar(emitir_linha_bombom(itens, ";"));
    mimo 0;
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        valido.paridade("csv-ler-valido"),
        Observacao {
            codigo: Some(0),
            stdout: "3\n7\n11\n13\n7;11;13\n".to_string(),
            falha: None,
        }
    );

    for (nome, linha) in [
        ("vazio", ""),
        ("token_vazio", "1,,2"),
        ("nao_numerico", "1,x,2"),
    ] {
        let fonte = format!(
            r#"
pacote main; trazer csv.ler_linha_bombom; trazer lista.bombom_tamanho;
carinho principal() -> bombom {{
    nova itens: lista<bombom> = ler_linha_bombom("{linha}", ",");
    mimo bombom_tamanho(itens);
}}
"#
        );
        let sujeito = Sujeito::novo(&format!("issue522_csv_ler_{nome}"), &fonte, &runtime_lib);
        assert_eq!(
            sujeito.paridade(&format!("csv-ler-{nome}")).falha,
            Some(
                "campo inválido em 'ler_linha_csv_bombom': esperado bombom simples sem quoting"
                    .to_string()
            )
        );
    }

    let separador_vazio = Sujeito::novo(
        "issue522_csv_ler_sep_vazio",
        r#"
pacote main; trazer csv.ler_linha_bombom; trazer lista.bombom_tamanho;
carinho principal() -> bombom {
    nova itens: lista<bombom> = ler_linha_bombom("1", "");
    mimo bombom_tamanho(itens);
}
"#,
        &runtime_lib,
    );
    assert_eq!(
        separador_vazio.paridade("csv-ler-separador-vazio").falha,
        Some("intrínseca 'ler_linha_csv_bombom' não aceita separador vazio".to_string())
    );
}

#[test]
fn sair_preserva_status_clamp_e_inalcancabilidade() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    // `acima_de_u32` é o caso que discrimina o clamp: sem `min(codigo, i32::MAX)`
    // o valor truncaria para 7 e o processo sairia com 7; com o clamp sai com
    // i32::MAX, que o status de processo trunca para 255. O caso `maximo` sozinho
    // não separa as duas implementações, porque ambas terminariam em 255.
    for (nome, codigo, esperado) in [
        ("sete", "7", 7),
        ("maximo", "18446744073709551615", 255),
        ("acima_de_u32", "4294967303", 255),
    ] {
        let fonte = format!(
            r#"
pacote main; trazer processo.sair;
carinho principal() -> bombom {{
    sair({codigo});
    falar("inalcançável");
    mimo 99;
}}
"#
        );
        let sujeito = Sujeito::novo(&format!("issue522_sair_{nome}"), &fonte, &runtime_lib);
        assert_eq!(
            sujeito.paridade(&format!("sair-{nome}")),
            Observacao {
                codigo: Some(esperado),
                stdout: String::new(),
                falha: None,
            }
        );
    }
}

#[test]
fn stdin_interativo_permanece_fora_do_subset_nativo() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    for (nome, corpo) in [
        ("ouvir", "nova valor: bombom = ouvir(); falar(valor);"),
        ("ouvir_verso", "falar(ouvir_verso());"),
        ("ouvir_verso_ou", "falar(ouvir_verso_ou(\"padrao\"));"),
    ] {
        let dir = NativeArtifactDir::create().expect("diretório negativo stdin #522");
        let fonte = escrever_fonte(
            &dir,
            &format!("issue522_{nome}_continua_excluido"),
            &format!("pacote main; carinho principal() -> bombom {{ {corpo} mimo 0; }}"),
        );
        let build = compilar_nativo(&dir, &fonte, &runtime_lib, &format!("stdin-{nome}"));
        assert!(!build.status.success(), "{nome} ganhou suporte nativo");
        assert!(
            String::from_utf8_lossy(&build.stderr).contains("call para função inexistente"),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
    }
}

#[test]
fn callee_inexistente_e_homonimo_de_usuario_nao_sao_capturados() {
    let Some(runtime_lib) = runtime_lib() else {
        return;
    };
    let dir = NativeArtifactDir::create().expect("diretório callee inexistente #522");
    let inexistente = escrever_fonte(
        &dir,
        "issue522_callee_inexistente",
        "pacote main; carinho principal() -> bombom { mimo inexistente_522(); }",
    );
    let build = compilar_nativo(&dir, &inexistente, &runtime_lib, "callee-inexistente");
    assert!(!build.status.success());
    assert!(
        String::from_utf8_lossy(&build.stderr).contains("inexistente_522"),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let declaracao_homonima = escrever_fonte(
        &dir,
        "issue522_declaracao_homonima",
        r#"
pacote main;
carinho afirmar(valor: bombom) -> bombom { mimo valor + 1; }
carinho principal() -> bombom { mimo afirmar(41); }
"#,
    );
    let build = compilar_nativo(
        &dir,
        &declaracao_homonima,
        &runtime_lib,
        "declaracao-homonima",
    );
    assert!(!build.status.success());
    assert!(
        String::from_utf8_lossy(&build.stderr)
            .contains("declaração callable 'afirmar' pertence à superfície intrínseca Pinker"),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let usuario = Sujeito::novo(
        "issue522_funcao_usuario",
        r#"
pacote main;
carinho afirmar_usuario(valor: bombom) -> bombom { mimo valor + 1; }
carinho principal() -> bombom { mimo afirmar_usuario(41); }
"#,
        &runtime_lib,
    );
    assert_eq!(
        usuario.paridade("funcao-usuario"),
        Observacao {
            codigo: Some(42),
            stdout: String::new(),
            falha: None,
        }
    );
}

#[test]
fn autoridade_mapeia_cinco_gaps_e_preserva_as_tres_exclusoes() {
    let backend = include_str!("../src/backend_s.rs");
    let runtime = include_str!("../runtime/pinker_rt/src/lib.rs");

    for (intrinseca, simbolo) in [
        ("dormir", "pinker_dormir"),
        ("emitir_linha_csv_bombom", "pinker_emitir_linha_csv_bombom"),
        ("ler_linha_csv_bombom", "pinker_ler_linha_csv_bombom"),
        ("sair", "pinker_sair"),
    ] {
        assert!(
            backend.contains(&format!("\"{intrinseca}\" => Some(\"{simbolo}\")")),
            "mapeamento ausente: {intrinseca} -> {simbolo}"
        );
        assert!(
            runtime.contains(&format!("fn {simbolo}(")),
            "símbolo ausente no runtime: {simbolo}"
        );
    }
    assert!(backend.contains("(\"afirmar\", 1 | 2)"));
    for simbolo in ["pinker_afirmar_1", "pinker_afirmar_2"] {
        assert!(runtime.contains(&format!("fn {simbolo}(")));
    }
    for excluida in ["ouvir", "ouvir_verso", "ouvir_verso_ou"] {
        assert!(
            !backend.contains(&format!("\"{excluida}\" => Some(")),
            "{excluida} entrou no mapeamento nativo"
        );
    }
}
