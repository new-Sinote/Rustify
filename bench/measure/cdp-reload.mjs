const port = 9222;
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find(t => t.type === 'page' && /open\.spotify\.com/.test(t.url));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
ws.send(JSON.stringify({ id: 1, method: 'Page.reload', params: { ignoreCache: false } }));
await new Promise(r => setTimeout(r, 500));
ws.close();
console.log('reload_sent');
