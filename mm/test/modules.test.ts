//
// The layout itself, as assertions.
//
// `src/` was flat until the module folders landed, and a folder structure is
// only worth the churn if something notices when it stops being true. Three
// properties are load-bearing and every one of them is invisible when broken --
// nothing throws, no picture changes, the gate stays green:
//
//   * the module graph is a DAG. A cycle between folders is what you get by
//     putting the editor's own panels next to `gui.ts` because they are "both
//     panels": `inspector` imports `editops`, `interaction` imports `inspector`.
//     As files that is fine; across two folders it is a cycle, and the layout
//     stops being a layering.
//
//   * the THREE-free files stay THREE-free. `imports.smoke.test.ts` imports
//     them, but importing a module that pulls THREE in *succeeds* headless, so
//     that spec cannot catch a regression here -- only the page bundle would
//     notice, by silently growing. This is a static check on the import graph
//     instead: it is the only thing standing between `colormap.ts` and a
//     `new THREE.Vector3()` someone adds for convenience.
//
//   * the entry points stay at `src/` root. Four HTML files name them by path
//     (`/src/main.ts`, `./src/gpt2page.ts`), and no typecheck reads an HTML
//     file -- moving one is caught by `npm run build` or by nobody.
//
// It reads the import graph off disk rather than importing anything, so it
// stays true for modules a jsdom test cannot construct.
//
import { describe, it, expect } from 'vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SRC = join(ROOT, 'src')

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) return walk(p)
    return p.endsWith('.ts') ? [p] : []
  })
}

const FILES = walk(SRC).sort()

