// @ts-nocheck
import * as THREE from 'three'
import * as util from '../util.js'
import { Array2D } from './array2d.js'
import { Mat } from './mat.js'
import { getInitFunc } from './init.js'
import { applyInPlaceEpilog_, POINTWISE } from './epilog.js'
import { setElemScale, grid } from './sizing.js'
import { ensureChildCounts } from './constants.js'

export class MatMul {

  constructor(params, context, init_viz = true) {
    this.context = context

    this.params = util.copyTree(params)
    ensureChildCounts(this.params)

    this.group = new THREE.Group()
    this.group.name = `${this.params.name}.group`

    const height = p => p.matmul ? height(p.left) : p.h
    const width = p => p.matmul ? width(p.right) : p.w

    this.H = height(params.left)
    this.D = width(params.left)
    this.W = width(params.right)

    if (this.D != height(params.right)) {
      console.log(`HEY left width ${this.D} != right height ${height(params.right)}`)
    }

    this.initLeft()
    this.initRight()
    this.initResult()

    if (init_viz) {
      this.initViz()
    }
  }

  getDispH() {
    const { i: { n, size } } = this.getBlockInfo()
    return this.H + this.params.layout.gap * (Math.min(n, Math.ceil(this.H / size)) - 1)
  }

  getDispD() {
    const { k: { n, size } } = this.getBlockInfo()
    return this.D + this.params.layout.gap * (Math.min(n, Math.ceil(this.D / size)) - 1)
  }

  getDispW() {
    const { j: { n, size } } = this.getBlockInfo()
    return this.W + this.params.layout.gap * (Math.min(n, Math.ceil(this.W / size)) - 1)
  }

  disposeAll() {
    util.disposeAndClear(this.group)
  }

  prepChildParams(base = undefined) {
    base ||= util.copyTree(this.params)
    return {
      ...base,
      ...(base != this.params ? {
        anim: { ...this.params.anim, ...base.anim || {} },
        block: { ...this.params.block, ...base.block || {} },
        deco: { ...this.params.deco, ...base.deco || {} },
        layout: { ...this.params.layout, ...base.layout || {} },
        viz: { ...this.params.viz, ...base.viz || {} },
      } : {}),
      getGlobalAbsmax: this.getGlobalAbsmax.bind(this),
    }
  }

  initLeft() {
    const left_params = this.prepChildParams(this.params.left)
    left_params.is_child = 'left'
    left_params.block['i blocks'] = this.params.block['i blocks']
    left_params.block['j blocks'] = this.params.block['k blocks']
    if (left_params.matmul) {
      this.left = new MatMul(left_params, this.context, false)
    } else {
      const { right, result, polarity } = this.getPlacementInfo()
      left_params.deco.shape_info = {
        h: { name: 'I', place: result == polarity },
        w: { name: 'K', place: right },
      }
      const data = Array2D.fromInit(this.H, this.D, getInitFunc(left_params))
      this.left = new Mat(data, left_params, this.context, false)
    }
  }

  initRight() {
    const right_params = this.prepChildParams(this.params.right)
    right_params.is_child = 'right'
    right_params.block['i blocks'] = this.params.block['k blocks']
    right_params.block['j blocks'] = this.params.block['j blocks']
    if (right_params.matmul) {
      this.right = new MatMul(right_params, this.context, false)
    } else {
      const { left, result, polarity } = this.getPlacementInfo()
      right_params.deco.shape_info = {
        h: { name: 'K', place: left },
        w: { name: 'J', place: result == polarity },
      }
      const data = Array2D.fromInit(this.D, this.W, getInitFunc(right_params))
      this.right = new Mat(data, right_params, this.context, false)
    }
  }

  initResult() {
    const result_init = (i, j) => this.dotprod(i, j, 0, this.D)
    const data = Array2D.fromInit(this.H, this.W, result_init, this.params.epilog)
    const result_params = this.prepChildParams()
    // if (this.params.total == this.params.count) {
    if (!this.params.is_child) {
      const placement = this.getPlacementInfo()
      result_params.deco.shape_info = {
        h: { name: 'I', place: placement.left },
        w: { name: 'J', place: placement.right },
      }
    }
    result_params.block['i blocks'] = result_params.block['i blocks']
    result_params.block['j blocks'] = result_params.block['j blocks']
    this.result = new Mat(data, result_params, this.context, false)
  }

