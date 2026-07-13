# Expandir implementações históricas da Pinker

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Este documento é a referência operacional para transformar implementações históricas **mínimas**, **pequenas**, **conservadoras** ou explicitamente limitadas por **recorte** em implementações adultas. Ele nasce no Bloco 20, após o encerramento do Eixo B, para apoiar a retomada do **Eixo A — linguagem** com um padrão novo: a próxima evolução da Pinker deve mirar recursos utilizáveis em profundidade, com lowering nativo desde o início, sem repetir automaticamente o padrão antigo de menor recorte auditável.

## 1. Papel deste documento

`docs/expandir.md` não é histórico e não substitui o roadmap. Seu papel é:

- localizar a dívida técnica criada por entregas deliberadamente pequenas;
- explicar como decidir se uma implementação antiga deve ser expandida;
- definir critérios de expansão para fases futuras;
- preservar a honestidade histórica: uma fase antiga continua sendo o que foi, mesmo quando uma fase nova a amplia;
- impedir que a palavra “mínimo” continue funcionando como padrão automático após o Eixo B;
- servir como checklist antes de abrir uma fase de expansão de linguagem, runtime, backend ou biblioteca.

## 2. Onde este documento fica na arquitetura documental

- **Ordem ativa:** `docs/roadmap.md` e `docs/roadmap/blocos/bloco_20.md` continuam definindo a sequência oficial.
- **Estado corrente:** `docs/handoff_codex.md` continua dizendo onde o projeto parou e qual é o próximo passo.
- **Histórico factual:** `docs/history.md` aponta para `docs/history/indice.md` e para os shards em `docs/history/`.
- **Inventário futuro:** `docs/future.md` continua reunindo lacunas técnicas que não são necessariamente a próxima etapa.
- **Referência de expansão:** este documento reúne critérios para elevar implementações existentes, sem virar backlog paralelo.

Leitura canônica: quando uma fase futura ampliar uma implementação antiga, a fase deve continuar obedecendo ao roadmap e ao histórico. Este documento ajuda a definir o nível de ambição, os limites e as evidências necessárias.

## 3. Diagnóstico quantitativo do padrão antigo

Foi feita uma contagem sobre os títulos das fases históricas em `docs/history/phases/*.md`, usando os termos:

- `mínim` / `minim`;
- `pequen`;
- `conservador` / `conservadora`;
- `recorte`.

Resultado da inspeção local:

| Métrica | Valor |
|---|---:|
| Títulos de fase inspecionados | 214 |
| Títulos marcados por padrão mínimo/conservador/recorte/pequeno | 113 |
| Proporção aproximada | 52,8% |

Leitura honesta:

- a contagem é **heurística**, baseada em título de fase;
- ela subconta casos em que o corpo da fase fala em limites mínimos, mas o título não;
- ela superconta alguns casos em que o termo aparece como nota de fechamento, não como toda a essência da implementação;
- ainda assim, ela confirma um fato documental importante: mais da metade dos marcos funcionais nomeados carregam explicitamente a marca de entrega mínima, conservadora, pequena ou por recorte.

## 4. Onde estão essas implementações

A concentração não está em um único subsistema. O padrão aparece em quase toda a história operacional:

### 4.1 Pipeline inicial e base de execução

Exemplos históricos:

- interpretador mínimo com `--run`;
- CI mínima e MSRV;
- ABI textual mínima interna;
- integração externa mínima com assembler/linker;
- inline asm mínimo;
- boot entry/linker script textual mínimo;
- primeiro kernel mínimo experimental.

Localização principal:

- `src/main.rs` e pipeline CLI;
- `src/interpreter.rs`;
- `src/backend_s.rs`;
- `src/abstract_machine.rs`;
- testes em `tests/*` relacionados a backend e CLI;
- exemplos versionados em `examples/`.

### 4.2 Memória, ponteiros e compostos

Exemplos históricos:

- representação mínima de ponteiro no runtime;
- leitura/escrita indireta em subset;
- aritmética mínima de ponteiro;
- acesso operacional mínimo a campo de `ninho`;
- arrays em recorte conservador;
- `fragil` com efeito operacional mínimo;
- `virar` em recortes controlados.

Localização principal:

- `src/ast.rs`;
- `src/parser.rs`;
- `src/semantic.rs`;
- `src/layout.rs`;
- `src/interpreter.rs`;
- `src/cfg_ir.rs` e validadores;
- `src/backend_s.rs`;
- `tests/semantic_tests.rs`, `tests/interpreter_tests.rs`, `tests/backend_s_external_toolchain_tests.rs`.

### 4.3 Texto, arquivos, caminho e ambiente

Exemplos históricos:

- `verso` operacional mínimo;
- operações mínimas de texto;
- leitura/escrita/truncamento mínimo de arquivo;
- introspecção mínima de caminho;
- ambiente mínimo com fallback;
- leitura textual direta por caminho em recorte conservador.

Localização principal:

- `src/interpreter.rs`;
- `src/semantic.rs`;
- `src/backend_s.rs` e runtime nativo após o Eixo B;
- `runtime/pinker_rt/src/lib.rs` para o caminho nativo;
- `docs/inventario_intrinsecas.md`;
- `MANUAL.md`;
- exemplos `examples/fase8x` a `examples/fase10x` e posteriores.

### 4.4 Processos e linguagem-cola

Exemplos históricos:

- execução mínima de processo externo;
- captura mínima de stdout/stderr;
- entrada mínima por stdin textual;
- pipe mínimo entre dois processos;
- argv explícito mínimo;
- REPL mínimo.

Localização principal:

- `src/interpreter.rs`;
- `src/repl.rs`;
- `runtime/pinker_rt/src/lib.rs` para paridade nativa do Eixo B;
- `tests/interpreter_tests.rs`;
- `tests/backend_nativo_tests.rs`;
- `examples/fase161*` a `examples/fase177*`.

### 4.5 Coleções, dados estruturados e aleatoriedade

Exemplos históricos:

- `lista<bombom>` mínima;
- `mapa<verso,bombom>` mínimo;
- iteração confortável mínima;
- CSV mínimo;
- JSON plano mínimo;
- tempo Unix mínimo;
- aleatoriedade básica com semente explícita.

Localização principal:

- `src/semantic.rs`;
- `src/interpreter.rs`;
- `runtime/pinker_rt/src/lib.rs`;
- `tests/semantic_tests.rs`;
- `tests/interpreter_tests.rs`;
- `tests/backend_nativo_tests.rs`;
- `docs/inventario_intrinsecas.md`;
- `docs/examples_index.md`.

### 4.6 Módulos, famílias e superfície documental

Exemplos históricos:

- exportação mínima de `ninho`;
- exportação mínima de `apelido`;
- uso qualificado mínimo;
- importação mínima por família;
- norma visual mínima;
- política mínima de aliases e nomes curtos.

Localização principal:

- `src/parser.rs`;
- `src/semantic.rs`;
- `docs/style.md`;
- `docs/familias/*.md`;
- `docs/familias_tematicas.md`;
- `docs/vocabulario.md`.

### 4.7 Bloco 20 antes do Eixo B

Exemplos históricos:

- `leque` mínimo;
- pattern matching mínimo;
- generics mínimos sobre `lista<T>`.

Localização principal:

- `src/ast.rs`;
- `src/parser.rs`;
- `src/semantic.rs`;
- `src/interpreter.rs`;
- `src/backend_s.rs`;
- `runtime/pinker_rt/src/lib.rs`;
- `examples/fase208*` a `examples/fase211*`;
- `tests/backend_nativo_tests.rs` após a paridade do Eixo B.

## 5. Novo padrão de implementação após o Eixo B

Daqui em diante, uma fase funcional nova não deve ser descrita como “menor recorte auditável” por padrão. O padrão esperado passa a ser:

1. **superfície utilizável**, não só prova de conceito;
2. **semântica consistente**, com erros claros e regras documentadas;
3. **interpretação e nativo juntos**, sem recurso interpreter-only;
4. **testes de paridade** quando houver comportamento observável;
5. **exemplos realistas**, além do caso canônico mínimo;
6. **limites explícitos**, mas limites não usados como desculpa para entregar um esqueleto;
7. **compatibilidade histórica preservada**, sem reescrever o passado nem remover recurso antigo sem plano de migração.

A palavra “mínimo” ainda pode aparecer quando o domínio realmente exigir contenção, mas deve ser exceção justificada, não hábito.

## 6. O que significa “expandir”

Expandir uma implementação antiga não é apenas adicionar uma função a mais. Uma expansão adulta deve avaliar, no mínimo:

### 6.1 Superfície

- O usuário consegue resolver um problema real sem gambiarras?
- A API tem nomes coerentes com a Pinker atual?
- Existe duplicação legada que precisa de ponte, alias, depreciação ou documentação?
- A expansão encaixa no vocabulário e nas famílias temáticas existentes?

### 6.2 Semântica

- Os tipos aceitos são gerais o suficiente para o domínio?
- Os erros são estáveis, úteis e testados?
- Há interação com mutabilidade, retorno, controle de fluxo, módulos, `leque`, coleções ou `verso`?
- A expansão preserva compatibilidade com programas antigos?

### 6.3 Runtime interpretado

- O comportamento no `--run` é determinístico onde precisa ser?
- Casos de erro operacional são claros?
- Handles, memória e estruturas internas têm disciplina de vida suficiente para o estágio atual?
- Existe cobertura para fluxo composto, não só chamada isolada?

### 6.4 Backend nativo

- O recurso baixa para `.s` e linka com `pinker_rt` quando necessário?
- O comportamento nativo tem paridade de stdout, retorno e erro relevante com o interpretador?
- O runtime nativo expõe ABI C estável para a nova superfície?
- O recurso não cria nova lacuna estrutural interpreter-only?

