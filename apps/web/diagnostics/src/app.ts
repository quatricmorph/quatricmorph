/**
 * The view-model: a manifest in, everything the surface displays out.
 *
 * Nothing here touches the DOM. The whole surface is a value, so a test reads
 * the words the reader will read instead of inspecting a screenshot — which is
 * how the two rules that govern this file are actually enforced:
 *
 * * **`AGENTS.md` §4 / `ARCHITECTURE.md` §18 AC-010** — every displayed result
 *   is labelled `exact`, `sampled` or `approximate`, in the surface itself.
 * * **`ARCHITECTURE.md` §19** — *do not assume a colour pattern corresponds to
 *   a semantic concept*. This surface may say **where** a measured number is
 *   large. It may not say **what that means** about the model. The required
 *   wordings below come from `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8, and
 *   `forbiddenClaimTermsIn` is the mechanical check that nothing drifts past
 *   them.
 */

import {
  buildGrid,
  encodeMagnitude,
  type Band,
  type BandSource,
  type Grid,
  type RowInput,
} from './heatmap.js'
import {
  relativeErrorOf,
  type ErrorAggregate,
  type Fidelity,
  type Manifest,
} from './manifest-client.js'

/** The three-way vocabulary the data model types end to end (`SRC-018`, `STAT-005`). */
export const FIDELITY_LABELS: readonly Fidelity[] = ['exact', 'sampled', 'approximate']

/**
 * Wordings this surface is required to use.
 *
 * The first three are `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8's "required
 * wording" column, reproduced because a consumer that renders the finding
 * without the caveat has emitted the forbidden claim. The fourth is this
 * surface's own answer to §19.
 */
export const REQUIRED_WORDING = {
  accuracy:
    'Weight-space error only. Accuracy impact is not measured — run your evaluation on the recommended configuration.',
  ranking: 'Ranked by relative weight-space error, a proxy for sensitivity.',
  frontier: 'Greedy over error-per-byte; not proven optimal.',
  colour:
    'A colour is not a finding. Cell colour, fill and glyph encode one measured quantity — relative weight-space error — and nothing else. What a layer does, and what quantising it costs in accuracy, are not measured here.',
} as const

/**
 * Claim vocabulary this surface may never use.
 *
 * Each stem is matched at word boundaries so ordinary prose is not flagged:
 * `deadline` is not `dead`, and `concepts` is not `concept`. A test asserts
 * that none of the required wordings above trips this list, and another that
 * the list is not vacuous.
 */
const CLAIM_RULES: readonly { term: string; pattern: RegExp }[] = [
  { term: 'important', pattern: /\bimportant\b/i },
  { term: 'unimportant', pattern: /\bunimportant\b/i },
  { term: 'matters', pattern: /\bmatters\b/i },
  { term: 'dead', pattern: /\bdead(ness)?\b/i },
  { term: 'semantic', pattern: /\bsemantics?\b/i },
  { term: 'concept', pattern: /\bconcept\b/i },
  { term: 'understand', pattern: /\bunderstands?\b/i },
  { term: 'knows', pattern: /\bknows\b/i },
  { term: 'reasoning', pattern: /\breasoning\b/i },
  { term: 'capability', pattern: /\bcapabilit(y|ies)\b/i },
  { term: 'hessian', pattern: /\bhessian\b/i },
  { term: 'predicted accuracy', pattern: /\bpredicted accuracy\b/i },
  { term: 'accuracy delta', pattern: /\baccuracy delta\b/i },
]

export const FORBIDDEN_CLAIM_TERMS: readonly string[] = CLAIM_RULES.map((rule) => rule.term)

/** Forbidden claim vocabulary present in `text`. Empty means the text is clean. */
export function forbiddenClaimTermsIn(text: string): string[] {
  return CLAIM_RULES.filter((rule) => rule.pattern.test(text)).map((rule) => rule.term)
}

/** The layer a canonical address belongs to, or `null` when it belongs to none. */
export function layerIndexOf(address: string): number | null {
  const match = /\blayers\[(\d+)\]/.exec(address)
  return match === null ? null : Number.parseInt(match[1], 10)
}

export type EmptyState = { explanation: string; requirementIds: string[] }

export type LegendEntry = {
  kind: 'magnitude' | 'uniform' | 'aggregated' | 'fidelity' | 'undefined'
  label: string
  colour: string
  greyscale: string
  glyph: string
}

export type Legend = {
  /** What the colour encodes, in terms of the measured quantity. */
  encodes: string
  /** What it does not mean. `REQUIRED_WORDING.colour`. */
  notAClaim: string
  /** `exact`, `sampled` or `approximate`, from the manifest. */
  fidelityLabel: Fidelity
  aggregationNote: string
  /** The range colour and fill are normalised against — not an absolute scale. */
  scaleNote: string
  entries: LegendEntry[]
}

