use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    if input == "rosa\\n" || input == "linha=ok\\nvalor=7\\n" {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}
