import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const uiBaseUrl = process.env.UI_BASE_URL ?? 'http://127.0.0.1:3000/';
const controlBaseUrl = process.env.CONTROL_BASE_URL ?? 'http://127.0.0.1:43210';
const specPath =
  process.env.SPEC_PATH
  ?? '/home/diego/github/void-control/examples/swarm-transform-optimization-3way.yaml';
const outputDir = process.env.VIDEO_DIR ?? '/tmp/void-control-swarm-demo';
const chromePath = process.env.CHROME_BIN ?? '/usr/bin/google-chrome';

async function createExecution() {
  const specText = await fs.readFile(specPath, 'utf8');
  const response = await fetch(`${controlBaseUrl}/v1/executions`, {
    method: 'POST',
    headers: { 'Content-Type': 'text/yaml' },
    body: specText,
  });
  if (!response.ok) {
    throw new Error(`execution create failed: HTTP ${response.status}`);
  }
  return response.json();
}

async function getExecution(executionId) {
  const response = await fetch(`${controlBaseUrl}/v1/executions/${executionId}`);
  if (!response.ok) {
    throw new Error(`execution fetch failed: HTTP ${response.status}`);
  }
  return response.json();
}

async function pollForCompletion(executionId, timeoutMs = 120000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const payload = await getExecution(executionId);
    const status = payload.execution?.status ?? payload.status;
    if (status === 'Completed' || status === 'Failed' || status === 'Cancelled') {
      return payload;
    }
    await new Promise((resolve) => setTimeout(resolve, 1500));
  }
  throw new Error(`execution ${executionId} did not complete within ${timeoutMs}ms`);
}

async function main() {
  await fs.mkdir(outputDir, { recursive: true });

  const execution = await createExecution();
  const executionId = execution.execution_id;
  if (!executionId) {
    throw new Error('execution create response missing execution_id');
  }

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
  await page.locator('.run-item').first().waitFor({ timeout: 15000 });
  await page.waitForTimeout(1000);

  const executionRow = page.locator('.run-item.execution-item').filter({
    hasText: executionId,
  }).first();
  await executionRow.waitFor({ timeout: 15000 });
  await executionRow.click();
  await page.waitForTimeout(2500);

  // Let the running swarm view be recorded before terminal state arrives.
  await page.waitForTimeout(7000);

  const completedPayload = await pollForCompletion(executionId);
  const runtimeRunId =
    completedPayload.result?.candidates?.find((candidate) => candidate.runtime_run_id)?.runtime_run_id
    ?? null;

  await page.reload({ waitUntil: 'domcontentloaded' });
  await executionRow.waitFor({ timeout: 15000 });
  await executionRow.click();
  await page.waitForTimeout(2500);

  if (runtimeRunId) {
    const openRuntimeButton = page.getByRole('button', { name: /Open Runtime Graph/i });
    await openRuntimeButton.waitFor({ timeout: 15000 });
    await page.waitForTimeout(1500);
    await openRuntimeButton.click();
    await page.waitForTimeout(4000);
  }

  await page.screenshot({ path: path.join(outputDir, 'swarm-demo-final.png') });
  await page.close();

  const video = page.video();
  if (!video) {
    throw new Error('playwright did not produce a video artifact');
  }
  const videoPath = await video.path();
  console.log(JSON.stringify({ executionId, runtimeRunId, videoPath }, null, 2));
  await context.close();
  await browser.close();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
