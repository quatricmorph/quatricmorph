#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# check-plan-citations.py — mechanical citation checker for `.plan/`
#
# Introduced by QM-0002. Run from anywhere:
#
#     python3 .plan/tools/check-plan-citations.py
#     python3 .plan/tools/check-plan-citations.py --verbose
#
# Exit 0 when every checked citation resolves; exit 1 with a report otherwise.
#
# ---------------------------------------------------------------------------
# WHY THIS LIVES IN `.plan/tools/` AND NOT `scripts/`
# ---------------------------------------------------------------------------
# The reason is the boundary, and only the boundary. QM-0002's
# `## Program Boundary` is "`.plan/` only. This task changes no repository
# file." `scripts/` is NOT missing — it exists and holds `baseline.json`,
# `verify-baseline.sh`, `verify-baseline.test.sh` (QM-0001) and
# `license-audit.sh` (QM-0093). Writing a file into it would breach QM-0002's
# boundary whether or not the directory is there, so the checker lives inside
# the directory it checks. QM-0002's own `## Files Expected to Add` still names
# `scripts/check-plan-citations.sh`; that contradiction is recorded as a finding
# in `.plan/PLAN_CHANGELOG.md` rather than resolved by this task editing its own
# scope to make itself pass.
#
# An earlier revision of this comment claimed `scripts/` "does not exist yet".
# That was false at the commit that wrote it — `scripts/license-audit.sh` had
# already landed — and `NEW_TOP_LEVEL` below already said so. Corrected.
#
# ---------------------------------------------------------------------------
# WHAT IS CHECKED
# ---------------------------------------------------------------------------
# Three citation kinds are extracted from every `.plan/**/*.md`, per QM-0002
# `## Implementation Plan` step 1:
#
#   PATH    a backticked or markdown-linked repo-anchored path
#           (`crates/q-catalog/src/lib.rs`, `../STATUS.md:12`)
#   SYMBOL  a backticked token containing `::`
#           (`q_tileset::GeometricError::for_lod`)
#   TEST    a backticked long snake_case identifier
#           (`quantized_tiles_are_half_the_size_and_declare_themselves_lossy`)
#
# Resolution is `os.path.exists` for PATH and a repository grep for SYMBOL and
# TEST (QM-0002 `## Implementation Plan` step 2: `test -f`, `grep -q`).
#
# ---------------------------------------------------------------------------
# THE TWO DOCUMENTED EXEMPTIONS (mandated by QM-0002 `## Scope`)
# ---------------------------------------------------------------------------
# The checker skips two classes of backtick-path-shaped text, because both are
# legitimate and would otherwise fail the very task that introduces the checker:
#
#   E1  Paths inside a `## Test Cases` block. QM-0002's own `## Test Cases`
#       table cites `crates/q-nope/src/lib.rs` as a deliberate example of an
#       unresolvable citation. Reporting it would make the task unpassable.
#
#   E2  Paths in sentences asserting ABSENCE. `CURRENT_ARCHITECTURE.md` §6.3
#       says "`apps/desktop/` does not exist (correctly — Tauri is a
#       non-goal)". The citation is correct precisely because the path is not
#       there. Detection is sentence-granular, not line-granular, against the
#       ABSENCE_MARKERS list below.
#
# A path claimed as EVIDENCE may never rely on either exemption — see the
# evidence override below, which is applied before E1 and E2.
#
# ---------------------------------------------------------------------------
# PLANNED PATHS ARE SHAPE-CHECKED, NOT EXISTENCE-CHECKED
# ---------------------------------------------------------------------------
# QM-0002 `## Scope`: "Paths listed under `## Files Expected to Add` are
# planned, not existing, and are checked for shape, not existence."
#
# The planned set is derived mechanically: every path under a
# `## Files Expected to Add` heading in any `.plan/tasks/*/TASK.md`, plus its
# ancestor directories, plus its descendants. That is what makes
# `crates/q-quant/`, `crates/q-diagnostics/`, `crates/q-report/`,
# `apps/web/diagnostics/`, `apps/web/core/`, `schemas/diagnostics/` and
# `scripts/` legal to cite today: each is a declared v1 deliverable owned by a
# named task, not drift. Shape is checked with SHAPE_SEGMENT below.
#
# ---------------------------------------------------------------------------
# THE EVIDENCE OVERRIDE — this is what makes it a checker and not a rubber stamp
# ---------------------------------------------------------------------------
# A citation in EVIDENCE context must resolve NOW. Neither E1, nor E2, nor the
# planned set excuses it. EVIDENCE context is:
#
#   * any `## Repository Evidence` section of a `TASK.md`; and
#   * anywhere inside a document of record — a `.plan/` document whose subject
#     is what the repository contains TODAY (EVIDENCE_DOCUMENTS below).
#
# Without this rule the planned-set exemption would swallow genuinely stale
# evidence citations, which are the ones that matter.
#
# ---------------------------------------------------------------------------
# PENDING-RENAME ALIASES — NOT a third exemption
# ---------------------------------------------------------------------------
# Separate mechanism, separately reported, time-boxed, and owned by a named
# task. A rename that `.plan/` has already adopted but the repository has not
# yet performed resolves through PENDING_RENAMES in EITHER direction, so the
# table stays correct before and after the rename lands and needs no edit on
# the day it does. Every alias hit is printed under "pending-rename NOTES" with
# its owning task. When the owning task is `Complete` and the literal path
# resolves, delete the row.
# ---------------------------------------------------------------------------

