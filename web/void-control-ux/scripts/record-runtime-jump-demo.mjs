import fs from 'node:fs/promises';
import { chromium } from 'playwright';

const uiBaseUrl = process.env.UI_BASE_URL ?? 'http://127.0.0.1:3000/';
const executionId = process.env.EXECUTION_ID;
const outputDir = process.env.VIDEO_DIR ?? '/tmp/void-control-runtime-jump';
const chromePath = process.env.CHROME_BIN ?? '/usr/bin/google-chrome';

if (!executionId) {
  throw new Error('EXECUTION_ID is required');
}

async function main() {
  await fs.mkdir(outputDir, { recursive: true });

  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
  });
  const context = await browser.newContext({
    viewport: { width: 1728, height: 980 },
    recordVideo: {
      dir: outputDir,
      size: { width: 1728, height: 980 },
    },
  });
  const page = await context.newPage();
  await page.goto(uiBaseUrl, { waitUntil: 'domcontentloaded' });
  const executionRow = page.locator('.run-item.execution-item').filter({ hasText: executionId }).first();
  await executionRow.waitFor({ timeout: 15000 });
  await executionRow.click();
  await page.waitForTimeout(2000);
  const openRuntimeButton = page.getByRole('button', { name: /Open Runtime Graph/i });
  await openRuntimeButton.waitFor({ timeout: 15000 });
  await page.waitForTimeout(1200);
  await openRuntimeButton.click();
  await page.waitForTimeout(5000);
  const video = page.video();
  if (!video) {
    throw new Error('missing runtime jump video');
  }
  await page.close();
  const videoPath = await video.path();
  console.log(JSON.stringify({ executionId, videoPath }, null, 2));
  await context.close();
  await browser.close();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
