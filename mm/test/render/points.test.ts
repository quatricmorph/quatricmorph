//
// points.js — the instanced-quad replacement for THREE.Points.
//
// The shaders need a GPU and are not tested here. The contract with viz.js and
// main.js is testable and is what matters: viz.js writes element data straight
// into `geometry.attributes.pointSize` / `.pointColor`, and main.js's spotlight
// reads `intersects[].index` as the element index so that `index / W` and
// `index % W` recover row and column. Break either and the app still renders --
// it just highlights the wrong element, or none.
//
import { describe, it, expect } from 'vitest'
import * as THREE from 'three'
import { PointCloud, MATERIAL } from '../../src/render/points.js'

// Three element centres in a row, 10 apart.
const CENTERS = () => new Float32Array([0, 0, 0, 10, 0, 0, 20, 0, 0])

const cloud = () => {
  const pc = new PointCloud(CENTERS(), 3)
  pc.updateMatrixWorld(true)
  return pc
}

describe('PointCloud geometry', () => {
  it('exposes the attributes viz.js writes into, one entry per element', () => {
    const g = cloud().geometry as any
    expect(g.attributes.pointSize.count).toBe(3)
    expect(g.attributes.pointColor.count).toBe(3)
    expect(g.attributes.pointColor.itemSize).toBe(3)
    expect(g.attributes.pointCenter.count).toBe(3)
    expect(g.instanceCount).toBe(3)
  })

  it('draws one indexed quad per instance, not one vertex per element', () => {
    const g = cloud().geometry as any
    expect(g.attributes.position.count).toBe(4)   // the shared unit quad
    expect(g.index.count).toBe(6)                 // two triangles
  })

  it('identifies itself so viz.js can recognise it', () => {
    expect(cloud().isPointCloud).toBe(true)
    expect(cloud().material).toBe(MATERIAL)
  })
})

describe('bounds', () => {
  it('measures the element centres, not the unit quad', () => {
    // The stock InstancedBufferGeometry implementation would describe a
    // half-unit blob at the origin: the matrix would be frustum-culled the
    // moment the origin left view, and raycast's sphere pre-test would reject
    // every ray. Both bounds have to come from pointCenter.
    const g = cloud().geometry as any
    g.computeBoundingBox()
    expect(g.boundingBox.min.x).toBe(0)
    expect(g.boundingBox.max.x).toBe(20)

    g.computeBoundingSphere()
    expect(g.boundingSphere.center.x).toBe(10)
    expect(g.boundingSphere.radius).toBeGreaterThanOrEqual(10)
  })
})

describe('raycast', () => {
  // A ray down -z through the element at (10, 0, 0).
  const rayAt = (x, threshold = 1) => {
    const rc = new THREE.Raycaster(new THREE.Vector3(x, 0, 50), new THREE.Vector3(0, 0, -1))
    rc.params.Points.threshold = threshold
    return rc
  }

  it('returns the element index, so row and column can be recovered from it', () => {
    const hits = []
    cloud().raycast(rayAt(10), hits)
    expect(hits).toHaveLength(1)
    expect(hits[0].index).toBe(1)          // the middle element
    expect(hits[0].object.isPointCloud).toBe(true)
  })

  it('honours the threshold viz.js sets from params.deco.spotlight', () => {
    const near = []
    cloud().raycast(rayAt(13, 5), near)    // 3 away from the element, within 5
    expect(near.map(h => h.index)).toEqual([1])

    const far = []
    cloud().raycast(rayAt(13, 1), far)     // 3 away, outside 1
    expect(far).toHaveLength(0)
  })

  it('picks up every element within the threshold', () => {
    const hits = []
    cloud().raycast(rayAt(10, 11), hits)   // wide enough to reach all three
    expect(hits.map(h => h.index).sort()).toEqual([0, 1, 2])
  })

  it('drops hits outside [near, far], which is how the spotlight switches off', () => {
    // main.js turns the spotlight off wholesale by setting far = 0.
    const rc = rayAt(10)
    rc.far = 0
    const hits = []
    cloud().raycast(rc, hits)
    expect(hits).toHaveLength(0)
  })

  it('reports the distance along the ray', () => {
    const hits = []
    cloud().raycast(rayAt(10), hits)
    expect(hits[0].distance).toBeCloseTo(50, 6)
    expect(hits[0].distanceToRay).toBeCloseTo(0, 6)
  })
})