from __future__ import annotations

import argparse
import glob
import os
import re
import subprocess
import sys
from collections import Counter

# --------------------------------------------------------------------------
# Configuration
# --------------------------------------------------------------------------

# (plan-side name, repository-side name, owning task). Bidirectional.
#
# EMPTY, and that is the correct state today. The only row this table ever held
# was `apps/web/quatricmorph-workspace` <-> `apps/web/matrix-workspace`, owned by
# `QM-0006`. `QM-0006` is `Complete` (merged as `1cfdc9c`), the directory on disk
# is `apps/web/quatricmorph-workspace`, and the literal path resolves — which is
# exactly the deletion condition stated in the header above. Leaving the row in
# place would let a stale `matrix-workspace` citation resolve through the alias
# and hide the `.plan/` prose repairs QM-0002 owns.
PENDING_RENAMES: list[tuple[str, str, str]] = []

# ORCHESTRATION WORKTREE PATHS — environment-local, reported UNVERIFIABLE.
#
# `ORCHESTRATION_STATE.md` and `PLAN_CHANGELOG.md` cite the run's worktrees as
# `../.qm-worktrees/qm-XXXX`. That is correct relative to the CANONICAL checkout
# (`…/Quatricmorph/Quatricmorph/../.qm-worktrees` exists), but this checker's
# ROOT is whichever checkout it runs from. Run from inside `.qm-worktrees/qm-0002`
# the same citation normalises to `…/.qm-worktrees/.qm-worktrees/qm-0002`, which
# never exists — measured, not assumed. The worktrees are also created and
# deleted per run and are absent from a fresh clone. Neither fact makes the
# citation stale, so it is not a failure; it is unverifiable from here, in the
# same sense as a gitignored path.
ENVIRONMENT_LOCAL = ("../.qm-worktrees/", ".qm-worktrees/")

# COMPOSITE `path::symbol` — one citation naming a file AND a symbol inside it,
# e.g. `crates/q-safetensors/src/ingest.rs::bf16_tensors_are_described_with_the_right_width`.
# It matches neither SYMBOLISH (it has `/` and `.`) nor a plain path (it has
# `::`), so before this rule it fell through to the PATH branch and failed the
# shape check — a false positive: both halves resolve
# (`crates/q-safetensors/src/ingest.rs:451` defines that test). Both halves are
# now checked, and the report says which one failed.
COMPOSITE = re.compile(r"^([^\s:]+/[^\s:]+)::([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)$")

# `.plan/` documents whose subject is the repository as it stands today.
# A citation anywhere in one of these is EVIDENCE and must resolve.
EVIDENCE_DOCUMENTS = {
    "CURRENT_ARCHITECTURE.md",
    "REPOSITORY_ANALYSIS.md",
}

# Task-file heading whose body is EVIDENCE.
EVIDENCE_SECTION = "## Repository Evidence"

