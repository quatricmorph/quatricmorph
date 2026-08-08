"use strict"

//
// The default scene: a full attention head as four nested matmuls,
//
//   out = ((attn = (Q = input @ wQ) @ (K_t = wK_t @ input_t)) @ (V = input @ wV)) @ wO
//
// with every knob the GUI exposes set to the value the app opens on.
//
// This lived at the top of main.js, which made it unreachable: main.js builds a
// WebGPU renderer at import time, so nothing else -- including a test -- could
// read the defaults without a GPU. It is the reference shape for a params tree
// (every key gui.js and viz.js expect, at the depth they expect it), so it is
// worth being able to import on its own.
//
// Returned from a function, never exported as a shared object: params is
// mutated in place all over main.js and gui.js, so a shared literal would carry
// one session's edits into the next reset.
//
// The params tree is heterogeneous and mutated in place: nodes gain and lose
// keys at runtime (`sync_expr` arrives by postMessage and is deleted again;
// deleteProps strips a leaf's h/w when the GUI turns it into a matmul). No
// static type describes that without lying, so this alias is `any` on purpose.
// It names the concept and is the single place to tighten if the tree is ever
// given a fixed shape.
export type Params = any

export function defaultParams(): Params {
  return {
    expr: 'out = ((attn = (Q = input @ wQ) @ (K_t = wK_t @ input_t)) @ (V = input @ wV)) @ wO',
    name: 'out',
    epilog: 'x/sqrt(k)',
    left: {
      epilog: 'none',
      anim: {
        alg: 'inherit',
      },
      block: {
        'i blocks': 1,
        'k blocks': 1,
        'j blocks': 1,
      },
      layout: {
        polarity: 'positive',
        'left placement': 'left',
        'right placement': 'bottom',
        'result placement': 'back',
      },
      left: {
        epilog: 'none',
        anim: {
          alg: 'inherit',
        },
        block: {
          'i blocks': 1,
          'k blocks': 1,
          'j blocks': 1,
        },
        layout: {
          polarity: 'negative',
          'left placement': 'left',
          'right placement': 'top',
          'result placement': 'front',
        },
        left: {
          epilog: 'none',
          anim: {
            alg: 'inherit',
          },
          block: {
            'i blocks': 1,
            'k blocks': 1,
            'j blocks': 1,
          },
          layout: {
            polarity: 'positive',
            'left placement': 'left',
            'right placement': 'bottom',
            'result placement': 'back',
          },
          left: {
            name: 'input',
            matmul: false,
            h: 32,
            w: 32,
            init: 'row major',
            url: '',
            min: -1,
            max: 1,
            dropout: 0,
            expr: '',
          },
          right: {
            name: 'wQ',
            matmul: false,
            h: 32,
            w: 32,
            init: 'col major',
            url: '',
            min: -1,
            max: 1,
            dropout: 0,
            expr: '',
          },
          name: 'Q',
          matmul: true,
        },
        right: {
          epilog: 'none',
          anim: {
            alg: 'inherit',
          },
          block: {
            'i blocks': 1,
            'k blocks': 1,
            'j blocks': 1,
          },
          layout: {
            polarity: 'positive',
            'left placement': 'right',
            'right placement': 'top',
            'result placement': 'back',
          },
          left: {
            name: 'wK_t',
            matmul: false,
            h: 32,
            w: 32,
            init: 'row major',
            url: '',
            min: -1,
            max: 1,
            dropout: 0,
            expr: '',
          },
          right: {
            name: 'input_t',
            matmul: false,
            h: 32,
            w: 32,
            init: 'col major',
            url: '',
            min: -1,
            max: 1,
            dropout: 0,
            expr: '',
          },
          name: 'K_t',
          matmul: true,
        },
        name: 'attn',
        matmul: true,
      },
      right: {
        epilog: 'none',
        anim: {
          alg: 'inherit',
        },
        block: {
          'i blocks': 1,
          'k blocks': 1,
          'j blocks': 1,
        },
        layout: {
          polarity: 'negative',
          'left placement': 'right',
          'right placement': 'top',
          'result placement': 'back',
        },
        left: {
          name: 'input',
          matmul: false,
          h: 32,
          w: 32,
          init: 'row major',
          url: '',
          min: -1,
          max: 1,
          dropout: 0,
          expr: '',
        },
        right: {
          name: 'wV',
          matmul: false,
          h: 32,
          w: 32,
          init: 'col major',
          url: '',
          min: -1,
          max: 1,
          dropout: 0,
          expr: '',
        },
        name: 'V',
        matmul: true,
      },
      name: 'attn @ V',
      matmul: true,
    },
    right: {
      name: 'wO',
      matmul: false,
      h: 32,
      w: 32,
      init: 'col major',
      url: '',
      min: -1,
      max: 1,
      dropout: 0,
      expr: '',
    },
    anim: {
      fuse: 'none',
      speed: 97,
      'hide inputs': true,
      alg: 'dotprod (col major)',
      spin: 0,
      folder: 'open',
    },
    block: {
      'i blocks': 1,
      'k blocks': 1,
      'j blocks': 1,
    },
    layout: {
      scheme: 'blocks',
      gap: 4,
      scatter: 0,
      molecule: 1,
      blast: 0,
      polarity: 'negative',
      'left placement': 'left',
      'right placement': 'top',
      'result placement': 'front',
      folder: 'closed',
    },
    deco: {
      legends: 6,
      shape: true,
      spotlight: 2,
      'row guides': 0.6,
      'flow guides': 0.5,
      'lens size': 0.5,
      magnification: 7,
      'interior spotlight': false,
      axes: false,
      folder: 'closed',
    },
    viz: {
      sensitivity: 'global',
      'min size': 0.8,
      'min light': 0.5,
      'max light': 0.8,
      'elem scale': 1.5,
      'zero hue': 0.356,
      'hue gap': 0.7,
      'hue spread': 0.04,
      folder: 'open',
    },
    diag: {
      url: '',
      folder: 'open',
    },
    cam: {
      x: 23.8601394508366,
      y: 13.55251706132545,
      z: 109.01416137057434,
      target: {
        x: 37.171002653209314,
        y: 3.7070451463575647,
        z: 38.467207145211944,
      },
    },
    folder: 'open',
    compress: true,
  }
}