  // TODO clean up the way epilogs are done.
  // currently we run a pointwise epi if we find one,
  // or we do preprocessing needed by the in-place epi
  // (which is run later) based on snooping the expression
  applyPointwiseEpilog(x) {
    const epi = this.params.epilog
    const pw = POINTWISE[epi]
    if (pw) {
      return pw(x)
    } else if (epi == 'x/k') {
      return x / this.D
    } else if (epi.includes('x/sqrt(k)')) {
      return x / Math.sqrt(this.D)
    } else if (epi.includes('x/8')) {
      return x / 8
    } else {
      return x
    }
  }

  dotprod(i, k, minj, maxj) {
    const lw = this.left.W
    const ld = this.left.getDataArray()
    const rw = this.right.W
    const rd = this.right.getDataArray()
    const maxlx = i * lw + maxj

    let x = 0.0
    for (let lx = i * lw + minj, rx = minj * rw + k; lx < maxlx; lx++, rx += rw) {
      x += ld[lx] * rd[rx]
    }

    if (isNaN(x)) {
      console.log(`HEY dotprod(${i}, ${k}, ${minj}, ${maxj}) is NaN`)
      return 0
    }

    return this.applyPointwiseEpilog(x, this.params.epilog)
  }

  getDataArray() {
    return this.result.getDataArray()
  }

  getData(i, j) {
    return this.result.getData(i, j)
  }

  show(r = undefined, c = undefined) {
    this.left.show(r, c)
    this.right.show(r, c)
    this.result.show(r, c)
  }

  hide(r = undefined, c = undefined) {
    this.left.hide(r, c)
    this.right.hide(r, c)
    this.result.hide(r, c)
  }

  setColorsAndSizes(r = undefined, c = undefined, size = undefined, color = undefined) {
    this.result.setColorsAndSizes(r, c, size, color)
  }

  bumpColor(r = undefined, c = undefined) {
    this.result.bumpColor(r, c)
  }

  ikjmul(i, k, j) {
    return this.left.getData(i, k) * this.right.getData(k, j)
  }

  getExtent() {
    const gap = this.params.layout.gap
    return this._extents || (this._extents = {
      x: this.getDispW() + 2 * gap - 1,
      y: this.getDispH() + 2 * gap - 1,
      z: this.getDispD() + 2 * gap - 1,
    })
  }

  initViz(params = undefined) {
    if (params) {
      this.params = params
    }

    util.disposeAndClear(this.group)
    this.flow_guide_group = undefined
    this.anim_mats = []

    if (this.left.params.anim.alg == 'inherit') {
      this.left.params.anim.alg = this.params.anim.alg
    }
    if (this.right.params.anim.alg == 'inherit') {
      this.right.params.anim.alg = this.params.anim.alg
    }

    setElemScale(this.params.viz['elem scale'])
    this.initResultViz()
    this.initLeftViz()
    this.initRightViz()

    this.setFlowGuide()
    this.setRowGuides()
  }

  initLeftViz() {
    this.left.initViz()
    if (this.params.layout.polarity.startsWith('positive')) {
      this.left.group.rotation.y = -Math.PI / 2
      this.left.group.position.x = this.params.layout['left placement'].startsWith('left') ?
        -this.getLeftScatter() :
        this.getExtent().x + this.left.getExtent().z + this.getLeftScatter()
    } else { // negative
      this.left.group.rotation.y = Math.PI / 2
      this.left.group.position.z = this.getExtent().z
      this.left.group.position.x = this.params.layout['left placement'].startsWith('left') ?
        -(this.left.getExtent().z + this.getLeftScatter()) :
        this.getExtent().x + this.getLeftScatter()
    }
    this.group.add(this.left.group)
  }

  initRightViz() {
    this.right.initViz()
    if (this.params.layout.polarity.startsWith('positive')) {
      this.right.group.rotation.x = Math.PI / 2
      this.right.group.position.y = this.params.layout['right placement'].startsWith('top') ?
        -this.getRightScatter() :
        this.getExtent().y + this.right.getExtent().z + this.getRightScatter()
    } else { // negative
      this.right.group.rotation.x = -Math.PI / 2
      this.right.group.position.z = this.getExtent().z
      this.right.group.position.y =
        this.params.layout['right placement'].startsWith('top') ?
          -(this.right.getExtent().z + this.getRightScatter()) :
          this.getExtent().y + this.getRightScatter()
    }
    this.group.add(this.right.group)
  }

