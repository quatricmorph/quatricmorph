// @ts-nocheck
import { applyInPlaceEpilog_ } from './epilog.js'

export function toRange(x, n) {
  return x === undefined ? [0, n] : x.constructor === Array ? x : [x, x + 1]
}

function initArrayData_(data, h, w, init, epi = undefined, r = undefined, c = undefined) {
  const [rstart, rend] = toRange(r, h)
  const [cstart, cend] = toRange(c, w)
  for (let i = rstart; i < rend; i++) {
    for (let j = cstart, ptr = i * w + cstart; j < cend; j++, ptr++) {
      data[ptr] = init(i, j, h, w)
    }
  }
  applyInPlaceEpilog_(data, h, w, epi)
}

export class Array2D {

  static fromInit(h, w, init, epi = undefined) {
    const data = new Float32Array(h * w)
    initArrayData_(data, h, w, init, epi)
    return new Array2D(h, w, data)
  }

  constructor(h, w, data) {
    this.h = h | 0
    this.w = w | 0
    this.data = data
  }

  reinit(f, epi = undefined, r = undefined, c = undefined) {
    initArrayData_(this.data, this.h, this.w, f, epi, r, c)
  }

  numel() {
    return this.h * this.w
  }

  get(i, j) {
    return this.data[this.addr(i, j)]
  }

  slice(i = undefined, j = undefined) {
    const [istart, iend] = toRange(i, this.h)
    const [jstart, jend] = toRange(j, this.w)
    const init = (i, j, h, w) => this.get(istart + i, jstart + j)
    return Array2D.fromInit(iend - istart, jend - jstart, init)
  }

  addr(i, j) {
    return i * this.w + j
  }

  absmax() {
    const data = this.data
    let absmax = 0
    for (let i = 0; i < data.length; i++) {
      const absx = Math.abs(data[i])
      if (absmax < absx) {
        absmax = absx
      }
    }
    return absmax
  }

  absmin() {
    const data = this.data
    let absmin = Infinity
    for (let i = 0; i < data.length; i++) {
      const absx = Math.abs(data[i])
      if (!isFinite(absmin) || absx < absmin) {
        absmin = absx
      }
    }
    return absmin
  }

  transpose() {
    return Array2D.fromInit(this.w, this.h, (i, j) => this.get(j, i))
  }

  map(f) {
    const n = this.h * this.w
    const data = new Float32Array(n)
    for (let ptr = 0; ptr < n; ptr++) {
      data[ptr] = f(this.data[ptr])
    }
    return new Array2D(this.h, this.w, data)
  }

  map2(f, a) {
    if (a.h != this.h || a.w != this.w) {
      throw Error(`shape error: this ${this.h} ${this.w} a ${a.h} ${a.w}`)
    }
    const n = this.h * this.w
    const data = new Float32Array(n)
    for (let ptr = 0; ptr < n; ptr++) {
      data[ptr] = f(this.data[ptr], a.data[ptr])
    }
    return new Array2D(this.h, this.w, data)
  }

  add(a) {
    return this.map2((x, y) => x + y, a)
  }
}

//
//
//

