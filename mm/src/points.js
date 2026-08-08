"use strict"

//
// Element rendering.
//
// mm draws every matrix element as one shaded sphere. Under WebGL that was a GL
// point: a per-vertex `pointSize` attribute driving gl_PointSize, and a fragment
// shader rebuilding a unit normal from gl_PointCoord to light it.
//
// WebGPU has no equivalent. Its point primitive is fixed at one pixel, which
// three.js states in PointsNodeMaterial itself: "Since WebGPU only supports
// point primitives with a pixel size of 1, it's not possible to define a size."
// So an element is now an instanced quad -- one 4-vertex billboard per element,
// expanded in clip space to exactly the pixel footprint gl_PointSize produced,
// with the impostor normal rebuilt from the quad's own corner instead of
// gl_PointCoord.
//
// The shading below is a line-by-line transcription of the GLSL it replaces,
// not a reinterpretation: same constants, same light directions, same
// screen-not-add compositing. Two differences are deliberate and marked at the
// point they occur (the y flip and the sqrt guard).
//

import * as THREE from 'three'
import {
  Fn, Discard, attribute, uniform, varying, vec3, vec4, float,
  positionGeometry, modelViewMatrix, cameraProjectionMatrix, viewportSize,
  normalize, dot, max, mix, pow, smoothstep, sqrt, colorSpaceToWorking,
} from 'three/tsl'

//
// geometry
//

// One unit quad, corners at +/-0.5 with +y up, shared by every element of every
// matrix. gl_PointCoord ran top-left-down; this runs bottom-left-up, which is
// why the fragment shader below does NOT negate y the way the GLSL did.
const QUAD_POSITION = new Float32Array([
  -0.5, -0.5, 0,
  0.5, -0.5, 0,
  0.5, 0.5, 0,
  -0.5, 0.5, 0,
])
const QUAD_INDEX = new Uint16Array([0, 1, 2, 0, 2, 3])

// The instanced geometry's `position` is the quad, so the stock
// computeBoundingSphere() would describe a half-unit blob at the origin -- the
// whole matrix would be frustum-culled the moment the origin left the view, and
// raycast()'s sphere pre-test would reject every ray. Both have to measure the
// element centres instead.
class PointCloudGeometry extends THREE.InstancedBufferGeometry {

  constructor(centers, n) {
    super()
    this.setIndex(new THREE.BufferAttribute(QUAD_INDEX, 1))
    this.setAttribute('position', new THREE.BufferAttribute(QUAD_POSITION, 3))
    this.setAttribute('pointCenter', new THREE.InstancedBufferAttribute(centers, 3))
    this.setAttribute('pointSize', new THREE.InstancedBufferAttribute(new Float32Array(n), 1))
    this.setAttribute('pointColor', new THREE.InstancedBufferAttribute(new Float32Array(n * 3), 3))
    this.instanceCount = n
  }

  computeBoundingBox() {
    if (this.boundingBox === null) this.boundingBox = new THREE.Box3()
    this.boundingBox.setFromBufferAttribute(this.attributes.pointCenter)
  }

  computeBoundingSphere() {
    if (this.boundingSphere === null) this.boundingSphere = new THREE.Sphere()
    this.computeBoundingBox()
    this.boundingBox.getBoundingSphere(this.boundingSphere)
  }
}

//
// material
//

const magUniform = uniform(1.0)
const colorUniform = uniform(new THREE.Color(0xffffff))

const pointSize = attribute('pointSize', 'float')
const pointCenter = attribute('pointCenter', 'vec3')

// carried to the fragment stage: the quad corner gives the impostor its normal,
// and pointColor *is* the data -- the value -> hue/lightness mapping in viz.js
const vCorner = varying(positionGeometry.xy, 'vCorner')
const vColor = varying(attribute('pointColor', 'vec3'), 'vColor')

