#!/usr/bin/env bash
#
# QM-0001 — unit tests for the pure functions inside `scripts/verify-baseline.sh`.
#
# These tests exist because a guard that has never been seen to fire is not a
# guard. They drive every parsing and comparison function with hand-written
# fixture text and hand-computed expectations, so the guard's behaviour is
# pinned without running either real suite.
#
# Deliberately NOT a `cargo test` or a `vitest` test: this harness must not
# change the very counts the floor records. It is run as a preflight step by
# `verify-baseline.sh` itself, and can also be run directly:
#
#     ./scripts/verify-baseline.test.sh
#
# Properties this harness preserves (.plan/TEST_STRATEGY.md §0 and §7):
#   * No test touches the network.
#   * Every expected value is a hand-written literal or is read from an
#     independent source (`fixtures/tiny-llama-2shard/golden.json`, produced by
#     Python `safetensors==0.8.0`) — never from the code under test.
#   * Every test name is an assertion.
#
# Written for bash 3.2 (the macOS system bash); no associative arrays, no
# `mapfile`, no `${var,,}`.

set -uo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT_UNDER_TEST="${TEST_DIR}/verify-baseline.sh"

if [ ! -f "${SCRIPT_UNDER_TEST}" ]; then
  echo "verify-baseline.test.sh: ${SCRIPT_UNDER_TEST} does not exist" >&2
  exit 1
fi

# Sourcing must not execute the guard. `verify-baseline.sh` runs `main` only
# when it is executed directly.
# shellcheck source=/dev/null
. "${SCRIPT_UNDER_TEST}"

TESTS_RUN=0
TESTS_FAILED=0
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/qm0001-selftest.XXXXXX")"
trap 'rm -rf "${TMP_ROOT}"' EXIT

pass() {
  TESTS_RUN=$((TESTS_RUN + 1))
  printf '  ok   %s\n' "$1"
}

fail() {
  TESTS_RUN=$((TESTS_RUN + 1))
  TESTS_FAILED=$((TESTS_FAILED + 1))
  printf '  FAIL %s\n' "$1" >&2
  printf '       %s\n' "$2" >&2
}

assert_eq() { # name expected actual
  if [ "$2" = "$3" ]; then
    pass "$1"
  else
    fail "$1" "expected [$2] but got [$3]"
  fi
}

assert_contains() { # name haystack needle
  case "$2" in
    *"$3"*) pass "$1" ;;
    *) fail "$1" "expected output to contain [$3] but got [$2]" ;;
  esac
}

assert_status() { # name expected_status actual_status
  if [ "$2" -eq "$3" ]; then
    pass "$1"
  else
    fail "$1" "expected exit status $2 but got $3"
  fi
}

write_tmp() { # name content -> path
  local path="${TMP_ROOT}/$1"
  printf '%s\n' "$2" >"${path}"
  printf '%s' "${path}"
}

# ---------------------------------------------------------------------------
# qm_sum_rust_results — sums the `test result:` lines cargo prints per binary.
# ---------------------------------------------------------------------------

RUST_THREE_BINARIES='   Compiling q-source v0.1.0
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 21 tests
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s'

# 9 + 13 + 21 = 43, computed by hand.
RUST_FIXTURE="$(write_tmp rust-three.txt "${RUST_THREE_BINARIES}")"
assert_eq "sums_the_passed_column_across_every_rust_test_binary" \
  "43 0 0 3" "$(qm_sum_rust_results "${RUST_FIXTURE}")"

# A count ending in zero must not be dropped. `grep -v '0 passed'` silently
# discards `10 passed`, `20 passed` and `290 passed` because they contain the
# substring `0 passed`; this test pins that the summation is not written that
# way. 10 + 40 = 50, by hand.
RUST_TRAILING_ZERO='test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s'
RUST_TZ_FIXTURE="$(write_tmp rust-trailing-zero.txt "${RUST_TRAILING_ZERO}")"
assert_eq "counts_a_binary_whose_passed_total_ends_in_a_zero_digit" \
  "50 0 0 2" "$(qm_sum_rust_results "${RUST_TZ_FIXTURE}")"

