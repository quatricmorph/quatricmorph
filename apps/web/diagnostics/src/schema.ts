/**
 * The published diagnostics manifest schema.
 *
 * Imported from `schemas/diagnostics/manifest.v1.json` at the repository root
 * rather than copied into this package. `QM-0140` wrote that schema and the
 * `q_report::Manifest` serde types together precisely so that one document
 * governs both sides; a copy here would be a third, and a third drifts.
 *
 * `src/__tests__/boundary.test.ts` asserts that no copy exists in this package,
 * and `src/__tests__/schema-validator.test.ts` asserts that this import equals
 * the bytes on disk.
 */

import schema from '../../../../schemas/diagnostics/manifest.v1.json'

/** The draft-07 schema every manifest this surface renders must satisfy. */
export const MANIFEST_SCHEMA_V1: Readonly<Record<string, unknown>> = schema as Record<string, unknown>

/**
 * The only manifest version this build reads.
 *
 * `CAT-002`'s rule: a reader that guesses at an unknown version produces a
 * plausible wrong answer, so it refuses and names both versions instead.
 */
export const SUPPORTED_MANIFEST_VERSION = 1
