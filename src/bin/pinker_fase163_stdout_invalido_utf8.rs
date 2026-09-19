// @pinker-nav:start evidence.processes.fixture-invalid-stdout
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that emits on stdout a byte sequence that is not valid UTF-8, anchoring the case in which capture must refuse or classify the output instead of assuming text.
use std::io::Write;

fn main() {
    let mut stdout = std::io::stdout();
    stdout.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stdout.flush().unwrap();
}
// @pinker-nav:end evidence.processes.fixture-invalid-stdout
