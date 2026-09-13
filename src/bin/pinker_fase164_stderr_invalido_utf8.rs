// @pinker-nav:start evidencia.processos.fixture-stderr-invalido
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que emite em stderr uma sequencia de bytes que nao e UTF-8 valido, ancorando o caso em que a captura do canal de erro precisa recusar ou classificar a saida em vez de assumir texto.
use std::io::Write;

fn main() {
    let mut stderr = std::io::stderr();
    stderr.write_all(&[0x66, 0x6f, 0x80]).unwrap();
    stderr.flush().unwrap();
}
// @pinker-nav:end evidencia.processos.fixture-stderr-invalido
