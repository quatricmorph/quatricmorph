// @ts-nocheck
const getMm = () => document.getElementById("mm") as HTMLIFrameElement


const getDoc = () => document.querySelector('.doc')

let mmconfig

function setSearchParams() {
  const prefix = window.location.origin + window.location.pathname
  const params = {
    ...(mmconfig ? { mm: mmconfig } : {}),
    doc: JSON.stringify({ scroll: getDoc().scrollTop })
  }
  const search_params = new URLSearchParams(params)
  window.history.pushState(params, '', prefix + '?' + search_params)
}

const RESPONDERS = {
  search_params: search_params => {
    mmconfig = search_params
    setSearchParams()
  }
}

window.addEventListener('message', event => {
  Object.entries(event.data).forEach(([k, v]) => {
    const r = RESPONDERS[k]
    r && r(v)
  })
})

function withResponse(msg, resp, f) {
  const cleanup = () => {
    delete RESPONDERS[resp]
  }
  const timeout = setTimeout(cleanup, 1000)
  RESPONDERS[resp] = r => {
    f(r)
    clearTimeout(timeout)
    cleanup()
  }
  getMm().contentWindow.postMessage(msg)
}

function popout() {
  withResponse({ getUrlInfo: undefined }, 'url_info', info => window.open(info.url, '_blank'))
}

function update(f, clear = false, focus = true) {
  withResponse({ getParams: undefined }, 'params', params => set(f(params), clear))
}

function set(props = {}, reset = false, focus = true) {
  getMm().contentWindow.postMessage({ setParams: { props, reset } })
  focus && getMm().focus()
}

function openFolders(names = []) {
  return {
    folder: "open",
    anim: { folder: "closed" },
    block: { folder: "closed" },
    layout: { folder: "closed" },
    left: { folder: "closed" },
    right: { folder: "closed" },
    deco: { folder: "closed" },
    viz: { folder: "closed" },
    diag: { folder: "closed" },
    ...names.reduce((acc, f) => ({ ...acc, [f]: { folder: "open" } }), {})
  }
}

const openFolder = name => openFolders([name])

function jumpTo(id) {
  const dest = document.getElementById(id)
  dest && (dest.parentNode.scrollTop = dest.offsetTop - dest.parentNode.offsetTop)
}

const matmap = (p, lf, rf = undefined) => ({
  ...p,
  left: p.left.matmul ? matmap(p.left, lf, rf) : lf(p.left),
  right: p.right.matmul ? matmap(p.right, lf, rf) : (rf || lf)(p.right),
})

const mmsiz = p => {
  const info = p => {
    const lf = p.left.matmul ? info(p.left) : { h: p.left.h, n: p.left.h * p.left.w }
    const rt = p.right.matmul ? info(p.right) : { w: p.right.w, n: p.right.h * p.right.w }
    return { h: lf.h, w: rt.w, n: lf.n + rt.n + lf.h * rt.w }
  }
  return info(p).n
}

const faster = p => ({ anim: { speed: p.anim.speed * 2 } })
const slower = p => ({ anim: { speed: p.anim.speed / 2 } })

const up = n => Math.round(n * 2)
const bigger = p => mmsiz(p) > 49152 ? {} : matmap(p, m => ({ h: up(m.h), w: up(m.w) }))

const dn = n => Math.round(n / 2)
const smaller = p => matmap(p, m => ({ h: dn(m.h), w: dn(m.w) }))

const uniform = p => matmap(p, _ => ({ init: 'uniform' }))
const rowcol = p => matmap(p, _ => ({ init: 'rows' }), _ => ({ init: 'cols' }))

const mlp = {
  expr: 'batch @ w0 @ w1 @ w2',
  sync_expr: true,
  epilog: 'softmax',
  viz: { sensitivity: 'global' },
  left: {
    epilog: 'relu',
    left: {
      epilog: 'relu',
      left: { h: 64, w: 32 },
      right: { h: 32, w: 64, init: 'pt linear' }
    },
    right: { h: 64, w: 64, init: 'pt linear' }
  },
  right: { h: 64, w: 32, init: 'pt linear' }
}