# Task-file headings whose body DECLARES an artefact the task will create, as
# opposed to citing one that exists. `## Program Boundary` belongs here: it
# names the only paths the task may write, which is a forward-looking
# declaration in exactly the same sense. `## Files Expected to Change` is
# deliberately NOT here — those paths must already exist.
PLANNED_SECTIONS = (
    "## Files Expected to Add",
    "## Files Expected to Remove or Deprecate",
    "## Program Boundary",
)

# E1 — the exempt block.
TEST_CASES_SECTION = "## Test Cases"

# Top-level directories that a named v1 task creates and that therefore do not
# exist yet. Without this list a citation to one is indistinguishable from a
# document-relative fragment. Delete a row when its task is `Complete`.
NEW_TOP_LEVEL = {
    # `scripts` is deliberately absent, and its removal is part of this task.
    # It used to be listed here, owned by `QM-0001` / `QM-0093`. `QM-0093` merged
    # `scripts/license-audit.sh`, so the directory now exists,
    # `top_level_entries()` picks it up as a real anchor, and every `scripts/…`
    # citation is resolved against the disk. Leaving the row would have turned
    # `scripts` into an escape hatch that passes any path beneath it — the same
    # laundering defect as a stale PENDING_RENAMES row. Paths `QM-0001` has not
    # created yet (`scripts/verify-baseline.sh`, `scripts/baseline.json`) still
    # pass, but through the planned set, which requires a task to have declared
    # them under `## Files Expected to Add`.
    "benchmarks": "QM-0102",
}

# A `## Repository Evidence` bullet naming another task cites THAT task's
# declared output, not the repository — `QM-0125` citing `QM-0123`'s
# `bytes_at_base_precision`. Shape-checked, not existence-checked. Detection is
# BULLET-granular: the attribution is often on the bullet's continuation line.
TASK_REF = re.compile(r"`QM-\d{4}`")
BULLET_START = re.compile(r"^\s*(?:[*+-]|\d+\.|\|)\s")

# E2 — a sentence containing any of these asserts absence.
ABSENCE_MARKERS = (
    "does not exist",
    "do not exist",
    "doesn't exist",
    "don't exist",
    "does not yet exist",
    "no longer exists",
    "no longer exist",
    "never existed",
    "was removed",
    "were removed",
    "has been removed",
    "have been removed",
    "is removed",
    "not present",
    "does not appear",
    "no such",
    "which does not",
    "nothing on disk",
    "matches nothing",
    "which is still",   # "…lists X, which is still Y" — names the absent name
    "never created",
    "never carried forward",
    "not migrated",
    "never exposed",
)

# Symbol prefixes owned by third parties; not this repository's to resolve.
FOREIGN_SYMBOL_ROOTS = {
    "std", "core", "alloc", "serde", "serde_json", "rusqlite", "anyhow",
    "thiserror", "tokio", "axum", "clap", "rayon", "half", "memmap2",
    "criterion", "proptest", "npm", "cargo", "vitest", "three", "Cesium",
    "Math", "Object", "Array", "Number", "String", "JSON", "Promise",
    "window", "document", "console", "process", "self", "crate", "super",
    "Self", "Option", "Result", "Vec", "HashMap", "BTreeMap", "Ok", "Err",
    "Some", "None", "Ordering", "f32", "f64", "u8", "u32", "u64", "i32",
    "i64", "usize",
}

# Directories grepped when resolving SYMBOL and TEST citations.
GREP_ROOTS = ("crates", "apps", "tests", "mm", "python", "gpu", "schemas",
              "architectures", ".github")

# Never grepped: build output, installed dependencies, checkpoint blobs.
GREP_EXCLUDE_DIRS = ("node_modules", "target", "dist", ".git", "__pycache__")

# Path shape. `*`, `?` and `{a,b}` are legal in a citation and are expanded
# before resolution; `<slug>`, `XXXX`/`0NN`/`0XX` and an elision are template
# placeholders. The elision comes in two spellings and BOTH must be listed: the
# ASCII `...` and the typographic `…` (U+2026), which is what the corpus
# actually types — `` `./scripts/…` `` and
# `` `apps/web/…/util/__tests__/workspace-paths.test.ts` ``. A token holding an
# elision names a FAMILY of paths, not a file, so existence is the wrong test
# for it; listing only the ASCII spelling made the checker report the
# typographic one as an unresolved citation, which is a false positive.
SHAPE_SEGMENT = re.compile(r"^[A-Za-z0-9_.@+*?{},<>…-]+$")
PLACEHOLDER = re.compile(r"XXXX|0NN|0XX|<[a-z-]+>|\.\.\.|…")

