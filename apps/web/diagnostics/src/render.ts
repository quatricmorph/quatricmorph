/**
 * One draw plan, two outputs.
 *
 * `surfaceToSvg` writes a deterministic SVG — the evidence artifact a reviewer
 * opens — and `paintHeatmap` paints the same plan onto a 2-D canvas, which is
 * what the browser runs. A test asserts the two pick the same colour for the
 * same cell, so the artifact stays evidence about the browser rather than a
 * separate drawing made for the screenshot.
 *
 * Deliberately a 2-D canvas and nothing more. The program boundary for this
 * package excludes Cesium, a Three.js scene graph, binary tile formats and tile
 * traversal, and `src/__tests__/boundary.test.ts` enforces that by scanning
 * these sources.
 *
 * Magnitude is drawn three times over: as a fill colour, as a bottom-anchored
 * bar whose height is the normalised value, and as an ordinal glyph. Two of the
 * three survive greyscale, and all three survive a reader who cannot
 * distinguish the hues.
 */

import { cellFidelityOf, encodeMagnitude, type Cell, type Grid } from './heatmap.js'
import type { Refusal } from './manifest-client.js'
import type { Surface } from './app.js'

/** The opening of a heat-map cell rectangle. Tests count and read these. */
export const CELL_RECT_MARKER = '<rect class="cell"'

/**
 * The opening of the mark drawn on a cell whose *number* is coarse (`QM-0153`).
 *
 * A filled triangular wedge in the cell's top-right corner. Deliberately a
 * different kind of mark from the aggregation dash — a solid shape against a
 * broken outline — because the two say different things and appear together on
 * the same cell whenever a sampled run is also merged for display.
 */
export const SAMPLED_MARK_MARKER = '<path class="sampled-mark"'

export type Palette = 'colour' | 'greyscale'

export type SvgOptions = {
  palette: Palette
  selected?: { layerIndex: number | null; columnIndex: number }
}

export type PaintOptions = {
  palette: Palette
  width: number
  height: number
  selected?: { layerIndex: number | null; columnIndex: number }
}

/** What a `CanvasRenderingContext2D` was told to do. Used by the painter's tests. */
export type PaintOp = {
  op: 'fillRect' | 'strokeRect' | 'clearRect'
  x: number
  y: number
  w: number
  h: number
  style: string
}

const WIDTH = 960
const MIN_GUTTER = 150
const MAX_GUTTER = 420
/** Advance width of one monospace character, as a fraction of the font size. */
const CHARACTER_ADVANCE = 0.62
const ROW_HEIGHT = 22
const ROW_GAP = 3
const LINE_HEIGHT = 17

const INK = '#111827'
const MUTED = '#4b5563'
const RULE = '#9ca3af'
const SELECTION_STROKE = '#111827'
/**
 * The aggregation mark, in one place.
 *
 * Used by the cells and by the legend swatch that describes them, so the key
 * cannot come to show a different mark from the one the map draws. The legend's
 * words name it too (`app.ts`, `buildLegend`).
 */
const AGGREGATED_DASH = '3 2'
/** The longest side of the sampled wedge, in pixels, before it is clipped to the cell. */
const SAMPLED_WEDGE = 6

/**
 * The wedge that marks an engine-side-coarse cell.
 *
 * A shape rather than a colour, so it is the same mark under both palettes and
 * survives a greyscale print. Its ink flips to white over the dark end of the
 * ramp for the same reason the magnitude glyph's does — a near-black wedge on a
 * near-black fill is a mark that is present in the file and absent to the eye.
 */
function sampledWedge(
  x: number,
  y: number,
  width: number,
  height: number,
  tier: number | null,
  where: 'cell' | 'legend',
): string {
  const size = Math.max(1, Math.min(SAMPLED_WEDGE, width, height))
  const right = x + width
  const path = `M ${n(right - size)} ${n(y)} L ${n(right)} ${n(y)} L ${n(right)} ${n(y + size)} Z`
  const fill = (tier ?? 0) >= 4 ? '#ffffff' : INK
  return `${SAMPLED_MARK_MARKER} data-in="${where}" d="${path}" fill="${fill}"/>`
}

/** A cell's geometry inside the plot area, in the order rows and columns appear. */
type Placed = { cell: Cell; x: number; y: number; width: number; height: number }

