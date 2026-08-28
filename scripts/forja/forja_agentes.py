#!/usr/bin/env python3
"""Autoridade única de layout de Task da Forja sobre `agentes/`.

Uma Task tem UM root físico observado. Todo recurso descartável exclusivo da
Task mora dentro dele. A mesma tupla `RECURSOS` alimenta provisionamento,
observação, selagem e retirada — é isso que torna
`PROVISIONER_SET == FINALIZER_SET` uma propriedade estrutural, e não uma
promessa de documentação.

Identidade lógica e caminho físico são coisas diferentes:

    TASK_ID   identidade estável, vinda do observador canônico
    TASK_ROOT root físico observado, alocado por slot livre

O slot NÃO é derivado do `TASK_ID`. Nenhum consumidor pode reconstruir o
`TASK_ROOT` por concatenação; ele é descoberto lendo o vínculo gravado dentro
de cada slot.

Toda destruição é fail-closed: canonicaliza, prova contenção, recusa symlink em
qualquer componente, recusa mountpoint, recusa processo vivo, recusa estado não
elegível, e nunca segue link simbólico ao apagar.
"""

from __future__ import annotations

import argparse
import errno
import grp
import json
import os
import pwd
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

VERSION = "1.0.0"
SCHEMA_LAYOUT = "forja-agentes-layout-v1"
SCHEMA_BINDING = "forja-agentes-binding-v1"

# ---------------------------------------------------------------------------
# Autoridades fixas. Não são configuráveis em produção; o override existe
# apenas sob modo de teste explícito, para que a suíte não precise de root nem
# do host real.
# ---------------------------------------------------------------------------

CANONICAL_MAIN_PADRAO = "/pinker/repo/pinker-v0"
AGENTES_DIRNAME = "agentes"
BINDING_FILENAME = "task.json"
OBSERVADOR_CANONICO = "/opt/pinker/bin/forja-contexto-ativo"
GRUPO_AGENTES = "pinker-agents"

MODO_DIR = 0o2770
MODO_ARQ = 0o660

TASK_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
SLOT_RE = re.compile(r"^a[0-9]{2,4}$")

AMBIENTE_LIMPO = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C"}

# Estados de lifecycle da Task no root físico.
ESTADOS = ("ACTIVE", "REVIEW", "FIX_REQUIRED", "SEALED", "RETIREABLE")
ESTADOS_DESTRUTIVEIS = ("SEALED", "RETIREABLE")


class Classe:
    """Classe de retenção de um recurso.

    EPHEMERAL   reproduzível; o EXECUTION_SEAL pode recuperar.
    DURABLE     necessário à revisão/recuperação; só o TASK_RETIRE remove.
    """

    EPHEMERAL = "EPHEMERAL"
    DURABLE = "DURABLE"


@dataclass(frozen=True)
class Recurso:
    nome: str
    subpath: str
    classe: str
    papel: str
    exporta: str | None = None


# A ÚNICA lista. Provisiona, observa, sela e retira a partir daqui.
RECURSOS: tuple[Recurso, ...] = (
    Recurso("worktree", "worktree", Classe.DURABLE, "git worktree da Task", "TASKDIR"),
    Recurso("target", "target", Classe.EPHEMERAL, "CARGO_TARGET_DIR", "CARGO_TARGET_DIR"),
    Recurso("cache", "cache", Classe.EPHEMERAL, "cache local da Task", None),
    Recurso("tmp", "tmp", Classe.EPHEMERAL, "TMPDIR da Task", "TMPDIR"),
    Recurso("scratch", "scratch", Classe.EPHEMERAL, "rascunho descartável", "FORJA_SCRATCH"),
    Recurso("logs", "logs", Classe.EPHEMERAL, "logs de execução", None),
    Recurso("memory", "memory", Classe.DURABLE, "memória factual JSON/JSONL", "FORJA_TASK_MEMORY"),
    Recurso("state", "state", Classe.DURABLE, "checkpoint/estado de retomada", "FORJA_TASK_STATE"),
    Recurso("artifacts", "artifacts", Classe.DURABLE, "evidência e artefatos", "FORJA_TASK_ARTIFACTS"),
)

# Recursos genuinamente compartilhados. Ficam FORA do task root de propósito e
# são declarados para que ninguém os esconda dentro de uma Task.
COMPARTILHADOS: tuple[dict[str, str], ...] = (
    {
        "nome": "cargo-registry",
        "path": os.path.expanduser("~/.cargo"),
        "ownership": "per-agent",
        "lifecycle": "SHARED_NOT_TASK_SCOPED",
    },
    {
        "nome": "book",
        "path": "/book",
        "ownership": "pinker-agents",
        "lifecycle": "SHARED_NOT_TASK_SCOPED",
    },
    {
        "nome": "toolchains",
        "path": "/pinker/sources",
        "ownership": "root",
        "lifecycle": "SHARED_NOT_TASK_SCOPED",
    },
)


# O contrato se valida a si mesmo na importação: um `subpath` com traversal
# nunca chega a rodar, em vez de ser pego só no gate.
# Um recurso declarado "compartilhado" com caminho relativo resolveria contra o
# cwd e escaparia da guarda que impede escondê-lo dentro de um task root.
for _c in COMPARTILHADOS:
    if not os.path.isabs(_c["path"]):
        raise RuntimeError(f"contrato inválido: recurso compartilhado sem caminho absoluto: {_c}")
