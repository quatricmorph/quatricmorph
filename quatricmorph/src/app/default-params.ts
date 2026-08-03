// @ts-nocheck
import * as viz from '../viz.js'

/** Initial visualization params (single-mm default). */
export function createDefaultParams() {
  return {
    expr: 'L @ R',
    name: 'L @ R',
    epilog: viz.default_epilog,
    left: viz.defaultLeft(),
    right: viz.defaultRight(),
    anim: {
      fuse: 'none',
      speed: 20,
      'hide inputs': false,
      alg: 'none',
      spin: 0,
    },
    block: viz.defaultBlock(),
    layout: {
      scheme: 'blocks',
      gap: 4,
      scatter: 0,
      molecule: 1,
      blast: 0,
      ...viz.defaultLayout(),
    },
    deco: {
      legends: 6,
      shape: true,
      spotlight: 2,
      'row guides': 0.6,
      'flow guides': 0.5,
      'lens size': 0.5,
      'magnification': 7,
      'interior spotlight': false,
      axes: false,
    },
    viz: {
      sensitivity: 'global',
      'min size': 0.2,
      'min light': 0.4,
      'max light': 0.7,
      'elem scale': 1.25,
      'zero hue': 0.77,
      'hue gap': 0.74,
      'hue spread': 0.04,
    },
    diag: {
      url: '',
    },
    cam: viz.defaultCam(),
  }
}