function placeCells(grid: Grid, plotWidth: number, rowHeight: number, rowGap: number, top: number): Placed[] {
  const placed: Placed[] = []
  grid.rows.forEach((row, rowIndex) => {
    const y = top + rowIndex * (rowHeight + rowGap)
    const widths = columnWidths(row.cells, plotWidth)
    let x = 0
    row.cells.forEach((cell, columnIndex) => {
      placed.push({ cell, x, y, width: widths[columnIndex], height: rowHeight })
      x += widths[columnIndex]
    })
  })
  return placed
}

/**
 * Column widths.
 *
 * Proportional to the channels a cell covers when the manifest publishes an
 * extent for every cell in the row, so a wide tensor looks wide. Equal widths
 * when it does not — guessing a width would be drawing a number that was never
 * measured.
 */
function columnWidths(cells: readonly Cell[], plotWidth: number): number[] {
  if (cells.length === 0) return []
  const extents = cells.map((cell) => cell.channelsPerCell)
  const total = extents.reduce<number>((sum, extent) => sum + (extent ?? 0), 0)
  if (extents.some((extent) => extent === null) || total <= 0) {
    return cells.map(() => plotWidth / cells.length)
  }
  return extents.map((extent) => (plotWidth * (extent as number)) / total)
}

function fillOf(cell: Cell, grid: Grid, palette: Palette): string {
  const encoding = encodeMagnitude(cell.relativeError, grid.domain)
  return palette === 'greyscale' ? encoding.greyscale : encoding.colour
}

function swatchOf(entry: { colour: string; greyscale: string }, palette: Palette): string {
  return palette === 'greyscale' ? entry.greyscale : entry.colour
}

function isSelected(cell: Cell, selected: SvgOptions['selected']): boolean {
  return (
    selected !== undefined &&
    selected.layerIndex === cell.layerIndex &&
    selected.columnIndex === cell.columnIndex
  )
}

/** Fixed-precision so the artifact is byte-stable across runs and machines. */
function n(value: number): string {
  return value.toFixed(2)
}

export function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;')
}

/** The inverse of `escapeXml`. `&amp;` last, so `&amp;lt;` does not become `<`. */
export function unescapeXml(text: string): string {
  return text
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&')
}

/**
 * Every line of text a rendered SVG puts in front of a reader.
 *
 * Exists so the forbidden-vocabulary guard can be run over what is *drawn*, not
 * only over the view-model. Roughly half the words on the page are literals in
 * this file — the headings, `not measured`, the whole of `refusalToSvg` — and
 * `surfaceStrings()` never sees any of them. Those are also the strings most
 * likely to be edited by hand.
 */
export function svgTextContent(svg: string): string[] {
  return [...svg.matchAll(/<text\b[^>]*>([^<]*)<\/text>/g)].map((match) => unescapeXml(match[1]))
}

function text(x: number, y: number, content: string, options: { fill?: string; size?: number } = {}): string {
  const fill = options.fill ?? INK
  const size = options.size ?? 12
  return `<text x="${n(x)}" y="${n(y)}" font-family="ui-monospace, monospace" font-size="${size}" fill="${fill}">${escapeXml(content)}</text>`
}

/** Characters that fit between `x` and the right margin at `size`. */
function columnsAt(x: number, size: number): number {
  return Math.max(20, Math.floor((WIDTH - x - 20) / (size * CHARACTER_ADVANCE)))
}

/**
 * How much of the page the row labels need.
 *
 * Sized to the longest label rather than fixed, because a label the plot draws
 * over is a label the reader cannot read — and one of these labels carries the
 * fidelity of the row beside it.
 */
function gutterFor(labels: readonly string[]): number {
  const longest = labels.reduce((n, label) => Math.max(n, label.length), 0)
  return Math.min(MAX_GUTTER, Math.max(MIN_GUTTER, 20 + longest * 12 * CHARACTER_ADVANCE + 10))
}

/**
 * Emit one line of copy, wrapped to the page.
 *
 * Text that runs off the right edge of the image is text the reader does not
 * have. Every caveat this surface is required to display has to fit.
 */
function emit(
  parts: string[],
  x: number,
  y: number,
  content: string,
  options: { fill?: string; size?: number } = {},
): number {
  const size = options.size ?? 12
  let cursor = y
  for (const line of wrap(content, columnsAt(x, size))) {
    parts.push(text(x, cursor, line, options))
    cursor += LINE_HEIGHT
  }
  return cursor
}

