// @ts-nocheck
import * as util from '../util.js'
import { setLayoutScheme } from './layout.js'
import {
  default_epilog, defaultAnim, defaultBlock, defaultLayout, defaultLeft, defaultRight,
} from './defaults.js'

export function fixBlocks(p, anc, root) {
  const getInfo = (p, anc, root) => {
    const is_root = anc.length == 0
    const pp = !is_root && anc[0](root)
    const panc = !is_root && anc.slice(1)
    const is_left = pp && p == pp.left
    const is_right = pp && p == pp.right
    return { is_left, is_right, pp, panc }
  }

  // from a given p, set i all the way down
  const setib = (i, p) => {
    p.block['i blocks'] = i
    p.left.block && setib(i, p.left)
  }

  // from a given p, set j all the way down
  const setjb = (j, p) => {
    p.block['j blocks'] = j
    p.right.block && setjb(j, p.right)
  }

  // from a given p, set k all the way down
  const setkb = (k, p) => {
    p.block['k blocks'] = k
    p.left.block && setjb(k, p.left)
    p.right.block && setib(k, p.right)
  }

  // return p and setter for where your i starts
  const iroot = (p, anc, root) => {
    const { is_left, is_right, pp, panc } = getInfo(p, anc, root)
    return is_left ? iroot(pp, panc, root) : is_right ? { p: pp, f: setkb } : { p, f: setib }
  }

  // return p and setter for where your j starts
  const jroot = (p, anc, root) => {
    const { is_left, is_right, pp, panc } = getInfo(p, anc, root)
    return is_right ? jroot(pp, panc, root) : is_left ? { p: pp, f: setkb } : { p, f: setjb }
  }

  const ir = iroot(p, anc, root)
  ir.f(p.block['i blocks'], ir.p)

  const jr = jroot(p, anc, root)
  jr.f(p.block['j blocks'], jr.p)

  // k always starts here
  setkb(p.block['k blocks'], p)
}

// adjust surroundings to match a param node's h/w
export function fixShape(h, w, p, anc, root) {
  const height = p => p.left ? height(p.left) : p.h
  const width = p => p.right ? width(p.right) : p.w

  const seth = (p, h) => p.left ? seth(p.left, h) : (p.h = h)
  const setw = (p, w) => p.right ? setw(p.right, w) : (p.w = w)

  const pp = anc[0](root)
  p === pp.left ? seth(pp.right, w) : setw(pp.left, h)
  anc.length > 1 && fixShape(height(pp.left), width(pp.right), pp, anc.slice(1), root)
}

export const leftLeaf = p => p.left.matmul ? leftLeaf(p.left) : p.left
export const rightLeaf = p => p.right.matmul ? rightLeaf(p.right) : p.right

// parseExpr, syncExpr

function parseExpr(s) {
  try {
    const node = spec => typeof spec == 'string' ? { name: spec } : make(spec)
    const make = spec => {
      const i = spec[1] == '=' ? 2 : 0
      const rname = r => /\s+/.test(r.name) ? '(' + r.name + ')' : r.name
      const f = (left, x) => {
        const right = node(x)
        return { left, right, name: left.name + ' @ ' + rname(right) }
      }
      const p = spec.slice(i + 1).reduce(f, node(spec[i]))
      i > 0 && (p.name = spec[0])
      return p
    }
    s = '[' + s.replace(/\s+/g, '').
      replace(/(\w+[\w\.\-\!\#\$\%\^\&\/\[\]]*)/g, '"$1"').
      replaceAll('@', ',').
      replaceAll('(', '[').
      replaceAll(')', ']').
      replaceAll('=', ',"=",') + ']'
    let spec = eval?.(s)
    while (spec.length == 1) {
      spec = spec[0]
    }
    return make(spec)
  } catch (e) {
    console.log(`error evaluating '${s}': ${e.message}`)
  }
}

export function syncExpr(params) {
  if (params.expr == genExpr(params)) {
    return true
  }

  const foundParams = {}

  const findParams = (p, n) => p.name == n ?
    (foundParams[p.name] = p) :
    (p.left && findParams(p.left, n)) ||
    (p.right && findParams(p.right, n)) ||
    undefined

  const childParams = (p, is_left) => {
    const found = findParams(params, p.name)
    if (p.left && p.right) {
      if (found && found.left && found.right) {
        return {
          ...util.copyTree(found),
          left: childParams(p.left, true),
          right: childParams(p.right, false),
          matmul: true,
        }
      } else {
        const cp = {
          epilog: default_epilog,
          anim: defaultAnim(),
          block: defaultBlock(),
          layout: defaultLayout(),
          left: childParams(p.left, true),
          right: childParams(p.right, false),
          name: p.name,
          matmul: true,
        }
        if (found) {
          leftLeaf(cp).h = found.h
          rightLeaf(cp).w = found.w
        }
        return cp
      }
    } else {
      if (found) {
        return !(found.left && found.right) ? util.copyTree(found) : {
          ...(is_left ? leftLeaf(found) : rightLeaf(found)),
          w: rightLeaf(found).w,
          name: p.name,
          matmul: false,
        }
      }
      return {
        ...(is_left ? defaultLeft() : defaultRight()),
        name: p.name,
        matmul: false,
      }
    }
  }

  const fixShapes = (p, anc = [p => p]) => {
    if (p.left && p.right) {
      const path = anc[0]
      if (!foundParams[p.right.name]) {
        fixShapes(p.left, [p => path(p).left].concat(anc))
        fixShapes(p.right, [p => path(p).right].concat(anc))
      } else {
        fixShapes(p.right, [p => path(p).right].concat(anc))
        fixShapes(p.left, [p => path(p).left].concat(anc))
      }
    } else {
      fixShape(p.h, p.w, p, anc.slice(1), new_params)
    }
  }

  const p = parseExpr(params.expr)
  if (!p) {
    return false
  }

  const new_params = {
    name: p.name,
    left: childParams(p.left, true),
    right: childParams(p.right, false)
  }

  fixShapes(new_params)
  util.updateProps(params, new_params)
  setLayoutScheme(params)

  return true
}

export function genExpr(p) {
  const passign = e => /^\w+\s+=/.test(e) ? `(${e})` : e
  const l = p.left.matmul ? passign(genExpr(p.left)) : p.left.name
  const r = p.right.matmul ? '(' + genExpr(p.right) + ')' : p.right.name
  const expanded = `${l} @ ${r}`
  const named = `${p.left.name} @ ${p.right.name}`
  return p.name == expanded || p.name == named ? expanded : `${p.name} = ${expanded}`
}


