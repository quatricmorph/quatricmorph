// @ts-nocheck
import * as util from '../util.js'

export function urlPrefix() {
  return window.location.origin + window.location.pathname
}

export function createUrlInfo() {
  return { json: '', url: urlPrefix(), compressed: '', search_params: '' }
}

export function saveUrlInfo(params, url_info) {
  url_info.json = JSON.stringify(params)
  const prefix = urlPrefix()
  let search_params = util.makeSearchParams(params)
  if (!params.compress && search_params.toString().length > 2048) {
    params.compress = true
    search_params = util.makeSearchParams(params)
  }
  url_info.url = prefix + '?' + search_params
  url_info.compressed = prefix + '?' + util.makeSearchParams({ ...params, compress: true })
  url_info.search_params = '' + search_params
}

export function saveUrl(params, url_info) {
  saveUrlInfo(params, url_info)
  window.history.pushState({}, '', url_info.url)
  if (window.parent != window) {
    window.parent.postMessage({ search_params: url_info.search_params }, parent.origin)
  }
}
