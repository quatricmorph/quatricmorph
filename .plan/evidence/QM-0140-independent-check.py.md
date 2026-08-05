# QM-0140 — the independent verification script

`.plan/evidence/QM-0140.md` `## Validation evidence` cites this script. It is
recorded here verbatim so a reviewer can re-run it without the implementing
session's scratchpad — the specific failure of the interrupted agent's draft,
which cited a script it had left in a temporary directory.

It is **not** a build or test dependency, and it is deliberately not a `.py`
file in the source tree: `TEST_STRATEGY.md` §1 keeps artifact validation a CI
job, and no `cargo test` may depend on a `pip install`.

## How it was run

```bash
python3 -m venv /tmp/qm-0140 && /tmp/qm-0140/bin/pip install jsonschema  # 4.26.0
/tmp/qm-0140/bin/python independent_check.py /path/to/repo
# 126 ok, 0 FAIL      exit 0
```

## What it establishes

It is written from `.plan/REPORT_ARCHITECTURE.md` §2,
`schemas/diagnostics/README.md`, `schemas/diagnostics/manifest.v1.json` and
`QM-0140`'s `TASK.md`. **It does not read `crates/q-report`.** Three groups:

* **A — byte layout.** CPython's `json` re-emits both goldens byte-for-byte at
  `indent=2` with a trailing newline, so the canonical form is pinned by a
  second implementation rather than by serde alone.
* **B — schema conformance.** `jsonschema` 4.26.0 with `FormatChecker`,
  positively for both goldens and the schema's example, negatively for 24
  mutations. Also records the one thing the schema *cannot* express: a duplicate
  canonical address passes it, and only `Manifest::validate()` refuses.
* **C — arithmetic and ordering.** The aggregation hierarchy, relative error,
  dtype widths, shape products, each frontier step's byte cost and error share,
  the greedy prefix property, all seven ordering rules, and the summary
  projection — all recomputed in Python.

## The script

