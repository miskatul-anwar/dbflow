#!/usr/bin/env node
/**
 * Accessibility audit of the phone interface (ISO/IEC 25010 interaction capability / WCAG 2.1 AA), the
 * counterpart of a11y-audit.cjs for the Android app.
 *
 * On a Pixel 7-sized screen with touch, in light and dark: runs axe-core on Data, Pipeline, every Results tab and
 * the Settings and Workspaces sheets, and measures every control's touch target. Exits non-zero on any axe
 * violation or any target under 24 × 24 CSS px (WCAG 2.2 SC 2.5.8). Targets under 48 × 48 (Material and
 * Android's guideline) are listed as advice.
 *
 *   bun run build && bunx vite preview --port 4311 &
 *   bun run audit:a11y:mobile          # URL=http://localhost:4311 by default
 */
const { chromium, devices } = require('playwright');
const fs = require('fs');
const os = require('os');
const path = require('path');

const URL = (process.env.URL || 'http://localhost:4311/').replace(/\/?$/, '/') + '?ui=mobile';
const axeSrc = fs.readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'nf-a11y-m-'));
const files = [path.join(dir, 'enrollment-records-for-the-autumn-semester.csv'), path.join(dir, 'courses.csv')];
const rows = ['student,course,teacher,dept,credits'];
const teach = { DB: 'Smith', OS: 'Jones', AI: 'Lee', Net: 'Kim' };
for (let i = 1; i <= 60; i++) {
  const c = Object.keys(teach)[i % 4];
  rows.push(`S${(i % 25) + 1},${c},${teach[c]},${c === 'Net' ? 'EE' : 'CS'},${c === 'DB' || c === 'AI' ? 3 : 4}`);
}
fs.writeFileSync(files[0], rows.join('\n'));
fs.writeFileSync(files[1], ['course,title', 'DB,Databases', 'OS,Operating systems', 'AI,Artificial intelligence', 'Net,Networks'].join('\n'));

(async () => {
  const browser = await chromium.launch();
  const violations = [];
  const small = new Map();
  for (const theme of ['light', 'dark']) {
    const ctx = await browser.newContext({ ...devices['Pixel 7'] });
    const page = await ctx.newPage();
    await page.addInitScript(t => {
      try {
        localStorage.setItem('dbflow.theme', t);
      } catch {
        /* storage blocked */
      }
    }, theme);
    await page.goto(URL);
    const scan = async name => {
      await page.waitForTimeout(300);
      await page.evaluate(src => {
        if (!window.axe) (0, eval)(src);
      }, axeSrc);
      const r = await page.evaluate(() => axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'wcag21aa', 'best-practice'] }));
      for (const v of r.violations) {
        violations.push(`${theme} / ${name}: ${v.id} (${v.impact}) x${v.nodes.length} - ${v.help}`);
        if (process.env.DETAIL) for (const n of v.nodes) violations.push(`      ${n.target.join(" ")} :: ${(n.any[0]?.message || "").slice(0, 140)}`);
      }
      const targets = await page.evaluate(() =>
        [...document.querySelectorAll('button, a[href], input:not([type=hidden]), select, textarea, [role=tab], [role=switch], [role=radio], [role=checkbox]')]
          .filter(e => !e.closest('[inert], [aria-hidden=true]'))
          .map(e => {
            const b = e.getBoundingClientRect();
            const label = (e.getAttribute('aria-label') || e.textContent || e.getAttribute('placeholder') || e.tagName).trim().replace(/\s+/g, ' ').slice(0, 40);
            return { label, w: Math.round(b.width), h: Math.round(b.height), visible: b.width > 0 && b.height > 0 && getComputedStyle(e).visibility !== 'hidden' };
          })
          .filter(t => t.visible && (t.w < 48 || t.h < 48)),
      );
      for (const t of targets) small.set(`${t.label} (${t.w}×${t.h})`, { ...t, where: name });
    };
    await scan('name your workspace');
    await page.getByRole('button', { name: 'Start' }).click(); // the launch sheet asks for a workspace name
    await scan('data, empty');
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('button', { name: /Add/ }).first().click();
    await (await chooser).setFiles(files);
    await scan('data, two files');
    await page.getByRole('button', { name: 'Pipeline' }).click();
    await scan('pipeline');
    await page.getByRole('button', { name: /^Run/ }).first().click();
    await page.getByRole('tab', { name: 'Overview' }).waitFor({ timeout: 180000 });
    const tabs = page.getByRole('tab');
    for (let i = 0, n = await tabs.count(); i < n; i++) {
      await tabs.nth(i).click();
      await scan(`results / ${(await tabs.nth(i).textContent()).trim()}`);
    }
    await page.getByRole('button', { name: 'Settings' }).click();
    await scan('settings sheet');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: /^Workspace / }).click();
    await scan('workspaces sheet');
    await ctx.close();
  }
  await browser.close();

  const tooSmall = [...small.entries()].filter(([, t]) => t.w < 24 || t.h < 24);
  console.log(`axe violations: ${violations.length}`);
  for (const v of violations) console.log(`  ${v}`);
  console.log(`touch targets under 24×24 (WCAG 2.5.8, fails): ${tooSmall.length}`);
  for (const [k, t] of tooSmall) console.log(`  ${k} on ${t.where}`);
  const advice = [...small.entries()].filter(([, t]) => t.w >= 24 && t.h >= 24);
  console.log(`touch targets under 48×48 (Material guideline, advice): ${advice.length}`);
  for (const [k, t] of advice) console.log(`  ${k} on ${t.where}`);
  process.exit(violations.length || tooSmall.length ? 1 : 0);
})().catch(e => {
  console.error(e);
  process.exit(2);
});
