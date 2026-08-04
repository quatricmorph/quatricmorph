# VALIDATION_PLAN — design partners, signals, kill criteria

Every other document in `.plan/` describes work an engineer can complete alone.
This one does not, and that is why it exists as a peer of `MASTER_PLAN.md` rather
than an appendix to it.

Derived from [`../Quatricmorph - Standalone Business, Market, and Technical
Strategy.md`](<../Quatricmorph - Standalone Business, Market, and Technical Strategy.md>)
§§8–11.

---

## 1. The one assumption under test

> That engineers working on quantisation, fine-tuning, and MoE routing will
> change a real decision, and pay, because of what Quatricmorph reveals.
> *(strategy §12)*

v1 exists to test that. Not to be complete, not to be beautiful, and not to
render anything in 3D.

The corollary that governs sequencing: **partner conversations start on day 1,
before the tool works.** The strategy is explicit that this is the ordering solo
founders get wrong, and `EXECUTION_ORDER.md` §10 puts `QM-0160` in parallel with
the first three engineering actions for that reason.

---

## 2. Who, and what to ask them

The revenue-bearing segments, narrowed (strategy §6):

| Segment | Their problem | Why they would look | Priority |
| --- | --- | --- | --- |
| **Model-compression / quantisation engineers** | Which layers are fragile under quantisation? | Ship a smaller model without losing accuracy → direct serving-cost savings | **Beachhead** |
| AI infrastructure / serving teams | Route imbalance, wasted capacity | Cut serving cost | Second |
| Fine-tuning / LoRA / merge shops | Did this merge collide? | Fewer failed merges | Third |
| Foundation-model labs | Deep checkpoint understanding | Research velocity | Hard, long cycles |
| Universities, open-source users | Teaching, papers | Brand and distribution only — **not revenue** |

**The first conversation is not a demo.** It is four questions:

1. Walk me through the last time a quantisation config surprised you. What did you
   see, and when did you see it?
2. What do you look at today to decide which layers stay at higher precision?
3. If you could see where the error concentrates before running an eval, what
   would you do differently?
4. Would you run this on a checkpoint you cannot share with me?

Question 4 is the one that matters. `V1-29` — a partner running it on a checkpoint
the founder did not choose — is the strategy's first PMF signal, and it is
answered by whether they say yes to that question, not by anything on a screen.

**Where to find them:** quantisation- and vLLM-adjacent Discords, r/LocalLLaMA,
ML-infra circles, the maintainers and heavy users of the compression toolchains,
and direct outreach. Target: **3–5 design partners lined up before the engine
works**, per the strategy's Days 0–30.

**NVIDIA Inception** (free, no equity, GPU cloud credits): apply in Days 0–30. It
costs nothing, it is the escape route from this machine's 51 GB disk ceiling
(`DEFINITION_OF_DONE.md` §1 waiver), and applying late is the only way it can
become blocking.

---

## 3. Signals, in priority order

From the strategy §10, unchanged, mapped to acceptance criteria:

| # | Signal | Criterion | How it is recorded |
| --- | --- | --- | --- |
| 1 | A design partner imports their own **private** checkpoint unprompted | `V1-29` | `QM-0161` — dated account: who, which model, what they found |
| 2 | **Repeated use** — the same user returns across weeks, not one session | `V1-31` | `QM-0164` — session log with dates |
| 3 | A **documented decision change** — a fragile quant config dropped, a keep-set adopted, a merge aborted | `V1-30` | `QM-0162` — decision before, output, decision after |
| 4 | **Willingness to pay** — a signed pilot, a card, or an explicit "we would budget for this" | `V1-32` | `QM-0163` — the artifact of the probe |

### Explicitly not signals

GitHub stars. Demo-video views. Retweets. "This is really cool." Conference
interest. A well-received screenshot.

The strategy names these as distribution, not validation, and the distinction is
the whole reason v1 is a diagnostic and not a 3D fly-through. A tool at Level 1 on
the value ladder collects all of the above and none of the four signals.

---

## 4. Kill criteria

**Any two of the four signals missing by month 6.**

The honest response then is the strategy's, and it is written down now so that it
is a decision rather than a mood later:

> Convert the technical brand into research output, a paper, or a role at an
> infra/interpretability lab — not to keep building outward. *(strategy §10)*

What is explicitly **not** the response to a failed wedge:

* Building module 2 (MoE) because module 1 did not land.
* Building the deferred platform because the report "needed a better UI."
* Broadening to a general interpretability platform.