const vertexNode = Fn(() => {
  const mvPosition = modelViewMatrix.mul(vec4(pointCenter, 1.0))
  const clip = cameraProjectionMatrix.mul(mvPosition)

  // gl_PointSize, verbatim -- framebuffer pixels, attenuated by view depth
  const sizePx = magUniform.mul(pointSize).div(mvPosition.z.negate())

  // A GL point of size N covered N framebuffer pixels, and NDC spans 2 units
  // across the viewport. viewportSize is already in physical pixels and tracks
  // setViewport() (ScreenNode multiplies getViewport() by the pixel ratio), so
  // this reproduces that footprint exactly -- including in the magnifier pass,
  // which renders the same scene through a much smaller viewport.
  const offset = vCorner.mul(sizePx).mul(2.0).div(viewportSize)

  // undo the perspective divide the rasteriser is about to apply
  return clip.add(vec4(offset.mul(clip.w), 0, 0))
})()

// Footprint of the ball in the sprite this shading replaced: its alpha mask
// covered 55 of ball.png's 64 texels, so the impostor is scaled to match and
// elements keep the apparent size every layout gap was tuned against.
const BALL_R = 0.86

// Brightness floor. Shading multiplies vColor, and vColor is the data, so the
// floor keeps low-value elements from shading away entirely. Every element is
// lit by the same normal field under the same lights, so relative lightness
// still reads.
const AMBIENT = 0.6

const fragmentNode = Fn(() => {
  // corner is +/-0.5, so doubling gives the [-1, 1] range gl_PointCoord*2-1
  // did. No y negation: gl_PointCoord pointed down from the sprite's top-left
  // and this quad is built +y up, so negating would put the key light on the
  // wrong side of every element.
  const p = vCorner.mul(2.0 / BALL_R)
  const r2 = dot(p, p)

  // hard silhouette -- the point cloud is dense and unsorted, so it cannot
  // afford alpha blending
  Discard(r2.greaterThan(1.0))

  // max() only guards against sqrt(-0) on the discarded edge; r2 < 1 here
  const n = vec3(p.x, p.y, sqrt(max(float(1.0).sub(r2), 0.0)))

  // Lights live in view space: the quads are billboards, so there is no
  // per-fragment world normal to light against, and a fixed view-space key
  // keeps the highlight on the same side of the screen as the camera orbits.
  const v = vec3(0.0, 0.0, 1.0)
  const key = normalize(vec3(-0.45, 0.62, 0.64))   // above and to the left
  const fill = normalize(vec3(0.62, -0.30, 0.45))  // dim bounce, below right
  const h = normalize(key.add(v))

  // Wrapped diffuse, so the terminator falls off gradually enough to read as
  // roundness even when an element is only a few pixels across.
  const wrap = 0.35
  const diffuse = dot(n, key).add(wrap).div(1.0 + wrap).mul(0.82)
    .add(max(dot(n, fill), 0.0).mul(0.26)).clamp(0.0, 1.0)

  const base = colorUniform.mul(vColor).mul(diffuse.mul(1.0 - AMBIENT).add(AMBIENT))

  // Gloss: a tight specular lobe plus a broad sheen, both hue-neutral so they
  // read as a surface property rather than as a shift in the encoded value.
  const nh = max(dot(n, h), 0.0)
  const gloss = pow(nh, 42.0).mul(0.6).add(pow(nh, 5.0).mul(0.1))

  // Reflection: a two-tone environment -- cool from above, warm bounce from
  // below -- weighted by Fresnel, so it shows only at grazing angles.
  const fresnel = pow(float(1.0).sub(max(n.z, 0.0)), 3.0)
  const reflection = mix(vec3(0.42, 0.34, 0.28), vec3(0.62, 0.72, 0.95),
    n.y.mul(0.5).add(0.5)).mul(fresnel).mul(0.3)

  // Screen rather than add: highlights brighten monotonically, so two elements
  // holding different values never clip to the same white.
  const lit = float(1.0).sub(
    float(1.0).sub(base).mul(float(1.0).sub(reflection.add(gloss).clamp(0.0, 1.0))))

  // The discard above leaves a hard edge; fading the outermost 2% is the only
  // antialiasing available without turning blending on.
  const out = lit.mul(float(1.0).sub(smoothstep(0.98, 1.0, sqrt(r2))))

  // Cancel the output colour-space conversion, so this lands in the framebuffer
  // as the value computed above.
  //
  // Under WebGL that needed no doing: the linear->sRGB encode was a shader
  // chunk that built-in materials included and ShaderMaterial did not, so this
  // shading wrote through untouched. WebGPU applies it to the whole frame
  // instead of per material, and a material cannot decline. Pre-decoding here
  // makes the round trip an identity and keeps elements the colour they have
  // always been.
  //
  // What this preserves is also, strictly, a colour-management bug inherited
  // from upstream mm: colorFromData() builds these values with setHSL(), which
  // stores linear, and they were then written as if they were already sRGB --
  // so elements read darker and more saturated than the HSL asked for. Dropping
  // this one line is the whole of the fix, and changes how every element looks.
  return colorSpaceToWorking(vec4(out, 1.0), THREE.SRGBColorSpace)
})()

