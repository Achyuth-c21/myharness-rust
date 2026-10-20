#!/usr/bin/env node
/*
 * Minimal Chromium DevTools Protocol client for the Claude Desktop renderer.
 * Requires Node 22+ (global WebSocket). No third-party dependencies.
 *
 * Usage:
 *   node cdp.mjs list
 *   node cdp.mjs eval --file /tmp/expr.js
 *   node cdp.mjs eval "document.title"
 *   node cdp.mjs screenshot /path/out.png
 *   node cdp.mjs ax /path/ax.json
 *   node cdp.mjs html /path/page.html
 *
 * Options:
 *   --target <substring>   pick a page target by URL substring (default app://)
 *   --timeout <ms>         command timeout (default 30000)
 */
import fs from 'node:fs';

const argv = process.argv.slice(2);
const command = argv.shift();
const opt = { target: 'app://', timeout: 30000 };
const rest = [];
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === '--target') opt.target = argv[++i];
  else if (a === '--timeout') opt.timeout = Number(argv[++i]);
  else if (a === '--file') {
    opt.file = argv[++i];
  } else if (a === '--text') opt.text = argv[++i];
  else if (a === '--aria') opt.aria = argv[++i];
  else if (a === '--testid') opt.testid = argv[++i];
  else if (a === '--selector') opt.selector = argv[++i];
  else if (a === '--index') opt.index = Number(argv[++i]);
  else if (a === '--value') opt.value = argv[++i];
  else rest.push(a);
}

const CDP = process.env.CDP_URL || 'http://127.0.0.1:9222';

function listTargets() {
  return fetch(`${CDP}/json/list`).then((r) => r.json());
}

function pickTarget(targets) {
  const pages = targets.filter((t) => t.type === 'page');
  return (
    pages.find((t) => t.url.includes(opt.target)) ||
    pages.find((t) => t.url.startsWith('app://')) ||
    pages[0]
  );
}

async function connect() {
  const targets = await listTargets();
  if (command === 'list') {
    console.log(JSON.stringify(targets, null, 2));
    process.exit(0);
  }
  const target = pickTarget(targets);
  if (!target) throw new Error('no page target');
  console.error(`[cdp] target ${target.id} ${target.url}`);
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('WebSocket open timeout')), opt.timeout);
    ws.onopen = () => { clearTimeout(timer); resolve(); };
    ws.onerror = (e) => { clearTimeout(timer); reject(new Error('WebSocket error')); };
  });
  let nextId = 1;
  const pending = new Map();
  ws.onmessage = (event) => {
    let msg;
    try { msg = JSON.parse(event.data); } catch { return; }
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject, timer } = pending.get(msg.id);
      pending.delete(msg.id);
      clearTimeout(timer);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result);
    }
  };
  function send(method, params = {}) {
    const id = nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`CDP timeout: ${method}`));
      }, opt.timeout);
      pending.set(id, { resolve, reject, timer });
      ws.send(JSON.stringify({ id, method, params }));
    });
  }
  const close = () => { try { ws.close(); } catch {} };
  return { send, close, target };
}