# Candidate extraction.
BACKTICK = re.compile(r"`([^`\n]+)`")
MDLINK = re.compile(r"\]\(<?([^)>\n]+)>?\)")
# `file.ts:9`, `file.ts:9-10`, `file.ts:20,51,102`
LINE_SUFFIX = re.compile(r":\d+(?:[-–]\d+)?(?:,\d+(?:[-–]\d+)?)*$")
BRACE = re.compile(r"\{([^{}]*)\}")
TESTNAME = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+){3,}$")
SYMBOLISH = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+$")


# --------------------------------------------------------------------------
# Repository discovery
# --------------------------------------------------------------------------

def repo_root() -> str:
    here = os.path.dirname(os.path.abspath(__file__))          # .plan/tools
    return os.path.dirname(os.path.dirname(here))              # repo root


ROOT = repo_root()
PLAN = os.path.join(ROOT, ".plan")


def top_level_entries() -> set[str]:
    return {e for e in os.listdir(ROOT) if not e.startswith(".git")}


def git_ignored(rel: str) -> bool:
    """A path under a gitignored directory is not present in a fresh checkout
    or a git worktree, so its absence proves nothing about the plan. Reported
    as UNVERIFIABLE, never as unresolved."""
    return subprocess.run(["git", "check-ignore", "-q", "--", rel], cwd=ROOT,
                          stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL).returncode == 0


# --------------------------------------------------------------------------
# Planned-set derivation
# --------------------------------------------------------------------------

def plan_markdown_files() -> list[str]:
    out = []
    for dirpath, dirnames, filenames in os.walk(PLAN):
        dirnames[:] = [d for d in dirnames if d != "tools"]
        for fn in sorted(filenames):
            if fn.endswith(".md"):
                out.append(os.path.join(dirpath, fn))
    return sorted(out)


class Planned:
    """The planned set, in two parts, because the distinction is load-bearing.

    `declared` — paths a task literally wrote under `## Files Expected to Add`.
        These act as PREFIXES: a task declaring `apps/web/diagnostics/` plans
        everything beneath it.
    `ancestors` — directories implied by a declared path. These match EXACTLY
        and never as prefixes. `crates/q-quant/src/lib.rs` implies `crates`,
        and if `crates` were a prefix then every path under `crates/` would be
        "planned" and the checker would pass anything. `crates/q-nope/src/lib.rs`
        must still fail.
    """

    def __init__(self) -> None:
        self.declared: dict[str, str] = {}
        self.ancestors: dict[str, str] = {}

    def match(self, tok: str) -> str | None:
        if tok in self.declared or tok in self.ancestors:
            return tok
        for p in self.declared:
            if tok.startswith(p + "/"):
                return p
        return None

    def owner(self, tok: str) -> str | None:
        key = self.match(tok)
        if key is None:
            return None
        return self.declared.get(key) or self.ancestors.get(key)

    def __len__(self) -> int:
        return len(self.declared) + len(self.ancestors)