export type RankingRow = {
  rank: number
  address: string
  layerIndex: number | null
  relativeError: number
  parameterCount: number
  fidelityLabel: Fidelity
}

export type FrontierRow = {
  step: number
  keepSetSize: number
  addedBytes: number
  errorRemovedFraction: number
  fidelityLabel: Fidelity
}

export type RefusalRow = { requirementId: string; what: string; why: string }

export type ExpertRow = {
  layerIndex: number
  expertIndex: number
  relativeError: number | null
  fidelityLabel: Fidelity
}

export type Surface = {
  runId: string
  modelId: string
  revisionHash: string
  architecture: string
  resolverConfidence: 'resolved' | 'unknown'
  precision: string
  backend: string
  fidelity: Fidelity
  fidelityLabel: Fidelity
  drill: { level: 'model' | 'layer'; layerIndex: number | null; path: string[] }
  totals: { relativeError: number | null; parameterCount: number; fidelityLabel: Fidelity }
  heatmap: {
    grid: Grid
    resolution: 'layer' | 'tensor'
    resolutionNote: string
    legend: Legend
    empty: EmptyState | null
  }
  ranking: { rows: RankingRow[]; caveat: string; empty: EmptyState | null }
  frontier: { rows: FrontierRow[]; claim: string; empty: EmptyState | null }
  refusals: { rows: RefusalRow[]; empty: EmptyState | null }
  /** `null` when the manifest reports no experts — the panel is omitted, not blanked. */
  experts: { rows: ExpertRow[] } | null
  caveats: string[]
}

export type BuildOptions = { cellCeiling?: number; layerIndex?: number }

/** Turn a validated manifest into the surface. */
export function buildSurface(manifest: Manifest, options: BuildOptions = {}): Surface {
  const fidelity = manifest.fidelity
  const requirementIds = manifest.refusals.map((refusal) => refusal.requirement_id)
  const selected = options.layerIndex

  if (selected !== undefined) {
    if (manifest.projection !== 'full') {
      throw new Error(
        `layer ${selected} cannot be opened from a summary manifest: the summary projection carries no ` +
          `per-tensor detail. Fetch the detail for that layer first.`,
      )
    }
    if (!layerIndicesOf(manifest).includes(selected)) {
      throw new Error(
        `layer ${selected} is not in this manifest; it reports layers ` +
          `${JSON.stringify(layerIndicesOf(manifest))}.`,
      )
    }
  }

  const resolution: 'layer' | 'tensor' = manifest.projection === 'full' ? 'tensor' : 'layer'
  const rows =
    resolution === 'tensor'
      ? tensorRows(manifest, selected)
      : manifest.layers.map(layerRow)
  const grid = buildGrid(rows, fidelity, options.cellCeiling)

  return {
    runId: manifest.run.run_id,
    modelId: manifest.model.model_id,
    revisionHash: manifest.model.revision_hash,
    architecture: manifest.model.architecture,
    resolverConfidence: manifest.model.resolver_confidence,
    precision: manifest.config.precision,
    backend: manifest.run.backend,
    fidelity,
    fidelityLabel: fidelity,
    drill: {
      level: selected === undefined ? 'model' : 'layer',
      layerIndex: selected ?? null,
      path: selected === undefined ? ['model'] : ['model', `layer ${selected}`],
    },
    totals: {
      relativeError: relativeErrorOf(manifest.totals),
      parameterCount: manifest.model.parameter_count,
      fidelityLabel: fidelity,
    },
    heatmap: {
      grid,
      resolution,
      resolutionNote: resolutionNote(resolution),
      legend: buildLegend(grid),
      empty:
        grid.cellCount > 0
          ? null
          : {
              explanation:
                resolution === 'tensor'
                  ? 'This run examined no tensor, so there is nothing to paint. The refusals below say why.'
                  : 'This run reports no layer aggregate, so there is nothing to paint. The refusals below say why.',
              requirementIds,
            },
    },
    ranking: {
      rows: manifest.ranking.map((entry, index) => ({
        rank: index + 1,
        address: entry.address,
        layerIndex: layerIndexOf(entry.address),
        relativeError: entry.relative_error,
        parameterCount: entry.parameter_count,
        fidelityLabel: fidelity,
      })),
      caveat: REQUIRED_WORDING.ranking,
      empty:
        manifest.ranking.length > 0
          ? null
          : {
              explanation: 'This run ranked no tensor. The refusals below say what was not computed.',
              requirementIds,
            },
    },
    frontier: {
      rows: manifest.frontier.steps.map((step, index) => ({
        step: index + 1,
        keepSetSize: step.keep_set.length,
        addedBytes: step.added_bytes,
        errorRemovedFraction: step.error_removed_fraction,
        fidelityLabel: fidelity,
      })),
      // Reproduced from the manifest, never rewritten: the schema pins this
      // string precisely so no consumer can present a frontier without it.
      claim: manifest.frontier.claim,
      empty:
        manifest.frontier.steps.length > 0
          ? null
          : {
              explanation: 'No frontier was computed for this run. The refusals below say why.',
              requirementIds,
            },
    },
    refusals: {
      rows: manifest.refusals.map((refusal) => ({
        requirementId: refusal.requirement_id,
        what: refusal.what,
        why: refusal.why,
      })),
      empty:
        manifest.refusals.length > 0
          ? null
          : {
              explanation:
                'The manifest carries an empty refusals array, which is a claim that nothing was refused.',
              requirementIds: [],
            },
    },
    experts:
      manifest.experts.length === 0
        ? null
        : {
            rows: manifest.experts.map((expert) => ({
              layerIndex: expert.layer_index,
              expertIndex: expert.expert_index,
              relativeError: relativeErrorOf(expert.aggregate),
              fidelityLabel: fidelity,
            })),
          },
    caveats: [REQUIRED_WORDING.accuracy, REQUIRED_WORDING.colour],
  }
}

