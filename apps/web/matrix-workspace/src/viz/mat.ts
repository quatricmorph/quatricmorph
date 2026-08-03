// @ts-nocheck
import * as THREE from 'three'
import * as util from '../util.js'
import { emptyPoints, ZERO_COLOR, COLOR_TEMP, elem_size, grid } from './sizing.js'
import { toRange } from './array2d.js'

export class Mat {

  constructor(data, params, context, init_viz) {
    this.params = params
    this.context = context

    this.data = data
    this.H = data.h
    this.W = data.w
    this.absmax = this.data.absmax()
    this.absmin = this.data.absmin()

    if (init_viz) {
      this.initViz()
    }
  }

  getBlockInfo() {
    const ni = Math.min(this.params.block['i blocks'], this.H)
    const nj = Math.min(this.params.block['j blocks'], this.W)
    return {
      i: { n: ni, size: Math.ceil(this.H / ni), max: this.H },
      j: { n: nj, size: Math.ceil(this.W / nj), max: this.W },
    }
  }

  grid(dims, f) {
    grid(this.getBlockInfo(), dims, f)
  }

  getDispH() {
    const { i: { n, size } } = this.getBlockInfo()
    return this.H + this.params.layout.gap * (Math.min(n, Math.ceil(this.H / size)) - 1)
  }

  getDispW() {
    const { j: { n, size } } = this.getBlockInfo()
    return this.W + this.params.layout.gap * (Math.min(n, Math.ceil(this.W / size)) - 1)
  }

  initViz() {
    const gap = this.params.layout.gap
    const info = { ...this.getBlockInfo(), gap }

    this.points = emptyPoints(this.H, this.W, info)
    this.points.name = `${this.params.name}.points`

    this.setColorsAndSizes()

    this.inner_group = new THREE.Group()
    this.inner_group.name = `${this.params.name}.inner_group`
    this.inner_group.add(this.points)

    util.updateProps(this.inner_group.position, { x: gap, y: gap })

    this.group = new THREE.Group()
    this.group.name = `${this.params.name}.group`
    this.group.add(this.inner_group)

    this.setLegends()
  }

  setColorsAndSizes(r = undefined, c = undefined, get_size = undefined, get_color = undefined) {
    const [rstart, rend] = toRange(r, this.H)
    const [cstart, cend] = toRange(c, this.W)
    get_size = get_size || this.sizeFromData.bind(this)
    get_color = get_color || this.colorFromData.bind(this)
    for (let i = rstart; i < rend; i++) {
      for (let j = cstart; j < cend; j++) {
        const x = this.getData(i, j)
        this.setSize(i, j, get_size(x))
        this.setColor(i, j, get_color(x))
        this.checkLabel(i, j, x)
      }
    }
  }

  getExtent() {
    const gap = this.params.layout.gap
    return this._extents || (this._extents = {
      x: this.getDispW() + 2 * gap - 1,
      y: this.getDispH() + 2 * gap - 1,
      z: 0,
    })
  }

  getRangeInfo() {
    const viz = this.params.viz
    const use_absmin = viz.sensitivity == 'superlocal'

    const local_absmax = this.absmax
    const global_absmax = this.getGlobalAbsmax()
    const absmax = (use_absmin || viz.sensitivity == 'local') ? local_absmax :
      viz.sensitivity == 'global' ? global_absmax :
        Math.sqrt(local_absmax * global_absmax) // semilocal
    const absmin = use_absmin ? this.absmin : 0
    const absdiff = absmax - absmin
    if (absmin > absmax) {
      console.log(`HEY absmin ${absmin} > absmax ${absmax}`)
    }
    return { viz, absmin, absmax, absdiff }
  }