RUST_ONE_FAILURE='test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

failures:
    reads_one_exact_scalar

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s'
RUST_FAIL_FIXTURE="$(write_tmp rust-one-failure.txt "${RUST_ONE_FAILURE}")"
assert_eq "reports_the_failed_column_when_a_rust_binary_fails" \
  "16 1 0 2" "$(qm_sum_rust_results "${RUST_FAIL_FIXTURE}")"

RUST_IGNORED='test result: ok. 5 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s'
RUST_IGNORED_FIXTURE="$(write_tmp rust-ignored.txt "${RUST_IGNORED}")"
assert_eq "reports_the_ignored_column_so_a_skipped_test_cannot_hide" \
  "5 0 2 1" "$(qm_sum_rust_results "${RUST_IGNORED_FIXTURE}")"

RUST_EMPTY_FIXTURE="$(write_tmp rust-empty.txt "")"
qm_sum_rust_results "${RUST_EMPTY_FIXTURE}" >/dev/null 2>&1
assert_status "raises_rather_than_reporting_zero_when_no_rust_test_result_line_exists" \
  1 $?

# ---------------------------------------------------------------------------
# qm_parse_vitest — reads the two summary lines vitest prints.
# ---------------------------------------------------------------------------

VITEST_CLEAN=' RUN  v4.1.10 /repo/apps/web

 Test Files  13 passed (13)
      Tests  115 passed (115)
   Start at  06:57:16'
VITEST_CLEAN_FIXTURE="$(write_tmp vitest-clean.txt "${VITEST_CLEAN}")"
assert_eq "parses_the_vitest_file_and_test_totals_from_the_summary_lines" \
  "13 13 115 115" "$(qm_parse_vitest "${VITEST_CLEAN_FIXTURE}")"

# The QM-0006 defect class: a suite that reports success while collecting less
# than it should. `passed` below `total` must be visible to the caller.
VITEST_SKIPPED=' Test Files  13 passed (13)
      Tests  2 skipped | 113 passed (115)'
VITEST_SKIPPED_FIXTURE="$(write_tmp vitest-skipped.txt "${VITEST_SKIPPED}")"
assert_eq "reports_passed_below_total_when_vitest_skips_a_test" \
  "13 13 113 115" "$(qm_parse_vitest "${VITEST_SKIPPED_FIXTURE}")"

VITEST_FAILED=' Test Files  1 failed | 12 passed (13)
      Tests  1 failed | 114 passed (115)'
VITEST_FAILED_FIXTURE="$(write_tmp vitest-failed.txt "${VITEST_FAILED}")"
assert_eq "reports_passed_below_total_when_a_vitest_file_fails" \
  "12 13 114 115" "$(qm_parse_vitest "${VITEST_FAILED_FIXTURE}")"

VITEST_EMPTY_FIXTURE="$(write_tmp vitest-empty.txt "")"
qm_parse_vitest "${VITEST_EMPTY_FIXTURE}" >/dev/null 2>&1
assert_status "raises_rather_than_reporting_zero_when_the_vitest_summary_is_absent" \
  1 $?

# ---------------------------------------------------------------------------
# qm_baseline_validate / qm_json_number / qm_json_string
# ---------------------------------------------------------------------------

qm_baseline_validate "${TMP_ROOT}/no-such-file.json" >/dev/null 2>&1
assert_status "exits_nonzero_when_the_baseline_file_is_absent" 1 $?

MISSING_MSG="$(qm_baseline_validate "${TMP_ROOT}/no-such-file.json" 2>&1)"
assert_contains "names_the_missing_baseline_file_rather_than_failing_silently" \
  "${MISSING_MSG}" "no-such-file.json"

MALFORMED="$(write_tmp malformed.json '{ this is not json at all')"
qm_baseline_validate "${MALFORMED}" >/dev/null 2>&1
assert_status "exits_nonzero_when_the_baseline_file_is_not_parseable_json" 1 $?

