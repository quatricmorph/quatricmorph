/**
 * A JSON-Schema draft-07 **subset** validator, sized to
 * `schemas/diagnostics/manifest.v1.json`.
 *
 * Why hand-written rather than a library: this package must validate with no
 * network and no new dependency, and the repository's rule is that a manifest
 * which does not satisfy the published schema is *refused*, not rendered. A
 * validator is the mechanism that makes that refusal real.
 *
 * The danger of a subset validator is that it meets a keyword it does not know,
 * ignores it, and passes a malformed document. That is guarded structurally:
 * `collectKeywords` enumerates every keyword the published schema actually
 * uses, and a test asserts the enumeration is covered by `IMPLEMENTED_KEYWORDS`
 * or `ANNOTATION_KEYWORDS`. Add a keyword to the schema without teaching it
 * here and the suite turns red rather than passing vacuously.
 */

/** Keywords this validator evaluates. */
export const IMPLEMENTED_KEYWORDS: readonly string[] = [
  '$ref',
  'additionalProperties',
  'allOf',
  'const',
  'else',
  'enum',
  'exclusiveMinimum',
  'if',
  'items',
  'maxItems',
  'maximum',
  'minItems',
  'minLength',
  'minimum',
  'not',
  'properties',
  'required',
  'then',
  'type',
] as const

/**
 * Keywords this validator knowingly ignores.
 *
 * `format` is an annotation in draft-07 by default, and the schema pairs its
 * one use (`date-time`) with `minLength: 20`, which is the constraint that
 * actually bites. The rest are documentation or structure.
 */
export const ANNOTATION_KEYWORDS: readonly string[] = [
  '$id',
  '$schema',
  'definitions',
  'description',
  'examples',
  'format',
  'title',
] as const

export type ValidationError = {
  /** JSON pointer to the offending value, `''` for the document root. */
  path: string
  keyword: string
  message: string
}

type Schema = Record<string, unknown>

/** Sub-schema positions, so the enumerator descends without treating a property name as a keyword. */
const SCHEMA_VALUED_KEYWORDS = new Set(['items', 'not', 'if', 'then', 'else', 'additionalProperties'])
const SCHEMA_MAP_KEYWORDS = new Set(['properties', 'definitions'])
const SCHEMA_LIST_KEYWORDS = new Set(['allOf', 'anyOf', 'oneOf'])

/**
 * Every keyword the given schema uses, anywhere.
 *
 * Descends through `properties`, `definitions`, `items`, `allOf`, `if`/`then`/
 * `else` and `not`. Values under `properties` and `definitions` are treated as
 * sub-schemas rather than as keywords, which is why the two maps are named
 * explicitly instead of inferred.
 */
export function collectKeywords(schema: unknown): Set<string> {
  const found = new Set<string>()
  walk(schema)
  return found

  function walk(node: unknown): void {
    if (node === null || typeof node !== 'object' || Array.isArray(node)) return
    for (const [key, value] of Object.entries(node as Schema)) {
      found.add(key)
      if (SCHEMA_MAP_KEYWORDS.has(key)) {
        for (const child of Object.values(value as Schema)) walk(child)
      } else if (SCHEMA_LIST_KEYWORDS.has(key)) {
        for (const child of (value as unknown[]) ?? []) walk(child)
      } else if (SCHEMA_VALUED_KEYWORDS.has(key)) {
        walk(value)
      } else if (key === 'examples') {
        // Instance documents, not schemas. Do not mine them for keywords.
      }
    }
  }
}

/** Validate `value` against `schema`. An empty array means it conforms. */
export function validate(schema: unknown, value: unknown): ValidationError[] {
  const errors: ValidationError[] = []
  check(schema as Schema, value, '', schema as Schema, errors)
  return errors
}

/** Whether `value` conforms, with no error detail. Used by `if` branches. */
function conforms(schema: Schema, value: unknown, root: Schema): boolean {
  const errors: ValidationError[] = []
  check(schema, value, '', root, errors)
  return errors.length === 0
}

function resolveRef(pointer: string, root: Schema): Schema {
  if (!pointer.startsWith('#/')) {
    throw new Error(`this validator resolves only local $ref pointers, not ${pointer}`)
  }
  let node: unknown = root
  for (const segment of pointer.slice(2).split('/')) {
    node = (node as Record<string, unknown>)?.[decodeURIComponent(segment)]
    if (node === undefined) throw new Error(`$ref ${pointer} does not resolve`)
  }
  return node as Schema
}

function typeName(value: unknown): string {
  if (value === null) return 'null'
  if (Array.isArray(value)) return 'array'
  return typeof value
}