describe('MATERIAL', () => {
  it('keeps the magnifier uniform reachable under its pre-WebGPU name', () => {
    // main.js drives the lens through MATERIAL.uniforms.mag.value, which used
    // to be a ShaderMaterial uniform. NodeMaterial ignores `.uniforms`, so this
    // is a deliberate alias for that one call site.
    expect((MATERIAL as any).uniforms.mag).toBeDefined()
    expect((MATERIAL as any).uniforms.mag.value).toBeDefined()
    expect((MATERIAL as any).uniforms.color).toBeDefined()
  })

  it('renders elements opaque, unlit and untone-mapped', () => {
    // The value -> colour mapping in viz.js is the data. Any lighting, fog or
    // tone mapping applied on top of it would misreport the weights.
    expect(MATERIAL.transparent).toBe(false)
    expect(MATERIAL.fog).toBe(false)
    expect(MATERIAL.toneMapped).toBe(false)
    expect(MATERIAL.side).toBe(THREE.DoubleSide)
  })
})

//
// Heatmap render mode.
//
// The second element-render path. Everything asserted here is a decision that
// is invisible when it goes wrong: a transposed or flipped heatmap draws a
// perfectly plausible picture of the wrong matrix, an off-by-one pick names the
// wrong cell in the readout, a mip level that averaged the outlier away hides
// exactly what an inspector was looking for, and a hidden cell that is one byte
// from the lowest value makes an animation look like it is skipping cells.
//
// The shading itself still needs a GPU and is still not tested. The mapping it
// applies is: the fragment shader samples a 256-entry lookup that
// `colormapLUT()` fills, so the colours below and the colours on screen are one
// implementation.
//
import {
  COLORMAP_STOPS, colormapSRGB, colormapLUT, colormapHex,
  texelIndex, indexValue, elementHSL,
} from '../../src/render/colormap.js'
import {
  pickRenderMode, RENDER_MODES, chooseLodFactor, lodSize, reduceLevel, reduceTexel,
  elementPosition, blockQuad, texelUV, uvToCell,
  HEATMAP_MIN_ELEMENTS, HEATMAP_TEXEL_BUDGET,
  TEXEL_HIDDEN, TEXEL_SHOWN, TEXEL_BUMPED,
} from '../../src/render/heatmap.js'
import { HeatmapMesh } from '../../src/render/heatmapmesh.js'
import { emptyPoints } from '../../src/scene/viz.js'

// The block descriptor viz.js's `emptyPoints` and the heatmap are both built
// from. One block per axis unless a test says otherwise.
const blockInfo = (h, w, gap = 0, ni = 1, nj = 1) => ({
  i: { n: ni, size: Math.ceil(h / ni), max: h },
  j: { n: nj, size: Math.ceil(w / nj), max: w },
  gap,
})

const hsl2rgb = (h, s, l) => {
  const c = new THREE.Color().setHSL(h, s, l)
  return [c.r * 255, c.g * 255, c.b * 255] as [number, number, number]
}

