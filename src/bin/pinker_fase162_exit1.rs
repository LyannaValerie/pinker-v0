// @pinker-nav:start evidence.processes.fixture-exit-one
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that terminates immediately with exit code 1, anchoring the observable failure case of the native execution matrix.
fn main() {
    std::process::exit(1);
}
// @pinker-nav:end evidence.processes.fixture-exit-one
