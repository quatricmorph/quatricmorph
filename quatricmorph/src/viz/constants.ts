// @ts-nocheck
export const SCHEMES = ['blocks', 'zigzag', 'wheel', 'custom']
export const POLARITIES = ['negative', 'positive']
export const LEFT_PLACEMENTS = ['left', 'right']
export const RIGHT_PLACEMENTS = ['top', 'bottom']
export const RESULT_PLACEMENTS = ['front', 'back']

export function layoutDesc(layout) {
  const pol = { 'positive': '+', 'negative': '-', }[layout.polarity]
  const lfp = { 'left': 'L', 'right': 'R', }[layout['left placement']]
  const rtp = { 'top': 'T', 'bottom': 'B', }[layout['right placement']]
  const rsp = { 'front': 'F', 'back': 'B', }[layout['result placement']]
  return `${pol}${lfp}${rtp}${rsp}`
}

export const SENSITIVITIES = ['global', 'semilocal', 'local', 'superlocal']
export const TOP_LEVEL_ANIM_ALGS = [
  'none', 'dotprod (row major)', 'dotprod (col major)', 'axpy', 'vmprod', 'mvprod', 'vvprod',
]
export const ANIM_ALGS = TOP_LEVEL_ANIM_ALGS.concat('inherit')
export const FUSE_MODE = ['none', 'sync', 'async']

export const ensureChildCounts = p => {
  if (p.count === undefined) {
    p.count = p.matmul === false ? 0 :
      (1 + ensureChildCounts(p.left).count + ensureChildCounts(p.right).count)
    // sloppy - this means root
    if (p.matmul === undefined) {
      const total = p.count
      const setTotal = p => {
        p.total = total
        p.left && setTotal(p.left)
        p.right && setTotal(p.right)
      }
      setTotal(p)
    }
  }
  return p
}

