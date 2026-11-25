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
const clicked = await call('Runtime.evaluate', { expression: `(() => { const b = document.querySelector('[data-testid="play-button"]'); if (!b) return 'no_button'; b.click(); return 'clicked'; })()`, returnByValue: true, userGesture: true });
console.log('play_button=' + clicked.result.value);
await new Promise(r => setTimeout(r, 9000));
const state = await call('Runtime.evaluate', { expression: `({label: document.querySelector('[data-testid="control-button-playpause"]')?.getAttribute('aria-label'), bar: document.querySelector('[data-testid="now-playing-bar"]')?.innerText.slice(0,200), href: location.href})`, returnByValue: true, userGesture: true });
console.log(JSON.stringify(state.result.value, null, 2));
ws.close();
