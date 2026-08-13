//! Autoridade única do limite de tempo de uma execução — Parte D.
//!
//! Um timeout precisa distinguir duas situações que um número sozinho não
//! separa:
//!
//! ```text
//! não há limite            -> a operação espera o filho terminar
//! o limite é zero          -> a operação expira imediatamente
//! ```
//!
//! A convenção `0 = sem limite` resolveria a primeira ao custo de tornar a
//! segunda irrepresentável. `0` é um valor legítimo do domínio — expiração
//! imediata —, então usá-lo como sentinela apaga um estado real. É diferente do
//! caminho vazio em `diretorio`, onde o valor excluído não é um cwd legítimo e
//! por isso não apaga nada.
//!
//! ```text
//! SENTINEL_JUSTIFIED  IFF  o valor sentinela NÃO é membro legítimo do domínio
//! ```
//!
//! Daí o leque. Ele custa zero maquinaria nova: `Ate(bombom)` é uma carga já
//! aceita por `CONTRATO_CARGAS`, e a viabilidade de um leque com carga como
//! parâmetro de função foi medida nos dois backends antes desta escolha.
//!
//! O que este módulo **não** faz:
//!
//! - não descreve política de terminação. Quem mata e quem reapa é da
//!   autoridade de execução;
//! - não cresce por analogia. Duas variantes são o mínimo que separa "sem
//!   limite" de "limitado"; não há aqui unidade alternativa, deadline absoluto
//!   nem política de retry.

// @pinker-nav:start processos.limite-tempo.taxonomia
// @pinker-nav:domain processos
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única do limite de tempo da Parte D: `LimiteTempo` separa `SemLimite` de `Ate(bombom)` em milissegundos, `VARIANTES` fixa a ordem de declaração — que é o discriminante lido pela IR — e `CARGA_ATE` nomeia a carga da variante limitada. A escolha do leque em vez de `0 = sem limite` existe para manter a expiração imediata representável: `0` é membro legítimo do domínio, então usá-lo como sentinela apagaria um estado real. O nome público e os nomes das variantes existem só aqui; o runtime nativo espelha os discriminantes e a paridade é fixada por evidência.

/// Nome público do leque predeclarado do limite de tempo.
pub const LEQUE_LIMITE_TEMPO: &str = "LimiteTempo";

/// Nome da variante sem limite. Sua posição de declaração é o discriminante 0.
pub const VARIANTE_SEM_LIMITE: &str = "SemLimite";

/// Nome da variante limitada. Sua posição de declaração é o discriminante 1.
pub const VARIANTE_ATE: &str = "Ate";

/// Unidade do limite, declarada uma única vez.
///
/// Milissegundo é a menor unidade que descreve um timeout operacional sem
/// exigir aritmética de ponto flutuante nem uma família de conversões.
pub const UNIDADE: &str = "milissegundos";

/// Limite de tempo de uma execução de processo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimiteTempo {
    /// A operação espera o filho terminar por conta própria.
    SemLimite,
    /// A operação expira depois de `ms` milissegundos.
    ///
    /// `Ate(0)` é expiração imediata, e é justamente o estado que a convenção
    /// `0 = sem limite` tornaria impossível de escrever.
    Ate(u64),
}

/// Variantes na ordem de declaração do leque.
///
/// A ordem **é** o discriminante: a IR usa o índice de declaração da variante.
/// Reordenar esta lista muda valores observáveis, e a evidência de paridade
/// quebra imediatamente se as duas pontas discordarem.
pub const VARIANTES: &[(&str, bool)] = &[
    // (nome, tem_carga)
    (VARIANTE_SEM_LIMITE, false),
    (VARIANTE_ATE, true),
];

/// Discriminante de [`LimiteTempo::SemLimite`].
pub const TAG_SEM_LIMITE: u64 = 0;

/// Discriminante de [`LimiteTempo::Ate`].
pub const TAG_ATE: u64 = 1;

