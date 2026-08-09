"use strict"

//
// Heatmap render mode: the THREE half.
//
// A matrix becomes one quad per block -- two triangles -- with the matrix
// itself as a texture. That *is* the square heatmap: viewed head on, texel
// (i, j) is a filled square at the same place element (i, j) is a sphere in
// ./points.ts, with no gap between neighbours, so it reads as an ordinary
// heatmap rather than as a grid of balls.
//
// The arithmetic -- which cell is where, which value a texel carries, how much
// resolution a matrix is allowed -- is in ./heatmap.ts and ./colormap.ts, and
// is tested there. This file is the wiring.
//
// Two channels per texel, one byte each, in a single RG8 texture:
//
//   R  the value, normalized through the Mat's own range. Never FP32: a
//      768x3072 weight is 9.4 MB as R32F and 2.4 MB as R8. The exact value
//      stays on the CPU in the Mat's Array2D, and that is what the hover
//      readout and the labels print. Visual fidelity is not numerical
//      fidelity.
//   G  the cell's state: hidden, shown, or highlighted. Its own channel and
//      not a colour, because `Mat.isHidden` decides by comparing to black and
//      this ramp's low stop is #03051A -- see the note on TEXEL_HIDDEN in
//      ./heatmap.ts.
//
// TSL, not GLSL: main.ts builds a THREE.WebGPURenderer.
//

import * as THREE from 'three'
import {
  Fn, Discard, uv, texture, vec2, vec4, float, mix, step, colorSpaceToWorking,
} from 'three/tsl'

import {
  TEXEL_HIDDEN, TEXEL_SHOWN, TEXEL_BUMPED,
  blockQuad, lodSize, reduceRegion, uvToCell,
} from './heatmap.js'
import { texelIndex } from './colormap.js'

// A unit quad per block; the mesh's scale and position place it. Four vertices
// each, so a per-block geometry costs nothing -- and it must be per block,
// because util.disposeAndClear() disposes whatever `geometry` it finds while
// walking a group, and a shared one would be torn out from under every other
// matrix the first time a scene was rebuilt.
//
// PlaneGeometry's uv runs (0,0) at -x/-y, which with DataTexture's flipY =
// false puts data row 0 at element row 0 -- nothing flips anywhere.
const unitPlane = () => new THREE.PlaneGeometry(1, 1)

// bumpColor() in viz.ts adds 0x808080 to an element's colour to highlight the
// row or column an animation is reading. Same constant here, so a highlighted
// cell reads the same in both modes.
const BUMP = 0x80 / 255

/**
 * The material for one block.
 *
 * One per block rather than one shared: a TextureNode binds its texture when
 * the node graph is built, so a shared material could only ever show one
 * matrix. The graphs are identical, so three.js's pipeline cache still hands
 * every one of them the same compiled shader.
 */
function makeMaterial(valueTex: THREE.DataTexture, lutTex: THREE.DataTexture) {
  const material = new THREE.NodeMaterial()

  material.fragmentNode = Fn(() => {
    const cell: any = texture(valueTex, uv())

    // G is 0/1/2 stored as a unorm byte, so scale it back to the state values.
    const state: any = cell.g.mul(255.0)

    // Hidden cells leave the quad, rather than drawing the ramp's low stop --
    // the whole reason state is not a colour.
    Discard(state.lessThan(0.5))

    // NearestFilter over a 256-wide LUT: u = byte/255 lands on texel `byte`
    // exactly, so the shader reads the entry colormapLUT() wrote and never an
    // interpolation between two entries.
    const lut: any = texture(lutTex, vec2(cell.r, 0.5))

    const bumped: any = lut.rgb.add(float(BUMP)).clamp(0.0, 1.0)
    const rgb: any = mix(lut.rgb, bumped, step(1.5, state))

    // Cancel WebGPU's frame-wide output conversion, exactly as points.ts does
    // and for the same reason, so the sRGB bytes colormapLUT() computed are
    // the bytes in the framebuffer. See the long note there.
    return colorSpaceToWorking(vec4(rgb, 1.0), THREE.SRGBColorSpace)
  })()

  material.fog = false
  material.toneMapped = false
  material.transparent = false
  material.side = THREE.DoubleSide
  return material
}

