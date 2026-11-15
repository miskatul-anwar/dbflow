export const RELEASES_URL = "https://github.com/miskatul-anwar/dbflow/releases";
export const RELEASE_API_URL =
  "https://api.github.com/repos/miskatul-anwar/dbflow/releases/latest";
export const SYSTEMS = [
  { id: "windows", name: "Windows" },
  { id: "macos", name: "macOS" },
  { id: "linux", name: "Linux" },
  { id: "android", name: "Android" },
];

export function detectSystem(navigatorInfo) {
  const platform =
    navigatorInfo.userAgentData?.platform || navigatorInfo.platform || "";
  const userAgent = navigatorInfo.userAgent || "";
  const source = `${platform} ${userAgent}`;
  if (/android/i.test(source)) return { id: "android", arch: null };
  if (
    /iphone|ipad|ipod/i.test(source) ||
    (/mac/i.test(platform) && navigatorInfo.maxTouchPoints > 1)
  ) {
    return { id: "mobile", arch: null };
  }
  const arch = /aarch64|arm64|armv[78]/i.test(source) ? "arm64" : "x64";
  if (/win/i.test(source)) return { id: "windows", arch };
  if (/mac/i.test(source)) return { id: "macos", arch: null }; // Browser Mac platform values do not reliably identify the chip.
  if (/linux|x11|cros/i.test(source)) return { id: "linux", arch };
  return { id: "unknown", arch: null };
}

export function classifyAsset(asset) {
  const name = asset?.name || "";
  if (/\.app\.tar\.gz$|\.sig$|latest\.json$/i.test(name)) return null;
  const arm = /aarch64|arm64/i.test(name);
  if (/\.exe$/i.test(name))
    return {
      system: "windows",
      kind: "EXE installer",
      detail: "Recommended for most Windows users",
      arch: arm ? "arm64" : "x64",
      rank: 1,
    };
  if (/\.msi$/i.test(name))
    return {
      system: "windows",
      kind: "MSI installer",
      detail: "For managed installs",
      arch: arm ? "arm64" : "x64",
      rank: 2,
    };
  if (/\.dmg$/i.test(name))
    return {
      system: "macos",
      kind: arm ? "Apple Silicon" : "Intel Mac",
      detail: arm ? "M1 or newer" : "Intel processor",
      arch: arm ? "arm64" : "x64",
      rank: arm ? 1 : 2,
    };
  if (/\.AppImage$/i.test(name))
    return {
      system: "linux",
      kind: "AppImage",
      detail: "Portable Linux app",
      arch: arm ? "arm64" : "x64",
      rank: 1,
    };
  if (/\.deb$/i.test(name))
    return {
      system: "linux",
      kind: "DEB package",
      detail: "Debian and Ubuntu",
      arch: arm ? "arm64" : "x64",
      rank: 2,
    };
  if (/\.apk$/i.test(name))
    return {
      system: "android",
      kind: "APK",
      detail: "Phones and tablets with Android 7 or newer",
      arch: arm || !/x86|x64/i.test(name) ? "arm64" : "x64",
      rank: 1,
    };
  if (/\.rpm$/i.test(name))
    return {
      system: "linux",
      kind: "RPM package",
      detail: "Fedora, RHEL and openSUSE",
      arch: arm ? "arm64" : "x64",
      rank: 3,
    };
  return null;
}

export function getInstallers(release) {
  return (Array.isArray(release?.assets) ? release.assets : [])
    .map((asset) => {
      const type = classifyAsset(asset);
      if (!type || !isReleaseAssetUrl(asset.browser_download_url)) return null;
      return {
        ...type,
        name: asset.name,
        url: asset.browser_download_url,
        bytes: asset.size,
        digest: asset.digest || "",
      };
    })
    .filter(Boolean)
    .sort(
      (a, b) =>
        SYSTEMS.findIndex((system) => system.id === a.system) -
          SYSTEMS.findIndex((system) => system.id === b.system) ||
        a.rank - b.rank ||
        a.name.localeCompare(b.name),
    );
}

export function isReleaseAssetUrl(value) {
  if (typeof value !== "string") return false;
  try {
    const url = new URL(value);
    return (
      url.protocol === "https:" &&
      url.hostname === "github.com" &&
      url.pathname.startsWith("/miskatul-anwar/dbflow/releases/download/")
    );
  } catch {
    return false;
  }
}

export function formatSize(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Size unavailable";
  return `${Math.round(bytes / 1_000_000)} MB`;
}

export function sha256(digest) {
  const match = /^sha256:([a-f\d]{64})$/i.exec(digest || "");
  return match ? match[1] : null;
}
