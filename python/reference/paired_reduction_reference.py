#!/usr/bin/env python3
"""Independent NumPy reference for the paired block reduction.

`QM-0121` / `QUANT-002`.  This script is the **reference**, not a port: it is
written directly from the contract block in `.plan/DIAGNOSTIC_ARCHITECTURE.md`
§4.1--§4.2 and `.plan/tasks/QM-0121-paired-block-reduction/TASK.md` §Data
Contracts (both reproduced verbatim below), and `crates/q-gpu`'s `CpuBackend`
is written to agree with the goldens this script emits.
`.plan/DIAGNOSTIC_ARCHITECTURE.md` §9 requires "a committed Python/NumPy script
under `python/`, run in CI-equivalent form"; `.plan/TEST_STRATEGY.md` §0
requires that expected values come from an independent implementation rather
than from the code under test.

The contract, verbatim (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.1)::

    pub struct PairedPartials {
        pub count: u64,
        pub sum_sq_base: f64,        // Σ w²        — denominator of relative error
        pub sum_sq_delta: f64,       // Σ (w − ŵ)²  — numerator; ‖·‖_F² before the root
        pub sum_abs_delta: f64,      // Σ |w − ŵ|
        pub max_abs_delta: f64,      // max |w − ŵ|
        pub max_abs_base: f64,       // max |w|     — for outlier attribution
        pub per_channel: Vec<ChannelPartials>,   // len == channel count of this block
    }

and (`TASK.md` §Data Contracts)::

    pub enum ChannelAxis { Rows, Columns }

    pub struct ChannelPartials {
        pub count: u64,
        pub sum_sq_base: f64,
        pub sum_sq_delta: f64,
        pub sum_abs_delta: f64,
        pub max_abs_delta: f64,
        pub max_abs_base: f64,
    }

**Everything is a partial, nothing is a final metric** (§4.1).  This script
emits no RMSE, no relative error and no norm, for the same reason the Rust does
not: those do not compose, and computing them per block and averaging is
"the single most likely correctness bug in this engine".

Arithmetic discipline
---------------------

`TASK.md` §Data Contracts: *"`f64` accumulators over `f32` inputs, throughout.
The inputs are f32; the sums are not."*  So:

* every input value is `float32` and the dtype is **asserted**;
* `f32 -> f64` widening is exact, so `float64(base)` and `float64(counterpart)`
  introduce no rounding of their own;
* `d = float64(base) - float64(counterpart)`, `d*d` and `b*b` are single
  IEEE-754 binary64 operations, each correctly rounded, so an implementation
  performing the same operations must produce the same bits;
* the three sums accumulate **sequentially in flat row-major index order**.

That last point is load-bearing and is why this script contains a Python loop
where idiomatic NumPy would write ``np.sum``.  `.plan/DIAGNOSTIC_ARCHITECTURE.md`
§4.2 fixes the reduction order --- *"partials accumulate in that order,
single-threaded at the accumulation step"*, because *"parallel accumulation with
floating-point addition is not associative, and `V1-13` requires byte-identical
output across runs"*.  ``np.sum`` on float64 uses **pairwise** summation, a
different and unspecified grouping; using it here would silently define a
different answer from the one §4.2 specifies.  The pairwise value is emitted
anyway, in ``discriminators.numpy_pairwise_*``, so the golden **records** how far
a different-but-reasonable order lands from the specified one rather than
leaving a reviewer to guess.  Python's ``float`` is IEEE-754 binary64 with
round-to-nearest-even, so the loop is the specification, executed.

The two ``max_abs_*`` fields are order-independent and exact --- a max reduction
has no rounding excuse (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.3).

Channel order
-------------

One pass over the flat buffer in row-major index order.  Element ``i`` sits at
``(row, column) = divmod(i, columns)`` and is dispatched into the whole-block
accumulator and into exactly one channel accumulator:

* ``axis = rows``    -> channel ``row``;    a channel's elements arrive in
  column order (contiguous).
* ``axis = columns`` -> channel ``column``; a channel's elements arrive in row
  order (strided).

The channel count is therefore ``rows`` or ``columns`` respectively, matching
§4.1's ``len == channel count of this block``.

Discriminating inputs, not merely covering ones
-----------------------------------------------

`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1, amended 2026-08-05 after `QM-0120`
shipped a 100 % error behind a passing differential test::

    Agreement with a reference proves the arithmetic matches on the values you
    chose. It does not prove you chose values that can distinguish two
    candidate formulas. A golden set needs inputs selected to **discriminate**,
    not merely to cover.

Every numeric case therefore carries a ``discriminators`` object holding the
value each **plausible wrong formula** would have produced on that same input:

============================  ==================================================
key                           the wrong formula it rules out
============================  ==================================================
``abs_of_sum_delta``          ``|Σ d|`` mistaken for ``Σ |d|`` --- identical
                              whenever every delta shares one sign
``max_signed_delta``          ``max d`` mistaken for ``max |d|`` --- identical
                              whenever the extreme delta is positive
``max_signed_base``           ``max w`` mistaken for ``max |w|`` --- identical
                              whenever the extreme weight is positive
``sum_sq_counterpart``        ``Σ ŵ²`` mistaken for ``Σ w²`` --- identical
                              whenever the two blocks are equal
``f32_sum_sq_base`` etc.      an ``f32`` accumulator instead of ``f64``
``numpy_pairwise_*``          a different (pairwise) summation order
``transposed_per_channel``    the two axes swapped --- indistinguishable on a
                              block that is symmetric under transposition
============================  ==================================================

``discriminates`` records, per key, whether the wrong value actually **differs**
from the right one on this input.  A case where it does not differ cannot rule
that formula out, and the Rust test asserts the flag rather than trusting the
prose.  ``build()`` refuses to emit a golden set in which some formula is ruled
out by no case at all.

Usage
-----

    python3 python/reference/paired_reduction_reference.py \\
        --emit-goldens crates/q-gpu/tests/goldens/

The emitted JSON carries **no version strings**, so re-running this script must
reproduce the file byte-for-byte.  The interpreter and NumPy versions are
printed to stderr instead, and recorded in `.plan/evidence/QM-0121.md` together
with the file's SHA-256.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys

import numpy as np

F32 = np.float32
F64 = np.float64

# The six additive-or-max partial fields, in the order §4.1 declares them.
SUM_FIELDS = ("sum_sq_base", "sum_sq_delta", "sum_abs_delta")
MAX_FIELDS = ("max_abs_delta", "max_abs_base")


class Refused(Exception):
    """The reference refuses this input.  Mirrors the `QError` the Rust returns.

    ``kind`` names the refusal; ``detail`` is the message content
    `TASK.md` §Error Handling requires the refusal to carry.
    """

    def __init__(self, kind: str, detail: str) -> None:
        super().__init__(f"{kind}: {detail}")
        self.kind = kind
        self.detail = detail


# --------------------------------------------------------------------------
# Validation — TASK.md §Error Handling, in the order the Rust applies it
# --------------------------------------------------------------------------


def validate(base: np.ndarray, counterpart: np.ndarray,
             rows: int, columns: int) -> None:
    """Refuse before any arithmetic.

    `TASK.md` §Error Handling lists shape mismatch first, then the empty block,
    then the non-finite scan; the budget check sits between the structural
    checks and the value scan because it is O(1) and needs to read nothing.
    This function owns everything except the budget, which is the backend's
    ``check_workload`` and has no reference-side equivalent.
    """
    br, bc = base.shape
    cr, cc = counterpart.shape
    if (br, bc) != (cr, cc):
        raise Refused(
            "shape_mismatch",
            f"shape mismatch: base [{br}, {bc}] and counterpart [{cr}, {cc}] "
            "must have identical shapes for a paired reduction",
        )
    if br == 0 or bc == 0:
        raise Refused(
            "empty_block",
            f"empty block: [{br}, {bc}] has no values; an empty reduction has "
            "no meaningful partials",
        )
    if (br, bc) != (rows, columns):
        raise Refused(
            "ragged_block",
            f"value count disagrees with the declared shape: {base.size} "
            f"values for a {rows}x{columns} block",
        )
    # Scan the two blocks together, position by position, so the refusal names
    # the EARLIEST offending position across both rather than whichever block
    # happened to be scanned first.  `base` is checked before `counterpart` at
    # the same position.
    flat_b = base.reshape(-1)
    flat_c = counterpart.reshape(-1)
    for i in range(flat_b.size):
        for name, flat in (("base", flat_b), ("counterpart", flat_c)):
            v = float(flat[i])
            if not np.isfinite(v):
                r, c = divmod(i, columns)
                raise Refused(
                    "non_finite",
                    f"{name} holds a non-finite value at row {r}, column {c} "
                    f"(index {i}); a non-finite weight is a finding, not "
                    "something to reduce",
                )


# --------------------------------------------------------------------------
# The reduction
# --------------------------------------------------------------------------


def _fresh() -> dict:
    return {
        "count": 0,
        "sum_sq_base": 0.0,
        "sum_sq_delta": 0.0,
        "sum_abs_delta": 0.0,
        "max_abs_delta": 0.0,
        "max_abs_base": 0.0,
    }


def _accumulate(acc: dict, b: float, d: float) -> None:
    """One element into one accumulator.  Every operation is binary64."""
    acc["count"] += 1
    acc["sum_sq_base"] += b * b
    acc["sum_sq_delta"] += d * d
    acc["sum_abs_delta"] += abs(d)
    acc["max_abs_delta"] = max(acc["max_abs_delta"], abs(d))
    acc["max_abs_base"] = max(acc["max_abs_base"], abs(b))


def reduce_paired(base: np.ndarray, counterpart: np.ndarray,
                  axis: str) -> tuple[dict, list[dict]]:
    """§4.1's paired reduction over one block pair, at one channel axis.

    Returns ``(whole_block, per_channel)``.  Neither carries a finished metric.
    """
    assert base.dtype == F32, base.dtype
    assert counterpart.dtype == F32, counterpart.dtype
    rows, columns = base.shape
    validate(base, counterpart, rows, columns)

    # f32 -> f64 is exact; the subtraction is a single correctly rounded
    # binary64 operation.
    b64 = base.reshape(-1).astype(F64)
    c64 = counterpart.reshape(-1).astype(F64)
    d64 = b64 - c64
    assert b64.dtype == F64 and d64.dtype == F64

    if axis == "rows":
        channels = rows
    elif axis == "columns":
        channels = columns
    else:
        raise Refused("axis_out_of_range", f"unknown channel axis {axis!r}")

    whole = _fresh()
    per = [_fresh() for _ in range(channels)]
    for i in range(rows * columns):
        r, c = divmod(i, columns)
        ch = r if axis == "rows" else c
        b = float(b64[i])
        d = float(d64[i])
        _accumulate(whole, b, d)
        _accumulate(per[ch], b, d)
    return whole, per


# --------------------------------------------------------------------------
# The wrong formulas, evaluated on the same input
# --------------------------------------------------------------------------


def wrong_formulas(base: np.ndarray, counterpart: np.ndarray) -> dict:
    """What each plausible mis-derivation would have produced here."""
    b64 = base.reshape(-1).astype(F64)
    c64 = counterpart.reshape(-1).astype(F64)
    d64 = b64 - c64

    # `|Σ d|` instead of `Σ |d|`, accumulated in the same specified order.
    sum_d = 0.0
    for v in d64:
        sum_d += float(v)

    # An f32 accumulator instead of an f64 one, same order.
    f32_sq_base = F32(0.0)
    f32_sq_delta = F32(0.0)
    f32_abs_delta = F32(0.0)
    flat_b = base.reshape(-1)
    flat_c = counterpart.reshape(-1)
    for i in range(flat_b.size):
        bb = F32(flat_b[i])
        dd = F32(flat_b[i] - flat_c[i])
        f32_sq_base = F32(f32_sq_base + bb * bb)
        f32_sq_delta = F32(f32_sq_delta + dd * dd)
        f32_abs_delta = F32(f32_abs_delta + abs(dd))

    return {
        "abs_of_sum_delta": abs(sum_d),
        "max_signed_delta": float(d64.max()),
        "max_signed_base": float(b64.max()),
        "sum_sq_counterpart": _ordered_sum(c64 * c64),
        "f32_sum_sq_base": float(f32_sq_base),
        "f32_sum_sq_delta": float(f32_sq_delta),
        "f32_sum_abs_delta": float(f32_abs_delta),
        # NumPy's own pairwise summation: a different, unspecified grouping.
        "numpy_pairwise_sum_sq_base": float(np.sum(b64 * b64)),
        "numpy_pairwise_sum_sq_delta": float(np.sum(d64 * d64)),
        "numpy_pairwise_sum_abs_delta": float(np.sum(np.abs(d64))),
    }


def _ordered_sum(values: np.ndarray) -> float:
    acc = 0.0
    for v in values:
        acc += float(v)
    return acc


# --------------------------------------------------------------------------
# Emission
# --------------------------------------------------------------------------


def f32_bits(x) -> str:
    return "0x%08x" % int(np.asarray(F32(x)).view(np.uint32))


def f64_bits(x) -> str:
    return "0x%016x" % int(np.asarray(F64(x)).view(np.uint64))


def f32_bit_list(a: np.ndarray) -> list[str]:
    return ["0x%08x" % int(v) for v in a.reshape(-1).astype(F32).view(np.uint32)]


def partials_json(acc: dict, decimals: bool) -> dict:
    out = {"count": acc["count"]}
    for key in SUM_FIELDS + MAX_FIELDS:
        out[key] = f64_bits(acc[key])
    if decimals:
        for key in SUM_FIELDS + MAX_FIELDS:
            out[key + "_decimal"] = float(acc[key])
    return out


def lcg(seed: int, count: int, lo: float, hi: float) -> np.ndarray:
    """A deterministic pseudo-random vector built from an integer LCG.

    Integer-only and version-independent, so regenerating this file cannot
    drift with a NumPy RNG-stream change --- the same discipline
    `python/reference/quantise_reference.py` uses.  Values land in ``[lo, hi)``.
    """
    state = seed
    out = np.empty(count, dtype=F32)
    span = F32(hi) - F32(lo)
    for i in range(count):
        state = (state * 1103515245 + 12345) % (1 << 31)
        out[i] = F32(lo) + span * (F32(state) / F32(1 << 31))
    return out


def case(name: str, why: str, rows: int, columns: int,
         base_values, counterpart_values) -> dict:
    base = np.asarray(base_values, dtype=F32).reshape(rows, columns)
    counterpart = np.asarray(counterpart_values, dtype=F32).reshape(rows, columns)
    small = base.size <= 32

    axes = {}
    for axis in ("rows", "columns"):
        whole, per = reduce_paired(base, counterpart, axis)
        axes[axis] = {
            "channels": len(per),
            "whole_block": partials_json(whole, small),
            "per_channel": [partials_json(p, small) for p in per],
        }

    wrong = wrong_formulas(base, counterpart)
    right = axes["rows"]["whole_block"]
    truth = {
        "abs_of_sum_delta": ("sum_abs_delta", wrong["abs_of_sum_delta"]),
        "max_signed_delta": ("max_abs_delta", wrong["max_signed_delta"]),
        "max_signed_base": ("max_abs_base", wrong["max_signed_base"]),
        "sum_sq_counterpart": ("sum_sq_base", wrong["sum_sq_counterpart"]),
        "f32_sum_sq_base": ("sum_sq_base", wrong["f32_sum_sq_base"]),
        "f32_sum_sq_delta": ("sum_sq_delta", wrong["f32_sum_sq_delta"]),
        "f32_sum_abs_delta": ("sum_abs_delta", wrong["f32_sum_abs_delta"]),
        "numpy_pairwise_sum_sq_base": ("sum_sq_base", wrong["numpy_pairwise_sum_sq_base"]),
        "numpy_pairwise_sum_sq_delta": ("sum_sq_delta", wrong["numpy_pairwise_sum_sq_delta"]),
        "numpy_pairwise_sum_abs_delta": ("sum_abs_delta", wrong["numpy_pairwise_sum_abs_delta"]),
    }
    discriminates = {
        key: f64_bits(value) != right[field] for key, (field, value) in truth.items()
    }
    # How far the wrong value lands from the right one, in bit patterns. For the
    # `numpy_pairwise_*` keys this is the **measured** cost of a different
    # summation order -- the number `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2's
    # fixed-order rule exists to eliminate, reported rather than guessed.
    ulp_distance = {}
    for key, (field, value) in truth.items():
        a = int(right[field], 16)
        b = int(f64_bits(value), 16)
        ulp_distance[key] = abs(a - b)
    # The orientation discriminator is structural rather than scalar: does
    # reducing over the other axis actually give a different per-channel
    # answer?  A block symmetric under transposition would say `false`, and
    # could not prove the axis was not silently swapped.
    same_channel_count = axes["rows"]["channels"] == axes["columns"]["channels"]
    discriminates["transposed_per_channel"] = (
        axes["rows"]["per_channel"] != axes["columns"]["per_channel"]
    )
    # The harder version: on a SQUARE block the channel counts match, so a
    # transposed implementation still returns a plausibly shaped answer and
    # only the values can catch it.
    discriminates["transposed_per_channel_at_equal_channel_count"] = (
        same_channel_count and axes["rows"]["per_channel"] != axes["columns"]["per_channel"]
    )

    entry = {
        "name": name,
        "why": why,
        "rows": rows,
        "columns": columns,
        "base_bits": f32_bit_list(base),
        "counterpart_bits": f32_bit_list(counterpart),
        "axes": axes,
        "discriminators": {k: f64_bits(v) for k, (_, v) in truth.items()},
        "discriminators_decimal": {k: float(v) for k, (_, v) in truth.items()},
        "discriminates": discriminates,
        "discriminator_ulp": ulp_distance,
    }
    if small:
        entry["base"] = [float(v) for v in base.reshape(-1)]
        entry["counterpart"] = [float(v) for v in counterpart.reshape(-1)]
    return entry


def split_case(name: str, why: str, rows: int, columns: int,
               base_values, counterpart_values, split_row: int) -> dict:
    """§4.2's composition, split at a row boundary.

    The whole-block partials of the two halves must compose to the whole-block
    partials of the undivided block: additively for the three sums and the
    count, by maximum for the two ``max_abs_*`` fields.  Emitted so that the
    composition claim is differential rather than self-referential --- the
    halves come from the reference, not from summing the Rust's own output.

    ``composition_is_bit_exact`` records whether the composed sums land on the
    same bits as the undivided sums.  They need not: sequential summation
    regrouped at ``split_row`` is a different association, so the two agree
    exactly only when every partial sum is exact.  ``composition_ulp`` is the
    measured distance, in bit patterns, for each of the three sums.
    """
    base = np.asarray(base_values, dtype=F32).reshape(rows, columns)
    counterpart = np.asarray(counterpart_values, dtype=F32).reshape(rows, columns)

    out = {
        "name": name,
        "why": why,
        "rows": rows,
        "columns": columns,
        "split_row": split_row,
        "base_bits": f32_bit_list(base),
        "counterpart_bits": f32_bit_list(counterpart),
        "axes": {},
    }
    for axis in ("rows", "columns"):
        whole, per = reduce_paired(base, counterpart, axis)
        top_w, top_p = reduce_paired(base[:split_row], counterpart[:split_row], axis)
        bot_w, bot_p = reduce_paired(base[split_row:], counterpart[split_row:], axis)

        composed = _fresh()
        composed["count"] = top_w["count"] + bot_w["count"]
        for f in SUM_FIELDS:
            composed[f] = top_w[f] + bot_w[f]
        for f in MAX_FIELDS:
            composed[f] = max(top_w[f], bot_w[f])

        ulp = {}
        exact = True
        for f in SUM_FIELDS:
            a = int(np.asarray(F64(whole[f])).view(np.uint64))
            b = int(np.asarray(F64(composed[f])).view(np.uint64))
            ulp[f] = abs(a - b)
            exact = exact and a == b

        entry = {
            "channels": len(per),
            "whole_block": partials_json(whole, True),
            "top_half": partials_json(top_w, True),
            "bottom_half": partials_json(bot_w, True),
            "composed": partials_json(composed, True),
            "composition_is_bit_exact": exact,
            "composition_ulp": ulp,
        }
        if axis == "columns":
            # A row split leaves the column channels intact, so the per-channel
            # partials compose additively too.  Under `axis = rows` a row split
            # PARTITIONS the channels instead: the two halves' channel arrays
            # concatenate rather than add, which is recorded rather than summed.
            comp_per = []
            per_ulp = []
            per_exact = True
            for t, b_ in zip(top_p, bot_p):
                c = _fresh()
                c["count"] = t["count"] + b_["count"]
                for f in SUM_FIELDS:
                    c[f] = t[f] + b_[f]
                for f in MAX_FIELDS:
                    c[f] = max(t[f], b_[f])
                comp_per.append(c)
            for whole_ch, comp_ch in zip(per, comp_per):
                u = {}
                for f in SUM_FIELDS:
                    a = int(np.asarray(F64(whole_ch[f])).view(np.uint64))
                    b = int(np.asarray(F64(comp_ch[f])).view(np.uint64))
                    u[f] = abs(a - b)
                    per_exact = per_exact and a == b
                per_ulp.append(u)
            entry["per_channel"] = [partials_json(p, True) for p in per]
            entry["composed_per_channel"] = [partials_json(p, True) for p in comp_per]
            entry["per_channel_composition_is_bit_exact"] = per_exact
            entry["per_channel_composition_ulp"] = per_ulp
        else:
            entry["per_channel"] = [partials_json(p, True) for p in per]
            entry["top_half_per_channel"] = [partials_json(p, True) for p in top_p]
            entry["bottom_half_per_channel"] = [partials_json(p, True) for p in bot_p]
            entry["per_channel_concatenates_under_a_row_split"] = True
        out["axes"][axis] = entry
    return out


def refusal(name: str, why: str, base_rows: int, base_cols: int, base_values,
            cp_rows: int, cp_cols: int, cp_values,
            declared_rows: int | None = None,
            declared_cols: int | None = None) -> dict:
    """An input the reference itself refuses.

    Committing these makes the Rust refusals differential too, rather than
    merely self-consistent: the Rust must refuse the same input, with the same
    ``kind``, and its message must carry the content ``detail`` names.
    """
    base = np.asarray(base_values, dtype=F32).reshape(base_rows, base_cols)
    counterpart = np.asarray(cp_values, dtype=F32).reshape(cp_rows, cp_cols)
    entry = {
        "name": name,
        "why": why,
        "base_rows": base_rows,
        "base_columns": base_cols,
        "counterpart_rows": cp_rows,
        "counterpart_columns": cp_cols,
        "base_bits": f32_bit_list(base),
        "counterpart_bits": f32_bit_list(counterpart),
    }
    rows = declared_rows if declared_rows is not None else base_rows
    cols = declared_cols if declared_cols is not None else base_cols
    if declared_rows is not None or declared_cols is not None:
        entry["declared_rows"] = rows
        entry["declared_columns"] = cols
    try:
        validate(base, counterpart, rows, cols)
    except Refused as exc:
        entry["kind"] = exc.kind
        entry["detail"] = exc.detail
        return entry
    raise SystemExit(f"reference did NOT refuse {name!r}; the golden set is wrong")


# --------------------------------------------------------------------------
# The golden set
# --------------------------------------------------------------------------

# Acceptance criterion 1's hand-computed 3x4 pair.  Every value is a dyadic
# rational with a small exponent, so every square and every partial sum is
# EXACT in binary64 -- which is what makes the case hand-checkable and what
# lets the composition assertion on it be bit-exact rather than toleranced.
#
# It is built to discriminate, not merely to cover:
#   * max|base| = 8 is attained at a NEGATIVE value, so `max w` (= 6) differs;
#   * max|delta| = 1 is attained at a NEGATIVE delta, so `max d` (= 0.5) differs;
#   * the two maxima sit at DIFFERENT positions ((1,0) and (2,1)), so swapping
#     the fields is visible;
#   * the deltas straddle zero and partly cancel, so |Σd| = 2.75 differs from
#     Σ|d| = 3.75;
#   * Σ base² = 137.140625 differs from Σ counterpart² = 143.828125, so reducing
#     the wrong operand is visible.
HAND_BASE = [
    1.0, -2.0, 0.5, 4.0,
    -8.0, 0.25, 3.0, -1.5,
    2.0, -0.75, 6.0, 0.125,
]
HAND_COUNTERPART = [
    1.5, -2.0, 0.0, 4.25,
    -8.0, 0.75, 3.5, -1.5,
    2.0, 0.25, 6.0, 0.625,
]

# A SQUARE, deliberately asymmetric pair.  On a non-square block a transposed
# implementation is caught by the channel COUNT alone; on a square one the count
# matches and only the values can catch it.  This is the fixture that makes the
# orientation acceptance criterion bite.
SQUARE_BASE = [
    1.0, 2.0, 4.0,
    8.0, 16.0, 32.0,
    64.0, -128.0, 0.25,
]
SQUARE_COUNTERPART = [
    1.0, 2.5, 4.0,
    8.5, 16.0, 30.0,
    64.0, -127.5, 0.75,
]

# TASK.md Test Cases row 4's asymmetric 2x5.
ASYM_BASE = [
    1.0, -2.0, 4.0, -8.0, 16.0,
    0.5, 0.25, -0.125, 32.0, -0.0625,
]
ASYM_COUNTERPART = [
    1.25, -2.0, 3.5, -8.0, 16.5,
    0.5, 0.75, -0.125, 31.0, -0.0625,
]


def precision_pair() -> tuple[np.ndarray, np.ndarray]:
    """A 64x64 pair on which an f32 accumulator gives a visibly wrong answer.

    ``base[0] = 1024`` contributes ``2**20`` to ``Σ w²``; the other 4095 values
    contribute ``2**-12`` each.  In ``f32`` the running total sits at ``2**20``
    where ``ulp = 2**-3``, so every ``2**-12`` term rounds away and the sum
    stays exactly ``1048576``.  In ``f64`` the ulp at ``2**20`` is ``2**-32``,
    so every term lands and the total ``2**20 + 4095*2**-12`` is itself exactly
    representable.  The two answers differ by ~1.0, not by an ulp.
    """
    base = np.full(4096, F32(0.015625), dtype=F32)   # 2**-6
    base[0] = F32(1024.0)                            # 2**10
    counterpart = np.zeros(4096, dtype=F32)
    counterpart[0] = F32(1024.0)
    return base, counterpart


def order_sensitive_pair() -> tuple[np.ndarray, np.ndarray]:
    """A 64x64 pair on which the SUMMATION ORDER changes ``Σ |d|``.

    ``base[0] = 2**20``; every other value is ``2**-34``, and the counterpart is
    zero, so ``|d| = base``.  Sequentially, the running total sits at ``2**20``
    where the binary64 ulp is ``2**-32``; each ``2**-34`` term is below the half
    ulp and rounds away, so the specified order gives exactly ``1048576.0``.
    NumPy's pairwise grouping sums the small terms among themselves first, where
    they survive, and the totals differ by hundreds of ulp.

    This case exists because none of the naturally shaped fixtures could tell
    the two orders apart on ``sum_abs_delta`` --- and a golden set that cannot
    tell them apart does not verify
    `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2's fixed-order rule at all.
    """
    base = np.full(4096, F32(2.0) ** F32(-34), dtype=F32)
    base[0] = F32(2.0) ** F32(20)
    return base, np.zeros(4096, dtype=F32)


def build() -> dict:
    lcg_base = lcg(20260805, 4096, -1.0, 1.0)
    lcg_perturb = lcg(19700101, 4096, -0.001, 0.001)
    lcg_counterpart = (lcg_base + lcg_perturb).astype(F32)
    lcg_independent = lcg(31415927, 4096, -1.0, 1.0)

    p_base, p_counterpart = precision_pair()

    cases = [
        case("hand_computed_3x4",
             "Acceptance criterion 1 and TASK.md Test Cases row 1. Dyadic "
             "rationals, so every square and partial sum is exact in binary64 "
             "and the case is checkable by hand. Chosen to DISCRIMINATE: "
             "max|base| sits on a negative value at (1,0), max|delta| on a "
             "negative delta at (2,1), the deltas partly cancel, and the two "
             "blocks have different sums of squares.",
             3, 4, HAND_BASE, HAND_COUNTERPART),
        case("identical_blocks_3x4",
             "TASK.md Test Cases row 2. Counterpart == base, so every delta "
             "field is exactly zero and sum_sq_base is unchanged from the "
             "hand-computed case. Note what this case CANNOT prove: with the "
             "blocks equal, Σ base² and Σ counterpart² coincide, so it cannot "
             "tell the two operands apart. That is why row 1 exists.",
             3, 4, HAND_BASE, HAND_BASE),
        case("counterpart_all_zero_3x4",
             "TASK.md Test Cases row 3. delta == base exactly, so sum_sq_delta "
             "must equal sum_sq_base BIT-FOR-BIT (identical terms, identical "
             "order), sum_abs_delta equals Σ|base|, and max_abs_delta equals "
             "max_abs_base.",
             3, 4, HAND_BASE, [0.0] * 12),
        case("asymmetric_2x5",
             "TASK.md Test Cases row 4 and acceptance criterion 2. Non-square, "
             "so reducing over the wrong axis yields 5 channels where 2 are "
             "expected. The channel COUNT alone catches a full transposition "
             "here; `square_asymmetric_3x3` is the case where it does not.",
             2, 5, ASYM_BASE, ASYM_COUNTERPART),
        case("square_asymmetric_3x3",
             "Acceptance criterion 2, the hard half. SQUARE, so both axes give "
             "3 channels and a transposed implementation still returns a "
             "plausibly shaped answer; only the per-channel VALUES can catch "
             "it, and this block is deliberately not symmetric under "
             "transposition. max|base| = 128 sits on a negative value.",
             3, 3, SQUARE_BASE, SQUARE_COUNTERPART),
        case("f32_accumulator_would_be_wrong_64x64",
             "The precision discriminator. base[0] = 1024 and 4095 values of "
             "2**-6: an f32 accumulator loses every small term and reports "
             "Σ w² = 1048576 exactly, where f64 reports 1048576.999755859375. "
             "TASK.md §Risks: 'f32 accumulation loses precision on large "
             "blocks'. This is the input that would show it.",
             64, 64, p_base, p_counterpart),
        case("lcg_pseudorandom_64x64",
             "A realistic 4096-element pair from an integer LCG, with a second "
             "LCG stream as a +-0.001 perturbation. No value here is dyadic, so "
             "this is the case where summation ORDER matters: the "
             "numpy_pairwise_* discriminators record what a different (pairwise) "
             "grouping gives, which is the divergence "
             "`.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2 forbids by fixing the "
             "order.",
             64, 64, lcg_base, lcg_counterpart),
        case("lcg_independent_counterpart_64x64",
             "The SAME kernel with an unrelated second operand rather than a "
             "perturbation of the first — `DIFF-001`'s checkpoint-diff shape, "
             "and the evidence for acceptance criterion 7 that nothing here is "
             "specialised to quantisation. It is also the case where the DELTA "
             "sums become order-sensitive: with O(1) deltas instead of 1e-3 "
             "ones, sequential and pairwise summation of Σ d² disagree.",
             64, 64, lcg_base, lcg_independent),
        case("summation_order_matters_64x64",
             "The determinism discriminator for Σ|d|. base[0] = 2**20 and 4095 "
             "values of 2**-34 against a zero counterpart: sequentially every "
             "small term falls below the half-ulp of the running total and "
             "rounds away, giving exactly 1048576.0, while a pairwise grouping "
             "sums the small terms among themselves first and keeps them. "
             "Without this case the golden set could not tell the specified "
             "order from any other, and so would not verify §4.2 at all.",
             64, 64, *order_sensitive_pair()),
        case("single_element_1x1",
             "The smallest non-empty block. One channel on either axis, count "
             "1, and every partial equals its single term.",
             1, 1, [-3.5], [1.25]),
        case("single_row_1x4",
             "Rank-degenerate in one direction: 1 row and 4 columns, so "
             "axis=rows gives one channel holding everything and axis=columns "
             "gives four channels of one element each.",
             1, 4, [1.0, -2.0, 0.5, -4.0], [1.5, -2.0, 0.75, -3.0]),
        case("single_column_4x1",
             "The transpose of the case above, which is what makes the pair "
             "useful: the two must not produce the same answer.",
             4, 1, [1.0, -2.0, 0.5, -4.0], [1.5, -2.0, 0.75, -3.0]),
        case("all_zero_blocks_2x3",
             "Both blocks entirely zero. Every partial is +0.0 and nothing "
             "divides by anything -- this kernel emits partials only, so a "
             "zero denominator is the AGGREGATION's problem, not this one.",
             2, 3, [0.0] * 6, [0.0] * 6),
        case("negative_zero_base_2x2",
             "Signed zeros. |-0.0| is +0.0, so max_abs_base must be +0.0 and "
             "never -0.0, and 0.0 - (-0.0) = +0.0 must not appear as a "
             "non-zero delta.",
             2, 2, [-0.0, 0.0, -0.0, 1.0], [0.0, -0.0, 0.0, 1.0]),
    ]

    splits = [
        split_case("hand_computed_3x4_split_at_row_1",
                   "Acceptance criterion 3 on the dyadic fixture, where every "
                   "partial sum is exact and the composition is therefore "
                   "BIT-EXACT: composed sums land on the same bits as the "
                   "undivided sums.",
                   3, 4, HAND_BASE, HAND_COUNTERPART, 1),
        split_case("lcg_pseudorandom_64x64_split_at_row_32",
                   "Acceptance criterion 3 where the arithmetic is NOT exact. "
                   "Regrouping a sequential sum at row 32 is a different "
                   "association, so the composed and undivided sums may differ; "
                   "composition_ulp records the measured distance in bit "
                   "patterns rather than asserting a guessed tolerance.",
                   64, 64, lcg_base, lcg_counterpart, 32),
        split_case("lcg_independent_counterpart_64x64_split_at_row_32",
                   "The same composition with an unrelated second operand, so "
                   "the DELTA sums are O(1) rather than O(1e-3) and their "
                   "composition divergence is measured too. On the perturbed "
                   "pair the delta sums compose bit-exactly, which would have "
                   "left the delta fields' tolerance unmeasured.",
                   64, 64, lcg_base, lcg_independent, 32),
    ]

    refusals = [
        refusal("shape_mismatch_rows",
                "TASK.md Test Cases row 6 and §Error Handling: refused before "
                "any value is read, naming BOTH shapes.",
                2, 3, [1.0] * 6, 3, 3, [1.0] * 9),
        refusal("shape_mismatch_columns",
                "The same refusal on the other dimension, so a check that "
                "compares only the element count cannot pass.",
                2, 6, [1.0] * 12, 3, 4, [1.0] * 12),
        refusal("empty_block_zero_rows",
                "TASK.md Test Cases row 8. An empty reduction has no "
                "meaningful partials, so it is refused rather than reported as "
                "a vacuous zero.",
                0, 4, [], 0, 4, []),
        refusal("empty_block_zero_columns",
                "The other empty shape, so a check on rows alone cannot pass.",
                3, 0, [], 3, 0, []),
        refusal("empty_block_zero_by_zero",
                "Both dimensions zero: the shapes MATCH, so this reaches the "
                "empty check rather than the shape check.",
                0, 0, [], 0, 0, []),
        refusal("nan_in_counterpart",
                "TASK.md Test Cases row 7: refused, naming the position. "
                "`QM-0120` refuses NaN at the source; this is defence in depth.",
                2, 3, [1.0] * 6, 2, 3, [1.0, 1.0, 1.0, 1.0, float("nan"), 1.0]),
        refusal("nan_in_base",
                "The same refusal on the other operand.",
                2, 3, [1.0, 1.0, float("nan"), 1.0, 1.0, 1.0], 2, 3, [1.0] * 6),
        refusal("positive_infinity_in_base",
                "+Inf is refused on the same rule as NaN rather than "
                "propagating into an infinite sum of squares.",
                2, 2, [1.0, float("inf"), 1.0, 1.0], 2, 2, [1.0] * 4),
        refusal("negative_infinity_in_counterpart",
                "-Inf, refused for the same reason. Note it would otherwise "
                "produce +Inf in sum_sq_delta and a finite-looking max.",
                2, 2, [1.0] * 4, 2, 2, [1.0, 1.0, 1.0, float("-inf")]),
        refusal("non_finite_in_base_precedes_one_in_counterpart",
                "Both blocks hold a non-finite value; the refusal must name "
                "the EARLIEST position across both, which is base at index 1, "
                "not counterpart at index 3.",
                2, 2, [1.0, float("nan"), 1.0, 1.0],
                2, 2, [1.0, 1.0, 1.0, float("inf")]),
        refusal("non_finite_in_counterpart_at_the_same_position_as_base",
                "At one position both operands are non-finite; base is named, "
                "because base is checked first at each position.",
                2, 2, [1.0, float("inf"), 1.0, 1.0],
                2, 2, [1.0, float("nan"), 1.0, 1.0]),
        refusal("ragged_value_count",
                "A block whose value count disagrees with its declared shape. "
                "`BlockData::new` rejects this at construction, but the fields "
                "are public, so the kernel checks it again rather than "
                "indexing out of bounds.",
                2, 3, [1.0] * 6, 2, 3, [1.0] * 6,
                declared_rows=2, declared_cols=4),
    ]

    doc = {
        "schema": "quatricmorph/paired-reduction-goldens/v1",
        "requirement": "QUANT-002",
        "task": "QM-0121",
        "generator": "python/reference/paired_reduction_reference.py",
        "spec": (
            ".plan/DIAGNOSTIC_ARCHITECTURE.md §4.1 and §4.2; "
            ".plan/tasks/QM-0121-paired-block-reduction/TASK.md §Data Contracts "
            "and §Error Handling"
        ),
        "note": (
            "f64 bit patterns (0x%016x) and f32 bit patterns (0x%08x) are the "
            "normative fields; decimal fields are for reading and are emitted "
            "only for blocks of 32 values or fewer, plus on the split cases. "
            "Sums accumulate SEQUENTIALLY in flat row-major index order, which "
            "is what §4.2 fixes; `discriminators.numpy_pairwise_*` records what "
            "NumPy's own pairwise grouping gives instead. This file carries no "
            "version strings so that regenerating it must reproduce it "
            "byte-for-byte; the interpreter and NumPy versions are printed to "
            "stderr by the generator and recorded in .plan/evidence/QM-0121.md "
            "with this file's SHA-256."
        ),
        "channel_order": (
            "One pass in flat row-major index order. Element i sits at "
            "(row, column) = divmod(i, columns) and joins channel `row` when "
            "axis = rows and channel `column` when axis = columns."
        ),
        "cases": cases,
        "splits": splits,
        "refusals": refusals,
    }

    # A golden set in which some wrong formula is ruled out by NO case would be
    # exactly the QM-0120 failure: coverage without discrimination. Refuse to
    # emit one.
    keys = set()
    for c in cases:
        keys.update(c["discriminates"].keys())
    unproven = sorted(
        k for k in keys if not any(c["discriminates"][k] for c in cases)
    )
    if unproven:
        raise SystemExit(
            "no case discriminates against: " + ", ".join(unproven) +
            " -- the golden set covers without discriminating, which is the "
            "QM-0120 failure mode"
        )
    doc["discrimination_coverage"] = {
        k: sorted(c["name"] for c in cases if c["discriminates"][k])
        for k in sorted(keys)
    }
    return doc


def main() -> int:
    parser = argparse.ArgumentParser(description="paired block reduction reference")
    parser.add_argument("--emit-goldens", metavar="DIR",
                        help="write paired-reduction-goldens.json into DIR")
    args = parser.parse_args()

    print(f"reference: python {sys.version.split()[0]}, numpy {np.__version__}",
          file=sys.stderr)

    doc = build()
    text = json.dumps(doc, indent=2, sort_keys=False) + "\n"
    digest = hashlib.sha256(text.encode("utf-8")).hexdigest()

    if args.emit_goldens:
        out_dir = pathlib.Path(args.emit_goldens)
        out_dir.mkdir(parents=True, exist_ok=True)
        target = out_dir / "paired-reduction-goldens.json"
        target.write_text(text, encoding="utf-8")
        print(f"wrote {target} ({len(text)} bytes, {len(doc['cases'])} cases, "
              f"{len(doc['splits'])} splits, {len(doc['refusals'])} refusals)",
              file=sys.stderr)
    else:
        sys.stdout.write(text)
    print(f"sha256: {digest}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
