// @ts-nocheck
import * as util from '../util.js'
import { POLARITIES, LEFT_PLACEMENTS, RIGHT_PLACEMENTS, RESULT_PLACEMENTS } from './constants.js'

const layoutToBool = layout => ({
  pol: !!POLARITIES.indexOf(layout.polarity),
  left: !!LEFT_PLACEMENTS.indexOf(layout['left placement']),
  right: !!RIGHT_PLACEMENTS.indexOf(layout['right placement']),
  res: !!RESULT_PLACEMENTS.indexOf(layout['result placement'])
})

const boolToLayout = ({ pol, left, right, res }) => ({
  polarity: POLARITIES[+pol],
  'left placement': LEFT_PLACEMENTS[+left],
  'right placement': RIGHT_PLACEMENTS[+right],
  'result placement': RESULT_PLACEMENTS[+res]
})

export const LAYOUT_RULES = {
  'blocks': (left_child, { pol, left, right, res }) => ({
    pol: !pol,
    left: left_child ? pol != res : !left,
    right: left_child ? !right : pol != res,
    res: pol == (left_child ? left : right),
  }),
  'zigzag': (left_child, { pol, left, right, res }) => ({
    pol: !pol,
    left: left_child ? pol != res : left,
    right: left_child ? right : pol != res,
    res: pol == (left_child ? left : right),
  }),
  'wheel': (left_child, { pol, left, right, res }) => ({
    pol: pol,
    left: left,
    right: right,
    res: res
  }),
}

export const childLayout = (parent_layout, rule, left_child) =>
  boolToLayout(rule(left_child, layoutToBool(parent_layout)))

export function setLayoutScheme(params, scheme_name) {
  scheme_name = util.syncProp(params.layout, 'scheme', scheme_name)
  const rule = LAYOUT_RULES[scheme_name]
  function f(p) {
    if (p.left.matmul) {
      p.left.layout = childLayout(p.layout, rule, true)
      f(p.left)
    }
    if (p.right.matmul) {
      p.right.layout = childLayout(p.layout, rule, false)
      f(p.right)
    }
  }
  rule && f(params)
}

// 
// exprs
//

