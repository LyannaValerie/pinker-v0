// @pinker-nav:start evidencia.processos.fixture-stdin-valido
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que le stdin ate o fim e devolve por codigo de saida se o conteudo recebido corresponde ao esperado no modo pedido, tornando a entrega de entrada observavel sem depender de stdout.
use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    match (args.next().as_deref(), args.next()) {
        (None, None) => {
            if input == "rosa\\n"
                || input == "rosa\n"
                || input == "linha=ok\\nvalor=7\\n"
                || input == "linha=ok\nvalor=7\n"
            {
                ExitCode::from(0)
            } else {
                ExitCode::from(1)
            }
        }
        (Some("--modo=ok"), None) => {
            if input == "argv=ok\\n"
                || input == "argv=ok\n"
                || input == "linha=argv\\nvalor=177\\n"
                || input == "linha=argv\nvalor=177\n"
            {
                ExitCode::from(0)
            } else {
                ExitCode::from(1)
            }
        }
        (Some(_), None) => ExitCode::from(2),
        _ => ExitCode::from(3),
    }
}
// @pinker-nav:end evidencia.processos.fixture-stdin-valido
