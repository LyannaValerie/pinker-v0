# Bloco 18 — core nobre e bibliotecas temáticas

## Status
Encerrado por suficiência conservadora na Fase 207.

## Tese
Separar com nitidez o núcleo da linguagem e suas famílias públicas de biblioteca, amadurecendo a nomeação temática sem fingir mecanismo funcional já implementado.

## Dependências
Bloco 17 encerrado; continuidade factual do motor técnico preservada.

## Mapa macro do bloco — leitura final honesta

- **18.1 inventário e taxonomia** — concluído; Fase 180.
- **18.2 famílias oficiais** — concluído; Fase 181.
- **18.3 superfície pública** — concluído no recorte documental; Fases 182–183.
- **18.4 domínio interno** — concluído; Fase 184.
- **18.5 resolução qualificada** — preparado documentalmente (Fase 185); a abertura operacional não aconteceu neste bloco e permanece no inventário futuro.
- **18.6 importação por família** — **concluído para as 7 famílias públicas**; aberto na Fase 186 (`tempo`), ampliado nas Fases 187–189 (`ambiente`, `acaso`, `texto`) e concluído na Fase 207 (`arquivo`, `caminho`, `processo`). Importação seletiva `trazer familia.simbolo;` permanece fora do recorte entregue.
- **18.7 documentação identitária** — cumprido no recorte mínimo pelos documentos canônicos de família (`docs/familias_tematicas.md`, `docs/familias/*.md`, `docs/vocabulario.md`); consolidação identitária mais ampla fica no inventário futuro.
- **18.8 família exemplar** — cumprido no recorte mínimo via `tempo` (Fases 182–183, 186); tratamento adulto da família fica no inventário futuro.
- **18.9 modo estrito opcional** — **não aberto por decisão conservadora**; sem obrigação de import, a superfície global legada permanece o contrato canônico.
- **18.10 reorganização interna** — **não aberta por decisão conservadora**; a organização atual do engine é suficiente para o recorte entregue e reorganizar agora seria risco sem ganho proporcional.
- **18.11 fechamento** — executado na Fase 207 junto com a rodada documental de encerramento.

## Estado factual final
Bloco encerrado com base documental consolidada (Fases 180–185) e mecanismo funcional real de importação por família cobrindo **todas as 7 famílias públicas** canonizadas na Fase 181: `trazer tempo;`, `trazer ambiente;`, `trazer acaso;`, `trazer texto;`, `trazer arquivo;`, `trazer caminho;` e `trazer processo;` (Fases 186–189 e 207). Os domínios provisórios `colecao` e `formato` permanecem não importáveis, coerente com a decisão lexical pendente de 18.2.

Durante a vigência do bloco, as Fases 190–206 expandiram ergonomia de linguagem e o domínio provisório `colecao` fora do escopo formal do bloco; essa expansão está registrada no histórico e não altera a leitura do fechamento.

## Limites explícitos do fechamento
- não há `familia.intrinseca` (resolução qualificada segue apenas preparada documentalmente);
- não há `trazer familia.intrinseca;` (importação seletiva);
- não há modo estrito; o import por família é válido mas nunca obrigatório;
- não houve reorganização funcional de `interpreter.rs` ou do pipeline de IR;
- compatibilidade global legada preservada integralmente para todas as famílias.

## Família exemplar
- Família exemplar do bloco: `tempo` (superfície mínima `tempo_unix()` e `formatar_tempo_unix(...)`).
- A eventual simplificação temática e a resolução qualificada futura continuam condicionadas a mecanismo real e contrato explícito de compatibilidade.

## Domínios internos consolidados
- `core`, `texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`, `colecao` (provisório) e `formato` (provisório).

## Relação com o histórico
- Execução factual preservada em `docs/history/phases/151a200.md` (Fases 180–200) e `docs/history/phases/201a250.md` (Fases 201–207).
- A refatoração histórica que reduz duplicação contextual está em `docs/history/documentation/001a050.md` (Doc-35).
