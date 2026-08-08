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
import { PointCloud, MATERIAL } from '../src/points.js'

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