/** Render the whole surface. Deterministic: same surface in, same bytes out. */
export function surfaceToSvg(surface: Surface, options: SvgOptions): string {
  const grid = surface.heatmap.grid
  const parts: string[] = []
  let y = 24

  y = emit(parts, 20, y, `Quantisation-error heat-map — ${surface.modelId} @ ${surface.revisionHash}`, {
    size: 14,
  })
  y = emit(
    parts,
    20,
    y,
    `run ${surface.runId} · ${surface.precision} · backend ${surface.backend} · architecture ${surface.architecture} (${surface.resolverConfidence})`,
    { fill: MUTED },
  )
  // The fidelity label, in the surface itself, before any number is shown.
  y = emit(parts, 20, y, `every value below is ${surface.fidelityLabel.toUpperCase()}`, { size: 13 })
  y = emit(parts, 20, y, `drill: ${surface.drill.path.join(' > ')}`, { fill: MUTED })
  y = emit(parts, 20, y, surface.heatmap.resolutionNote, { fill: MUTED })
  y += 6

  const plotTop = y
  if (surface.heatmap.empty !== null) {
    y = emit(parts, 20, y, surface.heatmap.empty.explanation)
    for (const id of surface.heatmap.empty.requirementIds) {
      y = emit(parts, 20, y, `see refusal ${id}`, { fill: MUTED })
    }
  } else {
    // The fidelity word beside every row, not only in the header: a reader who
    // looks at one row must still be told how the number was obtained.
    const labels = grid.rows.map((row) => `${row.label} · ${grid.fidelity}`)
    const gutter = gutterFor(labels)
    labels.forEach((label, rowIndex) => {
      const rowY = plotTop + rowIndex * (ROW_HEIGHT + ROW_GAP)
      parts.push(text(20, rowY + ROW_HEIGHT - 7, label, { fill: MUTED }))
    })
    for (const placed of placeCells(grid, WIDTH - gutter - 24, ROW_HEIGHT, ROW_GAP, plotTop)) {
      parts.push(...cellSvg(placed, grid, options, gutter))
    }
    y = plotTop + grid.rows.length * (ROW_HEIGHT + ROW_GAP) + 10
  }

  y += LINE_HEIGHT
  parts.push(text(20, y, 'Legend', { size: 13 }))
  y += LINE_HEIGHT
  y = emit(parts, 20, y, surface.heatmap.legend.encodes, { fill: MUTED })
  y = emit(parts, 20, y, surface.heatmap.legend.notAClaim, { fill: MUTED })
  y = emit(parts, 20, y, surface.heatmap.legend.aggregationNote, { fill: MUTED })
  y = emit(parts, 20, y, surface.heatmap.legend.fidelityNote, { fill: MUTED })
  y = emit(parts, 20, y, surface.heatmap.legend.scaleNote, { fill: MUTED })
  for (const entry of surface.heatmap.legend.entries) {
    // The aggregation entry's swatch carries the same dash the aggregated
    // cells carry, so the key shows the mark its words name.
    const dash = entry.kind === 'aggregated' ? AGGREGATED_DASH : 'none'
    parts.push(
      `<rect class="swatch" x="20.00" y="${n(y - 10)}" width="14.00" height="12.00" fill="${swatchOf(entry, options.palette)}" stroke="${RULE}" stroke-width="0.5" stroke-dasharray="${dash}"/>`,
    )
    // And the sampled entry's swatch carries the wedge, for the same reason.
    if (entry.kind === 'engine-coarse') {
      parts.push(sampledWedge(20, y - 10, 14, 12, 0, 'legend'))
    }
    parts.push(text(40, y, `${entry.glyph}  ${entry.label}`, { fill: MUTED }))
    y += LINE_HEIGHT
  }

  y += 6
  parts.push(text(20, y, 'Ranked by relative weight-space error', { size: 13 }))
  y += LINE_HEIGHT
  y = emit(parts, 20, y, surface.ranking.caveat, { fill: MUTED })
  if (surface.ranking.empty !== null) {
    y = emit(parts, 20, y, surface.ranking.empty.explanation, { fill: MUTED })
  }
  for (const row of surface.ranking.rows) {
    y = emit(
      parts,
      20,
      y,
      `${row.rank}. ${row.address} — ${row.relativeError} (${row.parameterCount} params) [${row.fidelityLabel}]`,
    )
  }

  y += 6
  parts.push(text(20, y, 'Mixed-precision frontier', { size: 13 }))
  y += LINE_HEIGHT
  y = emit(parts, 20, y, surface.frontier.claim, { fill: MUTED })
  if (surface.frontier.empty !== null) {
    y = emit(parts, 20, y, surface.frontier.empty.explanation, { fill: MUTED })
  }
  for (const row of surface.frontier.rows) {
    y = emit(
      parts,
      20,
      y,
      `${row.step}. keep ${row.keepSetSize} tensor(s) — +${row.addedBytes} bytes, ${row.errorRemovedFraction} of squared error removed [${row.fidelityLabel}]`,
    )
  }

  if (surface.experts !== null) {
    y += 6
    parts.push(text(20, y, 'Experts', { size: 13 }))
    y += LINE_HEIGHT
    for (const row of surface.experts.rows) {
      y = emit(
        parts,
        20,
        y,
        `layer ${row.layerIndex} expert ${row.expertIndex} — ${row.relativeError ?? 'not measured'} [${row.fidelityLabel}]`,
      )
    }
  }

  y += 6
  parts.push(text(20, y, 'Declared gaps', { size: 13 }))
  y += LINE_HEIGHT
  if (surface.refusals.empty !== null) {
    y = emit(parts, 20, y, surface.refusals.empty.explanation, { fill: MUTED })
  }
  for (const row of surface.refusals.rows) {
    y = emit(parts, 20, y, `${row.requirementId} — ${row.what}`)
    y = emit(parts, 36, y, row.why, { fill: MUTED })
  }

  y += 6
  for (const caveat of surface.caveats) {
    y = emit(parts, 20, y, caveat, { fill: MUTED })
  }

  const height = y + 16
  return document(WIDTH, height, parts)
}