// A bare NodeMaterial with fragmentNode set writes its result straight out --
// NodeMaterial.setupOutput() applies only fog and premultiplied alpha, and the
// output-colour-space conversion sits on the branch taken when fragmentNode is
// null. That is what the ShaderMaterial this replaces did too, so colours land
// in the framebuffer unconverted, exactly as before.
export const MATERIAL = new THREE.NodeMaterial()
MATERIAL.vertexNode = vertexNode
MATERIAL.fragmentNode = fragmentNode
MATERIAL.fog = false
MATERIAL.toneMapped = false
MATERIAL.transparent = false
MATERIAL.side = THREE.DoubleSide

// main.js drives the magnifier through `MATERIAL.uniforms.mag.value`, which was
// a ShaderMaterial uniform. A TSL uniform node carries the same `.value`, so
// exposing it under the old name keeps that call site unchanged. NodeMaterial
// ignores `.uniforms` itself.
MATERIAL.uniforms = { mag: magUniform, color: colorUniform }

//
// object
//

const _inverseMatrix = new THREE.Matrix4()
const _ray = new THREE.Ray()
const _sphere = new THREE.Sphere()
const _position = new THREE.Vector3()

/**
 * Drop-in replacement for the THREE.Points this used to be. viz.js still
 * reaches for `geometry.attributes.pointSize` / `.pointColor` and still
 * raycasts it expecting `intersects[].index` to be the element index, so both
 * are preserved; only the primitive underneath changed.
 */
export class PointCloud extends THREE.Mesh {

  constructor(centers, n) {
    super(new PointCloudGeometry(centers, n), MATERIAL)
    this.isPointCloud = true
  }

  /**
   * three.js's Points.raycast(), with the element centres read from the
   * instanced `pointCenter` attribute instead of `position`. Kept faithful to
   * the original on three counts that viz.js depends on: it still honours
   * `raycaster.params.Points.threshold` (viz.js sets it per call from
   * params.deco.spotlight), it still returns `index` in element order so
   * `index / W` and `index % W` recover the row and column, and it still drops
   * hits outside `[raycaster.near, raycaster.far]` -- main.js switches the
   * whole spotlight off by setting `far = 0`.
   */
  raycast(raycaster, intersects) {
    const geometry = this.geometry
    const matrixWorld = this.matrixWorld
    const threshold = raycaster.params.Points.threshold

    if (geometry.boundingSphere === null) geometry.computeBoundingSphere()

    _sphere.copy(geometry.boundingSphere)
    _sphere.applyMatrix4(matrixWorld)
    _sphere.radius += threshold

    if (raycaster.ray.intersectsSphere(_sphere) === false) return

    _inverseMatrix.copy(matrixWorld).invert()
    _ray.copy(raycaster.ray).applyMatrix4(_inverseMatrix)

    const localThreshold = threshold / ((this.scale.x + this.scale.y + this.scale.z) / 3)
    const localThresholdSq = localThreshold * localThreshold

    const centers = geometry.attributes.pointCenter

    for (let i = 0; i < centers.count; i++) {
      _position.fromBufferAttribute(centers, i)

      const rayPointDistanceSq = _ray.distanceSqToPoint(_position)
      if (rayPointDistanceSq >= localThresholdSq) continue

      const intersectPoint = new THREE.Vector3()
      _ray.closestPointToPoint(_position, intersectPoint)
      intersectPoint.applyMatrix4(matrixWorld)

      const distance = raycaster.ray.origin.distanceTo(intersectPoint)
      if (distance < raycaster.near || distance > raycaster.far) continue

      intersects.push({
        distance,
        distanceToRay: Math.sqrt(rayPointDistanceSq),
        point: intersectPoint,
        index: i,
        face: null,
        faceIndex: null,
        barycoord: null,
        object: this,
      })
    }
  }
}