  initResultViz() {
    this.result.initViz()
    this.result.group.position.z =
      this.params.layout['result placement'].startsWith('back') ?
        this.getExtent().z :
        0
    this.group.add(this.result.group)
  }

  getPlacementInfo() {
    return {
      polarity: this.params.layout.polarity.startsWith('positive'),
      left: this.params.layout['left placement'].startsWith('left'),
      right: this.params.layout['right placement'].startsWith('top'),
      result: this.params.layout['result placement'].startsWith('front'),
    }
  }

  getLayoutInfo() {
    const info = this.getPlacementInfo()
    Object.entries(info).forEach(([k, v]) => info[k] = v ? 1 : -1)
    info.gap = this.params.layout.gap
    info.left_scatter = this.getLeftScatter()
    info.right_scatter = this.getRightScatter()
    return info
  }

  setFlowGuide(light = undefined) {
    if (light != this.params.deco['flow guides']) {
      light = util.syncProp(this.params.deco, 'flow guides', light)
      if (this.flow_guide_group) {
        this.group.remove(this.flow_guide_group)
        util.disposeAndClear(this.flow_guide_group)
        this.flow_guide_group = undefined
      }
      if (light > 0.0) {
        this.flow_guide_group = util.flowGuide(
          this.getDispH(), this.getDispD(), this.getDispW(), this.getLayoutInfo(), light
        )
        this.group.add(this.flow_guide_group)
      }
    }
    this.left.setFlowGuide(light)
    this.right.setFlowGuide(light)
  }

  scatterFromCount(count) {
    const { scatter, molecule, blast } = this.params.layout
    const mult = count < molecule ? 0 :
      blast >= 0 ? count ** blast :
        (this.params.total - count) ** -blast
    return scatter * mult
  }

  getLeftScatter() {
    return this.scatterFromCount(this.left.params.count)
  }

  getRightScatter() {
    return this.scatterFromCount(this.right.params.count)
  }

  updateLabels(params = undefined) {
    if (params) {
      this.params.deco.spotlight = params.deco.spotlight
      this.params.deco['interior spotlight'] = params.deco['interior spotlight']
    }

    const spotlight = this.params.deco.spotlight
    this.left.updateLabels(this.left.params.matmul ? params : spotlight)
    this.right.updateLabels(this.right.params.matmul ? params : spotlight)
    this.result.updateLabels(spotlight)

    const interior_spotlight = this.params.deco['interior spotlight'] ? spotlight : 0
    this.anim_mats.map(m => m.updateLabels(interior_spotlight))
  }

  getBoundingBox() {
    const get_bb = mm => {
      const min = mm.group.localToWorld(new THREE.Vector3())
      const max = mm.group.localToWorld(new THREE.Vector3().copy(mm.getExtent()))
      const swap = d => { const temp = min[d]; min[d] = max[d]; max[d] = temp }
      ['x', 'y', 'z'].forEach(d => { if (min[d] > max[d]) swap(d) })
      let bb = new THREE.Box3(min, max)
      mm.params.left.matmul && bb.union(get_bb(mm.left))
      mm.params.right.matmul && bb.union(get_bb(mm.right))
      return bb
    }
    return get_bb(this)
  }

  center() {
    const c = this.getBoundingBox().getCenter(new THREE.Vector3())
    util.updateProps(this.group.position, c.negate())
  }

  getAbsmax() {
    return Math.max(this.left.getAbsmax(), this.right.getAbsmax(), this.result.getAbsmax())
  }

  getGlobalAbsmax() {
    return this.params.getGlobalAbsmax ? this.params.getGlobalAbsmax() : this.getAbsmax()
  }

  hideInputs(hide) {
    util.syncProp(this.params.anim, 'hide inputs', hide)
    if (this.params.left.matmul) {
      this.left.hideInputs(hide)
    } else if (this.params.anim.alg != 'none') {
      hide ? this.left.hide() : this.left.show()
    }
    if (this.params.right.matmul) {
      this.right.hideInputs(hide)
    } else if (this.params.anim.alg != 'none') {
      hide ? this.right.hide() : this.right.show()
    }
  }

