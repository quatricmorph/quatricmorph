#!/usr/bin/env bash
#
# QM-0001 — the repository's test floor guard.
#
# Runs the gates `.github/workflows/build.yaml` runs, then asserts that the
# measured test counts have not fallen below the floor recorded in
# `scripts/baseline.json`. Exits non-zero on any regression.
#
#   ./scripts/verify-baseline.sh
#
# The floor may only ever be RAISED. A task that lowers it is rejected in
# review (`.plan/tasks/QM-0001-baseline-verification/TASK.md`, Data Contracts).
#
# WHAT THIS GUARD DOES NOT DO — stated here rather than discovered later:
#
#   * A floor set BELOW the real count does not FAIL the count check. The
#     comparison fires only when the floor exceeds reality; it cannot tell that
#     a floor of 1 is protecting nothing. This is mitigated but not closed:
#       - the run REPORTS a stale floor with both numbers and the size of the
#         gap (see `qm_floor_status` and the STALE FLOOR block), so the hole is
#         legible rather than invisible — but the run still exits 0;
#       - `scripts/verify-baseline.test.sh` hard-codes the counts measured on
#         793e122 (rust 290 over 39 binaries, web 115 over 13 files) and fails
#         if `baseline.json` records less, so the floor cannot be lowered BELOW
#         that baseline. It does NOT stop a later, higher floor from being
#         lowered back down to it. That remains a review rule.
#   * It does not run the fixtures reproducibility gate
#     (`python3 fixtures/generate_fixtures.py && git diff --exit-code`).
#     That gate needs numpy and safetensors; this script must not depend on a
#     network, a GPU, or a Python virtualenv (TASK.md, Error Handling). CI's
#     `fixtures` job owns it. Fixture DRIFT is therefore outside this guard;
#     fixture ABSENCE is not, and is checked below.
#   * It does not install anything. If `apps/web/node_modules` is absent it
#     says so and fails, rather than reaching the network.
#
# Written for bash 3.2 (the macOS system bash): no associative arrays, no
# `mapfile`, no `${var,,}`.
#
# `set -e` is deliberately NOT used — this script captures non-zero exits from
# the commands it runs and reports every failure rather than stopping at the
# first.

set -uo pipefail

# ---------------------------------------------------------------------------
# Pure helpers. `verify-baseline.test.sh` sources this file and drives each of
# these directly, so they must stay free of global state and side effects.
# ---------------------------------------------------------------------------

# Repository root, resolved from this script's own location so that every
# relative path below works from any working directory.
qm_repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

# Collapse runs of whitespace and trim, so CLI goldens do not depend on the
# column alignment of a human-readable table.
qm_squeeze() {
  sed -e 's/[[:space:]][[:space:]]*/ /g' -e 's/^ //' -e 's/ *$//'
}

# Sum cargo's per-binary `test result:` lines.
# Echoes: "<passed> <failed> <ignored> <binaries>". Returns 1 if the log holds
# no `test result:` line at all — an empty log must not read as a clean zero.
qm_sum_rust_results() {
  local log="$1"
  [ -f "${log}" ] || { echo "qm_sum_rust_results: no such file: ${log}" >&2; return 1; }
  awk '
    /^ *test result:/ {
      n++
      for (i = 1; i <= NF; i++) {
        if ($i == "passed;")  p += $(i - 1)
        else if ($i == "failed;")  f += $(i - 1)
        else if ($i == "ignored;") g += $(i - 1)
      }
    }
    END {
      if (n == 0) {
        print "qm_sum_rust_results: no `test result:` line in the cargo log" > "/dev/stderr"
        exit 1
      }
      printf "%d %d %d %d\n", p, f, g, n
    }
  ' "${log}"
}