/** Every string this surface displays, for the vocabulary check and for tests. */
export function surfaceStrings(surface: Surface): string[] {
  const strings: string[] = [
    surface.runId,
    surface.modelId,
    surface.revisionHash,
    surface.architecture,
    surface.resolverConfidence,
    surface.precision,
    surface.backend,
    surface.fidelityLabel,
    surface.heatmap.resolutionNote,
    surface.heatmap.legend.encodes,
    surface.heatmap.legend.notAClaim,
    surface.heatmap.legend.aggregationNote,
    surface.heatmap.legend.scaleNote,
    surface.heatmap.legend.fidelityLabel,
    surface.ranking.caveat,
    surface.frontier.claim,
    ...surface.drill.path,
    ...surface.caveats,
    ...surface.heatmap.legend.entries.map((entry) => entry.label),
    ...surface.heatmap.grid.rows.map((row) => row.label),
    ...surface.ranking.rows.map((row) => row.address),
    ...surface.refusals.rows.flatMap((row) => [row.requirementId, row.what, row.why]),
  ]
  for (const empty of [
    surface.heatmap.empty,
    surface.ranking.empty,
    surface.frontier.empty,
    surface.refusals.empty,
  ]) {
    if (empty !== null) strings.push(empty.explanation, ...empty.requirementIds)
  }
  return strings
}

function resolutionNote(resolution: 'layer' | 'tensor'): string {
  return resolution === 'tensor'
    ? 'One cell per tensor, spanning the output channels its shape declares: manifest v1 publishes no per-channel partials, so no finer column exists to draw.'
    : 'One cell per layer: the summary projection and manifest v1 publish no per-channel partials, so no finer column exists to draw.'
}

