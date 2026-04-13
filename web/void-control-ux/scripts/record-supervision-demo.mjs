import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const uiBaseUrl = process.env.UI_BASE_URL ?? 'http://127.0.0.1:3000/';
const controlBaseUrl = process.env.CONTROL_BASE_URL ?? 'http://127.0.0.1:43210';
const specPath =
  process.env.SPEC_PATH
  ?? '/home/diego/github/void-control/examples/supervision-transform-review.yaml';
const outputDir = process.env.VIDEO_DIR ?? '/tmp/void-control-supervision-demo';
const chromePath = process.env.CHROME_BIN ?? '/usr/bin/google-chrome';

async function getExecution(executionId) {
  const response = await fetch(`${controlBaseUrl}/v1/executions/${executionId}`);
  if (!response.ok) {
    throw new Error(`execution fetch failed: HTTP ${response.status}`);
  }
  return response.json();
}

async function pollForUsefulState(executionId, timeoutMs = 180000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const payload = await getExecution(executionId);
    const execution = payload.execution ?? payload;
    const status = execution.status ?? 'unknown';
    const candidates = payload.candidates ?? [];
    const hasRuntimeRun = candidates.some((candidate) => Boolean(candidate.runtime_run_id));
    const hasReviewState = candidates.some((candidate) => Boolean(candidate.review_status));
    if (hasRuntimeRun && hasReviewState) {
      return payload;
    }
    if (status === 'Completed' || status === 'Failed' || status === 'Cancelled') {
      return payload;
    }
    await new Promise((resolve) => setTimeout(resolve, 1500));
  }
  throw new Error(`execution ${executionId} did not reach a useful state within ${timeoutMs}ms`);
}

function extractExecutionId(text) {
  const match = text.match(/exec-[0-9]+/);
  return match?.[0] ?? null;
}

async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  const specText = await fs.readFile(specPath, 'utf8');

  const browser = await chromium.launch({
    headless: true,
    executablePath: chromePath,
    args: ['--no-sandbox'],
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
  await page.getByRole('button', { name: '+ Launch Spec' }).waitFor({ timeout: 20000 });
  await page.waitForTimeout(1000);

  await page.getByRole('button', { name: '+ Launch Spec' }).click();
  await page.locator('.launch-modal').waitFor({ timeout: 10000 });
  await page.locator('textarea').fill(specText);
  await page.waitForTimeout(1000);
  await page.getByRole('button', { name: /^Launch$/ }).click();
  await page.waitForTimeout(3000);

  const selectedExecutionText = await page.locator('.run-item.execution-item.selected').first().textContent();
  const executionId = extractExecutionId(selectedExecutionText ?? '');
  if (!executionId) {
    throw new Error(`could not extract execution id from selection: ${selectedExecutionText ?? '<empty>'}`);
  }

  await page.waitForTimeout(4000);
  const payload = await pollForUsefulState(executionId);
  const runtimeRunId =
    payload.candidates?.find((candidate) => candidate.runtime_run_id)?.runtime_run_id ?? null;

  await page.reload({ waitUntil: 'domcontentloaded' });
  const executionRow = page.locator('.run-item.execution-item').filter({ hasText: executionId }).first();
  await executionRow.waitFor({ timeout: 20000 });
  await executionRow.click();
  await page.waitForTimeout(3000);

  const inspector = page.locator('.inspector-panel, .swarm-inspector').first();
  await inspector.waitFor({ timeout: 10000 }).catch(() => {});
  await page.waitForTimeout(2000);

  const openRuntimeButton = page.getByRole('button', { name: /Open Runtime Graph/i });
  if (await openRuntimeButton.count()) {
    await openRuntimeButton.click();
    await page.waitForTimeout(5000);
  }

  await page.screenshot({ path: path.join(outputDir, 'supervision-demo-final.png') });
  const video = page.video();
  if (!video) {
    throw new Error('playwright did not produce a supervision video artifact');
  }
  await page.close();
  const videoPath = await video.path();
  console.log(JSON.stringify({ executionId, runtimeRunId, videoPath }, null, 2));
  await context.close();
  await browser.close();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
