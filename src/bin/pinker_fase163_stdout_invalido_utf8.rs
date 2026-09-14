// @pinker-nav:start evidencia.processos.fixture-stdout-invalido
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que emite em stdout uma sequencia de bytes que nao e UTF-8 valido, ancorando o caso em que a captura precisa recusar ou classificar a saida em vez de assumir texto.
use std::io::Write;

fn main() {
    let mut stdout = std::io::stdout();
    stdout.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stdout.flush().unwrap();
}
// @pinker-nav:end evidencia.processos.fixture-stdout-invalido