// All three forms mm uses: `from '...'`, the dynamic `import('...')` that
// imports.smoke.test.ts is built from, and the side-effect `import '...'` --
// which is not a curiosity here, it is how `gpt2page.ts` pulls in its CSS and
// it is exactly the form a stray dependency would arrive in. Leaving it out
// made this file's cycle check silently blind, which a mutation caught.
//
// Bare specifiers ('three', 'lil-gui') come through as-is; relative ones carry
// the `.js` extension the emitted code would use, which is the `.ts` file on
// disk.
const SPECIFIER = /\b(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g

function importsOf(file: string): string[] {
  const src = readFileSync(file, 'utf8')
  return [...src.matchAll(SPECIFIER)].map(m => m[1])
}

/** Relative imports that land inside src/, resolved to the .ts file on disk. */
function localDeps(file: string): string[] {
  return importsOf(file)
    .filter(s => s.startsWith('.'))
    .map(s => resolve(dirname(file), s.replace(/\.js$/, '.ts')))
    .filter(p => p.startsWith(SRC + '/') && p.endsWith('.ts'))
}

/** 'editor/selection.ts' -> 'editor'; 'main.ts' -> '(entry)'. */
function moduleOf(file: string): string {
  const rel = relative(SRC, file)
  const parts = rel.split('/')
  return parts.length > 1 ? parts[0] : '(entry)'
}

describe('the src/ module layout', () => {
  it('keeps the two entry points at src/ root, where the HTML files name them', () => {
    // viewer/index.html loads /src/main.ts; index.html, attngpt2/ and attnqkov/
    // load src/gpt2page.ts. A third file appearing here is a library module
    // that missed its folder; a missing one is four broken pages.
    const atRoot = FILES.filter(f => moduleOf(f) === '(entry)')
      .map(f => relative(SRC, f))
      .sort()
    expect(atRoot).toEqual(['gpt2page.ts', 'main.ts'])
  })

  it('puts every other file in exactly one module folder', () => {
    const modules = [...new Set(FILES.map(moduleOf))].sort()
    expect(modules).toEqual(['(entry)', 'common', 'editor', 'gui', 'render', 'scene'])

    // One level deep, always: `src/editor/foo.ts`, never `src/editor/bar/foo.ts`.
    // Nesting is how a module quietly becomes two.
    for (const f of FILES) {
      expect(relative(SRC, f).split('/').length).toBeLessThanOrEqual(2)
    }
  })

  it('has an acyclic module graph', () => {
    // Edges between *folders*, self-edges dropped -- a file importing its
    // neighbour is the normal case and says nothing about the layering.
    const edges = new Map<string, Set<string>>()
    for (const f of FILES) {
      const from = moduleOf(f)
      if (!edges.has(from)) edges.set(from, new Set())
      for (const dep of localDeps(f)) {
        const to = moduleOf(dep)
        if (to !== from) edges.get(from)!.add(to)
      }
    }

    // Depth-first, reporting the cycle rather than just a boolean -- the whole
    // value of this spec is naming which two folders closed the loop.
    const state = new Map<string, 'open' | 'done'>()
    const cycles: string[] = []
    const visit = (m: string, path: string[]) => {
      if (state.get(m) === 'done') return
      if (state.get(m) === 'open') {
        cycles.push([...path.slice(path.indexOf(m)), m].join(' -> '))
        return
      }
      state.set(m, 'open')
      for (const next of edges.get(m) ?? []) visit(next, [...path, m])
      state.set(m, 'done')
    }
    for (const m of edges.keys()) visit(m, [])

    expect(cycles).toEqual([])
  })

  it('keeps the entry points out of every module\'s import graph', () => {
    // main.ts and gpt2page.ts may import any module; nothing may import them.
    // A module reaching back into an entry is how `main.ts`'s renderer-at-import
    // ends up in a test's module graph, and every downstream spec needs a GPU.
    const importedEntries = FILES.flatMap(f =>
      localDeps(f)
        .filter(d => moduleOf(d) === '(entry)')
        .map(d => `${relative(SRC, f)} -> ${relative(SRC, d)}`),
    )
    expect(importedEntries).toEqual([])
  })
})

describe('the THREE-free half of the codebase', () => {
  // The files whose reason for existing is that a jsdom test can reach every
  // decision they make. The first five say so in their own doc comments; this
  // is that comment made mechanical. `heatmap.ts` is the sharpest case -- it
  // decides which cell is where and which value a texel carries, and if it
  // moved into a shader no test could see it at all. `params.ts` claims no
  // such thing, it is plain data that has simply never needed THREE; it is
  // listed so that stays true, since it is what `gui` and `interaction` build
  // their trees from.
  const PURE = [
    'render/colormap.ts',
    'render/heatmap.ts',
    'editor/address.ts',
    'editor/scenetree.ts',
    'editor/selection.ts',
    'scene/params.ts',
  ]

  /** Everything reachable from `entry` through relative imports, inclusive. */
  function closure(entry: string): string[] {
    const seen = new Set<string>()
    const stack = [entry]
    while (stack.length) {
      const f = stack.pop()!
      if (seen.has(f)) continue
      seen.add(f)
      stack.push(...localDeps(f))
    }
    return [...seen]
  }

  for (const rel of PURE) {
    it(`${rel} pulls in no THREE, directly or transitively`, () => {
      const offenders = closure(join(SRC, rel))
        .filter(f => importsOf(f).some(s => s === 'three' || s.startsWith('three/')))
        .map(f => relative(SRC, f))
        .sort()
      expect(offenders).toEqual([])
    })
  }
})

describe('the test/ layout mirrors src/', () => {
  const TEST = join(ROOT, 'test')

  it('gives every src module folder a test folder of the same name', () => {
    const srcModules = [...new Set(FILES.map(moduleOf))].filter(m => m !== '(entry)').sort()
    const testDirs = readdirSync(TEST)
      .filter(n => statSync(join(TEST, n)).isDirectory())
      .sort()
    expect(testDirs).toEqual(srcModules)
  })

  it('has each suite import the module it sits under', () => {
    // The mirror is the point: test/editor/*.test.ts must be about src/editor.
    // A suite that drifted into the wrong folder still passes -- only this
    // notices. Cross-module imports are allowed (a selection test needs viz to
    // build a tree); what is checked is that the suite touches its *own*
    // module at all.
    const strays: string[] = []
    for (const dir of readdirSync(TEST)) {
      const p = join(TEST, dir)
      if (!statSync(p).isDirectory()) continue
      for (const name of readdirSync(p).filter(n => n.endsWith('.test.ts'))) {
        const deps = importsOf(join(p, name)).filter(s => s.includes('/src/'))
        if (!deps.some(s => s.includes(`/src/${dir}/`))) strays.push(`${dir}/${name}`)
      }
    }
    expect(strays).toEqual([])
  })

  it('leaves the cross-cutting suites at test/ root', () => {
    // setup.ts is vitest's, imports.smoke covers every module at once, and
    // gpt2page.test.ts mirrors an entry, which has no folder. Filing any of
    // them under a module would be filing them under the wrong one.
    const atRoot = readdirSync(TEST)
      .filter(n => n.endsWith('.ts'))
      .sort()
    expect(atRoot).toEqual([
      'gpt2page.test.ts',
      'imports.smoke.test.ts',
      'modules.test.ts',
      'setup.ts',
    ])
  })
})
