const port = process.argv[2] || "9222";
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find(t => t.type === 'page' && /open\.spotify\.com/.test(t.url));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let id = 1;
const call = (method, params = {}) => new Promise((res, rej) => {
  const my = id++;
  const onMsg = (e) => {
    const m = JSON.parse(e.data);
    if (m.id === my) { ws.removeEventListener('message', onMsg); m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result); }
  };
  ws.addEventListener('message', onMsg);
  ws.send(JSON.stringify({ id: my, method, params }));
});
const expr = `(() => {
  const q = (s) => document.querySelector(s);
  const btn = q('[data-testid="control-button-playpause"]');
  const widget = q('[data-testid="now-playing-widget"]');
  const bar = q('[data-testid="now-playing-bar"]');
  const ad = q('[data-testid="advertisement"]');
  return {
    playpause_label: btn ? btn.getAttribute('aria-label') : null,
    widget_text: widget ? widget.innerText : null,
    bar_text: bar ? bar.innerText.slice(0, 300) : null,
    ad_el: !!ad,
    visibility: document.visibilityState,
    href: location.href
  };
})()`;
const r = await call('Runtime.evaluate', { expression: expr, returnByValue: true });
console.log(JSON.stringify(r.result.value, null, 2));
ws.close();
