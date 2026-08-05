# QM-0126 — independent review

**VERDICT: APPROVE** (qualified — one blocking-severity documentation defect is
downgraded to non-blocking because it is arithmetic in `STATUS.md`'s own summary
rather than a false capability claim; see §D1. Two preconditions are placed on
`QM-0127`, §C5.)

Reviewer: independent review agent. Did not implement this task.
Branch `task/qm-0126-metal-backend` @ `945a7d4`, base `main` @ `39b3aa2`.
Worktree `/Users/thanh/Quatricmorph/.qm-worktrees/r4-qm0126`. Read-only: the only
file this review writes is this one.

---

## A. The central question: did a Metal kernel really execute?

**Yes. Established independently, by hand arithmetic and by reading every path
that could have produced the numbers.**

### A1. Hand-computation of the 3×4 fixture

The fixture is `crates/q-gpu/src/metal.rs:529-554`. Computed by this reviewer
before running anything, from the literal values in the source:

```
base                          counterpart                   delta = base − counterpart
 1.00  -2.00   3.00  -4.00     1.50  -1.00   3.00  -3.50     -0.50  -1.00   0.00  -0.50
 0.50   1.50  -2.50   4.50     0.25   2.00  -2.00   4.00      0.25  -0.50  -0.50   0.50
-1.25   2.25   0.00   3.75    -1.00   2.50   0.50   3.00     -0.25  -0.25  -0.50   0.75
```

| quantity | row 0 | row 1 | row 2 | whole block |
| --- | --- | --- | --- | --- |
| `sum_sq_base`  | 1+4+9+16 = **30**                         | 0.25+2.25+6.25+20.25 = **29**    | 1.5625+5.0625+0+14.0625 = **20.6875** | 30+29+20.6875 = **79.6875** |
| `sum_sq_delta` | 0.25+1+0+0.25 = **1.5**                   | 0.0625+0.25+0.25+0.25 = **0.8125** | 0.0625+0.0625+0.25+0.5625 = **0.9375** | **3.25** |
| `sum_abs_delta`| 0.5+1+0+0.5 = **2.0**                     | 0.25+0.5+0.5+0.5 = **1.75**      | 0.25+0.25+0.5+0.75 = **1.75**         | **5.5** |
| `max_abs_delta`| **1.0**                                   | **0.5**                          | **0.75**                              | **1.0** |
| `max_abs_base` | **4.0**                                   | **4.5**                          | **3.75**                              | **4.5** |
| `count`        | 4                                         | 4                                | 4                                     | **12** |

Every one of these eighteen figures matches the device output I re-ran (§B3),
field for field, including the per-channel breakdown. The implementer's reported
`count 12, sum_sq_base 79.6875, sum_sq_delta 3.25, sum_abs_delta 5.5,
max_abs_delta 1.0, max_abs_base 4.5`, 3 channels of count 4 and 4 of count 3, is
**confirmed**.

### A2. Why correct numbers prove execution here, and not merely correct arithmetic

Hand-checking the numbers is necessary but not sufficient — a stub could have
computed them on the host. I traced every path that writes the returned values:

* `MetalBackend::paired_block_reduction` (`metal.rs:438-494`) obtains all figures
  from `self.dispatch(...)` at `:462` and `:475`. There is no host-side branch,
  no `CpuBackend` delegation, no cached constant.
* `MetalBackend::dispatch` (`metal.rs:297-348`) allocates `out = vec![0f32; …]`
  at `:313` and passes it to the FFI. It performs no arithmetic.
* `widen` (`metal.rs:353-362`) is five `as f64` casts. No arithmetic.
* In the shim, `out` is written by exactly one statement:
  `memcpy(out, [out_buffer contents], output_bytes)` at
  `gpu/metal/qm_metal_shim.m:252`, reached only after
  `[command_buffer status] == MTLCommandBufferStatusCompleted` (`:242`).
  `out_buffer` comes from `newBufferWithLength` (`:216`), which is not seeded
  with anything.

Therefore the only way `79.6875` reaches the caller is
`qm_paired_channel_reduction` having run on the device and written the buffer.
Had it not run, the vector would be zeros and the assertions would fail.
Corroborating: the pipeline could not be created at all unless the embedded
metallib contains a kernel literally named `qm_paired_channel_reduction`
(`qm_metal_shim.m:103-109`), and the device figures reported (§B3) are real M3
Pro values queried through `MTLDevice`, not constants — `maxBufferLength =
22613000192`, `recommendedMaxWorkingSetSize = 30150672384`.

