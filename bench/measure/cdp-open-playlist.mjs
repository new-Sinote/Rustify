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
const links = await call('Runtime.evaluate', { expression: `Array.from(document.querySelectorAll('a[href^="/playlist/"]')).map(a => ({href: a.getAttribute('href'), text: a.innerText.split('\\n')[0]})).slice(0, 12)`, returnByValue: true });
console.log(JSON.stringify(links.result.value, null, 2));
const target = links.result.value.find(l => /Never Recovered/i.test(l.text)) || links.result.value[0];
if (!target) { console.log('no_playlist_link'); process.exit(1); }
await call('Runtime.evaluate', { expression: `document.querySelector('a[href="${target.href}"]').click(); 'opened'`, returnByValue: true });
await new Promise(r => setTimeout(r, 6000));
const state = await call('Runtime.evaluate', { expression: `({href: location.href, bigPlay: !!document.querySelector('[data-testid="play-button"]'), label: document.querySelector('[data-testid="control-button-playpause"]')?.getAttribute('aria-label')})`, returnByValue: true });
console.log(JSON.stringify(state.result.value, null, 2));
ws.close();
