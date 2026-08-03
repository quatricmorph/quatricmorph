import { describe, expect, it } from 'vitest'
import { check, parse, tokenize, WeightQLError } from '../weightql.js'
import { exprToLatex, scriptToLatex } from '../katex-preview.js'
import { buildQueryRequest, isSubmittable, preview } from '../app.js'

describe('CHAT-002 client-side WeightQL parser', () => {
  it('parses the ARCHITECTURE.md §7.4 script', () => {
    const script = parse(`
      A = tensor("Q[10][0:256,0:256]")
      B = transpose(tensor("K[10][0:256,0:256]"))
      show A @ B
    `)
    expect(script.statements.length).toBe(3)
    expect(script.statements[2]).toMatchObject({
      kind: 'show',
      expr: { kind: 'matmul' },
    })
  })

  it('is left-associative, matching the Rust parser', () => {
    const script = parse('show A @ B @ C')
    const show = script.statements[0]
    expect(show.kind).toBe('show')
    if (show.kind !== 'show' || show.expr.kind !== 'matmul') throw new Error('shape')
    // ((A @ B) @ C): the left operand is itself a matmul.
    expect(show.expr.left.kind).toBe('matmul')
    expect(show.expr.right).toEqual({ kind: 'binding', name: 'C' })
  })

  it('honours explicit parentheses', () => {
    const script = parse('show A @ (B @ C)')
    const show = script.statements[0]
    if (show.kind !== 'show' || show.expr.kind !== 'matmul') throw new Error('shape')
    expect(show.expr.left.kind).toBe('binding')
    expect(show.expr.right.kind).toBe('matmul')
  })

  it('parses subscripts in every documented spelling', () => {
    for (const src of ['show A[100,42]', 'show A[0:256,0:256]', 'show A[:]', 'show A[0:128,:]']) {
      expect(check(src).ok).toBe(true)
    }
  })

  it('rejects arbitrary-code-execution constructs', () => {
    for (const hostile of [
      'show eval("1+1")',
      'A = Function("return 1")',
      'show system("rm -rf /")',
      'show require("fs")',
      'show import("x")',
    ]) {
      expect(check(hostile).ok).toBe(false)
    }
    const r = check('show eval("x")')
    expect(r.ok).toBe(false)
    if (!r.ok) {
      expect(r.message).toContain('fixed function set')
      expect(r.message).toContain('no `eval`')
    }
  })

  it('reports the position of a syntax error', () => {
    const r = check('show A @')
    expect(r.ok).toBe(false)
    if (!r.ok) expect(r.at).toBeGreaterThan(0)
    expect(() => parse('show A[0:')).toThrow(WeightQLError)
  })

  it('rejects unsupported string escapes, matching the Rust lexer', () => {
    expect(check('show tensor("a\\nb")').ok).toBe(false)
    expect(check('show tensor("a\\"b")').ok).toBe(true)
  })

  it('skips comments', () => {
    expect(check('show A -- ignored\n@ B').ok).toBe(true)
  })

  it('tokenizes dotted identifiers as one token', () => {
    const toks = tokenize('MLP.down')
    expect(toks.length).toBe(1)
    expect(toks[0]).toMatchObject({ kind: 'ident', value: 'MLP.down' })
  })
})

describe('CHAT-003 KaTeX preview', () => {
  it('renders the grouping the parser chose, not the source order', () => {
    const flat = scriptToLatex(parse('show A @ B @ C'))[0]
    const grouped = scriptToLatex(parse('show A @ (B @ C)'))[0]
    expect(flat).not.toBe(grouped)
    // The explicitly grouped form must show parentheses.
    expect(grouped).toContain('\\left(')
  })

  it('renders transpose as a superscript', () => {
    const latex = scriptToLatex(parse('show transpose(A)'))[0]
    expect(latex).toContain('^{\\top}')
  })

  it('renders tensor addresses safely', () => {
    const latex = scriptToLatex(parse('show tensor("model.layers[10].self_attention.query_projection.weight")'))[0]
    expect(latex).toContain('\\mathbf{')
    // LaTeX-significant characters in an address must be escaped.
    const tricky = exprToLatex({ kind: 'tensor', address: 'a_b#c' })
    expect(tricky).toContain('\\_')
    expect(tricky).toContain('\\#')
  })

  it('renders assignments and slices', () => {
    const lines = scriptToLatex(parse('A = tensor("Q[10]")\nshow A[0:4,0:4]'))
    expect(lines.length).toBe(2)
    expect(lines[0]).toContain('=')
    expect(lines[1]).toContain('\\left[')
  })

  it('renders reductions and comparisons', () => {
    expect(scriptToLatex(parse('show l2_norm(A)'))[0]).toContain('\\lVert')
    expect(scriptToLatex(parse('show compare(A, B) by cosine_similarity'))[0]).toContain(
      'cosine',
    )
  })
})

describe('CHAT-004 preview state', () => {
  it('is empty for empty input', () => {
    expect(preview('   ').status).toBe('empty')
    expect(isSubmittable(preview(''))).toBe(false)
  })

  it('reports a caret under the offending character', () => {
    const state = preview('show A @')
    expect(state.status).toBe('invalid')
    if (state.status === 'invalid') {
      expect(state.caret.endsWith('^')).toBe(true)
      expect(state.caret.length).toBe(state.at + 1)
    }
  })

  it('is submittable only when it parses', () => {
    expect(isSubmittable(preview('show tensor("Q[10][100,42]")'))).toBe(true)
    expect(isSubmittable(preview('show eval("x")'))).toBe(false)
    expect(() => buildQueryRequest('m', 'show eval("x")')).toThrow(/does not parse/)
    expect(buildQueryRequest('m', 'show tensor("Q[10][100,42]")')).toEqual({
      model: 'm',
      expression: 'show tensor("Q[10][100,42]")',
    })
  })
})