# Read vitest's two summary lines.
# Echoes: "<files_passed> <files_total> <tests_passed> <tests_total>".
# Returns 1 if either summary line is absent — a truncated or crashed run must
# not read as a clean zero.
qm_parse_vitest() {
  local log="$1"
  [ -f "${log}" ] || { echo "qm_parse_vitest: no such file: ${log}" >&2; return 1; }
  awk '
    function extract_passed(s,   i, n, a) {
      n = split(s, a, " ")
      for (i = 1; i <= n; i++) if (a[i] == "passed") return a[i - 1] + 0
      return 0
    }
    function extract_total(s) {
      if (match(s, /\([0-9]+\)/)) return substr(s, RSTART + 1, RLENGTH - 2) + 0
      return -1
    }
    /Test Files/ && !seen_files {
      seen_files = 1; fp = extract_passed($0); ft = extract_total($0)
    }
    /^[ \t]*Tests[ \t]/ && !seen_tests {
      seen_tests = 1; tp = extract_passed($0); tt = extract_total($0)
    }
    END {
      if (!seen_files || !seen_tests || ft < 0 || tt < 0) {
        print "qm_parse_vitest: no vitest summary line in the log" > "/dev/stderr"
        exit 1
      }
      printf "%d %d %d %d\n", fp, ft, tp, tt
    }
  ' "${log}"
}

# Print the names of the Rust tests that failed, verbatim from cargo's own
# `failures:` summary. Test Cases row 2 requires the failure to be NAMED, not
# merely counted.
qm_rust_failure_names() {
  local log="$1"
  sed -n '/^failures:$/,/^test result:/p' "${log}" \
    | grep -E '^    [A-Za-z_][A-Za-z0-9_:]*$' \
    | sed 's/^ *//' | sort -u
}

# Print vitest's own FAIL lines.
qm_vitest_failure_names() {
  local log="$1"
  grep -E '^[[:space:]]*(FAIL|×)' "${log}" | sed 's/^ *//' | sort -u
}

# Extract a numeric JSON value by key. Returns 1 if the key is absent or its
# value is not a bare integer.
qm_json_number() {
  local file="$1" key="$2" value
  value="$(grep -o "\"${key}\"[[:space:]]*:[[:space:]]*[0-9][0-9]*" "${file}" 2>/dev/null \
    | head -1 | sed 's/.*:[[:space:]]*//')"
  if [ -z "${value}" ]; then
    echo "qm_json_number: key \"${key}\" is absent or not an integer in ${file}" >&2
    return 1
  fi
  printf '%s' "${value}"
}

# Extract a string JSON value by key, at any nesting depth. Returns 1 if absent.
qm_json_string() {
  local file="$1" key="$2" value
  value="$(sed -n "s/.*\"${key}\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" \
    "${file}" 2>/dev/null | head -1)"
  if [ -z "${value}" ]; then
    echo "qm_json_string: key \"${key}\" is absent or not a string in ${file}" >&2
    return 1
  fi
  printf '%s' "${value}"
}

# Validate `baseline.json`: present, structurally intact, and carrying every
# floor this script needs. A missing key must fail loudly rather than default to
# zero — a floor of zero protects nothing.
qm_baseline_validate() {
  local file="$1" opens closes quotes key

  if [ ! -f "${file}" ]; then
    echo "error: baseline file not found: ${file}" >&2
    echo "       the test floor is recorded there; create it before running this guard" >&2
    return 1
  fi

  opens="$(tr -cd '{' <"${file}" | wc -c | tr -d ' ')"
  closes="$(tr -cd '}' <"${file}" | wc -c | tr -d ' ')"
  quotes="$(tr -cd '"' <"${file}" | wc -c | tr -d ' ')"

  if [ "${opens}" -eq 0 ] || [ "${opens}" != "${closes}" ]; then
    echo "error: ${file} is not parseable JSON: ${opens} '{' vs ${closes} '}'" >&2
    return 1
  fi
  if [ $((quotes % 2)) -ne 0 ]; then
    echo "error: ${file} is not parseable JSON: odd number of double quotes (${quotes})" >&2
    return 1
  fi
  if ! head -c 1 "${file}" | grep -q '{'; then
    echo "error: ${file} is not parseable JSON: it does not begin with '{'" >&2
    return 1
  fi

  for key in rust_tests rust_binaries web_tests web_files; do
    if ! qm_json_number "${file}" "${key}" >/dev/null 2>&1; then
      echo "error: ${file} is missing the required integer floor \"${key}\"" >&2
      return 1
    fi
  done

  for key in commit value_q10_100_42; do
    if ! qm_json_string "${file}" "${key}" >/dev/null 2>&1; then
      echo "error: ${file} is missing the required string \"${key}\"" >&2
      return 1
    fi
  done

  if ! grep -q '"cli_golden"' "${file}"; then
    echo "error: ${file} is missing the \"cli_golden\" object" >&2
    return 1
  fi

  return 0
}