  sizeFromData(x) {
    if (x === undefined || isNaN(x)) {
      console.log(`HEY sizeFromData(${x})`)
      return 0
    }

    if (x === 0) {
      return 0
    }

    const absx = Math.abs(x)
    if (absx === Infinity) {
      return elem_size
    }

    const { viz, absmin, absmax, absdiff } = this.getRangeInfo()
    const vol = absmax <= absmin ? 0 : (absx - absmin) / absdiff
    const zsize = viz['min size'] * elem_size
    const size = zsize + (elem_size - zsize) * Math.sqrt(vol)

    if (isNaN(size)) {
      this.n_size_from_data_errors = (this.n_size_from_data_errors || 0) + 1
      if (this.n_size_from_data_errors <= 100) {
        console.log(`HEY x ${x} size ${size} absx ${absx} absmax ${absmax} absmin ${absmin} zsize ${zsize}`)
        if (this.n_size_from_data_errors == 100) {
          console.log(`HEY stopping logging after 100 errors`)
        }
      }
    }

    // boundary violations can happen in intermediates
    return Math.min(size, elem_size)
  }

  colorFromData(x) {
    if (x === undefined || isNaN(x)) {
      console.log(`HEY colorFromData(${x})`)
      return COLOR_TEMP.setHSL(0.0, 1.0, 1.0)
    }

    if (x === 0) {
      return COLOR_TEMP.setHSL(0.0, 1.0, 0.0)
    }

    const { viz, absmin, absmax, absdiff } = this.getRangeInfo()

    // boundary violations can happen in intermediates
    const absx = Math.min(absmax, Math.max(absmin, Math.abs(x)))

    if (absx === Infinity) {
      return COLOR_TEMP.setHSL(1.0, 1.0, 1.0)
    }

    const hue_vol = absdiff <= 0 ? 0 : (x - Math.sign(x) * absmin) / absdiff
    const gap = viz['hue gap'] * Math.sign(x)
    const hue = (viz['zero hue'] + gap + (hue_vol * viz['hue spread'])) % 1

    const min_light = Math.max(viz['min light'], 0.00001)
    const max_light = Math.max(viz['max light'], min_light)
    const range = max_light - min_light
    const light_vol = absdiff <= 0 ? 0 : (absx - absmin)
    const light = min_light + range * Math.sqrt(light_vol) / Math.sqrt(absdiff)

    return COLOR_TEMP.setHSL(hue, 1.0, light)
  }

  getAbsmax() {
    return this.absmax
  }

  getGlobalAbsmax() {
    return this.params.getGlobalAbsmax ? this.params.getGlobalAbsmax() : this.absmax
  }

  reinit(init, epi = undefined, r = undefined, c = undefined) {
    this.data.reinit(init, epi, r, c)
    this.setColorsAndSizes(r, c)
  }

  getDataArray() {
    return this.data.data
  }

  getData(i, j) {
    if (i >= this.H || j >= this.W) {
      console.log(`HEY i ${i} >= this.H ${this.H} || j ${j} >= this.W ${this.W}`)
      return 0
    }
    return this.data.get(i, j)
  }

  getColor(i, j) {
    const colors = this.points.geometry.attributes.pointColor.array
    return COLOR_TEMP.fromArray(colors, this.data.addr(i, j) * 3)
  }

  setColor(i, j, c) {
    const colors = this.points.geometry.attributes.pointColor.array
    c.toArray(colors, this.data.addr(i, j) * 3)
    this.points.geometry.attributes.pointColor.needsUpdate = true
  }

  getSize(i, j) {
    return this.points.geometry.attributes.pointSize.array[this.data.addr(i, j)]
  }

  setSize(i, j, x) {
    this.points.geometry.attributes.pointSize.array[this.data.addr(i, j)] = x
    this.points.geometry.attributes.pointSize.needsUpdate = true
  }

  show(r = undefined, c = undefined) {
    this.setColorsAndSizes(r, c)
  }

  hide(r = undefined, c = undefined) {
    this.setColorsAndSizes(r, c, _ => 0, _ => ZERO_COLOR)
  }

  isHidden(i, j) {
    return this.getColor(i, j).equals(ZERO_COLOR)
  }

  bumpColor(r = undefined, c = undefined) {
    COLOR_TEMP.set(0x808080)
    this.setColorsAndSizes(r, c, undefined, x => this.colorFromData(x).add(COLOR_TEMP))
  }

