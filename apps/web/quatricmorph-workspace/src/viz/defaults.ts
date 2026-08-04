// @ts-nocheck
import { DEFAULT_A, DEFAULT_B } from '../math/presets.js'
import { matrixToText } from '../math/parse.js'

export const default_dims = { i: 2, j: 3, k: 2 }

export const defaultCam = () => ({
  x: -12,
  y: 10,
  z: 14,
  target: { x: 0, y: 0, z: 0 },
})

export const default_epilog = 'none'

export const defaultLeft = () => ({
  name: 'A',
  matmul: false,
  h: DEFAULT_A.length,
  w: DEFAULT_A[0].length,
  init: 'values',
  valuesText: matrixToText(DEFAULT_A),
  url: '',
  expr: '',
  min: -1,
  max: 1,
  dropout: 0,
})

export const defaultRight = () => ({
  name: 'B',
  matmul: false,
  h: DEFAULT_B.length,
  w: DEFAULT_B[0].length,
  init: 'values',
  valuesText: matrixToText(DEFAULT_B),
  url: '',
  expr: '',
  min: -1,
  max: 1,
  dropout: 0,
})

export const defaultAnim = () => ({
  alg: 'inherit',
})

export const defaultBlock = () => ({
  'i blocks': 1,
  'k blocks': 1,
  'j blocks': 1,
})

export const defaultLayout = () => ({
  polarity: 'negative',
  'left placement': 'left',
  'right placement': 'top',
  'result placement': 'front',
  cellSize: 1,
})
