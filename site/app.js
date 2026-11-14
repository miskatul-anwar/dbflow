import {
  RELEASE_API_URL,
  RELEASES_URL,
  SYSTEMS,
  detectSystem,
  formatSize,
  getInstallers,
  sha256,
} from "./downloads.mjs";

const $ = (id) => document.getElementById(id);
const detected = detectSystem(navigator);
const state = {
  installers: [],
  system: SYSTEMS.some((item) => item.id === detected.id)
    ? detected.id
    : "windows",
};

function node(tag, className, content) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (content != null) element.textContent = content;
  return element;
}

const iconPaths = {
  windows: ["M3 4h8v7H3z", "M13 4h8v7h-8z", "M3 13h8v7H3z", "M13 13h8v7h-8z"],
  macos: ["M4 5h16v11H4z", "M2 19h20"],
  linux: ["M4 5h16v14H4z", "m8 9 3 3-3 3", "M13 16h4"],
};

function systemIcon(systemId) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "system-icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  for (const shape of iconPaths[systemId] || []) {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", shape);
    svg.append(path);
  }
  return svg;
}

function renderTabs() {
  const tabs = $("system-tabs");
  tabs.replaceChildren();
  for (const system of SYSTEMS) {
    const button = node("button", "system-tab");
    button.type = "button";
    button.append(systemIcon(system.id), node("span", "", system.name));
    button.setAttribute("aria-pressed", String(state.system === system.id));
    button.addEventListener("click", () => {
      state.system = system.id;
      renderTabs();
      renderChoices();
    });
    tabs.append(button);
  }
}

function renderChoices() {
  const target = $("installer-list");
  target.replaceChildren();
  $("install-system").textContent =
    SYSTEMS.find((item) => item.id === state.system)?.name || "";
  document.querySelectorAll("[data-install]").forEach((element) => {
    element.hidden = element.dataset.install !== state.system;
  });
  const current = state.installers.filter(
    (item) => item.system === state.system,
  );
  if (!current.length) {
    target.append(
      node(
        "p",
        "empty-state",
        "No installer for this system is available in this release. Check the GitHub release for updates.",
      ),
    );
    return;
  }
  const needsArchitectureCheck =
    detected.id === state.system &&
    detected.arch === "arm64" &&
    !current.some((item) => item.arch === "arm64");
  if (needsArchitectureCheck) {
    target.append(
      node(
        "p",
        "compatibility-note",
        "Your device appears to use ARM. The available installers for this system are x64 and may not run on it.",
      ),
    );
  }
  if (state.system === "macos") {
    target.append(
      node(
        "p",
        "choice-note",
        "Check Apple menu → About This Mac if you are unsure which processor you have.",
      ),
    );
  }
  const list = node("div", "installer-grid");
  for (const [index, installer] of current.entries()) {
    const card = node("article", "installer-card");
    const info = node("div", "installer-info");
    const heading = node("h3", "", installer.kind);
    info.append(
      heading,
      node(
        "p",
        "",
        `${installer.detail} · ${installer.arch === "arm64" ? "ARM64" : "x64"} · ${formatSize(installer.bytes)}`,
      ),
    );
    const action = node(
      "a",
      `download-button ${index > 0 || needsArchitectureCheck ? "secondary" : ""}`,
      "Download",
    );
    action.href = installer.url;
    action.setAttribute(
      "aria-label",
      `Download ${installer.kind} for ${state.system}`,
    );
    card.append(info, action);
    list.append(card);
  }
  target.append(list);
}

function renderAllFiles() {
  const target = $("all-files");
  target.replaceChildren();
  for (const installer of state.installers) {
    const row = node("tr");
    const platform =
      SYSTEMS.find((item) => item.id === installer.system)?.name ||
      installer.system;
    row.append(
      node("td", "", platform),
      node(
        "td",
        "",
        `${installer.kind} · ${installer.arch === "arm64" ? "ARM64" : "x64"}`,
      ),
    );
    const fileCell = node("td");
    const fileLink = node("a", "", installer.name);
    fileLink.href = installer.url;
    fileCell.append(fileLink);
    row.append(fileCell, node("td", "", formatSize(installer.bytes)));
    const hashCell = node("td", "hash-cell");
    const hash = sha256(installer.digest);
    if (hash) {
      hashCell.append(node("code", "", hash));
      const copy = node("button", "copy-button", "Copy");
      copy.type = "button";
      copy.setAttribute("aria-label", `Copy SHA-256 for ${installer.name}`);
      copy.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(hash);
          copy.textContent = "Copied";
          window.setTimeout(() => {
            copy.textContent = "Copy";
          }, 1800);
        } catch {
          copy.textContent = "Unavailable";
        }
      });
      hashCell.append(copy);
    } else hashCell.textContent = "Not provided";
    row.append(hashCell);
    target.append(row);
  }
}

function renderRelease(release) {
  state.installers = getInstallers(release);
  if (!state.installers.length)
    throw new Error("No installers in latest release");
  const version = String(release.tag_name || "latest").replace(/^v/, "");
  $("release-version").textContent = `Version ${version}`;
  $("release-status").textContent = "Latest stable release";
  const date = release.published_at ? new Date(release.published_at) : null;
  $("release-date").textContent =
    date && !Number.isNaN(date.valueOf())
      ? `Published ${new Intl.DateTimeFormat("en", { dateStyle: "medium" }).format(date)}`
      : "Published on GitHub";
  $("release-link").href = release.html_url?.startsWith(`${RELEASES_URL}/tag/`)
    ? release.html_url
    : `${RELEASES_URL}/latest`;
  $("release-content").hidden = false;
  $("loading-state").hidden = true;
  renderTabs();
  renderChoices();
  renderAllFiles();
}

function renderFailure(error) {
  $("release-status").textContent = "Release information unavailable";
  $("release-version").textContent = "Download on GitHub";
  $("release-date").textContent = "";
  $("all-files").replaceChildren(node("tr"));
  const cell = node(
    "td",
    "",
    "Could not load release files. Use the GitHub release link above.",
  );
  cell.colSpan = 5;
  $("all-files").firstChild.append(cell);
  $("loading-state").hidden = false;
  $("release-content").hidden = true;
  $("loading-state").textContent =
    error?.message === "403"
      ? "GitHub is limiting release lookups right now. Open the latest release on GitHub to download."
      : "The latest release could not be loaded. Open it on GitHub to download.";
}

async function loadRelease() {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 10000);
  try {
    const response = await fetch(RELEASE_API_URL, {
      headers: { Accept: "application/vnd.github+json" },
      signal: controller.signal,
    });
    if (!response.ok) throw new Error(String(response.status));
    renderRelease(await response.json());
  } catch (error) {
    renderFailure(error);
  } finally {
    window.clearTimeout(timeout);
  }
}

$("refresh-release").addEventListener("click", loadRelease);
loadRelease();
