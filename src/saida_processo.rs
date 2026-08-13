//! Autoridade única da observação estruturada de um processo — Parte D.
//!
//! O defeito que este módulo existe para fechar é de **contagem de execuções**,
//! não de conveniência de API. Antes da Parte D, observar o status e a saída do
//! mesmo comando custava duas intrínsecas:
//!
//! ```text
//! executar_processo(cmd)   -> código de saída   (spawn 1)
//! capturar_stdout(cmd)     -> stdout            (spawn 2)
//! ```
//!
//! Dois spawns, dois processos, dois mundos possíveis. Nada garante que o
//! segundo observou o que o primeiro fez.
//!
//! ```text
//! ONE_LOGICAL_PROCESS_EXECUTION
//! → ONE_EXECUTION
//! → ONE_STRUCTURED_OBSERVATION
//! ```
//!
//! # Por que um handle, e não um agregado
//!
//! `(código, stdout, stderr)` é um **produto** de três valores heterogêneos, e
//! `leque` é soma. O contrato de cargas vigente (ver `enum_payload`) aceita
//! `bombom`, `verso`, um leque declarado e handles de lista — não uma struct, e
//! `RuntimeValue` sequer possui variante de struct. A única forma de espremer o
//! produto numa carga existente seria serializar o código em texto, devolvendo
//! ao usuário a obrigação de reparsear um número que o runtime já tinha —
//! exatamente o que a Parte C recusou ao escolher `leque` para `TipoEntrada`.
//!
//! O handle, portanto, não nasce porque a API do host usa `Child`. Ele nasce
//! porque é a menor representação que preserva os três valores com seus tipos.
//!
//! # O que o handle NÃO é
//!
//! Não é alias de recurso do sistema operacional. Quando o handle passa a
//! existir, o filho já foi esperado e reapado e os pipes já foram fechados. Não
//! há descritor vivo por trás dele, nada a liberar e nenhuma ordem de
//! destruição observável.
//!
//! ```text
//! NO_OS_RESOURCE != NO_LIFETIME_POLICY
//! ```
//!
//! A política de lifetime existe e está declarada em [`PoliticaSnapshot`], com
//! seu custo — não é a ausência de uma.

// @pinker-nav:start processos.saida.snapshot
// @pinker-nav:domain processos
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única da observação estruturada da Parte D: `SaidaProcesso` guarda código de saída, stdout e stderr de UMA execução, `TabelaSaidas` materializa o snapshot atrás de um handle monotônico que nunca é reutilizado, e os acessores públicos `processo_codigo`/`processo_saida`/`processo_erro` apenas LEEM o snapshot — nenhum reexecuta o processo. O handle é valor por palavra sem recurso de SO por trás: quando ele existe, o filho já foi reapado e os pipes já foram fechados. A política de lifetime (retenção até o fim do programa, IDs monotônicos, sem reuso, sem ABA) é a mesma já vigente para listas/mapas/callables e está declarada em `PoliticaSnapshot`.

/// Nome público do tipo da observação estruturada.
///
/// Identidade **produzida pelo runtime**: o valor por trás deste nome é
/// fabricado pela implementação, então o nome não pode ser redeclarado pelo
/// usuário. A recusa vive no mesmo ponto de aceitação de item que já protege
/// `TipoEntrada` — ver `Parser`.
pub const TIPO_SAIDA_PROCESSO: &str = "SaidaProcesso";

/// Nome público do acessor do código de saída.
pub const ACESSOR_CODIGO: &str = "processo_codigo";

/// Nome público do acessor de stdout.
pub const ACESSOR_SAIDA: &str = "processo_saida";

/// Nome público do acessor de stderr.
pub const ACESSOR_ERRO: &str = "processo_erro";

/// Todos os acessores, em ordem estável.
///
/// Nenhum deles executa processo. A lista existe para que as camadas
/// reconheçam o conjunto por uma única declaração, em vez de repetir os três
/// nomes por camada.
pub const ACESSORES: [&str; 3] = [ACESSOR_CODIGO, ACESSOR_SAIDA, ACESSOR_ERRO];

