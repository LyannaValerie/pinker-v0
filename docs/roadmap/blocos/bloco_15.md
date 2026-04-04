# Bloco 15 — processos e integração sistêmica

## Status
Encerrado.

## Tese
Permitir que a Pinker execute processos externos e se integre ao ambiente de sistema como linguagem-cola auditável.

## Dependências
Bloco 14 concluído.

## Escada interna
- execução de processos externos mínima;
- captura de stdout;
- captura mínima de stderr;
- entrada mínima por stdin textual;
- pipe mínimo entre dois processos.

## Estado factual atual
Bloco encerrado por suficiência conservadora após as Fases 161–166; a Doc-30 apenas refinou a escada interna antes do fechamento formal da Doc-31.

## Limites explícitos
- sem shell amplo;
- sem PTY, job control ou sessão interativa;
- sem API adulta de subprocessos.

## Relação com o histórico
- Refino da escada interna em `docs/history/documentation/001a050.md` (Doc-30).
- Fechamento formal em `docs/history/documentation/001a050.md` (Doc-31).
- Execução factual preservada em `docs/history/phases/151a200.md`.
