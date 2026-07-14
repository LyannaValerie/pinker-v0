# Guardiao Pinker

Auditor minimo escrito em Pinker para checar contratos documentais e operacionais do proprio repositorio.

## Comando

```bash
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .
```

## Checagens da primeira versao

- arquivos canônicos principais existem;
- `docs/phases.md` continua ausente;
- `README.md` nao volta a crescer alem do limite operacional inicial;
- arquivos principais nao contem marcadores de conflito Git;
- `docs/handoff_codex.md` menciona o estado corrente esperado do Bloco 20.

## Contrato de resultado

- retorno `0`: contratos verificados;
- retorno `1`: pelo menos uma checagem falhou.

Esta aplicacao e intencionalmente pequena. O objetivo e transformar dores reais do desenvolvimento da Pinker em dogfooding continuo, sem depender de ferramentas externas para toda auditoria editorial.