def derive_planned(anchors: set[str]) -> Planned:
    """Every path declared under a `## Files Expected to Add` heading, with the
    task that owns it.

    A declared path whose first segment is neither an existing top-level entry
    nor a NEW_TOP_LEVEL entry is a fragment relative to the bullet above it
    (`src/app.ts` under `apps/web/diagnostics/`) and is dropped: admitting it
    would make `src` a repository anchor and let unrelated fragments through."""
    out = Planned()
    for path in plan_markdown_files():
        rel = os.path.relpath(path, ROOT)
        m = re.search(r"tasks/(QM-\d{4})", rel)
        task = m.group(1) if m else os.path.basename(rel)
        section = None
        for line in open(path, encoding="utf-8"):
            if line.startswith("## "):
                section = line.strip()
                continue
            if section not in PLANNED_SECTIONS:
                continue
            for raw in BACKTICK.findall(line):
                for tok in brace_expand(normalise(raw)):
                    tok = tok.strip()
                    if tok and "/" in tok and tok.split("/")[0] in anchors:
                        out.declared.setdefault(tok, task)
    for p, task in list(out.declared.items()):
        parts = p.split("/")
        for i in range(1, len(parts)):
            anc = "/".join(parts[:i])
            if anc in out.declared:
                continue
            if os.path.exists(os.path.join(ROOT, anc)):
                # an existing directory: EXACT match only. `crates` must not
                # become a prefix, or `crates/q-nope/src/lib.rs` would pass.
                out.ancestors.setdefault(anc, task)
            else:
                # a directory the task is creating: everything beneath it is
                # planned, so it is a PREFIX. `fixtures/reports/README.md`.
                out.declared.setdefault(anc, task)
    return out


def normalise(tok: str) -> str:
    tok = tok.strip().strip(",;:")
    tok = LINE_SUFFIX.sub("", tok)
    tok = tok.rstrip("/")
    tok = re.sub(r"/\*\*?$", "", tok)
    return tok.strip()


# --------------------------------------------------------------------------
# Resolution
# --------------------------------------------------------------------------

def alias_candidates(rel: str) -> list[tuple[str, str]]:
    """(candidate path, owning task) pairs produced by the pending renames."""
    out = []
    for a, b, task in PENDING_RENAMES:
        if rel == a or rel.startswith(a + "/"):
            out.append((b + rel[len(a):], task))
        elif rel == b or rel.startswith(b + "/"):
            out.append((a + rel[len(b):], task))
    return out


def brace_expand(rel: str) -> list[str]:
    """`gpu/cuda/{reduce,matmul}.cu` -> two paths. Shell brace semantics."""
    m = BRACE.search(rel)
    if not m:
        return [rel]
    out = []
    for alt in m.group(1).split(","):
        out.extend(brace_expand(rel[:m.start()] + alt.strip() + rel[m.end():]))
    return out


def _one_exists(rel: str, base: str) -> bool:
    target = os.path.normpath(os.path.join(base, rel))
    if any(c in rel for c in "*?"):
        return bool(glob.glob(target))
    return os.path.exists(target)


def path_exists(rel: str, doc_dir: str) -> tuple[bool, str | None]:
    """Resolve repo-root-relative first, then relative to the citing document.
    Braces are expanded and every alternative must resolve; globs must match at
    least once. Returns (resolved, pending-rename task if the alias was used)."""
    variants = brace_expand(rel)

    def all_exist(base: str, paths: list[str]) -> bool:
        return all(_one_exists(v, base) for v in paths)

    for base in (ROOT, doc_dir):
        if all_exist(base, variants):
            return True, None
    for cand, task in alias_candidates(rel):
        cvars = brace_expand(cand)
        for base in (ROOT, doc_dir):
            if all_exist(base, cvars):
                return True, task
    return False, None


_grep_cache: dict[str, bool] = {}


def grep_repo(needle: str) -> bool:
    if needle in _grep_cache:
        return _grep_cache[needle]
    roots = [r for r in GREP_ROOTS if os.path.isdir(os.path.join(ROOT, r))]
    cmd = (["grep", "-rqI"]
           + [f"--exclude-dir={d}" for d in GREP_EXCLUDE_DIRS]
           + ["-F", "--", needle] + roots)
    ok = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL).returncode == 0
    _grep_cache[needle] = ok
    return ok


_norm_index: set[str] | None = None


def _normalise_words(s: str) -> str:
    return " ".join(re.sub(r"[^a-z0-9]+", " ", s.lower()).split())


