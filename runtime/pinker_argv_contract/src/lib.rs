//! Contrato puro da leitura de argumentos nomeados da Pinker.
//!
//! Este crate existe pela mesma razão única e verificável do
//! `pinker_json_contract` e do `pinker_sha256_contract`: **uma** implementação.
//!
//! O interpretador vive no crate do compilador e o runtime nativo vive em
//! `pinker_rt`, que não pode depender do compilador. Enquanto a leitura de
//! `argv` foi escrita duas vezes, as duas cópias divergiram em silêncio: a
//! réplica nativa comprimiu `PresenteSemValor` e `Ausente` num único `None` e
//! passou a inventar o padrão para uma chave que o usuário escreveu.
//!
//! ```text
//! ONE_ARGV_GRAMMAR -> ONE_IMPLEMENTATION -> PARITY_BY_CONSTRUCTION
//! ```
//!
//! Aqui não há nome público da linguagem, tipo do compilador, ABI nem I/O: só
//! a gramática de `argv`, os três estados que ela distingue e a resolução de
//! cada superfície. Os nomes públicos e as assinaturas continuam em
//! `semantic`; **como** cada backend falha continua sendo de cada backend.

// @pinker-nav:start argv.contrato.estado-da-chave
// @pinker-nav:domain ambiente
// @pinker-nav:layer contrato
// @pinker-nav:summary Autoridade única da leitura de argumentos nomeados (#492): `EstadoChave` distingue os três estados que a gramática de `argv` produz — chave ausente, chave presente sem valor e chave presente com valor, incluindo valor vazio —, `estado_da_chave` é o único classificador (primeira ocorrência vence; a forma separada consome o próximo token qualquer que ele seja; a forma `chave=` entrega o sufixo), `resolver_pedido` e `resolver_contexto` dizem o que cada superfície faz com cada estado, e `contem_token_exato` responde a pergunta diferente do `tem_flag`. Compartilhado pelo compilador e pelo runtime nativo para que os dois backends derivem o mesmo contrato por construção; não contém I/O, nome público da linguagem nem ABI.

/// O que `argv` diz sobre uma chave nomeada.
///
/// Os três estados não são um refinamento decorativo: as três formas existem
/// de fato na gramática, são produzidas por entradas diferentes e exigem
/// respostas diferentes. Comprimir `Ausente` e `PresenteSemValor` num
/// `Option` faz o backend responder "o usuário não pediu nada" quando o
/// usuário pediu e esqueceu o valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoChave<'a> {
    /// A chave não aparece em `argv` em nenhuma das formas suportadas.
    Ausente,
    /// A chave aparece como token exato e não há token seguinte para ser o
    /// valor dela.
    PresenteSemValor,
    /// A chave aparece com valor. `""` é valor: `--chave=` é um valor vazio
    /// explícito, não uma ausência.
    PresenteComValor(&'a str),
}

/// Classifica `chave` contra `argumentos`. **O** classificador.
///
/// A gramática suportada, e nada além dela:
///
/// ```text
/// chave valor    forma separada: o token seguinte é o valor, qualquer que ele seja
/// chave=valor    forma com igual: o sufixo é o valor, inclusive vazio
/// ```
///
/// A forma separada não para no próximo `--outra`: a Pinker não implementa a
/// convenção GNU de tratar um token com prefixo de opção como fim do valor.
/// A primeira ocorrência decide; ocorrências posteriores não são lidas.
pub fn estado_da_chave<'a, S: AsRef<str>>(argumentos: &'a [S], chave: &str) -> EstadoChave<'a> {
    let chave_igual = format!("{chave}=");
    for (indice, argumento) in argumentos.iter().enumerate() {
        let argumento = argumento.as_ref();
        if argumento == chave {
            return match argumentos.get(indice + 1) {
                Some(valor) => EstadoChave::PresenteComValor(valor.as_ref()),
                None => EstadoChave::PresenteSemValor,
            };
        }
        if let Some(valor) = argumento.strip_prefix(&chave_igual) {
            return EstadoChave::PresenteComValor(valor);
        }
    }
    EstadoChave::Ausente
}

