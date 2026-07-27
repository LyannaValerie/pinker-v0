---
pinker-doc: 1
id: development.semana-estabilizacao-2026-07
domain: development
kind: state
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.stabilization-window
related:
  - development
  - engine
  - roadmap
---

# Semana de estabilização estrutural da Pinker

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

<!-- @pinker-doc:start
id: development.semana-estabilizacao-2026-07.estado
tags: [desenvolvimento, estabilizacao, manutencao, eixo-a, congelamento]
aliases:
  - semana de estabilização
  - congelamento do eixo a
  - manutenção estrutural
  - trabalho autorizado durante a estabilização
summary: Estado operacional, período, trabalhos autorizados e critérios de retomada da semana de estabilização estrutural.
-->
## Estado operacional roteável

De **27 de julho a 1º de agosto de 2026**, o Bloco 20 permanece
estruturalmente ativo e a expansão funcional do Eixo A fica temporariamente
suspensa.

Durante a janela, estão autorizados:

- refatoração estrutural sem mudança deliberada de comportamento;
- reorganização documental;
- bughunting;
- correção de bugs reproduzidos;
- testes, CI, sensibilidade, navegação e cartografia necessários.

Novas fases funcionais do Eixo A permanecem suspensas. A retomada exige decisão
humana explícita. As fontes operacionais superiores são `docs/roadmap.md` e
`docs/handoff_codex.md`.
<!-- @pinker-doc:end development.semana-estabilizacao-2026-07.estado -->

## Período e decisão

A Pinker entra em uma semana de estabilização estrutural entre **27 de julho e
1º de agosto de 2026**.

Durante essa janela, o **Eixo A fica temporariamente congelado**. O congelamento
não o declara concluído nem abre uma nova fase funcional. Ele não altera a
direção, a ordem de longo prazo ou o estado histórico do roadmap; registra
somente uma suspensão operacional temporária enquanto a base existente é
reorganizada, auditada e estabilizada.

A operação parte da `main` posterior ao merge da PR #407.

## Justificativa

O crescimento rápido da Pinker produziu unidades de código com milhares de
linhas, incluindo arquivos acima de oito mil linhas. Embora funcionais, esses
arquivos concentram responsabilidades demais e tornam mudanças simples mais
custosas e arriscadas para humanos e agentes.

A documentação também contém materiais conceitualmente próximos distribuídos
em pontos diferentes de `docs/`. A estrutura atual registra obrigações e
territórios, mas ainda não os reflete de forma suficientemente previsível na
organização física dos documentos e do código-fonte.

As revisões recentes ainda revelaram defeitos que atravessavam semântica,
metadados, IR, closures, callables e backend. Antes de continuar a expansão
funcional do Eixo A, a prioridade passa a ser reduzir essa dívida estrutural,
melhorar a navegabilidade e investigar sistematicamente a base atual.

## Trabalho autorizado

### Modularização do código-fonte

O código será dividido por responsabilidade arquitetural, preservando
comportamento, APIs públicas, ABI e paridade entre interpretador e backend
nativo.

A divisão deve produzir fachadas estáveis e módulos coesos, por exemplo:

- modelo e tipos;
- contexto e metadados;
- lowering por domínio;
- validação;
- ABI;
- emissão e rendering;
- callables, closures e objetos de trato quando constituírem responsabilidades
  próprias.

A modularização não deve ser feita por cortes arbitrários de quantidade de
linhas. Cada módulo precisa possuir responsabilidade reconhecível, dependências
explícitas e interface interna limitada.

Os diretórios do código-fonte podem permanecer em inglês. Não existe obrigação
de traduzir nomes técnicos consolidados como `ir`, `lowering`, `backend`,
`runtime`, `parser` ou `semantic`.

### Reorganização documental

A documentação será inventariada, classificada e reorganizada por território e
responsabilidade.

A **nova arquitetura de diretórios documentais deve usar nomes em português**.
A migração deverá avaliar e consolidar territórios equivalentes a:

- `rosa`;
- `ponte`;
- `motor`;
- `linguagem`;
- `desenvolvimento`;
- `roteiro`;
- `historico`;
- `referencia`;
- `manutencao`.

A lista final será confirmada pela auditoria estrutural, mas diretórios novos ou
renomeados na arquitetura-alvo não deverão adotar nomes ingleses. IDs estáveis,
links, catálogos, portais, histórico e referências internas deverão ser
migrados de forma verificável.

