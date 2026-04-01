fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--alvo=rosa") => print!("status=ok\nalvo=rosa\n"),
        _ => print!("status=ok\nvalor=7\n"),
    }
}
