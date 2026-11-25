const port = process.argv[2] || "9222";
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find(t => t.type === 'page' && /open\.spotify\.com/.test(t.url));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
const expression = `(() => {
  const out = [];
  for (const el of document.querySelectorAll('button, a, [role="button"]')) {
    const t = (el.innerText || el.getAttribute('aria-label') || '').trim();
    if (/explore premium|install app|get the app/i.test(t)) {
      const chain = [];
      let n = el;
      for (let i = 0; i < 4 && n; i++) {
        chain.push({ tag: n.tagName, testid: n.getAttribute('data-testid'), aria: n.getAttribute('aria-label'), cls: (n.className || '').toString().slice(0, 70) });
        n = n.parentElement;
      }
      out.push({ text: t.slice(0, 50), tag: el.tagName, testid: el.getAttribute('data-testid'), aria: el.getAttribute('aria-label'), href: el.getAttribute('href'), chain, html: el.outerHTML.slice(0, 200) });
    }
  }
  return { count: out.length, items: out };
})()`;
const result = await new Promise((res, rej) => {
  ws.addEventListener('message', e => {
    const m = JSON.parse(e.data);
    if (m.id === 1) m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
  });
  ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
});
ws.close();
console.log(JSON.stringify(result.result.value, null, 2));
