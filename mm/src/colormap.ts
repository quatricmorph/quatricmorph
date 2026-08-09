"use strict"

//
// The heatmap colour ramp and the value -> texel encoding, as pure arithmetic.
//
// Nothing here builds a THREE object or touches a GPU. That is the point: the
// heatmap fragment shader does not reimplement any of this, it samples a
// 256-entry lookup texture that `colormapLUT()` fills. So the colours a test
// asserts and the colours on screen come from one implementation, and a test
// can run under jsdom.
//
// Two encodings, and the status bar says which is live, because they are
// different claims about what the picture means:
//
//   'magnitude'  |x| through the seven-stop ramp. The ramp is *sequential* --
//                one monotone light ramp -- and mm's data is signed, so a
//                sequential ramp cannot carry a sign. Mapping |x| says so
//                honestly instead of implying a sign that is not shown. This
//                is the default.
//
//   'signed'     mm's own element encoding: hue by sign, lightness by
//                magnitude. The requirement offered "the existing hue encoding"
//                as the signed option and that is what this is -- reusing it
//                rather than inventing a diverging palette keeps heatmap mode
//                and elements mode directly comparable, and it is the only one
//                of the two from which the sign of a cell can be read.
//
// Colour space. Every triple here is **sRGB bytes** -- what a colour picker
// reports, and what must land in the framebuffer. See the long note in
// points.ts: the element shader ends with `colorSpaceToWorking(..., SRGB)` to
// cancel WebGPU's frame-wide output conversion, and the heatmap shader does the
// same, so the bytes written here are the bytes displayed. `elementHSL` below
// preserves the same upstream colour-management quirk, because it is the exact
// arithmetic `Mat.colorFromData` has always run.
//

// The ramp, low -> high, authoritative as written.
export const COLORMAP_STOPS = [
  0x03051A, 0x501D4C, 0xB41658, 0xDD2C45, 0xF16445, 0xF59970, 0xFAEBDD,
]

export const HEATMAP_ENCODINGS = ['magnitude', 'signed']

// Reduction operators for the mip ladder. See `reduceBlock` in heatmap.ts for
// why the default is not `mean`.
export const HEATMAP_REDUCERS = ['maxAbs', 'mean']

const clamp01 = (x: number) => x < 0 ? 0 : x > 1 ? 1 : x

/**
 * The ramp at `t` in [0, 1], as sRGB bytes.
 *
 * Stops are evenly spaced (t = k/6) and interpolated linearly **in sRGB byte
 * space** -- the hex values above are sRGB, so interpolating them anywhere else
 * would move the stops themselves off the specified colours.
 */
export function colormapSRGB(t: number): [number, number, number] {
  const n = COLORMAP_STOPS.length
  const u = clamp01(t) * (n - 1)
  const i = Math.min(n - 2, Math.floor(u))
  const f = u - i
  const [a, b] = [COLORMAP_STOPS[i], COLORMAP_STOPS[i + 1]]
  const ch = (shift: number) => {
    const x = (a >> shift) & 0xFF
    const y = (b >> shift) & 0xFF
    return x + (y - x) * f
  }
  return [ch(16), ch(8), ch(0)]
}

/**
 * mm's existing element encoding, as an (h, s, l) triple -- the arithmetic
 * lifted verbatim out of `Mat.colorFromData` so that the LUT and the element
 * path cannot drift. Returns null for the two cases colorFromData special-cases
 * to a flat colour (0 -> black, non-finite -> white).
 *
 * `range` is `Mat.getRangeInfo()`'s output; `viz` is params.viz.
 */
