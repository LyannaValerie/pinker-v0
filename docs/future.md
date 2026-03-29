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
- escrita de campo (`ninho`) e escrita por índice (arrays);
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
- O Bloco 12 foi encerrado por suficiência conservadora (Doc-28) após as Fases 144–146; o próximo bloco formal da trilha ativa passa a ser o Bloco 13 (aberto funcionalmente na Fase 147 com array fixo por valor no recorte mínimo).

Itens explicitamente para depois do foco inicial do Bloco 11:
- REPL;
- linguagem-cola;
- execução de processos externos + captura de saída de comandos;
- integração rica com stdin/stdout/stderr;
- coleções básicas (lista, mapa/dicionário, iteração mais confortável);
- JSON, CSV, datas/tempo, aleatoriedade básica e formatação simples.

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
- coleções, iteração, aleatoriedade → Bloco 13;
- formatação, CSV, JSON, datas → Bloco 14;
- processos externos, captura, stdin/stdout/stderr → Bloco 15;
- REPL, linguagem-cola → Bloco 16.

Itens de longo prazo ainda sem bloco definido:
- abstrações avançadas (traits/generics);
- biblioteca padrão mais robusta além da trilha 12–16;
- self-hosting (horizonte distante);
- kernel/ambiente bare-metal mais robusto;
- package manager soberano.

## 4. Relação com a camada Rosa

- Este arquivo diz **o que falta tecnicamente**.
- `docs/rosa.md` diz **por que e com que identidade** evoluir.
- `docs/ponte_engine_rosa.md` conecta ambas as perspectivas sem confusão.
