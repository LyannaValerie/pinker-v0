use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn source_path(value: u64, explicit_output: bool) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pinker-principal-exit-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).expect("diretório temporário");
    let path = root.join("principal.pink");
    let output = if explicit_output { "falar(42);" } else { "" };
    fs::write(
        &path,
        format!("pacote main;\ncarinho principal() -> bombom {{ {output} mimo {value}; }}\n"),
    )
    .expect("fonte temporária");
    path
}

#[test]
fn principal_define_exit_sem_imprimir_retorno() {
    for (value, expected_exit) in [(0, 0), (1, 1), (10, 10), (255, 255), (256, 0), (257, 1)] {
        let source = source_path(value, false);
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run"])
            .arg(&source)
            .output()
            .expect("processo interpretado");
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "retorno {value}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "retorno de principal não integra stdout para {value}: {:?}",
            output.stdout
        );
        fs::remove_dir_all(source.parent().unwrap()).expect("limpeza");
    }
}

#[test]
fn apenas_saida_explicita_permanece_em_stdout() {
    let source = source_path(10, true);
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run"])
        .arg(&source)
        .output()
        .expect("processo interpretado");
    assert_eq!(output.status.code(), Some(10));
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(source.parent().unwrap()).expect("limpeza");
}
