'use strict';
(function () {
  var REPO = 'miskatul-anwar/dbflow';
  var RELEASES = 'https://github.com/' + REPO + '/releases';
  var OS_NAME = { windows: 'Windows', macos: 'macOS', linux: 'Linux' };
  var $ = function (id) { return document.getElementById(id); };

  function detectSystem() {
    var ua = navigator.userAgent || '';
    var platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';
    if (/android|iphone|ipad|ipod/i.test(platform + ua) || (/mac/i.test(platform) && navigator.maxTouchPoints > 1)) return { os: 'mobile' };
    if (/win/i.test(platform) || /windows/i.test(ua)) return { os: 'windows' };
    if (/mac/i.test(platform) || /macintosh/i.test(ua)) return { os: 'macos' };
    if (/linux|x11|cros/i.test(platform + ua)) {
      return { os: 'linux', arm: /aarch64|arm64|armv[78]/i.test(platform + ua) };
    }
    return { os: 'unknown' };
  }

  // Tauri updater bundles are not standalone installers.
  function classify(asset) {
    var n = asset.name || '';
    if (/\.app\.tar\.gz$|\.sig$|latest\.json$/i.test(n)) return null;
    if (/-setup\.exe$/i.test(n) || /\.exe$/i.test(n)) return { os: 'windows', label: '.exe installer', note: 'Windows, x64', rank: 1 };
    if (/\.msi$/i.test(n)) return { os: 'windows', label: '.msi installer', note: 'Windows, x64 · managed installs', rank: 2 };
    if (/(?:aarch64|arm64)\.dmg$/i.test(n)) return { os: 'macos', label: 'Apple Silicon', note: 'macOS, Apple Silicon', rank: 1 };
    if (/\.dmg$/i.test(n)) return { os: 'macos', label: 'Intel', note: 'macOS, Intel', rank: 2 };
    if (/\.AppImage$/i.test(n)) return { os: 'linux', label: 'AppImage', note: 'Linux, x86_64', rank: 1 };
    if (/\.deb$/i.test(n)) return { os: 'linux', label: '.deb package', note: 'Debian / Ubuntu, x86_64', rank: 2 };
    if (/\.rpm$/i.test(n)) return { os: 'linux', label: '.rpm package', note: 'Fedora / RHEL, x86_64', rank: 3 };
    return null;
  }

  function size(bytes) {
    if (!bytes) return '';
    return bytes >= 1e9 ? (bytes / 1e9).toFixed(2) + ' GB' : (bytes / 1e6).toFixed(bytes >= 1e8 ? 0 : 1) + ' MB';
  }
  function safeUrl(url) {
    return typeof url === 'string' && url.startsWith(RELEASES + '/download/') ? url : RELEASES + '/latest';
  }
  function el(tag, cls, value) {
    var node = document.createElement(tag);
    if (cls) node.className = cls;
    if (value != null) node.textContent = value;
    return node;
  }
  function link(href, value, cls) {
    var node = el('a', cls, value);
    node.href = safeUrl(href);
    return node;
  }
  function fail(message) {
    $('dl-status').textContent = message;
  }

  function render(release) {
    var files = (release.assets || []).map(function (asset) {
      var type = classify(asset);
      return type && { asset: asset, type: type };
    }).filter(Boolean);
    if (!files.length) return fail('This release has no installers yet. Open the release on GitHub for details.');

    var box = $('download');
    box.textContent = '';
    var version = String(release.tag_name || '').replace(/^v/, '');
    box.appendChild(el('p', 'ver', 'Latest release · DBFlow ' + version));
    box.appendChild(el('p', 'picker-label', 'Choose your system'));
    var picker = el('div', 'os-picker');
    picker.setAttribute('role', 'group');
    picker.setAttribute('aria-label', 'Choose your system');
    box.appendChild(picker);
    var choice = el('div', 'download-choice');
    box.appendChild(choice);
    var detected = detectSystem();
    var selected = OS_NAME[detected.os] ? detected.os : null;

    function show(os) {
      selected = os;
      Array.prototype.forEach.call(picker.children, function (button) {
        button.setAttribute('aria-pressed', String(button.dataset.os === os));
      });
      choice.textContent = '';
      var mine = files.filter(function (file) { return file.type.os === os; })
        .sort(function (a, b) { return a.type.rank - b.type.rank; });
      if (!mine.length) {
        choice.appendChild(el('p', 'meta', 'No ' + OS_NAME[os] + ' installer is available in this release.'));
        return;
      }
      if (os === 'linux' && detected.os === 'linux' && detected.arm) {
        choice.appendChild(el('p', 'compat-note', 'Your device appears to use ARM. The current Linux installers are x86_64 and may not run on it.'));
      }
      if (os === 'macos') {
        choice.appendChild(el('p', 'meta', 'Apple menu → About This Mac shows whether your Mac uses Apple Silicon or Intel.'));
      }
      var buttons = el('div', 'download-options');
      mine.forEach(function (file, index) {
        var item = el('div', 'download-option');
        var title = os === 'windows' && index === 0 ? 'Download for Windows' : 'Download ' + file.type.label;
        item.appendChild(link(file.asset.browser_download_url, title, 'btn' + (index ? ' alt' : '')));
        item.appendChild(el('span', 'option-meta', file.type.note + ' · ' + size(file.asset.size)));
        buttons.appendChild(item);
      });
      choice.appendChild(buttons);
      if (os === 'windows' || os === 'macos') {
        choice.appendChild(el('p', 'warn', os === 'windows'
          ? 'First launch: Windows may show a warning because the installer is unsigned. Choose More info, then Run anyway.'
          : 'First launch: the app is not notarized. Right-click it and choose Open.'));
      }
      var instructions = $('n-' + os);
      if (instructions) instructions.open = true;
    }

    ['windows', 'macos', 'linux'].forEach(function (os) {
      var button = el('button', 'os-button', OS_NAME[os]);
      button.type = 'button';
      button.dataset.os = os;
      button.setAttribute('aria-pressed', 'false');
      button.addEventListener('click', function () { show(os); });
      picker.appendChild(button);
    });
    if (selected) show(selected);
    else choice.appendChild(el('p', 'meta', detected.os === 'mobile'
      ? 'DBFlow is a desktop app. Choose the system on the computer where you will install it.'
      : 'Choose a system above to see its installers.'));

    var body = $('files');
    body.textContent = '';
    ['windows', 'macos', 'linux'].forEach(function (os) {
      files.filter(function (file) { return file.type.os === os; })
        .sort(function (a, b) { return a.type.rank - b.type.rank; })
        .forEach(function (file) {
          var row = el('tr');
          var system = el('td');
          if (os === detected.os) system.appendChild(el('span', 'you', 'Your system'));
          system.appendChild(el('strong', null, OS_NAME[os]));
          system.appendChild(el('div', 'muted small', file.type.note));
          var name = el('td');
          name.appendChild(link(file.asset.browser_download_url, file.asset.name));
          var digest = el('td', 'sum');
          var hash = String(file.asset.digest || '').replace(/^sha256:/, '');
          if (/^[0-9a-f]{64}$/i.test(hash)) {
            digest.appendChild(document.createTextNode(hash));
            var copy = el('button', 'copy', 'Copy');
            copy.type = 'button';
            copy.setAttribute('aria-label', 'Copy SHA-256 of ' + file.asset.name);
            copy.addEventListener('click', function () {
              if (navigator.clipboard) navigator.clipboard.writeText(hash).then(function () {
                copy.textContent = 'Copied';
                setTimeout(function () { copy.textContent = 'Copy'; }, 1500);
              });
            });
            digest.appendChild(copy);
          } else digest.textContent = 'not listed';
          row.appendChild(system);
          row.appendChild(name);
          row.appendChild(el('td', 'nowrap', size(file.asset.size)));
          row.appendChild(digest);
          body.appendChild(row);
        });
    });
    $('more').hidden = false;
    $('size-note').textContent = 'Installers include the models that run on your computer.';
  }

  ['3nf', 'bcnf'].forEach(function (key) {
    var button = $('b-' + key);
    if (!button) return;
    button.addEventListener('click', function () {
      ['3nf', 'bcnf'].forEach(function (other) {
        $('p-' + other).hidden = other !== key;
        $('b-' + other).setAttribute('aria-pressed', String(other === key));
      });
    });
  });

  fetch('https://api.github.com/repos/' + REPO + '/releases/latest', {
    headers: { Accept: 'application/vnd.github+json' }
  }).then(function (response) {
    if (!response.ok) throw new Error(String(response.status));
    return response.json();
  }).then(render).catch(function (error) {
    fail(String(error && error.message) === '403'
      ? 'GitHub is limiting requests from your network right now.'
      : 'Could not look up the latest version.');
  });
})();