# The comparison the whole guard exists for.
#
# Fires when the real count is BELOW the floor. It is SILENT when the floor is
# below the real count — see the blind-spot note at the top of this file.
qm_check_floor() {
  local label="$1" actual="$2" floor="$3"
  if [ "${actual}" -lt "${floor}" ]; then
    echo "baseline regression: ${actual} < ${floor} (${label})" >&2
    return 1
  fi
  return 0
}

# Report a measurement against its floor, in both directions.
#
# Exists because `qm_check_floor` alone is silent when the floor sits BELOW the
# real count — the guard's blind spot. This makes that case VISIBLE (though not
# fatal): it prints how stale the floor is, so a floor that has quietly stopped
# protecting anything is legible in the log rather than invisible.
#
# Echoes one status line. Returns:
#   0  measured == floor          (at floor)
#   1  measured <  floor          (baseline regression — fatal to the caller)
#   2  measured >  floor          (floor is stale — advisory, not fatal)
qm_floor_status() {
  local label="$1" actual="$2" floor="$3"
  if [ "${actual}" -lt "${floor}" ]; then
    echo "${label}: measured ${actual}, floor ${floor} — REGRESSION, ${actual} < ${floor}"
    qm_check_floor "${label}" "${actual}" "${floor}"
    return 1
  elif [ "${actual}" -gt "${floor}" ]; then
    echo "${label}: measured ${actual}, floor ${floor} — FLOOR IS STALE by $((actual - floor)); it sits below reality and protects nothing above ${floor}"
    return 2
  else
    echo "${label}: measured ${actual}, floor ${floor} — at floor"
    return 0
  fi
}

qm_require_fixture() {
  local path="$1"
  if [ ! -e "${path}" ]; then
    echo "error: missing fixture: ${path}" >&2
    echo "       regenerate it with: python3 fixtures/generate_fixtures.py" >&2
    return 1
  fi
  return 0
}

# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

