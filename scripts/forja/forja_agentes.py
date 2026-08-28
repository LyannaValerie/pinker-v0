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
import datetime as dt
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
ESTADOS_DESTRUTIVEIS = ("RETIREABLE",)

# Transições autorizadas. A ausência de uma aresta é a regra, não um esquecimento:
# `ACTIVE -> RETIREABLE` não existe de propósito, porque pular o selo apagaria a
# worktree e a memória que a revisão ainda usa. O único caminho até a destruição
# passa por um selo real.
TRANSICOES = {
    "ACTIVE": ("REVIEW", "FIX_REQUIRED"),
    "REVIEW": ("ACTIVE", "FIX_REQUIRED"),
    "FIX_REQUIRED": ("ACTIVE", "REVIEW"),
    "SEALED": ("FIX_REQUIRED", "REVIEW", "RETIREABLE"),
    "RETIREABLE": (),
}

# `SEALED` não aparece em nenhum destino de `state`: ele é escrito apenas por um
# `seal --apply` bem-sucedido. Deixá-lo alcançável por `state` permitiria forjar
# o selo — declarar SEALED sem nunca ter recuperado nada — e daí chegar a
# RETIREABLE com a worktree e a memória intactas mas o gate satisfeito. O selo
# precisa ser um fato ocorrido, não um rótulo escrito à mão.
ESTADOS_SO_POR_OPERACAO = {"SEALED": "seal --apply"}


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
    except OSError as erro:
        # Devolver conjunto vazio faria `sem_mount_interno` certificar ausência
        # de mount a partir de uma leitura que nunca aconteceu.
        raise ForjaError("BLOCKED_BY_MOUNT", f"autoridade de mount ilegível: {erro}") from erro
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


