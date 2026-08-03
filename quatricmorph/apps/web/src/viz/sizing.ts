// @ts-nocheck
import * as THREE from 'three'
import { MATERIAL } from './material.js'

export function grid(info, dims, f) {
  const infos = Array.from(dims).map(d => info[d])
  const loop = (args, infos, f) => infos.length == 0 ?
    f(...args) :
    [...Array(infos[0].n).keys()].map(index => {
      const { size, max } = infos[0]
      const start = index * size
      if (start < max) {  // dead final block when size * n - max > size
        const end = Math.min(start + size, max)
        const extent = end - start
        loop([...args, { index, start, end, extent }], infos.slice(1), f)
      }
    })
  loop([], infos, f)
}

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

