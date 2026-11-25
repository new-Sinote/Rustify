const port = process.argv[2] || "9223";
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
await call('Page.bringToFront');
await new Promise(r => setTimeout(r, 1500));
const st = await call('Runtime.evaluate', { expression: `({vis: document.visibilityState, hidden: document.hidden, focus: document.hasFocus(), size: [innerWidth, innerHeight]})`, returnByValue: true });
console.log(JSON.stringify(st.result.value));
ws.close();
