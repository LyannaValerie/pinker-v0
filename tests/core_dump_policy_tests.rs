use std::process::Command;

#[repr(C)]
struct RLimit {
    current: u64,
    maximum: u64,
}

fn core_limit() -> RLimit {
    extern "C" {
        fn getrlimit(resource: i32, limit: *mut RLimit) -> i32;
    }
    const RLIMIT_CORE: i32 = 4;
    let mut limit = RLimit {
        current: 0,
        maximum: 0,
    };
    assert_eq!(unsafe { getrlimit(RLIMIT_CORE, &mut limit) }, 0);
    limit
}

#[test]
#[ignore = "ponto de reexecução para provar herança da esteira"]
fn core_probe_entry() {
    if std::env::var_os("PINKER_CORE_PROBE").is_none() {
        return;
    }
    let limit = core_limit();
    println!("soft={} hard={}", limit.current, limit.maximum);
}

#[test]
fn ci_env_core_zero_cobre_binario_rust_e_preserva_hard_limit() {
    let parent = core_limit();
    let output = Command::new("./ci_env.sh")
        .arg(std::env::current_exe().expect("test binary"))
        .args(["--exact", "core_probe_entry", "--ignored", "--nocapture"])
        .env("PINKER_CORE_PROBE", "1")
        .output()
        .expect("reexecuta binário Rust pela esteira");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("soft=0"), "{stdout}");
    assert!(
        stdout.contains(&format!("hard={}", parent.maximum)),
        "hard limit do operador mudou: {stdout}"
    );
}

#[test]
fn ci_env_core_zero_cobre_compilador_e_ferramenta_externa() {
    let external = Command::new("./ci_env.sh")
        .args([
            "sh",
            "-c",
            "printf 'external_soft=%s external_hard=%s' \"$(ulimit -Sc)\" \"$(ulimit -Hc)\"",
        ])
        .output()
        .expect("ferramenta externa");
    assert!(external.status.success(), "{external:?}");
    assert!(
        String::from_utf8_lossy(&external.stdout).contains("external_soft=0"),
        "{external:?}"
    );

    let compiler = Command::new("./ci_env.sh")
        .args([
            "sh",
            "-c",
            "printf 'compiler_soft=%s\\n' \"$(ulimit -Sc)\"; exec \"$1\" --help",
            "sh",
            env!("CARGO_BIN_EXE_pink"),
        ])
        .output()
        .expect("compilador pela esteira");
    assert!(compiler.status.success(), "{compiler:?}");
    assert!(
        String::from_utf8_lossy(&compiler.stdout).contains("compiler_soft=0"),
        "{compiler:?}"
    );
}

#[test]
fn politica_nao_altera_host_nem_remove_historico() {
    let script = std::fs::read_to_string("ci_env.sh").expect("ci_env");
    assert!(script.contains("ulimit -S -c 0"));
    assert!(!script.contains("core_pattern"));
    assert!(!script.contains("coredumpctl remove"));
    assert!(!script.contains("rm "));
    let runtime = std::fs::read_to_string("runtime/pinker_rt/src/lib.rs").expect("runtime Pinker");
    assert!(runtime.contains("RLIMIT_CORE"));
    assert!(runtime.contains("pinker_rt_iniciar"));
}
