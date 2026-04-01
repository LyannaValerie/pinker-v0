use std::io::Write;

fn main() {
    let mut stdout = std::io::stdout();
    stdout.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stdout.flush().unwrap();
}