**This is not the `gpu/cuda/*.cu` situation.** The kernel is compiled, linked,
dispatched, and its output is load-bearing for an assertion.

---

## B. Gates I re-ran myself

All commands run in `/Users/thanh/Quatricmorph/.qm-worktrees/r4-qm0126`.

| gate | exit | result |
| --- | --- | --- |
| `cargo build --workspace --all-targets` (feature OFF) | 0 | Finished |
| `cargo fmt --all -- --check` | 0 | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | clean |
| `cargo test --workspace` (feature OFF) | 0 | **745 passed; 0 failed; 0 ignored** over **54** binaries |
| `cargo build -p q-gpu --features metal` | 0 | Finished |
| `cargo test -p q-gpu --features metal` | 0 | **55 passed; 0 failed** (43 + 1 + 11 across three binaries) |
| `cargo clippy -p q-gpu --features metal --all-targets -- -D warnings` | 0 | clean |
| `./scripts/verify-baseline.sh` | 0 | `verify-baseline: OK`, elapsed 14s |
| `./scripts/license-audit.sh` | 0 | `license-audit: all checks passed` |

### B1. Default-feature test tally, derived not quoted

```
grep -E "^test result" <log> | awk '{p+=$4; f+=$6; n++} END {print n,p,f}'
→ binaries: 54   passed: 745   failed: 0
```

### B2. `verify-baseline.sh` decisive lines

```
ok    rust tests: measured 745, floor 745 — at floor
ok    rust test binaries: measured 54, floor 54 — at floor
ok    web tests: measured 336, floor 336 — at floor
ok    web test files: measured 21, floor 21 — at floor
```
All 14 CLI golden checks `ok`. Nothing regressed.

### B3. Real device output, re-run under `--nocapture`

```
QM-0126 device: Apple M3 Pro | unified=true | recommendedMaxWorkingSetSize=30150672384 B |
  maxBufferLength=22613000192 B | maxTotalThreadsPerThreadgroup=1024 | staging budget=268435456 B

QM-0126 device output, ChannelAxis::Rows: PairedPartials {
    count: 12, sum_sq_base: 79.6875, sum_sq_delta: 3.25,
    sum_abs_delta: 5.5, max_abs_delta: 1.0, max_abs_base: 4.5,
    per_channel: [
        { count: 4, sum_sq_base: 30.0,    sum_sq_delta: 1.5,    sum_abs_delta: 2.0,  max_abs_delta: 1.0,  max_abs_base: 4.0  },
        { count: 4, sum_sq_base: 29.0,    sum_sq_delta: 0.8125, sum_abs_delta: 1.75, max_abs_delta: 0.5,  max_abs_base: 4.5  },
        { count: 4, sum_sq_base: 20.6875, sum_sq_delta: 0.9375, sum_abs_delta: 1.75, max_abs_delta: 0.75, max_abs_base: 3.75 },
    ],
}
```
All nine `metal::tests::*` ran; none took a skip branch.

---

## C. Adjudication of items 1–8

### C1. `hardware_verified` / `capabilities().verified` must be `false` — **PASS**

`hardware_verified: false` is a **literal** at `crates/q-gpu/src/metal.rs:397`,
and it is the only occurrence anywhere in the Metal path
(`grep -rn hardware_verified crates/q-gpu/src crates/q-cuda/src`: the sole `true`
is `crates/q-gpu/src/lib.rs:190`, `CpuBackend`, pre-existing and unchanged).
There is no field, constructor argument, feature flag, or environment read that
could flip it. `caveat_requirement: Some("GPU-003")` likewise literal at `:398`.

`the_backend_never_claims_hardware_verification` (`metal.rs:556-579`) asserts it
on the deviceless instance unconditionally and on the probed real-device instance
inside `if let Some(backend) = device_or_skip(...)`. I observed that test run on
the M3 Pro (§B3 shows the device branch was taken in the same run), so **both**
assertions genuinely executed. It additionally asserts `display_name` contains
`"UNVERIFIED"`. **No path can report true.**

### C2. AC1 — feature-OFF workspace build protects CI — **PASS**

* `cargo build --workspace --all-targets` exits 0 (§B).
* `crates/q-gpu/build.rs:64-66` returns before any `xcrun`, `clang` or probe when
  `CARGO_FEATURE_METAL` is unset. It correctly uses `CARGO_FEATURE_*` rather than
  `cfg!(feature=…)`, which would be wrong in a build script.
* `metal = []` in `crates/q-gpu/Cargo.toml:32`, `default = []` at `:12`.
* No workspace member enables it: `q-cli`, `q-cuda` and `tests/` all depend on
  `q-gpu = { workspace = true }` with no `features` key, and the root
  `Cargo.toml:49` declares `q-gpu = { path = "crates/q-gpu" }` bare.
