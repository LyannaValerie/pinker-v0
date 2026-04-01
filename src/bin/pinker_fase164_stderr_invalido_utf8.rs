use std::io::Write;

fn main() {
    let mut stderr = std::io::stderr();
    stderr.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stderr.flush().unwrap();
}