function makeValueTexture(tw: number, th: number, linear: boolean) {
  const data = new Uint8Array(tw * th * 2)
  const tex = new THREE.DataTexture(data, tw, th, THREE.RGFormat, THREE.UnsignedByteType)
  // Nearest by default on both filters. A heatmap of weights is data, not a
  // photo: bilinear filtering invents values between texels, and at LOD > 0 it
  // would smear an outlier this ladder went out of its way to preserve.
  tex.magFilter = tex.minFilter = linear ? THREE.LinearFilter : THREE.NearestFilter
  tex.generateMipmaps = false
  tex.unpackAlignment = 1        // rows are tw*2 bytes, rarely a multiple of 4
  tex.needsUpdate = true
  return tex
}

/** One block of a matrix: a quad, a value texture, and the cells it covers. */
class HeatmapBlock extends THREE.Mesh {
  isHeatmapBlock = true

  i0: number; j0: number; bh: number; bw: number
  f: number; th: number; tw: number
  W: number                       // the matrix's width, for the global index
  bytes: Uint8Array
  valueTex: THREE.DataTexture

  constructor(i0, j0, bh, bw, f, W, lutTex, info, linear) {
    const { h: th, w: tw } = lodSize(bh, bw, f)
    const valueTex = makeValueTexture(tw, th, linear)
    super(unitPlane(), makeMaterial(valueTex, lutTex))

    this.i0 = i0; this.j0 = j0; this.bh = bh; this.bw = bw
    this.f = f; this.th = th; this.tw = tw; this.W = W
    this.valueTex = valueTex
    this.bytes = valueTex.image.data as Uint8Array

    const q = blockQuad(i0, j0, bh, bw, info)
    this.scale.set(q.w, q.h, 1)
    this.position.set(q.cx, q.cy, 0)
  }

  /**
   * three.js's Mesh.raycast, annotated with the element index viz.ts expects.
   *
   * `Mat.updateLabels` recovers the cell as `index / W`, `index % W` -- the
   * same arithmetic it applies to PointCloud's hits -- so the heatmap has to
   * report the *element* under the cursor, not the texel. The two differ at
   * LOD > 0, and the element is the one whose exact value the readout prints.
   */
  override raycast(raycaster, intersects) {
    const hits = []
    super.raycast(raycaster, hits)
    // A quad is two triangles sharing a diagonal, and a ray crossing that
    // diagonal is reported by both of them -- so a cell on it would be picked
    // twice. Deduplicating by cell keeps one hit per cell, which is what
    // `index` claims to be.
    const seen = new Set<number>()
    for (const hit of hits as any[]) {
      if (!hit.uv) continue
      const { i, j } = uvToCell(hit.uv.x, hit.uv.y, this.bh, this.bw)
      const index = (this.i0 + i) * this.W + (this.j0 + j)
      if (seen.has(index)) continue
      seen.add(index)
      hit.index = index
      hit.distanceToRay = 0
      intersects.push(hit)
    }
  }
}

/**
 * A matrix drawn as heatmap texels. Drop-in alongside PointCloud: viz.js keeps
 * it in `Mat.points`, raycasts it, and reads cells back out of it.
 *
 * Blocks exist because `layout.gap` inserts space between them, exactly as
 * `emptyPoints` does -- one quad per block keeps texels contiguous *within* a
 * block while the gaps between blocks stay where the rest of the scene expects
 * them. With the default single block and `gap` 0 this is one quad and one
 * contiguous texture, which is what "a normal heatmap" means.
 */
export class HeatmapMesh extends THREE.Group {
  isHeatmap = true

  H: number; W: number
  f: number; op: string; enc: string
  blocks: HeatmapBlock[] = []
  lutTex: THREE.DataTexture
  texels = 0

  constructor(H, W, info, opts: any) {
    super()
    this.H = H; this.W = W
    this.f = opts.lod || 1
    this.op = opts.reducer || 'maxAbs'
    this.enc = opts.encoding || 'magnitude'

    this.lutTex = new THREE.DataTexture(
      new Uint8Array(256 * 4), 256, 1, THREE.RGBAFormat, THREE.UnsignedByteType)
    // NoColorSpace (the default) is load-bearing: the LUT already holds sRGB
    // bytes, and letting the sampler convert them would double-convert against
    // the colorSpaceToWorking() above and darken every stop.
    this.lutTex.magFilter = this.lutTex.minFilter = THREE.NearestFilter
    this.lutTex.generateMipmaps = false
    this.lutTex.needsUpdate = true

    const { i: { size: si, n: ni }, j: { size: sj, n: nj } } = info
    for (let bi = 0; bi < ni; bi++) {
      const i0 = bi * si
      if (i0 >= H) continue
      for (let bj = 0; bj < nj; bj++) {
        const j0 = bj * sj
        if (j0 >= W) continue
        const bh = Math.min(si, H - i0), bw = Math.min(sj, W - j0)
        const b = new HeatmapBlock(i0, j0, bh, bw, this.f, W, this.lutTex, info, !!opts.linear)
        this.texels += b.th * b.tw
        this.blocks.push(b)
        this.add(b)
      }
    }
  }

