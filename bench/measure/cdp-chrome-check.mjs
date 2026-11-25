const port = process.argv[2] || '9222';

async function evalTarget(target, expression) {
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
        params: { expression, returnByValue: true, awaitPromise: true },
      }));
    });
    return result.result.value;
  } finally {
    ws.close();
  }
}

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const pages = targets.filter((target) => target.type === 'page');

const checks = await Promise.all(pages.map(async (target) => {
  const url = target.url;
  const bar = await evalTarget(target, `(() => {
    const drag = document.getElementById('drag');
    if (!drag) return null;
    return {
      title: (document.getElementById('title') || {}).textContent || null,
      width: innerWidth,
      height: innerHeight,
      buttons: [...document.querySelectorAll('.btn')].map((el) => el.id),
      bodyClass: document.body.className,
    };
  })()`);
  if (bar) return { kind: 'chrome-bar', url, bar };

  const upsells = await evalTarget(target, `(() => {
    const nav = document.querySelector('[data-testid="global-nav-bar"]');
    const premium = document.querySelector('[data-testid="global-nav-bar"] button[data-testid="upgrade-button"]');
    const install = document.querySelector('[data-testid="global-nav-bar"] a[href="/download"]');
    const info = (el) => el ? {
      display: getComputedStyle(el).display,
      visibility: getComputedStyle(el).visibility,
      width: el.getBoundingClientRect().width,
      height: el.getBoundingClientRect().height,
    } : null;
    return {
      title: document.title,
      nav: !!nav,
      premium: info(premium),
      install: info(install),
      upgradeCount: document.querySelectorAll('[data-testid="upgrade-button"]').length,
      downloadLinks: document.querySelectorAll('[data-testid="global-nav-bar"] a[href="/download"]').length,
      styleTag: !!document.getElementById('rustify-hide-upsells'),
    };
  })()`);
  return { kind: 'spotify-page', url, upsells };
}));

console.log(JSON.stringify(checks, null, 2));
