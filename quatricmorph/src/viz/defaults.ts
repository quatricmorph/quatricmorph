// @ts-nocheck
export const default_dims = { i: 32, j: 32, k: 32 }

export const defaultCam = () => ({
  x: -default_dims.i * 1.5,
  y: default_dims.j * 1.5,
  z: default_dims.k * 1.5,
})

export const default_epilog = 'none'

export const defaultLeft = () => ({
  name: 'L',
  matmul: false,
  h: default_dims.i,
  w: default_dims.j,
  init: 'row major',
  url: '',
  min: -1,
  max: 1,
  dropout: 0,
})

export const defaultRight = () => ({
  name: 'R',
  matmul: false,
  h: default_dims.j,
  w: default_dims.k,
  init: 'col major',
  url: '',
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
})

// adjust tree to match a param node's i/k/j blocks