export function elementHSL(x, range, viz): { h: number, s: number, l: number } | null {
  if (x === undefined || isNaN(x) || x === 0 || Math.abs(x) === Infinity) {
    return null
  }
  const { absmin, absmax, absdiff } = range

  // boundary violations can happen in intermediates
  const absx = Math.min(absmax, Math.max(absmin, Math.abs(x)))

  const hue_vol = absdiff <= 0 ? 0 : (x - Math.sign(x) * absmin) / absdiff
  const gap = viz['hue gap'] * Math.sign(x)
  const hue = (viz['zero hue'] + gap + (hue_vol * viz['hue spread'])) % 1

  const min_light = Math.max(viz['min light'], 0.00001)
  const max_light = Math.max(viz['max light'], min_light)
  const lrange = max_light - min_light
  const light_vol = absdiff <= 0 ? 0 : (absx - absmin)
  const light = min_light + lrange * Math.sqrt(light_vol) / Math.sqrt(absdiff)

  return { h: hue, s: 1.0, l: light }
}

//
// value <-> texel index
//
// One byte per element for the visual channel. FP32 is never uploaded: a
// 768x3072 weight is 9.4 MB as R32F and 2.4 MB as R8, and the exact value is
// still on the CPU in the Mat's Array2D, which is what the hover readout and
// the labels read. Visual fidelity is not numerical fidelity, and the readout
// must show the checkpoint's own number, never the quantized texel.
//

/** The texel byte for a value, under `enc`. Inverse of `indexValue`. */
export function texelIndex(x: number, enc: string, absmin: number, absmax: number): number {
  if (x === undefined || isNaN(x)) return 0
  if (enc === 'signed') {
    // [-absmax, +absmax] -> [0, 255]; zero lands mid-ramp. Chosen over an
    // (magnitude, sign) pair because one channel keeps the upload at 1 byte.
    if (!(absmax > 0)) return 128
    return Math.max(0, Math.min(255, Math.round(255 * (x + absmax) / (2 * absmax))))
  }
  const absdiff = absmax - absmin
  if (!(absdiff > 0)) return 0
  return Math.max(0, Math.min(255, Math.round(255 * (Math.abs(x) - absmin) / absdiff)))
}

/** The value a texel byte stands for -- the ramp's own x axis. */
export function indexValue(b: number, enc: string, absmin: number, absmax: number): number {
  if (enc === 'signed') {
    return absmax > 0 ? (b / 255) * 2 * absmax - absmax : 0
  }
  return absmin + (b / 255) * (absmax - absmin)
}

/**
 * The 256-entry lookup the shader samples, as RGBA bytes.
 *
 * Rebuilt per matrix rather than shared: 1 KB is nothing next to the value
 * texture, and it lets the 'signed' entries carry this matrix's own absmin /
 * absmax / viz settings instead of a normalized approximation of them.
 *
 * `toRGB` converts an (h, s, l) triple to bytes. It is injected so this module
 * stays free of THREE -- viz.ts passes THREE.Color's own setHSL, which is what
 * the element path uses, so the two agree by construction.
 */
export function colormapLUT(
  enc: string, absmin: number, absmax: number, viz: any,
  toRGB: (h: number, s: number, l: number) => [number, number, number],
): Uint8Array {
  const lut = new Uint8Array(256 * 4)
  const absdiff = absmax - absmin
  for (let b = 0; b < 256; b++) {
    let rgb: [number, number, number]
    if (enc === 'signed') {
      const hsl = elementHSL(indexValue(b, enc, absmin, absmax), { absmin, absmax, absdiff }, viz)
      rgb = hsl ? toRGB(hsl.h, hsl.s, hsl.l) : [0, 0, 0]
    } else {
      rgb = colormapSRGB(b / 255)
    }
    lut[b * 4 + 0] = Math.round(rgb[0])
    lut[b * 4 + 1] = Math.round(rgb[1])
    lut[b * 4 + 2] = Math.round(rgb[2])
    lut[b * 4 + 3] = 255
  }
  return lut
}

/** `#RRGGBB` for a ramp position -- used by the colorbar in the page chrome. */
export function colormapHex(t: number): string {
  const [r, g, b] = colormapSRGB(t).map(x => Math.round(x))
  return '#' + [r, g, b].map(x => x.toString(16).padStart(2, '0')).join('')
}