  isFacing() {
    const c = this.group.localToWorld(new THREE.Vector3()).sub(this.context.camera.position).normalize()
    const m = this.group.getWorldDirection(new THREE.Vector3())
    return m.angleTo(c) < Math.PI / 2
  }

  isRightSideUp() {
    const q = new THREE.Quaternion()
    const p = new THREE.Vector3(0, -1, 0).applyQuaternion(this.group.getWorldQuaternion(q))
    const c = new THREE.Vector3(0, 1, 0).applyQuaternion(this.context.camera.quaternion)
    return p.angleTo(c) < Math.PI / 2
  }

  setRowGuides(light = undefined) {
    const prev = this.params.deco['row guides']
    light = util.syncProp(this.params.deco, 'row guides', light)
    if (this.row_guide_groups && prev == light) {
      return
    }
    if (this.row_guide_groups) {
      this.row_guide_groups.forEach(g => {
        this.inner_group.remove(g)
        util.disposeAndClear(g)
      })
    }
    this.row_guide_groups = []
    if (light > 0.0) {
      const gap = this.params.layout.gap
      this.grid('ij', (
        { start: i, extent: ix, index: ii },
        { start: j, extent: jx, index: ji }
      ) => {
        const g = util.rowGuide(ix, jx, light)
        util.updateProps(g.position, { x: j + ji * gap, y: i + ii * gap })
        this.inner_group.add(g)
        this.row_guide_groups.push(g)
      })
    }
  }

  setFlowGuide(light) { }

  setName(name) {
    util.syncProp(this.params, 'name', name)
    this.setLegends()
  }

  setLegends(size = undefined, shape = undefined) {
    shape = util.syncProp(this.params.deco, 'shape', shape)
    const facing = this.isFacing()
    const rsu = this.isRightSideUp()
    const [H, W] = [this.H, this.W]
    const name = this.params.name // && this.params.name + (shape ? ` [${H}, ${W}]` : '')

    if ((size === undefined || size == this.params.deco.legends) &&
      this.legend_state &&
      this.legend_state.facing == facing &&
      this.legend_state.rsu == rsu &&
      this.legend_state.name == name &&
      this.legend_state.shape == shape &&
      this.legend_state.H == H && this.legend_state.W == W) {
      return
    }

    size = util.syncProp(this.params.deco, 'legends', size)
    this.legend_state = { facing, rsu, name, shape, H, W }
    const rmv = x => {
      if (x) {
        this.inner_group.remove(x)
        util.disposeAndClear(x)
      }
    }
    rmv(this.name_text)
    rmv(this.hdim_text)
    rmv(this.wdim_text)

    if (size > 0) {
      const color = 0xCCCCFF
      const adjsiz = size * Math.cbrt(H * W) / 10
      const xdir = facing ? 1 : -1
      const ydir = rsu ? 1 : 0
      const zdir = facing ? 1 : -1
      if (name) {
        const adjsiz2 = adjsiz * Math.min(1, 8 / name.length)
        this.name_text = util.getText(name, color, adjsiz2)
        this.name_text.name = `${name}.name`
        this.name_text.geometry.rotateZ(Math.PI)
        this.name_text.geometry.rotateY(facing ? Math.PI : 0)
        const { h, w } = util.gbbhwd(this.name_text.geometry)
        this.name_text.geometry.translate(
          util.center(this.getDispW() - 1, xdir * w),
          h + util.center(this.getDispH() - 1, h),
          -zdir
        )
        this.inner_group.add(this.name_text)
      }
      if (shape && this.params.deco.shape_info) {
        const htext = util.getText("X", color, adjsiz / 2.5)
        const { h } = util.gbbhwd(htext.geometry)
        util.disposeAndClear(htext)
        const { i: { n: ni }, j: { n: nj } } = this.getBlockInfo()
        {
          const { h: { name, place } } = this.params.deco.shape_info
          const hdim_str = `${name} = ${H}` + (ni == 1 ? '' : ` / ${ni}`)
          this.hdim_text = util.getText(hdim_str, color, adjsiz / 2.5)
          const { w } = util.gbbhwd(this.hdim_text.geometry)
          this.hdim_text.geometry.rotateZ((place == facing ? 1 : -1) * Math.PI / 2)
          this.hdim_text.geometry.rotateY(facing ? Math.PI : 0)
          const xgap = 2 * h
          this.hdim_text.geometry.translate(
            place ? this.getDispW() - 1 + xgap : -xgap,
            (place == facing ? 0 : w) + util.center(this.getDispH() - 1, w),
            0
          )
          this.inner_group.add(this.hdim_text)
        }
        {
          const { w: { name, place } } = this.params.deco.shape_info
          const wdim_str = `${name} = ${W}` + (nj == 1 ? '' : ` / ${nj}`)
          this.wdim_text = util.getText(wdim_str, color, adjsiz / 2.5)
          const { w } = util.gbbhwd(this.wdim_text.geometry)
          this.wdim_text.name = `${name}.wdim`
          this.wdim_text.geometry.rotateZ(Math.PI)
          this.wdim_text.geometry.rotateY(facing ? Math.PI : 0)
          this.wdim_text.geometry.translate(
            util.center(this.getDispW() - 1, (facing ? 1 : -1) * w),
            place ? this.getDispH() - 1 + 3 * h : -2 * h,
            0
          )
          this.inner_group.add(this.wdim_text)
        }
      }
    }
  }