function cellSvg(placed: Placed, grid: Grid, options: SvgOptions, gutter: number): string[] {
  const { cell, x, y, width, height } = placed
  const encoding = encodeMagnitude(cell.relativeError, grid.domain)
  const left = gutter + x
  const selected = isSelected(cell, options.selected)
  const barHeight = height * encoding.fillFraction

  const cellFidelity = cellFidelityOf(cell)

  const attributes = [
    `data-layer="${cell.layerIndex === null ? 'none' : cell.layerIndex}"`,
    `data-column="${cell.columnIndex}"`,
    // The manifest's own word, unchanged: `approximate` is never relabelled
    // `sampled` just because the two share a mark.
    `data-fidelity="${cell.fidelity}"`,
    `data-cell-fidelity="${cellFidelity.kind}"`,
    `data-aggregated="${cell.aggregated}"`,
    `data-defined="${encoding.defined}"`,
    `data-fill-fraction="${encoding.fillFraction}"`,
    `data-selected="${selected}"`,
    `data-glyph="${escapeXml(encoding.glyph)}"`,
  ].join(' ')

  const svg = [
    // A dashed border is the persistent aggregation marker: legible without
    // hovering, and it survives greyscale because it is not a colour.
    `${CELL_RECT_MARKER} ${attributes} x="${n(left)}" y="${n(y)}" width="${n(width)}" height="${n(height)}" fill="${fillOf(cell, grid, options.palette)}" stroke="${selected ? SELECTION_STROKE : RULE}" stroke-width="${selected ? '3' : '0.5'}" stroke-dasharray="${cell.aggregated ? AGGREGATED_DASH : 'none'}"/>`,
    // The second redundant channel: a bar whose height is the value, readable
    // with no colour at all.
    `<rect class="bar" x="${n(left + width * 0.25)}" y="${n(y + height - barHeight)}" width="${n(width * 0.5)}" height="${n(barHeight)}" fill="${INK}" stroke="none"/>`,
  ]

  // The engine's coarseness, marked on every cell it applies to and not only in
  // the header: a reader looking at one cell must be able to see that its
  // number was not computed over every element.
  if (cellFidelity.kind === 'sampled') {
    svg.push(sampledWedge(left, y, width, height, encoding.tier, 'cell'))
  }

  // The third redundant channel: an ordinal glyph, where the cell is wide
  // enough to carry one. Its ink flips against dark fills so it stays readable.
  if (width >= 14 && height >= 12) {
    svg.push(
      text(left + 3, y + height - 6, encoding.glyph, {
        fill: (encoding.tier ?? 0) >= 4 ? '#ffffff' : INK,
        size: 11,
      }),
    )
  }

  return svg
}

