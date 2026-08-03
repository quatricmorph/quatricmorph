// @ts-nocheck
import {
  flatten,
  unflatten,
  compress,
  uncompress,
  updateProps,
} from './objects.js'

export function makeSearchParams(params) {
  const sp = params.compress ? compress(flatten(params)) : { params: JSON.stringify(params) }
  return new URLSearchParams(sp)
}

export function updateObjectFromSearchParams(obj, searchParams) {
  const from_search_params = {}
  searchParams.forEach((v, k) => from_search_params[k] = v)
  const keys = Object.keys(from_search_params)

  if (keys.length == 0) {
    return // avoids introducing compression
  }

  if (keys.length == 1 && keys[0] == 'params') {
    const params = from_search_params.params
    try {
      updateProps(obj, JSON.parse(params))
      obj.compress = false
    } catch (e) {
      console.log(`error loading params from json '${params}' message '${e.message}`)
    }
    return
  }

  if (keys.length == 1 && keys[0] == 'config') {
    const config = from_search_params.config
    try {
      console.log(`loading params from config url ${config}...`)
      const url = new URL(config)
      const req = new XMLHttpRequest()
      req.open("GET", url, false)
      req.send(null)
      const params = JSON.parse(req.responseText)
      console.log(`done loading params from config url ${config}`)
      updateProps(obj, params)
      obj.compress = false
    } catch (e) {
      console.log(`error loading params from config url '${config}' message '${e.message}`)
    }
    return
  }

  // otherwise search params are a compressed flattened object
  const flat_obj = flatten(obj)
  const unqual = k => k.slice(k.lastIndexOf('.') + 1)
  const add_unqual = (acc, [k, v]) => ({ ...acc, [unqual(k)]: v })
  const types = Object.entries(flat_obj).reduce(add_unqual, {})
  const update = uncompress(from_search_params)
  Object.entries(update).forEach(([k, v]) => {
    let x
    if (unqual(k) in types) {
      const t = typeof types[unqual(k)]
      x = castToType(v, t)
      if (x === undefined) {
        console.log(`don't know how to cast param '${k}' to type ${t}, using string ${v}`)
        x = v
      }
    } else {
      console.log(`unknown param '${k}', setting value ${v} as string`)
      x = v
    }
    flat_obj[k] = x
  })
  updateProps(obj, unflatten(flat_obj))
  obj.compress = true
}

// need this bc earch param values are always strings
// we only know a limited set of value types for simplicity
function castToType(v, t) {
  switch (t) {
    case 'boolean':
      return v == 'true'
    case 'number':
      return Number(v)
    case 'string':
      return String(v)
    default:
      return undefined
  }
}

//
// things with lines
//

