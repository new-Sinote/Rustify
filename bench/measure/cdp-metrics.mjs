const port = process.argv[2] || "9222";

async function getJson(path) {
  const res = await fetch(`http://127.0.0.1:${port}${path}`);
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
}

function connect(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const pending = new Map();
    let nextId = 1;
    ws.addEventListener("open", () => {
      resolve({
        call(method, params = {}) {
          return new Promise((res, rej) => {
            const id = nextId++;
            pending.set(id, { res, rej });
            ws.send(JSON.stringify({ id, method, params }));
          });
        },
        close() { ws.close(); },
      });
    });
    ws.addEventListener("message", (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error) rej(new Error(`${msg.error.code}: ${msg.error.message}`));
        else res(msg.result);
      }
    });
    ws.addEventListener("error", (e) => reject(new Error(`websocket error: ${e.message || "unknown"}`)));
  });
}

const version = await getJson("/json/version");
const targets = await getJson("/json/list");
const page = targets.find((t) => t.type === "page" && /open\.spotify\.com/.test(t.url)) ||
             targets.find((t) => t.type === "page");

if (!page) {
  console.error(JSON.stringify({ error: "no_page_target", targets }, null, 2));
  process.exit(1);
}

const cdp = await connect(page.webSocketDebuggerUrl);
await cdp.call("Runtime.enable");
await cdp.call("Performance.enable");

const heap = await cdp.call("Runtime.getHeapUsage");
const dom = await cdp.call("Memory.getDOMCounters");
const perf = await cdp.call("Performance.getMetrics");
const metrics = Object.fromEntries(perf.metrics.map((m) => [m.name, m.value]));

const nav = await cdp.call("Runtime.evaluate", {
  expression: `({href: location.href, title: document.title, visibility: document.visibilityState, ua: navigator.userAgent})`,
  returnByValue: true,
});
cdp.close();

let browserProcesses = [];
try {
  const browser = await connect(version.webSocketDebuggerUrl);
  const info = await browser.call("SystemInfo.getProcessInfo");
  browserProcesses = info.processInfo;
  browser.close();
} catch (e) {
  browserProcesses = [{ error: String(e) }];
}

const out = {
  target: { id: page.id, url: page.url, title: page.title },
  page: nav.result?.value,
  heap: {
    used_mb: +(heap.usedSize / 1048576).toFixed(1),
    total_mb: +(heap.totalSize / 1048576).toFixed(1),
    embedder_used_mb: +(heap.embedderHeapUsedSize / 1048576).toFixed(1),
    backing_storage_mb: +(heap.backingStorageSize / 1048576).toFixed(1),
  },
  dom: {
    documents: dom.documents,
    nodes: dom.nodes,
    js_event_listeners: dom.jsEventListeners,
  },
  perf: {
    js_heap_used_mb: +(metrics.JSHeapUsedSize / 1048576).toFixed(1),
    js_heap_total_mb: +(metrics.JSHeapTotalSize / 1048576).toFixed(1),
    nodes: metrics.Nodes,
    documents: metrics.Documents,
    layout_count: metrics.LayoutCount,
    recalc_style_count: metrics.RecalcStyleCount,
    task_duration_s: metrics.TaskDuration,
  },
  browser_processes: browserProcesses.map((p) => ({
    id: p.id,
    type: p.type,
    cpu_s: +(p.cpuTime / 1e6).toFixed(2),
  })),
};

console.log(JSON.stringify(out, null, 2));