/// Verdadeiro para qualquer acessor de snapshot.
pub fn e_acessor(nome: &str) -> bool {
    ACESSORES.contains(&nome)
}

/// Observação imutável de **uma** execução.
///
/// Os três campos vêm obrigatoriamente do mesmo `wait`/`output`. Não existe
/// construtor que preencha um campo sem os outros: a estrutura torna a
/// observação parcial inexprimível em vez de meramente desaconselhada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaidaProcesso {
    codigo: u64,
    saida: String,
    erro: String,
}

impl SaidaProcesso {
    /// Constrói a observação a partir de uma execução concluída.
    ///
    /// `codigo` já é o código normal do filho: terminação anormal nunca chega
    /// aqui, porque o resultado vira `Erro` antes — ver a política de F4. Isso
    /// mantém [`SaidaProcesso::codigo`] total, sem inventar `128+signal`.
    pub fn nova(codigo: u64, saida: String, erro: String) -> Self {
        Self {
            codigo,
            saida,
            erro,
        }
    }

    /// Código de saída observado.
    pub fn codigo(&self) -> u64 {
        self.codigo
    }

    /// stdout observado, como texto.
    pub fn saida(&self) -> &str {
        &self.saida
    }

    /// stderr observado, como texto.
    ///
    /// Canal distinto de [`SaidaProcesso::saida`] por construção: os dois campos
    /// nunca compartilham buffer, então "misturar stdout e stderr" não é um
    /// comportamento que a estrutura permita expressar.
    pub fn erro(&self) -> &str {
        &self.erro
    }
}

/// Política de lifetime do snapshot, declarada explicitamente.
///
/// Existe como item nomeado, e não como comentário solto, porque a D13 exige
/// que uma família de valor por handle responda essas perguntas antes de ser
/// implementada.
pub struct PoliticaSnapshot;

impl PoliticaSnapshot {
    /// O snapshot é retido até o fim do programa.
    ///
    /// Custo real e assumido: cada execução retém seu stdout e seu stderr pelo
    /// resto do programa. Um programa que executa muitos processos com saída
    /// volumosa acumula memória. É a mesma política já vigente para listas,
    /// mapas e callables — não uma exceção aberta para processos.
    pub const RETIDO_ATE_O_FIM: bool = true;

    /// Handles são monotônicos e nunca reutilizados.
    ///
    /// Consequência direta: `stale alias` e ABA são impossíveis por construção,
    /// não por disciplina de uso.
    pub const HANDLE_REUTILIZADO: bool = false;

    /// O snapshot é imutável depois de criado.
    pub const MUTAVEL_APOS_CRIACAO: bool = false;

    /// Nenhum recurso do sistema operacional sobrevive à criação do handle.
    pub const RECURSO_DE_SO_VIVO: bool = false;
}

/// Tabela de snapshots do runtime.
///
/// Mesma forma de `RuntimeListState`/`CallableState`: mapa por handle mais um
/// contador monotônico, sem caminho de remoção. A semelhança é deliberada e
/// verificada, não presumida — reusar a política existente é o que mantém
/// `SaidaProcesso` fora da categoria de recurso com ciclo de vida próprio.
#[derive(Debug)]
pub struct TabelaSaidas {
    entradas: std::collections::HashMap<u64, SaidaProcesso>,
    proximo_handle: Option<u64>,
}

impl Default for TabelaSaidas {
    fn default() -> Self {
        Self {
            entradas: std::collections::HashMap::new(),
            proximo_handle: Some(1),
        }
    }
}

impl TabelaSaidas {
    pub fn nova() -> Self {
        Self::default()
    }

    /// Materializa um snapshot e devolve seu handle.
    ///
    /// O handle só é emitido depois que a execução terminou, então não existe
    /// janela em que ele aponte para uma observação incompleta. O incremento é
    /// verificado e transforma `u64::MAX` no último handle possível; uma chamada
    /// posterior é falha de invariante interna antes de tocar a tabela.
    pub fn inserir(&mut self, saida: SaidaProcesso) -> u64 {
        let handle = self
            .proximo_handle
            .expect("invariante interna violada: namespace de handles de SaidaProcesso esgotado");
        assert!(
            !self.entradas.contains_key(&handle),
            "invariante interna violada: handle de SaidaProcesso seria reutilizado"
        );
        let proximo_handle = handle.checked_add(1);
        self.entradas.insert(handle, saida);
        self.proximo_handle = proximo_handle;
        handle
    }