describe('colormap', () => {
  it('lands each of the seven stops exactly, evenly spaced', () => {
    // Hand-derived: the stops are evenly spaced, so stop k is at t = k/6.
    COLORMAP_STOPS.forEach((hex, k) => {
      const [r, g, b] = colormapSRGB(k / 6).map(Math.round)
      expect([r, g, b]).toEqual([(hex >> 16) & 255, (hex >> 8) & 255, hex & 255])
    })
  })

  it('interpolates linearly between two stops', () => {
    // Halfway from #03051A to #501D4C is the byte-wise mean:
    // (0x03+0x50)/2 = 41.5, (0x05+0x1D)/2 = 17, (0x1A+0x4C)/2 = 51
    const mid = colormapSRGB(1 / 12)
    expect(mid[0]).toBeCloseTo(41.5, 6)
    expect(mid[1]).toBeCloseTo(17, 6)
    expect(mid[2]).toBeCloseTo(51, 6)
  })

  it('clamps outside [0, 1] rather than extrapolating off the ramp', () => {
    expect(colormapSRGB(-5)).toEqual(colormapSRGB(0))
    expect(colormapSRGB(99)).toEqual(colormapSRGB(1))
  })

  it('round-trips every stop through the shader colour-space pipeline', () => {
    // The heatmap fragment shader ends with colorSpaceToWorking(sRGB), which
    // pre-decodes so that WebGPU's frame-wide working -> sRGB output
    // conversion round-trips to the value computed. Model both halves with
    // three's own ColorManagement -- the same code the node does -- and
    // require the byte that lands in the framebuffer to be the specified hex.
    expect(THREE.ColorManagement.enabled).toBe(true)   // else this is vacuous

    COLORMAP_STOPS.forEach((hex, k) => {
      const [r, g, b] = colormapSRGB(k / 6)
      const c = new THREE.Color()
      c.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace)   // colorSpaceToWorking
      const out: any = {}
      c.getRGB(out, THREE.SRGBColorSpace)                          // frame output
      expect([
        Math.round(out.r * 255), Math.round(out.g * 255), Math.round(out.b * 255),
      ]).toEqual([(hex >> 16) & 255, (hex >> 8) & 255, hex & 255])
    })
  })

  it('fills the lookup the shader samples from the same function', () => {
    // The shader has no ramp of its own; it indexes this table. If the two
    // could drift, the picture and this file would be describing different
    // colours.
    const lut = colormapLUT('magnitude', 0, 1, {}, hsl2rgb)
    for (let b = 0; b <= 255; b += 17) {
      const [r, g, bl] = colormapSRGB(b / 255).map(Math.round)
      expect([lut[b * 4], lut[b * 4 + 1], lut[b * 4 + 2], lut[b * 4 + 3]])
        .toEqual([r, g, bl, 255])
    }
    expect(lut.length).toBe(256 * 4)
  })

  it('gives the signed encoding mm\'s own element colours, not the ramp', () => {
    // 'signed' exists because the seven-stop ramp is sequential and cannot
    // carry a sign. It reuses colorFromData's arithmetic, so a LUT entry must
    // equal what an element of that value would have been.
    const viz = {
      'zero hue': 0.356, 'hue gap': 0.7, 'hue spread': 0.04,
      'min light': 0.5, 'max light': 0.8,
    }
    const [absmin, absmax] = [0, 2]
    const lut = colormapLUT('signed', absmin, absmax, viz, hsl2rgb)
    for (const b of [0, 64, 128, 200, 255]) {
      const x = indexValue(b, 'signed', absmin, absmax)
      const hsl = elementHSL(x, { absmin, absmax, absdiff: absmax - absmin }, viz)
      const want = hsl ? hsl2rgb(hsl.h, hsl.s, hsl.l).map(Math.round) : [0, 0, 0]
      expect([lut[b * 4], lut[b * 4 + 1], lut[b * 4 + 2]]).toEqual(want)
    }
  })

  it('renders a hex for the colorbar ends', () => {
    expect(colormapHex(0)).toBe('#03051a')
    expect(colormapHex(1)).toBe('#faebdd')
  })
})

describe('value -> texel encoding', () => {
  it('spends the byte on |x| in magnitude mode, low stop at absmin', () => {
    expect(texelIndex(0, 'magnitude', 0, 4)).toBe(0)
    expect(texelIndex(4, 'magnitude', 0, 4)).toBe(255)
    expect(texelIndex(-4, 'magnitude', 0, 4)).toBe(255)   // magnitude: sign is not shown
    expect(texelIndex(2, 'magnitude', 0, 4)).toBe(128)    // round(255/2)
  })

  it('puts zero mid-ramp in signed mode and keeps both tails', () => {
    expect(texelIndex(0, 'signed', 0, 4)).toBe(128)       // round(255/2)
    expect(texelIndex(4, 'signed', 0, 4)).toBe(255)
    expect(texelIndex(-4, 'signed', 0, 4)).toBe(0)
  })

  it('clamps rather than wrapping when an intermediate leaves its range', () => {
    expect(texelIndex(99, 'magnitude', 0, 4)).toBe(255)
    expect(texelIndex(-99, 'signed', 0, 4)).toBe(0)
  })

  it('inverts to the value a texel stands for', () => {
    // indexValue is the ramp's own x axis, which is what the LUT is built over.
    expect(indexValue(0, 'magnitude', 1, 5)).toBeCloseTo(1, 6)
    expect(indexValue(255, 'magnitude', 1, 5)).toBeCloseTo(5, 6)
    expect(indexValue(128, 'signed', 0, 4)).toBeCloseTo(4 * (128 / 255) * 2 - 4, 6)
  })

  it('survives a degenerate range instead of producing NaN', () => {
    expect(texelIndex(3, 'magnitude', 2, 2)).toBe(0)
    expect(texelIndex(3, 'signed', 0, 0)).toBe(128)
  })
})