/// A chave está presente **com valor**?
///
/// É a pergunta de `tem_chave`. `--chave` sozinha responde `false`: a chave
/// está lá, o valor não.
pub fn chave_tem_valor<S: AsRef<str>>(argumentos: &[S], chave: &str) -> bool {
    matches!(
        estado_da_chave(argumentos, chave),
        EstadoChave::PresenteComValor(_)
    )
}

/// O token exato aparece em `argv`?
///
/// É a pergunta de `tem_flag`, e é **outra** pergunta. `--chave=valor` não a
/// responde, porque o token exato não está lá; `--chave valor` responde, porque
/// está. Vive aqui, com nome próprio, para que a diferença entre "é uma flag" e
/// "carrega um valor" seja estrutural em vez de lembrada.
pub fn contem_token_exato<S: AsRef<str>>(argumentos: &[S], chave: &str) -> bool {
    argumentos
        .iter()
        .any(|argumento| argumento.as_ref() == chave)
}

/// O que `pedir_argumento` faz com cada estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pedido<'a> {
    /// Devolve o valor lido de `argv`.
    Valor(&'a str),
    /// Devolve o padrão recebido pela intrínseca.
    Padrao,
    /// Falha: a chave foi escrita e o valor não.
    ChaveSemValor,
}

/// O que `buscar_contexto` faz com cada estado.
///
/// O ambiente é fallback de `Ausente`, e só dele. `PresenteSemValor` bloqueia o
/// fallback inteiro — ambiente e padrão — porque uma chave escrita sem valor é
/// erro do usuário, não silêncio do usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contexto<'a> {
    /// Devolve o valor lido de `argv`; o ambiente não é consultado.
    Valor(&'a str),
    /// Consulta o ambiente e, se ele não responder, devolve o padrão.
    Ambiente,
    /// Falha: a chave foi escrita e o valor não.
    ChaveSemValor,
}

