# `.pinker/changes/` — manifestos versionados de mudança

Este diretório guarda os **manifestos estruturais de mudança** da Trama Pinker
(especificação, seções 15 e 17). Cada arquivo `pr-N.yaml` é a fonte estrutural
de uma mudança já importada, derivada do bloco ` ```pinker-change ` do corpo do
PR correspondente.

## Contrato

- **Um arquivo por PR:** `pr-<numero>.yaml`.
- **Idempotente por número de PR:** reimportar o mesmo PR não deve duplicar
  nem corromper o manifesto.
- **Revisável e versionado:** o manifesto é lido em revisão de código e vive no
  histórico Git como qualquer outro arquivo.
- **Independente de edições futuras no PR:** uma vez importado, o manifesto não
  é reescrito por alterações posteriores no corpo do PR.
- **Fonte para geração derivada:** índices, históricos mecânicos e tabelas são
  compilados a partir daqui, nunca inventados.

## Marco (anti-retroatividade)

Somente PRs **posteriores** ao marco definido em `.pinker/doc.toml`
(`baseline_pr = 330`, exclusivo) podem gerar manifesto. A tentativa de importar
um PR anterior ou igual ao marco falha com o código de erro `E-DOC-BASELINE` e
**não** cria arquivo algum aqui. Não há backfill automático de PRs antigos.

## Esquema

O formato de cada manifesto segue `../schemas/change-v1.schema.json`.