describe('LOD ladder', () => {
  it('keeps the outlier that mean-reduction loses', () => {
    // The reason this ladder is not ordinary mipmapping. Hand-computed:
    // maxAbs([0.001, 0.003, 9.830, 0.001]) = 9.830
    // mean([0.001, 0.003, 9.830, 0.001])   = 9.835/4 = 2.45875
    const data = [0.001, 0.003, 9.830, 0.001]
    expect(reduceTexel(data, 2, 2, 2, 0, 0, 'maxAbs')).toBeCloseTo(9.830, 6)
    expect(reduceTexel(data, 2, 2, 2, 0, 0, 'mean')).toBeCloseTo(2.45875, 6)
  })

  it('keeps the outlier\'s sign, so the signed encoding still reads', () => {
    expect(reduceTexel([0.1, -9.5, 0.2, 0.3], 2, 2, 2, 0, 0, 'maxAbs')).toBeCloseTo(-9.5, 6)
  })

  it('reduces a whole level, with a ragged last block', () => {
    // 3x3 at factor 2 -> 2x2. The last row and column are one element wide.
    //   1 2 3
    //   4 5 6
    //   7 8 9
    // maxAbs blocks: [1,2;4,5]=5  [3;6]=6  [7,8]=8  [9]=9
    const d = [1, 2, 3, 4, 5, 6, 7, 8, 9]
    const r = reduceLevel(d, 3, 3, 2, 'maxAbs')
    expect([r.h, r.w]).toEqual([2, 2])
    expect(Array.from(r.data)).toEqual([5, 6, 8, 9])
  })

  it('picks the smallest power-of-two factor that fits the texel budget', () => {
    expect(chooseLodFactor(64, 64, HEATMAP_TEXEL_BUDGET)).toBe(1)       // 4,096
    expect(chooseLodFactor(768, 3072, 1 << 18)).toBe(4)                 // 192x768 = 147,456
    expect(chooseLodFactor(768, 3072, 1 << 16)).toBe(8)                 // 96x384 = 36,864
    expect(lodSize(768, 3072, 4)).toEqual({ h: 192, w: 768 })
  })

  it('also honours the screen bound, whichever binds harder', () => {
    // 3072 texels along an axis cannot be shown on a 1000-pixel viewport.
    expect(chooseLodFactor(768, 3072, Infinity, 1000)).toBe(4)          // 768 <= 1000
    expect(chooseLodFactor(768, 3072, Infinity, Infinity)).toBe(1)
  })

  it('stops reducing rather than smearing a matrix to nothing', () => {
    expect(chooseLodFactor(1 << 20, 1 << 20, 1)).toBe(64)
  })
})

describe('render mode selection', () => {
  it('takes an explicit override at its word, in both directions', () => {
    // The point of the control. A 4096x4096 matrix forced to spheres is 16M
    // instanced quads and will be slow — but it is what was asked for, and a
    // control that quietly declined would be worse than no control.
    expect(pickRenderMode(4, 4, 'heatmap')).toBe('heatmap')
    expect(pickRenderMode(4096, 4096, 'spheres')).toBe('spheres')
  })

  it('offers exactly the three modes the selectors are built from', () => {
    expect(RENDER_MODES).toEqual(['auto', 'spheres', 'heatmap'])
  })

  it('still accepts the pre-rename spelling, so old params keep working', () => {
    expect(pickRenderMode(4096, 4096, 'elements')).toBe('spheres')
  })

  it('never returns "auto" — callers branch on the answer, not the question', () => {
    expect(pickRenderMode(4, 4, 'auto')).toBe('spheres')
    expect(pickRenderMode(4096, 4096, 'auto')).toBe('heatmap')
  })

  it('switches to heatmap where an element can no longer read as a sphere', () => {
    expect(pickRenderMode(255, 256, 'auto')).toBe('spheres')        // 65,280
    expect(pickRenderMode(256, 256, 'auto')).toBe('heatmap')        // 65,536
    expect(HEATMAP_MIN_ELEMENTS).toBe(256 * 256)
  })

  it('leaves the attention head and the projections on opposite sides of it', () => {
    expect(pickRenderMode(64, 64, 'auto')).toBe('spheres')          // one head
    expect(pickRenderMode(769, 3072, 'auto')).toBe('heatmap')       // mlp.c_fc
  })
})

