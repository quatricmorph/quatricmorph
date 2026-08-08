//
// Declarations for the non-code files src/ imports.
//
// CSS side-effect imports (`import './gpt2page.css'`) are covered by
// "vite/client" in tsconfig's `types`. What is not covered is the Three.js
// typeface in assets/, which is a hand-generated .js module with no typings and
// no package to attach them to.
//

// assets/katex_main_regular.typeface.js and its sibling droid_sans -- produced
// by assets/fonts/ttf2typeface.py, consumed by FontLoader.parse(). The shape is
// whatever that script emits; FontLoader takes it as-is, so `unknown` here would
// only force a cast at the one call site that already knows what it has.
declare module '*.typeface.js' {
  export const data: any
}
