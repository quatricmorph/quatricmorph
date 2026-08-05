/**
 * Grid geometry and magnitude encoding for the heat-map.
 *
 * Pure functions. No DOM, no canvas, no fetch — `render.ts` turns what this
 * produces into pixels, and `app.ts` decides what goes in.
 *
 * Three contracts live here.
 *
 * * **A ceiling, not a truncation.** At most `MAX_HEATMAP_CELLS` cells are ever
 *   produced. Above it, columns merge and every merged cell says so. Every
 *   channel remains covered by exactly one cell; nothing is dropped. This is
 *   the browser-side analogue of `assertBlockIsBounded` (`GRID-005`).
 * * **Merge by maximum, never by mean.** A mean of three healthy channels and
 *   one catastrophic one looks unremarkable, and the catastrophic channel is
 *   the finding the reader opened the tool for.
 * * **Three states, not two** (`QM-0153`). A cell is `exact`, `aggregated` —
 *   the renderer merged columns — or `sampled`, which is the engine's own
 *   coarseness arriving from the manifest. `cellFidelityOf` is where that is
 *   decided, and the two coarsenesses never share a mark or a legend entry.
 * * **Redundant encoding.** Magnitude is carried by colour *and* by a fill
 *   fraction *and* by an ordinal glyph, so it survives greyscale printing and
 *   colour-vision differences. `ARCHITECTURE.md` §19: a colour is not a
 *   meaning — and here, a colour is not even the only channel.
 */

import type { Fidelity } from './manifest-client.js'

export type { Fidelity }

/**
 * The rendering ceiling.
 *
 * A 100-layer model with 8 192 channels per layer is 819 200 cells. It
 * aggregates; it never truncates and never tries to draw every channel.
 */
export const MAX_HEATMAP_CELLS = 250_000

/** Ordered magnitude tiers. Six is enough to read and few enough to tell apart. */
export const MAGNITUDE_TIERS = 6

/**
 * One contiguous run of output channels the manifest publishes a number for.
 *
 * `channelStart` and `channelEnd` are `null` when the manifest publishes no
 * channel extent — a summary manifest carries one number per layer and no
 * shapes, and inventing an extent would be inventing data.
 */
export type Band = {
  channelStart: number | null
  channelEnd: number | null
  /** sqrt(sum_sq_delta / sum_sq_base); `null` means not measured, never zero. */
  relativeError: number | null
  /** What the manifest published this number for — a tensor address, or a layer. */
  source: string
}

export type Cell = {
  layerIndex: number | null
  columnIndex: number
  channelStart: number | null
  channelEnd: number | null
  /** Channels this cell covers, or `null` when the manifest publishes no extent. */
  channelsPerCell: number | null
  relativeError: number | null
  /** True when this cell covers more than one channel, or an unknown number of them. */
  aggregated: boolean
  /** Manifest bands merged into this cell. 1 means no merging happened. */
  bandsPerCell: number
  /** Propagated from the manifest. The engine's coarseness, not the renderer's. */
  fidelity: Fidelity
  sources: string[]
}

/**
 * What a cell's number is, and how it is drawn — in three states, never two.
 *
 * `sampled` is the **engine's** coarseness: the number itself was not computed
 * over every element. `aggregated` is the **renderer's**: the number is exactly
 * what the engine published, and this display merged neighbouring columns to
 * stay under the cell ceiling. Collapsing them into one flag would tell a
 * reader the data is coarse when only the picture is, or the reverse.
 *
 * `channelsPerCell` is `number | null` rather than `number` because
 * `Cell.channelsPerCell` is: manifest v1's summary projection publishes one
 * number per layer and no shape, so the channel extent is genuinely unpublished
 * for those cells. Claiming `1` there would assert a resolution the manifest
 * never gave. (`QM-0153`'s data contract writes `number`; this is the minimal
 * widening that keeps the type from having to invent an extent.)
 */
export type CellFidelity =
  | { kind: 'exact' }
  | { kind: 'aggregated'; channelsPerCell: number | null }
  | { kind: 'sampled' }

/**
 * Classify one cell for marker selection.
 *
 * The engine's coarseness wins over the renderer's, because a merged cell of
 * sampled numbers is first of all sampled — a reader told only "merged" would
 * believe the underlying numbers were complete. Nothing is lost by the
 * precedence: `Cell.aggregated` and `Cell.channelsPerCell` are still on the
 * cell, and `render.ts` draws both marks when both apply.
 *
 * The manifest's own word (`Cell.fidelity`: `exact`, `sampled` or
 * `approximate`) is what the surface *displays*; this function only chooses a
 * mark, so an `approximate` run is marked like a sampled one and still reads
 * "approximate" everywhere a reader can see.
 */
export function cellFidelityOf(cell: Cell): CellFidelity {
  if (cell.fidelity !== 'exact') return { kind: 'sampled' }
  if (cell.aggregated) return { kind: 'aggregated', channelsPerCell: cell.channelsPerCell }
  return { kind: 'exact' }
}