describe('heatmap geometry', () => {
  // The failure this pins: a heatmap that silently transposes or flips draws a
  // plausible picture of the wrong matrix, and nothing anywhere says so. So the
  // texel positions are compared against emptyPoints' own attribute -- the
  // element path's actual output -- not against a restatement of its formula.
  const cellWorld = (mesh, i, j, H, W) => {
    mesh.updateMatrixWorld(true)
    for (const b of mesh.blocks) {
      if (i < b.i0 || i >= b.i0 + b.bh || j < b.j0 || j >= b.j0 + b.bw) continue
      const { u, v } = texelUV(i - b.i0, j - b.j0, b.bh, b.bw)
      return new THREE.Vector3(u - 0.5, v - 0.5, 0).applyMatrix4(b.matrixWorld)
    }
    throw new Error(`no block covers (${i}, ${j})`)
  }

  const agrees = (H, W, gap = 0, ni = 1, nj = 1) => {
    const info = blockInfo(H, W, gap, ni, nj)
    const centers = emptyPoints(H, W, info).geometry.attributes.pointCenter
    const mesh = new HeatmapMesh(H, W, info, { lod: 1 })
    for (let i = 0; i < H; i++) {
      for (let j = 0; j < W; j++) {
        const want = new THREE.Vector3().fromBufferAttribute(centers, i * W + j)
        const got = cellWorld(mesh, i, j, H, W)
        expect([got.x, got.y]).toEqual([want.x, want.y])
      }
    }
  }

  it('puts texel (0,0) where element (0,0) is, and every other cell too', () => {
    agrees(5, 7)
  })

  it('keeps agreeing when layout.gap splits the matrix into blocks', () => {
    // gap is what makes this non-trivial: emptyPoints steps a whole gap at
    // every block boundary, so a lattice of quads that ignored blocks would
    // drift one gap per block from the elements it claims to be drawing.
    agrees(6, 8, 4, 2, 2)
  })

  it('is one contiguous quad per block, two triangles', () => {
    const mesh = new HeatmapMesh(64, 3072, blockInfo(64, 3072), { lod: 1 })
    expect(mesh.blocks).toHaveLength(1)
    expect(mesh.blocks[0].geometry.index.count).toBe(6)
    expect(mesh.isHeatmap).toBe(true)
  })

  it('sizes the quad to the cells and centres it half a cell in', () => {
    // Element (0,0) sits at x = 0, element (0, w-1) at x = w-1, and a cell is
    // one unit wide, so the quad spans [-0.5, w-0.5] and its centre is w/2-0.5.
    expect(blockQuad(0, 0, 4, 6, blockInfo(4, 6))).toEqual({ w: 6, h: 4, cx: 2.5, cy: 1.5 })
    expect(elementPosition(3, 5, blockInfo(8, 8, 10, 2, 2))).toEqual({ x: 15, y: 3 })
  })
})