* `.github/workflows/build.yaml` — I read it in full. Four jobs, all
  `ubuntu-latest`. The Rust job runs `cargo fmt`, `cargo clippy --workspace
  --all-targets`, `cargo build --workspace --all-targets`, `cargo test
  --workspace`, `./scripts/verify-baseline.sh`. **No `--features` and no
  `--all-features` appears anywhere in the file** (`grep -n features
  .github/workflows/` returns nothing). The implementer's claim holds.

Note for the record, not a defect: `cargo test --all-features` on Linux *would*
now panic from `build.rs:70-77` with a named message. That is deliberate and
correct behaviour, it is documented in the panic text itself, and no CI job or
repo script invokes `--all-features`.

### C3. `CpuBackend` remains the default — **PASS**

`default_backend()` (`crates/q-gpu/src/lib.rs:279-281`) returns the concrete type
`CpuBackend`, not a `Box<dyn Backend>` — so there is no dynamic dispatch site
where selection could be introduced without changing the signature. It is
unconditional; there is no `#[cfg(feature = "metal")]` variant.

Stronger evidence: `grep -rn "MetalBackend\|default_backend\|q_gpu::metal" crates
tests apps | grep -v crates/q-gpu/` returns **nothing**. Metal is not reachable
from any other crate at all; the module is `#[cfg(feature = "metal")] pub mod
metal;` (`lib.rs:38-39`) and simply does not exist in a default build.
`the_default_backend_is_the_cpu_reference_whatever_features_are_enabled`
(`lib.rs:317-325`) compiles and passes in both configurations — I saw it pass in
the feature-on run (§B3 listing).

### C4. Staging budget enforced before dispatch, counting both blocks — **PASS**

Order of operations in `paired_block_reduction` (`metal.rs:438-452`):
`validate_pair` `:444` → **`check_workload` `:445`** → `require_dense` `:446-447`
→ `require_finite` `:448-449` → `self.device.is_none()` refusal `:450-452` →
first `dispatch` `:462`. The budget check is the second statement in the
function and precedes both the device check and any buffer allocation. No Metal
API is touched before it. `the_budget_refusal_precedes_any_dispatch`
(`metal.rs:618-632`) proves this positively: a 16-byte declared budget produces
`BudgetExceeded`, not the no-device error that would otherwise fire first.

The doubling is real and I checked the arithmetic independently.
`Workload::for_paired_blocks` (`crates/q-gpu/src/lib.rs:92-99`) is
`rows × columns × 2`, `bytes_per_element: 4` — pre-existing from QM-0121 and
**unmodified** by this branch.

```
4096 × 8192 × 2 × 4 = 268 435 456  = exactly 256 MiB   → accepted
4096 × 8193 × 2 × 4 = 268 468 224  > 268 435 456        → refused
one block alone:  4096 × 8193 × 4 = 134 234 112  < 268 435 456  → would have fit
```

The refusal therefore fires **only because both blocks are counted**, exactly as
claimed. `budget_name: "metal_device_staging"` is named in the error
(`metal.rs:411`) and asserted at `metal.rs:606`. The test at `metal.rs:609-612`
explicitly asserts `requested > 4096*8193*4`, which is the assertion that breaks
if the doubling is ever dropped. Confirmed.

Minor, disclosed: the budget counts the two input buffers, not the output buffer
(`channel_count × 5 × 4` bytes; ≤ 163 840 B for the largest accepted workload).
The doc comment at `metal.rs:53-56` states precisely what is counted, so this is
disclosure-complete rather than an overclaim. See §D3.

### C5. Reduction order fixed, documented, no atomics — **PASS, with a named residual risk**

The prose (`gpu/metal/paired_reduction.metal:17-40`) and the kernel agree. I
checked line by line rather than taking the header's word:

| documented | in the kernel |
| --- | --- |
| one threadgroup per channel, no cross-threadgroup accumulation | `origin = group * params.channel_stride` (`:104`); each group writes only `out[group*5 .. +5]` (`:135-141`) |
| no atomics anywhere | confirmed — `grep -i atomic gpu/metal/paired_reduction.metal` returns nothing |
| thread `t` visits `t, t+256, t+512, …` in strictly increasing order | `for (uint i = tid; i < params.elements_per_channel; i += QM_THREADS)` (`:105`) |
| fixed 256-lane binary tree, `stride = 128 → 1`, `lane[t] += lane[t+stride]` | `:124-133`, verbatim |
| host must dispatch 256 threads/threadgroup via `dispatchThreadgroups:threadsPerThreadgroup:` | `qm_metal_shim.m:236-237`, exactly that selector, `MTLSizeMake(QM_THREADS,1,1)` |
| refuses a device below 256 rather than shrinking | `qm_metal_shim.m:195-204`, returns `QM_METAL_THREADGROUP_TOO_SMALL` |

