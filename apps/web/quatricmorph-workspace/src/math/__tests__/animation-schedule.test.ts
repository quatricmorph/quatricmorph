import { describe, expect, it } from 'vitest'
import { AnimationCursor, cycleLength } from '../animation-schedule.js'

describe('MATMUL-03 animation schedule', () => {
  it('walks primary only when not sweeping', () => {
    const steps = new AnimationCursor(3, 4, false).cycle()
    expect(steps.map((s) => s.primary)).toEqual([0, 1, 2, 3])
    expect(steps.every((s) => s.secondary === 0)).toBe(true)
    expect(steps.at(-1)!.done).toBe(true)
    // Three live steps plus one terminating step.
    expect(steps.filter((s) => !s.done).length).toBe(3)
  })

  it('sweeps the secondary axis before advancing the primary', () => {
    const steps = new AnimationCursor(2, 3, true).cycle().filter((s) => !s.done)
    expect(steps.map((s) => [s.primary, s.secondary])).toEqual([
      [0, 0],
      [0, 1],
      [0, 2],
      [1, 0],
      [1, 1],
      [1, 2],
    ])
  })

  it('flags exactly one cycle start', () => {
    const steps = new AnimationCursor(2, 3, true).cycle()
    expect(steps.filter((s) => s.cycleStart).length).toBe(1)
    expect(steps[0].cycleStart).toBe(true)
  })

  it('reports the previous position so old highlights can be cleared', () => {
    const c = new AnimationCursor(2, 2, true)
    const first = c.next()
    expect(first.previousPrimary).toBe(-1)
    const second = c.next()
    expect(second.previousPrimary).toBe(0)
    expect(second.previousSecondary).toBe(0)
  })

  it('terminates immediately for an empty primary axis', () => {
    const steps = new AnimationCursor(0, 4, false).cycle()
    expect(steps.length).toBe(1)
    expect(steps[0].done).toBe(true)
  })

  it('rejects negative sizes rather than looping forever', () => {
    expect(() => new AnimationCursor(-1, 2, true)).toThrow()
  })

  it('cycle length matches the number of live steps', () => {
    const blocks = { i: 3, k: 2, j: 4 }
    const live = new AnimationCursor(blocks.i, blocks.j, true)
      .cycle()
      .filter((s) => !s.done).length
    expect(live).toBe(cycleLength('vmprod', blocks))
    expect(cycleLength('mvprod', blocks)).toBe(12)
    expect(cycleLength('vvprod', blocks)).toBe(2)
  })
})