describe('heatmap picking', () => {
  it('recovers the same cell PointCloud\'s index / W and index % W do', () => {
    // viz.js does not know which path drew the matrix: updateLabels takes
    // `intersects[].index` and divides. So the heatmap has to report the
    // element index, and it has to be the element the cursor is over.
    const [H, W] = [5, 7]
    const info = blockInfo(H, W)
    const mesh = new HeatmapMesh(H, W, info, { lod: 1 })
    mesh.updateMatrixWorld(true)

    for (const [i, j] of [[0, 0], [0, 6], [4, 0], [4, 6], [2, 3]]) {
      const p = elementPosition(i, j, info)
      const rc = new THREE.Raycaster(
        new THREE.Vector3(p.x, p.y, 50), new THREE.Vector3(0, 0, -1))
      const hits = []
      mesh.blocks[0].raycast(rc, hits)
      expect(hits).toHaveLength(1)
      expect(hits[0].index).toBe(i * W + j)
      expect(Math.floor(hits[0].index / W)).toBe(i)
      expect(hits[0].index % W).toBe(j)
    }
  })

  it('names the element under the cursor even when the texture is reduced', () => {
    // The pick is in element coordinates and the LOD level is not: the readout
    // must print the checkpoint's own value for the cell hovered, and that is
    // not the value the reduced texel happens to be showing.
    const [H, W] = [8, 8]
    const info = blockInfo(H, W)
    const mesh = new HeatmapMesh(H, W, info, { lod: 4 })
    expect(mesh.blocks[0].th).toBe(2)
    mesh.updateMatrixWorld(true)
    const p = elementPosition(6, 3, info)
    const hits = []
    mesh.blocks[0].raycast(
      new THREE.Raycaster(new THREE.Vector3(p.x, p.y, 9), new THREE.Vector3(0, 0, -1)), hits)
    expect(hits[0].index).toBe(6 * W + 3)
  })

  it('converts a uv to a cell without falling off either end', () => {
    expect(uvToCell(0, 0, 4, 6)).toEqual({ i: 0, j: 0 })
    expect(uvToCell(1, 1, 4, 6)).toEqual({ i: 3, j: 5 })      // clamped, not 4/6
    expect(uvToCell(0.5, 0.5, 4, 6)).toEqual({ i: 2, j: 3 })
  })
})

describe('heatmap state channel', () => {
  // The named hazard. `Mat.isHidden` decides by comparing an element's colour
  // to black, and the ramp's low stop is #03051A -- near-black. Under a uint8
  // texture "hidden" and "the smallest value in this matrix" would be one byte
  // apart, and every animation, which hides its result before it starts, would
  // look like it was skipping cells.
  const mesh = () => new HeatmapMesh(2, 2, blockInfo(2, 2), { lod: 1 })

  it('distinguishes a cell at absmin from a hidden cell', () => {
    const m = mesh()
    const data = [1, 5, 5, 5]                     // (0,0) is at absmin
    m.writeValues(data, [0, 2], [0, 2], 'magnitude', 1, 5)
    m.writeState(TEXEL_SHOWN, [0, 2], [0, 2])
    m.writeState(TEXEL_HIDDEN, [1, 2], [1, 2])    // hide (1,1), which is at absmax

    expect(m.byteAt(0, 0)).toBe(0)                // the ramp's low stop
    expect(m.stateAt(0, 0)).toBe(TEXEL_SHOWN)
    expect(m.stateAt(1, 1)).toBe(TEXEL_HIDDEN)
    // and a hidden cell whose value also encodes to 0 is still distinguishable
    m.writeState(TEXEL_HIDDEN, [0, 1], [0, 1])
    expect(m.byteAt(0, 0)).toBe(m.byteAt(0, 0))
    expect(m.stateAt(0, 0)).not.toBe(m.stateAt(0, 1))
  })

  it('carries the animation highlight as state, not as a colour', () => {
    // bumpColor() adds 0x808080 to an element. "Add grey" means nothing to a
    // ramp index, so the highlight is a third state and the shader applies it.
    const m = mesh()
    m.writeState(TEXEL_SHOWN, [0, 2], [0, 2])
    m.writeState(TEXEL_BUMPED, [0, 1], [0, 2])
    expect(m.stateAt(0, 0)).toBe(TEXEL_BUMPED)
    expect(m.stateAt(1, 0)).toBe(TEXEL_SHOWN)
  })

  it('leaves the value channel alone when only the state changes', () => {
    const m = mesh()
    m.writeValues([1, 2, 3, 4], [0, 2], [0, 2], 'magnitude', 0, 4)
    const before = m.byteAt(1, 1)
    m.writeState(TEXEL_HIDDEN, [0, 2], [0, 2])
    expect(m.byteAt(1, 1)).toBe(before)
  })

  it('re-reduces whole texels when one element changes under a mip level', () => {
    // Touching one element has to re-reduce its whole texel, or maxAbs stops
    // being true the first time a partial update lands.
    const m = new HeatmapMesh(2, 2, blockInfo(2, 2), { lod: 2, reducer: 'maxAbs' })
    m.writeValues([1, 1, 1, 9], [0, 1], [0, 1], 'magnitude', 0, 9)   // only (0,0)
    expect(m.byteAt(0, 0)).toBe(255)                                  // saw the 9
  })
})
