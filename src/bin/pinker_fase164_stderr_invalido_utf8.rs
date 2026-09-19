// @pinker-nav:start evidence.processes.fixture-invalid-stderr
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Controlled-process fixture that emits on stderr a byte sequence that is not valid UTF-8, anchoring the case in which capturing the error channel must refuse or classify the output instead of assuming text.
use std::io::Write;

fn main() {
    let mut stderr = std::io::stderr();
    stderr.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stderr.flush().unwrap();
}
// @pinker-nav:end evidence.processes.fixture-invalid-stderr
