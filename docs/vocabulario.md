# Vocabulario Pinker

Catalogo vivo de keywords fofinhas para a linguagem Pinker.
Palavras em portugues, carinhosas, sem diminutivo.
Consultar antes de implementar uma feature nova — riscar ao implementar.

---

## Keywords atuais (implementadas)

| Keyword        | Conceito         | Espirito              |
|----------------|------------------|-----------------------|
| `pacote`       | package          | organizacao           |
| `carinho`      | function         | afeto, cuidado        |
| `mimo`         | return           | presente, oferenda    |
| `talvez`       | if               | possibilidade         |
| `senao`        | else             | alternativa           |
| `sempre que`   | while            | persistencia          |
| `quebrar`      | break            | ruptura gentil        |
| `continuar`    | continue         | seguir em frente      |
| `eterno`       | const global     | permanencia           |
| `nova`         | let (declaracao) | nascimento            |
| `muda`         | mutable          | mudanca               |
| `bombom`       | integer (u64)    | doce                  |
| `logica`       | boolean          | razao                 |
| `verdade`      | true             | certeza               |
| `falso`        | false            | negacao               |
| `principal`    | main/entry       | protagonista          |
| `apelido`      | type alias       | nome alternativo      |
| `ninho`        | struct           | agrupamento acolhedor |
| `virar`        | cast explícito   | transformação visível |
| `peso`         | sizeof explícito | medida estática       |
| `alinhamento`  | alignof explícito| alinhar com cuidado   |
| `seta`         | ponteiro / ref   | direção, apontar      |
| `fragil`       | volatile         | cuidado redobrado     |
| `sussurro`     | inline asm       | fala direta mínima com máquina |
| `livre`        | freestanding/no-std (marca de unidade) | intenção de ambiente sem runtime hospedeiro implícito |
| `trazer`       | import mínimo    | convidar símbolo/módulo para perto |
| `verso`        | string / texto   | palavras como poesia                |
| `falar`        | print / saída    | expressar para o mundo              |
| `abrir`       | file open mínimo | abrir arquivo no subset de I/O      |
| `fechar`      | file close mínimo| encerrar handle de arquivo com cuidado |
| `escrever`    | file write mínimo| gravar `bombom` em arquivo aberto no subset de I/O |
| `argumento`   | argv posicional mínimo | ler argumento por índice em `--run` |
| `argumento_ou` | argv posicional com fallback mínimo | ler argumento por índice ou usar padrão em `--run` |
| `quantos_argumentos` | contagem mínima de argv | contar argv posicional repassado em `--run` |
| `tem_argumento` | presença mínima de argv | verificar existência de índice em argv no `--run` |
| `ambiente_ou` | leitura mínima de env com fallback | ler variável de ambiente por chave com padrão em `--run` |
| `diretorio_atual` | leitura mínima do cwd | obter diretório atual do processo como `verso` em `--run` |
| `caminho_existe` | existência mínima de caminho | verificar presença de caminho simples em `--run` |
| `e_arquivo` | classificação mínima de caminho | verificar se o caminho simples é arquivo em `--run` |
| `e_diretorio` | classificação mínima de diretório | verificar se o caminho simples é diretório em `--run` |
| `juntar_caminho` | composição mínima de caminho | juntar base+trecho de caminho no `--run` sem API rica |
| `tamanho_arquivo` | metadado mínimo de tamanho | obter o tamanho de arquivo regular em `--run` |
| `e_vazio` | teste mínimo de vazio | verificar se arquivo regular está vazio em `--run` |
| `criar_diretorio` | criação mínima de diretório | criar diretório simples (não recursivo) em `--run` |
| `remover_arquivo` | remoção mínima de arquivo | remover arquivo simples em `--run` (não remove diretório) |
| `remover_diretorio` | remoção mínima de diretório | remover diretório vazio simples em `--run` (não recursivo) |
| `ler_verso_arquivo` | leitura textual mínima de arquivo | ler conteúdo textual completo de handle aberto em `--run` |
| `criar_arquivo` | criação mínima de arquivo | criar arquivo vazio e retornar handle em `--run` |
| `escrever_verso` | escrita textual mínima de arquivo | sobrescrever conteúdo completo com `verso` em handle aberto no `--run` |
| `truncar_arquivo` | truncamento mínimo de arquivo | truncar para vazio um arquivo por handle aberto no `--run` |
| `sair`        | status de saída mínimo | encerrar script com código explícito |
| `nope`        | bitwise NOT textual    | negação bitwise em forma Pinker (equivale a `~`) |

