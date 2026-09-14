// @pinker-nav:start evidencia.processos.fixture-argv-unico
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixture de processo controlado que classifica por codigo de saida o unico argumento recebido, tornando a entrega de argv observavel sem depender de stdout, stderr ou stdin.
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("--modo=ok"), None) => ExitCode::from(0),
        (Some("--modo=falha"), None) => ExitCode::from(1),
        (Some("--alvo=rosa"), None) => ExitCode::from(0),
        (Some(_), None) => ExitCode::from(2),
        _ => ExitCode::from(3),
    }
}
// @pinker-nav:end evidencia.processos.fixture-argv-unico
