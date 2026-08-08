//
// jsdom gaps that the app's dependencies probe for.
//
// Nothing here changes app behaviour -- each entry is a browser API jsdom does
// not implement, stubbed at its documented default so the code under test takes
// the same branch a desktop browser would.
//
import { vi } from 'vitest'

// lil-gui asks `(pointer: coarse)` to decide whether a number field gets touch
// drag handling. jsdom has no matchMedia at all, so the query throws before the
// answer matters. Answering `false` is the desktop branch, which is the one the
// panel is designed around.
if (!window.matchMedia) {
  window.matchMedia = vi.fn(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),        // deprecated, still called by older libs
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }))
}