const mlp_named = {
  ...mlp,
  expr: 'out = (x1 = (x0 = batch @ w0) @ w1) @ w2'
}

const mlp_lr = {
  expr: 'out = batch @ (w0 = w0_L @ w0_R) @ (w1 = w1_L @ w1_R) @ (w2 = w2_L @ w2_R)',
  sync_expr: true,
  epilog: 'softmax',
  viz: { sensitivity: 'global' },
  left: {
    'epilog': 'relu',
    left: {
      epilog: 'relu',
      left: { h: 64, w: 32 },
      right: {
        left: { h: 32, w: 8, init: 'pt linear' },
        right: { h: 8, w: 64, init: 'pt linear' }
      }
    },
    right: {
      left: { h: 64, w: 8, init: 'pt linear' },
      right: { h: 8, w: 64, init: 'pt linear' }
    }
  },
  right: {
    left: { h: 64, w: 8, init: 'pt linear' },
    right: { h: 8, w: 32, init: 'pt linear' }
  }
}

const mlp_lr_named = {
  ...mlp_lr,
  expr: 'out = (x1 = (x0 = batch @ (w0 = w0_L @ w0_R)) @ (w1 = w1_L @ w1_R)) @ (w2 = w2_L @ w2_R)'
}

const attn = {
  expr: 'out = (attn = Q @ K) @ V',
  sync_expr: true,
  epilog: 'none',
  left: {
    epilog: 'softmax(tril(x/sqrt(k)))',
    left: { h: 64, w: 16, init: 'gaussian' },
    right: { h: 16, w: 64, init: 'gaussian' }
  },
  right: { 'h': 64, 'w': 16, 'init': 'gaussian' },
  viz: { sensitivity: 'local' }
}

const attn_proj = {
  expr: 'out = (attn = (Q = input @ wQ) @ (K = wK @ input_t)) @ (V = input @ wV) @ wO',
  sync_expr: true,
  epilog: 'none',
  left: {
    left: {
      epilog: 'softmax(tril(x/sqrt(k)))',
      left: {
        left: { h: 64, w: 64, init: 'gaussian' },
        right: { h: 64, w: 16, init: 'gaussian' },
      },
      right: {
        left: { h: 16, w: 64, init: 'gaussian' },
        right: { h: 64, w: 64, init: 'gaussian' },
      }
    },
    right: {
      left: { h: 64, w: 64, init: 'gaussian' },
      right: { h: 64, w: 16, init: 'gaussian' },
    },
  },
  right: { h: 16, w: 64, init: 'gaussian' },
  viz: { sensitivity: 'local' }
}

const legends_only = { deco: { shape: true, legends: 6, 'row guides': 0, 'flow guides': 0 } }
const guides_only = { deco: { shape: false, legends: 0, 'row guides': 0.5, 'flow guides': 0.5 } }
const undeco = { deco: { shape: false, legends: 0, 'row guides': 0, 'flow guides': 0 } }
const default_deco = { deco: { shape: true, legends: 6, 'row guides': 0.5, 'flow guides': 0.5 } }

const reset = () => {
  window.location.href = window.location.origin + window.location.pathname
}


// drag resize
let dragging = false

document.getElementById("sep").addEventListener('pointerdown', e => {
  dragging = true
})

document.addEventListener('pointerup', e => {
  dragging = false
})

document.addEventListener('pointermove', e => {
  const dir = getComputedStyle(document.querySelector('body')).flexDirection;
  if (dragging) {
    if (dir == 'row') {
      const [uw, lw] = [document.getElementById("upper").offsetWidth, document.getElementById("lower").offsetWidth]
      const w = uw + lw
      const dx = e.clientX - document.getElementById("sep").offsetLeft
      document.getElementById("upper").style.width = `${100 * (uw + dx) / w}` + '%'
      document.getElementById("lower").style.width = `${100 * (lw - dx) / w}` + '%'
      document.getElementById("upper").style.height = '100%'
      document.getElementById("lower").style.height = '100%'
    } else { // column
      const [uh, lh] = [document.getElementById("upper").offsetHeight, document.getElementById("lower").offsetHeight]
      const h = uh + lh
      const dy = e.clientY - document.getElementById("sep").offsetTop
      document.getElementById("upper").style.height = `${100 * (uh + dy) / h}` + '%'
      document.getElementById("lower").style.height = `${100 * (lh - dy) / h}` + '%'
      document.getElementById("upper").style.width = '100%'
      document.getElementById("lower").style.width = '100%'
    }
    e.preventDefault()
  }
})