Each of those converts a clear negative result into an ambiguous one, and spends
the remaining window doing it.

The out-of-core streaming core, the canonical address space, and the residency
discipline are publishable and durable regardless — IEEE VIS / EuroVis-adjacent
large-scale visualisation venues, and MLSys / NeurIPS-ICML workshop tracks
(strategy §5). That is a real outcome, not a consolation.

---

## 5. Pivot criteria

Two, both from the strategy §10, both with a defined engineering consequence in
this repository:

### 5.1 Diagnosis lands, the spatial view does not

**Signal:** partners act on the ranking and the report; nobody opens the heat-map
twice, or `V1-25`'s legibility test keeps failing.

**Response:** go headless. Same out-of-core core, less rendering investment. The
report and the manifest already are the product; `apps/web/diagnostics` stops
growing and the deferred platform release stops being the assumed next step.

This is the pivot v1 is *pre-positioned* for — which is precisely why the surface
is one heat-map fed by a manifest rather than a tile pipeline.

### 5.2 Quantisation is cold, something else is hot

**Signal:** partners shrug at quantisation error but keep asking about expert
health or checkpoint diffs.

**Response:** follow the heat. [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §3 keeps
modules 2 and 3 ordered and their seams open — `MOE-001` reuses the expert-keyed
aggregation, `DIFF-001` reuses the paired reduction verbatim. Both are weeks, not
months, *because* v1 built the reduction as genuinely paired rather than
hard-coding base-vs-its-own-quantisation.

### 5.3 The inverse — the spatial view is the reason they would pay

**Signal:** a partner says, unprompted, that the 3D view is what they want.

**Response:** the platform lane resumes where it stopped —
`STRATEGY_ALIGNMENT.md` §7. Nothing was deleted; the deferral is one line per
task.

---

## 6. The 90-day shape

The strategy's §11 plan, mapped to this repository's tasks. No calendar, an
ordering.

### Days 0–30 — wedge and validation setup

| Do | Task |
| --- | --- |
| Lock the value-proposition sentence and use it verbatim everywhere | `QM-0160`, `MASTER_PLAN.md` §3 |
| Start the real-checkpoint acquisition — longest lead time in the plan | `QM-0100` |
| Build the residency proof against it | `QM-0101` |
| Line up 3–5 design partners **before polishing anything** | `QM-0160` |
| Apply to NVIDIA Inception | `QM-0160` |
| Read the Palace paper (arXiv:2509.26213) before finalising the streaming design | `QM-0101` |

The Palace reading is cheap and specifically recommended: it is the closest
published prior art to this streaming layer, and the plan's chunked pull-based
architecture should be a deliberate agreement or a deliberate departure, not a
coincidence.

### Days 30–60 — first real diagnosis

| Do | Task |
| --- | --- |
| Ship the engine end to end on a real model **with a partner watching** | `QM-0120`…`QM-0125` |
| Ship the Markdown report from the first run, not at the end | `QM-0140`, `QM-0141` |
| Track whether they import their own checkpoint, and whether the output surprised them | `QM-0161` |

"With a partner watching" is load-bearing. A diagnosis nobody watched produces no
signal.

### Days 60–90 — decision influence and price

| Do | Task |
| --- | --- |
| Get one documented decision-change case | `QM-0162` — the release gate |
| Run a price probe — to test willingness, not to maximise revenue | `QM-0163` |
| Draft a short technical write-up on the out-of-core diagnosis | `QM-0166` |

---

## 7. Competitive watch

Quarterly, one paragraph each, appended to
[`RISK_REGISTER.md`](RISK_REGISTER.md). The strategy asks for a 2–3 month cadence
and names the three that matter:

| Watch | Question | Why it matters |
| --- | --- | --- |
| **Goodfire** | Has the "model design environment" shipped anything at raw-tensor level, as opposed to features/SAEs? | Best-capitalised neighbour; different abstraction layer today |
| **MixtureKit** and successors | Has routing visualisation extended to arbitrary large open-weight checkpoints? | Already occupies the MoE-routing *idea*; would contest module 2 |
| **CoreWeave / Weights & Biases** | Has anything shipped that touches static checkpoint weights rather than runtime telemetry? | Best distribution in the space; adjacent, not inside, today |
| **Palace and the visualisation-research community** | Has anyone retargeted an out-of-core tensor framework at ML checkpoints? | Would remove the systems moat directly |

The strategy's read is that the window is **12–18 months, not indefinite**. The
watch exists to notice it closing, not to react to every announcement.
