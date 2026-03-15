import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto('http://127.0.0.1:3000/', { waitUntil: 'networkidle' });
const result = await page.evaluate(async () => {
  const res = await fetch('/api/v1/runs/run-1773361919868/events');
  const events = await res.json();
  return events
    .filter((e) => (e.message || '').includes('/workspace/output.json'))
    .map((e) => ({ seq: e.seq, event_type: e.event_type_v2 || e.event_type, stage_name: e.stage_name, payload: e.payload, message: e.message }));
});
console.log(JSON.stringify(result, null, 2));
await browser.close();
