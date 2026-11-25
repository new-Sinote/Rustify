const port = process.argv[2] || "9223";
const url = process.argv[3] || "https://open.spotify.com/";
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
await call('Page.navigate', { url });
console.log('navigated');
ws.close();
