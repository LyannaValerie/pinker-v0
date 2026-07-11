# Arquitetura lexical da Pinker

- **Classe:** Rosa
- **Papel:** léxico
- **Status:** ativo

Este documento define o eixo lexical da Pinker de forma arquitetural.

## 1. Critérios de keyword forte

Uma keyword é forte quando, ao mesmo tempo:

1. **Nomeia bem a função técnica** (sem ambiguidade operacional).
2. **Carrega tom da linguagem** (gentil, firme, sem infantilização).
3. **É sustentável no ecossistema** (cabe em docs, erros, tooling e ensino).
4. **Escala semanticamente** (não quebra quando o recurso amadurece).
5. **Mantém diferenciação identitária** sem sacrificar legibilidade.

## 2. Sinais de keyword ruim

Evitar keywords que:

- exigem explicação longa para tarefa simples;
- parecem piada ou caricatura do projeto;
- competem semanticamente com palavras já estabelecidas;
- são tecnicamente corretas, mas descaracterizam o tom Pinker;
- funcionam só no recorte atual e colapsam em versões futuras.

## 3. Famílias lexicais fortes da Pinker

- **Estruturas centrais:** `pacote`, `carinho`, `mimo`, `principal`.
- **Fluxo e controle:** `talvez`, `senao`, `sempre que`, `quebrar`, `continuar`.
- **Estado e tipo:** `nova`, `muda`, `apelido`, `ninho`, `seta`, `fragil`, `virar`.
- **Texto e expressão:** `verso`, `falar`, `juntar_verso`, `tamanho_verso`, `formatar_verso`, `ler_linha_csv_bombom`, `emitir_linha_csv_bombom`, `ler_json_plano_bombom`, `emitir_json_plano_bombom`, `tempo_unix`, `formatar_tempo_unix`.
- **I/O, tooling e integração sistêmica mínima:** `ouvir`, `abrir`, `fechar`, `escrever`, `argumento`, `executar_processo`, `executar_com_entrada`, `pipeline_minimo`, `capturar_stdout`, `capturar_stderr`, `sair`.

## 3.1 Famílias temáticas oficiais iniciais do Bloco 18

No plano **nominal e arquitetural** do Bloco 18, a Pinker passa a reconhecer como famílias públicas iniciais:

- `texto`
- `arquivo`
- `caminho`
- `processo`
- `tempo`
- `ambiente`
- `acaso`

Os domínios `colecao` e `formato` permanecem **provisórios** nesta fase: o agrupamento técnico já existe, mas o nome público ainda não está lexicalmente maduro o suficiente para canonização.

Critérios mínimos para aceitar uma família pública:

1. força técnica do agrupamento já existente;
2. clareza de domínio;
3. sustentabilidade de crescimento;
4. legibilidade;
5. coerência com a identidade Pinker;
6. capacidade de sustentar documentação pública futura;
7. baixo risco de colisão ou confusão com construtos já existentes.

Referência cruzada canônica desta decisão: `docs/familias_tematicas.md`.

## 3.2 Família exemplar `tempo`

`tempo` fica formalizada como a família exemplar do Bloco 18.

Leitura lexical canônica desta fase:

- os nomes aceitos e preservados hoje são `tempo_unix` e `formatar_tempo_unix`;
- esses nomes continuam sendo a superfície pública real do estado implementado;
- a existência de `tempo` como família não rebaixa nem invalida a compatibilidade desses nomes;
- a revisão lexical futura continua aberta para o momento em que a linguagem tiver mecanismo próprio de superfície por família.

Direção futura plausível, mas não canonizada agora:

- reduzir redundâncias herdadas dos nomes globais;
- mover o peso nominal para a família `tempo`;
- avaliar formas temáticas mais curtas apenas quando houver base operacional para isso.

Exemplos apenas ilustrativos, não aceitos nesta fase:

- `tempo.agora_unix(...)`
- `tempo.formatar_unix(...)`
- `tempo.agora(...)`
- `tempo.formatar(...)`

Referências cruzadas: `docs/familias_tematicas.md` e `docs/familias/tempo.md`.

## 3.3 Superfície futura por família

No Bloco 18, a futura superfície por família deve ser lida como direção arquitetural condicionada por mecanismo real.

