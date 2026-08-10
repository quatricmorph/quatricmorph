"use strict"

//
// Heatmap render mode: the arithmetic half.
//
// Nothing in this file imports THREE. The mesh, the textures and the TSL
// shading live in ./heatmapmesh.ts; everything that decides *which cell is
// where*, *which value a texel carries* and *how much resolution a matrix is
// allowed* lives here, as pure functions, so it is reachable from a jsdom test
// with no GPU. Every one of those decisions is invisible when it goes wrong --
// a transposed heatmap, an off-by-one pick, a mip level that averaged away the
// outlier someone is looking for -- so none of them may live only in a shader.
//
// The other render path is ./points.ts: one instanced sphere-impostor quad per
// element. This one is a single quad per block of the matrix, with the matrix
// itself as a texture. Both draw the same cells in the same places; that
// agreement is asserted in test/points.test.ts, not left to the eye.
//

// What the GUI and the page's `Render` selector offer. 'spheres' is the
// original element path -- one shaded sphere impostor per element -- named for
// what it looks like rather than for what the code calls it internally. The
// older value 'elements' is still accepted (see pickRenderMode) so a params
// tree or a deep link written against it keeps working.
export const RENDER_MODES = ['auto', 'spheres', 'heatmap']

//
// budgets
//
// One named block, with the reasoning next to each number, in the style of
// BALL_R / AMBIENT in points.ts. These are the only magic numbers in the
// heatmap path; nothing below picks a threshold of its own.
//

/**
 * Above this element count, `auto` picks heatmap.
 *
 * 65,536 is 256x256. The sphere impostor stops reading as a sphere below about
 * four pixels across, so on a 1080-pixel-tall viewport a matrix drawn edge to
 * edge can show at most ~270x270 elements *as spheres*. Past that the shading
 * is spent on subpixel detail and the per-element CPU cost -- two attribute
 * writes and a label check per element, per animation bump -- buys nothing.
 * gpt2/index.html already reasons this way when it caps attention at 64x64.
 */
export const HEATMAP_MIN_ELEMENTS = 1 << 16

/**
 * Texels a single matrix may hold, before LOD reduction is forced.
 *
 * 262,144 is 512x512. At 2 bytes per texel (value + state, see heatmapmesh.ts)
 * that is 512 KB per matrix, and it is already more texels than a 1080p
 * viewport can show for one matrix among many. Raising it costs upload
 * bandwidth and VRAM linearly and shows nothing.
 */
export const HEATMAP_TEXEL_BUDGET = 1 << 18

/**
 * Texels the whole scene may hold. 8,388,608 = 16 MB at 2 bytes per texel.
 *
 * This is what makes the whole-model view possible at all: distilgpt2's six
 * blocks are 42.5M weights and the tied-wte logits stage another 38.6M, which
 * is 162 MB of texture at LOD 0 and ~170M vertices as instanced quads. The
 * scene budget is divided among the matrices actually in the tree, so adding a
 * stage reduces every other stage's resolution rather than growing the upload.
 */
export const HEATMAP_SCENE_TEXEL_BUDGET = 1 << 23

/** Never reduce more than this per axis; past it a matrix is a smear. */
export const LOD_MAX_FACTOR = 64

//
// mode selection
//

/**
 * Which element-render path a matrix gets.
 *
 * `override` is `viz['render mode']`: 'spheres' and 'heatmap' are taken at
 * their word -- that is what an override is for, and a control that quietly
 * declined to do what it says would be worse than no control -- while 'auto'
 * decides on element count.
 *
 * Returns 'spheres' or 'heatmap', never 'auto'.
 */
export function pickRenderMode(h: number, w: number, override: string): string {
  // 'elements' is the pre-rename spelling of 'spheres'.
  if (override === 'spheres' || override === 'elements') return 'spheres'
  if (override === 'heatmap') return 'heatmap'
  return h * w >= HEATMAP_MIN_ELEMENTS ? 'heatmap' : 'spheres'
}

//
// LOD
//

/**
 * The reduction factor for a matrix, as a power of two.
 *
 * Two independent bounds, both honest, whichever binds harder:
 *
 *   - `maxTexels`   the memory budget above.
 *   - `screenPx`    the viewport's larger dimension in physical pixels. A
 *                   matrix can never show more texels along an axis than the
 *                   viewport has pixels, whatever the camera is doing.
 *
 * The screen bound is deliberately the *viewport*, not the matrix's live
 * projected size. A live footprint would give a tighter ladder, but it changes
 * every time the camera moves, and re-uploading a 512 KB texture on camera
 * motion is worse than showing more texels than are needed. So a matrix that
 * is currently small on screen may be carrying more resolution than it needs.
 * It is never carrying less than the bound allows, and the level is printed.
 */
export function chooseLodFactor(
  h: number, w: number,
  maxTexels: number = HEATMAP_TEXEL_BUDGET,
  screenPx: number = Infinity,
): number {
  let f = 1
  const ok = () => {
    const rh = Math.ceil(h / f), rw = Math.ceil(w / f)
    return rh * rw <= maxTexels && Math.max(rh, rw) <= screenPx
  }
  while (!ok() && f < LOD_MAX_FACTOR) f *= 2
  return f
}

/** Reduced dimensions at a factor. */
export const lodSize = (h: number, w: number, f: number) =>
  ({ h: Math.ceil(h / f), w: Math.ceil(w / f) })

