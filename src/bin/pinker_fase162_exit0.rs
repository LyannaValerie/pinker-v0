// @pinker-nav:start evidence.processes.fixture-exit-zero
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that terminates immediately with exit code 0, anchoring the observable success case of the native execution matrix.
fn main() {
    std::process::exit(0);
}
// @pinker-nav:end evidence.processes.fixture-exit-zero