function buildLegend(grid: Grid): Legend {
  const entries: LegendEntry[] = []
  const domain = grid.domain

  // A key describes what is on the map. Nothing measurable means no ranking to
  // key — the six-tier ramp used to be printed against a substituted 0..1
  // domain, which is a range this run never produced.
  if (domain !== null && domain.max > domain.min) {
    for (let tier = 0; tier < 6; tier += 1) {
      const encoding = encodeMagnitude(domain.min + ((domain.max - domain.min) * tier) / 5, domain)
      entries.push({
        kind: 'magnitude',
        label: `tier ${tier + 1} of 6 — fill ${Math.round(encoding.fillFraction * 100)}%`,
        colour: encoding.colour,
        greyscale: encoding.greyscale,
        glyph: encoding.glyph,
      })
    }
  } else if (domain !== null) {
    // One value across the whole map. Six tiers here would all be the same
    // swatch reading "fill 100%", which is a key that decodes nothing.
    const encoding = encodeMagnitude(domain.min, domain)
    entries.push({
      kind: 'uniform',
      label: `every measured cell has the same value (${domain.min}); there is no spread to rank`,
      colour: encoding.colour,
      greyscale: encoding.greyscale,
      glyph: encoding.glyph,
    })
  }

  entries.push({
    kind: 'fidelity',
    label: `every value on this map is ${grid.fidelity}`,
    colour: '#ffffff',
    greyscale: '#ffffff',
    glyph: grid.fidelity.charAt(0),
  })

  if (grid.anyAggregated) {
    entries.push({
      kind: 'aggregated',
      // Names the mark `render.ts` actually draws — a dashed border. The key
      // draws that same dash around this entry's swatch, so the reader is told
      // to look for a mark they can also see here.
      label: `cells with a dashed border aggregate more than one channel, by maximum (factor ${grid.aggregationFactor})`,
      colour: '#ffffff',
      greyscale: '#ffffff',
      glyph: '- -',
    })
  }

  if (grid.undefinedCellCount > 0) {
    const undefinedEncoding = encodeMagnitude(null, domain)
    entries.push({
      kind: 'undefined',
      label: `${grid.undefinedCellCount} cell(s) have no measurement; that is not a value of zero`,
      colour: undefinedEncoding.colour,
      greyscale: undefinedEncoding.greyscale,
      glyph: undefinedEncoding.glyph,
    })
  }

  return {
    encodes:
      'Colour, fill width and glyph all encode relative weight-space error, sqrt(sum_sq_delta / sum_sq_base).',
    notAClaim: REQUIRED_WORDING.colour,
    fidelityLabel: grid.fidelity,
    aggregationNote:
      grid.aggregationFactor > 1
        ? `Columns are merged ${grid.aggregationFactor} to a cell, by maximum rather than mean, so one bad channel is not averaged away.`
        : 'No column merging was needed: every cell is drawn at the resolution the manifest publishes.',
    // Without this line the darkest cell reads as "bad" in absolute terms. It
    // is only the largest value on this map.
    scaleNote:
      grid.domain === null
        ? 'Nothing on this map is measurable, so there is no scale.'
        : grid.domain.max > grid.domain.min
          ? `The scale is relative to this map: the lightest cell is ${grid.domain.min} and the darkest is ${grid.domain.max}. It is not an absolute threshold.`
          : // No spread, so no scale: saying "the lightest is 0.2 and the
            // darkest is 0.2" invites the reader to read a ranking into a map
            // that has none.
            `Every measured cell on this map has the same value, ${grid.domain.min}. There is no lightest and no darkest, so no cell is drawn as worse than another, and nothing here says whether ${grid.domain.min} is large.`,
    entries,
  }
}

function layerIndicesOf(manifest: Manifest): number[] {
  const indices = new Set<number>(manifest.layers.map((layer) => layer.layer_index))
  for (const tensor of manifest.tensors ?? []) {
    const index = layerIndexOf(tensor.address)
    if (index !== null) indices.add(index)
  }
  return [...indices].sort((a, b) => a - b)
}

function layerRow(layer: { layer_index: number; aggregate: ErrorAggregate }): RowInput {
  const band: Band = {
    // A layer aggregate covers every channel of every tensor in the layer, and
    // the summary projection publishes no shape. `null` says so.
    channelStart: null,
    channelEnd: null,
    relativeError: relativeErrorOf(layer.aggregate),
    source: `layer ${layer.layer_index}`,
  }
  return { layerIndex: layer.layer_index, label: `layer ${layer.layer_index}`, bands: [band] }
}

/**
 * One row per layer, one band per tensor, the band as wide as the tensor's
 * output-channel count.
 *
 * Tensors outside the repeated stack — embeddings, the head — get their own
 * row. Dropping them would be truncation, and truncation is what produces a
 * confidently wrong picture.
 */
function tensorRows(manifest: Manifest, selected: number | undefined): RowInput[] {
  const byLayer = new Map<number | null, Band[]>()
  const offsets = new Map<number | null, number>()

  for (const tensor of manifest.tensors ?? []) {
    const index = layerIndexOf(tensor.address)
    if (selected !== undefined && index !== selected) continue
    const channels = tensor.shape.length > 0 ? tensor.shape[0] : null
    const offset = offsets.get(index) ?? 0
    const bands = byLayer.get(index) ?? []
    bands.push({
      channelStart: channels === null ? null : offset,
      channelEnd: channels === null ? null : offset + channels,
      relativeError: relativeErrorOf(tensor.aggregate),
      source: tensor.address,
    })
    byLayer.set(index, bands)
    offsets.set(index, offset + (channels ?? 0))
  }

  if (selected === undefined) {
    for (const layerIndex of layerIndicesOf(manifest)) {
      if (!byLayer.has(layerIndex)) byLayer.set(layerIndex, [])
    }
  }

  const ordered = [...byLayer.keys()].sort(compareRowKeys)
  return ordered.map((layerIndex) => ({
    layerIndex,
    label: layerIndex === null ? 'outside the layer stack' : `layer ${layerIndex}`,
    bands: byLayer.get(layerIndex) as BandSource,
  }))
}

function compareRowKeys(a: number | null, b: number | null): number {
  if (a === null) return 1
  if (b === null) return -1
  return a - b
}
