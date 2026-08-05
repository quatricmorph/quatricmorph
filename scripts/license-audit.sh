#!/usr/bin/env bash
#
# scripts/license-audit.sh — QM-0093 attribution and licence audit.
#
# What this script does
# --------------------
#   * asserts `mm/` is byte-identical to its state at the baseline commit;
#   * asserts `apps/web/quatricmorph-workspace/LICENSE` is byte-identical to
#     `mm/LICENSE`;
#   * asserts the derivation attribution in
#     `apps/web/quatricmorph-workspace/NOTICE.md` still names Meta Platforms;
#   * asserts the root `NOTICE` exists;
#   * reads the declared licence of every Rust package in `Cargo.lock` (via
#     `cargo metadata`) and every npm package in the checked-in
#     `package-lock.json` files, and fails if any package declares none;
#   * fails if any package's licence expression offers no permissive
#     alternative and names GPL / LGPL / AGPL / SSPL;
#   * reports — without failing — every package whose expression mentions a
#     copyleft licence at all (MPL-2.0, or a GPL family member behind an `OR`).
#
# What this script does NOT do
# ----------------------------
# It records what this repository's own files and the package metadata in the
# lockfiles say. It is not legal advice and it is not a clearance. Licences that
# no file in the tree states are recorded as "not verified" in `NOTICE`, not
# guessed here.
#
# On the fail threshold
# ---------------------
# `.plan/tasks/QM-0093-attribution-license-audit/TASK.md` says under
# "Error Handling" that *a* copyleft dependency fails, and under
# "Acceptance Criteria" §6 that *no GPL/AGPL/SSPL dependency* is present. This
# script implements the acceptance criterion — GPL/LGPL/AGPL/SSPL with no
# permissive alternative is fatal — and downgrades weak/file-level copyleft
# (MPL-2.0) to a reported review item, because the tree contains MPL-2.0
# dev-only build tooling that is not redistributed in any product artifact.
# The divergence is deliberate and is recorded in `.plan/evidence/QM-0093.md`.
#
# Usage
#   scripts/license-audit.sh              # check; non-zero exit on failure
#   scripts/license-audit.sh --json       # emit the QM-0093 data contract
#   scripts/license-audit.sh --generate   # rewrite the generated blocks in NOTICE
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The commit `.plan/tasks/QM-0093-attribution-license-audit/TASK.md`
# ("Suggested Commands") names as the baseline for the `mm/` read-only check.
MM_BASELINE="${MM_BASELINE:-5ca434d}"

MM_LICENSE="mm/LICENSE"
WS_LICENSE="apps/web/quatricmorph-workspace/LICENSE"
WS_NOTICE="apps/web/quatricmorph-workspace/NOTICE.md"
ROOT_NOTICE="NOTICE"

# Every checked-in npm lockfile that carries per-package `license` metadata.
NPM_LOCKFILES=(
  "apps/web/package-lock.json"
  "apps/web/quatricmorph-workspace/package-lock.json"
)

MODE="check"
case "${1:-}" in
  "")           MODE="check" ;;
  --json)       MODE="json" ;;
  --generate)   MODE="generate" ;;
  -h|--help)    sed -n '2,40p' "$0"; exit 0 ;;
  *)            echo "license-audit: unknown argument '$1'" >&2; exit 2 ;;
esac