main() {
  local root baseline log_dir started elapsed
  root="$(qm_repo_root)"
  baseline="${root}/scripts/baseline.json"
  log_dir="$(mktemp -d "${TMPDIR:-/tmp}/qm-verify-baseline.XXXXXX")"
  started="$(date +%s)"

  QM_FAILURES=0
  QM_STALE=0
  QM_STALE_REPORT=""
  QM_REPORT=""

  # Evaluate one measurement against its floor. A regression fails; a floor
  # below reality is recorded as stale and reported at the end, but does not
  # fail — raising the floor is the job of the task that added the tests.
  check_against_floor() { # label actual floor
    local line status
    line="$(qm_floor_status "$1" "$2" "$3")"
    status=$?
    case ${status} in
      0) record_pass "${line}" ;;
      2) record_pass "${line}"
         QM_STALE=$((QM_STALE + 1))
         QM_STALE_REPORT="${QM_STALE_REPORT}
  ${1}: floor ${3} -> measured ${2}" ;;
      *) record_failure "${line}" ;;
    esac
  }

  record_failure() {
    QM_FAILURES=$((QM_FAILURES + 1))
    QM_REPORT="${QM_REPORT}
  FAIL  $1"
    echo "FAIL  $1" >&2
  }
  record_pass() {
    QM_REPORT="${QM_REPORT}
  ok    $1"
    echo "ok    $1"
  }

  echo "verify-baseline: repository root ${root}"
  echo "verify-baseline: logs in ${log_dir}"
  echo

  # -- 0. Preflight: the guard's own unit tests ------------------------------
  echo "== preflight: scripts/verify-baseline.test.sh =="
  if bash "${root}/scripts/verify-baseline.test.sh" >"${log_dir}/selftest.log" 2>&1; then
    record_pass "guard self-tests"
  else
    cat "${log_dir}/selftest.log" >&2
    record_failure "guard self-tests (see above)"
  fi
  echo

  # -- 1. The recorded floor ------------------------------------------------
  echo "== baseline file =="
  if ! qm_baseline_validate "${baseline}"; then
    record_failure "scripts/baseline.json is absent or unusable"
    echo
    echo "verify-baseline: cannot continue without a valid floor." >&2
    rm -rf "${log_dir}"
    return 1
  fi
  local floor_rust floor_rust_bin floor_web floor_web_files golden_scalar
  floor_rust="$(qm_json_number "${baseline}" rust_tests)"
  floor_rust_bin="$(qm_json_number "${baseline}" rust_binaries)"
  floor_web="$(qm_json_number "${baseline}" web_tests)"
  floor_web_files="$(qm_json_number "${baseline}" web_files)"
  golden_scalar="$(qm_json_string "${baseline}" value_q10_100_42)"
  echo "floor: rust ${floor_rust} tests over ${floor_rust_bin} binaries; web ${floor_web} tests over ${floor_web_files} files"
  record_pass "baseline.json parses and carries every floor"
  echo

  # -- 2. Fixtures ----------------------------------------------------------
  echo "== fixtures =="
  local fixture="${root}/fixtures/tiny-llama-2shard"
  if qm_require_fixture "${fixture}" \
    && qm_require_fixture "${fixture}/golden.json" \
    && qm_require_fixture "${fixture}/model.safetensors.index.json"; then
    record_pass "fixtures/tiny-llama-2shard is present"
  else
    record_failure "fixtures/tiny-llama-2shard is incomplete"
  fi
  echo

  # -- 3. Rust gates --------------------------------------------------------
  echo "== cargo fmt --all -- --check =="
  if (cd "${root}" && cargo fmt --all -- --check) >"${log_dir}/fmt.log" 2>&1; then
    record_pass "cargo fmt --all -- --check"
  else
    tail -40 "${log_dir}/fmt.log" >&2
    record_failure "cargo fmt --all -- --check"
  fi

  echo "== cargo clippy --workspace --all-targets -- -D warnings =="
  if (cd "${root}" && cargo clippy --workspace --all-targets -- -D warnings) \
    >"${log_dir}/clippy.log" 2>&1; then
    record_pass "cargo clippy --workspace --all-targets -- -D warnings"
  else
    grep -E '^(error|warning)' "${log_dir}/clippy.log" | head -40 >&2
    record_failure "cargo clippy --workspace --all-targets -- -D warnings"
  fi

  echo "== cargo build --workspace --all-targets =="
  if (cd "${root}" && cargo build --workspace --all-targets) \
    >"${log_dir}/build.log" 2>&1; then
    record_pass "cargo build --workspace --all-targets"
  else
    tail -40 "${log_dir}/build.log" >&2
    record_failure "cargo build --workspace --all-targets"
  fi

  echo "== cargo test --workspace =="
  local rust_status rust_counts rust_passed rust_failed rust_ignored rust_bins
  (cd "${root}" && cargo test --workspace) >"${log_dir}/cargo-test.log" 2>&1
  rust_status=$?
  if [ "${rust_status}" -eq 0 ]; then
    record_pass "cargo test --workspace exited 0"
  else
    record_failure "cargo test --workspace exited ${rust_status}"
    echo "--- failing Rust tests ---" >&2
    qm_rust_failure_names "${log_dir}/cargo-test.log" >&2
    echo "--------------------------" >&2
  fi

  if rust_counts="$(qm_sum_rust_results "${log_dir}/cargo-test.log")"; then
    set -- ${rust_counts}
    rust_passed="$1"; rust_failed="$2"; rust_ignored="$3"; rust_bins="$4"
    echo "rust: ${rust_passed} passed; ${rust_failed} failed; ${rust_ignored} ignored; ${rust_bins} binaries"

    if [ "${rust_failed}" -eq 0 ]; then
      record_pass "rust: 0 failed"
    else
      record_failure "rust: ${rust_failed} failed"
    fi
    check_against_floor "rust tests" "${rust_passed}" "${floor_rust}"
    # Structural floor: a binary that fails to build stops printing its
    # `test result:` line, which would otherwise shrink the total silently.
    check_against_floor "rust test binaries" "${rust_bins}" "${floor_rust_bin}"
  else
    record_failure "cargo test produced no parseable 'test result:' line"
  fi
  echo

  # -- 4. Web gate ----------------------------------------------------------
  echo "== npx vitest run (apps/web) =="
  local web_status web_counts web_files_passed web_files_total web_tests_passed web_tests_total
  if [ ! -d "${root}/apps/web/node_modules" ]; then
    echo "error: apps/web/node_modules is absent." >&2
    echo "       run 'npm install' in apps/web first; this guard does not touch the network." >&2
    record_failure "apps/web dependencies are not installed"
  else
    (cd "${root}/apps/web" && npx vitest run) >"${log_dir}/vitest.log" 2>&1
    web_status=$?
    if [ "${web_status}" -eq 0 ]; then
      record_pass "npx vitest run exited 0"
    else
      record_failure "npx vitest run exited ${web_status}"
      echo "--- failing web tests ---" >&2
      qm_vitest_failure_names "${log_dir}/vitest.log" >&2
      echo "-------------------------" >&2
    fi

    if web_counts="$(qm_parse_vitest "${log_dir}/vitest.log")"; then
      set -- ${web_counts}
      web_files_passed="$1"; web_files_total="$2"
      web_tests_passed="$3"; web_tests_total="$4"
      echo "web: ${web_tests_passed}/${web_tests_total} tests passed over ${web_files_passed}/${web_files_total} files"

      # Anti-vacuous-pass: `2 skipped | 113 passed (115)` must fail. Every
      # collected test must have passed, not merely not-failed.
      if [ "${web_tests_passed}" -eq "${web_tests_total}" ] \
        && [ "${web_files_passed}" -eq "${web_files_total}" ]; then
        record_pass "web: every collected test passed"
      else
        record_failure "web: ${web_tests_passed} of ${web_tests_total} tests passed (skipped, todo, or failed)"
      fi
      check_against_floor "web tests" "${web_tests_total}" "${floor_web}"
      check_against_floor "web test files" "${web_files_total}" "${floor_web_files}"
    else
      record_failure "vitest produced no parseable summary line"
    fi
  fi
  echo

  # -- 5. CLI goldens -------------------------------------------------------
  #
  # Every expected value below is read from `scripts/baseline.json`, and every
  # value recorded there was derived independently of q-cli:
  #   * tensor/shard/byte counts and the exact scalar come from
  #     fixtures/tiny-llama-2shard/golden.json (Python safetensors==0.8.0);
  #   * the layer rows and the planned matmul shape come from the fixture's own
  #     config.json;
  #   * the block statistics were computed from golden.json's f32 bit patterns.
  # See .plan/evidence/QM-0001.md for the derivations.
  echo "== CLI goldens (fixtures/tiny-llama-2shard) =="
  local q_bin
  if [ -x "${root}/target/debug/q" ]; then
    q_bin="${root}/target/debug/q"
  else
    q_bin=""
  fi

  qm_cli() { # subcommand args... -> squeezed stdout
    if [ -n "${q_bin}" ]; then
      (cd "${root}" && "${q_bin}" "$@") 2>/dev/null | qm_squeeze
    else
      (cd "${root}" && cargo run -q -p q-cli -- "$@") 2>/dev/null | qm_squeeze
    fi
  }

  check_golden() { # label key actual_output
    local label="$1" key="$2" actual="$3" expected
    if ! expected="$(qm_json_string "${baseline}" "${key}")"; then
      record_failure "cli golden ${label}: key ${key} absent from baseline.json"
      return 1
    fi
    case "${actual}" in
      *"${expected}"*) record_pass "cli golden ${label}: ${expected}" ;;
      *)
        record_failure "cli golden ${label}: expected to find [${expected}]"
        echo "  actual output was:" >&2
        printf '%s\n' "${actual}" | head -20 | sed 's/^/    /' >&2
        ;;
    esac
  }

  local out
  out="$(qm_cli inspect fixtures/tiny-llama-2shard)"
  check_golden "inspect/tensors" inspect_tensors "${out}"
  check_golden "inspect/shards" inspect_shards "${out}"
  check_golden "inspect/payload" inspect_payload "${out}"

  out="$(qm_cli layers fixtures/tiny-llama-2shard)"
  check_golden "layers/first" layers_first_row "${out}"
  check_golden "layers/last" layers_last_row "${out}"
  local layer_rows expected_rows
  layer_rows="$(qm_cli layers fixtures/tiny-llama-2shard | grep -cE '^[0-9]+ [0-9]+ [0-9]+ [0-9]+$')"
  expected_rows="$(qm_json_string "${baseline}" layers_row_count)"
  if [ "${layer_rows}" = "${expected_rows}" ]; then
    record_pass "cli golden layers/row-count: ${layer_rows}"
  else
    record_failure "cli golden layers/row-count: expected ${expected_rows}, got ${layer_rows}"
  fi

  out="$(qm_cli value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42)"
  check_golden "value/Q[10][100,42]" value_q10_100_42 "${out}"
  # AC-4 names the exact scalar; assert it is the FIRST line, not merely present.
  if [ "$(printf '%s\n' "${out}" | head -1)" = "${golden_scalar}" ]; then
    record_pass "cli golden value: the exact scalar is the first line of output"
  else
    record_failure "cli golden value: first line is not ${golden_scalar}"
  fi

  out="$(qm_cli slice fixtures/tiny-llama-2shard 'Q[10]' --rows 100:102 --columns 40:43)"
  check_golden "slice/Q[10][100:102,40:43]" slice_q10_row100 "${out}"

  out="$(qm_cli query fixtures/tiny-llama-2shard 'show tensor("Q[10]") @ transpose(tensor("K[10]"))')"
  check_golden "query/shape" query_qkt_shape "${out}"

  out="$(qm_cli stats fixtures/tiny-llama-2shard \
    'model.layers[10].self_attention.query_projection.weight' --rows 100:104 --columns 40:44)"
  check_golden "stats/min-max" stats_min_max "${out}"
  check_golden "stats/mean" stats_mean "${out}"
  check_golden "stats/l1-l2" stats_l1_l2 "${out}"
  echo

  # -- 6. Report ------------------------------------------------------------
  elapsed=$(( $(date +%s) - started ))
  echo "== verify-baseline summary =="
  printf '%s\n' "${QM_REPORT}"
  echo
  echo "elapsed: ${elapsed}s (budget: 300s — TASK.md, Memory and Performance Constraints)"
  if [ "${elapsed}" -gt 300 ]; then
    echo "warning: the guard took longer than the 5-minute budget" >&2
  fi

  # The blind spot, made visible. A floor below reality does not fail this
  # guard — it silently stops protecting the gap. Say so, loudly, with numbers.
  if [ "${QM_STALE}" -ne 0 ]; then
    echo
    echo "STALE FLOOR — scripts/baseline.json sits below what this tree measures:"
    printf '%s\n' "${QM_STALE_REPORT}"
    echo
    echo "  This is NOT a regression and does not fail the guard. It means the floor"
    echo "  has stopped protecting the difference: tests above the floor could be"
    echo "  deleted without this guard noticing. Raise the recorded floor to the"
    echo "  measured values in the same commit that added the tests."
    echo "  The floor may only ever be raised, never lowered."
  fi

  if [ "${QM_FAILURES}" -ne 0 ]; then
    echo "verify-baseline: FAILED — ${QM_FAILURES} check(s) did not pass" >&2
    echo "logs retained in ${log_dir}" >&2
    return 1
  fi

  rm -rf "${log_dir}"
  echo "verify-baseline: OK"
  return 0
}

# Run only when executed, never when sourced by the test harness.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  main "$@"
  exit $?
fi
