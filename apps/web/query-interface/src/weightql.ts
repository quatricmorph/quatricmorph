/**
 * Client-side WeightQL parser — Metadata Plane (ARCHITECTURE.md §7).
 *
 * A faithful subset of `crates/q-weightql`: same tokens, same grammar, same
 * rejections. It exists so the input box can report a syntax error at the
 * character without a round trip, and so the KaTeX preview has an AST to
 * render.
 *
 * The daemon remains the authority. This parser decides *whether the text is
 * well-formed*; only the daemon can decide whether `Q[10]` resolves, what shape
 * it has, or whether an expression type-checks — those need the catalog.
 *
 * **No evaluation.** There is no `eval`, no `Function`, no dynamic import, and
 * no construct that could introduce one. The function set is closed and matches
 * the Rust side exactly.
 */

export type Token =
  | { kind: 'ident'; value: string; at: number }
  | { kind: 'string'; value: string; at: number }
  | { kind: 'int'; value: number; at: number }
  | { kind: 'punct'; value: string; at: number }

/** The closed function set. Adding to it here without adding to the Rust
 *  parser makes `parity.test.ts` fail, which is the point. */
export const FUNCTIONS = [
  'tensor',
  'transpose',
  'compare',
  'min',
  'max',
  'mean',
  'variance',
  'stddev',
  'l1_norm',
  'l2_norm',
  'zero_ratio',
] as const

export type FunctionName = (typeof FUNCTIONS)[number]

export class WeightQLError extends Error {
  constructor(
    message: string,
    readonly at: number,
  ) {
    super(message)
    this.name = 'WeightQLError'
  }
}

const PUNCT = new Set(['@', '+', '-', '=', ',', ':', ';', '(', ')', '[', ']'])

export function tokenize(src: string): Token[] {
  const out: Token[] = []
  let i = 0
  while (i < src.length) {
    const c = src[i]
    if (/\s/.test(c)) {
      i++
      continue
    }
    if (c === '-' && src[i + 1] === '-') {
      while (i < src.length && src[i] !== '\n') i++
      continue
    }
    const at = i
    if (c === '"') {
      i++
      let value = ''
      for (;;) {
        if (i >= src.length) throw new WeightQLError('unterminated string literal', at)
        if (src[i] === '"') {
          i++
          break
        }
        if (src[i] === '\\') {
          const next = src[i + 1]
          if (next !== '"' && next !== '\\') {
            throw new WeightQLError(
              `unsupported escape \\${next ?? ''} — only \\" and \\\\ are supported`,
              i,
            )
          }
          value += next
          i += 2
          continue
        }
        value += src[i]
        i++
      }
      out.push({ kind: 'string', value, at })
      continue
    }
    if (/[0-9]/.test(c)) {
      let j = i
      while (j < src.length && /[0-9]/.test(src[j])) j++
      out.push({ kind: 'int', value: Number(src.slice(i, j)), at })
      i = j
      continue
    }
    if (/[A-Za-z_]/.test(c)) {
      let j = i
      while (j < src.length && /[A-Za-z0-9_.]/.test(src[j])) j++
      out.push({ kind: 'ident', value: src.slice(i, j), at })
      i = j
      continue
    }
    if (PUNCT.has(c)) {
      out.push({ kind: 'punct', value: c, at })
      i++
      continue
    }
    throw new WeightQLError(`unexpected character \`${c}\``, at)
  }
  return out
}

export type Expr =
  | { kind: 'tensor'; address: string }
  | { kind: 'binding'; name: string }
  | { kind: 'transpose'; operand: Expr }
  | { kind: 'matmul'; left: Expr; right: Expr }
  | { kind: 'add'; left: Expr; right: Expr }
  | { kind: 'sub'; left: Expr; right: Expr }
  | { kind: 'reduce'; fn: FunctionName; operand: Expr }
  | { kind: 'compare'; left: Expr; right: Expr; metric: string }
  | { kind: 'slice'; operand: Expr; terms: string[] }

export type Statement =
  | { kind: 'assign'; name: string; expr: Expr }
  | { kind: 'show'; expr: Expr }

export type Script = { statements: Statement[] }

class Parser {
  private pos = 0
  constructor(
    private readonly tokens: Token[],
    private readonly sourceLength: number,
  ) {}

  private peek(): Token | undefined {
    return this.tokens[this.pos]
  }

  /**
   * Position to blame for an error.
   *
   * When the parser has run out of tokens the offence is at end-of-input, not
   * at character 0 — otherwise `show A @` puts the caret under the `s`.
   */
  private at(): number {
    return this.peek()?.at ?? this.sourceLength
  }

  private isPunct(v: string): boolean {
    const t = this.peek()
    return t?.kind === 'punct' && t.value === v
  }

  private eatPunct(v: string): boolean {
    if (this.isPunct(v)) {
      this.pos++
      return true
    }
    return false
  }

  private expectPunct(v: string): void {
    if (!this.eatPunct(v)) throw new WeightQLError(`expected \`${v}\``, this.at())
  }

  private isKeyword(kw: string): boolean {
    const t = this.peek()
    return t?.kind === 'ident' && t.value.toLowerCase() === kw
  }