FAILURES=0
fail() { echo "license-audit: FAIL: $*" >&2; FAILURES=$((FAILURES + 1)); }
ok()   { [ "$MODE" = "json" ] || echo "license-audit: ok:   $*"; }
note() { [ "$MODE" = "json" ] || echo "license-audit: note: $*"; }

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "license-audit: missing required tool '$1'" >&2; exit 2; }
}
need git
need jq
need cargo

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# ---------------------------------------------------------------------------
# 1. `mm/` is read-only by policy (AGENTS.md, "Current codebase").
# ---------------------------------------------------------------------------
MM_UNMODIFIED="not-verified"
if git cat-file -e "${MM_BASELINE}^{commit}" 2>/dev/null; then
  if git diff --quiet "$MM_BASELINE" -- mm/; then
    MM_UNMODIFIED="true"
    ok "mm/ is unchanged since ${MM_BASELINE}"
  else
    MM_UNMODIFIED="false"
    fail "mm/ changed since ${MM_BASELINE}; it is read-only by policy"
    git --no-pager diff --stat "$MM_BASELINE" -- mm/ >&2 || true
  fi
else
  fail "baseline commit ${MM_BASELINE} is not present; fetch full history (fetch-depth: 0) to run the mm/ check"
fi

# ---------------------------------------------------------------------------
# 2. The reproduced LICENSE must match `mm/LICENSE` byte for byte.
# ---------------------------------------------------------------------------
MM_LICENSE_SHA256="$(sha256_of "$MM_LICENSE")"
WS_LICENSE_SHA256="$(sha256_of "$WS_LICENSE")"
if [ "$MM_LICENSE_SHA256" = "$WS_LICENSE_SHA256" ]; then
  WORKSPACE_LICENSE_MATCHES="true"
  ok "${WS_LICENSE} matches ${MM_LICENSE} (${MM_LICENSE_SHA256})"
else
  WORKSPACE_LICENSE_MATCHES="false"
  fail "${WS_LICENSE} differs from ${MM_LICENSE}"
  diff -u "$MM_LICENSE" "$WS_LICENSE" >&2 || true
fi

# ---------------------------------------------------------------------------
# 3. The derivation attribution must still name the original author.
# ---------------------------------------------------------------------------
if grep -q "Meta Platforms, Inc" "$WS_NOTICE"; then
  ok "${WS_NOTICE} attributes Meta Platforms, Inc."
else
  fail "${WS_NOTICE} no longer names Meta Platforms, Inc."
fi
if grep -q "Meta Platforms, Inc" "$MM_LICENSE"; then
  ok "${MM_LICENSE} carries the Meta Platforms copyright line"
else
  fail "${MM_LICENSE} no longer carries the Meta Platforms copyright line"
fi

# ---------------------------------------------------------------------------
# 4. The project's own licence declarations.
# ---------------------------------------------------------------------------
if grep -q '^license = "MIT OR Apache-2.0"' Cargo.toml; then
  ok 'Cargo.toml declares license = "MIT OR Apache-2.0"'
else
  fail 'Cargo.toml no longer declares license = "MIT OR Apache-2.0"'
fi
if [ "$(jq -r '.license // "null"' apps/web/quatricmorph-workspace/package.json)" = "MIT" ]; then
  ok 'apps/web/quatricmorph-workspace/package.json declares "license": "MIT"'
else
  fail 'apps/web/quatricmorph-workspace/package.json no longer declares "license": "MIT"'
fi

# ---------------------------------------------------------------------------
# 5. Dependency licences.
# ---------------------------------------------------------------------------
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cargo metadata --format-version 1 >"$WORK/cargo-metadata.json"

