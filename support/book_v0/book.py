#!/usr/bin/env python3
"""Experimental, deterministic OperationalCase library for Pinker Issue #520.

Stored text is UNTRUSTED_DATA.  This program never executes case content.
"""

from __future__ import annotations

import argparse
import copy
import fcntl
import hashlib
import json
import os
import re
import sys
import tempfile
import unicodedata
from contextlib import contextmanager
from pathlib import Path
from typing import Any


TOOL_VERSION = "0.1.0"
SCHEMA_VERSION = 1
CONTENT_TRUST = "UNTRUSTED_DATA"

STATUSES = {"candidate", "verified", "challenged", "superseded", "historical"}
EPISTEMIC_CLASSES = {"OBSERVED", "ASSERTED", "VALIDATED"}
CASE_ID_RE = re.compile(r"B-[A-Z0-9]{12}\Z")
CHALLENGE_ID_RE = re.compile(r"C-[A-Z0-9]{10}\Z")
HASH_RE = re.compile(r"[0-9a-f]{64}\Z")
UTC_RE = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
TRAMA_RE = re.compile(r"trama:[a-z0-9][a-z0-9._-]*\Z")
GIT_RE = re.compile(r"git:[0-9a-f]{40}\Z")
TOKEN_RE = re.compile(r"[a-z0-9]+(?:[._-][a-z0-9]+)*")

MAX_FILE_BYTES = 128 * 1024
MAX_TEXT = 8192
MAX_SHORT_TEXT = 512
MAX_LIST = 32

CASE_FIELDS = {
    "id",
    "schema_version",
    "title",
    "cues",
    "scope",
    "observed_at",
    "environment",
    "problem",
    "discriminating_probe",
    "observed_result",
    "guidance",
    "contraindications",
    "evidence",
    "references",
    "status",
    "challenges",
    "revision",
}
EDITABLE_FIELDS = CASE_FIELDS - {"id", "schema_version", "challenges", "revision"}
REVISION_DRAFT_FIELDS = {"updated_at", "updated_by", "reason"}
REVISION_FIELDS = REVISION_DRAFT_FIELDS | {"number", "parent_hash", "hash"}
CHALLENGE_FIELDS = {
    "id",
    "observed_at",
    "reported_by",
    "statement",
    "evidence",
    "references",
}

SECRET_PATTERNS = (
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"\bghp_[A-Za-z0-9]{20,}\b"),
    re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(r"\bsk-[A-Za-z0-9]{20,}\b"),
    re.compile(
        r"\b(?:password|passwd|pwd|secret|token|credential|private[_-]?key)\s*[:=]\s*[^\s,;]+",
        re.I,
    ),
)
SENSITIVE_KEY_RE = re.compile(r"(?:secret|token|password|credential|private[_-]?key)", re.I)


