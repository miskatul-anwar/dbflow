import { test, vi } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { setTimeout } from "node:timers";
import { URL } from "node:url";
import { JSDOM } from "jsdom";

const html = readFileSync(
  new URL("../../site/index.html", import.meta.url),
  "utf8",
);
const url = (name) =>
  `https://github.com/miskatul-anwar/dbflow/releases/download/v6.0.0/${name}`;
const release = {
  tag_name: "v6.0.0",
  html_url: "https://github.com/miskatul-anwar/dbflow/releases/tag/v6.0.0",
  published_at: "2026-11-14T12:00:00Z",
  assets: [
    "DBFlow_6.0.0_x64-setup.exe",
    "DBFlow_6.0.0_aarch64.dmg",
    "DBFlow_6.0.0_x64.dmg",
    "DBFlow_6.0.0_amd64.AppImage",
    "DBFlow_6.0.0_amd64.deb",
  ].map((name) => ({
    name,
    browser_download_url: url(name),
    size: 200_000_000,
    digest: `sha256:${"a".repeat(64)}`,
  })),
};
async function mount(platform, response) {
  const dom = new JSDOM(html, { url: "https://example.test/" });
  Object.defineProperty(dom.window.navigator, "platform", {
    configurable: true,
    value: platform,
  });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: dom.window.navigator,
  });
  globalThis.document = dom.window.document;
  globalThis.window = dom.window;
  globalThis.fetch = async () => response;
  vi.resetModules();
  await import("../../site/app.js");
  await new Promise((resolve) => setTimeout(resolve, 0));
  return dom;
}

test("shows latest installers and updates help when the visitor changes systems", async () => {
  const dom = await mount("Win32", { ok: true, json: async () => release });
  const document = dom.window.document;
  assert.equal(
    document.querySelector("#release-version").textContent,
    "Version 6.0.0",
  );
  assert.equal(
    document.querySelectorAll("#installer-list .installer-card").length,
    1,
  );
  assert.equal(document.querySelectorAll("#all-files tr").length, 5);
  // Each cell names its column, for the stacked layout on phones.
  assert.deepEqual(
    [...document.querySelector("#all-files tr").children].map((td) => td.dataset.label),
    ["System", "Format", "File", "Size", "SHA-256"],
  );
  assert.equal(
    document.querySelector("#install-system").textContent,
    "Windows",
  );
  assert.equal(
    document.querySelector("#installer-list a").href,
    url("DBFlow_6.0.0_x64-setup.exe"),
  );
  [...document.querySelectorAll(".system-tab")]
    .find((button) => button.textContent.includes("macOS"))
    .click();
  assert.equal(
    document.querySelectorAll("#installer-list .installer-card").length,
    2,
  );
  assert.equal(document.querySelector("#install-system").textContent, "macOS");
  assert.equal(document.querySelector('[data-install="macos"]').hidden, false);
  dom.window.close();
});

test("provides the GitHub fallback on API failure", async () => {
  const dom = await mount("Win32", { ok: false, status: 403 });
  const document = dom.window.document;
  assert.match(
    document.querySelector("#loading-state").textContent,
    /limiting release lookups/,
  );
  assert.equal(
    document.querySelector("#release-link").href,
    "https://github.com/miskatul-anwar/dbflow/releases/latest",
  );
  assert.match(
    document.querySelector("#all-files").textContent,
    /Could not load release files/,
  );
  dom.window.close();
});

test("switching to a system without files updates the install guidance", async () => {
  const windowsOnly = {
    ...release,
    assets: release.assets.filter((asset) => asset.name.endsWith(".exe")),
  };
  const dom = await mount("Win32", { ok: true, json: async () => windowsOnly });
  const document = dom.window.document;
  [...document.querySelectorAll(".system-tab")]
    .find((button) => button.textContent.includes("Linux"))
    .click();
  assert.match(
    document.querySelector("#installer-list").textContent,
    /No installer for this system/,
  );
  assert.equal(document.querySelector("#install-system").textContent, "Linux");
  assert.equal(document.querySelector('[data-install="linux"]').hidden, false);
  assert.equal(document.querySelector('[data-install="windows"]').hidden, true);
  dom.window.close();
});