  setLUT(bytes: Uint8Array) {
    (this.lutTex.image.data as Uint8Array).set(bytes)
    this.lutTex.needsUpdate = true
  }

  /** Texel range of a block covering element rows/cols [a0, a1). */
  private static span(a0: number, a1: number, origin: number, n: number, f: number) {
    const lo = Math.max(0, Math.floor((a0 - origin) / f))
    const hi = Math.min(Math.ceil(n / f), Math.ceil((a1 - origin) / f))
    return [lo, hi]
  }

  /**
   * Re-encode the value channel for an element range from the CPU-side data.
   *
   * Whole texels, always: a texel at LOD > 0 stands for f x f elements, so
   * touching one element re-reduces its texel over all of them. That is what
   * keeps `maxAbs` true after a partial update.
   */
  writeValues(data: ArrayLike<number>, r: number[], c: number[],
    enc: string, absmin: number, absmax: number) {
    this.enc = enc
    for (const b of this.blocks) {
      const [t0, t1] = HeatmapMesh.span(r[0], r[1], b.i0, b.bh, b.f)
      const [s0, s1] = HeatmapMesh.span(c[0], c[1], b.j0, b.bw, b.f)
      if (t0 >= t1 || s0 >= s1) continue
      for (let a = t0; a < t1; a++) {
        const gi0 = b.i0 + a * b.f, gi1 = Math.min(b.i0 + b.bh, gi0 + b.f)
        for (let bb = s0; bb < s1; bb++) {
          const gj0 = b.j0 + bb * b.f, gj1 = Math.min(b.j0 + b.bw, gj0 + b.f)
          const x = reduceRegion(data, this.W, gi0, gi1, gj0, gj1, this.op)
          b.bytes[(a * b.tw + bb) * 2] = texelIndex(x, enc, absmin, absmax)
        }
      }
      b.valueTex.needsUpdate = true
    }
  }

  /** Set the state channel over an element range. */
  writeState(state: number, r: number[], c: number[]) {
    for (const b of this.blocks) {
      const [t0, t1] = HeatmapMesh.span(r[0], r[1], b.i0, b.bh, b.f)
      const [s0, s1] = HeatmapMesh.span(c[0], c[1], b.j0, b.bw, b.f)
      if (t0 >= t1 || s0 >= s1) continue
      for (let a = t0; a < t1; a++) {
        for (let bb = s0; bb < s1; bb++) {
          b.bytes[(a * b.tw + bb) * 2 + 1] = state
        }
      }
      b.valueTex.needsUpdate = true
    }
  }

  private cell(i: number, j: number) {
    for (const b of this.blocks) {
      if (i >= b.i0 && i < b.i0 + b.bh && j >= b.j0 && j < b.j0 + b.bw) {
        const a = Math.floor((i - b.i0) / b.f), bb = Math.floor((j - b.j0) / b.f)
        return { b, off: (a * b.tw + bb) * 2 }
      }
    }
    return null
  }

  /**
   * A cell's state. Texel-granular by construction: at LOD > 0 one texel is the
   * only visibility a cell has, which is the honest granularity for a view that
   * is already showing one texel per f x f block.
   */
  stateAt(i: number, j: number): number {
    const c = this.cell(i, j)
    return c ? c.b.bytes[c.off + 1] : TEXEL_HIDDEN
  }

  /** A cell's *quantized* byte. Never the value -- viz.ts reads that from Array2D. */
  byteAt(i: number, j: number): number {
    const c = this.cell(i, j)
    return c ? c.b.bytes[c.off] : 0
  }

  dispose() {
    this.lutTex.dispose()
    for (const b of this.blocks) {
      b.valueTex.dispose()
        ; (b.material as THREE.Material).dispose()
    }
  }
}

export { TEXEL_HIDDEN, TEXEL_SHOWN, TEXEL_BUMPED }