```python
#!/usr/bin/env python3
"""QM-0140 — independent verification of the diagnostics manifest goldens.

Written from the *written contract* only:
  .plan/REPORT_ARCHITECTURE.md §2, schemas/diagnostics/README.md (the ordering,
  floating-point and rank tables), schemas/diagnostics/manifest.v1.json, and
  QM-0140's TASK.md. It does not import, link, or read crates/q-report.

Three independent things are established here:

  A. Byte layout. CPython's `json` re-emits each golden byte-for-byte with
     indent=2 plus a trailing newline. That pins the canonical form to a
     second implementation, so `the_golden_manifest_round_trips_byte_identically`
     compares against an externally-fixed layout rather than against whatever
     serde happened to print.
  B. Schema conformance, via jsonschema 4.26.0 (third party, draft-07,
     FormatChecker on) rather than the hand-rolled validator in the crate's
     test file. Positive and negative.
  C. Arithmetic and ordering, recomputed in Python from the contract: the
     aggregation hierarchy, relative error, the byte cost and error-removed
     fraction of every frontier step, the greedy step order, dtype widths,
     shape products, and all seven ordering rules.

Usage:  <venv>/bin/python independent_check.py <repo-root>
Exit 0 iff every check passes.
"""

import copy
import json
import math
import sys
from pathlib import Path

from jsonschema import Draft7Validator, FormatChecker

OK = 0
FAIL = 0


def check(name, condition, detail=""):
    global OK, FAIL
    if condition:
        OK += 1
    else:
        FAIL += 1
        print(f"FAIL  {name}  {detail}")


def main(root: Path) -> int:
    schema_path = root / "schemas" / "diagnostics" / "manifest.v1.json"
    golden_dir = root / "crates" / "q-report" / "tests" / "golden"
    full_path = golden_dir / "manifest.v1.json"
    summary_path = golden_dir / "manifest.v1.summary.json"

    schema = json.loads(schema_path.read_text())
    full_text = full_path.read_text()
    summary_text = summary_path.read_text()
    full = json.loads(full_text)
    summary = json.loads(summary_text)

    # ---------------------------------------------------------------- A. bytes
    # REPORT_ARCHITECTURE.md §2.1 / README "Floating point": a fixed
    # representation, the same digits every time. If CPython's json and the
    # crate's serializer agree byte-for-byte, the layout is not one
    # implementation's private habit.
    for label, text, value in (
        ("manifest.v1.json", full_text, full),
        ("manifest.v1.summary.json", summary_text, summary),
    ):
        reemitted = json.dumps(value, indent=2, ensure_ascii=False) + "\n"
        check(
            f"A/{label}: CPython json re-emits it byte-for-byte",
            reemitted == text,
            f"{len(reemitted)} vs {len(text)} bytes",
        )

    check(
        "A: 0.1 + 0.2 needs all 17 digits in this language too",
        repr(0.1 + 0.2) == "0.30000000000000004",
    )

    # --------------------------------------------------------------- B. schema
    Draft7Validator.check_schema(schema)
    check("B: the schema is a valid draft-07 schema", True)
    check(
        "B: $schema declares draft-07",
        schema["$schema"] == "http://json-schema.org/draft-07/schema#",
    )
    check("B: $id carries an explicit version (SCHEMA-001)", schema["$id"].endswith("/v1"))

    validator = Draft7Validator(schema, format_checker=FormatChecker())

    def errors(doc):
        return sorted(validator.iter_errors(doc), key=lambda e: list(e.absolute_path))

    check("B: the full golden validates", errors(full) == [], errors(full)[:1])
    check("B: the summary golden validates", errors(summary) == [], errors(summary)[:1])
    for i, example in enumerate(schema.get("examples", [])):
        check(f"B: schema examples[{i}] validates", errors(example) == [], errors(example)[:1])
    check("B: the schema carries at least one example", len(schema.get("examples", [])) >= 1)

    # An empty run is valid when refusals explain it (TASK.md Error Handling).
    empty = schema["examples"][0]
    check("B: the empty run is the documented example", empty["tensors"] == [])
    check("B: the empty run carries refusals", len(empty["refusals"]) >= 1)

    # Negative paths, asserted against the schema file by a third-party
    # validator — so the constraint lives in the artifact, not only in Rust.
    def mutate(fn):
        doc = copy.deepcopy(full)
        fn(doc)
        return doc

    negatives = [
        ("future manifest_version", lambda d: d.__setitem__("manifest_version", 2)),
        ("manifest_version 0", lambda d: d.__setitem__("manifest_version", 0)),
        ("unknown top-level member", lambda d: d.__setitem__("zz_future", 1)),
        ("unknown member inside run", lambda d: d["run"].__setitem__("gpu_name", "RTX 3090")),
        ("missing run.backend", lambda d: d["run"].pop("backend")),
        ("a backend that never ran", lambda d: d["run"].__setitem__("backend", "cuda")),
        ("missing refusals", lambda d: d.pop("refusals")),
        ("refusal without requirement_id", lambda d: d["refusals"][0].pop("requirement_id")),
        ("summary that still carries tensors", lambda d: d.__setitem__("projection", "summary")),
        ("full projection without tensors", lambda d: d.pop("tensors")),
        ("rank-4 shape", lambda d: d["tensors"][0].__setitem__("shape", [32, 4, 128, 128])),
        ("unknown dtype", lambda d: d["tensors"][0].__setitem__("dtype", "F4_SECRET")),
        ("unknown role", lambda d: d["tensors"][0].__setitem__("role", "attention_query_projeciton")),
        ("unmeasured peak residency", lambda d: d["run"].__setitem__("peak_resident_bytes", 0)),
        ("blank revision_hash", lambda d: d["model"].__setitem__("revision_hash", "")),
        ("dropped not-proven-optimal claim", lambda d: d["frontier"].__setitem__("claim", "Optimal.")),
        ("malformed started_at", lambda d: d["run"].__setitem__("started_at", "yesterday")),
        ("per_group without group_size", lambda d: d["config"]["granularity"].pop("group_size")),
        ("error_removed_fraction above one", lambda d: d["frontier"]["steps"][0].__setitem__("error_removed_fraction", 1.5)),
        ("negative sum of squares", lambda d: d["totals"].__setitem__("sum_sq_delta", -1.0)),
        ("empty keep_set", lambda d: d["frontier"]["steps"][0].__setitem__("keep_set", [])),
        ("negative elapsed_seconds", lambda d: d["run"].__setitem__("elapsed_seconds", -1.0)),
        ("group_size on a non-group granularity", lambda d: d["config"]["granularity"].update({"kind": "per_tensor"})),
        ("outlier share above one", lambda d: d["tensors"][0]["outlier_attribution"].__setitem__("top_1_percent_share", 1.5)),
    ]
    for label, fn in negatives:
        check(f"B/negative: the schema refuses {label}", errors(mutate(fn)) != [])

    # The schema cannot express address uniqueness in the asserted subset.
    dup = copy.deepcopy(full)
    dup["tensors"].append(copy.deepcopy(dup["tensors"][0]))
    check(
        "B/limit: a duplicate address passes the schema (Rust `validate` catches it)",
        errors(dup) == [],
    )

    # ------------------------------------------------- C. arithmetic, ordering
    tensors = full["tensors"]
    addresses = [t["address"] for t in tensors]

    check("C/ordering: tensors ascend by canonical address", addresses == sorted(addresses))
    check("C: addresses are unique (SRC-006)", len(set(addresses)) == len(addresses))
    check(
        "C/ordering: layers ascend by layer_index",
        [l["layer_index"] for l in full["layers"]] == sorted(l["layer_index"] for l in full["layers"]),
    )
    check(
        "C/ordering: experts ascend by (layer_index, expert_index)",
        [(e["layer_index"], e["expert_index"]) for e in full["experts"]]
        == sorted((e["layer_index"], e["expert_index"]) for e in full["experts"]),
    )
    check(
        "C/ordering: ranking is (relative_error desc, parameter_count desc, address asc)",
        [(r["address"]) for r in full["ranking"]]
        == [
            r["address"]
            for r in sorted(
                full["ranking"],
                key=lambda r: (-r["relative_error"], -r["parameter_count"], r["address"]),
            )
        ],
    )
    check(
        "C/ordering: frontier steps ascend by cumulative added_bytes",
        [s["added_bytes"] for s in full["frontier"]["steps"]]
        == sorted(s["added_bytes"] for s in full["frontier"]["steps"]),
    )
    for i, step in enumerate(full["frontier"]["steps"]):
        check(f"C/ordering: keep_set[{i}] ascends", step["keep_set"] == sorted(step["keep_set"]))
    check(
        "C/ordering: refusals ascend by (requirement_id, what, why)",
        [(r["requirement_id"], r["what"], r["why"]) for r in full["refusals"]]
        == sorted((r["requirement_id"], r["what"], r["why"]) for r in full["refusals"]),
    )

    ADDITIVE = [
        "count",
        "sum_sq_base",
        "sum_sq_delta",
        "sum_abs_delta",
        "bytes_at_base_precision",
        "bytes_at_target_precision",
    ]

    def combine(entries):
        out = {k: sum(e[k] for e in entries) for k in ADDITIVE}
        out["max_abs_delta"] = max(e["max_abs_delta"] for e in entries)
        return out

    # DIAGNOSTIC_ARCHITECTURE.md §4.1: partials compose; the hierarchy must add up.
    whole = combine([t["aggregate"] for t in tensors])
    for k in ADDITIVE + ["max_abs_delta"]:
        check(
            f"C/hierarchy: totals.{k} is the reduction over tensors",
            whole[k] == full["totals"][k],
            f"{whole[k]} vs {full['totals'][k]}",
        )

    # Layer membership is read off the canonical address, not off the crate.
    def layer_of(address):
        return int(address.split("model.layers[", 1)[1].split("]", 1)[0])

    for layer in full["layers"]:
        members = [t["aggregate"] for t in tensors if layer_of(t["address"]) == layer["layer_index"]]
        check(f"C/hierarchy: layer {layer['layer_index']} has members", members != [])
        reduced = combine(members)
        for k in ADDITIVE + ["max_abs_delta"]:
            check(
                f"C/hierarchy: layers[{layer['layer_index']}].{k}",
                reduced[k] == layer["aggregate"][k],
                f"{reduced[k]} vs {layer['aggregate'][k]}",
            )

    for expert in full["experts"]:
        marker = f"model.layers[{expert['layer_index']}].experts[{expert['expert_index']}]"
        members = [t["aggregate"] for t in tensors if t["address"].startswith(marker)]
        check(f"C/hierarchy: expert {marker} has members", members != [])
        reduced = combine(members)
        for k in ADDITIVE + ["max_abs_delta"]:
            check(f"C/hierarchy: experts{marker}.{k}", reduced[k] == expert["aggregate"][k])

    WIDTH = {
        "BOOL": 1, "U8": 1, "I8": 1, "F8_E4M3": 1, "F8_E5M2": 1,
        "I16": 2, "U16": 2, "F16": 2, "BF16": 2,
        "I32": 4, "U32": 4, "F32": 4,
        "I64": 8, "U64": 8, "F64": 8,
    }
    TARGET_BITS = {"int8": 8, "int4": 4}
    target_bits = TARGET_BITS[full["config"]["precision"]]

    for t in tensors:
        count = t["aggregate"]["count"]
        check(
            f"C/shape: prod(shape) == count for {t['address']}",
            math.prod(t["shape"]) == count,
        )
        check(f"C/rank: rank <= 3 for {t['address']} (ADR-010)", len(t["shape"]) <= 3)
        check(
            f"C/bytes: base precision for {t['address']}",
            t["aggregate"]["bytes_at_base_precision"] == count * WIDTH[t["dtype"]],
        )
        check(
            f"C/bytes: target precision for {t['address']}",
            t["aggregate"]["bytes_at_target_precision"] == count * target_bits // 8,
        )

    by_address = {t["address"]: t for t in tensors}
    for r in full["ranking"]:
        a = by_address[r["address"]]["aggregate"]
        expected = math.sqrt(a["sum_sq_delta"] / a["sum_sq_base"])
        check(
            f"C/metric: relative_error == sqrt(sum_sq_delta/sum_sq_base) for {r['address']}",
            expected == r["relative_error"],
            f"{expected} vs {r['relative_error']}",
        )
        check(
            f"C/metric: parameter_count == count for {r['address']}",
            r["parameter_count"] == a["count"],
        )
        check(f"C/xref: {r['address']} was examined", r["address"] in by_address)

    total_delta = full["totals"]["sum_sq_delta"]
    for i, step in enumerate(full["frontier"]["steps"]):
        kept = [by_address[a]["aggregate"] for a in step["keep_set"]]
        for a in step["keep_set"]:
            check(f"C/xref: frontier step {i} keeps an examined tensor {a}", a in by_address)
        cost = sum(k["bytes_at_base_precision"] - k["bytes_at_target_precision"] for k in kept)
        check(
            f"C/frontier: added_bytes of step {i} is the keep set's cost",
            cost == step["added_bytes"],
            f"{cost} vs {step['added_bytes']}",
        )
        removed = sum(k["sum_sq_delta"] for k in kept) / total_delta
        check(
            f"C/frontier: error_removed_fraction of step {i}",
            removed == step["error_removed_fraction"],
            f"{removed} vs {step['error_removed_fraction']}",
        )
        check(f"C/frontier: step {i} keeps something", len(step["keep_set"]) >= 1)

    # frontier.method is `greedy_error_per_byte`: steps must be the prefixes of
    # the descending error-per-byte order, which is what makes the claim true.
    def density(address):
        a = by_address[address]["aggregate"]
        return a["sum_sq_delta"] / (a["bytes_at_base_precision"] - a["bytes_at_target_precision"])

    greedy = sorted(by_address, key=lambda a: (-density(a), a))
    for i, step in enumerate(full["frontier"]["steps"]):
        check(
            f"C/frontier: step {i} is the greedy prefix of length {i + 1}",
            sorted(greedy[: i + 1]) == step["keep_set"],
            f"{sorted(greedy[: i + 1])} vs {step['keep_set']}",
        )
    check(
        "C/frontier: the claim is the required wording",
        full["frontier"]["claim"] == "Greedy over error-per-byte; not proven optimal.",
    )

    # A refused tensor is why parameter_count may exceed the sum over `tensors`.
    check(
        "C: model.parameter_count >= sum over tensors",
        full["model"]["parameter_count"] >= full["totals"]["count"],
    )
    ids = {r["requirement_id"] for r in full["refusals"]}
    check("C: refusals carry EVAL-001 (accuracy estimate)", "EVAL-001" in ids)
    check("C: refusals carry GRID-007 (rank above the ceiling)", "GRID-007" in ids)
    check(
        "C: the rank-4 tensor appears only as a refusal, never reshaped",
        all("router" not in a for a in addresses),
    )

    # -------------------------------------------------- summary is a projection
    projected = {k: v for k, v in full.items() if k != "tensors"}
    projected["projection"] = "summary"
    check(
        "C/summary: it is the full manifest minus `tensors`, with the discriminator flipped",
        projected == summary,
    )
    check("C/summary: `tensors` is absent", "tensors" not in summary)
    check(
        "C/summary: key order is preserved",
        list(summary.keys()) == [k for k in full.keys() if k != "tensors"],
    )
    check(
        "C/summary: refusals survive the projection",
        summary["refusals"] == full["refusals"],
    )

    print(f"{OK} ok, {FAIL} FAIL")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1] if len(sys.argv) > 1 else ".")))
```