  script(): Script {
    const statements: Statement[] = []
    while (this.pos < this.tokens.length) {
      while (this.eatPunct(';')) {
        /* skip */
      }
      if (this.pos >= this.tokens.length) break
      statements.push(this.statement())
    }
    if (statements.length === 0) throw new WeightQLError('empty query', 0)
    return { statements }
  }

  private statement(): Statement {
    if (this.isKeyword('show')) {
      this.pos++
      return { kind: 'show', expr: this.expr() }
    }
    const t = this.peek()
    const next = this.tokens[this.pos + 1]
    if (t?.kind === 'ident' && next?.kind === 'punct' && next.value === '=') {
      this.pos += 2
      return { kind: 'assign', name: t.value, expr: this.expr() }
    }
    throw new WeightQLError('expected `NAME = expr` or `show expr`', this.at())
  }

  private expr(): Expr {
    let left = this.mul()
    for (;;) {
      if (this.eatPunct('+')) left = { kind: 'add', left, right: this.mul() }
      else if (this.eatPunct('-')) left = { kind: 'sub', left, right: this.mul() }
      else break
    }
    return left
  }

  private mul(): Expr {
    let left = this.postfix()
    while (this.eatPunct('@')) left = { kind: 'matmul', left, right: this.postfix() }
    return left
  }

  private postfix(): Expr {
    let e = this.primary()
    while (this.isPunct('[')) e = { kind: 'slice', operand: e, terms: this.subscript() }
    return e
  }

  private subscript(): string[] {
    this.expectPunct('[')
    const terms: string[] = []
    for (;;) {
      let term = ''
      if (this.eatPunct(':')) {
        term = ':'
        const t = this.peek()
        if (t?.kind === 'int') {
          term = `:${t.value}`
          this.pos++
        }
      } else {
        const t = this.peek()
        if (t?.kind !== 'int') throw new WeightQLError('expected an integer', this.at())
        this.pos++
        term = String(t.value)
        if (this.eatPunct(':')) {
          const u = this.peek()
          if (u?.kind === 'int') {
            term += `:${u.value}`
            this.pos++
          } else {
            term += ':'
          }
        }
      }
      terms.push(term)
      if (this.eatPunct(',')) continue
      break
    }
    this.expectPunct(']')
    if (terms.length === 0) throw new WeightQLError('empty subscript', this.at())
    return terms
  }

  private primary(): Expr {
    if (this.eatPunct('(')) {
      const e = this.expr()
      this.expectPunct(')')
      return e
    }
    const t = this.peek()
    if (t?.kind !== 'ident') {
      throw new WeightQLError('expected a tensor reference or function call', this.at())
    }
    const name = t.value
    const lower = name.toLowerCase()
    const calls = this.tokens[this.pos + 1]
    const isCall = calls?.kind === 'punct' && calls.value === '('

    if (lower === 'tensor') {
      this.pos++
      this.expectPunct('(')
      const s = this.peek()
      if (s?.kind !== 'string') {
        throw new WeightQLError('expected a quoted tensor address or alias', this.at())
      }
      this.pos++
      this.expectPunct(')')
      return { kind: 'tensor', address: s.value }
    }
    if (lower === 'transpose') {
      this.pos++
      this.expectPunct('(')
      const inner = this.expr()
      this.expectPunct(')')
      return { kind: 'transpose', operand: inner }
    }
    if (lower === 'compare') {
      this.pos++
      this.expectPunct('(')
      const left = this.expr()
      this.expectPunct(',')
      const right = this.expr()
      this.expectPunct(')')
      if (!this.isKeyword('by')) throw new WeightQLError('expected `by`', this.at())
      this.pos++
      const m = this.peek()
      if (m?.kind !== 'ident') throw new WeightQLError('expected a metric name', this.at())
      this.pos++
      if (m.value !== 'cosine_similarity' && m.value !== 'relative_l2') {
        throw new WeightQLError(
          `unknown comparison metric \`${m.value}\`; supported: cosine_similarity, relative_l2`,
          m.at,
        )
      }
      return { kind: 'compare', left, right, metric: m.value }
    }
    if ((FUNCTIONS as readonly string[]).includes(lower) && isCall) {
      this.pos++
      this.expectPunct('(')
      const inner = this.expr()
      this.expectPunct(')')
      return { kind: 'reduce', fn: lower as FunctionName, operand: inner }
    }
    if (isCall) {
      throw new WeightQLError(
        `unknown function \`${name}\`. WeightQL has a fixed function set: ` +
          `${FUNCTIONS.join(', ')}. There is no \`eval\` and no way to define new functions.`,
        t.at,
      )
    }
    this.pos++
    return { kind: 'binding', name }
  }
}

export function parse(src: string): Script {
  return new Parser(tokenize(src), src.length).script()
}

/** Whether the text parses, without throwing. */
export function check(src: string): { ok: true; script: Script } | { ok: false; message: string; at: number } {
  try {
    return { ok: true, script: parse(src) }
  } catch (e) {
    if (e instanceof WeightQLError) return { ok: false, message: e.message, at: e.at }
    return { ok: false, message: String(e), at: 0 }
  }
}