Barrier placement is correct: the barrier sits *outside* the `if (tid < stride)`
(`:126`) so every thread in the group reaches every barrier — the hang the
comment warns about is genuinely avoided, and there is no read-before-barrier
race. **The documented order is the implemented order.** This is not a fiction.

**Residual risk — the order is true but not pinned.** `build.rs:96-97` compiles
with only `-O2 -std=metal3.0`. I determined empirically what that implies:

```
$ xcrun -sdk macosx metal -O2 -std=metal3.0 -S -o d.air paired_reduction.metal
$ xcrun -sdk macosx metal -O2 -std=metal3.0 -fmetal-math-mode=fast -S -o ff.air …
$ diff (stripped of comments) → identical            # default == fast
$ …-fmetal-math-mode=relaxed → differs               # default != relaxed
$ …-fmetal-math-mode=safe    → differs;  24 `fast` flags vs 9
```

So the shader is compiled under **full fast-math**: every `fadd`/`fmul` in the
AIR carries LLVM's `fast` flag (`reassoc | nnan | ninf | arcp | contract | afn`).
The compiler is *licensed* to reassociate the stripe accumulation.

Crucially, **it did not**. I read the emitted AIR: block `%28` is a single serial
`phi` chain per metric (`%29`–`%33`), stepping `%52 = add i32 %34, 256`, with
`%46 = fadd fast float %45, %29` — one accumulator, not split, not unrolled. The
tree at `%56`–`%82` is the documented `stride` loop with `air.wg.barrier`.
**Today's binary is faithful to the documented order**, which is why this is a
residual risk and not a defect in what merges.

Two preconditions I place on `QM-0127`, which must not set a tolerance against
the stated order until they are resolved:

1. **Pin the math mode.** Add `-fmetal-math-mode=safe` (or re-verify the AIR per
   toolchain) in `crates/q-gpu/build.rs:96`. Nothing in the repository currently
   detects a future Xcode reassociating the loop, because no CI job compiles the
   shader at all (§D2).
2. **`ninf` is undocumented.** Under fast-math the kernel assumes no infinities.
   `require_finite` guarantees finite *inputs*, but `b*b` overflows f32 for
   `|b| ≳ 1.8e19`, where `CpuBackend`'s f64 accumulation does not. That is a
   divergence class the accumulation doc comment does not mention.

`V1-13` itself is **not** at risk: the same compiled binary is deterministic
run-to-run, and `repeated_dispatches_of_the_same_block_return_identical_bytes`
(`metal.rs:746-773`) exercises a 7×300 block — 300 > 256, so the stripe loop and
the tree both run — five times for bit-identical output. I saw it pass.

### C6. Accumulation strategy — **PASS**

Verified against source, not against prose:

* **f32 on device.** Every accumulator in the kernel is `float`
  (`paired_reduction.metal:88-92`, `:110-118`). Metal has no `double`; the header
  says so at `:44`.
* **Delta formed in f32.** `const float delta = b - counterpart[index];`
  (`:108`). The header states plainly at `:46-49` that `CpuBackend` widens to f64
  *before* subtracting and that this, not the reduction order, is the dominant
  divergence.
* **Widening, not re-accumulation, on readback.** `widen`
  (`metal.rs:353-362`) is five `as f64` casts and nothing else. Confirmed by
  reading; there is no summation on the host.
* **Whole-block partials from a second independent flat dispatch.**
  `metal.rs:475-476`: `self.dispatch(&base.values, &counterpart.values, 1, rows *
  columns, 1, 0)` — `channel_count = 1`, `channel_stride = 0`, `element_stride =
  1`, i.e. flat row-major. Not a re-sum of the per-channel results. The
  consequence is asserted at `metal.rs:731-735` (rows-axis and columns-axis
  whole-block figures compared for exact equality) and I saw it pass.

**Is it stated clearly enough for QM-0127 to set a tolerance?** Yes, for
everything it does state — `metal.rs:37-49` and `paired_reduction.metal:42-59`
are unusually explicit, and they correctly frame the direction of dependency
("`QM-0127`'s tolerance is set against it rather than the other way round").
Incomplete only in the `ninf`/overflow respect named in §C5.2.

