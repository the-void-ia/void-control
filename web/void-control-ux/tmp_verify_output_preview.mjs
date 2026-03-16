import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto('http://127.0.0.1:3000/', { waitUntil: 'networkidle' });
const buttonCount = await page.getByRole('button', { name: 'Open output.json' }).count();
const inspectorText = await page.locator('.inspector-panel').innerText().catch(() => '');
console.log(JSON.stringify({ hasOutputButton: buttonCount > 0, inspectorText: inspectorText.slice(0, 500) }, null, 2));
await browser.close();