---

Nota de status operacional:
- `verso` foi introduzido como tipo/literal na Fase 61.
- Recorte operacional mínimo em `--run` foi oficializado na Fase 88: variável local, passagem por chamada, retorno e `falar(verso)`.
- Operações mínimas de texto da Fase 89:
  - `juntar_verso(verso, verso) -> verso` (concatenação mínima);
  - `tamanho_verso(verso) -> bombom` (comprimento mínimo).
- Fase 90 (fechamento mínimo de indexação):
  - `indice_verso(verso, bombom) -> verso` (retorna `verso` unitário);
  - índice fora da faixa gera erro explícito em runtime.
- Fase 91 (melhoria mínima de saída):
  - `falar` passou a aceitar múltiplos argumentos;
  - separação previsível por espaço simples entre argumentos na mesma linha;
  - mistura mínima heterogênea coberta no recorte da fase (ex.: `bombom` + `verso`).
- Fase 92 (base mínima de tooling em `--run`):
  - `argumento(bombom) -> verso` para leitura posicional mínima de argv;
  - `sair(bombom)` para status explícito de saída;
  - repasse de argv via CLI com `--run ... -- <args...>`.
- Fase 93 (ergonomia mínima de argv em `--run`):
  - `quantos_argumentos() -> bombom` para contagem mínima de argv;
  - `tem_argumento(bombom) -> logica` para verificação mínima de presença por índice;
  - integração natural com `argumento(i)` sem abrir coleção/iterador amplo de argv.
- Fase 94 (fallback mínimo de argv em `--run`):
  - `argumento_ou(bombom, verso) -> verso` para obter argv posicional com valor padrão;
  - foco em scripts pequenos sem exigir sempre `tem_argumento(i)` + `talvez`;
  - sem parser de flags/subcomandos/env e sem coleção ampla de argv.
- Fase 95 (ambiente mínimo de processo em `--run`):
  - `ambiente_ou(verso, verso) -> verso` para leitura de variável de ambiente com fallback;
  - `diretorio_atual() -> verso` para leitura mínima do diretório corrente;
  - sem mutação/listagem de env vars, sem `chdir` e sem API rica de paths.
- Fase 96 (introspecção mínima de caminho em `--run`):
  - `caminho_existe(verso) -> logica` para verificação mínima de existência;
  - `e_arquivo(verso) -> logica` para classificação mínima de arquivo no mesmo recorte;
  - sem listagem de diretórios, sem `chdir`, sem globbing e sem biblioteca ampla de paths.
- Fase 97 (refinamento mínimo de caminho em `--run`):
  - `e_diretorio(verso) -> logica` para classificação complementar de diretório;
  - `juntar_caminho(verso, verso) -> verso` para composição mínima (sem canonicalização/normalização rica);
  - sem listagem de diretórios, sem `chdir`, sem globbing e sem biblioteca ampla de paths.
- Fase 98 (refinamento mínimo de arquivo em `--run`):
  - `tamanho_arquivo(verso) -> bombom` para tamanho mínimo de arquivo regular;
  - `e_vazio(verso) -> logica` para teste mínimo de vazio em arquivo regular;
  - sem timestamps/permissões/ownership, sem criação/remoção e sem biblioteca ampla de metadados/filesystem.