  checkLabel(i, j, x) {
    if (this.label_cache) {
      const addr = this.data.addr(i, j)
      const label = this.label_cache[addr]
      if (label != undefined && label.value != x) {
        util.disposeAndClear(label)
        this.label_cache[addr] = undefined
      }
    }
  }

  updateLabels(spotlight = undefined) {
    spotlight = util.syncProp(this.params.deco, 'spotlight', spotlight)
    if (spotlight == 0) {
      if (this.label_group) {
        this.inner_group.remove(this.label_group)
        util.disposeAndClear(this.label_group)
        this.label_group = undefined
      }
    } else {
      if (!this.label_group) {
        this.label_group = new THREE.Group()
        this.label_group.name = `${this.params.name}.label_group`
        this.inner_group.add(this.label_group)
        this.label_cache = []
      } else {
        util.disposeAndClear(this.label_group)
      }
      const gap = this.params.layout.gap
      const { i: { size: si }, j: { size: sj } } = this.getBlockInfo()
      this.context.raycaster.params.Points.threshold = spotlight
      const intersects = this.context.raycaster.intersectObject(this.points)
      let count = 0
      intersects.forEach(p => {
        const index = p.index
        const i = Math.floor(index / this.W)
        const j = index % this.W
        if (!this.isHidden(i, j)) {
          const x = this.getData(i, j)
          let label = this.label_cache[index]
          const facing = this.isFacing()
          const rsu = this.isRightSideUp()
          if (!label || label.facing != facing || label.rsu != rsu) {
            const fsiz = isNaN(x) || !isFinite(x) ? 0.12 :
              0.16 - 0.008 * Math.log10(Math.floor(1 + Math.abs(x)))
            label = util.getText(x.toFixed(5), 0xffffff, fsiz)
            count += 1
            // label.name = `${this.params.name}.label[${i}, ${j}]`
            label.value = x
            label.facing = facing
            label.rsu = rsu
            const zdir = facing ? 1 : -1
            label.geometry.rotateX(zdir * Math.PI)
            label.geometry.rotateY(facing ? 0 : Math.PI)
            label.geometry.rotateZ(rsu ? 0 : Math.PI)
            const { h, w } = util.gbbhwd(label.geometry)
            const disp_i = i + Math.floor(i / si) * gap
            const disp_j = j + Math.floor(j / sj) * gap
            label.geometry.translate(
              util.center(disp_j * 2, (rsu ? zdir : -zdir) * w),
              h + util.center(disp_i * 2, h),
              -zdir * 0.5
            )
            this.label_cache[index] = label
          }
          this.label_group.add(label)
        }
      })
    }
  }
}

//
// MatMul
//