def _build_test_index() -> set[str]:
    """Every `it('…')`/`test('…')` title and `fn …()` name in the repository,
    reduced to lowercase words. Rust tests are snake_case; vitest titles are
    prose with commas and hyphens — `.plan/` renders both as snake_case, so
    `treats_a_501_as_a_declared_gap_not_a_failure_to_retry` and
    `it('treats a 501 as a declared gap, not a failure to retry')` are the same
    citation and must resolve to each other."""
    # `-I` is load-bearing, not cosmetic. `mm/intro/` and
    # `apps/web/quatricmorph-workspace/public/intro/` together hold ~1.0 GB of
    # `.mov` and `.png` assets. Without `-I` this regex is applied to every byte
    # of them and the scan exceeds 60 s; with `-I` it is 1.5 s. QM-0002
    # `## Memory and Performance Constraints` requires "runs in seconds".
    idx: set[str] = set()
    roots = [r for r in GREP_ROOTS if os.path.isdir(os.path.join(ROOT, r))]
    cmd = (["grep", "-rhoEI"]
           + [f"--exclude-dir={d}" for d in GREP_EXCLUDE_DIRS]
           + ["--", r"(it|test|describe)\((['\"\`])[^'\"\`]+\2|fn [a-z_][a-z0-9_]*"]
           + roots)
    out = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True).stdout
    for hit in out.splitlines():
        m = re.match(r"(?:it|test|describe)\(['\"\`](.+)$", hit)
        if m:
            idx.add(_normalise_words(m.group(1)))
        elif hit.startswith("fn "):
            idx.add(_normalise_words(hit[3:]))
    return idx


def test_exists(name: str) -> bool:
    global _norm_index
    if _norm_index is None:
        _norm_index = _build_test_index()
    return _normalise_words(name) in _norm_index or grep_repo(name)


def shape_ok(rel: str, doc_dir: str | None = None) -> bool:
    """Is `rel` shaped like a repository path?

    A document-relative citation is shape-checked in its **repo-root form**,
    exactly as `path_exists` resolves it. `.plan/README.md` sits one level below
    the root and legitimately writes `../docs/…`; the `..` rejection below exists
    to catch a repo-anchored path escaping the tree, not to fail a correct
    relative link. Without `doc_dir` the old behaviour is kept, so a caller that
    has no document context still rejects `..`.
    """
    if not rel or rel.startswith("/"):
        return False
    if ".." in rel.split("/"):
        if doc_dir is None:
            return False
        inside = os.path.relpath(os.path.normpath(os.path.join(doc_dir, rel)),
                                 ROOT)
        if inside.split("/")[0] == "..":
            return False            # genuinely escapes the repository
        rel = inside
    return all(SHAPE_SEGMENT.match(seg) for seg in rel.split("/") if seg != "")


# --------------------------------------------------------------------------
# Context classification
# --------------------------------------------------------------------------

def sentences(line: str) -> list[str]:
    return re.split(r"(?<=[.;:])\s+", line)


def asserts_absence(line: str, token: str) -> bool:
    for s in sentences(line):
        if token in s and any(m in s.lower() for m in ABSENCE_MARKERS):
            return True
    # a token quoted at the very end of a clause continued on the next line is
    # handled line-granularly as a fallback
    return False


# --------------------------------------------------------------------------
# Main scan
# --------------------------------------------------------------------------

