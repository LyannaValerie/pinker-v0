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