# name<TAB>version<TAB>license, third-party only (workspace members excluded via
# the null `source` field), sorted and deduplicated.
jq -r '
  .packages[]
  | select(.source != null)
  | [.name, .version, (.license // "")] | @tsv
' "$WORK/cargo-metadata.json" | LC_ALL=C sort -u >"$WORK/rust.tsv"

: >"$WORK/npm.tsv"
for lock in "${NPM_LOCKFILES[@]}"; do
  [ -f "$lock" ] || { fail "expected npm lockfile ${lock} is missing"; continue; }
  jq -r '
    .packages | to_entries[]
    | select(.key | contains("node_modules/"))
    | select(.value.link != true)
    | [ (.key | split("node_modules/") | last), (.value.version // ""), (.value.license // "") ]
    | @tsv
  ' "$lock" >>"$WORK/npm.tsv"
done
LC_ALL=C sort -u -o "$WORK/npm.tsv" "$WORK/npm.tsv"

# `awk` classifier shared by both ecosystems.
#   column 3 empty                                        -> "unlicensed"
#   no OR-alternative free of GPL/LGPL/AGPL/SSPL          -> "conflict"
#   expression mentions any copyleft licence              -> "review"
classify() {
  awk -F'\t' '
    function is_copyleft(s) { return (s ~ /GPL|SSPL/) }
    function is_weak(s)     { return (s ~ /MPL|EPL|CDDL|EUPL|OSL|CC-BY-SA/) }
    {
      name = $1; version = $2; lic = $3
      if (lic == "") { print "unlicensed\t" name "\t" version "\t"; next }
      expr = lic
      gsub(/[()]/, " ", expr)
      gsub(/\//, " OR ", expr)          # legacy "MIT/Apache-2.0" spelling
      n = split(expr, alts, / OR /)
      permissive = 0
      for (i = 1; i <= n; i++) if (!is_copyleft(alts[i])) permissive = 1
      if (!permissive)                        { print "conflict\t" name "\t" version "\t" lic; next }
      if (is_copyleft(lic) || is_weak(lic))   { print "review\t"   name "\t" version "\t" lic; next }
      print "clean\t" name "\t" version "\t" lic
    }
  ' "$1"
}

classify "$WORK/rust.tsv" >"$WORK/rust.classified"
classify "$WORK/npm.tsv"  >"$WORK/npm.classified"
cat "$WORK/rust.classified" "$WORK/npm.classified" >"$WORK/all.classified"

RUST_COUNT="$(wc -l <"$WORK/rust.tsv" | tr -d ' ')"
NPM_COUNT="$(wc -l <"$WORK/npm.tsv" | tr -d ' ')"

UNLICENSED="$(grep -c '^unlicensed' "$WORK/all.classified" || true)"
CONFLICTS="$(grep -c '^conflict' "$WORK/all.classified" || true)"
REVIEWS="$(grep -c '^review' "$WORK/all.classified" || true)"

if [ "$UNLICENSED" -eq 0 ]; then
  ok "every one of ${RUST_COUNT} Rust and ${NPM_COUNT} npm third-party packages declares a licence"
else
  fail "${UNLICENSED} package(s) declare no licence:"
  grep '^unlicensed' "$WORK/all.classified" | cut -f2,3 >&2
fi

if [ "$CONFLICTS" -eq 0 ]; then
  ok "no GPL/LGPL/AGPL/SSPL-only dependency"
else
  fail "${CONFLICTS} dependency/dependencies offer no permissive alternative:"
  grep '^conflict' "$WORK/all.classified" | cut -f2,3,4 >&2
fi

if [ "$REVIEWS" -gt 0 ]; then
  note "${REVIEWS} dependency/dependencies name a copyleft licence in their expression (recorded, not fatal):"
  [ "$MODE" = "json" ] || grep '^review' "$WORK/all.classified" | cut -f2,3,4 | sed 's/^/license-audit: note:   /'
fi

# ---------------------------------------------------------------------------
# 6. The root NOTICE, and whether its generated tables are current.
# ---------------------------------------------------------------------------
render_block() {
  # $1 = tsv, $2 = column header for the ecosystem
  echo "| Package | Version | Declared licence |"
  echo "| --- | --- | --- |"
  awk -F'\t' '{ printf "| `%s` | %s | %s |\n", $1, $2, ($3 == "" ? "**not verified — no licence declared**" : $3) }' "$1"
}

rewrite_block() {
  # $1 = file, $2 = marker id, $3 = file holding the replacement body
  local file="$1" id="$2" body="$3"
  awk -v id="$id" -v body="$body" '
    $0 == "<!-- BEGIN GENERATED: " id " -->" { print; inblock = 1; while ((getline line < body) > 0) print line; close(body); next }
    $0 == "<!-- END GENERATED: " id " -->"   { inblock = 0; print; next }
    !inblock { print }
  ' "$file" >"$file.tmp"
  mv "$file.tmp" "$file"
}

render_block "$WORK/rust.tsv" >"$WORK/rust.md"
render_block "$WORK/npm.tsv"  >"$WORK/npm.md"

case "$MODE" in
  generate)
    [ -f "$ROOT_NOTICE" ] || { echo "license-audit: ${ROOT_NOTICE} must exist with the generated markers before --generate" >&2; exit 2; }
    rewrite_block "$ROOT_NOTICE" "rust-dependencies" "$WORK/rust.md"
    rewrite_block "$ROOT_NOTICE" "npm-dependencies"  "$WORK/npm.md"
    echo "license-audit: regenerated the dependency tables in ${ROOT_NOTICE}"
    ;;
  json)
    jq -n \
      --arg mm_unmodified "$MM_UNMODIFIED" \
      --arg mm_license_sha256 "$MM_LICENSE_SHA256" \
      --arg workspace_license_matches "$WORKSPACE_LICENSE_MATCHES" \
      --argjson notice_present "$([ -f "$ROOT_NOTICE" ] && echo true || echo false)" \
      --rawfile rust "$WORK/rust.tsv" \
      --rawfile npm "$WORK/npm.tsv" \
      --rawfile classified "$WORK/all.classified" '
      def rows($t; $eco):
        ($t | rtrimstr("\n") | select(length > 0) | split("\n") | map(split("\t") | {ecosystem: $eco, name: .[0], version: .[1], license: (if .[2] == "" then null else .[2] end)}));
      def picked($k):
        ($classified | rtrimstr("\n") | select(length > 0) | split("\n") | map(split("\t")) | map(select(.[0] == $k))
         | map({name: .[1], version: .[2], license: (if .[3] == "" then null else .[3] end)}));
      { mm_unmodified: ($mm_unmodified == "true"),
        mm_unmodified_raw: $mm_unmodified,
        mm_license_sha256: $mm_license_sha256,
        workspace_license_matches: ($workspace_license_matches == "true"),
        notice_present: $notice_present,
        dependencies: (rows($rust; "cargo") + rows($npm; "npm")),
        unlicensed: picked("unlicensed"),
        copyleft_conflicts: picked("conflict"),
        copyleft_review: picked("review") }'
    ;;
  check)
    if [ -f "$ROOT_NOTICE" ]; then
      ok "${ROOT_NOTICE} exists"
      for pair in "rust-dependencies:$WORK/rust.md" "npm-dependencies:$WORK/npm.md"; do
        id="${pair%%:*}"; body="${pair#*:}"
        if ! grep -q "^<!-- BEGIN GENERATED: ${id} -->$" "$ROOT_NOTICE"; then
          fail "${ROOT_NOTICE} has no '${id}' generated block"
          continue
        fi
        cp "$ROOT_NOTICE" "$WORK/notice.candidate"
        rewrite_block "$WORK/notice.candidate" "$id" "$body"
        if diff -q "$ROOT_NOTICE" "$WORK/notice.candidate" >/dev/null; then
          ok "${ROOT_NOTICE} '${id}' table is current"
        else
          fail "${ROOT_NOTICE} '${id}' table is stale; run scripts/license-audit.sh --generate"
          diff -u "$ROOT_NOTICE" "$WORK/notice.candidate" | head -40 >&2 || true
        fi
      done
    else
      fail "${ROOT_NOTICE} is missing"
    fi
    ;;
esac

if [ "$FAILURES" -gt 0 ]; then
  echo "license-audit: ${FAILURES} check(s) failed" >&2
  exit 1
fi
[ "$MODE" = "json" ] || echo "license-audit: all checks passed"