class BookError(Exception):
    def __init__(self, code: str, message: str, details: Any = None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def revision_hash(case: dict[str, Any]) -> str:
    payload = copy.deepcopy(case)
    payload.get("revision", {}).pop("hash", None)
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def semantic_fingerprint(case: dict[str, Any]) -> str:
    payload = copy.deepcopy(case)
    payload.pop("id", None)
    payload.pop("revision", None)
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def normalize_tokens(text: str) -> list[str]:
    folded = unicodedata.normalize("NFKD", text.casefold())
    folded = "".join(ch for ch in folded if not unicodedata.combining(ch))
    return TOKEN_RE.findall(folded)


def require_object(value: Any, where: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise BookError("SCHEMA_INVALID", f"{where} must be an object")
    return value


def require_exact_fields(value: dict[str, Any], fields: set[str], where: str) -> None:
    missing = sorted(fields - set(value))
    unknown = sorted(set(value) - fields)
    if missing or unknown:
        raise BookError(
            "SCHEMA_INVALID",
            f"{where} fields are invalid",
            {"missing": missing, "unknown": unknown},
        )


def require_text(value: Any, where: str, limit: int = MAX_TEXT) -> str:
    if not isinstance(value, str) or not value.strip():
        raise BookError("SCHEMA_INVALID", f"{where} must be non-empty text")
    if len(value) > limit:
        raise BookError("CONTENT_LIMIT", f"{where} exceeds {limit} characters")
    for pattern in SECRET_PATTERNS:
        if pattern.search(value):
            raise BookError("SENSITIVE_CONTENT", f"{where} resembles a secret")
    return value


def validate_text_list(
    value: Any, where: str, *, allow_empty: bool, limit: int = MAX_SHORT_TEXT
) -> None:
    if not isinstance(value, list):
        raise BookError("SCHEMA_INVALID", f"{where} must be a list")
    if len(value) > MAX_LIST or (not allow_empty and not value):
        raise BookError("CONTENT_LIMIT", f"{where} must contain 1..{MAX_LIST} items")
    for index, item in enumerate(value):
        require_text(item, f"{where}[{index}]", limit)
    if len(value) != len(set(value)):
        raise BookError("SCHEMA_INVALID", f"{where} contains duplicate values")


def validate_timestamp(value: Any, where: str, *, allow_unknown: bool = False) -> None:
    if value is None and allow_unknown:
        return
    if not isinstance(value, str) or not UTC_RE.fullmatch(value):
        suffix = " or null (UNKNOWN)" if allow_unknown else ""
        raise BookError("SCHEMA_INVALID", f"{where} must be UTC YYYY-MM-DDTHH:MM:SSZ{suffix}")


def validate_reference(reference: Any, where: str) -> None:
    require_text(reference, where, MAX_SHORT_TEXT)
    if TRAMA_RE.fullmatch(reference) or GIT_RE.fullmatch(reference):
        return
    if reference.startswith("file:"):
        target = reference[5:]
        if target and "\x00" not in target and not any(part == ".." for part in Path(target).parts):
            return
    raise BookError("REFERENCE_INVALID", f"{where} has unsupported or invalid syntax")


def validate_references(value: Any, where: str) -> None:
    if not isinstance(value, list) or len(value) > MAX_LIST:
        raise BookError("CONTENT_LIMIT", f"{where} must be a list of at most {MAX_LIST} references")
    for index, reference in enumerate(value):
        validate_reference(reference, f"{where}[{index}]")
    if len(value) != len(set(value)):
        raise BookError("SCHEMA_INVALID", f"{where} contains duplicate references")


def validate_claim(value: Any, where: str, allowed: set[str] | None = None) -> None:
    claim = require_object(value, where)
    claim_class = claim.get("class")
    expected_fields = {"class", "text", "oracle"} if claim_class == "VALIDATED" else {"class", "text"}
    require_exact_fields(claim, expected_fields, where)
    if claim_class not in (allowed or EPISTEMIC_CLASSES):
        raise BookError("EPISTEMIC_CLASS_INVALID", f"{where}.class is invalid")
    require_text(claim["text"], f"{where}.text")
    if claim_class == "VALIDATED":
        require_text(claim["oracle"], f"{where}.oracle")


def validate_evidence(value: Any, where: str, *, allow_empty: bool = False) -> None:
    if not isinstance(value, list) or len(value) > MAX_LIST or (not allow_empty and not value):
        raise BookError("CONTENT_LIMIT", f"{where} must contain 1..{MAX_LIST} entries")
    for index, item in enumerate(value):
        evidence = require_object(item, f"{where}[{index}]")
        evidence_class = evidence.get("class")
        fields = {"class", "description", "source", "oracle"} if evidence_class == "VALIDATED" else {"class", "description", "source"}
        require_exact_fields(evidence, fields, f"{where}[{index}]")
        if evidence_class not in EPISTEMIC_CLASSES:
            raise BookError("EPISTEMIC_CLASS_INVALID", f"{where}[{index}].class is invalid")
        require_text(evidence["description"], f"{where}[{index}].description")
        validate_reference(evidence["source"], f"{where}[{index}].source")
        if evidence_class == "VALIDATED":
            require_text(evidence["oracle"], f"{where}[{index}].oracle")


def validate_scope(value: Any) -> None:
    if value is None:  # Explicit UNKNOWN, never APPLIES_TO_ALL.
        return
    scope = require_object(value, "scope")
    require_exact_fields(scope, {"components", "conditions"}, "scope")
    validate_text_list(scope["components"], "scope.components", allow_empty=True)
    validate_text_list(scope["conditions"], "scope.conditions", allow_empty=True)
    if not scope["components"] and not scope["conditions"]:
        raise BookError("SCHEMA_INVALID", "empty scope must be represented as null (UNKNOWN)")


def validate_environment(value: Any) -> None:
    if value is None:  # Explicit UNKNOWN.
        return
    environment = require_object(value, "environment")
    if not environment or len(environment) > MAX_LIST:
        raise BookError("CONTENT_LIMIT", "environment must have 1..32 entries or be null")
    for key, item in environment.items():
        require_text(key, "environment key", 64)
        require_text(item, f"environment.{key}", MAX_SHORT_TEXT)
        if SENSITIVE_KEY_RE.search(key):
            raise BookError("SENSITIVE_CONTENT", f"environment.{key} is a sensitive field")


def validate_probe(value: Any) -> None:
    if value is None:
        return
    probe = require_object(value, "discriminating_probe")
    require_exact_fields(probe, {"action", "observable"}, "discriminating_probe")
    require_text(probe["action"], "discriminating_probe.action")
    require_text(probe["observable"], "discriminating_probe.observable")


def validate_challenge(value: Any, where: str = "challenge") -> None:
    challenge = require_object(value, where)
    require_exact_fields(challenge, CHALLENGE_FIELDS, where)
    if not isinstance(challenge["id"], str) or not CHALLENGE_ID_RE.fullmatch(challenge["id"]):
        raise BookError("CHALLENGE_ID_INVALID", f"{where}.id must match C-[A-Z0-9]{{10}}")
    validate_timestamp(challenge["observed_at"], f"{where}.observed_at", allow_unknown=True)
    require_text(challenge["reported_by"], f"{where}.reported_by", 128)
    validate_claim(challenge["statement"], f"{where}.statement")
    validate_evidence(challenge["evidence"], f"{where}.evidence")
    validate_references(challenge["references"], f"{where}.references")


def validate_revision(value: Any, case: dict[str, Any]) -> None:
    revision = require_object(value, "revision")
    require_exact_fields(revision, REVISION_FIELDS, "revision")
    number = revision["number"]
    if not isinstance(number, int) or isinstance(number, bool) or number < 1:
        raise BookError("REVISION_INVALID", "revision.number must be a positive integer")
    parent = revision["parent_hash"]
    if number == 1 and parent is not None:
        raise BookError("REVISION_INVALID", "initial revision must have null parent_hash")
    if number > 1 and (not isinstance(parent, str) or not HASH_RE.fullmatch(parent)):
        raise BookError("REVISION_INVALID", "later revision must have a SHA-256 parent_hash")
    validate_timestamp(revision["updated_at"], "revision.updated_at")
    require_text(revision["updated_by"], "revision.updated_by", 128)
    require_text(revision["reason"], "revision.reason", MAX_SHORT_TEXT)
    if not isinstance(revision["hash"], str) or not HASH_RE.fullmatch(revision["hash"]):
        raise BookError("REVISION_INVALID", "revision.hash must be SHA-256")
    expected = revision_hash(case)
    if revision["hash"] != expected:
        raise BookError(
            "REVISION_HASH_MISMATCH",
            "revision.hash does not match canonical case content",
            {"expected": expected, "actual": revision["hash"]},
        )


def validate_case(case: Any) -> dict[str, Any]:
    case = require_object(case, "case")
    require_exact_fields(case, CASE_FIELDS, "case")
    if type(case["schema_version"]) is not int or case["schema_version"] != SCHEMA_VERSION:
        code = "SCHEMA_FUTURE_UNSUPPORTED" if isinstance(case["schema_version"], int) and case["schema_version"] > SCHEMA_VERSION else "SCHEMA_UNSUPPORTED"
        raise BookError(code, f"unsupported schema_version {case['schema_version']!r}")
    if not isinstance(case["id"], str) or not CASE_ID_RE.fullmatch(case["id"]):
        raise BookError("CASE_ID_INVALID", "id must match B-[A-Z0-9]{12}")
    require_text(case["title"], "title", MAX_SHORT_TEXT)
    validate_text_list(case["cues"], "cues", allow_empty=False)
    validate_scope(case["scope"])
    validate_timestamp(case["observed_at"], "observed_at", allow_unknown=True)
    validate_environment(case["environment"])
    validate_claim(case["problem"], "problem", {"OBSERVED", "ASSERTED"})
    validate_probe(case["discriminating_probe"])
    validate_claim(case["observed_result"], "observed_result", {"OBSERVED", "VALIDATED"})
    validate_claim(case["guidance"], "guidance")
    validate_text_list(case["contraindications"], "contraindications", allow_empty=True)
    validate_evidence(case["evidence"], "evidence")
    validate_references(case["references"], "references")
    if case["status"] not in STATUSES:
        raise BookError("STATUS_INVALID", f"unsupported status {case['status']!r}")
    if not isinstance(case["challenges"], list) or len(case["challenges"]) > MAX_LIST:
        raise BookError("CONTENT_LIMIT", "challenges must be a list of at most 32 entries")
    challenge_ids: set[str] = set()
    for index, challenge in enumerate(case["challenges"]):
        validate_challenge(challenge, f"challenges[{index}]")
        if challenge["id"] in challenge_ids:
            raise BookError("CHALLENGE_ID_DUPLICATE", f"duplicate challenge id {challenge['id']}")
        challenge_ids.add(challenge["id"])
    if case["status"] == "challenged" and not case["challenges"]:
        raise BookError("SCHEMA_INVALID", "challenged status requires at least one challenge")
    validate_revision(case["revision"], case)
    return case


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise BookError("JSON_DUPLICATE_KEY", f"duplicate JSON key {key!r}")
        value[key] = item
    return value


def read_json(path: Path) -> Any:
    try:
        size = path.stat().st_size
    except OSError as exc:
        raise BookError("READ_FAILED", f"cannot stat {path}: {exc.strerror}") from exc
    if size > MAX_FILE_BYTES:
        raise BookError("CONTENT_LIMIT", f"{path} exceeds {MAX_FILE_BYTES} bytes")
    try:
        return json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicate_keys
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise BookError("PARSE_FAILED", f"cannot parse {path}: {exc}") from exc


def cases_dir(book_root: Path) -> Path:
    return book_root / "cases"


def case_files(book_root: Path) -> list[Path]:
    directory = cases_dir(book_root)
    if not directory.exists():
        return []
    if directory.is_symlink():
        raise BookError("STRUCTURE_INVALID", f"{directory} must not be a symbolic link")
    if not directory.is_dir():
        raise BookError("STRUCTURE_INVALID", f"{directory} is not a directory")
    return sorted(directory.glob("*.json"), key=lambda path: path.name)


def load_case_file(path: Path, *, require_matching_filename: bool = True) -> dict[str, Any]:
    case = validate_case(read_json(path))
    if require_matching_filename and path.stem != case["id"]:
        raise BookError("STRUCTURE_INVALID", f"filename {path.name} does not match id {case['id']}")
    return case


def load_corpus(book_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    seen: set[str] = set()
    for path in case_files(book_root):
        case = load_case_file(path)
        if case["id"] in seen:
            raise BookError("CASE_ID_DUPLICATE", f"duplicate case id {case['id']}")
        seen.add(case["id"])
        cases.append(case)
    return cases


def searchable_fields(case: dict[str, Any]) -> dict[str, str]:
    scope = case["scope"]
    scope_text = "" if scope is None else " ".join(scope["components"] + scope["conditions"])
    return {
        "title": case["title"],
        "cues": " ".join(case["cues"]),
        "scope": scope_text,
        "problem": case["problem"]["text"],
        "guidance": case["guidance"]["text"],
    }


def lexical_score(case: dict[str, Any], query_tokens: set[str]) -> tuple[int, list[str]]:
    weights = {"title": 5, "cues": 4, "scope": 3, "problem": 2, "guidance": 1}
    score = 0
    for field, text in searchable_fields(case).items():
        score += weights[field] * len(query_tokens & set(normalize_tokens(text)))
    matched_cues = sorted(query_tokens & set(normalize_tokens(" ".join(case["cues"]))))
    return score, matched_cues


def compact_candidate(case: dict[str, Any], score: int, matched_cues: list[str]) -> dict[str, Any]:
    scope = case["scope"]
    if scope is not None:
        scope = {
            "components": [item[:160] for item in scope["components"][:5]],
            "conditions": [item[:160] for item in scope["conditions"][:3]],
            "truncated": len(scope["components"]) > 5
            or len(scope["conditions"]) > 3
            or any(len(item) > 160 for item in scope["components"] + scope["conditions"]),
        }
    return {
        "id": case["id"],
        "title": case["title"],
        "status": case["status"],
        "score": score,
        "matched_cues": [cue[:80] for cue in matched_cues[:8]],
        "matched_cues_truncated": len(matched_cues) > 8
        or any(len(cue) > 80 for cue in matched_cues),
        "scope": scope,
        "revision": case["revision"]["hash"],
    }


def search(book_root: Path, query: str) -> dict[str, Any]:
    require_text(query, "query", MAX_SHORT_TEXT)
    tokens = set(normalize_tokens(query))
    if not tokens:
        raise BookError("QUERY_INVALID", "query must contain a lexical token")
    results = []
    for case in load_corpus(book_root):
        score, matched_cues = lexical_score(case, tokens)
        if score:
            results.append(compact_candidate(case, score, matched_cues))
    results.sort(key=lambda item: (-item["score"], item["id"]))
    return {"content_trust": CONTENT_TRUST, "query": query, "count": len(results), "results": results}


def find_case(book_root: Path, case_id: str) -> dict[str, Any]:
    if not CASE_ID_RE.fullmatch(case_id):
        raise BookError("CASE_ID_INVALID", "id must match B-[A-Z0-9]{12}")
    matches = [case for case in load_corpus(book_root) if case["id"] == case_id]
    if not matches:
        raise BookError("CASE_NOT_FOUND", f"case {case_id} not found")
    return matches[0]


def set_revision(
    case: dict[str, Any], *, number: int, parent_hash: str | None, updated_at: str, updated_by: str, reason: str
) -> None:
    case["revision"] = {
        "number": number,
        "parent_hash": parent_hash,
        "updated_at": updated_at,
        "updated_by": updated_by,
        "reason": reason,
        "hash": "0" * 64,
    }
    case["revision"]["hash"] = revision_hash(case)


def prepare_new_case(draft: Any) -> dict[str, Any]:
    case = require_object(copy.deepcopy(draft), "case draft")
    require_exact_fields(case, CASE_FIELDS, "case draft")
    revision = require_object(case["revision"], "case draft.revision")
    require_exact_fields(revision, REVISION_DRAFT_FIELDS, "case draft.revision")
    if case["challenges"] != []:
        raise BookError("SCHEMA_INVALID", "new case must start with an empty challenges list")
    set_revision(
        case,
        number=1,
        parent_hash=None,
        updated_at=revision["updated_at"],
        updated_by=revision["updated_by"],
        reason=revision["reason"],
    )
    return validate_case(case)


def candidate_tokens(case: dict[str, Any]) -> set[str]:
    return set(normalize_tokens(" ".join(searchable_fields(case).values())))


def similar_candidates(new_case: dict[str, Any], corpus: list[dict[str, Any]]) -> list[dict[str, Any]]:
    new_tokens = candidate_tokens(new_case)
    candidates = []
    for case in corpus:
        old_tokens = candidate_tokens(case)
        union = new_tokens | old_tokens
        similarity = 0.0 if not union else len(new_tokens & old_tokens) / len(union)
        shared_cues = sorted(set(normalize_tokens(" ".join(new_case["cues"]))) & set(normalize_tokens(" ".join(case["cues"]))))
        if similarity >= 0.30 or shared_cues:
            candidates.append(
                {"id": case["id"], "title": case["title"], "similarity": round(similarity, 6), "shared_cues": shared_cues}
            )
    candidates.sort(key=lambda item: (-item["similarity"], item["id"]))
    return candidates[:5]


def atomic_write(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_name, path)
        directory_descriptor = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except BaseException:
        try:
            os.unlink(temporary_name)
        except FileNotFoundError:
            pass
        raise


@contextmanager
def exclusive_book_lock(book_root: Path):
    """Serialize read-check-write operations without adding a persistent lock file."""
    directory = cases_dir(book_root)
    directory.mkdir(parents=True, exist_ok=True)
    if directory.is_symlink():
        raise BookError("STRUCTURE_INVALID", f"{directory} must not be a symbolic link")
    descriptor = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def add_case(book_root: Path, input_path: Path, allow_similar: bool) -> dict[str, Any]:
    new_case = prepare_new_case(read_json(input_path))
    with exclusive_book_lock(book_root):
        corpus = load_corpus(book_root)
        if any(case["id"] == new_case["id"] for case in corpus):
            raise BookError("CASE_ID_DUPLICATE", f"case id {new_case['id']} already exists")
        fingerprint = semantic_fingerprint(new_case)
        exact = [case["id"] for case in corpus if semantic_fingerprint(case) == fingerprint]
        if exact:
            raise BookError(
                "CASE_EXACT_DUPLICATE", "exact case content already exists", {"candidates": exact}
            )
        candidates = similar_candidates(new_case, corpus)
        if candidates and not allow_similar:
            raise BookError(
                "CASE_SIMILAR_CANDIDATES",
                "lexically similar cases require an explicit new-case decision",
                {
                    "candidates": candidates,
                    "hint": "use revise or repeat add-case with --allow-similar",
                },
            )
        destination = cases_dir(book_root) / f"{new_case['id']}.json"
        if destination.exists():
            raise BookError("CASE_ID_DUPLICATE", f"destination {destination} already exists")
        atomic_write(destination, new_case)
    return {"ok": True, "operation": "add", "id": new_case["id"], "revision": new_case["revision"]["hash"], "similar_candidates": candidates}


def next_revision(
    case: dict[str, Any], *, updated_at: str, updated_by: str, reason: str
) -> None:
    current = case["revision"]
    set_revision(
        case,
        number=current["number"] + 1,
        parent_hash=current["hash"],
        updated_at=updated_at,
        updated_by=updated_by,
        reason=reason,
    )


def require_current_revision(case: dict[str, Any], expected: str) -> None:
    if case["revision"]["hash"] != expected:
        raise BookError(
            "REVISION_CONFLICT",
            "current revision differs from --if-revision",
            {"expected": expected, "current": case["revision"]["hash"]},
        )


def revise_case(
    book_root: Path,
    case_id: str,
    expected: str,
    patch_path: Path,
    updated_at: str,
    updated_by: str,
    reason: str,
) -> dict[str, Any]:
    patch = require_object(read_json(patch_path), "revision patch")
    invalid = sorted(set(patch) - EDITABLE_FIELDS)
    if invalid or not patch:
        raise BookError(
            "PATCH_INVALID", "revision patch has invalid or no fields", {"invalid": invalid}
        )
    with exclusive_book_lock(book_root):
        case = find_case(book_root, case_id)
        require_current_revision(case, expected)
        for key, value in patch.items():
            case[key] = value
        next_revision(case, updated_at=updated_at, updated_by=updated_by, reason=reason)
        validate_case(case)
        atomic_write(cases_dir(book_root) / f"{case_id}.json", case)
    return {"ok": True, "operation": "revise", "id": case_id, "revision": case["revision"]["hash"], "parent_revision": expected}


def challenge_case(
    book_root: Path,
    case_id: str,
    expected: str,
    challenge_path: Path,
    updated_at: str,
    updated_by: str,
    reason: str,
) -> dict[str, Any]:
    challenge = read_json(challenge_path)
    validate_challenge(challenge)
    with exclusive_book_lock(book_root):
        case = find_case(book_root, case_id)
        require_current_revision(case, expected)
        if case["status"] in {"superseded", "historical"}:
            raise BookError(
                "STATUS_TRANSITION_INVALID", f"cannot challenge a {case['status']} case"
            )
        if any(item["id"] == challenge["id"] for item in case["challenges"]):
            raise BookError(
                "CHALLENGE_ID_DUPLICATE", f"challenge id {challenge['id']} already exists"
            )
        case["challenges"].append(challenge)
        case["status"] = "challenged"
        next_revision(case, updated_at=updated_at, updated_by=updated_by, reason=reason)
        validate_case(case)
        atomic_write(cases_dir(book_root) / f"{case_id}.json", case)
    return {
        "ok": True,
        "operation": "challenge",
        "id": case_id,
        "challenge_id": challenge["id"],
        "status": case["status"],
        "revision": case["revision"]["hash"],
        "parent_revision": expected,
    }


def verify_corpus(book_root: Path) -> tuple[dict[str, Any], int]:
    errors = []
    ids: dict[str, list[str]] = {}
    files = []
    try:
        files = case_files(book_root)
    except BookError as exc:
        errors.append({"path": str(book_root), "code": exc.code, "message": exc.message})
    valid_count = 0
    for path in files:
        try:
            case = load_case_file(path, require_matching_filename=False)
            valid_count += 1
            ids.setdefault(case["id"], []).append(str(path))
            if path.stem != case["id"]:
                errors.append(
                    {
                        "path": str(path),
                        "code": "STRUCTURE_INVALID",
                        "message": f"filename {path.name} does not match id {case['id']}",
                    }
                )
        except BookError as exc:
            item = {"path": str(path), "code": exc.code, "message": exc.message}
            if exc.details is not None:
                item["details"] = exc.details
            errors.append(item)
    for case_id, paths in sorted(ids.items()):
        if len(paths) > 1:
            errors.append({"path": paths, "code": "CASE_ID_DUPLICATE", "message": f"duplicate case id {case_id}"})
    result = {
        "ok": not errors,
        "operation": "verify",
        "tool_version": TOOL_VERSION,
        "schema_version": SCHEMA_VERSION,
        "files": len(files),
        "valid_cases": valid_count,
        "errors": errors,
    }
    return result, 0 if not errors else 1


def emit(value: Any, *, compact: bool = False) -> None:
    json.dump(value, sys.stdout, ensure_ascii=False, sort_keys=True, indent=None if compact else 2)
    sys.stdout.write("\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="book", description="Experimental Pinker OperationalCase library")
    parser.add_argument("--book", type=Path, default=Path(__file__).resolve().parent, help="Book root containing cases/")
    parser.add_argument("--version", action="store_true", help="show tool/schema version and exit")
    subparsers = parser.add_subparsers(dest="command")

    search_parser = subparsers.add_parser("search", help="lexically search cases")
    search_parser.add_argument("query")
    show_parser = subparsers.add_parser("show", help="show an exact case")
    show_parser.add_argument("id")
    add_parser = subparsers.add_parser("add-case", help="validate and atomically add a case draft")
    add_parser.add_argument("input", type=Path)
    add_parser.add_argument("--allow-similar", action="store_true")

    revise_parser = subparsers.add_parser("revise", help="optimistically revise an existing case")
    revise_parser.add_argument("id")
    revise_parser.add_argument("--if-revision", required=True)
    revise_parser.add_argument("--patch", required=True, type=Path)
    revise_parser.add_argument("--updated-at", required=True)
    revise_parser.add_argument("--updated-by", required=True)
    revise_parser.add_argument("--reason", required=True)

    challenge_parser = subparsers.add_parser("challenge", help="record contrary evidence without erasing history")
    challenge_parser.add_argument("id")
    challenge_parser.add_argument("--if-revision", required=True)
    challenge_parser.add_argument("--challenge", required=True, type=Path)
    challenge_parser.add_argument("--updated-at", required=True)
    challenge_parser.add_argument("--updated-by", required=True)
    challenge_parser.add_argument("--reason", required=True)

    subparsers.add_parser("verify", help="deterministically verify the corpus")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.version:
        emit({"tool_version": TOOL_VERSION, "schema_version": SCHEMA_VERSION})
        return 0
    if args.command is None:
        parser.error("a command is required")
    try:
        if args.command == "search":
            emit(search(args.book, args.query))
        elif args.command == "show":
            emit({"content_trust": CONTENT_TRUST, "case": find_case(args.book, args.id)})
        elif args.command == "add-case":
            emit(add_case(args.book, args.input, args.allow_similar))
        elif args.command == "revise":
            emit(revise_case(args.book, args.id, args.if_revision, args.patch, args.updated_at, args.updated_by, args.reason))
        elif args.command == "challenge":
            emit(challenge_case(args.book, args.id, args.if_revision, args.challenge, args.updated_at, args.updated_by, args.reason))
        elif args.command == "verify":
            result, status = verify_corpus(args.book)
            emit(result)
            return status
        return 0
    except BookError as exc:
        error = {"ok": False, "error": exc.code, "message": exc.message}
        if exc.details is not None:
            error["details"] = exc.details
        emit(error)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
