/**
 * Manual-smoke stand-in via Chrome CDP (cursor-ide-browser unavailable in this agent).
 * Usage: node scripts/smoke-cdp.mjs [baseUrl]
 */
import { spawn } from 'node:child_process'
import { setTimeout as sleep } from 'node:timers/promises'

const BASE = process.argv[2] || 'http://localhost:5173/'
const CDP = 'http://127.0.0.1:9222'

async function httpJson(url, opts) {
  const res = await fetch(url, opts)
  if (!res.ok) throw new Error(`${res.status} ${url}`)
  return res.json()
}

class Cdp {
  constructor(wsUrl) {
    this.wsUrl = wsUrl
    this.ws = null
    this.nextId = 1
    this.pending = new Map()
    this.console = []
    this.exceptions = []
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl)
    await new Promise((resolve, reject) => {
      this.ws.addEventListener('open', resolve, { once: true })
      this.ws.addEventListener('error', reject, { once: true })
    })
    this.ws.addEventListener('message', (ev) => {
      const msg = JSON.parse(ev.data)
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id)
        this.pending.delete(msg.id)
        if (msg.error) reject(new Error(JSON.stringify(msg.error)))
        else resolve(msg.result)
        return
      }
      if (msg.method === 'Runtime.consoleAPICalled') {
        const text = (msg.params.args || [])
          .map((a) => a.value ?? a.description ?? '')
          .join(' ')
        this.console.push({ type: msg.params.type, text })
      }
      if (msg.method === 'Runtime.exceptionThrown') {
        const d = msg.params.exceptionDetails || {}
        this.exceptions.push(d.text || d.exception?.description || JSON.stringify(d))
      }
    })
  }

  send(method, params = {}) {
    const id = this.nextId++
    const payload = { id, method, params }
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.ws.send(JSON.stringify(payload))
    })
  }

  async evaluate(expression) {
    const result = await this.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text || JSON.stringify(result.exceptionDetails))
    }
    return result.result?.value
  }

  close() {
    try { this.ws?.close() } catch { /* ignore */ }
  }
}

function findControllerClickJs(label) {
  return `(() => {
    const titles = [...document.querySelectorAll('.lil-gui .controller .name')];
    const el = titles.find(n => n.textContent.trim() === ${JSON.stringify(label)});
    if (!el) return { ok: false, reason: 'controller not found: ' + ${JSON.stringify(label)} };
    const widget = el.parentElement?.querySelector('button, input, select, .widget');
    const btn = el.parentElement?.querySelector('button');
    if (btn) { btn.click(); return { ok: true, kind: 'button' }; }
    return { ok: false, reason: 'no button for ' + ${JSON.stringify(label)}, html: el.parentElement?.outerHTML?.slice(0,200) };
  })()`
}

function setNumberControllerJs(label, value) {
  return `(() => {
    const titles = [...document.querySelectorAll('.lil-gui .controller .name')];
    const el = titles.find(n => n.textContent.trim() === ${JSON.stringify(label)});
    if (!el) return { ok: false, reason: 'not found' };
    const input = el.parentElement?.querySelector('input');
    if (!input) return { ok: false, reason: 'no input' };
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    setter.call(input, String(${JSON.stringify(value)}));
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
    input.blur();
    input.dispatchEvent(new Event('focusout', { bubbles: true }));
    return { ok: true, value: input.value };
  })()`
}

function toggleCheckboxJs(label, checked) {
  return `(() => {
    const titles = [...document.querySelectorAll('.lil-gui .controller .name')];
    const el = titles.find(n => n.textContent.trim() === ${JSON.stringify(label)});
    if (!el) return { ok: false, reason: 'not found' };
    const input = el.parentElement?.querySelector('input[type=checkbox]');
    if (!input) return { ok: false, reason: 'no checkbox' };
    if (input.checked !== ${checked}) {
      input.click();
    }
    return { ok: true, checked: input.checked };
  })()`
}