- Fase 99 (refinamento mínimo de mutação de filesystem em `--run`):
  - `criar_diretorio(verso) -> nulo` para criação simples de diretório (não recursiva);
  - `remover_arquivo(verso) -> nulo` para remoção simples de arquivo (não remove diretórios);
  - sem `create_dir_all`, sem remoção de diretório, sem rename/move/cópia e sem biblioteca ampla de filesystem.
- Fase 100 (refinamento mínimo complementar em `--run`):
  - `remover_diretorio(verso) -> nulo` para remoção simples de diretório vazio (não recursiva);
  - `ler_verso_arquivo(bombom) -> verso` para leitura textual integral do conteúdo de handle aberto por `abrir(...)`;
  - sem remoção recursiva, sem listagem/rename/move/cópia, sem streaming/append e sem API textual rica de arquivo.
- Fase 101 (escrita textual mínima em `--run`):
  - `escrever_verso(bombom, verso) -> nulo` para sobrescrever conteúdo textual completo no handle aberto;
  - `criar_arquivo(verso) -> bombom` para criar arquivo vazio e já devolver handle utilizável no fluxo mínimo;
  - sem append, sem streaming, sem escrita por linha, sem encoding sofisticado e sem API textual ampla de arquivo.
- Fase 102 (truncamento mínimo em `--run`):
  - `truncar_arquivo(bombom) -> nulo` para truncar o arquivo associado ao handle aberto;
  - pós-estado observável no recorte mínimo existente com `tamanho_arquivo(verso)`, `e_vazio(verso)` e `ler_verso_arquivo(bombom)`;
  - sem truncamento por caminho, sem append/streaming e sem modos ricos de arquivo.
- Fora do subset textual atual: slicing, indexação negativa e formatação/interpolação.

---

## Sugestoes para features futuras

Cada entrada marca: conceito, keyword sugerida, alternativas, e o sentimento.
Ao implementar, mover para a tabela de cima e riscar daqui.

### Tipos novos

| Conceito         | Sugestao    | Alternativas        | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| float            | `bolha`     | `brisa`, `pluma`     | leve, flutuante                     |
| ~~string / texto~~   | ~~`verso`~~     | `prosa`, `fala`      | implementado na Fase 61             |
| byte (u8)        | `grao`   | `migalha`               | pequeno mas precioso                |
| char             | `letra`     | `sinal`              | simples, direto                     |
| void explito     | `nulo`      | `vazio`              | ausencia serena                     |
| ~~ponteiro / ref~~ | ~~`seta`~~  | `olhar`, `aceno`     | implementado na Fase 48             |

### Estruturas de dados

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| ~~struct~~       | ~~`ninho`~~ | `cesta`, `colcha`    | implementado na Fase 47             |
| enum             | `leque`     | `flor`, `arco`       | variedade, abrir opcoes             |
| array            | `colar`     | `fila`, `cordao`     | sequencia encadeada                 |
| tuple            | `par`       | `laco`, `broche`     | coisas juntas                       |
| map / dicionario | `gaveta` | `cofre`, `almanaque`    | guardar com organizacao             |

### Controle de fluxo

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| for / iteracao   | `passeio`   | `ronda`, `caminho`   | caminhar por cada elemento          |
| match / switch   | `encaixe`   | `espelho`, `reflexo` | encontrar o lugar certo             |
| loop infinito    | `roda`      | `gira`, `ciranda`    | movimento sem fim                   |
| try / catch      | `amparo`    | `abrigo`, `resguardo`| protecao contra tropecos            |
| throw / error    | `tropeco`   | `susto`              | algo inesperado, mas superavel      |

