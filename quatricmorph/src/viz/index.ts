// @ts-nocheck
/** Public viz API — re-exports focused modules for a stable import surface. */

export { MATERIAL } from './material.js'

export {
  INIT_FUNCS,
  INITS,
  useRange,
  useDropout,
  getInitFunc,
} from './init.js'

export {
  EPILOGS,
  POINTWISE,
  applyInPlaceEpilog_,
} from './epilog.js'

export { Array2D, toRange } from './array2d.js'

export {
  setElemSize,
  setElemScale,
  grid,
  emptyPoints,
} from './sizing.js'

export { Mat } from './mat.js'

export {
  SCHEMES,
  POLARITIES,
  LEFT_PLACEMENTS,
  RIGHT_PLACEMENTS,
  RESULT_PLACEMENTS,
  SENSITIVITIES,
  TOP_LEVEL_ANIM_ALGS,
  ANIM_ALGS,
  FUSE_MODE,
  layoutDesc,
  ensureChildCounts,
} from './constants.js'

export { MatMul } from './matmul.js'

export {
  LAYOUT_RULES,
  childLayout,
  setLayoutScheme,
} from './layout.js'

export {
  default_dims,
  defaultCam,
  default_epilog,
  defaultLeft,
  defaultRight,
  defaultAnim,
  defaultBlock,
  defaultLayout,
} from './defaults.js'

export {
  fixBlocks,
  fixShape,
  leftLeaf,
  rightLeaf,
  syncExpr,
  genExpr,
} from './expr.js'