def processos_no_root(
    raiz: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    """Processos que tocam `raiz`, e aqueles cuja evidência não pôde ser lida.

    Devolve `(achados, indeterminados, nao_inspecionaveis)`. A separação existe porque um link de
    `/proc` ilegível não é evidência de ausência: é ausência de evidência. Um
    único `except OSError: continue` transformaria "não consegui olhar" em "não
    há ninguém" — que é exatamente o modo fail-open que uma função fail-closed
    não pode ter. Numa varredura real deste host, 354 links eram ilegíveis.

    Além de cwd/exe/root, inspeciona descritores abertos: um processo pode
    manter um arquivo ou socket dentro do root sem que nenhum dos três links
    aponte para lá.
    """
    prefixo = str(Path(os.path.abspath(raiz))) + os.sep
    alvo_exato = str(Path(os.path.abspath(raiz)))
    achados: list[dict[str, Any]] = []
    indeterminados: list[dict[str, Any]] = []
    nao_inspecionaveis: list[dict[str, Any]] = []
    proc = Path("/proc")
    if not proc.is_dir():
        raise ForjaError("DENIED", "/proc indisponível: impossível provar ausência de processo")

    def dentro(alvo: str) -> bool:
        return alvo == alvo_exato or alvo.startswith(prefixo)

    meu_pid = os.getpid()
    for entrada in proc.iterdir():
        if not entrada.name.isdigit():
            continue
        pid = int(entrada.name)
        try:
            comm = (entrada / "comm").read_text(encoding="utf-8").strip()
        except OSError:
            comm = "?"
        atingido = False
        ilegivel: list[str] = []
        for campo in ("cwd", "exe", "root"):
            try:
                alvo = os.readlink(entrada / campo)
            except FileNotFoundError:
                continue  # processo terminou entre o scandir e o readlink
            except OSError:
                ilegivel.append(campo)
                continue
            if dentro(alvo):
                achados.append({"pid": pid, "field": campo, "target": alvo, "comm": comm})
                atingido = True
                break
        if atingido:
            continue
        # descritores abertos: arquivo, socket ou mmap dentro do root
        fddir = entrada / "fd"
        try:
            nomes = os.listdir(fddir)
        except FileNotFoundError:
            nomes = []
        except OSError:
            ilegivel.append("fd")
            nomes = []
        for nome in nomes:
            try:
                alvo = os.readlink(fddir / nome)
            except FileNotFoundError:
                continue  # descritor fechado entre o listdir e o readlink
            except OSError:
                # Terceira aparição do mesmo fail-open nesta função: engolir o
                # erro aqui fazia um descritor ilegível desaparecer da conta,
                # e nem o modo estrito bloqueava por ele.
                if "fd" not in ilegivel:
                    ilegivel.append("fd")
                continue
            if dentro(alvo):
                achados.append({"pid": pid, "field": f"fd/{nome}", "target": alvo, "comm": comm})
                atingido = True
                break
        if not atingido and ilegivel and pid != meu_pid:
            # Ilegível não é automaticamente perigoso. O uid do processo vem de
            # /proc/PID/status, que é legível mesmo quando os links não são, e
            # um uid que não alcança o root não pode estar segurando nada lá
            # dentro. Isso converte a maior parte dos "não consegui olhar" numa
            # prova real, em vez de num override genérico — sem o qual o gate
            # bloquearia por centenas de processos alheios e seria desligado.
            uid = uid_do_processo(entrada)
            # A exclusão por modo só é sólida quando conseguimos ver os
            # descritores: um fd herdado sobrevive a um `chmod` posterior, então
            # julgar alcance pelo modo ATUAL descartaria um processo que abriu o
            # root quando ele era mais permissivo. Se a lista de fd foi
            # ilegível, o modo não decide nada.
            if "fd" not in ilegivel and uid is not None and not uid_alcanca(uid, raiz):
                continue
            if uid == 0:
                # Processos de root — na prática threads do kernel e daemons do
                # sistema — nunca são legíveis por um agente sem privilégio. Um
                # gate que bloqueasse por eles bloquearia sempre, e um gate que
                # bloqueia sempre é desligado. Eles são contados e reportados
                # para que o alcance da prova fique explícito, em vez de a
                # limitação desaparecer num silêncio conveniente.
                nao_inspecionaveis.append({"pid": pid, "uid": uid, "comm": comm})
                continue
            indeterminados.append(
                {"pid": pid, "uid": uid, "unreadable": ilegivel, "comm": comm}
            )
    return achados, indeterminados, nao_inspecionaveis


def uid_do_processo(entrada: Path) -> int | None:
    try:
        for linha in (entrada / "status").read_text(encoding="utf-8").splitlines():
            if linha.startswith("Uid:"):
                return int(linha.split()[1])  # real uid
    except (OSError, ValueError, IndexError):
        return None
    return None


def uid_alcanca(uid: int, raiz: Path) -> bool:
    """O uid tem alguma chance de manter um descritor dentro de `raiz`?

    Root alcança tudo. O dono alcança. Um membro do grupo alcança quando o
    diretório dá permissão ao grupo. Qualquer outro uid não atravessa o modo
    2770 — e então sua ilegibilidade é irrelevante para esta prova.
    """
    if uid == 0:
        return True
    try:
        st = raiz.lstat()
    except OSError:
        return True  # não consegui medir o alvo: mantenha a suspeita
    if uid == st.st_uid:
        return True
    modo = st.st_mode & 0o7777
    if not (modo & 0o070):
        return False  # grupo sem acesso: só dono e root alcançam
    try:
        nome = pwd.getpwuid(uid).pw_name
        grupo = grp.getgrgid(st.st_gid)
    except KeyError:
        return True
    if nome in grupo.gr_mem:
        return True
    try:
        return pwd.getpwnam(nome).pw_gid == st.st_gid
    except KeyError:
        return True


def processos_estritos() -> bool:
    """Bloqueia por processo NÃO-ROOT de alcance equivalente que seja ilegível.

    O nome importa: este modo não prova ausência de processo de root. Threads do
    kernel e daemons de root continuam apenas contados, porque nenhum agente sem
    privilégio consegue lê-los — chamar isso de "estrito" sem qualificar seria
    prometer uma prova que o modo não entrega.

    O padrão é reportar, não bloquear, e a razão é medida e não estética:
    processos privilege-separated do mesmo uid (`sshd-session`, com dumpable=0)
    e as threads do kernel nunca são legíveis por um agente sem privilégio.
    Bloquear por eles tornaria a retirada impossível em qualquer host com SSH —
    e um gate que nunca deixa passar é um gate que alguém desliga.

    O que o gate garante por padrão é preciso e está escrito na prova emitida:
    nenhum processo INSPECIONÁVEL toca o root. Os não inspecionáveis são
    contados e nomeados na saída, de modo que o alcance da prova fique visível
    em vez de virar silêncio. Quem precisar da versão estrita liga esta variável
    e aceita bloquear.
    """
    return os.environ.get("FORJA_AGENTES_STRICT_PROCESSES") == "1"


def instante_canonico(valor: Any, campo: str) -> "dt.datetime":
    """Converte um carimbo em instante UTC, exigindo forma canônica exata.

    `time.strptime` é permissivo: aceita segundo 61 e tolera variações que nunca
    foram escritas por este código. Como o valor autoriza destruição, a
    conversão precisa ser ida-e-volta: se reserializar não devolver o texto
    original, o carimbo não é o que diz ser.
    """
    if not isinstance(valor, str):
        raise ForjaError("DENIED", f"{campo} ausente ou não textual: {valor!r}")
    try:
        instante = dt.datetime.strptime(valor, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except (TypeError, ValueError) as erro:
        raise ForjaError("DENIED", f"{campo} malformado: {valor!r}") from erro
    if instante.strftime("%Y-%m-%dT%H:%M:%SZ") != valor:
        raise ForjaError("DENIED", f"{campo} não está em forma canônica: {valor!r}")
    return instante


def confirmar_identidade(alvo: Path, identidade: tuple[int, int]) -> None:
    """Reconfere que `alvo` ainda é o mesmo objeto aprovado pelas guardas."""
    try:
        st = alvo.lstat()
    except OSError as erro:
        raise ForjaError("DENIED", f"alvo desapareceu entre a guarda e a operação: {alvo}") from erro
    if (st.st_dev, st.st_ino) != identidade:
        raise ForjaError(
            "DENIED",
            f"identidade do alvo mudou entre a guarda e a operação: {alvo} não é mais {identidade}",
        )


def remover_arvore_sem_seguir_links(
    raiz: Path, identidade: tuple[int, int] | None = None
) -> tuple[int, int]:
    """Remove `raiz` inteira sem jamais atravessar um symlink.

    Usa descritores de diretório e `os.unlink(..., dir_fd=)` para que a
    resolução aconteça relativa ao fd já aberto, e não ao caminho textual, que
    poderia ser trocado por um symlink no meio da operação.

    `identidade` é o par `(st_dev, st_ino)` medido no momento em que as guardas
    aprovaram o alvo. Sem ele, as guardas validam um caminho e o delete reabre
    o MESMO NOME — que pode já ser outro diretório. Comparar a identidade
    imediatamente antes de descer fecha essa janela.
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
        if identidade is not None:
            st = os.lstat(raiz.name, dir_fd=fd_pai)
            if (st.st_dev, st.st_ino) != identidade:
                raise ForjaError(
                    "DENIED",
                    f"identidade do alvo mudou entre a guarda e a remoção: "
                    f"{raiz} não é mais {identidade}",
                )
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
    """Resolve o TASK_ID para comandos de LEITURA.

    Um `--task-id` explícito é aceito aqui porque observar o layout de outra
    Task não muta nada.
    """
    return validar_task(arg) if arg else task_do_observador()


def resolver_task_propria(arg: str | None) -> str:
    """Resolve o TASK_ID para comandos que MUTAM ou DESTROEM.

    Aqui o `--task-id` deixa de ser um endereço e vira uma asserção: ele precisa
    coincidir com a Task que o observador canônico atribui ao chamador. Sem esta
    distinção, `--task-id` seria exatamente o mecanismo pelo qual a Task A
    apagaria a Task B — e nenhuma das guardas de caminho detectaria isso, porque
    o caminho da B é perfeitamente válido.
    """
    observada = task_do_observador()
    if arg is not None and validar_task(arg) != observada:
        raise ForjaError(
            "DENIED",
            f"comando mutante recusado: --task-id {arg!r} não é a Task ativa do chamador ({observada!r})",
        )
    return observada


def exigir_dono(binding: dict[str, Any] | None, task_id: str) -> None:
    """O vínculo gravado no slot precisa concordar com quem está chamando."""
    if not binding:
        raise ForjaError("DENIED", "task root sem vínculo legível; recusado para mutação")
    if binding.get("task_id") != task_id:
        raise ForjaError("DENIED", "vínculo do slot não corresponde à Task do chamador")
    dono = binding.get("agent")
    if dono and dono != agente_corrente():
        raise ForjaError("DENIED", f"task root pertence ao agente {dono!r}, não a {agente_corrente()!r}")


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
    # nome exclusivo por processo: um `.tmp` compartilhado faria dois escritores
    # concorrentes disputarem o mesmo arquivo intermediário
    tmp = slot_dir / f".{BINDING_FILENAME}.{os.getpid()}.tmp"
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


def reivindicar_slot(raiz: Path) -> str:
    """Reivindica atomicamente o menor slot livre. O nome NÃO deriva do task_id.

    `os.mkdir` é a primitiva de exclusão mútua: ele falha com `EEXIST` se outro
    processo criou o diretório entre a varredura e a criação. Varrer e depois
    criar sem tratar `EEXIST` era o defeito F4 — duas Tasks provisionando ao
    mesmo tempo podiam concordar sobre qual slot estava livre.
    """
    usados = {nome for nome, _ in slots_existentes(raiz)}
    for n in range(1, 10000):
        candidato = f"a{n:02d}"
        if candidato in usados:
            continue
        alvo = raiz / candidato
        try:
            os.mkdir(alvo, MODO_DIR)
        except FileExistsError:
            continue  # perdemos a corrida para outro provisionamento: siga adiante
        try:
            os.chmod(alvo, MODO_DIR)
            gid = gid_agentes()
            if gid is not None and alvo.lstat().st_gid != gid:
                os.chown(alvo, -1, gid)
        except PermissionError:
            pass
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
    if proc.returncode != 0:
        raise ForjaError(
            "FAILED",
            f"impossível inspecionar worktrees registradas: {proc.stderr.strip()[:200]}",
        )
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
    # provisionar cria worktree, branch e diretórios: é mutante, e portanto
    # `--task-id` aqui também é asserção, não endereço
    task_id = resolver_task_propria(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    criado = False
    if slot is None:
        slot = reivindicar_slot(raiz)  # já criou o diretório, atomicamente
        criado = True
    task_root = raiz / slot
    sem_symlink_em_componentes(task_root, raiz)
    if not criado:
        # Root pré-existente só é reusado quando o vínculo já é desta Task.
        # Adotar um diretório sem vínculo era o outro lado do F4.
        exigir_dono(ler_binding(task_root), task_id)
    if not task_root.is_dir() or task_root.is_symlink():
        raise ForjaError("DENIED", f"task root não é diretório regular: {task_root}")

    for recurso in RECURSOS:
        alvo = exigir_contido(task_root / recurso.subpath, task_root, f"recurso {recurso.nome}")
        if recurso.nome == "worktree":
            continue  # criada pelo git, não por mkdir
        sem_symlink_em_componentes(alvo, raiz)
        criar_dir(alvo)

    binding = ler_binding(task_root) or {}
    # Reprovisionar significa que a Task voltou a estar viva. Herdar um estado
    # terminal deixaria um root recém-provisionado imediatamente destrutível, e
    # herdar o `sealed_at` faria um selo antigo autorizar a destruição de um
    # trabalho novo. Ambos são zerados aqui, de propósito.
    anterior = binding.get("state")
    reaberta = anterior in ("SEALED", "RETIREABLE")
    binding.update(
        {
            "schema": SCHEMA_BINDING,
            "task_id": task_id,
            "slot": slot,
            "agent": binding.get("agent") or agente_corrente(),
            "created_at": binding.get("created_at") or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "state": "ACTIVE" if (reaberta or not anterior) else anterior,
        }
    )
    if reaberta:
        # O selo antigo não pode autorizar a destruição de trabalho novo, mas
        # apagá-lo sem deixar rastro perde evidência que a revisão pode querer.
        # Ele sai do campo que autoriza e entra num histórico append-only.
        # Append-only por CONVENÇÃO, não por mecanismo: o vínculo mora em
        # armazenamento gravável pelo agente, então isto preserva evidência
        # contra descarte acidental, não contra adulteração deliberada. Uma
        # garantia real exigiria estado fora do alcance de escrita da Task.
        historico = list(binding.get("seal_history") or [])
        historico.append(
            {
                "sealed_at": binding.get("sealed_at"),
                "reclaimed_bytes": binding.get("sealed_reclaimed_bytes"),
                "superseded_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "reopened_from": anterior,
            }
        )
        binding["seal_history"] = historico
        binding.pop("sealed_at", None)
        binding.pop("sealed_reclaimed_bytes", None)
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
    layout["reopened_from"] = anterior if reaberta else None
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
    task_id = resolver_task_propria(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    if args.set not in ESTADOS:
        raise ForjaError("DENIED", f"estado inválido: {args.set}; use um de {ESTADOS}")
    task_root = raiz / slot
    binding = ler_binding(task_root)
    exigir_dono(binding, task_id)
    binding = dict(binding or {})
    anterior = binding.get("state") or "ACTIVE"
    if args.set in ESTADOS_SO_POR_OPERACAO and args.set != anterior:
        raise ForjaError(
            "DENIED",
            f"{args.set} não é atribuível por `state`: só é alcançado por "
            f"`{ESTADOS_SO_POR_OPERACAO[args.set]}`, que precisa ter ocorrido de fato",
        )
    if args.set != anterior and args.set not in TRANSICOES.get(anterior, ()):
        raise ForjaError(
            "DENIED",
            f"transição não autorizada: {anterior} -> {args.set}; de {anterior} só é permitido {TRANSICOES.get(anterior, ())}",
        )
    binding.update({"schema": SCHEMA_BINDING, "task_id": task_id, "slot": slot, "state": args.set})
    escrever_binding(task_root, binding)
    emitir({"status": "STATE_SET", "task_id": task_id, "slot": slot, "from": anterior, "to": args.set})
    return EXIT_OK


def guardas_de_destruicao(
    main: Path, raiz: Path, task_root: Path, binding: dict[str, Any] | None
) -> tuple[list[str], tuple[int, int]]:
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

    vivos, indeterminados, nao_inspecionaveis = processos_no_root(task_root)
    if vivos:
        raise ForjaError("BLOCKED_BY_PROCESS", f"processos ativos no task root: {vivos}")
    if indeterminados and processos_estritos():
        raise ForjaError(
            "BLOCKED_BY_UNKNOWN_PROCESS",
            f"modo estrito: {len(indeterminados)} processo(s) que alcançam o root com evidência "
            f"ilegível; ausência não provada: {indeterminados[:5]}",
        )
    provas.append(
        f"NO_INSPECTABLE_PROCESS_IN_TASK_ROOT"
        f"(uninspectable_same_reach={len(indeterminados)};"
        f"uninspectable_root_owned={len(nao_inspecionaveis)})"
    )

    estado = (binding or {}).get("state")
    if estado not in ESTADOS_DESTRUTIVEIS:
        raise ForjaError("DENIED", f"estado {estado!r} não autoriza destruição; exigido um de {ESTADOS_DESTRUTIVEIS}")
    provas.append(f"TASK_STATE_ALLOWS_DESTRUCTION({estado})")

    # O rótulo não basta: o selo tem de ter deixado marca. Sem isto, escrever
    # RETIREABLE à mão seria suficiente para destruir uma Task nunca selada.
    selado_em = (binding or {}).get("sealed_at")
    if not selado_em:
        raise ForjaError(
            "DENIED",
            "sem evidência de EXECUTION_SEAL no vínculo: destruir exige um selo ocorrido, não um rótulo",
        )
    # Exigir apenas que a chave exista aceitaria `sealed_at: "x"`. Mas validar
    # frouxo é quase o mesmo: `strptime` aceita segundo 61 e formas não
    # canônicas, `created_at` ausente pulava a comparação, e um `created_at`
    # ilegível caía num `pass`. Cada um desses ramos autorizava destruição a
    # partir de algo que não foi provado. Agora todos falham fechado.
    criado_em = (binding or {}).get("created_at")
    instante = instante_canonico(selado_em, "sealed_at")
    nascimento = instante_canonico(criado_em, "created_at")
    if instante < nascimento:
        raise ForjaError(
            "DENIED",
            f"selo datado antes da criação do root ({selado_em} < {criado_em})",
        )
    provas.append(f"EXECUTION_SEAL_EVIDENCE({selado_em})")

    st = task_root.lstat()
    if st.st_uid != os.getuid():
        raise ForjaError("DENIED", "task root não pertence ao uid do chamador")
    provas.append("OWNERSHIP_IS_CALLER")

    dono = (binding or {}).get("agent")
    if dono != agente_corrente():
        raise ForjaError("DENIED", f"vínculo declara o agente {dono!r}, não {agente_corrente()!r}")
    provas.append("BINDING_AGENT_IS_CALLER")

    for compartilhado in COMPARTILHADOS:
        alvo = Path(os.path.abspath(compartilhado["path"]))
        if contido_em(alvo, real_root):
            raise ForjaError("DENIED", f"recurso compartilhado dentro do task root: {alvo}")
    provas.append("NO_SHARED_RESOURCE_INSIDE_TASK_ROOT")

    identidade = (st.st_dev, st.st_ino)
    provas.append(f"TARGET_IDENTITY_PINNED(dev={st.st_dev},ino={st.st_ino})")
    return provas, identidade


def cmd_seal(args: argparse.Namespace) -> int:
    """EXECUTION_SEAL: recupera o efêmero, preserva o durável."""
    main, raiz = exigir_raizes()
    task_id = resolver_task_propria(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    task_root = raiz / slot
    exigir_dono(ler_binding(task_root), task_id)
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
        # A identidade do recurso é fixada AQUI, imediatamente antes de remover:
        # o selo apagava por nome, sem o mesmo cuidado que a retirada já tinha.
        try:
            st_alvo = alvo.lstat()
        except OSError as erro:
            raise ForjaError("DENIED", f"recurso desapareceu antes da selagem: {alvo}") from erro
        identidade_alvo = (st_alvo.st_dev, st_alvo.st_ino)
        vivos, indeterminados, _ = processos_no_root(alvo)
        if vivos:
            raise ForjaError("BLOCKED_BY_PROCESS", f"processo ativo em {alvo}: {vivos}")
        if indeterminados and processos_estritos():
            raise ForjaError(
                "BLOCKED_BY_UNKNOWN_PROCESS",
                f"modo estrito: {len(indeterminados)} processo(s) que alcançam {alvo} com "
                "evidência ilegível",
            )
        if args.apply:
            remover_arvore_sem_seguir_links(alvo, identidade_alvo)
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
        binding.update(
            {
                "schema": SCHEMA_BINDING,
                "task_id": task_id,
                "slot": slot,
                "state": "SEALED",
                "sealed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "sealed_reclaimed_bytes": recuperados,
            }
        )
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
    task_id = resolver_task_propria(args.task_id)
    slot = descobrir_slot(raiz, task_id)
    if slot is None:
        raise ForjaError("NOT_FOUND", f"nenhum task root observado para {task_id}")
    task_root = raiz / slot
    binding = ler_binding(task_root)
    exigir_dono(binding, task_id)

    provas, identidade = guardas_de_destruicao(main, raiz, task_root, binding)

    bytes_antes, arquivos_antes = medir(task_root)
    worktree = task_root / "worktree"
    registrada = worktree_registrada(main, worktree)
    # O `git worktree remove` resolve este caminho por TEXTO. Fixar só a
    # identidade do task_root deixava a worktree — o objeto que o Git realmente
    # apaga — sem verificação própria.
    identidade_worktree = None
    if worktree.exists() or worktree.is_symlink():
        st_wt = worktree.lstat()
        identidade_worktree = (st_wt.st_dev, st_wt.st_ino)
    # Toda inspeção falível acontece ANTES do ponto sem volta: se o Git não
    # responder, a Task continua inteira e o erro é sobre a inspeção, não sobre
    # um estado meio destruído.
    metadata_orfa(main)

    if not args.apply:
        emitir(
            {
                "status": "RETIRE_PLAN",
                "schema": SCHEMA_LAYOUT,
                "task_id": task_id,
                "slot": slot,
                "task_root": str(task_root),
                "proofs": provas,
                "process_proof_scope": "INSPECTABLE_ONLY",
                "worktree_registered": registrada,
                "bytes": bytes_antes,
                "files": arquivos_antes,
            }
        )
        return EXIT_OK

    # 1. desregistrar a worktree pelo Git, antes de remover o diretório.
    #    Ignorar o código de saída aqui foi o defeito F2: um desregistro que
    #    falha deixa metadata órfã e o comando ainda reportaria RETIRED.
    if registrada:
        # `git worktree remove --force` desregistra E remove o diretório numa
        # única operação do Git. Fazer isso ANTES da remoção do root encolhe a
        # janela pós-destrutiva ao mínimo: se falhar, nada foi tocado.
        #
        # A identidade é reconferida AQUI e não só antes de `remover_arvore`:
        # esta é a primeira operação destrutiva, e o Git resolve o caminho por
        # texto. Fixar o inode só para a segunda remoção deixaria a primeira
        # apagando o que quer que estivesse no nome naquele instante.
        confirmar_identidade(task_root, identidade)
        if identidade_worktree is not None:
            confirmar_identidade(worktree, identidade_worktree)
        r = git(main, "worktree", "remove", "--force", str(worktree), check=False)
        if r.returncode != 0:
            # "nada foi removido" seria mentira: medido neste host, um subdiretório
            # 0500 faz o Git sair 255 com "Permission denied" DEPOIS de já ter
            # apagado o registro, deixando a worktree no disco e órfã. O estado
            # real é medido e reportado, em vez de presumido pelo código de saída.
            # Não concluir. Dos quatro quadrantes possíveis, só um justificava
            # "nada foi removido", e a versão anterior imprimia essa frase em
            # três — inclusive quando registro E diretório já tinham sumido.
            # Os observáveis são emitidos como estado; a leitura fica com quem
            # lê, que é o único que pode saber o resto.
            ainda_registrada = worktree_registrada(main, worktree)
            no_disco = worktree.exists() or worktree.is_symlink()
            estado = {
                (True, True): "registro presente e diretório presente",
                (True, False): "registro presente e diretório AUSENTE",
                (False, True): "registro AUSENTE e diretório presente (worktree órfã)",
                (False, False): "registro AUSENTE e diretório AUSENTE",
            }[(ainda_registrada, no_disco)]
            raise ForjaError(
                "FAILED",
                f"desregistro da worktree falhou: {r.stderr.strip()[:150]} | "
                f"estado observado: {estado} "
                f"(registro_presente={ainda_registrada}; worktree_no_disco={no_disco}). "
                "Nenhuma conclusão sobre o conteúdo é emitida aqui; "
                "reexecute a retirada depois de resolver a causa.",
            )
        if worktree_registrada(main, worktree):
            raise ForjaError(
                "FAILED",
                "Git reportou sucesso mas a worktree continua registrada; nada foi removido",
            )
    # 2. remover o root físico sem seguir symlinks
    if task_root.exists():
        remover_arvore_sem_seguir_links(task_root, identidade)
    # 3. podar metadata do Git. Este passo é necessariamente pós-destrutivo: o
    #    prune só reconhece a worktree como ida depois que o diretório sumiu.
    #    Por isso a falha aqui NÃO pode se apresentar como "nada aconteceu" — o
    #    root já foi removido, e o relatório precisa dizer isso.
    # `prune` é rede de segurança idempotente: o desregistro já aconteceu antes
    # do ponto sem volta. Se falhar aqui, o remédio é reexecutá-lo — e a
    # mensagem diz exatamente isso, em vez de fingir que nada mudou.
    r = git(main, "worktree", "prune", check=False)
    ausente = not task_root.exists() and not task_root.is_symlink()
    if r.returncode != 0:
        raise ForjaError(
            "FAILED",
            f"prune falhou APÓS a remoção do root (root_removed={ausente}); "
            f"metadata pode ter ficado órfã e exige `git worktree prune` manual: "
            f"{r.stderr.strip()[:160]}",
        )
    # 4. provar ausência de metadata órfã — a prova é obrigatória, não informativa
    orfas = metadata_orfa(main)
    if not ausente:
        raise ForjaError("FAILED", f"task root ainda presente após a remoção: {task_root}")
    if orfas:
        raise ForjaError(
            "FAILED",
            f"metadata Git órfã após a retirada (root_removed={ausente}): {orfas}",
        )
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
            "process_proof_scope": "INSPECTABLE_ONLY",
            "worktree_was_registered": registrada,
            "reclaimed_bytes": bytes_antes,
            "reclaimed_files": arquivos_antes,
            "task_root_absent": ausente,
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
    """Entradas em .git/worktrees cujo diretório de trabalho não existe mais.

    Falha de inspeção é erro, nunca lista vazia: devolver `[]` quando não foi
    possível olhar transformaria "não consegui verificar" em "está limpo", que
    é a mesma inversão fail-open do F3.
    """
    proc = git(main, "rev-parse", "--git-common-dir", check=False)
    if proc.returncode != 0:
        raise ForjaError(
            "FAILED",
            f"impossível inspecionar metadata de worktree: {proc.stderr.strip()[:200]}",
        )
    comum = Path(proc.stdout.strip())
    if not comum.is_absolute():
        comum = main / comum
    base = comum / "worktrees"
    try:
        st_base = base.lstat()
    except FileNotFoundError:
        return []  # ausência genuína: nenhuma worktree registrada
    except OSError as erro:
        raise ForjaError("FAILED", f"metadata de worktree ilegível: {erro}") from erro
    if not stat.S_ISDIR(st_base.st_mode):
        raise ForjaError("FAILED", f"metadata de worktree não é diretório: {base}")
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


def worktrees_desregistradas(main: Path, raiz: Path) -> list[str]:
    """Diretórios `worktree` que existem sem registro Git correspondente."""
    achados: list[str] = []
    for nome, _ in slots_existentes(raiz):
        wt = raiz / nome / "worktree"
        try:
            st = wt.lstat()
        except FileNotFoundError:
            continue  # ausência genuína: slot sem worktree
        except OSError as erro:
            achados.append(f"{wt} (ilegível: {erro.strerror})")
            continue
        # Symlink e tipo inesperado no lugar de uma worktree SÃO estados
        # residuais. Pular era o mesmo fail-open de sempre: o caso anômalo
        # desaparecia justamente da função escrita para encontrá-lo.
        if stat.S_ISLNK(st.st_mode):
            achados.append(f"{wt} (symlink no lugar da worktree)")
            continue
        if not stat.S_ISDIR(st.st_mode):
            achados.append(f"{wt} (não é diretório)")
            continue
        marcador = wt / ".git"
        try:
            marcador.lstat()
        except FileNotFoundError:
            achados.append(f"{wt} (diretório de worktree sem marcador .git)")
            continue
        except OSError as erro:
            achados.append(f"{wt} (.git ilegível: {erro.strerror})")
            continue
        try:
            if not worktree_registrada(main, wt):
                achados.append(str(wt))
        except ForjaError as erro:
            achados.append(f"{wt} (inspeção falhou: {erro.mensagem[:60]})")
    return achados


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

    # Resíduo inverso: worktree no disco cujo registro sumiu. `metadata_orfa`
    # é cega para isto — ela parte de `.git/worktrees`, e aqui a entrada é que
    # não existe mais. Sem esta checagem o estado meio-destruído do F3 ficaria
    # invisível para `verify` e para o invariante de metadata limpa.
    desregistradas = worktrees_desregistradas(main, raiz)
    checagens["unregistered_worktree_dirs"] = desregistradas
    if desregistradas:
        problemas.append(f"worktree presente no disco sem registro Git: {desregistradas}")

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
