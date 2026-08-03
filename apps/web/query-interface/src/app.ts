/**
 * The query interface: an input, a parse, and a preview.
 *
 * Deliberately not a chat interface (`CHAT-001`). ARCHITECTURE.md §15 requires
 * an assistant to produce a plan and show its estimated I/O before executing
 * anything, and to reach weights only through the WeightQL planner. None of
 * that exists yet, so this app offers the honest subset: type an expression,
 * see whether it parses, see how it was read.
 */

import { scriptToLatex } from './katex-preview.js'
import { check } from './weightql.js'

export type PreviewState =
  | { status: 'empty' }
  | { status: 'valid'; latex: string[]; statements: number }
  | { status: 'invalid'; message: string; at: number; caret: string }

/**
 * Compute the preview for the current input text.
 *
 * Pure, so the whole interaction is testable without a DOM.
 */
export function preview(input: string): PreviewState {
  if (input.trim() === '') return { status: 'empty' }
  const result = check(input)
  if (!result.ok) {
    return {
      status: 'invalid',
      message: result.message,
      at: result.at,
      caret: `${' '.repeat(Math.max(0, result.at))}^`,
    }
  }
  return {
    status: 'valid',
    latex: scriptToLatex(result.script),
    statements: result.script.statements.length,
  }
}

/**
 * Whether the daemon should be asked to run this.
 *
 * A valid parse is necessary but not sufficient: the daemon still resolves
 * references and checks shapes, and may reject what parses fine here.
 */
export function isSubmittable(state: PreviewState): boolean {
  return state.status === 'valid'
}

/** The request body for `POST /v1/query`. */
export function buildQueryRequest(modelId: string, input: string): { model: string; expression: string } {
  const state = preview(input)
  if (!isSubmittable(state)) {
    throw new Error('refusing to submit a query that does not parse')
  }
  return { model: modelId, expression: input }
}
