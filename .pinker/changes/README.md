# `.pinker/changes/` — acervo histórico de manifestos de mudança

Este diretório guarda os **manifestos estruturais de mudança** da Trama Pinker
(especificação, seções 15 e 17). Cada arquivo `pr-N.yaml` é a fonte estrutural
de uma mudança antiga, derivada do bloco ` ```pinker-change ` que o corpo do PR
correspondente carregava enquanto essa autoria existiu.

## Acervo fechado

A Issue #698 encerrou a cobrança universal do bloco `pinker-change`. Desde esse
corte:

- **nenhum PR novo deve manifesto.** A ausência do bloco não é erro, não é
  achado de preflight e não reprova portão remoto algum;
- **nenhum manifesto novo é criado.** O `pink` não possui mais caminho de
  autoria: ele só lê o acervo aceito;
- **nada é apagado por isso.** Os manifestos, as exceções históricas e o
  histórico mecânico derivado continuam versionados e continuam verificados.

## Intervalo histórico preservado

A cobertura obrigatória é finita e já está fechada:

- marco documental: PR #330, exclusivo (`.pinker/doc.toml`);
- fim do intervalo: merge do PR #410
  (`1df2a6afff423bc7564e7322880e24af683f6089`).

Todo PR de merge alcançável nesse intervalo tem exatamente uma forma de
cobertura aceita: um manifesto `pr-N.yaml` ou uma entrada em
`historical-exceptions-v1.yaml`. Merges posteriores ao cutover ficam fora da
obrigação, e isso não é lacuna nem corrupção — é o intervalo sendo finito.
`tests/change_history_coverage_tests.rs` é o gate dessa propriedade.

## Contrato dos arquivos preservados

- **Um arquivo por PR:** `pr-<numero>.yaml`, no formato
  `../schemas/change-v1.schema.json`.
- **Imutável:** os bytes aceitos não são reescritos.
- **Sem backfill:** manifesto de PR anterior ou igual ao marco é recusado na
  leitura com `E-DOC-BASELINE`.
- **Fonte para geração derivada:** `index.jsonl` e as projeções documentais são
  compilados a partir daqui por `pink doc sincronizar`, nunca inventados.
- **Verificável:** `pink doc verificar` relê os manifestos, revalida o schema e
  compara o histórico mecânico; adulteração falha fechado.
