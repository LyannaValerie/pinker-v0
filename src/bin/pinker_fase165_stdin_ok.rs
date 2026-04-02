use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    match (args.next().as_deref(), args.next()) {
        (None, None) => {
            if input == "rosa\\n" || input == "linha=ok\\nvalor=7\\n" {
                ExitCode::from(0)
            } else {
                ExitCode::from(1)
            }
        }
        (Some("--modo=ok"), None) => {
            if input == "argv=ok\\n" || input == "linha=argv\\nvalor=177\\n" {
                ExitCode::from(0)
            } else {
                ExitCode::from(1)
            }
        }
        (Some(_), None) => ExitCode::from(2),
        _ => ExitCode::from(3),
    }
}
