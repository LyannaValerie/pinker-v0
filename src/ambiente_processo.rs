//! Contrato de validação e aplicação do overlay de ambiente de processos.

// @pinker-nav:start processes.environment.overlay
// @pinker-nav:domain processes
// @pinker-nav:layer runtime
// @pinker-nav:summary Authority for validating and applying the environment overlay for structured processes: an empty key, an equals sign in the key and a NUL in the key or the value are invalid; an equals sign in the value is preserved in full and the pairs stay separated all the way to Command::env, with no textual serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAmbienteProcesso {
    ChaveVazia,
    IgualNaChave,
    NulNaChave,
    NulNoValor,
}

pub fn validar_entrada(chave: &str, valor: &str) -> Result<(), ErroAmbienteProcesso> {
    if chave.is_empty() {
        return Err(ErroAmbienteProcesso::ChaveVazia);
    }
    if chave.contains('=') {
        return Err(ErroAmbienteProcesso::IgualNaChave);
    }
    if chave.contains('\0') {
        return Err(ErroAmbienteProcesso::NulNaChave);
    }
    if valor.contains('\0') {
        return Err(ErroAmbienteProcesso::NulNoValor);
    }
    // '=' no valor é dado, não separador: Command::env recebe chave e valor
    // separadamente e nunca faz split adicional.
    Ok(())
}

pub fn aplicar_overlay<'a, I>(
    comando: &mut std::process::Command,
    entradas: I,
) -> Result<(), ErroAmbienteProcesso>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    for (chave, valor) in entradas {
        validar_entrada(chave, valor)?;
        comando.env(chave, valor);
    }
    Ok(())
}

// @pinker-nav:end processes.environment.overlay

// @pinker-nav:start evidence.processes.environment-overlay
// @pinker-nav:domain processes
// @pinker-nav:layer evidence
// @pinker-nav:summary Proves the closed contract of the environment overlay, including the mandatory PINKER_TEST case with the value a=b=c observed exactly by the child, without treating the additional equals signs as separators.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrato_distingue_chave_de_valor() {
        assert_eq!(
            validar_entrada("", "x"),
            Err(ErroAmbienteProcesso::ChaveVazia)
        );
        assert_eq!(
            validar_entrada("A=B", "x"),
            Err(ErroAmbienteProcesso::IgualNaChave)
        );
        assert_eq!(
            validar_entrada("A\0B", "x"),
            Err(ErroAmbienteProcesso::NulNaChave)
        );
        assert_eq!(
            validar_entrada("A", "x\0y"),
            Err(ErroAmbienteProcesso::NulNoValor)
        );
        assert_eq!(validar_entrada("PINKER_TEST", "a=b=c"), Ok(()));
    }

    #[test]
    fn filho_observa_igual_no_valor_sem_split_adicional() {
        let mut comando = std::process::Command::new("/usr/bin/env");
        comando.env_clear();
        aplicar_overlay(&mut comando, [("PINKER_TEST", "a=b=c")]).unwrap();
        let output = comando.output().expect("executar filho /usr/bin/env");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "PINKER_TEST=a=b=c\n"
        );
    }
}
// @pinker-nav:end evidence.processes.environment-overlay