/**
 * The value one reduced texel stands for.
 *
 * Ordinary mipmapping averages, and averaging is wrong for this data:
 * [0.001, 0.003, 9.830, 0.001] reduces to 2.459 under `mean`, which erases the
 * one number an inspector was looking for. `maxAbs` keeps the largest-magnitude
 * element **with its sign**, so an outlier survives every level of the ladder
 * and a signed encoding still reads. `mean` is offered because it is the right
 * operator for asking "what is the typical size here", and it is never the
 * default.
 */
export function reduceRegion(
  data: ArrayLike<number>, w: number,
  i0: number, i1: number, j0: number, j1: number, op: string,
): number {
  if (op === 'mean') {
    let sum = 0, n = 0
    for (let i = i0; i < i1; i++) {
      for (let j = j0; j < j1; j++) { sum += data[i * w + j]; n++ }
    }
    return n ? sum / n : 0
  }
  let best = 0, bestabs = -1
  for (let i = i0; i < i1; i++) {
    for (let j = j0; j < j1; j++) {
      const x = data[i * w + j]
      const a = Math.abs(x)
      if (a > bestabs) { bestabs = a; best = x }
    }
  }
  return bestabs < 0 ? 0 : best
}

export function reduceTexel(
  data: ArrayLike<number>, h: number, w: number, f: number,
  ti: number, tj: number, op: string,
): number {
  return reduceRegion(data, w,
    ti * f, Math.min(h, ti * f + f),
    tj * f, Math.min(w, tj * f + f), op)
}

/** The whole reduced matrix. `f == 1` still copies, so callers can own it. */
export function reduceLevel(
  data: ArrayLike<number>, h: number, w: number, f: number, op = 'maxAbs',
): { data: Float32Array, h: number, w: number } {
  const { h: rh, w: rw } = lodSize(h, w, f)
  const out = new Float32Array(rh * rw)
  for (let ti = 0; ti < rh; ti++) {
    for (let tj = 0; tj < rw; tj++) {
      out[ti * rw + tj] = reduceTexel(data, h, w, f, ti, tj, op)
    }
  }
  return { data: out, h: rh, w: rw }
}

//
// geometry
//
// The heatmap must land its texel centres exactly where `emptyPoints` in
// viz.ts lands its element centres. Both are derived from the same block info,
// and the agreement is asserted in test/points.test.ts against the actual
// PointCloud attribute rather than against a restatement of this arithmetic.
//

/**
 * World position of element (i, j) inside a Mat's `inner_group`.
 *
 * Identical by construction to `emptyPoints`: column then row, block index
 * times gap added on each axis.
 */
export function elementPosition(i: number, j: number, info: any) {
  const { i: { size: si }, j: { size: sj }, gap } = info
  return {
    x: j + Math.floor(j / sj) * gap,
    y: i + Math.floor(i / si) * gap,
  }
}

/**
 * The quad for one block: its size, and the centre it is translated to, such
 * that texel centres coincide with element centres.
 *
 * A block covering rows [i0, i0+bh) and columns [j0, j0+bw) occupies
 * x in [x0 - 0.5, x0 + bw - 0.5] where x0 is element (i0, j0)'s own x. So the
 * quad is bw by bh and its centre sits half a cell in from the first element.
 */
export function blockQuad(i0: number, j0: number, bh: number, bw: number, info: any) {
  const { x, y } = elementPosition(i0, j0, info)
  return { w: bw, h: bh, cx: x + bw / 2 - 0.5, cy: y + bh / 2 - 0.5 }
}

/**
 * UV of the centre of texel (a, b) in a bh x bw block.
 *
 * DataTexture sets flipY = false, so data row 0 is at v = 0, which is the
 * quad's -y edge, which is element row 0. Nothing flips anywhere; that is the
 * property the orientation test pins.
 */
export const texelUV = (a: number, b: number, bh: number, bw: number) =>
  ({ u: (b + 0.5) / bw, v: (a + 0.5) / bh })

/**
 * The cell under a UV, in block-local coordinates.
 *
 * Deliberately in terms of the block's *full* height and width, not its
 * reduced texel count: the pick names an element of the matrix, which is what
 * the hover readout must print, and that is independent of the LOD level the
 * texture happens to be at.
 */
export function uvToCell(u: number, v: number, bh: number, bw: number) {
  const b = Math.min(bw - 1, Math.max(0, Math.floor(u * bw)))
  const a = Math.min(bh - 1, Math.max(0, Math.floor(v * bh)))
  return { i: a, j: b }
}

//
// texel state
//
// `Mat.isHidden` decides visibility by comparing an element's colour to black,
// and `initAnimation` hides every result and every input before animating. That
// cannot survive into heatmap mode: the ramp's low stop is #03051A, which is
// near-black rather than black, so under a uint8 texture "hidden" and "lowest
// value" would be one byte apart and the animation would appear to skip cells.
//
// So visibility is its own channel -- the G byte of the value texture -- and
// these are its three states. `bumpColor`, the animation's input highlight, is
// the same hazard for the same reason (adding grey to a ramp colour means
// nothing), so it is a state here too rather than a colour operation.
//
export const TEXEL_HIDDEN = 0
export const TEXEL_SHOWN = 1
export const TEXEL_BUMPED = 2