del _c

for _r in RECURSOS:
    if os.path.isabs(_r.subpath) or os.path.normpath(_r.subpath).startswith(".."):
        raise RuntimeError(f"contrato inválido: recurso {_r.nome} escapa do task root: {_r.subpath}")
    if os.path.normpath(_r.subpath) != _r.subpath:
        raise RuntimeError(f"contrato inválido: subpath não normalizado: {_r.subpath}")
del _r


class ForjaError(Exception):
    def __init__(self, status: str, mensagem: str):
        super().__init__(mensagem)
        self.status = status
        self.mensagem = mensagem


EXIT_OK = 0
EXIT_USO = 2
EXIT_RECUSADO = 20
EXIT_FALHA = 21
EXIT_NAO_ENCONTRADO = 4


# ---------------------------------------------------------------------------
# Raiz canônica
# ---------------------------------------------------------------------------


def canonical_main() -> Path:
    override = os.environ.get("FORJA_AGENTES_TEST_MAIN")
    if override:
        if os.environ.get("FORJA_AGENTES_TEST_MODE") != "1":
            raise ForjaError("DENIED", "FORJA_AGENTES_TEST_MAIN exige FORJA_AGENTES_TEST_MODE=1")
        return Path(override)
    if any(k.startswith("FORJA_AGENTES_TEST_") for k in os.environ) and os.environ.get(
        "FORJA_AGENTES_TEST_MODE"
    ) != "1":
        raise ForjaError("DENIED", "override de teste sem FORJA_AGENTES_TEST_MODE=1")
    return Path(CANONICAL_MAIN_PADRAO)


def agentes_root() -> Path:
    return canonical_main() / AGENTES_DIRNAME


def exigir_raizes() -> tuple[Path, Path]:
    main = canonical_main()
    if not main.is_dir():
        raise ForjaError("DENIED", f"checkout canônico ausente: {main}")
    raiz = agentes_root()
    if not raiz.is_dir():
        raise ForjaError("DENIED", f"raiz de agentes ausente: {raiz}")
    if raiz.is_symlink() or main.is_symlink():
        raise ForjaError("DENIED", "raiz canônica ou de agentes é symlink")
    return main, raiz


# ---------------------------------------------------------------------------
# Segurança de caminho
# ---------------------------------------------------------------------------


def sem_symlink_em_componentes(caminho: Path, base: Path) -> None:
    """Prova que nenhum componente entre `base` e `caminho` é symlink."""
    base = Path(os.path.abspath(base))
    caminho = Path(os.path.abspath(caminho))
    try:
        rel = caminho.relative_to(base)
    except ValueError as exc:
        raise ForjaError("DENIED", f"caminho escapa da base: {caminho}") from exc
    if base.is_symlink():
        raise ForjaError("DENIED", f"base é symlink: {base}")
    atual = base
    for parte in rel.parts:
        atual = atual / parte
        try:
            st = atual.lstat()
        except FileNotFoundError:
            return
        if stat.S_ISLNK(st.st_mode):
            raise ForjaError("DENIED", f"componente é symlink: {atual}")


def mountpoints() -> set[Path]:
    resultado: set[Path] = set()
    try:
        linhas = Path("/proc/self/mountinfo").read_text(encoding="utf-8").splitlines()
    except OSError:
        return resultado
    for linha in linhas:
        campos = linha.split()
        if len(campos) >= 5:
            bruto = re.sub(r"\\([0-7]{3})", lambda m: chr(int(m.group(1), 8)), campos[4])
            resultado.add(Path(bruto))
    return resultado


def sem_mount_interno(raiz: Path) -> None:
    raiz_abs = Path(os.path.abspath(raiz))
    for mp in mountpoints():
        if mp == raiz_abs or str(mp).startswith(str(raiz_abs) + os.sep):
            raise ForjaError("BLOCKED_BY_MOUNT", f"mountpoint dentro do root: {mp}")


def processos_no_root(raiz: Path) -> list[dict[str, Any]]:
    """Processos cujo cwd, exe ou root aponta para dentro de `raiz`."""
    prefixo = str(Path(os.path.abspath(raiz))) + os.sep
    achados: list[dict[str, Any]] = []
    proc = Path("/proc")
    if not proc.is_dir():
        return achados
    for entrada in proc.iterdir():
        if not entrada.name.isdigit():
            continue
        for campo in ("cwd", "exe", "root"):
            try:
                alvo = os.readlink(entrada / campo)
            except OSError:
                continue
            if alvo == str(raiz).rstrip(os.sep) or alvo.startswith(prefixo):
                try:
                    comm = (entrada / "comm").read_text(encoding="utf-8").strip()
                except OSError:
                    comm = "?"
                achados.append({"pid": int(entrada.name), "field": campo, "target": alvo, "comm": comm})
                break
    return achados


