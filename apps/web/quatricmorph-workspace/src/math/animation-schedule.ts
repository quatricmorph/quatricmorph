/**
 * Matrix-multiplication animation schedules — **pure index math, no Three.js**.
 *
 * Extracted from `viz/matmul.ts`, where the cursor arithmetic driving
 * `getVmprodBump` / `getMvprodBump` / `getVvprodBump` was interleaved with
 * `bumpColor`, `setRowGuides`, and scene-graph mutation. The schedule is what
 * decides *which* indices are active at step t; the renderer decides how they
 * look. Separating them means the schedule is testable without a WebGL context
 * — which is why the tests below exist at all.
 *
 * ARCHITECTURE.md §8.2 animation: highlight A[i,k] → highlight B[k,j] →
 * multiply → accumulate C[i,j].
 */

/** Which product decomposition the animation walks. */
export type AnimationAlgorithm =
  /** vector × matrix: one row of A against all of B. */
  | 'vmprod'
  /** matrix × vector: all of A against one column of B. */
  | 'mvprod'
  /** vector × vector: one outer product per k. */
  | 'vvprod'

/** The cursor's position and what changed since the previous step. */
export type AnimationStep = {
  /** Step ordinal, from 0. */
  step: number
  /** Active index along the primary axis, or -1 before the first step. */
  primary: number
  /** Active index along the sweep axis; always 0 when not sweeping. */
  secondary: number
  /** Previous primary, for clearing the old highlight. */
  previousPrimary: number
  previousSecondary: number
  /** True on the first step of a cycle: reset intermediates. */
  cycleStart: boolean
  /** True once the cursor has walked past the end: the animation is done. */
  done: boolean
}

/**
 * The cursor from `getVmprodBump` et al., as a standalone state machine.
 *
 * Faithful to the original: `secondary` advances first and wraps, `primary`
 * advances when `secondary` wraps to 0, and the cycle ends when `primary`
 * reaches `primarySize`. With `sweep = false`, `secondary` stays at 0 and each
 * step advances `primary`.
 */
export class AnimationCursor {
  private primary: number
  private secondary: number
  private steps = 0

  constructor(
    readonly primarySize: number,
    readonly secondarySize: number,
    readonly sweep: boolean,
  ) {
    if (primarySize < 0 || secondarySize < 0) {
      throw new Error(
        `AnimationCursor sizes must be non-negative, got ${primarySize}x${secondarySize}`,
      )
    }
    this.primary = -1
    this.secondary = sweep ? -1 : 0
  }

  /** Advance one step. */
  next(): AnimationStep {
    const previousPrimary = this.primary
    const previousSecondary = this.secondary
    if (this.sweep) {
      this.secondary = this.secondarySize === 0 ? 0 : (this.secondary + 1) % this.secondarySize
    }
    if (this.secondary === 0) {
      this.primary++
    }
    const done = this.primary >= this.primarySize
    return {
      step: this.steps++,
      primary: this.primary,
      secondary: this.secondary,
      previousPrimary,
      previousSecondary,
      cycleStart: this.primary === 0 && this.secondary === 0,
      done,
    }
  }

  /** Every step of one full cycle, including the terminating `done` step. */
  cycle(limit = 100_000): AnimationStep[] {
    const out: AnimationStep[] = []
    for (let n = 0; n < limit; n++) {
      const s = this.next()
      out.push(s)
      if (s.done) return out
    }
    throw new Error(`animation cycle exceeded ${limit} steps; sizes look wrong`)
  }
}

/**
 * Steps in one full cycle of an algorithm, excluding the terminating step.
 *
 * Useful for progress reporting and for estimating how long an animation runs
 * before it starts.
 */
export function cycleLength(
  algorithm: AnimationAlgorithm,
  blocks: { i: number; k: number; j: number },
): number {
  switch (algorithm) {
    case 'vmprod':
      // One row of A per step, sweeping columns of B.
      return blocks.i * blocks.j
    case 'mvprod':
      // One column of B per step, sweeping rows of A.
      return blocks.j * blocks.i
    case 'vvprod':
      // One outer product per k.
      return blocks.k
  }
}
