// @pinker-nav:start evidence.processes.fixture-valid-stdout
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that emits valid UTF-8 stdout, varying the content according to the first argument so that the evidence distinguishes output capture from argument forwarding.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--alvo=rosa") => print!("status=ok\nalvo=rosa\n"),
        _ => print!("status=ok\nvalor=7\n"),
    }
}
// @pinker-nav:end evidence.processes.fixture-valid-stdout