### 6.5 Testes e exemplos

- Há testes positivos e negativos no nível certo?
- Há exemplo versionado canônico?
- Há exemplo composto que pareça uso real?
- O manifesto de paridade nativa deve ser atualizado?
- `docs/examples_index.md` foi atualizado se houver exemplo novo?

### 6.6 Documentação

- `README.md` e `MANUAL.md` precisam refletir a superfície pública?
- `docs/inventario_intrinsecas.md` precisa mudar?
- `docs/future.md` deve remover ou reclassificar a lacuna entregue?
- `docs/handoff_codex.md` e `docs/history/phases/*.md` registram o novo estado?

## 7. Classificação de alvos de expansão

Para evitar expansão difusa, cada proposta deve cair em uma classe:

| Classe | Descrição | Exemplo de alvo |
|---|---|---|
| Completar superfície | Tornar uma API antiga realmente prática | processos com múltiplos argv, cwd, env, timeout |
| Generalizar tipos | Tirar restrição artificial de tipo | arrays/listas/mapas além de combinações monomorfizadas |
| Unificar interpretador+nativo | Fechar lacuna entre modos | qualquer recurso que ainda execute só em `--run` |
| Elevar semântica | Transformar desugaring/atalho em regra sólida | pattern matching com guards e padrões aninhados |
| Robustecer runtime | Dar disciplina operacional a handles/memória | liberação, erros de handle, invariantes de coleção |
| Tornar documentação adulta | Trocar “recorte mínimo” por contrato presente | manual, inventário, exemplos realistas |

## 8. Critérios de pronto para uma expansão adulta

Uma fase de expansão só deve ser considerada pronta quando cumprir os critérios aplicáveis:

- compila e passa `make ci`;
- tem comportamento no interpretador e no nativo, quando o recurso é executável;
- tem testes unitários ou integração cobrindo o caminho feliz;
- tem testes negativos para pelo menos os erros principais;
- tem exemplo versionado se a superfície for pública;
- atualiza documentação pública quando a superfície muda;
- registra histórico factual sem apagar a descrição da fase antiga;
- declara limites remanescentes com precisão;
- remove ou reclassifica a dívida em `docs/future.md` quando aplicável;
- não cria novo “modo especial” sem justificativa.

## 9. Como usar antes de abrir uma fase

Checklist de abertura:

1. localizar a implementação antiga no histórico e no código;
2. identificar por que ela foi mínima/conservadora;
3. decidir se a limitação ainda é válida;
4. escolher a classe de expansão;
5. definir superfície pública pretendida;
6. desenhar comportamento no interpretador;
7. desenhar lowering nativo e chamadas de runtime;
8. listar testes e exemplos necessários;
9. listar docs afetados;
10. confirmar se a expansão respeita a ordem ativa do Bloco 20.

Se a proposta não couber na ordem ativa, ela deve ir para `docs/future.md` ou aguardar abertura formal, não furar a trilha.

## 10. Aplicação imediata ao Bloco 20, Eixo A

A retomada atual é o **Eixo A, Faixa 1, item 5 — error handling estruturado**. Pelo novo padrão, esse item não deve nascer apenas como sintaxe simbólica ou caso único. Uma implementação compatível com este documento deve considerar:

- representação de resultado/erro que sirva ao compilador escrito em Pinker;
- propagação ou recuperação explícita sem abortar o processo por padrão;
- integração com `leque`, `encaixe`, funções e retorno;
- diagnósticos úteis;
- execução no interpretador e no backend nativo;
- exemplos de parsing/compilação de brinquedo ou fluxo real de arquivo/processo quando aplicável;
- testes de paridade nativa para o comportamento observável.

A decisão lexical exata (`tentar/pegar`, `Resultado`, operador de propagação ou combinação) ainda pertence à fase funcional que abrir o item. Este documento define o patamar, não congela a sintaxe.

## 11. Cuidados históricos

Ao expandir uma implementação antiga:

- não renomear a fase antiga como se ela tivesse entregue mais do que entregou;
- não apagar “mínimo” do histórico factual;
- não tratar dívida histórica como erro: o padrão mínimo foi útil para chegar ao Eixo B;
- registrar a expansão como nova fase numerada;
- manter compatibilidade ou declarar plano explícito de migração;
- evitar reescrever a timeline para parecer que o projeto sempre teve o padrão novo.

## 12. Relação com `docs/phases.md`

`docs/phases.md` não existe no workspace atual. A história canônica está em:

- `docs/history.md`;
- `docs/history/indice.md`;
- `docs/history/phases/*.md`;
- `docs/history/documentation/*.md`;
- `docs/history/hotfixes/*.md`;
- `docs/history/parallel_phases/*.md`.

Referências futuras a “phases.md” devem ser tratadas como legado documental. Não recriar o arquivo por inércia; atualizar a referência para o sistema histórico shardado.