def remover_arvore_sem_seguir_links(raiz: Path) -> tuple[int, int]:
    """Remove `raiz` inteira sem jamais atravessar um symlink.

    Usa descritores de diretório e `os.unlink(..., dir_fd=)` para que a
    resolução aconteça relativa ao fd já aberto, e não ao caminho textual, que
    poderia ser trocado por um symlink no meio da operação.
    """
    bytes_removidos = 0
    arquivos_removidos = 0

    def descer(fd_pai: int, nome: str) -> None:
        nonlocal bytes_removidos, arquivos_removidos
        st = os.lstat(nome, dir_fd=fd_pai)
        if stat.S_ISDIR(st.st_mode):
            fd = os.open(nome, os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY, dir_fd=fd_pai)
            try:
                for filho in os.listdir(fd):
                    descer(fd, filho)
            finally:
                os.close(fd)
            os.rmdir(nome, dir_fd=fd_pai)
        else:
            # symlink, arquivo regular, socket, fifo: unlink direto, nunca seguido
            if stat.S_ISREG(st.st_mode):
                bytes_removidos += st.st_size
            arquivos_removidos += 1
            os.unlink(nome, dir_fd=fd_pai)

    pai = raiz.parent
    fd_pai = os.open(str(pai), os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY)
    try:
        descer(fd_pai, raiz.name)
    finally:
        os.close(fd_pai)
    return bytes_removidos, arquivos_removidos


def medir(caminho: Path) -> tuple[int, int]:
    total = 0
    arquivos = 0
    if not caminho.exists() and not caminho.is_symlink():
        return 0, 0
    for base, dirs, nomes in os.walk(caminho, followlinks=False):
        for nome in nomes:
            p = os.path.join(base, nome)
            try:
                st = os.lstat(p)
            except OSError:
                continue
            if stat.S_ISREG(st.st_mode):
                total += st.st_size
            arquivos += 1
    return total, arquivos


# ---------------------------------------------------------------------------
# Identidade
# ---------------------------------------------------------------------------


def agente_corrente() -> str:
    override = os.environ.get("FORJA_AGENTES_TEST_AGENT")
    if override and os.environ.get("FORJA_AGENTES_TEST_MODE") == "1":
        return override
    return pwd.getpwuid(os.getuid()).pw_name


def validar_task(valor: str) -> str:
    if not isinstance(valor, str) or not TASK_RE.fullmatch(valor):
        raise ForjaError("DENIED", "task_id inválido: use [A-Za-z0-9._-] iniciando por alfanumérico")
    if ".." in valor or "/" in valor:
        raise ForjaError("DENIED", "task_id contém traversal")
    return valor


def task_do_observador() -> str:
    """Recupera o TASK_ID do observador canônico do active context."""
    override = os.environ.get("FORJA_AGENTES_TEST_TASK")
    if override and os.environ.get("FORJA_AGENTES_TEST_MODE") == "1":
        return validar_task(override)
    agente = agente_corrente()
    if not Path(OBSERVADOR_CANONICO).exists():
        raise ForjaError("DENIED", f"observador canônico ausente: {OBSERVADOR_CANONICO}")
    proc = subprocess.run(  # noqa: S603 - argv fixo, sem shell
        [OBSERVADOR_CANONICO, "show", "--agent", agente],
        capture_output=True,
        text=True,
        env=AMBIENTE_LIMPO,
        shell=False,
        check=False,
    )
    saida = proc.stdout.strip() or proc.stderr.strip()
    try:
        relatorio = json.loads(saida) if saida else {}
    except json.JSONDecodeError as exc:
        raise ForjaError("DENIED", "observador canônico devolveu saída ilegível") from exc
    contexto = relatorio.get("context")
    if not contexto:
        raise ForjaError("DENIED", "nenhum active context publicado; nada a observar")
    if contexto.get("agent") != agente:
        raise ForjaError("DENIED", "active context não pertence ao chamador")
    return validar_task(contexto.get("task", ""))


def resolver_task(arg: str | None) -> str:
    return validar_task(arg) if arg else task_do_observador()


# ---------------------------------------------------------------------------
# Vínculo TASK_ID <-> slot (observado, nunca concatenado)
# ---------------------------------------------------------------------------


