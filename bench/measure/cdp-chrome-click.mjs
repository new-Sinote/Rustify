const port = process.argv[2] || '9222';
const id = process.argv[3];
if (!id) throw new Error('usage: node cdp-chrome-click.mjs <port> <button-id>');

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const pages = targets.filter((target) => target.type === 'page');

async function evaluate(target, expression) {
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', reject, { once: true });
  });
  try {
    const result = await new Promise((resolve, reject) => {
      ws.addEventListener('message', (event) => {
        const message = JSON.parse(event.data);
        if (message.id !== 1) return;
        if (message.error) reject(new Error(JSON.stringify(message.error)));
        else resolve(message.result);
      });
      ws.send(JSON.stringify({
        id: 1,
        method: 'Runtime.evaluate',
        params: { expression, returnByValue: true, userGesture: true },
      }));
    });
    return result.result.value;
  } finally {
    ws.close();
  }
}

for (const page of pages) {
  const isBar = await evaluate(page, `!!document.getElementById('drag')`);
  if (!isBar) continue;
  const result = await evaluate(page, `(() => {
    const el = document.getElementById(${JSON.stringify(id)});
    if (!el) return { clicked: false, reason: 'missing' };
    const style = getComputedStyle(el);
    el.click();
    return { clicked: true, display: style.display };
  })()`);
  console.log(JSON.stringify({ id, ...result }));
  process.exit(0);
}
throw new Error('chrome bar target not found');