### C7. Objective-C shim instead of a crates.io binding — **PASS, with minor findings**

**Lockfile claim verified.** `git diff main..HEAD -- Cargo.lock` produces **zero
bytes of output**. `./scripts/license-audit.sh` exits 0 with `all checks passed`,
including `NOTICE 'rust-dependencies' table is current`. The claim holds exactly.

`build.rs:118-133` compiles the shim with `xcrun clang -x objective-c -fobjc-arc
-fmodules -O2 -Wall -Werror -c`, archives with `ar rcs` after removing any stale
archive (`:150`, a correct precaution), and links `-l framework=Metal` and
`framework=Foundation` (`:83-84`).

FFI-boundary scrutiny — what I checked and what I found:

* **Lifetimes across the boundary: sound.** The interface is deliberately
  C-flat; no opaque handle crosses it, so Rust owns nothing it must free
  (`qm_metal_shim.m:18-24`). Device, library, pipeline and queue are file-scope
  statics under `dispatch_once` (`:76-131`), built once. Every per-call
  allocation is inside `@autoreleasepool` (`:206-253`) and released under ARC on
  return.
* **`dispatch_data_create` with `DISPATCH_DATA_DESTRUCTOR_DEFAULT` (`:91-92`):
  correct.** That destructor causes the buffer to be **copied**, so the
  `'static` metallib slice is not aliased past the call, and the queue argument
  is ignored. Passing `dispatch_get_main_queue()` is inert here — no deadlock
  risk, since the block runs on the calling thread.
* **Command-buffer error checking: present and correct** (`:242-251`). Status is
  compared against `MTLCommandBufferStatusCompleted` and `memcpy` is reached only
  on success; the error text names the device and says "no partial output is
  returned". This satisfies the TASK.md row *"Never partial results presented as
  complete"* for the device-loss case.
* **Nil `commandBuffer`: safe by accident, but safe.** `[g_queue commandBuffer]`
  is not nil-checked, but messaging nil returns nil/0, so `[command_buffer
  status]` yields `0 == MTLCommandBufferStatusNotEnqueued ≠ Completed` and the
  function returns `QM_METAL_DISPATCH_FAILED`. The failure is caught.
* **Nil buffer allocation checked** (`:218-222`) → `QM_METAL_ALLOCATION_FAILED`.
* **Argument validation** (`:190-194`) rejects NULL pointers and zero extents.
* **`strlcpy` bounds** (`:59-68`, `:151`) are honoured; the Rust `SAFETY` comments
  at `metal.rs:190-193` and `:316-320` accurately describe what the shim does.
* **Rust-side unsafe: sound.** Both `unsafe` blocks pass pointers to live locals
  of the declared length, synchronously. `element_count` is bounded by
  `u32::try_from(base.len())` at `metal.rs:306-312` **before** the unchecked `as
  u32` casts at `:328-331`, and `channel_count`/`elements_per_channel` are
  derived from `rows`/`columns` of the same validated block, so the casts cannot
  truncate. Airtight today; see §D4.
* **Index bounds inside the kernel.** For `ChannelAxis::Columns` the maximum
  index is `(columns−1) + (rows−1)·columns = rows·columns − 1`; for `Rows` it is
  `(rows−1)·columns + (columns−1)`; for the whole-block pass, `rows·columns − 1`.
  All within `element_count`. No out-of-bounds device read.
* **Thread safety.** `dispatch_once` serialises setup; `MTLCommandQueue` is
  documented thread-safe for concurrent command-buffer creation. The Rust test
  harness runs these tests on multiple threads and did not fault.

Minor findings at §D3–D5.

### C8. Build-time failure names the shader and never falls back — **PASS**

Four distinct panic sites, each naming its file by the `SHADER`/`SHIM` constants
(`build.rs:47-48`):

| site | condition | names |
| --- | --- | --- |
| `build.rs:70-77` | `target_os != "macos"` with the feature on | the target, and how to build without it |
| `build.rs:93-99` | `xcrun metal` not runnable | `gpu/metal/paired_reduction.metal` |
| `build.rs:101-109` | shader fails to compile | the shader **plus stderr**, and states explicitly that a stub must never ship |
| `build.rs:131-158` | shim fails to compile, or `ar` fails | `gpu/metal/qm_metal_shim.m` |

There is no `if let Ok(...)`, no `unwrap_or_default`, no `cfg` that disables the
backend on failure. The metallib is consumed by `include_bytes!(concat!(env!(
"OUT_DIR"), "/paired_reduction.metallib"))` at `metal.rs:82`, so even a silently
missing artifact is a **compile** error in the crate, not a runtime one. The
panic path is not swallowed. Consistent with the `gpu/cuda/*.cu` doctrine.