### Visibilidade e modulos

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| pub / publico    | `aberto`    | `livre`              | transparencia                       |
| private          | `segredo`   | `intimo`             | protecao, guardado                  |
| ~~import / use~~ | ~~`trazer`~~| `buscar`, `chamar`   | implementado na Fase 60             |
| module           | `canto`     | `ala`, `sala`        | um espaco proprio                   |
| self             | `eu`        |                      | identidade                          |

### Orientacao a tipos

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| trait/interface  | `molde`     | `jeitinho`, `feitio` | forma de ser                        |
| impl             | `vestir`    | `assumir`            | vestir um molde, incorporar         |
| ~~type alias~~   | ~~`apelido`~~ | `alcunha`          | implementado na Fase 45             |
| ~~cast / as~~    | ~~`virar`~~ | `tornar`             | implementado na Fase 50             |
| ~~sizeof~~       | ~~`peso`~~  | `tamanho`            | implementado na Fase 51             |
| generic          | `qualquer`  | `molde livre`        | flexibilidade                       |

### Programacao de sistemas

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| unsafe           | `coragem`   | `ousadia`            | saber que ha risco e seguir         |
| ~~volatile~~     | ~~`fragil`~~| `instavel`           | implementado na Fase 52             |
| ~~align/alinhamento~~ | ~~`alinhamento`~~ | `ajuste`     | implementado na Fase 51             |
| static           | `raiz`      | `ancora`             | fixo, inamovivel                    |
| ~~inline asm~~   | ~~`sussurro`~~ | `sopro`          | implementado na Fase 56             |
| ~~freestanding / no-std~~ | ~~`livre`~~ | `desnudo`     | implementado na Fase 57             |
| alloc            | `reserva`   | `guardar`            | separar espaco com intencao         |
| free / drop      | `soltar`    | `liberar`, `largar`  | deixar ir                           |

### Concorrencia (futuro distante)

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| async            | `sonho`     | `promessa`           | algo que vai acontecer              |
| await            | `espera`    | `aguardo`            | paciencia                           |
| spawn / thread   | `broto`     | `semente`            | algo que nasce e cresce em paralelo |
| mutex / lock     | `tranca`    | `chave`, `cadeado`   | protecao de acesso                  |
| channel          | `ponte`     | `tunel`, `passagem`  | conexao entre partes                |

### I/O

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| ~~print~~        | ~~`falar`~~ | `contar`, `dizer`    | implementado na Fase 62             |
| ~~read / input~~ | ~~`ouvir`~~ | `escutar`            | implementado na Fase 85 como intrínseca `ouvir()` |
| ~~file open~~        | ~~`abrir`~~     |                      | implementado na Fase 86 (`abrir("caminho")`)                 |
| ~~file close~~       | ~~`fechar`~~    |                      | implementado na Fase 86 (`fechar(handle)`)                |
| ~~write to file~~    | ~~`escrever`~~  | `registrar`          | implementado na Fase 87 (`escrever(handle, bombom)`) |
| ~~argv posicional~~  | ~~`argumento`~~ | `parametro`          | implementado na Fase 92 (`argumento(i)`) |
| ~~exit/status~~      | ~~`sair`~~      | `encerrar`           | implementado na Fase 92 (`sair(codigo)`) |

---

## Principios do vocabulario

1. **Sem diminutivo** — `ninho`, nao `ninhozinho`
2. **Portugues natural** — palavras que existem e fazem sentido sozinhas
3. **Sentimento claro** — cada keyword evoca algo afetuoso ou poetico
4. **Sem ambiguidade tecnica** — a palavra deve ser intuitiva para quem le codigo
5. **Consistencia** — manter o tom da Pinker: gentil, acolhedor, firme

---

## Como usar este documento

- Antes de implementar uma feature, consultar a sugestao aqui
- Se a sugestao nao servir, anotar a alternativa escolhida
- Ao implementar, mover a linha para "Keywords atuais"
- Novas ideias podem ser adicionadas a qualquer momento
- Se uma keyword for rejeitada, marcar com ~~riscado~~ e anotar o motivo