  setRowGuides(light) {
    light = util.syncProp(this.params.deco, 'row guides', light)
    this.left.setRowGuides(light)
    this.right.setRowGuides(light)
    this.result.setRowGuides(light)
    this.anim_mats.forEach(m => m.setRowGuides(light))
  }

  setName(name) {
    name = util.syncProp(this.params, 'name', name)
    this.result.setName(name)
  }

  setLegends(name = undefined, shape = undefined) {
    name = util.syncProp(this.params.deco, 'legends', name)
    shape = util.syncProp(this.params.deco, 'shape', shape)
    this.left.setLegends(name, shape)
    this.right.setLegends(name, shape)
    this.result.setLegends(name, shape)
  }

  // animation

  initAnimation(cb = undefined) {
    if (this.params.anim.alg == 'none') {
      if (this.params.anim['hide inputs']) {
        !this.params.left.matmul && this.left.show()
        !this.params.right.matmul && this.right.show()
      }
      return
    }

    const bumps = {
      'dotprod (row major)': () => this.getVmprodBump(true),
      'dotprod (col major)': () => this.getMvprodBump(true),
      'axpy': () => this.getVvprodBump(true),
      'mvprod': () => this.getMvprodBump(false),
      'vmprod': () => this.getVmprodBump(false),
      'vvprod': () => this.getVvprodBump(false),
    }

    const nj = this.getBlockInfo().j.n
    const nlk = () => this.left.getBlockInfo().k.n
    const nri = () => this.right.getBlockInfo().i.n

    const { alg, fuse } = this.params.anim

    let left_done = true, right_done = true

    this.alg_join = () => {
      const lalg = !this.params.left.matmul || left_done ? 'none' :
        (fuse == 'async' || this.left.getIndex() == this.getIndex() ?
          this.left.alg_join() :
          'mixed')

      const ralg = !this.params.right.matmul || right_done ? 'none' :
        (fuse == 'async' || this.right.getIndex() == this.getIndex() ?
          this.right.alg_join() :
          'mixed')

      const or_none = (a, b) => a == b || a == 'none'

      return (alg == 'vmprod' && or_none(lalg, 'vmprod') && ralg == 'none') ? 'vmprod' :
        (alg == 'mvprod' && lalg == 'none' && or_none(ralg, 'mvprod')) ? 'mvprod' :
          (alg == 'vvprod' && or_none(lalg, 'mvprod') && or_none(ralg, 'vmprod')) ? 'vvprod' :
            (lalg == 'none' && ralg == 'none') ? alg :
              'mixed'
    }

    const can_fuse = () => fuse != 'none' && this.alg_join() != 'mixed'

    const start = () => {
      const result_bump = bumps[alg]()

      this.bump = () => {
        const go = left_done && right_done || can_fuse()
        left_done || this.left.bump()
        right_done || this.right.bump()
        go && result_bump()
      }

      if (this.params.left.matmul && this.params.left.anim.alg != 'none') {
        left_done = false
        this.left.initAnimation(() => left_done = true)
      }

      if (this.params.right.matmul && this.params.right.anim.alg != 'none') {
        right_done = false
        this.right.initAnimation(() => right_done = true)
      }

      if (this.params.anim['hide inputs']) {
        this.left.hide()
        this.right.hide()
      }
      this.result.hide()

      !cb && this.bump()
    }

    this.onAnimDone = () => {
      this.clearAnimMats()
      nj > 1 && this.result.show()
      cb ? cb() : start()
    }

    start()
  }

  getBlockInfo() {
    const ni = Math.min(this.params.block['i blocks'], this.H)
    const nk = Math.min(this.params.block['k blocks'], this.D)
    const nj = Math.min(this.params.block['j blocks'], this.W)
    return {
      i: { n: ni, size: Math.ceil(this.H / ni), max: this.H },
      k: { n: nk, size: Math.ceil(this.D / nk), max: this.D },
      j: { n: nj, size: Math.ceil(this.W / nj), max: this.W },
    }
  }

  grid(dims, f) {
    grid(this.getBlockInfo(), dims, f)
  }

  getAnimIntermediateParams(name) {
    const params = this.prepChildParams()
    // params.name = name // debug
    delete params.name
    params.viz.sensitivity == 'superlocal' && (params.viz.sensitivity = 'local')
    params.block['i blocks'] = 1
    params.block['k blocks'] = 1
    params.block['j blocks'] = 1
    return params
  }