// portrait/landscape

const isLandscape = () => window.matchMedia("(orientation: landscape)").matches

function setDocScrollListener() {
  getDoc().addEventListener('scrollend', () => {
    setSearchParams()
  })
}

function arrange() {
  // viz needs to be in upper pane vert or *right* pane horiz
  if (document.getElementById("upper").className == 'viz' && isLandscape()) {
    document.getElementById("upper").style.height = document.getElementById("upper").style.width = document.getElementById("lower").style.height = document.getElementById("lower").style.width = ''
    const temp_html = document.getElementById("upper").innerHTML
    document.getElementById("upper").innerHTML = document.getElementById("lower").innerHTML
    document.getElementById("upper").className = 'doc'
    document.getElementById("lower").innerHTML = temp_html
    document.getElementById("lower").className = 'viz'
  } else if (document.getElementById("upper").className == 'doc' && !isLandscape()) {
    document.getElementById("upper").style.height = document.getElementById("upper").style.width = document.getElementById("lower").style.height = document.getElementById("lower").style.width = ''
    const temp_html = document.getElementById("upper").innerHTML
    document.getElementById("upper").innerHTML = document.getElementById("lower").innerHTML
    document.getElementById("upper").className = 'viz'
    document.getElementById("lower").innerHTML = temp_html
    document.getElementById("lower").className = 'doc'
  }
}

window.addEventListener('resize', arrange)

// init block
{
  const searchParams = new URL(window.location).searchParams
  mmconfig = searchParams.get('mm')
  mmconfig && (getMm().src = '/index.html?' + mmconfig)
  const docstr = searchParams.get('doc')
  // console.log(`HEY docstr ${docstr}`)
  if (docstr) {
    const doc = JSON.parse(docstr)
    const scroll = doc.scroll
    // console.log(`HEY scroll ${scroll}`)
    if (scroll > 0) {
      window.onload = () => {
        getDoc().scrollTop = scroll
      }
    }
  }
}

arrange()
setDocScrollListener()


// expose for javascript: hrefs in markdown
const _w = window as any
if (typeof popout !== "undefined") _w.popout = popout
if (typeof set !== "undefined") _w.set = set
if (typeof update !== "undefined") _w.update = update
if (typeof reset !== "undefined") _w.reset = reset
if (typeof jumpTo !== "undefined") _w.jumpTo = jumpTo
if (typeof openFolders !== "undefined") _w.openFolders = openFolders
if (typeof openFolder !== "undefined") _w.openFolder = openFolder
if (typeof faster !== "undefined") _w.faster = faster
if (typeof slower !== "undefined") _w.slower = slower
if (typeof bigger !== "undefined") _w.bigger = bigger
if (typeof smaller !== "undefined") _w.smaller = smaller
if (typeof uniform !== "undefined") _w.uniform = uniform
if (typeof rowcol !== "undefined") _w.rowcol = rowcol
if (typeof mlp !== "undefined") _w.mlp = mlp
if (typeof mlp_named !== "undefined") _w.mlp_named = mlp_named
if (typeof mlp_lr !== "undefined") _w.mlp_lr = mlp_lr
if (typeof mlp_lr_named !== "undefined") _w.mlp_lr_named = mlp_lr_named
if (typeof attn !== "undefined") _w.attn = attn
if (typeof attn_proj !== "undefined") _w.attn_proj = attn_proj
if (typeof legends_only !== "undefined") _w.legends_only = legends_only
if (typeof guides_only !== "undefined") _w.guides_only = guides_only
if (typeof undeco !== "undefined") _w.undeco = undeco
if (typeof default_deco !== "undefined") _w.default_deco = default_deco
