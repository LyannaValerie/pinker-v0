// @pinker-nav:start evidencia.processos.fixture-stderr-valido
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que emite stderr UTF-8 valido, variando o conteudo conforme o primeiro argumento para que a evidencia separe o canal de erro do canal de saida.
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
// @pinker-nav:end evidencia.processos.fixture-stderr-valido
