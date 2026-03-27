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
- evolução prudente de `verso` sem abrir biblioteca textual gigante de uma vez.

### 2.4 Editor/TUI oficial da Pinker (subprojeto futuro do ecossistema)

- O editor/TUI oficial da Pinker passa a existir como direção reconhecida do ecossistema do projeto.
- Ele não substitui o compilador/backend como foco técnico atual.
- Com a Doc-24, ele passa de direção futura reconhecida para próxima frente funcional oficial a ser aberta na sequência do projeto.

Papel inicial do subprojeto:
- superfície própria de uso da Pinker para edição, execução e visualização de diagnósticos;
- interface de uso do compilador/ecossistema com identidade visual coerente com a paleta Pinker;
- editor textual/TUI oficial do ecossistema (não IDE ampla).

Funções iniciais previstas (recorte pequeno e disciplinado):
- abrir/salvar arquivos `.pink`;
- edição textual básica;
- realce/sinalização mínima;
- barra de status;
- painel de saída/diagnóstico;
- comandos Pinker visíveis no fluxo principal (`rodar`, `testar`, `tokens`, `ast`, `maquina`, `diagnostico`, `montar`, `limpar`).

Limites iniciais explícitos:
- não começar como IDE ampla;
- não competir cedo com editores maduros;
- não prometer edição "perfeita" no início;
- não virar shell genérico;
- não inflar no arranque watch, árvore de símbolos, linguagem estrutural rica ou ecossistema completo.

## 3. Itens de longo prazo (sem compromisso de ordem)

- sistema de módulos mais completo;
- abstrações avançadas (traits/generics);
- biblioteca padrão mais robusta;
- self-hosting (horizonte distante);
- kernel/ambiente bare-metal mais robusto.

## 4. Relação com a camada Rosa

- Este arquivo diz **o que falta tecnicamente**.
- `docs/rosa.md` diz **por que e com que identidade** evoluir.
- `docs/ponte_engine_rosa.md` conecta ambas as perspectivas sem confusão.

