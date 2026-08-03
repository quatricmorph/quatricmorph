// @ts-nocheck
import * as THREE from 'three'

const MMGUIDE_MATERIAL = new THREE.RawShaderMaterial({
  glslVersion: THREE.GLSL1,
  vertexShader: `
  precision mediump float;
  precision mediump int;
  uniform mat4 modelViewMatrix; // optional
  uniform mat4 projectionMatrix; // optional
  attribute vec3 position;
  attribute vec4 color;
  varying vec3 vPosition;
  varying vec4 vColor;
  void main()	{
    vPosition = position;
    vColor = color;
    gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
  }`,
  fragmentShader: `
  precision mediump float;
  precision mediump int;
  varying vec3 vPosition;
  varying vec4 vColor;
  void main()	{
    vec4 color = vec4( vColor );
    gl_FragColor = color;
  }`,
  side: THREE.DoubleSide,
  transparent: true
});

//
// reading/writing params
//

export function lineSeg(start, end, color) {
  const material = new THREE.LineBasicMaterial({ color })
  const geometry = new THREE.BufferGeometry().setFromPoints([start, end])
  return new THREE.Line(geometry, material)
}

// x y z axis lines from origin
export function axes() {
  const origin = new THREE.Vector3(0, 0, 0)
  const group = new THREE.Group()
  group.add(lineSeg(origin, new THREE.Vector3(128, 0, 0), new THREE.Color(1, 0, 0)))
  group.add(lineSeg(origin, new THREE.Vector3(0, 128, 0), new THREE.Color(0, 1, 0)))
  group.add(lineSeg(origin, new THREE.Vector3(0, 0, 128), new THREE.Color(0, 0, 1)))
  return group
}

export function rowGuide(h, w, light = 1.0, denom = 8) {
  const group = new THREE.Group()
  const color = new THREE.Color()

  const draw = (i0, j0, i1, j1) => {
    const start = new THREE.Vector3(j0, i0, 0)
    const end = new THREE.Vector3(j1, i1, 0)
    color.setHSL(1.0, 0.0, light)
    group.add(lineSeg(start, end, color))
  }

  draw(0, 0, h - 1, 0)
  draw(0, w - 1, h - 1, w - 1)

  const rstride = Math.max(1, (h - 1) / denom)
  for (let i = 0; i < h; i += rstride) {
    draw(i, 0, i, w - 1)
  }

  draw(0, w / denom, h / denom, 0)

  return group
}

//
// mm flow guide arrow
// 

const LEFT_ARROW_COLOR = new THREE.Uint8BufferAttribute([
  150, 200, 255, 255,
  150, 200, 255, 255,
  150, 200, 255, 255,
], 4)
LEFT_ARROW_COLOR.normalized = true

const RIGHT_ARROW_COLOR = new THREE.Uint8BufferAttribute([
  255, 150, 150, 255,
  255, 150, 150, 255,
  255, 150, 150, 255,
], 4)
RIGHT_ARROW_COLOR.normalized = true

export function flowGuide(h, d, w, layout, scale = 1.0) {
  const light = 0.5 + scale / 2
  LEFT_ARROW_COLOR.array[3] = LEFT_ARROW_COLOR.array[7] = LEFT_ARROW_COLOR.array[3] = 255 * light
  LEFT_ARROW_COLOR.needsUpdate = true
  RIGHT_ARROW_COLOR.array[3] = RIGHT_ARROW_COLOR.array[7] = RIGHT_ARROW_COLOR.array[3] = 255 * light
  RIGHT_ARROW_COLOR.needsUpdate = true

  const { left, right, result, gap, left_scatter, right_scatter } = layout
  const extent = x => x + gap * 2 - 1
  const center = x => extent(x) / 2
  const place = (n, p, x) => p == 1 ? x : n - x
  const place_left = x => place(extent(w), left, x)
  const place_right = x => place(extent(h), right, x)
  const place_result = x => place(extent(d), result, x)

  const group = new THREE.Group()

  const left_geometry = new THREE.BufferGeometry()
  left_geometry.setAttribute('position', new THREE.Float32BufferAttribute([
    place_left(center(w) - (center(w) - gap + left_scatter) * scale),
    center(h),
    place_result(center(d)),

    place_left(center(w)),
    center(h),
    place_result(center(d)),

    place_left(center(w)),
    place_right(center(h)),
    place_result(center(d) - (center(d) - gap) * scale),
  ], 3))
  left_geometry.setAttribute('color', LEFT_ARROW_COLOR)
  group.add(new THREE.Mesh(left_geometry, MMGUIDE_MATERIAL));

  const right_geometry = new THREE.BufferGeometry()
  right_geometry.setAttribute('position', new THREE.Float32BufferAttribute([
    center(w),
    place_right(center(h) - (center(h) - gap + right_scatter) * scale),
    place_result(center(d)),

    center(w),
    center(h),
    place_result(center(d)),

    center(w),
    place_right(center(h)),
    place_result(center(d) - (center(d) - gap) * scale),
  ], 3))
  right_geometry.setAttribute('color', RIGHT_ARROW_COLOR)
  group.add(new THREE.Mesh(right_geometry, MMGUIDE_MATERIAL));

  return group
}

//
// bounding box stuff etc
//

export function bbhwd(bb) {
  return {
    h: bb.max.y - bb.min.y,
    w: bb.max.x - bb.min.x,
    d: bb.max.z - bb.min.z,
  }
}

export function gbbhwd(g) {
  return bbhwd(g.boundingBox)
}

export function center(x, y = 0) {
  return (x - y) / 2
}

//
// misc object utils
//