async function main() {
  const { send, close, target } = await connect();
  try {
    await send('Runtime.enable');
    await send('Page.enable');

    if (command === 'eval') {
      let expression = opt.file ? fs.readFileSync(opt.file, 'utf8') : rest.join(' ');
      if (!expression.trim()) throw new Error('empty expression');
      const res = await send('Runtime.evaluate', {
        expression,
        awaitPromise: true,
        returnByValue: true,
        userGesture: true,
      });
      if (res.exceptionDetails) {
        console.error('EXCEPTION');
        console.error(JSON.stringify(res.exceptionDetails, null, 2));
        process.exitCode = 2;
      } else {
        const v = res.result?.value;
        console.log(typeof v === 'string' ? v : JSON.stringify(v, null, 2));
      }
    } else if (command === 'screenshot') {
      const out = rest[0] || '/tmp/opencode/cdp-screenshot.png';
      const res = await send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(out, Buffer.from(res.data, 'base64'));
      console.log(out);
    } else if (command === 'ax') {
      const out = rest[0] || '/tmp/opencode/ax.json';
      const res = await send('Accessibility.getFullAXTree');
      fs.writeFileSync(out, JSON.stringify(res, null, 2));
      console.log(out);
    } else if (command === 'html') {
      const out = rest[0] || '/tmp/opencode/page.html';
      const res = await send('Runtime.evaluate', {
        expression: 'document.documentElement.outerHTML',
        returnByValue: true,
      });
      fs.writeFileSync(out, res.result?.value || '');
      console.log(out);
    } else if (command === 'click') {
      const spec = { text: opt.text, aria: opt.aria, testid: opt.testid, selector: opt.selector, index: opt.index ?? 0 };
      const expression = `(() => {
        const spec = ${JSON.stringify(spec)};
        const cands = spec.selector
          ? [...document.querySelectorAll(spec.selector)]
          : [...document.querySelectorAll('button,a,[role="button"],[role="menuitem"],[role="tab"],[role="option"],label')];
        const norm = (s) => (s || '').replace(/\\s+/g, ' ').trim();
        let matches = cands.filter((el) => {
          if (spec.testid && el.getAttribute('data-testid') !== spec.testid) return false;
          if (spec.aria && el.getAttribute('aria-label') !== spec.aria) return false;
          if (spec.text && norm(el.innerText) !== norm(spec.text)) return false;
          return true;
        });
        if (!matches.length && spec.text) matches = cands.filter((el) => norm(el.innerText).includes(norm(spec.text)));
        const el = matches[spec.index];
        if (!el) return { ok: false, count: matches.length, candidates: cands.slice(0, 40).map(e => norm(e.innerText).slice(0, 80)) };
        el.scrollIntoView({ block: 'center', inline: 'nearest' });
        const r = el.getBoundingClientRect();
        const info = { tag: el.tagName, text: norm(el.innerText).slice(0, 300), aria: el.getAttribute('aria-label'), testid: el.getAttribute('data-testid'), rect: { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) } };
        el.click();
        return { ok: true, clicked: info, count: matches.length };
      })()`;
      const res = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true, userGesture: true });
      console.log(JSON.stringify(res.result?.value ?? res, null, 2));
    } else if (command === 'text') {
      const res = await send('Runtime.evaluate', { expression: 'document.body.innerText', returnByValue: true });
      console.log(res.result?.value || '');
    } else if (command === 'type') {
      const selector = opt.selector || 'textarea, [contenteditable="true"], input, [role="textbox"]';
      const focus = await send('Runtime.evaluate', { expression: `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return {ok:false}; el.focus(); return {ok:true, tag:el.tagName}; })()`, returnByValue: true });
      if (!focus.result?.value?.ok) {
        console.log(JSON.stringify(focus.result?.value || focus));
      } else {
        await send('Input.insertText', { text: opt.value ?? '' });
        console.log(JSON.stringify({ ok: true, typed: (opt.value ?? '').length, selector }));
      }
    } else if (command === 'key') {
      const key = rest[0] || opt.value || 'Enter';
      const vk = key === 'Enter' ? 13 : key === 'Escape' ? 27 : 0;
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key, code: key, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
      if (key.length === 1) await send('Input.insertText', { text: key });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key, code: key, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
      console.log(JSON.stringify({ ok: true, key }));
    } else if (command === 'metrics') {
      const res = await send('Page.getLayoutMetrics');
      console.log(JSON.stringify(res, null, 2));
    } else {
      throw new Error(`unknown command: ${command}`);
    }
  } finally {
    close();
  }
}

main().catch((e) => {
  console.error(`[cdp] ${e.stack || e}`);
  process.exit(1);
});