/// Resolução de `pedir_argumento` a partir do estado da chave.
pub fn resolver_pedido(estado: EstadoChave<'_>) -> Pedido<'_> {
    match estado {
        EstadoChave::PresenteComValor(valor) => Pedido::Valor(valor),
        EstadoChave::Ausente => Pedido::Padrao,
        EstadoChave::PresenteSemValor => Pedido::ChaveSemValor,
    }
}

/// Resolução de `buscar_contexto` a partir do estado da chave de argumento.
pub fn resolver_contexto(estado: EstadoChave<'_>) -> Contexto<'_> {
    match estado {
        EstadoChave::PresenteComValor(valor) => Contexto::Valor(valor),
        EstadoChave::Ausente => Contexto::Ambiente,
        EstadoChave::PresenteSemValor => Contexto::ChaveSemValor,
    }
}

/// Diagnóstico de chave escrita sem valor.
///
/// O texto mora aqui para que os dois backends não possam divergir na
/// mensagem. O envelope — `Erro Runtime:` com stack trace no interpretador,
/// `Erro de Execução (pinker_rt):` no nativo — continua sendo de cada host.
pub fn mensagem_chave_sem_valor(intrinseca: &str, chave: &str) -> String {
    format!(
        "intrínseca '{intrinseca}' encontrou chave '{chave}' sem valor na forma '--chave valor'"
    )
}

/// Diagnóstico de chave de argumento vazia.
pub fn mensagem_chave_vazia(intrinseca: &str) -> String {
    format!("intrínseca '{intrinseca}' exige chave não vazia")
}

/// Diagnóstico de chave de ambiente vazia.
///
/// É deliberadamente distinto de [`mensagem_chave_vazia`]: `buscar_contexto`
/// recebe duas chaves e o diagnóstico precisa dizer qual delas está vazia.
pub fn mensagem_chave_ambiente_vazia(intrinseca: &str) -> String {
    format!("intrínseca '{intrinseca}' exige chave de ambiente não vazia")
}
// @pinker-nav:end argv.contrato.estado-da-chave

#[cfg(test)]
mod testes {
    use super::*;

    fn argv(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn os_tres_estados_sao_distinguiveis() {
        assert_eq!(estado_da_chave(&argv(&[]), "--c"), EstadoChave::Ausente);
        assert_eq!(
            estado_da_chave(&argv(&["--c"]), "--c"),
            EstadoChave::PresenteSemValor
        );
        assert_eq!(
            estado_da_chave(&argv(&["--c", "v"]), "--c"),
            EstadoChave::PresenteComValor("v")
        );
        // Os três são diferentes entre si: é o que a compressão em `Option`
        // perdia.
        assert_ne!(
            estado_da_chave(&argv(&[]), "--c"),
            estado_da_chave(&argv(&["--c"]), "--c")
        );
    }

    #[test]
    fn valor_vazio_explicito_e_valor_e_nao_ausencia() {
        assert_eq!(
            estado_da_chave(&argv(&["--c="]), "--c"),
            EstadoChave::PresenteComValor("")
        );
        assert_ne!(
            estado_da_chave(&argv(&["--c="]), "--c"),
            EstadoChave::PresenteSemValor
        );
    }

    #[test]
    fn a_forma_separada_consome_o_proximo_token_qualquer_que_ele_seja() {
        assert_eq!(
            estado_da_chave(&argv(&["--c", "--outra"]), "--c"),
            EstadoChave::PresenteComValor("--outra")
        );
    }

    #[test]
    fn a_primeira_ocorrencia_vence_nas_tres_misturas() {
        assert_eq!(
            estado_da_chave(&argv(&["--c", "um", "--c", "dois"]), "--c"),
            EstadoChave::PresenteComValor("um")
        );
        assert_eq!(
            estado_da_chave(&argv(&["--c=um", "--c=dois"]), "--c"),
            EstadoChave::PresenteComValor("um")
        );
        assert_eq!(
            estado_da_chave(&argv(&["--c", "um", "--c=dois"]), "--c"),
            EstadoChave::PresenteComValor("um")
        );
    }

    #[test]
    fn a_chave_so_e_encontrada_nas_formas_suportadas() {
        // Prefixo não é chave, e sufixo também não.
        assert_eq!(
            estado_da_chave(&argv(&["--cd", "v"]), "--c"),
            EstadoChave::Ausente
        );
        assert_eq!(
            estado_da_chave(&argv(&["x--c=v"]), "--c"),
            EstadoChave::Ausente
        );
    }

    #[test]
    fn as_duas_perguntas_de_presenca_sao_diferentes() {
        let com_igual = argv(&["--c=v"]);
        assert!(chave_tem_valor(&com_igual, "--c"));
        assert!(!contem_token_exato(&com_igual, "--c"));

        let sem_valor = argv(&["--c"]);
        assert!(!chave_tem_valor(&sem_valor, "--c"));
        assert!(contem_token_exato(&sem_valor, "--c"));
    }

    #[test]
    fn a_resolucao_de_cada_superficie_segue_o_estado() {
        assert_eq!(
            resolver_pedido(EstadoChave::PresenteComValor("v")),
            Pedido::Valor("v")
        );
        assert_eq!(resolver_pedido(EstadoChave::Ausente), Pedido::Padrao);
        assert_eq!(
            resolver_pedido(EstadoChave::PresenteSemValor),
            Pedido::ChaveSemValor
        );

        assert_eq!(
            resolver_contexto(EstadoChave::PresenteComValor("v")),
            Contexto::Valor("v")
        );
        // O ambiente é fallback de Ausente, e só dele.
        assert_eq!(resolver_contexto(EstadoChave::Ausente), Contexto::Ambiente);
        // Chave sem valor bloqueia o fallback inteiro: não vira Ambiente.
        assert_eq!(
            resolver_contexto(EstadoChave::PresenteSemValor),
            Contexto::ChaveSemValor
        );
        assert_ne!(
            resolver_contexto(EstadoChave::PresenteSemValor),
            Contexto::Ambiente
        );
    }

    #[test]
    fn os_diagnosticos_de_chave_vazia_sao_distinguiveis() {
        // `buscar_contexto` recebe duas chaves; as mensagens têm de dizer qual.
        assert_ne!(
            mensagem_chave_vazia("buscar_contexto"),
            mensagem_chave_ambiente_vazia("buscar_contexto")
        );
        assert!(mensagem_chave_ambiente_vazia("buscar_contexto").contains("chave de ambiente"));
        assert!(mensagem_chave_sem_valor("pedir_argumento", "--c").contains("--c"));
    }
}