  getAnimResultParams() {
    const params = this.prepChildParams()
    // params.name = name // debug
    delete params.name
    params.viz.sensitivity == 'superlocal' && (params.viz.sensitivity = 'local')
    params.block['i blocks'] = params.block['i blocks']
    params.block['k blocks'] = params.block['j blocks']
    return params
  }

  clearAnimMats() {
    this.anim_mats.forEach(m => {
      this.group.remove(m.group)
      util.disposeAndClear(m.group)
    })
    this.anim_mats = []
  }

  getAnimResultMats() {
    const { k: { n: nk, size: sk } } = this.getBlockInfo()
    if (nk == 1) {
      return [this.result]
    }
    const { gap, polarity, result } = this.getLayoutInfo()
    const { z: extz } = this.getExtent()
    const results = []
    this.grid('k', ({ start: k, end: ke, index: ki }) => {
      const result_init = (i, j) => this.dotprod(i, j, k, ke)
      const data = Array2D.fromInit(this.H, this.W, result_init)
      const mat = new Mat(data, this.getAnimResultParams(), this.context, true)
      mat.group.position.z = polarity > 0 ?
        result > 0 ?
          ki == 0 ?
            this.result.group.position.z :
            gap + k + Math.floor(gap * k / sk - gap / 4) :
          ki == nk - 1 ?
            this.result.group.position.z :
            gap + ke + Math.floor(gap * k / sk + (gap - 1) / 4) :
        result > 0 ?
          ki == nk - 1 ?
            this.result.group.position.z :
            extz - ke - Math.floor(gap * ke / sk + (gap - 1) / 4) :
          ki == 0 ?
            this.result.group.position.z :
            extz - k - Math.floor(gap * ke / sk - gap / 4)
      mat.setRowGuides()
      mat.hide()
      results.push(mat)
      this.group.add(mat.group)
      this.anim_mats.push(mat)
    })
    return results
  }

  getVmprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const results = this.getAnimResultMats()