function document(width: number, height: number, parts: string[]): string {
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${n(height)}" viewBox="0 0 ${width} ${n(height)}">`,
    `<rect class="page" x="0" y="0" width="${width}" height="${n(height)}" fill="#ffffff"/>`,
    ...parts,
    '</svg>',
    '',
  ].join('\n')
}

/**
 * The refusal state.
 *
 * No cells, no placeholder grid, no zeros. A surface that cannot describe real
 * data says why and stops — a plausible-looking picture drawn from a manifest
 * this build cannot read is worse than no picture.
 */
export function refusalToSvg(refusal: Refusal): string {
  const parts: string[] = []
  let y = 28
  parts.push(text(20, y, 'Nothing was rendered.', { size: 15 }))
  y += LINE_HEIGHT + 4
  parts.push(text(20, y, `reason: ${refusal.kind}`, { size: 13 }))
  y += LINE_HEIGHT
  for (const line of wrap(refusal.message, 96)) {
    parts.push(text(20, y, line, { fill: MUTED }))
    y += LINE_HEIGHT
  }
  if (refusal.kind === 'unsupported_version') {
    parts.push(
      text(20, y, `manifest_version found: ${refusal.found} · manifest_version supported: ${refusal.supported}`),
    )
    y += LINE_HEIGHT
  }
  if (refusal.kind === 'declared_gap') {
    parts.push(text(20, y, `declared gap: ${refusal.requirement} — look it up in STATUS.md`, { fill: MUTED }))
    y += LINE_HEIGHT
  }
  if (refusal.kind === 'schema_invalid') {
    for (const error of refusal.errors.slice(0, 8)) {
      parts.push(text(20, y, `${error.path || '/'} — ${error.keyword}: ${error.message}`, { fill: MUTED }))
      y += LINE_HEIGHT
    }
  }
  if (refusal.kind === 'transport_failure' && refusal.retryable) {
    parts.push(text(20, y, 'Retry', { size: 13 }))
    y += LINE_HEIGHT
  }
  parts.push(
    text(20, y, 'No placeholder data is shown: a plausible picture drawn from an unreadable manifest', {
      fill: MUTED,
    }),
  )
  y += LINE_HEIGHT
  parts.push(text(20, y, 'is worse than no picture.', { fill: MUTED }))
  y += LINE_HEIGHT

  return document(WIDTH, y + 16, parts)
}

function wrap(message: string, columns: number): string[] {
  const words = message.split(/\s+/)
  const lines: string[] = []
  let line = ''
  for (const word of words) {
    if (line.length > 0 && line.length + 1 + word.length > columns) {
      lines.push(line)
      line = word
    } else {
      line = line.length === 0 ? word : `${line} ${word}`
    }
  }
  if (line.length > 0) lines.push(line)
  return lines
}

/** Paint the same plan onto a 2-D canvas. */
export function paintHeatmap(
  context: CanvasRenderingContext2D,
  surface: Surface,
  options: PaintOptions,
): void {
  const grid = surface.heatmap.grid
  context.clearRect(0, 0, options.width, options.height)
  if (grid.rows.length === 0 || grid.cellCount === 0) return

  const rowHeight = options.height / grid.rows.length
  const placed = placeCells(grid, options.width, rowHeight, 0, 0)

  for (const item of placed) {
    const encoding = encodeMagnitude(item.cell.relativeError, grid.domain)
    context.fillStyle = fillOf(item.cell, grid, options.palette)
    context.fillRect(item.x, item.y, item.width, item.height)

    const barHeight = item.height * encoding.fillFraction
    context.fillStyle = INK
    context.fillRect(
      item.x + item.width * 0.25,
      item.y + item.height - barHeight,
      item.width * 0.5,
      barHeight,
    )

    if (isSelected(item.cell, options.selected)) {
      context.strokeStyle = SELECTION_STROKE
      context.lineWidth = 3
      context.strokeRect(item.x, item.y, item.width, item.height)
    }
  }
}