Leitura lexical correta:

- nomes globais legados continuam sendo a superfície canônica do presente implementado;
- formas temáticas futuras existem hoje apenas como horizonte plausível;
- variantes ilustrativas não equivalem a spelling final aceito.

Critérios lexicais mínimos da futura transição:

1. preservar compatibilidade explícita;
2. reduzir redundância só quando houver ganho claro;
3. mover o peso nominal para a família temática;
4. manter distinções funcionais importantes entre nomes irmãos;
5. não congelar exemplos ilustrativos como decisão lexical final.

Referências cruzadas: `docs/familias/superficie.md`, `docs/familias/tempo.md` e `docs/familias_tematicas.md`.

## 3.4 Resolução qualificada futura por família

No eixo lexical do Bloco 18, a futura resolução qualificada por família deve ser lida como camada posterior à superfície temática futura.

Leitura lexical correta:

- nomes globais legados continuam sendo a forma aceita do presente;
- formas qualificadas como `tempo.alguma_coisa(...)` só podem aparecer como ilustração arquitetural nesta fase;
- a forma qualificada futura não apaga automaticamente a forma legada;
- importação por família continua fora do escopo desta preparação.

Critérios lexicais mínimos antes de abertura operacional:

1. compatibilidade explícita com a superfície atual;
2. ganho nominal claro;
3. distinção estável entre exemplo ilustrativo e forma aceita;
4. relação limpa entre família pública e nomes internos;
5. mecanismo real antes de qualquer canonização pública de uso.

Referências cruzadas: `docs/familias/resolucao.md`, `docs/familias/superficie.md` e `docs/familias/tempo.md`.

## 4. Keywords aceitas e implementadas

### Núcleo de linguagem

`pacote`, `carinho`, `mimo`, `talvez`, `senao`, `sempre que`, `quebrar`, `continuar`, `eterno`, `nova`, `muda`, `bombom`, `logica`, `verdade`, `falso`, `principal`, `encaixe`.

Nota factual: `encaixe` foi promovida de provisória a aceita/implementada na Fase 209 como pattern matching mínimo sobre `leque` (despacho por variante, extração de carga e exaustividade verificada no parse).

### Sistema de tipos e memória

`apelido`, `ninho`, `leque`, `seta`, `virar`, `peso`, `alinhamento`, `fragil`, `sussurro`, `livre`, `trazer`, `verso`.

Nota factual: `leque` foi promovida de provisória a aceita/implementada na Fase 208 como enumeração nominal mínima (variantes sem dados; acesso `Leque.Variante`; despacho via `escolha`).

### Runtime textual/I/O (Bloco 8, encerrado como trilha ativa)

`falar`, `ouvir`, `ouvir_verso`, `ouvir_verso_ou`, `abrir`, `fechar`, `escrever`, `ler_arquivo`, `juntar_verso`, `tamanho_verso`, `formatar_verso`, `ler_linha_csv_bombom`, `emitir_linha_csv_bombom`, `ler_json_plano_bombom`, `emitir_json_plano_bombom`, `tempo_unix`, `formatar_tempo_unix`, `indice_verso`, `argumento`, `argumento_ou`, `quantos_argumentos`, `tem_argumento`, `tem_chave`, `pedir_argumento`, `ambiente_ou`, `buscar_contexto`, `diretorio_atual`, `caminho_existe`, `e_arquivo`, `e_diretorio`, `juntar_caminho`, `tamanho_arquivo`, `e_vazio`, `criar_diretorio`, `remover_arquivo`, `remover_diretorio`, `ler_verso_arquivo`, `ler_arquivo_verso`, `arquivo_ou`, `criar_arquivo`, `escrever_verso`, `truncar_arquivo`, `abrir_anexo`, `anexar_verso`, `contem_verso`, `comeca_com`, `termina_com`, `igual_verso`, `vazio_verso`, `aparar_verso`, `minusculo_verso`, `maiusculo_verso`, `indice_verso_em`, `buscar_verso`, `nao_vazio_verso`, `aleatorio_criar`, `aleatorio_proximo`, `executar_processo`, `executar_com_entrada`, `pipeline_minimo`, `capturar_stdout`, `capturar_stderr`, `sair`.