### Bughunting exaustivo

Até sábado, a base será investigada sistematicamente em:

- lexer e parser;
- análise semântica;
- aliases e identidade nominal;
- IR e validadores;
- CFG e seleção de instruções;
- interpretador;
- backend nativo e ABI;
- closures, callables e objetos de trato;
- runtime, layouts, offsets e alocações;
- diagnósticos, spans e panics;
- paridade entre interpretador e nativo;
- determinismo e execução em ambiente limpo.

Todo bug confirmado deve ser reproduzido, corrigido no menor escopo coerente e
protegido por teste de regressão. Bugs impossíveis de corrigir por dependência
externa ou decisão arquitetural devem ser registrados com evidência e causa.

## Política de documentação para mudanças no código

Mudanças ordinárias relacionadas ao código-fonte não exigem uma **rodada
documental autônoma** apenas por alterarem `src/`, `tests/` ou `examples/`.
Atualizações factuais obrigatórias podem acompanhar a própria PR funcional,
hotfix ou refatoração.

Mudanças estruturais são a exceção. Reorganizações de módulos, diretórios,
fronteiras de responsabilidade, caminhos públicos ou arquitetura documental
devem possuir registro documental estrutural verificável. A operação desta
semana pertence a essa classe porque reorganizará simultaneamente código e
documentos.

## Critério para novas adições estruturais

Toda adição ampla — incluindo a Trama Pinker, futuras Tramas, novos subsistemas,
catálogos, aplicações internas ou mecanismos equivalentes — deve ser avaliada
em dois planos obrigatórios:

1. **Documentação específica em diretório específico**, com portal, contrato,
   ownership, ciclo de vida, fontes de verdade, validação e navegação;
2. **Código-fonte específico modularizado de forma específica**, quando a
   implementação for grande, evitando concentrar um subsistema inteiro em um
   arquivo monolítico.

Antes de criar uma nova Trama, deve-se avaliar se a Trama existente pode ser
reutilizada ou estendida sem misturar responsabilidades. A reutilização é
preferida quando fontes, contratos, ciclo de sincronização, validação e modelo
de consulta são compatíveis. Uma nova Trama só se justifica quando houver uma
fronteira independente de responsabilidade, fonte de verdade, ciclo de vida,
gates ou semântica de consulta.

Essa avaliação deve ser registrada antes da implementação estrutural, mas não
obriga a criação de uma nova Trama quando a infraestrutura atual já satisfizer
o contrato.

## Regras do congelamento

Durante a janela ficam suspensos:

- novas funcionalidades da linguagem;
- avanço para nova fase do Eixo A;
- expansão de escopo motivada apenas por oportunidade;
- mudanças de roadmap que simulem progresso funcional;
- mistura de feature com modularização ou bugfix.

Continuam autorizados:

- `refactor(...)` sem mudança comportamental deliberada;
- `docs(...)` de organização estrutural;
- `fix(...)` para defeitos reproduzidos;
- `test(...)` para regressões e sensibilidade;
- ajustes de CI, navegação e cartografia diretamente necessários.

## Retorno antecipado ao Eixo A

O Eixo A poderá ser retomado ainda nesta semana caso o trabalho termine antes
de 1º de agosto.

A retomada antecipada exige cumulativamente:

- modularização planejada concluída no nível aceito pela Founder;
- reorganização documental concluída no nível aceito pela Founder;
- bughunting encerrado sem bug reproduzível conhecido deixado silenciosamente;
- bugs bloqueados registrados com causa e evidência;
- `main` íntegra e gates obrigatórios aprovados;
- decisão humana explícita encerrando o congelamento;
- nova tarefa e branch baseadas na `main` vigente.

A data final é um limite operacional, não uma obrigação de manter o Eixo A
parado quando todos os objetivos já tiverem sido cumpridos.

## Gates

As entregas devem preservar, conforme aplicável:

```text
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
pink doc verificar
pink nav verificar
make guard
make preflight
make ci
```

Valores históricos congelados não podem ser substituídos pelo estado corrente
apenas para fazer testes passarem.

## Limites desta formalização

Este documento não:

- altera o estado histórico do Eixo A;
- marca fase como concluída;
- executa a reorganização planejada;
- modifica compilador, runtime ou testes;
- autoriza merge automático;
- substitui a autorização humana para retomar o Eixo A.