    const vmps = {}
    this.grid('ikj', (
      { start: i, index: ii },
      { start: k, extent: kx, index: ki },
      { start: j, extent: jx, index: ji }
    ) => {
      const vmpinit = (kii, jii) => this.ikjmul(i, k + kii, j + jii)
      const data = Array2D.fromInit(kx, sweep ? 1 : jx, vmpinit)
      const vmp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.vmp[${ii}, ${ki}, ${ji}]`), this.context, true)
      vmp.hide()
      const z = polarity < 0 ? this.getExtent().z - k - (gap * ki) : k + (gap * ki)
      util.updateProps(vmp.group.position, { x: j + ji, y: gap + i + ii, z })
      vmp.group.rotation.x = polarity * Math.PI / 2
      vmps[[i, k, j]] = vmp
      this.anim_mats.push(vmp)
      this.group.add(vmp.group)
    })

    const { i: { size: isize }, j: { size: jsize } } = this.getBlockInfo()
    let curi = -1
    let curj = sweep ? -1 : 0

    this.getIndex = () => curi

    return () => {
      // update indexes
      const [oldi, oldj] = [curi, curj]
      sweep && (curj = (curj + 1) % jsize)
      curj == 0 && curi++

      // clear old input hilights
      if (oldi >= 0 && !this.params.anim['hide inputs']) {
        sweep && this.grid('j', ({ start: j, extent: jx }) => {
          oldj < jx && this.right.setColorsAndSizes(undefined, j + oldj)
        })
        oldi != curi && this.grid('i', ({ start: i, extent: ix }) => {
          oldi < ix && this.left.setColorsAndSizes(i + oldi, undefined)
        })
      }

      // end of cycle
      if (curi == isize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curi == 0 && curj == 0) {
        Object.values(vmps).forEach(vmp => vmp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input hilights
      if (!this.params.anim['hide inputs']) {
        sweep && this.grid('j', ({ start: j, extent: jx }) => {
          curj < jx && this.right.bumpColor(undefined, j + curj)
        })
        oldi != curi && this.grid('i', ({ start: i, extent: ix }) => {
          curi < ix && this.left.bumpColor(i + curi, undefined)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i, extent: ix, index: ii },
        { start: k },
        { start: j, extent: jx, index: ji }
      ) => {
        const vmp = vmps[[i, k, j]]
        if (curi < ix && curj < jx) {
          util.updateProps(vmp.group.position, { x: j + (ji * gap) + curj, y: gap + i + (ii * gap) + curi })
          vmp.reinit((ki, ji) => this.ikjmul(i + curi, k + ki, j + curj + ji))
        }
      })

      // reveal new results
      this.grid('ij', ({ start: i, extent: ix }, { start: j, end: je, extent: jx }) => {
        curi < ix && curj < jx && results.forEach(r => r.show(i + curi, sweep ? j + curj : [j, je]))
      })

      // update labels
      this.updateLabels()
    }
  }

  getMvprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const results = this.getAnimResultMats()

    const mvps = {}
    this.grid('ikj', (
      { start: i, extent: ix, index: ii },
      { start: k, extent: kx, index: ki },
      { start: j, index: ji },
    ) => {
      const mvpinit = (iii, kii) => this.ikjmul(i + iii, k + kii, j)
      const data = Array2D.fromInit(sweep ? 1 : ix, kx, mvpinit)
      const mvp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.mvp[${ii}, ${ki}, ${ji}]`), this.context, true)
      mvp.hide()
      const z = polarity < 0 ? this.getExtent().z - k - (gap * ki) : k + (gap * ki)
      util.updateProps(mvp.group.position, { x: gap + j + ji, y: i + ii, z })
      mvp.group.rotation.y = polarity * -Math.PI / 2
      mvps[[i, k, j]] = mvp
      this.anim_mats.push(mvp)
      this.group.add(mvp.group)
    })

    const { i: { size: isize }, j: { size: jsize } } = this.getBlockInfo()
    let curj = -1
    let curi = sweep ? -1 : 0

    this.getIndex = () => curj

    return () => {
      // update indexes
      const [oldi, oldj] = [curi, curj]
      sweep && (curi = (curi + 1) % isize)
      curi == 0 && curj++

      // clear old input hilights
      if (oldj >= 0 && !this.params.anim['hide inputs']) {
        sweep && this.grid('i', ({ start: i, extent: ix }) => {
          oldi < ix && this.left.setColorsAndSizes(i + oldi, undefined)
        })
        oldj != curj && this.grid('j', ({ start: j, extent: jx }) => {
          oldj < jx && this.right.setColorsAndSizes(undefined, j + oldj)
        })
      }

      // end of cycle
      if (curj == jsize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curj == 0 && curi == 0) {
        Object.values(mvps).forEach(vmp => vmp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input hilights
      if (!this.params.anim['hide inputs']) {
        sweep && this.grid('i', ({ start: i, extent: ix }) => {
          curi < ix && this.left.bumpColor(i + curi, undefined)
        })
        oldj != curj && this.grid('j', ({ start: j, extent: jx }) => {
          curj < jx && this.right.bumpColor(undefined, j + curj)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i, extent: ix, index: ii },
        { start: k },
        { start: j, extent: jx, index: ji }
      ) => {
        const mvp = mvps[[i, k, j]]
        if (curi < ix && curj < jx) {
          util.updateProps(mvp.group.position, { x: gap + j + (ji * gap) + curj, y: i + (ii * gap) + curi })
          mvp.reinit((ii, ki) => this.ikjmul(i + curi + ii, k + ki, j + curj))
        }
      })

      // reveal new results
      this.grid('ij', ({ start: i, end: ie, extent: ix }, { start: j, extent: jx }) => {
        curi < ix && curj < jx && results.forEach(r => r.show(sweep ? i + curi : [i, ie], j + curj))
      })

      // update labels
      this.updateLabels()
    }
  }

  getVvprodBump(sweep) {
    const { gap, polarity } = this.getLayoutInfo()
    const { z: extz } = this.getExtent()
    // no intermediate result planes for vvprod, too cluttered. just sum it into final result
    const results = [this.result]

    // pre-epilog shadow for result accum
    const pre_epilog = Array2D.fromInit(this.H, this.W, () => 0)

    const vvps = {}
    this.grid('ikj', (
      { start: i, extent: ix, index: ii },
      { start: k, index: ki },
      { start: j, extent: jx, index: ji }
    ) => {
      const vvpinit = (iii, jii) => this.ikjmul(i + iii, k, j + jii)
      const data = Array2D.fromInit(ix, sweep ? 1 : jx, vvpinit)
      const vvp = new Mat(data, this.getAnimIntermediateParams(this.params.name + `.vvp[${ii}, ${ki}, ${ji}]`), this.context, true)
      vvp.hide()
      const z = polarity > 0 ? gap + k + ki : extz - gap - k - ki
      util.updateProps(vvp.group.position, { x: j + ji * gap, y: i + ii * gap, z })
      vvps[[i, k, j]] = vvp
      this.anim_mats.push(vvp)
      this.group.add(vvp.group)
    })

    const { k: { size: ksize }, j: { size: jsize } } = this.getBlockInfo()
    let curk = -1
    let curj = sweep ? -1 : 0

    this.getIndex = () => curk

    return () => {
      // update indexes
      const [oldk, oldj] = [curk, curj]
      curk++
      if (sweep && curk % ksize == 0) {
        curk = 0
        curj++
      }

      // clear old input highlights
      if (oldk >= 0 && !this.params.anim['hide inputs']) {
        sweep ?
          this.grid('kj', ({ start: k, extent: kx }, { start: j, extent: jx }) => {
            oldk < kx && oldj < jx && this.right.setColorsAndSizes(k + oldk, j + oldj)
          }) :
          this.grid('k', ({ start: k, extent: kx }) => {
            oldk < kx && this.right.setColorsAndSizes(k + oldk, undefined)
          })
        this.grid('k', ({ start: k, extent: kx }) => {
          oldk < kx && this.left.setColorsAndSizes(undefined, k + oldk)
        })
      }

      // end of cycle
      if (sweep ? curj == jsize : curk == ksize) {
        this.onAnimDone()
        return
      }

      // start of cycle
      if (curj == 0 && curk == 0) {
        Object.values(vvps).forEach(vvp => vvp.setRowGuides())
        results.forEach(r => r.hide())
      }

      // new input highlights
      if (!this.params.anim['hide inputs']) {
        sweep ?
          this.grid('kj', ({ start: k, extent: kx }, { start: j, extent: jx }) => {
            curk < kx && curj < jx && this.right.bumpColor(k + curk, j + curj)
          }) :
          this.grid('k', ({ start: k, extent: kx }) => {
            curk < kx && this.right.bumpColor(k + curk, undefined)
          })
        this.grid('k', ({ start: k, extent: kx }) => {
          curk < kx && this.left.bumpColor(undefined, k + curk)
        })
      }

      // update intermediates
      this.grid('ikj', (
        { start: i },
        { start: k, extent: kx, index: ki },
        { start: j, extent: jx, index: ji }
      ) => {
        const vvp = vvps[[i, k, j]]
        if (curk < kx && curj < jx) {
          const z = polarity > 0 ? gap + k + (ki * gap) + curk : extz - gap - k - (ki * gap) - curk
          util.updateProps(vvp.group.position, { x: j + ji * gap + curj, z })
          vvp.reinit((iii, jii) => this.ikjmul(i + iii, k + curk, j + curj + jii))
        }
      })

      // no intermediate result planes for vvprod, too cluttered. just sum it into final result
      // also we go thru some gymnastics to do epilog during sweep
      this.grid('kj', ({ start: k, extent: kx, index: ki }, { start: j, end: je, extent: jx }) => {
        if (curk < kx && curj < jx) {
          const running_dp = (ii, ji) => {
            const x = this.left.getData(ii, k + curk) * this.right.getData(k + curk, ji)
            return (ki == 0 && curk == 0) ? x : (pre_epilog.get(ii, ji) + x)
          }
          pre_epilog.reinit(running_dp, undefined, undefined, sweep ? j + curj : [j, je])

          const pw_epilog_dp = (ii, ji) => this.applyPointwiseEpilog(pre_epilog.get(ii, ji))
          results[0].reinit(pw_epilog_dp, undefined, undefined, sweep ? j + curj : [j, je])
        }
      })
      applyInPlaceEpilog_(results[0].data.data, results[0].H, results[0].W, this.params.epilog)
      if (sweep) {
        this.grid('kj', ({ extent: kx }, { start: j, end: je, extent: jx }) => {
          if (curk < kx && curj < jx) {
            results[0].reinit(() => 0, undefined, undefined, [j + curj + 1, je])
          }
        })
      }
      results[0].setColorsAndSizes()

      // update labels
      this.updateLabels()
    }
  }
}

//
// layout schemes
//

