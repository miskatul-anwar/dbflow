import { test } from "vitest";
import assert from "node:assert/strict";
import {
  classifyAsset,
  detectSystem,
  formatSize,
  getInstallers,
  sha256,
} from "../../site/downloads.mjs";

const releaseUrl = (name) =>
  `https://github.com/miskatul-anwar/dbflow/releases/download/v6.0.0/${name}`;
const asset = (name, extra = {}) => ({
  name,
  browser_download_url: releaseUrl(name),
  size: 200_000_000,
  ...extra,
});

const fixture = {
  assets: [
    asset("DBFlow_6.0.0_x64-setup.exe"),
    asset("DBFlow_6.0.0_x64_en-US.msi"),
    asset("DBFlow_6.0.0_aarch64.dmg"),
    asset("DBFlow_6.0.0_x64.dmg"),
    asset("DBFlow_6.0.0_amd64.AppImage"),
    asset("DBFlow_6.0.0_amd64.deb"),
    asset("DBFlow-6.0.0-1.x86_64.rpm"),
    asset("DBFlow_aarch64.app.tar.gz"),
    asset("DBFlow_6.0.0_x64-setup.exe", {
      browser_download_url: "https://example.com/bad.exe",
    }),
  ],
};

test("lists only the seven installable, trusted release assets in useful order", () => {
  const files = getInstallers(fixture);
  assert.equal(files.length, 7);
  assert.deepEqual(
    files.map((file) => file.kind),
    [
      "EXE installer",
      "MSI installer",
      "Apple Silicon",
      "Intel Mac",
      "AppImage",
      "DEB package",
      "RPM package",
    ],
  );
  assert.ok(
    files.every((file) =>
      file.url.startsWith(
        "https://github.com/miskatul-anwar/dbflow/releases/download/",
      ),
    ),
  );
});

test("labels architecture correctly when ARM builds are released", () => {
  assert.equal(classifyAsset(asset("DBFlow_7.0.0_arm64.deb")).arch, "arm64");
  assert.equal(
    classifyAsset(asset("DBFlow_7.0.0_aarch64.dmg")).kind,
    "Apple Silicon",
  );
  assert.equal(classifyAsset(asset("DBFlow_7.0.0_x64.dmg")).kind, "Intel Mac");
});

test("detects common desktop, mobile, and ARM platforms", () => {
  assert.deepEqual(
    detectSystem({ platform: "Win32", userAgent: "Windows NT 10.0" }),
    { id: "windows", arch: "x64" },
  );
  assert.deepEqual(
    detectSystem({ platform: "Linux aarch64", userAgent: "Linux" }),
    { id: "linux", arch: "arm64" },
  );
  assert.deepEqual(
    detectSystem({
      platform: "MacIntel",
      userAgent: "Macintosh",
      maxTouchPoints: 5,
    }),
    { id: "mobile", arch: null },
  );
  assert.equal(
    detectSystem({ platform: "MacIntel", userAgent: "Macintosh" }).id,
    "macos",
  );
});

test("formats size and accepts only GitHub SHA-256 digests", () => {
  assert.equal(formatSize(273_555_280), "274 MB");
  assert.equal(formatSize(null), "Size unavailable");
  assert.equal(sha256("sha256:" + "a".repeat(64)), "a".repeat(64));
  assert.equal(sha256("sha256:bad"), null);
});
