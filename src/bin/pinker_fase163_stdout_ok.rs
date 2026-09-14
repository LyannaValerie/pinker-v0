// @pinker-nav:start evidencia.processos.fixture-stdout-valido
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que emite stdout UTF-8 valido, variando o conteudo conforme o primeiro argumento para que a evidencia distinga captura de saida de repasse de argumento.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--alvo=rosa") => print!("status=ok\nalvo=rosa\n"),
        _ => print!("status=ok\nvalor=7\n"),
    }
}
// @pinker-nav:end evidencia.processos.fixture-stdout-valido
