// @ts-nocheck
const ERF_A1 = 0.254829592
const ERF_A2 = -0.284496736
const ERF_A3 = 1.421413741
const ERF_A4 = -1.453152027
const ERF_A5 = 1.061405429
const ERF_P = 0.3275911

function erf(x) {
  const absx = Math.abs(x)
  const t = 1.0 / (1.0 + ERF_P * absx)
  const y = (((((ERF_A5 * t + ERF_A4) * t) + ERF_A3) * t + ERF_A2) * t + ERF_A1) * t
  return Math.sign(x) * (1 - y * Math.exp(-absx * absx))
}

const SQRT2 = Math.sqrt(2)

const gelu = x => x * (1 + erf(x / SQRT2)) / 2

const sigmoid = x => 1 / (1 + Math.exp(-x))

const silu = x => x * sigmoid(x)

const relu = x => Math.max(0, x)

const pow2 = x => x ** 2


export const POINTWISE = {
  'relu': relu,
  'gelu': gelu,
  'sigmoid': sigmoid,
  'silu': silu,
  'tanh': Math.tanh,
  'x**2': pow2,
}

// epilogs
// TODO the way epis are done is kind of messy rn

export const EPILOGS = [
  'none',
  'relu',
  'gelu',
  'sigmoid',
  'silu',
  'tanh',
  'layernorm',
  'softmax',
  'softmax(x/sqrt(k))',
  'softmax(tril(x/sqrt(k)))',
  'softmax(tril(x/8))',
  'x/k',
  'x/sqrt(k)',
  'x**2',
]

function softmax_(h, w, data, tril = false) {

  const row_max = (ptr, w) => {
    let x = 0
    for (let j = 0; j < w; j++, ptr++) {
      x = Math.max(x, data[ptr])
    }
    return x
  }

  const calc_denom = (ptr, w, rmax) => {
    let d = 0
    for (let j = 0; j < w; j++, ptr++) {
      d += Math.exp(data[ptr] - rmax)
      if (!isFinite(d)) {
        // console.log(`HEY denom at data[${ptr}) = ${data[ptr]} becomes infinite`)
        break
      }
    }
    return d
  }

  for (let i = 0, ptr = 0; i < h; i++) {
    const rmax = row_max(ptr, tril ? i + 1 : w)
    const denom = calc_denom(ptr, tril ? i + 1 : w, rmax)
    for (let j = 0; j < w; j++, ptr++) {
      const x = tril && j > i ? 0 : Math.exp(data[ptr] - rmax) / denom
      if (isNaN(x)) {
        // console.log(`HEY Math.exp(data[${ptr}) = ${data[ptr]}]) / ${denom} is NaN`)
        data[ptr] = 0
      } else {
        data[ptr] = x
      }
    }
  }
}

const softmax_tril_ = (h, w, data) => softmax_(h, w, data, true)

function layernorm_(h, w, data) {
  const mean = data.reduce((acc, x) => acc + x) / data.length
  const mean2 = data.map(x => x ** 2).reduce((acc, x) => acc + x) / data.length
  const variance = mean2 - mean ** 2
  const denom = Math.sqrt(variance + 1e-5)
  const n = h * w
  for (let ptr = 0; ptr < n; ptr++) {
    const x = data[ptr]
    data[ptr] = (x - mean) / denom
  }
}

const IN_PLACE_EPILOGS = {
  'softmax': softmax_,
  'softmax(x/sqrt(k))': softmax_,
  'softmax(tril(x/sqrt(k)))': softmax_tril_,
  'softmax(tril(x/8))': softmax_tril_, // TODO remove with epi cleanup
  'layernorm': layernorm_,
}

const getInPlaceEpilog = name => IN_PLACE_EPILOGS[name]

export function applyInPlaceEpilog_(data, h, w, epi) {
  const epi_ = epi && getInPlaceEpilog(epi)
  if (epi_) {
    epi_(h, w, data)
  }
}
