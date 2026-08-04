// @ts-nocheck
import * as THREE from 'three'
import { MATERIAL } from './material.js'

// Block iteration lives in `math/blocking.ts` — pure index arithmetic with no
// Three.js dependency, so it can be tested without a WebGL context and reused
// by the tile compiler. Re-exported here to keep the legacy import path working.
export { gridIterate as grid } from '../math/blocking.js'

export let elem_scale = 1.25
export let elem_size = elem_scale

export function setElemScale(s) {
  s ||= elem_scale
  const old_elem_scale = elem_scale
  elem_scale = s
  elem_size *= elem_scale / old_elem_scale
}

export function setElemSize(scale, pixel_ratio) {
  elem_size = elem_scale * Math.min(scale.x, scale.y) * pixel_ratio
}

export const ZERO_COLOR = new THREE.Color(0, 0, 0)
export const COLOR_TEMP = new THREE.Color()

export function emptyPoints(h, w, info) {
  const { i: { size: si }, j: { size: sj }, gap } = info
  const n = h * w
  const points = new Float32Array(n * 3)
  for (let i = 0, ptr = 0; i < h; i++) {
    const ioff = Math.floor(i / si)
    for (let j = 0; j < w; j++) {
      const joff = Math.floor(j / sj)
      points[ptr++] = j + joff * gap
      points[ptr++] = i + ioff * gap
      points[ptr++] = 0
    }
  }
  const geom = new THREE.BufferGeometry()
  geom.setAttribute('position', new THREE.BufferAttribute(points, 3))
  geom.setAttribute('pointSize', new THREE.Float32BufferAttribute(new Float32Array(n), 1))
  geom.setAttribute('pointColor', new THREE.Float32BufferAttribute(new Float32Array(n * 3), 3))
  return new THREE.Points(geom, MATERIAL)
}