def ler_binding(slot_dir: Path) -> dict[str, Any] | None:
    arquivo = slot_dir / BINDING_FILENAME
    try:
        st = arquivo.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(st.st_mode):
        return None
    try:
        dados = json.loads(arquivo.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    if not isinstance(dados, dict) or dados.get("schema") != SCHEMA_BINDING:
        return None
    return dados


def escrever_binding(slot_dir: Path, dados: dict[str, Any]) -> None:
    arquivo = slot_dir / BINDING_FILENAME
    tmp = slot_dir / f".{BINDING_FILENAME}.tmp"
    tmp.write_text(json.dumps(dados, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    os.chmod(tmp, MODO_ARQ)
    os.replace(tmp, arquivo)


def slots_existentes(raiz: Path) -> list[tuple[str, dict[str, Any] | None]]:
    achados: list[tuple[str, dict[str, Any] | None]] = []
    for entrada in sorted(os.scandir(raiz), key=lambda e: e.name):
        if not entrada.is_dir(follow_symlinks=False):
            continue
        if not SLOT_RE.fullmatch(entrada.name):
            continue
        achados.append((entrada.name, ler_binding(raiz / entrada.name)))
    return achados


def descobrir_slot(raiz: Path, task_id: str) -> str | None:
    """Descobre o slot pela leitura do vínculo. Nunca por concatenação."""
    encontrados = [nome for nome, b in slots_existentes(raiz) if b and b.get("task_id") == task_id]
    if len(encontrados) > 1:
        raise ForjaError("DENIED", f"task_id vinculado a múltiplos slots: {encontrados}")
    return encontrados[0] if encontrados else None


def alocar_slot(raiz: Path) -> str:
    """Aloca o menor slot livre. O nome NÃO deriva do task_id."""
    usados = {nome for nome, _ in slots_existentes(raiz)}
    for n in range(1, 10000):
        candidato = f"a{n:02d}"
        if candidato in usados:
            continue
        if (raiz / candidato).exists() or (raiz / candidato).is_symlink():
            continue
        return candidato
    raise ForjaError("FAILED", "nenhum slot livre em agentes/")


# ---------------------------------------------------------------------------
# Layout
# ---------------------------------------------------------------------------


def contido_em(caminho: Path, raiz: Path) -> bool:
    """Contenção canônica, não prefixo textual.

    `raiz/"../x"` começa com `str(raiz)` como texto e escaparia um teste de
    prefixo ingênuo. `normpath` resolve os `..` lexicais antes da comparação;
    `realpath` não serve aqui porque o alvo pode ainda não existir.
    """
    c = os.path.normpath(os.path.abspath(caminho))
    r = os.path.normpath(os.path.abspath(raiz))
    return c == r or c.startswith(r + os.sep)


def exigir_contido(caminho: Path, raiz: Path, quem: str) -> Path:
    if not contido_em(caminho, raiz):
        raise ForjaError("DENIED", f"{quem} escaparia do root: {caminho}")
    return caminho


def descrever_recurso(task_root: Path, recurso: Recurso, medido: bool) -> dict[str, Any]:
    caminho = exigir_contido(task_root / recurso.subpath, task_root, f"recurso {recurso.nome}")
    item: dict[str, Any] = {
        "name": recurso.nome,
        "path": str(caminho),
        "class": recurso.classe,
        "role": recurso.papel,
        "export": recurso.exporta,
        "present": caminho.is_dir() and not caminho.is_symlink(),
    }
    if medido:
        b, f = medir(caminho)
        item["bytes"] = b
        item["files"] = f
    return item


def montar_layout(
    main: Path, raiz: Path, task_id: str, slot: str, binding: dict[str, Any] | None, medido: bool = False
) -> dict[str, Any]:
    task_root = raiz / slot
    return {
        "schema": SCHEMA_LAYOUT,
        "version": VERSION,
        "task_id": task_id,
        "slot": slot,
        "state": (binding or {}).get("state", "ACTIVE"),
        "agent": (binding or {}).get("agent"),
        "created_at": (binding or {}).get("created_at"),
        "canonical_main": str(main),
        "agentes_root": str(raiz),
        "task_root": str(task_root),
        "resources": [descrever_recurso(task_root, r, medido) for r in RECURSOS],
        "shared": [dict(x) for x in COMPARTILHADOS],
        "provisioner_set": [r.nome for r in RECURSOS],
        "finalizer_set": [r.nome for r in RECURSOS],
        "sealable": [r.nome for r in RECURSOS if r.classe == Classe.EPHEMERAL],
        "retire_only": [r.nome for r in RECURSOS if r.classe == Classe.DURABLE],
    }


# ---------------------------------------------------------------------------
# Git
# ---------------------------------------------------------------------------


def git(main: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(  # noqa: S603
        ["git", "-C", str(main), *args],
        capture_output=True,
        text=True,
        shell=False,
        check=False,
    )
    if check and proc.returncode != 0:
        raise ForjaError("FAILED", f"git {' '.join(args)}: {proc.stderr.strip() or proc.stdout.strip()}")
    return proc


def worktree_registrada(main: Path, caminho: Path) -> bool:
    proc = git(main, "worktree", "list", "--porcelain", check=False)
    alvo = str(Path(os.path.abspath(caminho)))
    for linha in proc.stdout.splitlines():
        if linha.startswith("worktree "):
            if linha[len("worktree ") :].strip() == alvo:
                return True
    return False


# ---------------------------------------------------------------------------
# Comandos
# ---------------------------------------------------------------------------


def emitir(dados: dict[str, Any]) -> None:
    print(json.dumps(dados, ensure_ascii=False, sort_keys=True, indent=2))


def gid_agentes() -> int | None:
    try:
        return grp.getgrnam(GRUPO_AGENTES).gr_gid
    except KeyError:
        return None


def criar_dir(caminho: Path) -> bool:
    if caminho.is_symlink():
        raise ForjaError("DENIED", f"destino existe como symlink: {caminho}")
    if caminho.exists():
        if not caminho.is_dir():
            raise ForjaError("DENIED", f"destino existe e não é diretório: {caminho}")
        return False
    os.mkdir(caminho, MODO_DIR)
    try:
        os.chmod(caminho, MODO_DIR)
        gid = gid_agentes()
        if gid is not None:
            st = caminho.lstat()
            if st.st_gid != gid:
                os.chown(caminho, -1, gid)
    except PermissionError:
        pass
    return True


def cmd_provision(args: argparse.Namespace) -> int:
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    criado = False
    if slot is None:
        slot = alocar_slot(raiz)
        criado = True
    task_root = raiz / slot
    sem_symlink_em_componentes(task_root, raiz)
    criar_dir(task_root)

    for recurso in RECURSOS:
        alvo = exigir_contido(task_root / recurso.subpath, task_root, f"recurso {recurso.nome}")
        if recurso.nome == "worktree":
            continue  # criada pelo git, não por mkdir
        sem_symlink_em_componentes(alvo, raiz)
        criar_dir(alvo)

    binding = ler_binding(task_root) or {}
    binding.update(
        {
            "schema": SCHEMA_BINDING,
            "task_id": task_id,
            "slot": slot,
            "agent": binding.get("agent") or agente_corrente(),
            "created_at": binding.get("created_at") or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "state": binding.get("state") or "ACTIVE",
        }
    )
    escrever_binding(task_root, binding)

    # Worktree Git: registrada pelo Git, nunca por cópia manual.
    worktree = task_root / "worktree"
    wt_criada = False
    if args.branch and not worktree.exists():
        sem_symlink_em_componentes(worktree, raiz)
        base = args.base or "HEAD"
        existe_branch = git(main, "rev-parse", "--verify", "--quiet", f"refs/heads/{args.branch}", check=False).returncode == 0
        if existe_branch:
            git(main, "worktree", "add", str(worktree), args.branch)
        else:
            git(main, "worktree", "add", "-b", args.branch, str(worktree), base)
        wt_criada = True
        binding["branch"] = args.branch
        escrever_binding(task_root, binding)

    layout = montar_layout(main, raiz, task_id, slot, ler_binding(task_root))
    layout["status"] = "PROVISIONED"
    layout["slot_created"] = criado
    layout["worktree_created"] = wt_criada
    emitir(layout)
    return EXIT_OK


def cmd_observe(args: argparse.Namespace) -> int:
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    layout = montar_layout(main, raiz, task_id, slot, ler_binding(raiz / slot), medido=args.measure)
    layout["status"] = "OBSERVED"
    emitir(layout)
    return EXIT_OK


def cmd_env(args: argparse.Namespace) -> int:
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    task_root = raiz / slot
    print(f"export TASK_ID={task_id}")
    print(f"export FORJA_TASK_ROOT={task_root}")
    print(f"export FORJA_CANONICAL_MAIN={main}")
    for recurso in RECURSOS:
        if recurso.exporta:
            print(f"export {recurso.exporta}={task_root / recurso.subpath}")
    return EXIT_OK


def cmd_list(args: argparse.Namespace) -> int:
    main, raiz = exigir_raizes()
    linhas = []
    for nome, binding in slots_existentes(raiz):
        task_root = raiz / nome
        item = {
            "slot": nome,
            "task_root": str(task_root),
            "task_id": (binding or {}).get("task_id"),
            "state": (binding or {}).get("state"),
            "agent": (binding or {}).get("agent"),
            "branch": (binding or {}).get("branch"),
            "bound": binding is not None,
        }
        if args.measure:
            b, f = medir(task_root)
            item["bytes"] = b
            item["files"] = f
        linhas.append(item)
    emitir(
        {
            "schema": SCHEMA_LAYOUT,
            "status": "LISTED",
            "canonical_main": str(main),
            "agentes_root": str(raiz),
            "slots": linhas,
            "count": len(linhas),
        }
    )
    return EXIT_OK


def cmd_state(args: argparse.Namespace) -> int:
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    if args.set not in ESTADOS:
        raise ForjaError("DENIED", f"estado inválido: {args.set}; use um de {ESTADOS}")
    task_root = raiz / slot
    binding = ler_binding(task_root) or {}
    anterior = binding.get("state")
    binding.update({"schema": SCHEMA_BINDING, "task_id": task_id, "slot": slot, "state": args.set})
    escrever_binding(task_root, binding)
    emitir({"status": "STATE_SET", "task_id": task_id, "slot": slot, "from": anterior, "to": args.set})
    return EXIT_OK


def guardas_de_destruicao(main: Path, raiz: Path, task_root: Path, binding: dict[str, Any] | None) -> list[str]:
    """Todas as provas exigidas antes de qualquer remoção. Fail-closed."""
    provas: list[str] = []

    real_main = Path(os.path.realpath(main))
    real_raiz = Path(os.path.realpath(raiz))
    real_root = Path(os.path.realpath(task_root))

    if real_raiz.parent != real_main:
        raise ForjaError("DENIED", "agentes/ não é filho direto do checkout canônico")
    provas.append("AGENTES_ROOT_IS_CHILD_OF_CANONICAL_MAIN")

    if real_root.parent != real_raiz:
        raise ForjaError("DENIED", f"task root não está contido em {real_raiz}")
    provas.append("TASK_ROOT_CONTAINED_IN_AGENTES_ROOT")

    if real_root == real_main or real_root == real_raiz:
        raise ForjaError("DENIED", "task root coincide com o checkout canônico ou com agentes/")
    if contido_em(real_main, real_root):
        raise ForjaError("DENIED", "checkout canônico está contido no task root")
    provas.append("TASK_ROOT_IS_NOT_CANONICAL_MAIN")

    if not SLOT_RE.fullmatch(real_root.name):
        raise ForjaError("DENIED", f"nome de slot inválido para destruição: {real_root.name}")
    provas.append("SLOT_NAME_WELL_FORMED")

    sem_symlink_em_componentes(task_root, real_main)
    if task_root.is_symlink():
        raise ForjaError("DENIED", "task root é symlink")
    provas.append("NO_SYMLINK_IN_PATH_COMPONENTS")

    sem_mount_interno(task_root)
    provas.append("NO_MOUNTPOINT_INSIDE_TASK_ROOT")

    vivos = processos_no_root(task_root)
    if vivos:
        raise ForjaError("BLOCKED_BY_PROCESS", f"processos ativos no task root: {vivos}")
    provas.append("NO_LIVE_PROCESS_IN_TASK_ROOT")

    estado = (binding or {}).get("state")
    if estado not in ESTADOS_DESTRUTIVEIS:
        raise ForjaError("DENIED", f"estado {estado!r} não autoriza destruição; exigido um de {ESTADOS_DESTRUTIVEIS}")
    provas.append(f"TASK_STATE_ALLOWS_DESTRUCTION({estado})")

    st = task_root.lstat()
    if st.st_uid not in {0, os.getuid()}:
        raise ForjaError("DENIED", "task root pertence a outra identidade")
    provas.append("OWNERSHIP_COMPATIBLE")

    for compartilhado in COMPARTILHADOS:
        alvo = Path(os.path.abspath(compartilhado["path"]))
        if contido_em(alvo, real_root):
            raise ForjaError("DENIED", f"recurso compartilhado dentro do task root: {alvo}")
    provas.append("NO_SHARED_RESOURCE_INSIDE_TASK_ROOT")

    return provas


def cmd_seal(args: argparse.Namespace) -> int:
    """EXECUTION_SEAL: recupera o efêmero, preserva o durável."""
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    task_root = raiz / slot
    sem_symlink_em_componentes(task_root, canonical_main())
    sem_mount_interno(task_root)

    itens: list[dict[str, Any]] = []
    recuperados = 0
    for recurso in RECURSOS:
        alvo = exigir_contido(task_root / recurso.subpath, task_root, f"recurso {recurso.nome}")
        antes, arquivos = medir(alvo)
        if recurso.classe != Classe.EPHEMERAL:
            itens.append(
                {"name": recurso.nome, "path": str(alvo), "class": recurso.classe, "action": "PRESERVED", "bytes_before": antes}
            )
            continue
        if not alvo.exists():
            itens.append({"name": recurso.nome, "path": str(alvo), "class": recurso.classe, "action": "ABSENT", "bytes_before": 0})
            continue
        sem_symlink_em_componentes(alvo, canonical_main())
        if alvo.is_symlink():
            raise ForjaError("DENIED", f"recurso é symlink: {alvo}")
        vivos = processos_no_root(alvo)
        if vivos:
            raise ForjaError("BLOCKED_BY_PROCESS", f"processo ativo em {alvo}: {vivos}")
        if args.apply:
            remover_arvore_sem_seguir_links(alvo)
            criar_dir(alvo)
            recuperados += antes
            acao = "RECLAIMED"
        else:
            acao = "WOULD_RECLAIM"
        itens.append(
            {
                "name": recurso.nome,
                "path": str(alvo),
                "class": recurso.classe,
                "action": acao,
                "bytes_before": antes,
                "files_before": arquivos,
            }
        )

    binding = ler_binding(task_root) or {}
    if args.apply:
        binding.update({"schema": SCHEMA_BINDING, "task_id": task_id, "slot": slot, "state": "SEALED"})
        escrever_binding(task_root, binding)

    emitir(
        {
            "status": "SEALED" if args.apply else "SEAL_PLAN",
            "schema": SCHEMA_LAYOUT,
            "task_id": task_id,
            "slot": slot,
            "task_root": str(task_root),
            "reclaimed_bytes": recuperados,
            "items": itens,
            "preserved_for_review": [r.nome for r in RECURSOS if r.classe == Classe.DURABLE],
        }
    )
    return EXIT_OK


def cmd_retire(args: argparse.Namespace) -> int:
    """TASK_RETIRE: destrói o root inteiro, depois de todas as provas."""
    main, raiz = exigir_raizes()
    task_id = resolver_task(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    task_root = raiz / slot
    binding = ler_binding(task_root)

    provas = guardas_de_destruicao(main, raiz, task_root, binding)

    bytes_antes, arquivos_antes = medir(task_root)
    worktree = task_root / "worktree"
    registrada = worktree_registrada(main, worktree)

    if not args.apply:
        emitir(
            {
                "status": "RETIRE_PLAN",
                "schema": SCHEMA_LAYOUT,
                "task_id": task_id,
                "slot": slot,
                "task_root": str(task_root),
                "proofs": provas,
                "worktree_registered": registrada,
                "bytes": bytes_antes,
                "files": arquivos_antes,
            }
        )
        return EXIT_OK

    # 1. desregistrar a worktree pelo Git, antes de remover o diretório
    if registrada:
        git(main, "worktree", "remove", "--force", str(worktree), check=False)
    # 2. remover o root físico sem seguir symlinks
    if task_root.exists():
        remover_arvore_sem_seguir_links(task_root)
    # 3. podar metadata do Git
    git(main, "worktree", "prune", check=False)
    # 4. provar ausência de metadata órfã
    orfas = metadata_orfa(main)
    # 5. a branch sobrevive de propósito: apagá-la seria destruir identidade
    #    de commit que o root físico não possuía. Mas silêncio vira resíduo,
    #    então ela é reportada com o que ainda lhe é exclusivo.
    branch_restante = branch_orfa(main, (binding or {}).get("branch"))

    emitir(
        {
            "status": "RETIRED",
            "schema": SCHEMA_LAYOUT,
            "task_id": task_id,
            "slot": slot,
            "task_root": str(task_root),
            "proofs": provas,
            "worktree_was_registered": registrada,
            "reclaimed_bytes": bytes_antes,
            "reclaimed_files": arquivos_antes,
            "task_root_absent": not task_root.exists() and not task_root.is_symlink(),
            "stale_git_worktree_metadata": orfas,
            "branch_left_behind": branch_restante,
        }
    )
    return EXIT_OK


def branch_orfa(main: Path, branch: str | None) -> dict[str, Any] | None:
    """Descreve a branch que sobrevive à retirada do root físico.

    A retirada destrói recurso, não histórico. Uma branch com commits que não
    estão em `origin/main` é trabalho que ninguém deve perder por limpeza de
    disco — por isso ela é reportada, com a contagem exata, em vez de removida.
    """
    if not branch:
        return None
    if git(main, "rev-parse", "--verify", "--quiet", f"refs/heads/{branch}", check=False).returncode != 0:
        return None
    proc = git(main, "rev-list", "--count", f"origin/main..refs/heads/{branch}", check=False)
    exclusivos = int(proc.stdout.strip()) if proc.returncode == 0 and proc.stdout.strip().isdigit() else None
    return {
        "name": branch,
        "commits_not_in_origin_main": exclusivos,
        "disposition": "PRESERVED_NOT_DELETED_BY_RETIRE",
    }


def metadata_orfa(main: Path) -> list[str]:
    """Entradas em .git/worktrees cujo diretório de trabalho não existe mais."""
    proc = git(main, "rev-parse", "--git-common-dir", check=False)
    if proc.returncode != 0:
        return []
    comum = Path(proc.stdout.strip())
    if not comum.is_absolute():
        comum = main / comum
    base = comum / "worktrees"
    if not base.is_dir():
        return []
    orfas: list[str] = []
    for entrada in base.iterdir():
        gitdir = entrada / "gitdir"
        if not gitdir.is_file():
            orfas.append(str(entrada))
            continue
        alvo = Path(gitdir.read_text(encoding="utf-8").strip())
        # `gitdir` aponta para o arquivo `.git` da worktree
        if not alvo.exists():
            orfas.append(str(entrada))
    return orfas


def cmd_verify(args: argparse.Namespace) -> int:
    """Prova os invariantes terminais de forma mecânica."""
    main, raiz = exigir_raizes()
    problemas: list[str] = []
    checagens: dict[str, Any] = {}

    checagens["canonical_main"] = str(main)
    checagens["canonical_main_is_git"] = (main / ".git").exists()
    if not checagens["canonical_main_is_git"]:
        problemas.append("checkout canônico não é repositório Git")

    checagens["agentes_root_is_child"] = Path(os.path.realpath(raiz)).parent == Path(os.path.realpath(main))
    if not checagens["agentes_root_is_child"]:
        problemas.append("agentes/ não é filho do checkout canônico")

    # PROVISIONER_SET == FINALIZER_SET, por construção e por verificação
    provisiona = {r.nome for r in RECURSOS}
    finaliza = {r.nome for r in RECURSOS if r.classe == Classe.EPHEMERAL} | {
        r.nome for r in RECURSOS if r.classe == Classe.DURABLE
    }
    checagens["provisioner_set"] = sorted(provisiona)
    checagens["finalizer_set"] = sorted(finaliza)
    checagens["provisioner_equals_finalizer"] = provisiona == finaliza
    if provisiona != finaliza:
        problemas.append("provisioner e finalizer divergem")

    # slots: unicidade de vínculo e disjunção de roots
    vistos: dict[str, str] = {}
    roots: list[str] = []
    slots_info = []
    for nome, binding in slots_existentes(raiz):
        task_root = raiz / nome
        roots.append(str(Path(os.path.realpath(task_root))))
        tid = (binding or {}).get("task_id")
        if tid:
            if tid in vistos:
                problemas.append(f"task_id {tid} vinculado a {vistos[tid]} e {nome}")
            vistos[tid] = nome
        else:
            problemas.append(f"slot sem vínculo legível: {nome}")
        if task_root.is_symlink():
            problemas.append(f"slot é symlink: {nome}")
        slots_info.append({"slot": nome, "task_id": tid, "state": (binding or {}).get("state")})
    checagens["slots"] = slots_info

    # disjunção: nenhum root é prefixo de outro
    for i, a in enumerate(roots):
        for b in roots[i + 1 :]:
            if a == b or a.startswith(b + os.sep) or b.startswith(a + os.sep):
                problemas.append(f"task roots não disjuntos: {a} e {b}")
    checagens["task_roots_disjoint"] = not any("não disjuntos" in p for p in problemas)

    # identidade lógica != caminho físico
    conflacoes = [t for t, s in vistos.items() if t == s]
    checagens["task_identity_ne_physical_path"] = not conflacoes
    if conflacoes:
        problemas.append(f"slot igual ao task_id (conflação de identidade): {conflacoes}")

    orfas = metadata_orfa(main)
    checagens["stale_git_worktree_metadata"] = orfas
    if orfas:
        problemas.append(f"metadata Git órfã: {orfas}")

    # o checkout canônico não deve carregar mutação de implementação de Task
    sujo = git(main, "status", "--porcelain", check=False).stdout.strip().splitlines()
    checagens["canonical_main_dirty_entries"] = sujo
    if sujo:
        problemas.append("checkout canônico não está limpo")

    emitir(
        {
            "status": "OK" if not problemas else "INVALID",
            "schema": SCHEMA_LAYOUT,
            "version": VERSION,
            "checks": checagens,
            "problems": problemas,
        }
    )
    return EXIT_OK if not problemas else 5


def cmd_contract(args: argparse.Namespace) -> int:
    """Emite o contrato de recursos sem exigir Task nem host provisionado."""
    emitir(
        {
            "schema": SCHEMA_LAYOUT,
            "version": VERSION,
            "status": "CONTRACT",
            "canonical_main": CANONICAL_MAIN_PADRAO,
            "agentes_dirname": AGENTES_DIRNAME,
            "binding_file": BINDING_FILENAME,
            "states": list(ESTADOS),
            "destructive_states": list(ESTADOS_DESTRUTIVEIS),
            "resources": [
                {
                    "name": r.nome,
                    "subpath": r.subpath,
                    "class": r.classe,
                    "role": r.papel,
                    "export": r.exporta,
                }
                for r in RECURSOS
            ],
            "shared": [dict(x) for x in COMPARTILHADOS],
            "provisioner_set": [r.nome for r in RECURSOS],
            "finalizer_set": [r.nome for r in RECURSOS],
        }
    )
    return EXIT_OK


def construir_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="forja-agentes",
        description="Autoridade única de layout de Task da Forja sobre agentes/.",
    )
    p.add_argument("--version", action="version", version=VERSION)
    sub = p.add_subparsers(dest="comando", required=True)

    sp = sub.add_parser("provision", help="aloca/garante o task root observado")
    sp.add_argument("--task-id")
    sp.add_argument("--branch", help="cria/usa a branch da worktree")
    sp.add_argument("--base", help="base da nova branch (padrão HEAD)")
    sp.set_defaults(handler=cmd_provision)

    so = sub.add_parser("observe", help="emite o layout observado da Task")
    so.add_argument("--task-id")
    so.add_argument("--measure", action="store_true")
    so.set_defaults(handler=cmd_observe)

    se = sub.add_parser("env", help="emite exports de shell do layout observado")
    se.add_argument("--task-id")
    se.set_defaults(handler=cmd_env)

    sl = sub.add_parser("list", help="lista os slots observados")
    sl.add_argument("--measure", action="store_true")
    sl.set_defaults(handler=cmd_list)

    ss = sub.add_parser("state", help="define o estado de lifecycle do task root")
    ss.add_argument("--task-id")
    ss.add_argument("--set", required=True, choices=list(ESTADOS))
    ss.set_defaults(handler=cmd_state)

    sseal = sub.add_parser("seal", help="EXECUTION_SEAL: recupera efêmero, preserva durável")
    sseal.add_argument("--task-id")
    sseal.add_argument("--apply", action="store_true")
    sseal.set_defaults(handler=cmd_seal)

    sr = sub.add_parser("retire", help="TASK_RETIRE: destrói o root, fail-closed")
    sr.add_argument("--task-id")
    sr.add_argument("--apply", action="store_true")
    sr.set_defaults(handler=cmd_retire)

    sv = sub.add_parser("verify", help="prova os invariantes do layout")
    sv.set_defaults(handler=cmd_verify)

    sc = sub.add_parser("contract", help="emite o contrato de recursos")
    sc.set_defaults(handler=cmd_contract)

    return p


def main(argv: Sequence[str] | None = None) -> int:
    try:
        args = construir_parser().parse_args(argv)
        return args.handler(args)
    except ForjaError as erro:
        print(json.dumps({"status": erro.status, "error": erro.mensagem}, ensure_ascii=False), file=sys.stderr)
        if erro.status == "NOT_FOUND":
            return EXIT_NAO_ENCONTRADO
        if erro.status in {"DENIED", "BLOCKED_BY_MOUNT", "BLOCKED_BY_PROCESS"}:
            return EXIT_RECUSADO
        return EXIT_FALHA
    except OSError as erro:
        print(json.dumps({"status": "FAILED", "error": str(erro)}, ensure_ascii=False), file=sys.stderr)
        return EXIT_FALHA


if __name__ == "__main__":
    raise SystemExit(main())
