// @ts-nocheck
function gaussianRandom(mean = 0, stdev = 1) {
  let u = 1 - Math.random() //Converting [0,1) to (0,1)
  let v = Math.random()
  let z = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v)
  // Transform to the desired mean and standard deviation:
  return z * stdev + mean
}

// https://github.com/facebookresearch/shumai/blob/main/test/gradient.test.ts#L5
function sampleSphere(args) {
  const u = sm.randn(args)
  const d = sm.sum(u.mul(u)).sqrt()
  return u.div(d)
}

export const INIT_FUNCS = {
  rows: (i, j, h) => h > 1 ? i / (h - 1) : 0,
  cols: (i, j, h, w) => w > 1 ? j / (w - 1) : 0,
  'row major': (i, j, h, w) => h * w > 1 ? (i * w + j) / (h * w - 1) : 0,
  'col major': (i, j, h, w) => h * w > 1 ? (j * h + i) / (h * w - 1) : 0,
  'pt linear': (i, j, h, w) => (2 * Math.random() - 1) / Math.sqrt(w),
  uniform: () => Math.random(),
  gaussian: () => gaussianRandom(0.5, 0.5),
  // sphere: (i, j, h, w) => sampleSphere([h, w]),
  'tril mask': (i, j) => j <= i ? 1 : 0,
  'triu mask': (i, j) => j >= i ? 1 : 0,
  eye: (i, j) => +(i == j),
  diff: (i, j) => i == j ? 1 : i == j + 1 ? -1 : 0,
}

export const INITS = Object.keys(INIT_FUNCS).concat(['url', 'expr'])

const USE_RANGE = ['rows', 'cols', 'row major', 'col major', 'uniform', 'gaussian']
const USE_DROPOUT = USE_RANGE.concat(['pt linear'])

export const useRange = name => USE_RANGE.indexOf(name) >= 0
export const useDropout = name => USE_DROPOUT.indexOf(name) >= 0

const DATA_CACHE = {}

function tryLoadData(data_url) {
  if (DATA_CACHE[data_url]) {
    return DATA_CACHE[data_url]
  }
  try {
    console.log(`loading data from ${data_url}...`)
    const url = new URL(data_url)
    const req = new XMLHttpRequest()
    req.open("GET", url, false)
    req.send(null)
    DATA_CACHE[url] = req.responseText.split(/\r?\n|\r/).map(l => l.split(',').map(s => +s))
    console.log(`done loading data from ${data_url}`)
    return DATA_CACHE[url]
  } catch (e) {
    console.log(`error loading data from URL '${data_url}' message '${e.message}`)
  }
}

function tryURLInit(url) {
  const data = tryLoadData(url)
  if (data) {
    return (i, j, h, w) => {
      const row = data[i % data.length]
      return row[j % row.length]
    }
  }
}

function tryEvalInitExpr(expr) {
  try {
    return eval?.(`(i, j, h, w) => { try { return (${expr}) } catch (e) { return 0 } }`)
  } catch ({ name, message }) {
    console.log(`error ${name} evaluating init expr '${expr}' message '${message}'`)
    return () => 0
  }
}

export function getInitFunc(init_params) {
  const { init, min, max, dropout, url, expr } = init_params
  const f = INIT_FUNCS[init] ||
    (init == 'url' && tryURLInit(url)) ||
    (init == 'expr' && tryEvalInitExpr(expr))
  if (!f) {
    console.log(init == 'url' ?
      `'can't load from URL '${url}'` :
      `unrecognized initializer '${init}'`)
    return () => 0
  }
  const scaled = useRange(init) && (min != 0 || max != 1) ?
    (i, j, h, w) => min + Math.max(0, max - min) * f(i, j, h, w) :
    f
  const sparse = useDropout(init) && dropout > 0 ?
    (i, j, h, w) => Math.random() > dropout ? scaled(i, j, h, w) : 0 :
    scaled
  return sparse
}

// pointwise funcs
