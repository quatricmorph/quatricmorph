export { validateMatmulDims } from './validate.js'
export type { DimResult, DimOk, DimError } from './validate.js'
export { matmul, dotprodCell } from './matmul.js'
export { inferTensorKind, shapeLabel } from './shape.js'
export type { TensorKind } from './shape.js'
export { parseMatrixText, matrixToText, flatFromRows } from './parse.js'
export type { ParseResult } from './parse.js'
export {
  fillPreset,
  DEFAULT_A,
  DEFAULT_B,
  DEFAULT_C,
} from './presets.js'
export type { PresetName } from './presets.js'
