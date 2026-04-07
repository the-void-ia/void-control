import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 820 }, deviceScaleFactor: 1 });
await page.goto('http://127.0.0.1:3003/', { waitUntil: 'domcontentloaded' });
await page.locator('.run-item').first().waitFor();
await page.locator('.run-item').first().click();
await page.getByRole('button', { name: 'Graph', exact: true }).click();
await page.screenshot({ path: '/tmp/swarm_graph_live.png' });
const info = await page.evaluate(() => {
  const graph = document.querySelector('.sigma-canvas');
  const detail = document.querySelector('.swarm-detail-grid');
  const inspector = document.querySelector('.swarm-inspector');
  const graphRect = graph?.getBoundingClientRect();
  const detailRect = detail?.getBoundingClientRect();
  const inspectorRect = inspector?.getBoundingClientRect();
  return {
    graphRect,
    detailRect,
    inspectorRect,
    devicePixelRatio: window.devicePixelRatio,
  };
});
console.log(JSON.stringify(info, null, 2));
await browser.close();