TRUNCATED="$(write_tmp truncated.json '{ "commit": "abc", "rust_tests": 290,')"
qm_baseline_validate "${TRUNCATED}" >/dev/null 2>&1
assert_status "exits_nonzero_when_the_baseline_file_is_truncated_mid_object" 1 $?

INCOMPLETE="$(write_tmp incomplete.json '{ "commit": "abc", "rust_tests": 290 }')"
qm_baseline_validate "${INCOMPLETE}" >/dev/null 2>&1
assert_status "exits_nonzero_when_a_required_floor_key_is_absent" 1 $?

# Exactly one floor is absent, so the message can only name that one. A fixture
# missing several keys would pass this assertion by accident.
ONE_KEY_SHORT="$(write_tmp one-key-short.json '{
  "commit": "abc",
  "rust_tests": 290,
  "rust_binaries": 39,
  "web_files": 13,
  "cli_golden": { "value_q10_100_42": "0.006408154033124447" }
}')"
ONE_KEY_SHORT_MSG="$(qm_baseline_validate "${ONE_KEY_SHORT}" 2>&1)"
assert_contains "names_the_one_absent_floor_key_rather_than_defaulting_it_to_zero" \
  "${ONE_KEY_SHORT_MSG}" "web_tests"

NON_NUMERIC="$(write_tmp non-numeric.json '{ "commit": "abc", "rust_tests": "many", "rust_binaries": 39, "web_tests": 115, "web_files": 13 }')"
qm_baseline_validate "${NON_NUMERIC}" >/dev/null 2>&1
assert_status "exits_nonzero_when_a_floor_value_is_not_a_positive_integer" 1 $?

WELL_FORMED="$(write_tmp well-formed.json '{
  "commit": "793e122044cf3a778d4e68fa8e38e69e91bc203a",
  "rust_tests": 290,
  "rust_binaries": 39,
  "web_tests": 115,
  "web_files": 13,
  "cli_golden": { "value_q10_100_42": "0.006408154033124447" }
}')"
qm_baseline_validate "${WELL_FORMED}" >/dev/null 2>&1
assert_status "exits_zero_on_a_well_formed_baseline_file" 0 $?

assert_eq "reads_the_rust_floor_verbatim_from_the_baseline_file" \
  "290" "$(qm_json_number "${WELL_FORMED}" rust_tests)"
assert_eq "reads_the_web_floor_verbatim_from_the_baseline_file" \
  "115" "$(qm_json_number "${WELL_FORMED}" web_tests)"
assert_eq "reads_a_nested_cli_golden_string_verbatim_from_the_baseline_file" \
  "0.006408154033124447" "$(qm_json_string "${WELL_FORMED}" value_q10_100_42)"
assert_eq "reads_the_recorded_commit_verbatim_from_the_baseline_file" \
  "793e122044cf3a778d4e68fa8e38e69e91bc203a" "$(qm_json_string "${WELL_FORMED}" commit)"

qm_json_number "${WELL_FORMED}" no_such_key >/dev/null 2>&1
assert_status "raises_rather_than_returning_empty_for_an_absent_numeric_key" 1 $?

# ---------------------------------------------------------------------------
# qm_check_floor — the comparison the whole guard exists for.
# ---------------------------------------------------------------------------

qm_check_floor rust 290 999 >/dev/null 2>&1
assert_status "exits_nonzero_when_the_real_count_is_below_the_floor" 1 $?

REGRESSION_MSG="$(qm_check_floor rust 290 999 2>&1)"
assert_contains "names_both_numbers_in_the_baseline_regression_message" \
  "${REGRESSION_MSG}" "baseline regression: 290 < 999"

qm_check_floor rust 290 290 >/dev/null 2>&1
assert_status "exits_zero_when_the_real_count_equals_the_floor" 0 $?

qm_check_floor rust 291 290 >/dev/null 2>&1
assert_status "exits_zero_when_the_real_count_exceeds_the_floor" 0 $?

# The documented blind spot, pinned as behaviour rather than left implicit: a
# floor BELOW the real count is silent. This test asserts the guard does not
# fire, so anyone reading the suite sees the limit.
SILENT_MSG="$(qm_check_floor rust 290 1 2>&1)"
assert_eq "is_silent_when_the_floor_is_set_below_the_real_count_which_is_the_blind_spot" \
  "" "${SILENT_MSG}"

