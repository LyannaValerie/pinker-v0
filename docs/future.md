# Backlog futuro técnico (inventário Engine)

- **Classe:** Engine
- **Papel:** referência
- **Status:** referência

> **Precedência:** este documento não define ordem ativa. A trilha ativa está em `docs/roadmap.md`.
> Para visão identitária/estética, consultar `docs/rosa.md` e `docs/parallel.md`.

## 1. Papel deste arquivo

`future.md` concentra o inventário técnico de médio/longo prazo em formato operacional:

- sem virar roadmap paralelo;
- sem duplicar manifesto conceitual;
- com foco em lacunas de implementação.

## 2. Frentes técnicas estruturais (resumo executivo)

### 2.1 Memória operacional além do subset atual

- ampliar `seta<T>` além de `bombom`;
- escrita de campo (`ninho`) e escrita por índice (arrays além do subset `[bombom; N]` das Fases 147–148);
- ampliar `virar` para casos seguros ainda ausentes;
- robustez adicional de `fragil` (sem prometer MMIO/fences de imediato).

### 2.2 Backend nativo real além do subset linear

- controle de fluxo geral no backend externo;
- cobertura de memória indireta/ponteiros;
- ABI mais completa (além de até 3 args `bombom`);
- passos para artefato executável mais amplo e reproduzível.

### 2.3 Biblioteca e ecossistema útil

- I/O mais rica (arquivo/texto) com recorte incremental;
- tooling de projeto além do `pink build` mínimo;
- evolução prudente de `verso` sem abrir biblioteca textual gigante de uma vez;
- **exportação de tipos pelo sistema de módulos**: `ninho` e `apelido` já são exportáveis via `trazer` (Fases 144 e 145), e a Fase 146 abriu uso qualificado mínimo (`modulo.Tipo`) em contexto tipado; o Bloco 12 fica fechado no recorte conservador sem visibilidade rica/namespaces amplos.
- O Bloco 11 foi encerrado por suficiência conservadora (Doc-27) após as Fases 137–143.
- A FE-1 aqueceu canonicamente a periferia argv/env para `tem_chave`, `pedir_argumento` e `buscar_contexto` sem reabrir o Bloco 11 nem alterar seu encerramento.
- O Bloco 12 foi encerrado por suficiência conservadora (Doc-28) após as Fases 144–146.
- O Bloco 13 foi encerrado por suficiência conservadora na Fase 156 após abrir `lista<bombom>`, `mapa<verso,bombom>`, iteração confortável mínima e aleatoriedade básica com semente explícita.
- O Bloco 14 foi encerrado por suficiência conservadora (Doc-29) após as Fases 157–160, que abriram e entregaram: `formatar_verso` (14.1), CSV mínimo de linha única (14.2), JSON plano mínimo para `mapa<verso,bombom>` (14.3) e tempo básico mínimo via timestamp Unix em `bombom` com formatação UTC fixa (14.4). O recorte permanece pequeno e auditável; formatação rica, CSV completo, JSON geral, timezones, locale e biblioteca adulta de datas continuam fora do escopo atual.

Itens explicitamente para depois do foco inicial do Bloco 11:
- linguagem-cola;
- captura de saída de comandos e integração de processos além do recorte mínimo aberto nas Fases 161–163;
- integração rica com stdin/stdout/stderr além da escada pequena atualmente ativa no Bloco 15;
- coleções básicas além do recorte já aberto (`lista<bombom>` + `mapa<verso,bombom>` mínimos, iteração confortável mínima em `lista<bombom>` e `mapa<verso,bombom>`, além de aleatoriedade básica com semente explícita na Fase 156; seguem fora `lista<T>`/`mapa<K,V>` amplos, dicionário rico e iteração confortável ampla/generics);
- JSON amplo, datas/tempo com timezone/locale e formatação rica além do recorte mínimo já entregue e encerrado no Bloco 14 (Fases 157–160).

Alinhamento factual do Bloco 15:
- o bloco foi encerrado por suficiência conservadora na Doc-31;
- 15.1, 15.2, 15.3, 15.4 e 15.5 já foram entregues no recorte mínimo (`executar_processo`, `capturar_stdout`, `capturar_stderr`, `executar_com_entrada` e `pipeline_minimo`);
- a Doc-30 refinou a continuação imediata do bloco em subdegraus menores (`stderr`, `stdin` e `pipe` mínimos), e a Fase 166 concluiu essa escada sem declarar integração completa de processos como já entregue;
- shell amplo, quoting rico, cadeia longa de pipes, sessão interativa, PTY, job control e integração adulta de subprocessos continuam fora do que foi entregue;
- expansões além desses subdegraus pequenos continuam pertencendo ao inventário futuro, não a uma continuação automática da trilha oficial do bloco.

### 2.4 Editor/TUI oficial da Pinker (frente funcional aberta em camada 1, atualmente pausada)

- A Fase 136 abriu a base funcional inicial do editor/TUI oficial da Pinker.
- O recorte técnico permanece pequeno e conservador.
- O editor/TUI continua sem substituir o compilador/backend como fundação técnica; o backend permanece consolidado e preservado.
- A continuidade funcional dessa frente segue pausada por decisão estratégica.

Papel inicial do subprojeto:
- superfície própria de uso da Pinker para edição, execução e visualização de diagnósticos;
- interface de uso do compilador/ecossistema com identidade visual coerente com a paleta Pinker;
- editor textual/TUI oficial do ecossistema (não IDE ampla).

Funções iniciais previstas (próximas camadas, após a Fase 136):
- ampliar edição textual básica ainda conservadora;
- reforçar sinalização/diagnóstico mínimos;
- expandir gradualmente comandos Pinker no fluxo principal além do recorte já ativo (`tokens` e `ast`).

Limites iniciais explícitos:
- não começar como IDE ampla;
- não competir cedo com editores maduros;
- não prometer edição "perfeita" no início;
- não virar shell genérico;
- não inflar no arranque watch, árvore de símbolos, linguagem estrutural rica ou ecossistema completo.

## 3. Itens de longo prazo (sem compromisso de ordem)

Os itens abaixo que antes estavam listados aqui como backlog amplo foram reorganizados na trilha canônica dos Blocos 12–16 em `docs/roadmap.md`:
- módulos tipados (`ninho`/`apelido` exportáveis) → Bloco 12;
- coleções, iteração, aleatoriedade básica → Bloco 13 (concluído);
- formatação, CSV, JSON, datas → Bloco 14;
- processos externos, captura, stdin/stdout/stderr → Bloco 15;
- REPL, linguagem-cola → Bloco 16.

Itens de longo prazo ainda sem bloco definido:
- abstrações avançadas (traits/generics);
- biblioteca padrão mais robusta além da trilha 12–16;
- self-hosting (horizonte distante);
- kernel/ambiente bare-metal mais robusto;
- package manager soberano.

Alinhamento do Bloco 16 após a Fase 167:
- 16.1 foi entregue no recorte mínimo por `pink repl`;
- seguem no inventário futuro, e não foram abertos nesta fase: REPL adulto com estado persistente rico, multiline amplo, histórico sofisticado, autocomplete, inspeção rica e shell ampla;
- 16.2 (linguagem-cola) continua como próximo degrau funcional do bloco na trilha oficial.

## 4. Relação com a camada Rosa

- Este arquivo diz **o que falta tecnicamente**.
- `docs/rosa.md` diz **por que e com que identidade** evoluir.
- `docs/ponte_engine_rosa.md` conecta ambas as perspectivas sem confusão.