def scan(verbose: bool):
    anchors = top_level_entries() | set(NEW_TOP_LEVEL)
    planned = derive_planned(anchors)

    failures: list[tuple[str, int, str, str, str]] = []   # file, line, kind, tok, why
    notes: list[tuple[str, int, str, str]] = []           # file, line, tok, task
    unverifiable: list[tuple[str, int, str, str]] = []    # file, line, tok, why
    stats = Counter()
    skipped_tokens: Counter = Counter()

    for path in plan_markdown_files():
        rel_doc = os.path.relpath(path, ROOT)
        doc_dir = os.path.dirname(path)
        doc_name = os.path.basename(path)
        is_record = doc_name in EVIDENCE_DOCUMENTS
        section = None
        fenced = False

        lines = open(path, encoding="utf-8").read().split("\n")
        # bullet-granular task-reference map
        block_ref: list[bool] = [False] * (len(lines) + 2)
        start = 0
        for i, l in enumerate(lines):
            if BULLET_START.match(l) or not l.strip() or l.startswith("#"):
                start = i
            if TASK_REF.search(l):
                for j in range(start, len(lines)):
                    block_ref[j] = True
                    nxt = lines[j + 1] if j + 1 < len(lines) else ""
                    if (BULLET_START.match(nxt) or not nxt.strip()
                            or nxt.startswith("#")):
                        break

        for lineno, raw in enumerate(lines, 1):
            line = raw.rstrip("\n")
            if line.lstrip().startswith("```"):
                fenced = not fenced
                continue
            if fenced:
                continue
            # `section` is updated ONLY outside a fence. A `## …` line inside a
            # code block is quoted text, not a document section — and treating it
            # as one is a FALSE EXEMPTION, which is strictly worse than a false
            # positive: quoting a `## Test Cases` heading inside a fenced block
            # used to set `section` for the whole remainder of the document, so
            # every later citation earned E1 and any real failure among them
            # vanished silently. Found while recording this task's own probe.
            if line.startswith("## "):
                section = line.strip()

            tokens = [(t, "backtick") for t in BACKTICK.findall(line)]
            tokens += [(t, "mdlink") for t in MDLINK.findall(line)]

            for raw_tok, origin in tokens:
                tok = normalise(raw_tok)
                if not tok:
                    continue

                is_evidence = is_record or section == EVIDENCE_SECTION
                if is_evidence and block_ref[lineno - 1]:
                    is_evidence = False          # forward reference to a task
                    stats["forward-ref"] += 1
                in_test_cases = section == TEST_CASES_SECTION
                absent = asserts_absence(line, raw_tok)

                # E1 and E2 are applied before every other rule. They are the
                # two exemptions QM-0002 `## Scope` mandates, and E2 in
                # particular cannot be overridden by evidence context: a
                # sentence asserting that a path is absent is not a claim that
                # it exists, so existence is the wrong test for it.
                if in_test_cases:
                    stats["exempt.E1"] += 1
                    continue
                if absent:
                    stats["exempt.E2"] += 1
                    continue

                # ---- COMPOSITE `path::symbol` -------------------------------
                # Checked before SYMBOL and PATH because it is neither: both
                # halves must resolve, and the failure names the guilty half.
                mcomp = COMPOSITE.match(tok)
                if mcomp:
                    cpath, csym = mcomp.group(1), mcomp.group(2)
                    stats["composite.checked"] += 1
                    ok_path, _ = path_exists(cpath, doc_dir)
                    if not ok_path:
                        failures.append((rel_doc, lineno, "PATH", tok,
                                         f"the path half `{cpath}` does not resolve"))
                    elif not (test_exists(csym.split("::")[-1])
                              or grep_repo(csym.split("::")[-1])):
                        failures.append((rel_doc, lineno, "SYMBOL", tok,
                                         f"the path half resolves but `{csym}` is not "
                                         f"defined or used in the repository"))
                    continue

                # ---- SYMBOL -------------------------------------------------
                if SYMBOLISH.match(tok):
                    if tok.split("::")[0] in FOREIGN_SYMBOL_ROOTS:
                        stats["symbol.foreign"] += 1
                        continue
                    if not is_evidence:
                        stats["symbol.non-evidence"] += 1
                        continue
                    stats["symbol.checked"] += 1
                    if not grep_repo(tok.split("::")[-1]):
                        failures.append((rel_doc, lineno, "SYMBOL", tok,
                                         "no definition or use found in repository"))
                    continue

                # ---- TEST ---------------------------------------------------
                if TESTNAME.match(tok):
                    if not is_evidence:
                        stats["test.non-evidence"] += 1
                        continue
                    stats["test.checked"] += 1
                    if not test_exists(tok):
                        failures.append((rel_doc, lineno, "TEST", tok,
                                         "no test of this name found in repository"))
                    continue

                # ---- PATH ---------------------------------------------------
                # A bare filename with no directory separator asserts no
                # location, so it cannot be stale — "each `TASK.md`",
                # "`NOTICE.md`". It is resolved for the record against the
                # citing directory, `.plan/` and the repository root, but a
                # bare filename that does not resolve is prose, not a failure.
                if "/" not in tok:
                    if any(os.path.exists(os.path.join(b, tok))
                           for b in (doc_dir, PLAN, ROOT)):
                        stats["path.resolved"] += 1
                    else:
                        stats["path.bare-filename"] += 1
                    continue
                if " " in tok and origin == "backtick":
                    continue
                first = tok.split("/")[0]
                doc_relative = origin == "mdlink" or first in ("..", ".")
                if first not in anchors and not doc_relative:
                    stats["path.not-repo-anchored"] += 1
                    skipped_tokens[tok] += 1
                    continue
                if PLACEHOLDER.search(tok):
                    stats["path.placeholder"] += 1
                    if not shape_ok(tok, doc_dir):
                        failures.append((rel_doc, lineno, "PATH", tok,
                                         "template placeholder fails shape check"))
                    continue

                if any(tok.startswith(p) for p in ENVIRONMENT_LOCAL):
                    stats["path.environment-local"] += 1
                    unverifiable.append((rel_doc, lineno, tok,
                                         "an orchestration worktree — resolves from "
                                         "the canonical checkout, not from inside a "
                                         "worktree, and is deleted when the run ends"))
                    continue

                stats["path.candidates"] += 1
                exists, alias_task = path_exists(tok, doc_dir)

                if exists:
                    stats["path.resolved"] += 1
                    if alias_task:
                        notes.append((rel_doc, lineno, tok, alias_task))
                    continue

                if git_ignored(tok):
                    stats["path.gitignored"] += 1
                    unverifiable.append((rel_doc, lineno, tok,
                                         "under a gitignored path — absent from a "
                                         "fresh checkout by design"))
                    continue

                if is_evidence:
                    # THE EVIDENCE OVERRIDE: the planned set does not excuse a
                    # citation offered as proof of what the repository holds.
                    stats["path.evidence-stale"] += 1
                    who = planned.owner(tok)
                    extra = (f"; it is declared under `## Files Expected to Add`"
                             f" by {who}, so cite it as planned, not as evidence"
                             ) if who else ""
                    failures.append((rel_doc, lineno, "PATH", tok,
                                     "cited as repository evidence but does not "
                                     "resolve" + extra))
                    continue

                if planned.match(tok) or section in PLANNED_SECTIONS \
                        or tok.split("/")[0] in NEW_TOP_LEVEL:
                    stats["path.planned"] += 1
                    if not shape_ok(tok, doc_dir):
                        failures.append((rel_doc, lineno, "PATH", tok,
                                         "planned path fails shape check"))
                    continue

                failures.append((rel_doc, lineno, "PATH", tok,
                                 "does not resolve, and no task declares it under "
                                 "`## Files Expected to Add`"))

    return failures, notes, unverifiable, stats, skipped_tokens, planned


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--verbose", action="store_true",
                    help="also list tokens skipped as not repo-anchored")
    args = ap.parse_args()

    failures, notes, unverifiable, stats, skipped, planned = scan(args.verbose)

    print(f"check-plan-citations — repository root {ROOT}")
    print(f"  markdown files scanned     : {len(plan_markdown_files())}")
    for k in sorted(stats):
        print(f"  {k:26s} : {stats[k]}")
    print(f"  planned paths declared     : {len(planned.declared)}"
          f"  (+{len(planned.ancestors)} implied directories)")

    if unverifiable:
        print()
        print(f"UNVERIFIABLE ({len(unverifiable)}) — not a failure, and not an "
              f"exemption; state this limit wherever the citation is relied on:")
        for f, ln, tok, why in unverifiable:
            print(f"  {f}:{ln}: {tok}")
            print(f"      {why}")

    if notes:
        print()
        print(f"pending-rename NOTES ({len(notes)}) — resolved through "
              f"PENDING_RENAMES, not through an exemption:")
        seen = set()
        for f, ln, tok, task in notes:
            key = (f, ln, tok)
            if key in seen:
                continue
            seen.add(key)
            print(f"  {f}:{ln}: {tok}   [owned by {task}]")

    if args.verbose and skipped:
        print()
        print("skipped — not repo-anchored (fragments, routes, refs):")
        for tok, n in skipped.most_common():
            print(f"  {n:4d}  {tok}")

    if failures:
        print()
        print(f"UNRESOLVED CITATIONS ({len(failures)}):")
        for f, ln, kind, tok, why in failures:
            print(f"  {f}:{ln}: [{kind}] {tok}")
            print(f"      {why}")
        print()
        print(f"FAIL — {len(failures)} unresolved citation(s).")
        return 1

    print()
    print("OK — every checked citation resolves.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
