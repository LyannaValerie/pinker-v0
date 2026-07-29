use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn fixture_invalida_nao_pode_ser_aceita_por_todos_os_modos_relevantes() {
    let tracked = Command::new("git")
        .args(["ls-files", "-z", "--", "examples/*_invalido*.pink"])
        .output()
        .expect("inventariar fixtures pelo Git");
    assert!(tracked.status.success());

    let pink = env!("CARGO_BIN_EXE_pink");
    for raw in tracked
        .stdout
        .split(|byte| *byte == 0)
        .filter(|p| !p.is_empty())
    {
        let path = std::str::from_utf8(raw).expect("caminho de fixture UTF-8");
        if !Command::new(pink)
            .args(["--check", path])
            .status()
            .expect("pink --check")
            .success()
        {
            continue;
        }
        if !Command::new(pink)
            .args(["--run", path])
            .status()
            .expect("pink --run")
            .success()
        {
            continue;
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("relógio")
            .as_nanos();
        let out_dir = std::env::temp_dir().join(format!(
            "pinker_obsolete_negative_{}_{}",
            std::process::id(),
            nonce
        ));
        let build = Command::new(pink)
            .args(["build", "--nativo", "--out-dir"])
            .arg(&out_dir)
            .arg(path)
            .status()
            .expect("pink build --nativo");
        if !build.success() {
            let _ = fs::remove_dir_all(&out_dir);
            continue;
        }
        let executable = out_dir.join(
            std::path::Path::new(path)
                .file_stem()
                .expect("stem da fixture"),
        );
        let native = Command::new(&executable)
            .status()
            .expect("executar fixture nativa");
        let _ = fs::remove_dir_all(&out_dir);
        assert!(
            !native.success(),
            "fixture negativa aceita por check, interpretador e nativo sem anotação explícita: {path}"
        );
    }
}

#[test]
fn fixtures_historicamente_obsoletas_foram_renomeadas_como_regressoes_validas() {
    for phase in 129..=132 {
        let old = format!("examples/fase{phase}_ninho_heterogeneo_camada");
        assert!(
            !fs::read_dir("examples")
                .expect("listar exemplos")
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .any(|name| name.starts_with(&old["examples/".len()..])
                    && name.ends_with("_invalido.pink")),
            "fixture obsoleta da Fase {phase} ainda usa sufixo negativo"
        );
    }
}