---

## D. Defects

### D1. `STATUS.md` summary tally is now internally inconsistent — **file: `STATUS.md:261`**

The branch moved `GPU-003` out of `Not Started` in the requirement table but did
not update the Summary table that counts it.

```
main: 9 rows match '| **Not Started** |';  summary reads '| Not Started | 9 |'   ✔ consistent
HEAD: 8 rows match '| **Not Started** |';  summary reads '| Not Started | 9 |'   ✘ stale
```

`STATUS.md:255` declares "131 requirement rows"; the summary now sums to
104+10+9+5+1+2 = 131 only because it still counts a row that has left the
category. With the correct count of 8 it sums to 130, and `GPU-003` belongs to no
listed status. **This is the one place the branch makes `STATUS.md` say something
that is not true.** It is a stale count in a self-summary rather than a
capability claim, so it does not meet my bar for rejection — but it should be
fixed in the merge commit, not deferred.

Related, non-blocking: `Metal: Implemented, Not Verified` is a compound not in
the Status vocabulary at `STATUS.md:19-26`. I judge the compound to be *more*
honest than either defined term — `Implemented` is defined as "not covered by a
dedicated test" (false: there are 9), and `Hardware-Unverified` is defined as
"has **never** been executed on the hardware it targets" (false: it has). The
vocabulary table has no term for "executed on real hardware, not yet diffed
against the reference". Either add one or accept the compound; do not downgrade
the row to a defined-but-wrong term.

Everything else in the `GPU-003` row I checked and it is accurate and does not
overclaim: 9 tests behind an off-by-default feature (I counted 9), shader
compiles, dispatched on a real Apple M3 Pro, **not** diffed against `CpuBackend`,
`hardware_verified` false until `QM-0127`, wgpu **none — not started**.

### D2. The 9 metal tests sit outside every automated gate — structural, not this task's fault

Correctly excluded from the floor (see §E), but the honest consequence deserves
recording: **no CI job compiles them**, and `verify-baseline.sh` runs default
features, so nothing would notice if a future commit deleted, weakened or
`#[ignore]`d them. They are protected only by review and by whoever next runs
`cargo test -p q-gpu --features metal` on a Mac. `QM-0127` will add more tests to
this same unguarded set; that is the point at which the repository should decide
whether an opt-in local gate is warranted.

### D3. Staging budget under-counts the output buffer — `crates/q-gpu/src/metal.rs:407-416`, `gpu/metal/qm_metal_shim.m:216`

`check_workload` counts base + counterpart. The shim additionally allocates
`channel_count × 5 × 4` bytes (`qm_metal_shim.m:208, 216`). At the largest
accepted workload that is ≤ 163 840 B against a 256 MiB budget — 0.06 % — and the
doc comment at `metal.rs:53-56` states exactly what the budget counts, so nothing
is overclaimed. Recording it because `V1-03`'s residency ceiling covers the whole
process and the arithmetic should be exact when a real workload is planned.

### D4. Unchecked `as u32` casts — `crates/q-gpu/src/metal.rs:328-331`

`channel_count as u32`, `elements_per_channel as u32`, `element_stride as u32`,
`channel_stride as u32` are unchecked. They are sound **only** because
`u32::try_from(base.len())` at `:306` bounds `rows × columns`, and every one of
these is ≤ that product. Correct today; fragile against any future caller that
reaches `dispatch` by another route. A `debug_assert` or `try_from` would cost
nothing.

### D5. `[[g_device name] UTF8String]` unchecked for nil — `gpu/metal/qm_metal_shim.m:151`

`qm_copy_error` (`:59-68`) carefully handles a nil `NSString` and a NULL
`UTF8String`; the probe path does not, and would pass NULL to `strlcpy` if
`[MTLDevice name]` ever returned nil. Not observed, not reachable in practice on
Apple silicon. One-line asymmetry with the care taken elsewhere in the same file.

### D6. `require_finite` is an O(n) host scan before every dispatch — `crates/q-gpu/src/metal.rs:448-449`

No performance claim is made anywhere in this branch, so this is not a defect
now. Flagging it because any future benchmark that times
`paired_block_reduction` end to end will be timing a full host-side pass over
both blocks and will read as GPU throughput when it is not.

---

## E. Floor discipline

**The arithmetic reconciles exactly, and I verified it rather than trusting it.**

* Measured on this branch: **745 tests over 54 binaries**, derived by summing
  `test result:` lines (§B1), matching `scripts/baseline.json` `rust_tests: 745`,
  `rust_binaries: 54`. `verify-baseline.sh` reports "at floor" for all four
  fields (§B2). No stale floor left behind.
