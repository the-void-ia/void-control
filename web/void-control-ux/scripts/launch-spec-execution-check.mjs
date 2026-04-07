import { chromium } from 'playwright';
import fs from 'node:fs/promises';

const baseUrl = process.env.UI_BASE_URL ?? 'http://127.0.0.1:3000/';
const specPath = process.env.SPEC_PATH ?? '/home/diego/github/void-control/examples/swarm-transform-optimization-3way.yaml';

const specText = await fs.readFile(specPath, 'utf8');

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1728, height: 980 }, deviceScaleFactor: 1 });

await page.goto(baseUrl, { waitUntil: 'domcontentloaded' });
await page.getByRole('button', { name: '+ Launch Spec' }).click();
await page.locator('.launch-modal').waitFor();

await page.locator('textarea').fill(specText);
await page.getByRole('button', { name: /^Launch$/ }).click();

await page.waitForTimeout(2000);

const result = await page.evaluate(() => {
  const selectedExecution = document.querySelector('.run-item.execution-item.selected');
  const toolbar = document.querySelector('.toolbar');
  const modal = document.querySelector('.launch-modal');
  const firstExecution = document.querySelector('.run-item.execution-item');

  return {
    modalStillOpen: Boolean(modal),
    selectedExecutionText: selectedExecution?.textContent?.replace(/\s+/g, ' ').trim() ?? null,
    firstExecutionText: firstExecution?.textContent?.replace(/\s+/g, ' ').trim() ?? null,
    toolbarText: toolbar?.textContent?.replace(/\s+/g, ' ').trim() ?? null,
  };
});

await page.screenshot({ path: '/tmp/launch-spec-execution-check.png' });
console.log(JSON.stringify(result, null, 2));

await browser.close();
