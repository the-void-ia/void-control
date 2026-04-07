import { chromium } from 'playwright';

const baseUrl = process.env.UI_BASE_URL ?? 'http://127.0.0.1:3000/';

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 820 } });

  await page.goto(baseUrl, { waitUntil: 'domcontentloaded' });
  await page.locator('.run-item').first().waitFor();
  await page.locator('.run-item').first().click();
  await page.getByRole('button', { name: 'Graph', exact: true }).click();

  const fillGap = await page.evaluate(() => {
    const detail = document.querySelector('.detail-panel');
    const swarm = document.querySelector('.swarm-detail-grid');
    if (!(detail instanceof HTMLElement) || !(swarm instanceof HTMLElement)) return null;
    const detailRect = detail.getBoundingClientRect();
    const swarmRect = swarm.getBoundingClientRect();
    return Math.round(detailRect.bottom - swarmRect.bottom);
  });
  if (fillGap === null || fillGap > 32) {
    throw new Error(`expected swarm detail row to fill the panel, gap=${fillGap ?? 'null'}px`);
  }

  const inspectorSections = page.locator('.swarm-inspector .inspector-section');
  const sectionCount = await inspectorSections.count();
  if (sectionCount < 4) {
    throw new Error(`expected richer swarm inspector with at least 4 sections, got ${sectionCount}`);
  }

  const heroCount = await page.locator('.swarm-inspector .swarm-inspector-hero').count();
  if (heroCount !== 1) {
    throw new Error(`expected swarm inspector hero block, got ${heroCount}`);
  }

  const scrollableInspector = await page.locator('.swarm-inspector').evaluate((node) => {
    const style = window.getComputedStyle(node);
    return style.overflowY !== 'hidden' && node.scrollHeight > node.clientHeight;
  });
  if (!scrollableInspector) {
    throw new Error('expected swarm inspector to be vertically scrollable when content exceeds its height');
  }

  await browser.close();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