export type Row = {
  layerIndex: number | null
  label: string
  /** Channels the row covers, or `null` when no extent is published. */
  channelCount: number | null
  cells: Cell[]
  /** Bands merged into each cell. 1 means the row is drawn as published. */
  aggregationFactor: number
}

export type Grid = {
  rows: Row[]
  cellCount: number
  cellCeiling: number
  /** The largest aggregation factor any row needed. */
  aggregationFactor: number
  anyAggregated: boolean
  fidelity: Fidelity
  /** The range of defined values, or `null` when nothing is measurable. */
  domain: { min: number; max: number } | null
  undefinedCellCount: number
}

/** Random access over a row's bands, so a 65 536-channel row costs nothing to describe. */
export interface BandSource {
  readonly length: number
  at(index: number): Band | undefined
}

export type RowInput = {
  layerIndex: number | null
  label: string
  bands: BandSource
}

/** `channelCount` unit-wide bands, produced on demand. */
export function uniformBands(
  channelCount: number,
  valueAt: (index: number) => number | null,
  source: string,
): BandSource {
  return {
    length: channelCount,
    at(index: number): Band | undefined {
      if (index < 0 || index >= channelCount) return undefined
      return { channelStart: index, channelEnd: index + 1, relativeError: valueAt(index), source }
    },
  }
}

/**
 * How many columns one row may have.
 *
 * More rows than the ceiling has cells is refused rather than silently dropping
 * rows: a heat-map missing layers is a confidently wrong picture.
 */
export function maxColumnsPerRow(rowCount: number, cellCeiling: number = MAX_HEATMAP_CELLS): number {
  if (rowCount <= 0) return cellCeiling
  if (rowCount > cellCeiling) {
    throw new Error(
      `${rowCount} rows cannot be drawn under a ceiling of ${cellCeiling} cells without dropping ` +
        `rows. Nothing is truncated silently; narrow the selection instead.`,
    )
  }
  return Math.floor(cellCeiling / rowCount)
}

/** Bands merged into each cell so that `bandCount` fits in `maxColumns`. */
export function aggregationFactorFor(bandCount: number, maxColumns: number): number {
  if (maxColumns <= 0) throw new Error(`a row must be allowed at least one column, not ${maxColumns}`)
  if (bandCount <= maxColumns) return 1
  return Math.ceil(bandCount / maxColumns)
}

/** Merge a row's bands down to at most `maxColumns` cells, by maximum. */
export function planRow(input: RowInput, maxColumns: number, fidelity: Fidelity): Row {
  const bandCount = input.bands.length
  const factor = bandCount === 0 ? 1 : aggregationFactorFor(bandCount, maxColumns)
  const cells: Cell[] = []

  for (let columnIndex = 0; columnIndex * factor < bandCount; columnIndex += 1) {
    const first = columnIndex * factor
    const last = Math.min(first + factor, bandCount)

    let start: number | null = null
    let end: number | null = null
    let extentKnown = true
    let worst: number | null = null
    const sources: string[] = []

    for (let i = first; i < last; i += 1) {
      const band = input.bands.at(i)
      if (band === undefined) {
        throw new Error(`band ${i} of row ${String(input.layerIndex)} is missing; the grid would have a hole`)
      }
      if (band.channelStart === null || band.channelEnd === null) {
        extentKnown = false
      } else {
        start = start === null ? band.channelStart : Math.min(start, band.channelStart)
        end = end === null ? band.channelEnd : Math.max(end, band.channelEnd)
      }
      // Maximum, not mean. `null` is absent, and absent never lowers a maximum.
      if (band.relativeError !== null) {
        worst = worst === null ? band.relativeError : Math.max(worst, band.relativeError)
      }
      sources.push(band.source)
    }

    const channelsPerCell = extentKnown && start !== null && end !== null ? end - start : null
    cells.push({
      layerIndex: input.layerIndex,
      columnIndex,
      channelStart: extentKnown ? start : null,
      channelEnd: extentKnown ? end : null,
      channelsPerCell,
      relativeError: worst,
      // An unknown extent is never claimed to be a single channel.
      aggregated: channelsPerCell === null || channelsPerCell > 1,
      bandsPerCell: last - first,
      fidelity,
      sources,
    })
  }

  const lastCell = cells[cells.length - 1]
  const channelCount =
    cells.length === 0 ? 0 : lastCell.channelEnd !== null && cells[0].channelStart !== null
      ? lastCell.channelEnd - cells[0].channelStart
      : null

  return { layerIndex: input.layerIndex, label: input.label, channelCount, cells, aggregationFactor: factor }
}