# ---------------------------------------------------------------------------
# qm_floor_status — reports a measurement against its floor in BOTH directions,
# so the below-reality blind spot is visible in the log even though it is not
# fatal. A concrete instance exists: QM-0012 merged 28 rust tests to main after
# this floor was measured, so main measures 318 against a recorded floor of 290.
# ---------------------------------------------------------------------------

qm_floor_status "rust tests" 290 999 >/dev/null 2>&1
assert_status "reports_status_1_when_the_measurement_is_below_the_floor" 1 $?

qm_floor_status "rust tests" 290 290 >/dev/null 2>&1
assert_status "reports_status_0_when_the_measurement_equals_the_floor" 0 $?

qm_floor_status "rust tests" 318 290 >/dev/null 2>&1
assert_status "reports_status_2_when_the_measurement_exceeds_the_floor" 2 $?

STALE_LINE="$(qm_floor_status "rust tests" 318 290 2>/dev/null)"
assert_contains "names_the_floor_as_stale_when_it_sits_below_the_real_count" \
  "${STALE_LINE}" "FLOOR IS STALE by 28"
assert_contains "prints_the_measured_count_beside_the_floor_when_the_floor_is_stale" \
  "${STALE_LINE}" "measured 318, floor 290"

AT_FLOOR_LINE="$(qm_floor_status "rust tests" 290 290 2>/dev/null)"
assert_contains "prints_the_measured_count_beside_the_floor_when_they_are_equal" \
  "${AT_FLOOR_LINE}" "measured 290, floor 290"

REGRESSION_LINE="$(qm_floor_status "rust tests" 290 999 2>/dev/null)"
assert_contains "prints_the_measured_count_beside_the_floor_on_a_regression" \
  "${REGRESSION_LINE}" "measured 290, floor 999"

# The regression message Test Cases row 3 specifies must still reach stderr.
REGRESSION_STDERR="$(qm_floor_status "rust tests" 290 999 2>&1 >/dev/null)"
assert_contains "still_emits_the_specified_baseline_regression_message_on_stderr" \
  "${REGRESSION_STDERR}" "baseline regression: 290 < 999"

# ---------------------------------------------------------------------------
# qm_require_fixture
# ---------------------------------------------------------------------------

qm_require_fixture "${TMP_ROOT}/no-such-fixture" >/dev/null 2>&1
assert_status "exits_nonzero_when_a_required_fixture_directory_is_absent" 1 $?

FIXTURE_MSG="$(qm_require_fixture "${TMP_ROOT}/no-such-fixture" 2>&1)"
assert_contains "names_the_fixture_generator_when_a_fixture_is_absent" \
  "${FIXTURE_MSG}" "fixtures/generate_fixtures.py"

qm_require_fixture "${TMP_ROOT}" >/dev/null 2>&1
assert_status "exits_zero_when_the_required_fixture_is_present" 0 $?

# ---------------------------------------------------------------------------
# qm_squeeze — makes the CLI goldens independent of column alignment.
# ---------------------------------------------------------------------------

assert_eq "collapses_runs_of_spaces_so_cli_goldens_are_column_independent" \
  "tensors 111" "$(printf 'tensors             111\n' | qm_squeeze)"

assert_eq "strips_leading_and_trailing_whitespace_from_a_squeezed_line" \
  "0 9 24672 92544" "$(printf '    0         9           24672           92544   \n' | qm_squeeze)"

# ---------------------------------------------------------------------------
# The recorded floor itself. These read the real `scripts/baseline.json` and
# compare it against the counts measured on base commit 793e122. They are the
# floor-only-rises rule expressed as a test: a task that lowers the floor turns
# them red.
# ---------------------------------------------------------------------------

REPO_ROOT_FOR_TESTS="$(cd "${TEST_DIR}/.." && pwd)"
BASELINE_FILE="${REPO_ROOT_FOR_TESTS}/scripts/baseline.json"

