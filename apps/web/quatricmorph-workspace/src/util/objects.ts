// @ts-nocheck

function updatePropRec(obj, k, v) {
  (typeof obj[k] == 'object' && typeof v == 'object') ? updatePropsRec(obj[k], v) : (obj[k] = v)
}

export function updatePropsRec(obj, donor) {
  Object.entries(donor).forEach(([k, v]) => updatePropRec(obj, k, v))
}

export function updateProps(obj, donor) {
  Object.entries(donor).forEach(([k, v]) => obj[k] = v)
}

export function deleteProps(obj, props) {
  Object.keys(obj).forEach(k => props.includes(k) && delete obj[k])
  return obj
}

export function syncProp(obj, k, v) {
  if (v === undefined) {
    return obj[k]
  }
  obj[k] = v
  return v
}

// NOTE only handles our nested params - nothing null 
// or undefined, no arrays, no empty subobjects, etc
export function flatten(obj) {
  const f = (obj, pre) => Object.entries(obj).reduce((acc, [k, v]) => ({
    ...acc,
    ...(typeof v === 'object' ? f(obj[k], pre + k + '.') : { [pre + k]: v })
  }), {})
  return f(obj, '')
}

export function unflatten(flat) {
  const add = (obj, [k, v]) => {
    const i = k.indexOf('.')
    if (i >= 0) {
      const [base, suf] = [k.slice(0, i), k.slice(i + 1)]
      obj[base] = add(obj[base] || {}, [suf, v])
    } else {
      obj[k] = v
    }
    return obj
  }
  return Object.entries(flat).reduce(add, {})
}

export function compress(obj) {
  const names = {}
  const getname = p =>
    p == '' ? '' : names[p] || (names[p] = `${Object.keys(names).length}`)
  const getpath = p => {
    const i = p.lastIndexOf('.')
    return i == -1 ? getname(p) : `${getname(p.slice(0, i))}.${getname(p.slice(i + 1))}`
  }
  const comp = {}
  Object.entries(obj).forEach(([k, v]) => comp[getpath(k)] = v)
  Object.entries(names).forEach(([k, v]) => comp[k] = v)
  return comp
}

export function uncompress(comp) {
  const [names, props] = [[], []]
  Object.entries(comp).forEach(([k, v]) => +k == k ? (props[k] = v) : (names[v] = k))
  const getpath = n => {
    const i = n.indexOf('.')
    return i == -1 ? names[n] : `${names[n.slice(0, i)] + '.' + names[n.slice(i + 1)]}`
  }
  const obj = {}
  Object.entries(props).forEach(([k, v]) => obj[getpath(k)] = v)
  return obj
}

export function copyTree(obj) {
  return unflatten({ ...flatten(obj) })
}

//
// misc THREE utils
//

export function disposeAndClear(obj) {
  obj.geometry && obj.geometry.dispose()
  obj.children && obj.children.map(disposeAndClear)
  obj.clear()
}