* **No test removed, weakened or ignored.** I diffed the test-function names
  between `main` and `HEAD`:
  ```
  git grep -h -A1 '^\s*#\[test\]' <ref> -- '*.rs' | grep -oE 'fn [a-z0-9_]+' | sort
  diff main HEAD  →  10 lines, all '>' (additions). Zero '<' lines.
  ```
  `#[test]` attribute count 727 → 737, i.e. **+10, zero removals**.
  `git grep -n '#\[ignore'` over the branch returns **nothing** — no ignored test
  exists anywhere in the repository, so none was added to hide a failure.
* **+10 attributes, +1 to the floor.** Nine of the ten live in
  `crates/q-gpu/src/metal.rs`, inside a `mod tests` that is only compiled when
  the module is (`lib.rs:38-39`, `#[cfg(feature = "metal")] pub mod metal;`). The
  tenth, `the_default_backend_is_the_cpu_reference_whatever_features_are_enabled`
  (`lib.rs:317-325`), is unconditional. **744 + 1 = 745.** Reconciles exactly.
* Cross-check: `q-gpu` reports 43+1+11 = 55 with the feature and 34+1+11 = 46
  without — a delta of exactly 9, matching the 9 cfg-gated tests.
* `rust_binaries` unchanged at 54, correctly: every new test went into an
  existing target, and no `tests/*.rs` file was added (which would have
  incremented the structural count even with its contents cfg'd out).
* Web untouched at 336/21 — no web file appears in `git diff --stat main..HEAD`.

**Is excluding the 9 feature-on tests from the floor correct?** **Yes, and it is
the only defensible choice.** `verify-baseline.sh` measures `cargo test
--workspace` with default features, on any machine including Linux CI. A floor of
754 could never be met by the command that enforces it, and per `CLAUDE.md` a
floor that cannot be reached — or one sitting below reality — "protects nothing".
Recording 745 keeps the guard's arithmetic honest and keeps the "no test was
removed" reconciliation meaningful. The implementer states the exclusion
explicitly in `baseline.json`'s `_rust_floor_raised_by` and
`_qm_0126_measurement_note` rather than burying it, which is the right handling.
The cost is real and I have recorded it as §D2, but it is a cost of the
architecture, not a concealment.

`commit` correctly left at `9c071dc` for the controller to repin at merge, with
the reasoning inherited from the QM-0101 note. Consistent with prior practice
(`6fb593a`).

---

## F. Honesty rules

* **Never claim a capability not exercised.** Held. `hardware_verified: false` is
  unreachable-otherwise (§C1). The module doc comment at `metal.rs:17-21` carries
  an explicit three-row claim/status table separating "the shader compiles" and
  "a dispatch returns partials of the right shape" (both proven) from "those
  partials agree with `CpuBackend`" (**not proven here**). The `display_name`
  string embeds `UNVERIFIED` so even a casual `q backends`-style listing would
  carry the caveat.
* **`QError::NotImplemented` rather than a fake.** `block_statistics`
  (`metal.rs:418-426`) and `matmul` (`:428-430`) both return
  `Self::unimplemented(...)`, which is `QError::not_implemented("GPU-003", …)`
  (`:281-290`) pointing at `CpuBackend`. Asserted at `metal.rs:670`
  (`err.requirement_id() == Some("GPU-003")`). `supports_statistics`,
  `supports_matmul`, `supports_histogram` are all literal `false`
  (`:388-393`) — and I confirmed no scheduler reads them, because `MetalBackend`
  is unreachable outside `q-gpu` entirely (§C3).
* **Data-plane doc comments.** All four new files declare the **Tensor Tile
  Plane** citing `ARCHITECTURE.md §2.1, §12.3`: `metal.rs:3-4`,
  `build.rs:3-4`, `paired_reduction.metal:4-5`, `qm_metal_shim.m:3-4`.
* **No semantic claims** about weights anywhere in the diff.
* **`gpu/wgsl/compute.wgsl` and `gpu/metal/compute.metal` left as placeholders** —
  confirmed absent from the diffstat.

### F1. The `## Not performed` disclosures — verified one by one, in code and `STATUS.md`

| disclosure | verified as genuinely non-claimed |
| --- | --- |
| No numerical verification against `CpuBackend`; §4.2's agreement is an *observation* | ✔ No assertion anywhere compares Metal to CPU. `a_small_paired_reduction_runs_on_device_and_returns_the_right_shape` (`metal.rs:704-744`) asserts only shape, counts, and Metal-vs-Metal axis independence; the CPU reference is `eprintln!`ed at `:738-743`, never compared. The comment at `:728-730` says so. `STATUS.md:188` repeats it. |
| The no-device skip path never fired here | ✔ Honest — I confirmed from my own `--nocapture` run that `device_or_skip` took `Some(_)` every time (§B3, no `SKIP` lines). The branch exists at `metal.rs:511-526` and prints a named reason. |
| Feature-on build with toolchain absent: read, not triggered | ✔ `build.rs:93-109` — panic path exists, not exercised. Correctly distinguished from a missing *device* in the module doc at `build.rs:26-29`. |
| Non-macOS feature-on build: read, not triggered | ✔ `build.rs:70-77`. |
| No benchmark | ✔ No `criterion`, no timing, no throughput figure in the diff or in `STATUS.md`. |
| `block_statistics` / `matmul` / histogram return `NotImplemented { GPU-003 }` | ✔ See §F above. |

The disclosures are **complete and honest** as far as I can determine. I found
nothing undisclosed except the fast-math/`ninf` point (§C5), which is a gap in
what the implementer knew rather than something withheld.

### F2. Deviations from the task plan — assessed

`gpu/metal/qm_metal_shim.m` is outside `Files Expected to Add`. It sits inside
the task's stated Program Boundary (`gpu/metal/`), the rationale is sound (a
crates.io binding would have added an unaudited transitive tree to
`license-audit.sh`'s surface for every consumer of the workspace), the claimed
consequence is verified (`Cargo.lock` byte-identical, `license-audit.sh` exit 0),
and the deviation is recorded in `.plan/evidence/QM-0126.md` §8. Acceptable.
Likewise `paired.rs`: three validators became `pub(crate)` with no behaviour
change, which is better than duplicating the reference's refusal logic.

---

## G. Why APPROVE

The one outcome this repository could not survive is a stub that ships claiming
GPU execution. That did not happen, and I did not take anyone's word for it: I
hand-computed eighteen figures from the fixture before running anything, traced
every code path that could write the returned values, confirmed the only writer
is a `memcpy` from an `MTLBuffer` gated on `MTLCommandBufferStatusCompleted`, and
read the emitted AIR to confirm the compiled kernel implements the order its
header documents. The kernel ran on an Apple M3 Pro and its output is
load-bearing.

Everything the branch claims, it has exercised. Everything it has not exercised,
it says so — in the code, in `capabilities()`, and in `STATUS.md`, not only in an
evidence file. `hardware_verified` cannot report true. The floor rose by exactly
the one test that a default build gains, no test was removed or weakened, and all
nine gates pass under my own hands.

**Residual risk I am accepting, named:** the reduction order is faithful under
today's toolchain but is not pinned by a compiler flag (§C5), and the nine tests
that prove the device path live outside every automated gate (§D2). Neither makes
anything merged false today; both are preconditions I place on `QM-0127`.

**Should be fixed in the merge commit:** the `STATUS.md:261` summary tally (§D1).

I would stake the repository's credibility on this branch.

---

## Not verified

* **That `main` @ `39b3aa2` measures 744/54.** I did not build `main`. I
  established the equivalent by name-diffing every `#[test]` between the two refs
  (+10, zero removals, zero `#[ignore]`) and by measuring 745/54 here, which
  forces 744 under default features. The floor's arithmetic is confirmed; its
  stated base measurement is inferred.
* **The no-device skip path.** This machine has an M3 Pro. The branch is
  compiled and reviewed by reading only — same limitation the implementer
  discloses.
* **A feature-on build with the Xcode toolchain absent, and a non-macOS
  feature-on build.** Both panic paths read, neither triggered; I did not
  uninstall Xcode or cross-compile.
* **Device-loss mid-dispatch.** The `MTLCommandBufferStatusCompleted` check
  (`qm_metal_shim.m:242-251`) was read, not provoked. I have no way to induce
  device loss.
* **Numerical agreement between `MetalBackend` and `CpuBackend`.** Deliberately
  not assessed — that is `QM-0127`, and the agreement visible in §B3 on one
  exactly-representable fixture is an observation, not a verification. I repeat
  the implementer's framing because it is correct.
* **Behaviour at f32 overflow** (`|b| ≳ 1.8e19`, §C5.2). Reasoned from the AIR's
  `ninf` flag, not measured.
* **Concurrency of `qm_metal_paired_reduction` under heavy multi-threaded load.**
  Metal's documented thread-safety guarantees were relied on; only the cargo test
  harness's own parallelism was exercised.
