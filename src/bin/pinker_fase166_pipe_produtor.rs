// @pinker-nav:start evidence.processes.fixture-producer-pipe
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that produces a fixed line on stdout, serving as a deterministic producer at the write end of a pipe between processes.
fn main() {
    print!("linha=ok\\nvalor=7\\n");
}
// @pinker-nav:end evidence.processes.fixture-producer-pipe
