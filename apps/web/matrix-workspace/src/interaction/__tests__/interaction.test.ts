import { describe, expect, it } from 'vitest'
import {
  createAnimState,
  stepForward,
  stepBackward,
  resetAnim,
  cellCoords,
} from '../animation.js'
import { selectOutput, pathFromSelection, clearSelection } from '../selection.js'

describe('animation state machine', () => {
  it('steps deterministically through K then cells', () => {
    let s = createAnimState(2, 3, 2)
    const product = (i: number, j: number, k: number) => i * 100 + j * 10 + k
    s = stepForward(s, product)
    expect(s.kIndex).toBe(1)
    expect(s.runningSum).toBe(0) // i=0,j=0,k=0 → 0
    s = stepForward(s, product)
    expect(s.kIndex).toBe(2)
    // finish cell → next cell
    s = stepForward(s, product)
    expect(s.cellIndex).toBe(1)
    expect(s.kIndex).toBe(0)
    expect(cellCoords(s)).toEqual({ i: 0, j: 1 })
  })

  it('reset returns to start', () => {
    let s = createAnimState(2, 2, 2)
    s = stepForward(s, () => 1)
    s = resetAnim(s)
    expect(s.cellIndex).toBe(0)
    expect(s.kIndex).toBe(0)
    expect(s.status).toBe('idle')
  })

  it('stepBackward does not go below zero', () => {
    let s = createAnimState(1, 2, 1)
    s = stepBackward(s)
    expect(s.cellIndex).toBe(0)
    expect(s.kIndex).toBe(0)
  })
})

describe('selection path', () => {
  it('selects C[i,j] path for A row and B col', () => {
    const sel = selectOutput(1, 0, 2, 2)
    const path = pathFromSelection(sel)
    expect(path).toEqual({ aRow: 1, bCol: 0, cCell: { i: 1, j: 0 } })
  })

  it('clears selection', () => {
    expect(clearSelection().kind).toBe('none')
  })

  it('rejects out of bounds', () => {
    expect(selectOutput(5, 0, 2, 2).kind).toBe('none')
  })
})
