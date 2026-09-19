// @pinker-nav:start evidence.processes.fixture-valid-stderr
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that emits valid UTF-8 stderr, varying the content according to the first argument so that the evidence separates the error channel from the output channel.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--alvo=rosa") => {
            eprintln!("erro=sim");
            eprintln!("alvo=rosa");
        }
        _ => {
            eprintln!("erro=sim");
            eprintln!("codigo=9");
        }
    }
}
// @pinker-nav:end evidence.processes.fixture-valid-stderr