### Forma textual dual adicional

`nope` (equivalente textual de `~`).

## 5. Keywords rejeitadas (decisão atual)

- `ninhozinho` e diminutivos em geral: quebra de tom técnico.
- `instavel` para volatile: perde nuance de cuidado ativo de `fragil`.
- `tornar` como cast principal: menos preciso que `virar` no uso atual.

## 6. Keywords provisórias (aceitáveis, ainda não fechadas)

- Tipos/estrutura: `par`, `canto`, `letra`, `grao` (`leque` foi promovida a aceita na Fase 208).
- Controle: `passeio`, `roda` (`encaixe` foi promovida a aceita na Fase 209).
- Erros/fluxo: `amparo`, `tropeco`.
- Abstração: `molde`, `vestir`, `qualquer`.
- Sistemas: `reserva`, `soltar`, `raiz`.

## 6.1 Legado provisório documentado

- `tem_argumento_nomeado` permanece aceito como legado provisório compatível; nome canônico atual: `tem_chave`.
- `argumento_nomeado_ou` permanece aceito como legado provisório compatível; nome canônico atual: `pedir_argumento`.
- `argumento_nomeado_ou_ambiente_ou` permanece aceito como legado provisório compatível; nome canônico atual: `buscar_contexto`.

## 7. Keywords tecnicamente aceitáveis, mas fracas identitariamente

- `publico` / `privado` (claras, porém genéricas para o tom Pinker).
- `modulo` (funcional, porém menos característico que `canto`).
- `unsafe` transliterado (técnico, mas desalinhado à proposta de voz).

## 8. Vocabulário técnico x final x provisório

- **Técnico:** termos de implementação/documentação de engine (`volatile`, `cast`, `alias`, `backend`).
- **Final (Pinker):** superfície pública da linguagem (`fragil`, `virar`, `apelido`).
- **Provisório:** hipótese lexical ainda em avaliação para futuras features.

Regra prática: termo técnico pode aparecer na documentação interna, mas a superfície da linguagem deve priorizar o vocabulário final Pinker.

## 9. Governança lexical mínima

Antes de aceitar keyword nova:

1. validar critério técnico + identitário;
2. avaliar colisão com família lexical já existente;
3. registrar status (aceita/rejeitada/provisória);
4. atualizar `docs/history.md` quando houver decisão material.


## 10. Nota de continuidade Engine ↔ Rosa

- Encerramento formal do Bloco 8 não reduz o valor lexical das intrínsecas de I/O já aceitas; apenas mudou a prioridade da trilha ativa.
- O encerramento conservador do Bloco 9 (Doc-20) não introduziu keyword nova por padrão; consolidou ganho de backend nativo externo já implementado.
- A abertura canônica do Bloco 10 (Doc-21) também não abre keyword nova por si: define foco de cobertura semântica no backend nativo com ordem disciplinada (incluindo `ninho` antes de `virar` e `verso` como item final condicional).
- As Fases 111, 112, 113, 114, 115, 116, 117, 118 e 119 mantiveram a diretriz de ampliar backend nativo sem inflar superfície lexical pública.
- As Fases 120–135 no backend nativo externo também não introduziram keyword nova: abriram cobertura semântica mínima de tipos já existentes (`u32`/`u64`), comparações ampliadas mínimas (`!=`, `>`, `<=` e `>=`), três degraus conservadores do recorte externo auditável de `quebrar`/`continuar` no caminho de `sempre que`, quatro camadas conservadoras do recorte heterogêneo mínimo de `ninho` em 10.4 (`u32` e `u64` em leitura/escrita e composição mínima auditável no mesmo `ninho`), as camadas 1 e 2 conservadoras de `virar` em 10.5 (casts explícitos mínimos `u32 -> u64` e `u64 -> u32` no backend externo, sem coerções implícitas e sem sistema geral de casts) e a camada 1 conservadora/condicional de `verso` em 10.6 (literal estático em `.rodata` + tráfego opaco por slot/parâmetro, sem abrir textualidade ampla).
- A Fase 181 do Bloco 18 não abriu keyword nova nem superfície qualificada em código; ela apenas canonizou nomes de famílias públicas iniciais e manteve `colecao`/`formato` em revisão lexical explícita.
