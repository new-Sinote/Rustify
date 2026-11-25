const port = process.argv[2] || '9222';

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === 'page' && /open\.spotify\.com/.test(t.url));
if (!page) throw new Error('Spotify page target not found');

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener('open', resolve, { once: true });
  ws.addEventListener('error', reject, { once: true });
});

let id = 1;
const call = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const my = id++;
    const onMessage = (event) => {
      const message = JSON.parse(event.data);
      if (message.id !== my) return;
      ws.removeEventListener('message', onMessage);
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    };
    ws.addEventListener('message', onMessage);
    ws.send(JSON.stringify({ id: my, method, params }));
  });

const readLabel = async () => {
  const result = await call('Runtime.evaluate', {
    expression: `document.querySelector('[data-testid="control-button-playpause"]')?.getAttribute('aria-label') ?? null`,
    returnByValue: true,
  });
  return result.result.value;
};

const before = await readLabel();
if (before !== 'Pause') {
  console.log(`already paused label=${before}`);
  ws.close();
  process.exit(0);
}

const clicked = await call('Runtime.evaluate', {
  expression: `(() => { const b = document.querySelector('[data-testid="control-button-playpause"]'); if (!b) return 'no_button'; b.click(); return 'clicked'; })()`,
  returnByValue: true,
  userGesture: true,
});
console.log(`playpause=${clicked.result.value} before=${before}`);

await new Promise((resolve) => setTimeout(resolve, 2500));
const after = await readLabel();
console.log(`after=${after}`);
ws.close();
if (after !== 'Play') {
  console.error('pause did not settle');
  process.exit(1);
}
