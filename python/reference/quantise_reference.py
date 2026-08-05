#!/usr/bin/env python3
"""Independent NumPy reference for round-to-nearest quantisation simulation.

`QM-0120` / `QUANT-001`.  This script is the **reference**, not a port: it is
written directly from the formula block in `.plan/DIAGNOSTIC_ARCHITECTURE.md`
§3.1 (reproduced verbatim below), and `crates/q-quant` is written to agree with
the goldens this script emits.  `.plan/DIAGNOSTIC_ARCHITECTURE.md` §9 requires
"a committed Python/NumPy script under `python/`, run in CI-equivalent form";
`.plan/TEST_STRATEGY.md` §0 requires that expected values come from an
independent implementation rather than from the code under test.

The spec, verbatim (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1)::

    symmetric:    s = max|g| / (n/2 - 1)
                  q = clamp(round_half_to_even(x / s), -(n/2), n/2 - 1)
                  x̂ = q · s

    asymmetric:   s = (max(g) - min(g)) / (n - 1)
                  z = round_half_to_even(-min(g) / s)
                  q = clamp(round_half_to_even(x / s) + z, 0, n - 1)
                  x̂ = (q - z) · s

Arithmetic discipline
---------------------

Every value the checkpoint holds is `float32`, so every step here is `float32`
and the dtype is **asserted** after each one.  NumPy 2's NEP-50 promotion keeps
`float32 op python_scalar` in `float32`, but one stray `np.float64` would turn a
bit-exact comparison into a toleranced one, so the assertions are load-bearing
rather than decorative.

Two steps are deliberately *not* `float32`:

* `round_half_to_even` is `np.rint`, which is round-half-to-even, on `float32`.
  This is the step the scheme's rounding mode names, and it happens in
  `float32`.
* Forming the integer code index — the `+ z` and the `clamp` — is done in
  `float64` and then `int64`.  `float32 -> float64` is always exact, every
  `int32` zero point is exact in `float64`, and every sum that can land inside
  the code range is below `2**53`, so this step introduces **no rounding of its
  own** wherever the result is in range.  Doing it in `float32` would lose the
  low bits of `rint(x/s) + z` for large zero points and make the two
  implementations disagree on a value neither of them rounded.

Degenerate cases
----------------

`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1 tabulates them; this script implements
the table, plus one **minimal consistent extension** recorded in the evidence:
the table's `max|g| == 0` row exists so that a unit with **zero dynamic range**
never divides by zero.  Under `symmetric` the only such unit is the all-zero
one.  Under `asymmetric` the scale is `(max - min) / (n - 1)`, which is zero for
**any constant** unit, so a rule is needed whenever `max == min`.

The rule is `s = |c|` for a constant `c != 0`, and the tabulated `s = 1` only for
`c == 0`.  §3.1's zero-point and code formulas are left untouched, so the
deviation is exactly one line.  With `s = |c|` the formula gives
`z = rint(-c/|c|) = ∓1` and `x̂ = ±|c| = c` **bit-exactly**, for both signs and
every magnitude -- which is what makes refusing a constant unit unnecessary.

`s = 1` would NOT do that, and the difference is not academic: at `s = 1` the
formula sends a constant `0.5` to `0.0` and a constant `0.823457` to `1.0`, silently,
while `symmetric` reconstructs both exactly.  A constant bias or norm weight is
common, so that is a real wrong number rather than a corner case.  A constant
*subnormal* unit now refuses on §3.1's existing subnormal-scale row, which is also
what `symmetric` already did with it.

Usage
-----

    python3 python/reference/quantise_reference.py --emit-goldens crates/q-quant/tests/goldens/

The emitted JSON carries **no version strings**, so re-running this script must
reproduce the file byte-for-byte (`TASK.md` §Verification Plan, "Manual").  The
interpreter and NumPy versions are printed to stderr instead, and recorded in
`.plan/evidence/QM-0120.md`.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

import numpy as np

F32 = np.float32

# --------------------------------------------------------------------------
# The scheme
# --------------------------------------------------------------------------


class Refused(Exception):
    """The reference refuses this unit.  Mirrors `q_quant::QuantError`."""

    def __init__(self, kind: str, detail: str) -> None:
        super().__init__(f"{kind}: {detail}")
        self.kind = kind
        self.detail = detail


def levels(precision: str) -> int:
    """`n = 2**bits`.  A closed set: int8 and int4 are the whole v1 surface."""
    if precision == "int8":
        return 256
    if precision == "int4":
        return 16
    raise Refused("not_implemented", f"precision {precision!r} is not int8 or int4")


def code_range(precision: str, zero_point: str) -> tuple[int, int]:
    """`(qmin, qmax)` straight from §3.1's two `clamp` calls."""
    n = levels(precision)
    if zero_point == "symmetric":
        return (-(n // 2), n // 2 - 1)
    if zero_point == "asymmetric":
        return (0, n - 1)
    raise Refused("not_implemented", f"zero point {zero_point!r} is unknown")


def round_half_to_even(x: np.ndarray) -> np.ndarray:
    """§3.1's `round_half_to_even`.  `np.rint` is round-half-to-even."""
    assert x.dtype == F32, x.dtype
    out = np.rint(x)
    assert out.dtype == F32, out.dtype
    return out


def _require_finite(values: np.ndarray, unit: str) -> None:
    """§3.1: "A group contains NaN or ±Inf -> refuse the tensor"."""
    if not np.all(np.isfinite(values)):
        bad = int(np.argmin(np.isfinite(values)))
        raise Refused(
            "non_finite",
            f"{unit} holds a non-finite value at index {bad}; "
            "non-finite weights are a finding, not something to quantise",
        )


def derive_params(values: np.ndarray, precision: str, zero_point: str,
                  unit: str = "<unnamed unit>") -> tuple[F32, int]:
    """§3.1's scale and zero-point derivation over one granularity unit."""
    assert values.dtype == F32, values.dtype
    if values.size == 0:
        raise Refused("empty_unit", f"{unit} is empty; there is nothing to quantise")
    _require_finite(values, unit)
    n = levels(precision)

    if zero_point == "symmetric":
        max_abs = np.abs(values).max()
        assert max_abs.dtype == F32, max_abs.dtype
        if max_abs == F32(0.0):
            # Tabulated: all-zero unit -> s = 1, every code 0, no division by zero.
            return F32(1.0), 0
        s = max_abs / F32(n // 2 - 1)
        assert s.dtype == F32, s.dtype
        _require_normal_scale(s, unit)
        return s, 0

    if zero_point == "asymmetric":
        lo = values.min()
        hi = values.max()
        assert lo.dtype == F32 and hi.dtype == F32
        if hi == lo:
            # Zero dynamic range: the extension documented in the module docstring.
            # `s = |c|` rather than `s = 1`, so that a constant unit reconstructs
            # EXACTLY at any magnitude.  With `s = 1` the §3.1 formula gives
            # z = rint(-c) and q = clamp(rint(c) + z, 0, n-1), which sends 0.5 to
            # 0.0 and 0.823457 to 1.0 -- a silent 100% error on a constant tensor.
            # With s = |c| the unchanged formula gives z = rint(-c/|c|) = ∓1 and
            # x̂ = ±|c| = c, bit-exactly, for both signs and every magnitude.
            s = F32(1.0) if lo == F32(0.0) else abs(lo)
            _require_normal_scale(s, unit)
        else:
            # `hi - lo` can overflow f32 to +Inf on a unit that spans the whole
            # range. That is a refusal, not a warning, and `_require_normal_scale`
            # is what raises it -- so the overflow is expected here rather than
            # something to be surprised by at stderr.
            with np.errstate(over="ignore"):
                s = (hi - lo) / F32(n - 1)
            assert s.dtype == F32, s.dtype
            _require_normal_scale(s, unit)
        zf = round_half_to_even(np.asarray(-lo / s, dtype=F32))
        assert zf.dtype == F32, zf.dtype
        z = float(zf)
        if not (-2147483648.0 <= z <= 2147483647.0):
            raise Refused(
                "zero_point_out_of_range",
                f"{unit} needs zero point {z:.0f}, which is outside i32",
            )
        return s, int(z)

    raise Refused("not_implemented", f"zero point {zero_point!r} is unknown")


def _require_normal_scale(s: F32, unit: str) -> None:
    """§3.1: "s underflows to subnormal -> refuse ... never silently produce
    infinities".  `is_normal` in Rust terms: finite, non-zero, not subnormal."""
    v = float(s)
    tiny = float(np.finfo(F32).tiny)  # smallest positive normal f32
    if not np.isfinite(s) or v == 0.0 or abs(v) < tiny:
        raise Refused(
            "scale_not_normal",
            f"{unit} derived scale {v!r}, which is not a normal f32; "
            "quantising it would divide by zero or emit infinities",
        )


def simulate(values: np.ndarray, scale: F32, zero: int,
             precision: str, zero_point: str,
             unit: str = "<unnamed unit>") -> np.ndarray:
    """`x̂ = dequant(quant(x))` for one unit, per §3.1."""
    assert values.dtype == F32, values.dtype
    scale = F32(scale)
    _require_normal_scale(scale, unit)
    if float(scale) <= 0.0:
        raise Refused("scale_not_normal", f"{unit} scale {float(scale)!r} is not positive")
    if zero_point == "symmetric" and zero != 0:
        raise Refused(
            "zero_point_out_of_range",
            f"{unit} is symmetric, so its zero point must be 0, not {zero}",
        )
    _require_finite(values, unit)
    qmin, qmax = code_range(precision, zero_point)

    r = round_half_to_even(values / scale)              # float32 — the named rounding
    idx = np.clip(r.astype(np.float64) + float(zero),   # exact where it matters
                  float(qmin), float(qmax)).astype(np.int64)
    with np.errstate(over="ignore"):
        # `(q - z) · s` can overflow f32 when the scale is close to f32::MAX and
        # the code lands away from zero. §3.1: "never silently produce
        # infinities" -- so this is a refusal, and the overflow is expected here
        # rather than something to be surprised by at stderr.
        out = (idx - np.int64(zero)).astype(F32) * scale  # float32 dequantisation
    assert out.dtype == F32, out.dtype
    if not np.all(np.isfinite(out)):
        bad = int(np.argmin(np.isfinite(out)))
        raise Refused(
            "reconstruction_not_finite",
            f"{unit} reconstructs to a non-finite value at index {bad}: "
            f"code {int(idx[bad])} times scale {float(scale)!r} overflows f32. "
            "Refused rather than emitted",
        )
    return out


def group_extents(length: int, size: int) -> list[tuple[int, int]]:
    """§3.1: "Group size does not divide the axis -> final group is clamped,
    never padded" — the rule `BlockExtent::clamped_to` already applies."""
    if size == 0:
        # A config rejection: §3.1's last error row requires refusal "at config
        # validation, before any arithmetic".
        raise Refused("config_rejected", "group size 0 would produce no groups")
    out = []
    start = 0
    while start < length:
        out.append((start, min(start + size, length)))
        start += size
    return out


# --------------------------------------------------------------------------
# Golden emission
# --------------------------------------------------------------------------


def bits(x) -> str:
    """The normative field: the f32 bit pattern, so no decimal round-trip is
    involved in the comparison."""
    return "0x%08x" % int(np.asarray(F32(x)).view(np.uint32))


def bit_list(a: np.ndarray) -> list[str]:
    return ["0x%08x" % int(v) for v in a.astype(F32).view(np.uint32)]


def dec_list(a: np.ndarray) -> list[float]:
    return [float(v) for v in a.astype(F32)]


def lcg_unit(count: int) -> np.ndarray:
    """A deterministic pseudo-random unit built from an integer LCG.

    Integer-only and version-independent, so regenerating this file cannot
    drift with a NumPy RNG-stream change.  Values land in [-0.5, 0.5).
    """
    state = 20260805
    out = np.empty(count, dtype=F32)
    for i in range(count):
        state = (state * 1103515245 + 12345) % (1 << 31)
        out[i] = F32(state) / F32(1 << 31) - F32(0.5)
    return out


def case(name: str, why: str, values, precision: str, zero_point: str,
         granularity="per_tensor", params=None) -> dict:
    v = np.asarray(values, dtype=F32)
    if params is None:
        scale, zero = derive_params(v, precision, zero_point, name)
        derived_here = True
    else:
        scale, zero = F32(params[0]), int(params[1])
        derived_here = False
    out = simulate(v, scale, zero, precision, zero_point, name)
    entry = {
        "name": name,
        "why": why,
        "config": {
            "precision": precision,
            "granularity": granularity,
            "zero_point": zero_point,
            "round": "nearest_even",
        },
        "params_derived_from_this_unit": derived_here,
        "params": {"scale_bits": bits(scale), "zero": zero},
        "input_bits": bit_list(v),
        "output_bits": bit_list(out),
        # A round trip that reproduced every input bit-for-bit is `exact` in
        # `.plan/DATA_ARCHITECTURE.md` §8's vocabulary; anything else is
        # `quantized` — values present but lossily encoded.
        "round_trip_is_bit_exact": bool(
            np.array_equal(v.view(np.uint32), out.view(np.uint32))
        ),
    }
    if v.size <= 32:
        entry["params"]["scale"] = float(scale)
        entry["input"] = dec_list(v)
        entry["output"] = dec_list(out)
    return entry


def grouped_case(name: str, why: str, values, precision: str, zero_point: str,
                 size: int) -> dict:
    v = np.asarray(values, dtype=F32)
    extents = group_extents(v.size, size)
    out = np.empty_like(v)
    groups = []
    for start, end in extents:
        unit = f"{name}[{start}..{end}]"
        scale, zero = derive_params(v[start:end], precision, zero_point, unit)
        out[start:end] = simulate(v[start:end], scale, zero, precision, zero_point, unit)
        groups.append({
            "start": start,
            "end": end,
            "count": end - start,
            "scale_bits": bits(scale),
            "zero": zero,
        })
    return {
        "name": name,
        "why": why,
        "config": {
            "precision": precision,
            "granularity": {"per_group": size},
            "zero_point": zero_point,
            "round": "nearest_even",
        },
        "group_size": size,
        "groups": groups,
        "input_bits": bit_list(v),
        "output_bits": bit_list(out),
        "round_trip_is_bit_exact": bool(
            np.array_equal(v.view(np.uint32), out.view(np.uint32))
        ),
    }


def refusal(name: str, why: str, values, precision: str, zero_point: str,
            params=None, size=None, derive_then_simulate=False) -> dict:
    """A case the reference itself refuses.  Committing these makes the Rust
    refusals differential too, rather than merely self-consistent.

    `entry_point` names which function refused, so the Rust test drives the same
    one rather than guessing.
    """
    v = np.asarray(values, dtype=F32)
    entry = {
        "name": name,
        "why": why,
        "config": {"precision": precision, "zero_point": zero_point},
        "input_bits": bit_list(v),
    }
    if size is not None:
        entry["entry_point"] = "group_extents"
        entry["group_size"] = size
    elif derive_then_simulate:
        entry["entry_point"] = "derive_then_simulate"
    elif params is None:
        entry["entry_point"] = "derive_params"
    else:
        entry["entry_point"] = "simulate"
        entry["params"] = {"scale_bits": bits(params[0]), "zero": int(params[1])}
    try:
        if size is not None:
            group_extents(v.size, size)
        elif derive_then_simulate:
            s, z = derive_params(v, precision, zero_point, name)
            entry["params"] = {"scale_bits": bits(s), "zero": int(z)}
            simulate(v, s, z, precision, zero_point, name)
        elif params is None:
            derive_params(v, precision, zero_point, name)
        else:
            simulate(v, F32(params[0]), int(params[1]), precision, zero_point, name)
    except Refused as exc:
        entry["kind"] = exc.kind
        entry["detail"] = exc.detail
        return entry
    raise SystemExit(f"reference did NOT refuse {name!r}; the golden set is wrong")


def build() -> dict:
    boundaries = []
    for x in [0.5, 1.5, -0.5, 2.5, -1.5, -2.5, 0.4999999, 3.5]:
        r = round_half_to_even(np.asarray([x], dtype=F32))
        boundaries.append({
            "input_bits": bits(x),
            "input": float(F32(x)),
            "output_bits": bits(r[0]),
            "output": float(r[0]),
        })

    exactly_representable = np.asarray(
        [F32(k) * (F32(4.0) / F32(127.0)) for k in range(-127, 1)], dtype=F32
    )

    cases = [
        case("minus_one_zero_one_int8_symmetric",
             "TASK.md Test Cases row 1. s = max|g|/127 = 1/127; -1 -> -127, 0 -> 0, "
             "1 -> 127; the round trip is exact because each input IS a code times s.",
             [-1.0, 0.0, 1.0], "int8", "symmetric"),
        case("tenths_int4_asymmetric",
             "TASK.md Test Cases row 2. None of 0.1, 0.2, 0.3 is exact in f32, and "
             "that decides the answer: (f32(0.3) - f32(0.1)) rounds UP to a tie-to-"
             "even, so s is slightly larger than 0.2/15 and -f32(0.1)/s is "
             "-7.4999998 rather than -7.5. z is therefore -7, not the -8 that exact "
             "decimal arithmetic would give. The reconstructed minimum then sits "
             "BELOW 0.1 and the reconstructed maximum below 0.3, because the grid "
             "is offset by the rounding of z itself. Correct per §3.1; not an "
             "off-by-one.",
             [0.1, 0.2, 0.3], "int4", "asymmetric"),
        case("all_zero_int8_symmetric",
             "TASK.md Test Cases row 3 and §3.1's first degenerate row: s = 1, every "
             "code 0, output all zero, no division by zero and no NaN.",
             [0.0, 0.0, 0.0], "int8", "symmetric"),
        case("all_zero_int8_asymmetric",
             "The same degenerate row under the asymmetric formula, where the scale "
             "is (max-min)/(n-1) and max == min.",
             [0.0, 0.0, 0.0], "int8", "asymmetric"),
        case("constant_ones_int4_asymmetric",
             "Zero dynamic range, non-zero: an all-ones norm weight. s = |1| = 1, "
             "z = rint(-1/1) = -1, so every value reconstructs bit-exactly. Note "
             "that s = 1 and s = |c| coincide at c = 1, which is exactly why this "
             "case alone cannot verify the rule -- see the three that follow.",
             [1.0] * 8, "int4", "asymmetric"),
        case("constant_half_int4_asymmetric",
             "The case that pins the zero-dynamic-range rule. s = |0.5| = 0.5, "
             "z = rint(-0.5/0.5) = -1, q = clamp(rint(1) + (-1), 0, 15) = 0, "
             "x̂ = (0 - (-1))*0.5 = 0.5 bit-exactly. Under the s = 1 rule this "
             "reconstructs to 0.0 -- a silent 100% error on a constant tensor.",
             [0.5] * 8, "int4", "asymmetric"),
        case("constant_negative_three_tenths_int8_asymmetric",
             "The negative sign of the same rule, and the one that pins z. "
             "s = |-0.3| = f32(0.3), z = rint(0.3/0.3) = +1, "
             "q = clamp(rint(-1) + 1, 0, 255) = 0, x̂ = (0 - 1)*0.3 = -0.3 "
             "bit-exactly. Under the s = 1 rule this also reconstructs to 0.0.",
             [-0.3] * 8, "int8", "asymmetric"),
        case("constant_irrational_int4_asymmetric",
             "A constant with no short binary expansion, at both zero points, to "
             "show the two modes now agree. Under the s = 1 rule this reconstructs "
             "to 1.0 -- a 41% error in the WRONG direction.",
             [0.823457] * 6, "int4", "asymmetric"),
        case("constant_irrational_int4_symmetric",
             "The symmetric counterpart of the case above: s = |0.823457|/7, "
             "q = 7, x̂ = 7*s = 0.823457. Symmetric was always exact here, and the "
             "two modes disagreeing on a constant tensor was the tell.",
             [0.823457] * 6, "int4", "symmetric"),
        case("constant_f32_max_int8_asymmetric",
             "The largest magnitude f32 holds, as a constant unit. s = |c| = "
             "f32::MAX, z = -1, code 0, x̂ = 1*s = f32::MAX bit-exactly -- no "
             "overflow, because the reachable code is 0 rather than 255. The "
             "SYMMETRIC path at the same magnitude does overflow and is refused; "
             "see `reconstruction_overflows_to_infinity_int8_symmetric`.",
             [3.4028235e38] * 4, "int8", "asymmetric"),
        case("constant_negative_f32_max_int4_symmetric",
             "int4 symmetric at -f32::MAX: s = |c|/7 and the reachable code is -7, "
             "so 7*s stays finite where int8's 127*s would not. Bit-exact.",
             [-3.4028235e38] * 4, "int4", "symmetric"),
        case("large_but_valid_zero_point_int8_asymmetric",
             "The only case that exercises the i64 -> f32 conversion in "
             "`(q - z).astype(F32)`. A huge offset with a two-ULP range gives "
             "z = 1687448320, which FITS in i32 (unlike the one-ULP version, which "
             "overflows and is refused), so |q - z| > 2^24 and the conversion "
             "ROUNDS. Every other accepted case has |z| <= 8, where the conversion "
             "is exact. If Rust's `i64 as f32` and NumPy's int64->float32 "
             "disagreed on ties, this is the case that would show it.",
             [-1e30,
              np.nextafter(F32(-1e30), F32(0.0), dtype=F32),
              np.nextafter(np.nextafter(F32(-1e30), F32(0.0), dtype=F32),
                           F32(0.0), dtype=F32)],
             "int8", "asymmetric"),
        case("constant_large_negative_int8_asymmetric",
             "A constant at -4e9, which the s = 1 rule REFUSED because it needed "
             "z = rint(4e9), outside i32. Under s = |c| the zero point is -(-1) = 1 "
             "and the unit reconstructs exactly, so no refusal is needed. The "
             "zero-point-overflow path is still reachable and still tested -- see "
             "the refusal `zero_point_outside_i32_int8_asymmetric`.",
             [-4e9] * 4, "int8", "asymmetric"),
        case("exactly_representable_int8_symmetric",
             "TASK.md Test Cases row 7 and acceptance criterion 7: every input is "
             "already a multiple of s = 4/127, so the round trip is idempotent and "
             "bit-exact.",
             exactly_representable, "int8", "symmetric"),
        case("boundary_halves_scale_one_int8_symmetric",
             "Acceptance criterion 2's rounding boundaries fed through simulate with "
             "s = 1 exactly: 0.5 -> 0, 1.5 -> 2, -0.5 -> -0, 2.5 -> 2. Half-away-"
             "from-zero would give 1, 2, -1, 3 and disagree on three of four.",
             [0.5, 1.5, -0.5, 2.5, -1.5, -2.5], "int8", "symmetric",
             params=(1.0, 0)),
        case("clamping_boundaries_int4_symmetric",
             "Acceptance criterion 2's clamping boundaries: params derived elsewhere "
             "(s = 1) applied to a unit that overshoots the int4 code range, so "
             "+9 clamps to +7 and -12 clamps to -8. This is the per-tensor and "
             "per-channel case -- the unit does not set its own scale.",
             [7.0, 8.0, 9.0, 100.0, -8.0, -9.0, -12.0, 0.0], "int4", "symmetric",
             params=(1.0, 0)),
        case("clamping_boundaries_int4_asymmetric",
             "The asymmetric clamp: q = clamp(rint(x/s) + z, 0, 15) with s = 1, "
             "z = 8, so x = 8 clamps at the top and x = -9 clamps at the bottom.",
             [-9.0, -8.0, -1.0, 0.0, 1.0, 7.0, 8.0, 50.0], "int4", "asymmetric",
             params=(1.0, 8)),
        case("mixed_signs_int8_asymmetric",
             "A realistic asymmetric unit whose range straddles zero, so z lands "
             "inside [0, 255] the way the textbook case does.",
             [-0.75, -0.25, 0.0, 0.125, 0.5, 1.0], "int8", "asymmetric"),
        case("mixed_signs_int4_symmetric",
             "int4 symmetric over the same values: 15 levels of headroom, so the "
             "reconstruction error is visible and the result is `quantized`, never "
             "`exact`.",
             [-0.75, -0.25, 0.0, 0.125, 0.5, 1.0], "int4", "symmetric"),
        case("lcg_4096_int8_symmetric",
             "TASK.md Test Cases row 9: a 4096-element unit, the size q-quant must "
             "handle with allocation proportional to the unit and not to a "
             "4096x4096 tensor.",
             lcg_unit(4096), "int8", "symmetric"),
        case("lcg_4096_int4_asymmetric",
             "The same 4096-element unit at the other extreme of the v1 surface.",
             lcg_unit(4096), "int4", "asymmetric"),
    ]

    grouped = [
        grouped_case("group_130_of_128_int8_symmetric",
                     "TASK.md Test Cases row 6 and acceptance criterion 4: 130 "
                     "values at group size 128 give two groups of 128 and 2. The "
                     "final group is CLAMPED, never padded, so its scale comes "
                     "from its own two values.",
                     lcg_unit(130), "int8", "symmetric", 128),
        grouped_case("group_130_of_128_int4_asymmetric",
                     "The same clamped final group under the asymmetric formula, "
                     "where a 2-element final group has its own zero point too.",
                     lcg_unit(130), "int4", "asymmetric", 128),
        grouped_case("group_9_of_4_int4_symmetric",
                     "A tiny non-dividing case that is legible by hand: 9 values at "
                     "group size 4 give 4 + 4 + 1, and the 1-element final group is "
                     "exactly representable at its own scale.",
                     [-1.0, 0.25, 0.5, 1.0, -4.0, 2.0, 0.0, 1.0, -0.5],
                     "int4", "symmetric", 4),
    ]

    refusals = [
        refusal("subnormal_scale_int8_symmetric",
                "TASK.md Test Cases row 4: max|g| is the smallest f32 subnormal, so "
                "s = max|g|/127 underflows to zero. Refused, naming the unit.",
                [1e-45, 0.0], "int8", "symmetric"),
        refusal("subnormal_scale_int4_asymmetric",
                "The same underflow under the asymmetric formula, where the range "
                "rather than the magnitude is subnormal.",
                [1e-45, 0.0], "int4", "asymmetric"),
        refusal("constant_subnormal_int4_asymmetric",
                "A CONSTANT subnormal unit. Under s = |c| the scale is the "
                "subnormal itself, so §3.1's existing subnormal-scale row refuses "
                "it -- which is what symmetric already did with the same input. "
                "Under the s = 1 rule it silently reconstructed to 0.0.",
                [1e-45] * 4, "int4", "asymmetric"),
        refusal("constant_subnormal_int4_symmetric",
                "The symmetric counterpart, which refuses on the same row: "
                "s = max|g|/7 underflows to zero.",
                [1e-45] * 4, "int4", "symmetric"),
        refusal("nan_in_unit_int8_symmetric",
                "TASK.md Test Cases row 5: refused, and the refusal names the "
                "tensor. A non-finite weight is a finding, reported as one.",
                [1.0, float("nan")], "int8", "symmetric"),
        refusal("positive_infinity_in_unit_int8_asymmetric",
                "+Inf is refused on the same rule as NaN rather than propagating "
                "through max() into an infinite scale.",
                [1.0, float("inf")], "int8", "asymmetric"),
        refusal("negative_infinity_in_unit_int4_symmetric",
                "-Inf, refused for the same reason.",
                [float("-inf"), 1.0], "int4", "symmetric"),
        refusal("reconstruction_overflows_to_infinity_int8_symmetric",
                "A constant unit at f32::MAX under int8 symmetric. The scale is "
                "normal (f32::MAX/127) and every input is finite, but 127*s rounds "
                "UP past f32::MAX, so the reconstruction overflows. §3.1: never "
                "silently produce infinities -- so it is refused, naming the unit "
                "and the offending index. Note this is the RECONSTRUCTION "
                "overflowing, not the scale: `scale_overflows_to_infinity_...` "
                "below is the other case.",
                [3.4028235e38] * 4, "int8", "symmetric",
                derive_then_simulate=True),
        refusal("reconstruction_overflows_to_infinity_negative_int8_symmetric",
                "The same overflow with the sign flipped, so -Inf is refused too "
                "rather than only +Inf.",
                [-3.4028235e38] * 4, "int8", "symmetric",
                derive_then_simulate=True),
        refusal("reconstruction_overflows_from_supplied_params_int4_symmetric",
                "The same refusal reached with params supplied rather than "
                "derived, which is the per-tensor and per-channel case. s = 2e38 "
                "and x = f32::MAX give rint(1.7014) = 2, and 2*s = 4e38 overflows. "
                "A scale of f32::MAX itself would NOT reach this: every finite "
                "input then rounds to code 0 and reconstructs to 0.0.",
                [3.4028235e38, -3.4028235e38], "int4", "symmetric",
                params=(2e38, 0)),
        refusal("scale_overflows_to_infinity_int8_asymmetric",
                "max - min overflows f32 to +Inf, so the scale is not finite. "
                "§3.1: never silently produce infinities.",
                [3.4028235e38, -3.4028235e38], "int8", "asymmetric"),
        refusal("zero_point_outside_i32_int8_asymmetric",
                "A huge offset with a one-ULP range: lo = -1e30 and hi its "
                "neighbour toward zero, so s = ulp(1e30)/255 = 2.96e20 and "
                "z = rint(1e30/s) = 3374896640, outside i32 (max 2147483647). "
                "Refused rather than wrapped. Not a constant unit: constants are "
                "exactly representable under the s = |c| rule and no longer reach "
                "this path.",
                [-1e30, np.nextafter(F32(-1e30), F32(0.0), dtype=F32)],
                "int8", "asymmetric"),
        refusal("empty_unit_int8_symmetric",
                "An empty unit has no max and no range; refused rather than "
                "returning a fabricated scale.",
                [], "int8", "symmetric"),
        refusal("non_finite_reaches_simulate_int8_symmetric",
                "Non-finite values are refused in simulate too, not only in "
                "derive_params -- params may have been derived from another unit.",
                [1.0, float("nan")], "int8", "symmetric", params=(1.0, 0)),
        refusal("subnormal_scale_reaches_simulate_int8_symmetric",
                "A subnormal scale handed to simulate is refused rather than "
                "dividing by something that underflows.",
                [1.0, 2.0], "int8", "symmetric", params=(1e-45, 0)),
        refusal("zero_scale_reaches_simulate_int8_symmetric",
                "Division by a zero scale is refused, not attempted.",
                [1.0, 2.0], "int8", "symmetric", params=(0.0, 0)),
        refusal("symmetric_with_non_zero_zero_point",
                "A symmetric config carries no zero point; a non-zero one is an "
                "inconsistent parameter pair and is refused before any arithmetic.",
                [1.0, 2.0], "int8", "symmetric", params=(1.0, 3)),
        refusal("group_size_zero",
                "Group size 0 would produce no groups; refused at validation.",
                [1.0, 2.0, 3.0], "int8", "symmetric", size=0),
    ]

    return {
        "schema": "quatricmorph/quant-goldens/v1",
        "requirement": "QUANT-001",
        "task": "QM-0120",
        "generator": "python/reference/quantise_reference.py",
        "spec": ".plan/DIAGNOSTIC_ARCHITECTURE.md §3.1",
        "note": (
            "Bit patterns are the normative fields; decimal fields are for "
            "reading and are emitted only for units of 32 values or fewer. This "
            "file carries no version strings so that regenerating it must "
            "reproduce it byte-for-byte; the interpreter and NumPy versions are "
            "printed to stderr by the generator and recorded in "
            ".plan/evidence/QM-0120.md."
        ),
        "round_half_to_even": boundaries,
        "cases": cases,
        "grouped_cases": grouped,
        "refusals": refusals,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emit-goldens", metavar="DIR",
                        help="write quant-goldens.json into DIR")
    args = parser.parse_args()

    print(f"reference: python {sys.version.split()[0]}, numpy {np.__version__}",
          file=sys.stderr)

    doc = build()
    text = json.dumps(doc, indent=2, sort_keys=False) + "\n"

    if args.emit_goldens:
        out_dir = pathlib.Path(args.emit_goldens)
        out_dir.mkdir(parents=True, exist_ok=True)
        target = out_dir / "quant-goldens.json"
        target.write_text(text, encoding="utf-8")
        print(f"wrote {target} ({len(text)} bytes, "
              f"{len(doc['cases'])} cases, {len(doc['grouped_cases'])} grouped, "
              f"{len(doc['refusals'])} refusals)", file=sys.stderr)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