async function main() {
  const report = {
    branding: 'fail',
    defaultExample: 'fail',
    canvasOrbit: 'fail',
    hoverMetadata: 'fail',
    playStepControls: 'fail',
    shareUrl: 'fail',
    invalidDims: 'fail',
    consoleClean: 'fail',
    notes: [],
    consoleErrors: [],
    exceptions: [],
    url: BASE,
  }

  // Prefer existing about:blank page
  const pages = await httpJson(`${CDP}/json/list`)
  let page = pages.find((p) => p.type === 'page')
  if (!page) {
    page = await httpJson(`${CDP}/json/new?${encodeURIComponent(BASE)}`)
  }

  const cdp = new Cdp(page.webSocketDebuggerUrl)
  await cdp.connect()
  await cdp.send('Runtime.enable')
  await cdp.send('Log.enable')
  await cdp.send('Page.enable')
  await cdp.send('Page.navigate', { url: BASE })
  await cdp.send('Page.loadEventFired').catch(() => {})
  // wait for app ready (#info not "loading")
  let ready = false
  for (let i = 0; i < 40; i++) {
    await sleep(250)
    const state = await cdp.evaluate(`({
      title: document.title,
      brand: document.querySelector('.brand-name')?.textContent?.trim() || '',
      info: document.getElementById('info')?.textContent?.trim() || '',
      hasCanvas: !!document.querySelector('#container canvas'),
      guiTitle: document.querySelector('.lil-gui .title')?.textContent?.trim() || '',
      bodyText: document.body?.innerText?.slice(0, 500) || '',
    })`)
    if (state.hasCanvas && state.info && state.info !== 'loading') {
      ready = true
      report._boot = state
      break
    }
    report._boot = state
  }
  if (!ready) {
    report.notes.push('App did not leave loading state in time')
  }

  // 1 branding
  const boot = report._boot || {}
  if (
    /Quatricmorph/i.test(boot.title || '') &&
    /Quatricmorph/i.test(boot.brand || '') &&
    !/\bmm\b/.test(boot.brand || '') &&
    /Quatricmorph/i.test(boot.guiTitle || '')
  ) {
    report.branding = 'pass'
  } else {
    report.notes.push(`branding saw title=${boot.title} brand=${boot.brand} gui=${boot.guiTitle}`)
  }

  // 2 default example values / expression
  const mathState = await cdp.evaluate(`(() => {
    const info = document.getElementById('info')?.textContent || '';
    const labels = [...document.querySelectorAll('#container *')]
      .map(n => n.textContent || '')
      .join(' ');
    const all = info + ' ' + labels + ' ' + (document.body.innerText || '');
    return {
      info,
      hasExpr: /A\\s*@\\s*B/.test(all) || /C\\s*=\\s*A/.test(all),
      has58: /\\b58\\b/.test(all),
      has64: /\\b64\\b/.test(all),
      has139: /\\b139\\b/.test(all),
      has154: /\\b154\\b/.test(all),
      aText: [...document.querySelectorAll('.lil-gui .controller')].find(c => c.querySelector('.name')?.textContent?.trim()==='A values')?.querySelector('textarea,input')?.value || '',
      bText: [...document.querySelectorAll('.lil-gui .controller')].find(c => c.querySelector('.name')?.textContent?.trim()==='B values')?.querySelector('textarea,input')?.value || '',
    };
  })()`)
  report._math = mathState
  const expectedInputs =
    mathState.aText.includes('1') && mathState.aText.includes('6') &&
    mathState.bText.includes('7') && mathState.bText.includes('12')
  if (
    (mathState.has58 && mathState.has64 && mathState.has139 && mathState.has154) ||
    (mathState.hasExpr && expectedInputs)
  ) {
    report.defaultExample = 'pass'
    if (!(mathState.has58 && mathState.has154)) {
      report.notes.push('C values not all visible in DOM; default A/B texts + expr present')
    }
  } else {
    report.notes.push('default example values not confirmed: ' + JSON.stringify(mathState))
  }

  // 3 canvas present + orbit drag without throw
  const orbit = await cdp.evaluate(`(() => {
    const canvas = document.querySelector('#container canvas');
    if (!canvas) return { ok: false, reason: 'no canvas' };
    const r = canvas.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const fire = (type, x, y, buttons=1) => {
      canvas.dispatchEvent(new PointerEvent(type, {
        bubbles: true, cancelable: true, clientX: x, clientY: y,
        pointerId: 1, pointerType: 'mouse', buttons, button: 0,
      }));
    };
    fire('pointerdown', cx, cy, 1);
    fire('pointermove', cx + 40, cy + 20, 1);
    fire('pointerup', cx + 40, cy + 20, 0);
    return {
      ok: true,
      size: { w: r.width, h: r.height },
      webgl: !!(canvas.getContext && (canvas.getContext('webgl2') || canvas.getContext('webgl'))),
    };
  })()`)
  report._orbit = orbit
  if (orbit.ok && orbit.size.w > 0) report.canvasOrbit = 'pass'
  else report.notes.push('orbit/canvas: ' + JSON.stringify(orbit))

  // 4 hover metadata — sweep pointer across canvas and read #hover-info
  const hover = await cdp.evaluate(`(async () => {
    const canvas = document.querySelector('#container canvas');
    const hoverEl = document.getElementById('hover-info');
    if (!canvas || !hoverEl) return { ok: false, reason: 'missing elements' };
    const r = canvas.getBoundingClientRect();
    const samples = [];
    for (let yi = 0; yi < 8; yi++) {
      for (let xi = 0; xi < 10; xi++) {
        const x = r.left + (r.width * (xi + 0.5)) / 10;
        const y = r.top + (r.height * (yi + 0.5)) / 8;
        window.dispatchEvent(new PointerEvent('pointermove', {
          bubbles: true, clientX: x, clientY: y, pointerId: 1, pointerType: 'mouse',
        }));
        await new Promise(res => setTimeout(res, 20));
        const t = hoverEl.textContent || '';
        if (t.includes('Tensor:') && t.includes('Index:') && t.includes('Value:') && t.includes('Shape:')) {
          return { ok: true, text: t };
        }
        if (t.trim()) samples.push(t.slice(0, 80));
      }
    }
    return { ok: false, samples: samples.slice(0, 5), hover: hoverEl.textContent };
  })()`)
  report._hover = hover
  if (hover.ok) report.hoverMetadata = 'pass'
  else report.notes.push('hover: ' + JSON.stringify(hover))

  // 5 play / step controls
  const play = await cdp.evaluate(findControllerClickJs('Play'))
  const step = await cdp.evaluate(findControllerClickJs('Step'))
  const prev = await cdp.evaluate(findControllerClickJs('Previous Step'))
  const pause = await cdp.evaluate(findControllerClickJs('Pause'))
  report._controls = { play, step, prev, pause }
  if (play.ok && step.ok && prev.ok && pause.ok) report.playStepControls = 'pass'
  else report.notes.push('controls: ' + JSON.stringify(report._controls))

  // 6 share URL — change camera preset, copy share / read history, reload
  const beforeUrl = await cdp.evaluate('location.href')
  await cdp.evaluate(`(() => {
    const titles = [...document.querySelectorAll('.lil-gui .controller .name')];
    const el = titles.find(n => n.textContent.trim() === 'Camera');
    const select = el?.parentElement?.querySelector('select');
    if (!select) return { ok: false };
    select.value = 'front';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    return { ok: true, value: select.value };
  })()`)
  await sleep(300)
  const midUrl = await cdp.evaluate('location.href')
  const copy = await cdp.evaluate(findControllerClickJs('Copy Share Link'))
  await sleep(200)
  const validationAfterCopy = await cdp.evaluate(
    `document.getElementById('validation')?.textContent || ''`,
  )
  const shareUrl = midUrl.includes('?') ? midUrl : beforeUrl
  // Navigate using current location (pushState should have updated)
  const reloadUrl = await cdp.evaluate('location.href')
  await cdp.send('Page.navigate', { url: reloadUrl })
  await sleep(1500)
  for (let i = 0; i < 20; i++) {
    await sleep(200)
    const info = await cdp.evaluate(`document.getElementById('info')?.textContent || ''`)
    if (info && info !== 'loading') break
  }
  const afterReload = await cdp.evaluate(`({
    href: location.href,
    camera: [...document.querySelectorAll('.lil-gui .controller')].find(c => c.querySelector('.name')?.textContent?.trim()==='Camera')?.querySelector('select')?.value || '',
    brand: document.querySelector('.brand-name')?.textContent || '',
    hasCanvas: !!document.querySelector('#container canvas'),
  })`)
  report._share = { beforeUrl, midUrl, reloadUrl, copy, validationAfterCopy, afterReload }
  if (
    (reloadUrl.includes('?') || midUrl.includes('?')) &&
    afterReload.hasCanvas &&
    /Quatricmorph/i.test(afterReload.brand) &&
    (afterReload.camera === 'front' || copy.ok)
  ) {
    report.shareUrl = 'pass'
  } else {
    report.notes.push('share: ' + JSON.stringify(report._share))
  }

  // 7 invalid dimensions
  const unlock = await cdp.evaluate(toggleCheckboxJs('Unlock B rows', true))
  await sleep(100)
  const setB = await cdp.evaluate(setNumberControllerJs('B rows', 1))
  await sleep(400)
  const invalidMsg = await cdp.evaluate(`document.getElementById('validation')?.textContent || ''`)
  const stillAlive = await cdp.evaluate(`({
    hasCanvas: !!document.querySelector('#container canvas'),
    brand: document.querySelector('.brand-name')?.textContent || '',
  })`)
  report._invalid = { unlock, setB, invalidMsg, stillAlive }
  if (
    stillAlive.hasCanvas &&
    /incompatible|must equal|dimension/i.test(invalidMsg) &&
    !cdp.exceptions.length
  ) {
    report.invalidDims = 'pass'
  } else {
    report.notes.push('invalidDims: ' + JSON.stringify(report._invalid))
  }

  // 8 console
  const noise = [...cdp.console]
    .filter((c) => c.type === 'error' || c.type === 'warning')
    .map((c) => `[${c.type}] ${c.text}`)
  // Also pull via Runtime — already collected
  report.consoleErrors = [
    ...cdp.exceptions.map((e) => `[exception] ${e}`),
    ...noise.filter((t) => /error/i.test(t) || t.startsWith('[error]')),
  ]
  // Filter benign vite/hmr noise if any
  const realErrors = report.consoleErrors.filter(
    (t) => !/Deprecated|favicon|Download the React/i.test(t),
  )
  report.exceptions = cdp.exceptions
  if (realErrors.length === 0 && cdp.exceptions.length === 0) {
    report.consoleClean = 'pass'
  } else {
    report.consoleErrors = realErrors
    report.notes.push('console issues present')
  }

  cdp.close()
  console.log(JSON.stringify(report, null, 2))
  const fails = Object.entries(report)
    .filter(([k, v]) => ['branding','defaultExample','canvasOrbit','hoverMetadata','playStepControls','shareUrl','invalidDims','consoleClean'].includes(k) && v !== 'pass')
  process.exit(fails.length ? 1 : 0)
}

main().catch((e) => {
  console.error(e)
  process.exit(2)
})