/// Contrato explícito para SemLimite quando descendentes herdam os pipes.
///
/// O escopo de execução continua sendo o filho direto, porém captura completa
/// requer EOF dos dois pipes. Logo, depois que o filho direto termina, um
/// descendente que mantenha um write-end aberto pode manter a operação
/// aguardando indefinidamente. Isso é semântica deliberada, não efeito
/// acidental de uma futura implementação por poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoliticaPipeDescendenteSemLimite {
    pub unlimited_descendant_pipe_behavior: ComportamentoPipeDescendente,
    pub completion_condition: CondicaoConclusaoCaptura,
    pub captured_output_scope: EscopoSaidaCapturada,
    pub direct_child_scope_relation: RelacaoEscopoFilhoDireto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComportamentoPipeDescendente {
    PodeEsperarIndefinidamente,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondicaoConclusaoCaptura {
    FilhoDiretoReapadoEStdoutStderrEmEof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscopoSaidaCapturada {
    BytesRecebidosNosPipesHerdadosAteEof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelacaoEscopoFilhoDireto {
    ExecucaoSoDoFilhoDiretoCapturaPodeSerProlongadaPorDescendentes,
}

pub const POLITICA_PIPE_DESCENDENTE_SEM_LIMITE: PoliticaPipeDescendenteSemLimite =
    PoliticaPipeDescendenteSemLimite {
        unlimited_descendant_pipe_behavior:
            ComportamentoPipeDescendente::PodeEsperarIndefinidamente,
        completion_condition: CondicaoConclusaoCaptura::FilhoDiretoReapadoEStdoutStderrEmEof,
        captured_output_scope: EscopoSaidaCapturada::BytesRecebidosNosPipesHerdadosAteEof,
        direct_child_scope_relation:
            RelacaoEscopoFilhoDireto::ExecucaoSoDoFilhoDiretoCapturaPodeSerProlongadaPorDescendentes,
    };

impl LimiteTempo {
    /// Discriminante da variante: seu índice de declaração em [`VARIANTES`].
    pub fn discriminante(self) -> u64 {
        match self {
            LimiteTempo::SemLimite => TAG_SEM_LIMITE,
            LimiteTempo::Ate(_) => TAG_ATE,
        }
    }

    /// Reconstrói o limite a partir do par (discriminante, carga).
    ///
    /// A carga é ignorada para [`LimiteTempo::SemLimite`]: a variante não tem
    /// carga própria, e aceitar lixo ali seria inventar um estado.
    pub fn de_discriminante(tag: u64, carga: u64) -> Option<Self> {
        match tag {
            TAG_SEM_LIMITE => Some(LimiteTempo::SemLimite),
            TAG_ATE => Some(LimiteTempo::Ate(carga)),
            _ => None,
        }
    }

    /// Duração correspondente, ou `None` quando não há limite.
    ///
    /// Devolver `Option` em vez de um `u64` com valor mágico mantém a distinção
    /// que este módulo existe para preservar, também do lado da implementação.
    pub fn duracao(self) -> Option<std::time::Duration> {
        match self {
            LimiteTempo::SemLimite => None,
            LimiteTempo::Ate(ms) => Some(std::time::Duration::from_millis(ms)),
        }
    }

    /// Verdadeiro quando o limite expira sem conceder tempo algum ao filho.
    pub fn expira_imediatamente(self) -> bool {
        matches!(self, LimiteTempo::Ate(0))
    }
}
// @pinker-nav:end processos.limite-tempo.taxonomia

#[cfg(test)]
mod tests {
    use super::*;

    /// A ordem de declaração é o discriminante. Este teste existe para que
    /// reordenar `VARIANTES` quebre a suíte em vez de corromper valores em
    /// silêncio — mesma disciplina de `tipo_entrada`.
    #[test]
    fn ordem_de_declaracao_fixa_os_discriminantes() {
        assert_eq!(VARIANTES[TAG_SEM_LIMITE as usize].0, VARIANTE_SEM_LIMITE);
        assert_eq!(VARIANTES[TAG_ATE as usize].0, VARIANTE_ATE);
        assert_eq!(LimiteTempo::SemLimite.discriminante(), TAG_SEM_LIMITE);
        assert_eq!(LimiteTempo::Ate(7).discriminante(), TAG_ATE);
    }

    /// O motivo de existir deste leque: `Ate(0)` não é `SemLimite`.
    #[test]
    fn zero_e_expiracao_imediata_nao_ausencia_de_limite() {
        assert!(LimiteTempo::Ate(0).expira_imediatamente());
        assert!(!LimiteTempo::SemLimite.expira_imediatamente());
        assert_eq!(
            LimiteTempo::Ate(0).duracao(),
            Some(std::time::Duration::ZERO)
        );
        assert_eq!(LimiteTempo::SemLimite.duracao(), None);
    }

    #[test]
    fn roundtrip_de_discriminante() {
        assert_eq!(
            LimiteTempo::de_discriminante(TAG_SEM_LIMITE, 999),
            Some(LimiteTempo::SemLimite)
        );
        assert_eq!(
            LimiteTempo::de_discriminante(TAG_ATE, 250),
            Some(LimiteTempo::Ate(250))
        );
        assert_eq!(LimiteTempo::de_discriminante(2, 0), None);
    }

    #[test]
    fn sem_limite_com_descendente_que_mantem_pipe_aberto_pode_esperar_indefinidamente() {
        assert_eq!(
            POLITICA_PIPE_DESCENDENTE_SEM_LIMITE,
            PoliticaPipeDescendenteSemLimite {
                unlimited_descendant_pipe_behavior:
                    ComportamentoPipeDescendente::PodeEsperarIndefinidamente,
                completion_condition:
                    CondicaoConclusaoCaptura::FilhoDiretoReapadoEStdoutStderrEmEof,
                captured_output_scope:
                    EscopoSaidaCapturada::BytesRecebidosNosPipesHerdadosAteEof,
                direct_child_scope_relation:
                    RelacaoEscopoFilhoDireto::ExecucaoSoDoFilhoDiretoCapturaPodeSerProlongadaPorDescendentes,
            }
        );
    }
}