    /// Lê um snapshot já materializado.
    ///
    /// `None` significa handle não produzido por esta tabela — violação de
    /// invariante interna, não falha operacional do programa do usuário.
    pub fn obter(&self, handle: u64) -> Option<&SaidaProcesso> {
        self.entradas.get(&handle)
    }

    /// Quantidade de snapshots retidos. Serve à evidência da política de
    /// lifetime: o crescimento é observável em teste, não uma alegação.
    pub fn retidos(&self) -> usize {
        self.entradas.len()
    }
}
// @pinker-nav:end processos.saida.snapshot

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acessores_sao_reconhecidos_por_uma_unica_declaracao() {
        for nome in ACESSORES {
            assert!(e_acessor(nome));
        }
        assert!(!e_acessor("executar_processo"));
        assert!(!e_acessor("capturar_stdout"));
    }

    #[test]
    fn stdout_e_stderr_sao_canais_distintos() {
        let saida = SaidaProcesso::nova(0, "para stdout".to_string(), "para stderr".to_string());
        assert_eq!(saida.saida(), "para stdout");
        assert_eq!(saida.erro(), "para stderr");
        assert_ne!(saida.saida(), saida.erro());
    }

    /// Handles monotônicos e sem reuso: é o que torna ABA impossível.
    #[test]
    fn handles_sao_monotonicos_e_nunca_reutilizados() {
        let mut tabela = TabelaSaidas::nova();
        let a = tabela.inserir(SaidaProcesso::nova(0, "a".into(), String::new()));
        let b = tabela.inserir(SaidaProcesso::nova(1, "b".into(), String::new()));
        let c = tabela.inserir(SaidaProcesso::nova(2, "c".into(), String::new()));

        assert_eq!(a, 1, "zero não é identidade produzida");
        assert!(a < b && b < c, "handles precisam ser monotônicos");
        assert_eq!(tabela.retidos(), 3, "nada é removido");
        assert!(!std::hint::black_box(PoliticaSnapshot::HANDLE_REUTILIZADO));
    }

    #[test]
    fn esgotamento_de_handle_nao_faz_wrap_nem_aba() {
        let mut tabela = TabelaSaidas::nova();
        let antigo = tabela.inserir(SaidaProcesso::nova(11, "antigo".into(), "a".into()));
        tabela.proximo_handle = Some(u64::MAX);

        let ultimo = tabela.inserir(SaidaProcesso::nova(22, "ultimo".into(), "u".into()));
        assert_eq!(ultimo, u64::MAX, "último estado permitido");
        assert_eq!(tabela.proximo_handle, None, "namespace está esgotado");
        assert_eq!(tabela.retidos(), 2);

        let esgotamento = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tabela.inserir(SaidaProcesso::nova(33, "novo".into(), "n".into()));
        }));
        assert!(esgotamento.is_err(), "esgotamento é falha de invariante");
        assert_eq!(tabela.retidos(), 2, "falha não insere nem sobrescreve");
        assert_eq!(tabela.obter(antigo).expect("alias antigo").codigo(), 11);
        assert_eq!(tabela.obter(ultimo).expect("último snapshot").codigo(), 22);
        assert!(tabela.obter(0).is_none(), "wrap não reutiliza zero");
    }

    /// Duas cópias do handle observam o MESMO snapshot.
    #[test]
    fn copias_do_handle_observam_o_mesmo_snapshot() {
        let mut tabela = TabelaSaidas::nova();
        let handle = tabela.inserir(SaidaProcesso::nova(7, "saida".into(), "erro".into()));
        let copia = handle;

        assert_eq!(tabela.obter(handle), tabela.obter(copia));
        assert_eq!(tabela.obter(copia).map(SaidaProcesso::codigo), Some(7));
    }

    #[test]
    fn handle_nao_produzido_nao_resolve() {
        let tabela = TabelaSaidas::nova();
        assert!(tabela.obter(0).is_none());
    }
}
