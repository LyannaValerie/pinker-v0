// @pinker-nav:start evidencia.processos.fixture-pipe-produtor
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que produz em stdout uma linha fixa, servindo de produtor determinista na ponta de escrita de um pipe entre processos.
fn main() {
    print!("linha=ok\\nvalor=7\\n");
}
// @pinker-nav:end evidencia.processos.fixture-pipe-produtor