/** Plan every row under one shared ceiling. */
export function buildGrid(
  rows: RowInput[],
  fidelity: Fidelity,
  cellCeiling: number = MAX_HEATMAP_CELLS,
): Grid {
  const maxColumns = maxColumnsPerRow(rows.length, cellCeiling)
  const planned = rows.map((row) => planRow(row, maxColumns, fidelity))

  let cellCount = 0
  let aggregationFactor = 1
  let anyAggregated = false
  let undefinedCellCount = 0
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY

  for (const row of planned) {
    cellCount += row.cells.length
    aggregationFactor = Math.max(aggregationFactor, row.aggregationFactor)
    for (const cell of row.cells) {
      if (cell.aggregated) anyAggregated = true
      if (cell.relativeError === null) {
        undefinedCellCount += 1
      } else {
        min = Math.min(min, cell.relativeError)
        max = Math.max(max, cell.relativeError)
      }
    }
  }

  return {
    rows: planned,
    cellCount,
    cellCeiling,
    aggregationFactor,
    anyAggregated,
    fidelity,
    domain: Number.isFinite(min) ? { min, max } : null,
    undefinedCellCount,
  }
}

/**
 * The colour ramp.
 *
 * Chosen for **strictly decreasing relative luminance**, which is what makes
 * the map readable in greyscale and in print. The property is asserted by test
 * rather than assumed, including a minimum step between neighbouring tiers.
 */
const RAMP = ['#ffffcc', '#ffeda0', '#fed976', '#fd8d3c', '#e31a1c', '#800026'] as const

/** Ordinal glyphs — a third channel, for a monochrome terminal or a printout. */
const GLYPHS = ['·', ':', '+', '=', '#', '█'] as const

/** Neither a low value nor a high one: no measurement at all. */
const UNDEFINED_COLOUR = '#d0d0d8'
const UNDEFINED_GLYPH = '?'

/**
 * Measured, but with nothing to rank it against.
 *
 * Off the ramp on purpose. Its luminance is asserted to sit clear of every
 * tier's and of the undefined grey, so "every cell measured the same", "this
 * cell is the worst on the map" and "this cell was not measured" remain three
 * visibly different statements in greyscale as well as in colour.
 */
const UNIFORM_COLOUR = '#9ecae1'
const UNIFORM_GLYPH = '≡'

export type Encoding = {
  defined: boolean
  /**
   * True when the value is measured but the map has no spread to place it in.
   * Colour, fill and glyph all encode position within the visible range; when
   * that range is a single point there is no position, and claiming one — the
   * darkest tier, or the lightest — asserts a ranking taken from one sample.
   */
  uniform: boolean
  normalised: number | null
  /** The redundant channel: area, readable with no colour at all. */
  fillFraction: number
  tier: number | null
  glyph: string
  colour: string
  greyscale: string
}

/** Place one value on the ramp, or declare that it has no place there. */
export function encodeMagnitude(
  value: number | null,
  domain: { min: number; max: number } | null,
): Encoding {
  if (value === null || domain === null) {
    return {
      defined: false,
      uniform: false,
      normalised: null,
      fillFraction: 0,
      tier: null,
      glyph: UNDEFINED_GLYPH,
      colour: UNDEFINED_COLOUR,
      greyscale: greyscaleOf(UNDEFINED_COLOUR),
    }
  }

  const span = domain.max - domain.min
  if (!(span > 0)) {
    // Single-valued map. Every cell measured the same, so no cell is worse than
    // another and none is drawn as if it were.
    return {
      defined: true,
      uniform: true,
      normalised: null,
      fillFraction: 0,
      tier: null,
      glyph: UNIFORM_GLYPH,
      colour: UNIFORM_COLOUR,
      greyscale: greyscaleOf(UNIFORM_COLOUR),
    }
  }

  const normalised = clamp01((value - domain.min) / span)
  const tier = Math.min(MAGNITUDE_TIERS - 1, Math.floor(normalised * MAGNITUDE_TIERS))
  const colour = RAMP[tier]

  return {
    defined: true,
    uniform: false,
    normalised,
    fillFraction: normalised,
    tier,
    glyph: GLYPHS[tier],
    colour,
    greyscale: greyscaleOf(colour),
  }
}

/** WCAG relative luminance of an `#rrggbb` colour, 0 (black) to 1 (white). */
export function relativeLuminance(colour: string): number {
  const [r, g, b] = channelsOf(colour).map(toLinear)
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** The equal-channel grey with the same relative luminance as `colour`. */
export function greyscaleOf(colour: string): string {
  const level = Math.round(fromLinear(relativeLuminance(colour)) * 255)
  const hex = level.toString(16).padStart(2, '0')
  return `#${hex}${hex}${hex}`
}

function channelsOf(colour: string): [number, number, number] {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(colour)
  if (match === null) throw new Error(`not an #rrggbb colour: ${colour}`)
  return [
    Number.parseInt(match[1], 16) / 255,
    Number.parseInt(match[2], 16) / 255,
    Number.parseInt(match[3], 16) / 255,
  ]
}

function toLinear(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
}

function fromLinear(value: number): number {
  const v = clamp01(value)
  return v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value))
}
