/**
 * KaTeX rendering of a parsed WeightQL expression.
 *
 * Renders **the AST the parser produced**, not the raw text. That is the whole
 * value of the preview: if the user typed `A @ B @ C` and meant `A @ (B @ C)`,
 * the preview shows `(A B) C` and the mistake is visible before the query runs.
 *
 * Emits a KaTeX source string. No KaTeX dependency is bundled here — the host
 * page supplies the renderer — so this module stays pure and testable.
 */

import type { Expr, Script, Statement } from './weightql.js'

/** Escape a tensor address for use inside `\text{}`. */
function textEscape(s: string): string {
  return s.replace(/([\\{}$&#^_%~])/g, '\\$1')
}

const REDUCTION_LATEX: Record<string, string> = {
  min: '\\min',
  max: '\\max',
  mean: '\\operatorname{mean}',
  variance: '\\operatorname{var}',
  stddev: '\\sigma',
  l1_norm: '\\lVert\\cdot\\rVert_1',
  l2_norm: '\\lVert\\cdot\\rVert_2',
  zero_ratio: '\\operatorname{zero\\_ratio}',
}

/**
 * Render one expression.
 *
 * `parentPrecedence` drives parenthesization: the preview must show the
 * grouping the parser chose, so parentheses appear wherever the tree differs
 * from left-to-right reading.
 */
export function exprToLatex(expr: Expr, parentPrecedence = 0): string {
  const wrap = (s: string, mine: number): string =>
    mine < parentPrecedence ? `\\left(${s}\\right)` : s

  switch (expr.kind) {
    case 'tensor':
      return `\\mathbf{${textEscape(expr.address)}}`
    case 'binding':
      return `\\mathbf{${textEscape(expr.name)}}`
    case 'transpose':
      return `${exprToLatex(expr.operand, 3)}^{\\top}`
    case 'matmul':
      return wrap(
        `${exprToLatex(expr.left, 2)} \\cdot ${exprToLatex(expr.right, 3)}`,
        2,
      )
    case 'add':
      return wrap(`${exprToLatex(expr.left, 1)} + ${exprToLatex(expr.right, 2)}`, 1)
    case 'sub':
      return wrap(`${exprToLatex(expr.left, 1)} - ${exprToLatex(expr.right, 2)}`, 1)
    case 'reduce':
      return `${REDUCTION_LATEX[expr.fn] ?? `\\operatorname{${expr.fn}}`}\\left(${exprToLatex(expr.operand)}\\right)`
    case 'compare':
      return `\\operatorname{${textEscape(expr.metric)}}\\left(${exprToLatex(expr.left)}, ${exprToLatex(expr.right)}\\right)`
    case 'slice':
      return `${exprToLatex(expr.operand, 3)}\\left[${expr.terms.map(textEscape).join(',\\,')}\\right]`
  }
}

export function statementToLatex(statement: Statement): string {
  return statement.kind === 'assign'
    ? `\\mathbf{${textEscape(statement.name)}} = ${exprToLatex(statement.expr)}`
    : exprToLatex(statement.expr)
}

/** One LaTeX line per statement, in source order. */
export function scriptToLatex(script: Script): string[] {
  return script.statements.map(statementToLatex)
}