function matchesType(expected: string, value: unknown): boolean {
  switch (expected) {
    case 'object':
      return value !== null && typeof value === 'object' && !Array.isArray(value)
    case 'array':
      return Array.isArray(value)
    case 'string':
      return typeof value === 'string'
    case 'number':
      return typeof value === 'number' && Number.isFinite(value)
    case 'integer':
      return typeof value === 'number' && Number.isInteger(value)
    case 'boolean':
      return typeof value === 'boolean'
    case 'null':
      return value === null
    default:
      throw new Error(`this validator does not know the type \`${expected}\``)
  }
}

function check(
  schema: Schema,
  value: unknown,
  path: string,
  root: Schema,
  errors: ValidationError[],
): void {
  if (typeof schema.$ref === 'string') {
    check(resolveRef(schema.$ref, root), value, path, root, errors)
    return
  }

  if (typeof schema.type === 'string' && !matchesType(schema.type, value)) {
    errors.push({
      path,
      keyword: 'type',
      message: `${path || '/'} should be ${schema.type} but is ${typeName(value)}`,
    })
    // Every other keyword here is about a value of the declared type.
    return
  }

  if ('const' in schema && !Object.is(schema.const, value)) {
    errors.push({
      path,
      keyword: 'const',
      message: `${path || '/'} should be ${JSON.stringify(schema.const)} but is ${JSON.stringify(value)}`,
    })
  }

  if (Array.isArray(schema.enum) && !schema.enum.some((allowed) => Object.is(allowed, value))) {
    errors.push({
      path,
      keyword: 'enum',
      message: `${path || '/'} is ${JSON.stringify(value)}, which is not one of ${JSON.stringify(schema.enum)}`,
    })
  }

  if (typeof value === 'string') {
    if (typeof schema.minLength === 'number' && value.length < schema.minLength) {
      errors.push({
        path,
        keyword: 'minLength',
        message: `${path || '/'} is ${value.length} characters, below the minimum of ${schema.minLength}`,
      })
    }
  }

  if (typeof value === 'number') {
    if (typeof schema.minimum === 'number' && value < schema.minimum) {
      errors.push({
        path,
        keyword: 'minimum',
        message: `${path || '/'} is ${value}, below the minimum of ${schema.minimum}`,
      })
    }
    if (typeof schema.maximum === 'number' && value > schema.maximum) {
      errors.push({
        path,
        keyword: 'maximum',
        message: `${path || '/'} is ${value}, above the maximum of ${schema.maximum}`,
      })
    }
    if (typeof schema.exclusiveMinimum === 'number' && value <= schema.exclusiveMinimum) {
      errors.push({
        path,
        keyword: 'exclusiveMinimum',
        message: `${path || '/'} is ${value}, which must be greater than ${schema.exclusiveMinimum}`,
      })
    }
  }

  if (Array.isArray(value)) {
    if (typeof schema.minItems === 'number' && value.length < schema.minItems) {
      errors.push({
        path,
        keyword: 'minItems',
        message: `${path || '/'} has ${value.length} items, below the minimum of ${schema.minItems}`,
      })
    }
    if (typeof schema.maxItems === 'number' && value.length > schema.maxItems) {
      errors.push({
        path,
        keyword: 'maxItems',
        message: `${path || '/'} has ${value.length} items, above the maximum of ${schema.maxItems}`,
      })
    }
    if (schema.items !== undefined) {
      value.forEach((item, index) => {
        check(schema.items as Schema, item, `${path}/${index}`, root, errors)
      })
    }
  }

  if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
    const object = value as Record<string, unknown>
    const properties = (schema.properties as Record<string, Schema> | undefined) ?? {}

    for (const name of (schema.required as string[] | undefined) ?? []) {
      if (!(name in object)) {
        errors.push({
          path,
          keyword: 'required',
          message: `${path || '/'} is missing the required member \`${name}\``,
        })
      }
    }

    if (schema.additionalProperties === false) {
      for (const name of Object.keys(object)) {
        if (!(name in properties)) {
          errors.push({
            path,
            keyword: 'additionalProperties',
            message: `${path || '/'} carries the unknown member \`${name}\`; this document forbids additional properties`,
          })
        }
      }
    }

    for (const [name, subSchema] of Object.entries(properties)) {
      if (name in object) {
        check(subSchema, object[name], `${path}/${name}`, root, errors)
      }
    }
  }

  if (schema.not !== undefined && conforms(schema.not as Schema, value, root)) {
    errors.push({
      path,
      keyword: 'not',
      message: `${path || '/'} matches a shape this document forbids`,
    })
  }

  // `if` selects a branch; only the selected branch's failures are errors. The
  // `if` sub-schema's own failures never are — that is what distinguishes it
  // from `allOf`.
  if (schema.if !== undefined) {
    const branch = conforms(schema.if as Schema, value, root) ? schema.then : schema.else
    if (branch !== undefined) {
      check(branch as Schema, value, path, root, errors)
    }
  }

  for (const branch of (schema.allOf as Schema[] | undefined) ?? []) {
    check(branch, value, path, root, errors)
  }
}