if [ ! -f "${BASELINE_FILE}" ]; then
  fail "the_repository_records_a_baseline_floor_file" \
    "${BASELINE_FILE} does not exist"
else
  # 290 and 115 are the counts measured on 793e122 by running both suites; see
  # .plan/evidence/QM-0001.md. They are literals here on purpose, so that
  # lowering the recorded floor fails rather than silently agreeing with itself.
  RECORDED_RUST="$(qm_json_number "${BASELINE_FILE}" rust_tests)"
  RECORDED_WEB="$(qm_json_number "${BASELINE_FILE}" web_tests)"
  RECORDED_RUST_BINARIES="$(qm_json_number "${BASELINE_FILE}" rust_binaries)"
  RECORDED_WEB_FILES="$(qm_json_number "${BASELINE_FILE}" web_files)"

  if [ "${RECORDED_RUST:-0}" -ge 290 ]; then
    pass "the_recorded_rust_floor_is_never_below_the_290_measured_on_793e122"
  else
    fail "the_recorded_rust_floor_is_never_below_the_290_measured_on_793e122" \
      "recorded ${RECORDED_RUST}, measured 290 — the floor may only be raised"
  fi

  if [ "${RECORDED_WEB:-0}" -ge 115 ]; then
    pass "the_recorded_web_floor_is_never_below_the_115_measured_on_793e122"
  else
    fail "the_recorded_web_floor_is_never_below_the_115_measured_on_793e122" \
      "recorded ${RECORDED_WEB}, measured 115 — the floor may only be raised"
  fi

  if [ "${RECORDED_RUST_BINARIES:-0}" -ge 39 ]; then
    pass "the_recorded_rust_binary_floor_is_never_below_the_39_measured_on_793e122"
  else
    fail "the_recorded_rust_binary_floor_is_never_below_the_39_measured_on_793e122" \
      "recorded ${RECORDED_RUST_BINARIES}, measured 39"
  fi

  if [ "${RECORDED_WEB_FILES:-0}" -ge 13 ]; then
    pass "the_recorded_web_file_floor_is_never_below_the_13_measured_on_793e122"
  else
    fail "the_recorded_web_file_floor_is_never_below_the_13_measured_on_793e122" \
      "recorded ${RECORDED_WEB_FILES}, measured 13"
  fi

  # STATUS.md still records 101 for the web suite, measured at 5ca434d before
  # QM-0006 repaired vitest's include globs. Recording 101 would set the floor
  # 14 tests below reality.
  if [ "${RECORDED_WEB:-0}" -ne 101 ]; then
    pass "the_recorded_web_floor_is_not_the_stale_101_from_status_md"
  else
    fail "the_recorded_web_floor_is_not_the_stale_101_from_status_md" \
      "101 is the pre-QM-0006 count; the suite collects 115"
  fi

  # The CLI golden must come from the Python reference, not from q-cli. This
  # ties scripts/baseline.json to fixtures/.../golden.json, which
  # fixtures/generate_fixtures.py produced with safetensors==0.8.0.
  GOLDEN_JSON="${REPO_ROOT_FOR_TESTS}/fixtures/tiny-llama-2shard/golden.json"
  RECORDED_SCALAR="$(qm_json_string "${BASELINE_FILE}" value_q10_100_42)"
  if grep -q "${RECORDED_SCALAR}" "${GOLDEN_JSON}" 2>/dev/null; then
    pass "the_recorded_cli_scalar_golden_appears_in_the_python_generated_golden_json"
  else
    fail "the_recorded_cli_scalar_golden_appears_in_the_python_generated_golden_json" \
      "[${RECORDED_SCALAR}] is not in ${GOLDEN_JSON}; the expected value must not come from q-cli"
  fi
fi

# ---------------------------------------------------------------------------

printf '\n%s: %d run, %d failed\n' "$(basename "${BASH_SOURCE[0]}")" \
  "${TESTS_RUN}" "${TESTS_FAILED}"

if [ "${TESTS_FAILED}" -ne 0 ]; then
  exit 1
fi
exit 0
