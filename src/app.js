import { APP_META, DRAWERS, VIEWS, applyStatic, detectLanguage, language, setLanguage, t } from './i18n.js';
import { renderMarkdown } from './markdown.js';

const tauri = window.__TAURI__;
const invoke = (cmd, args) => tauri.core.invoke(cmd, args);

const $ = (sel, root = document) => root.querySelector(sel);
const esc = (s) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const store = {
  get(key, fallback) {
    try {
      return localStorage.getItem(key) ?? fallback;
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* storage unavailable: keep the choice for this session only */
    }
  },
};

const state = {
  env: null,
  installed: {},
  releases: {}, // id -> release view
  errors: {}, // id -> error
  tasks: {}, // id -> { phase, received, total }
  drawer: store.get('co.drawer', 'all'),
  view: null,
  sheetId: null,
  loading: true,
};

// ---------------------------------------------------------------- version helpers

function parseVersion(raw) {
  const m = String(raw ?? '').match(/(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:-([0-9A-Za-z.-]+))?/);
  if (!m) return null;
  return { nums: [Number(m[1]), Number(m[2] ?? 0), Number(m[3] ?? 0)], pre: m[4] ?? '' };
}

function isNewer(latest, installed) {
  const a = parseVersion(latest);
  const b = parseVersion(installed);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i += 1) {
    if (a.nums[i] !== b.nums[i]) return a.nums[i] > b.nums[i];
  }
  if (a.pre === b.pre) return false;
  if (!a.pre) return true; // release beats pre-release
  if (!b.pre) return false;
  return a.pre > b.pre;
}

function statusOf(id) {
  const inst = state.installed[id];
  const rel = state.releases[id];
  if (inst) {
    if (rel && inst.version && isNewer(rel.version, inst.version)) return 'update';
    return 'current';
  }
  if (!rel) return state.loading ? 'unknown' : 'missing';
  if (!rel.asset || !state.env?.can_install) return 'unavailable';
  return 'missing';
}

// ---------------------------------------------------------------- formatting

function locale() {
  return language() === 'ko' ? 'ko-KR' : 'en-US';
}

function formatDate(iso, opts = { year: 'numeric', month: 'short', day: 'numeric' }) {
  if (!iso) return '';
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? '' : new Intl.DateTimeFormat(locale(), opts).format(d);
}

function formatBytes(n) {
  if (!n) return '';
  return t('bytes.mb', { n: (n / (1024 * 1024)).toFixed(n > 100 * 1024 * 1024 ? 0 : 1) });
}

function greetingKey(hour) {
  if (hour >= 5 && hour < 12) return 'greet.morning';
  if (hour >= 12 && hour < 18) return 'greet.afternoon';
  if (hour >= 18 && hour < 23) return 'greet.evening';
  return 'greet.night';
}

function errorText(err) {
  const code = err?.code || 'io';
  const text = t(`error.${code}`);
  return text.startsWith('error.') ? String(err?.detail || err) : text;
}

// ---------------------------------------------------------------- rendering

function apps() {
  return state.env ? state.env.apps : [];
}

function visibleApps() {
  return apps().filter((app) => {
    const meta = APP_META[app.id];
    if (state.view) {
      const st = statusOf(app.id);
      if (state.view === 'installed' && !state.installed[app.id]) return false;
      if (state.view === 'updates' && st !== 'update') return false;
      if (state.view === 'missing' && state.installed[app.id]) return false;
      return true;
    }
    return state.drawer === 'all' || meta?.drawer === state.drawer;
  });
}

function countFor(key) {
  if (DRAWERS.includes(key)) {
    return key === 'all' ? apps().length : apps().filter((a) => APP_META[a.id]?.drawer === key).length;
  }
  if (key === 'installed') return apps().filter((a) => state.installed[a.id]).length;
  if (key === 'updates') return apps().filter((a) => statusOf(a.id) === 'update').length;
  return apps().filter((a) => !state.installed[a.id]).length;
}

const DRAWER_ICONS = {
  all: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18M3 15h18M10 7h4"/>',
  docs: '<path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5M10 13h6M10 17h6"/>',
  sheets: '<rect x="3" y="4" width="18" height="16" rx="1.5"/><path d="M3 9h18M3 14h18M9 4v16M15 4v16"/>',
  slides: '<rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M12 16v4M8 20h8M7 12l3-3 2 2 4-4"/>',
  pdf: '<path d="M6 3h9l4 4v14H6z"/><path d="M9 13h2a1.5 1.5 0 0 0 0-3H9v6M15 10v6M15 10h2.5M15 13h2"/>',
  media: '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="m4 18 5-5 3 3 3-3 5 5"/>',
  installed: '<path d="M5 12.5 10 17 19 7"/>',
  updates: '<path d="M12 19V6M6 11l6-6 6 6"/>',
  missing: '<path d="M12 5v11M7 11l5 5 5-5M5 20h14"/>',
};

function renderNav() {
  const item = (key, label, active) => `
    <button type="button" class="drawer${active ? ' active' : ''}" data-key="${key}">
      <svg viewBox="0 0 24 24" aria-hidden="true">${DRAWER_ICONS[key]}</svg>
      <span>${esc(label)}</span>
      <span class="badge">${countFor(key)}</span>
    </button>`;
  $('#drawers').innerHTML =
    `<h2>${esc(t('nav.drawers'))}</h2>` +
    DRAWERS.map((k) => item(k, t(`drawer.${k}`), !state.view && state.drawer === k)).join('');
  $('#views').innerHTML =
    `<h2>${esc(t('nav.views'))}</h2>` + VIEWS.map((k) => item(k, t(`view.${k}`), state.view === k)).join('');
}

function primaryAction(id) {
  const st = statusOf(id);
  const task = state.tasks[id];
  if (task) return { label: phaseLabel(task), action: null, busy: true };
  if (st === 'update') return { label: t('action.update'), action: 'install', kind: 'accent' };
  if (st === 'current') return { label: t('action.open'), action: 'open', kind: 'primary' };
  if (st === 'missing') return { label: t('action.install'), action: 'install', kind: 'primary' };
  if (st === 'unavailable') return { label: t('action.releases'), action: 'releases', kind: 'quiet' };
  return { label: t('status.unknown'), action: null, kind: 'quiet' };
}

function phaseLabel(task) {
  if (task.phase === 'download') {
    const pct = task.total ? `${Math.floor((task.received / task.total) * 100)}%` : '';
    return t('phase.download', { pct });
  }
  return t(`phase.${task.phase}`);
}

function progressValue(task) {
  if (!task) return 0;
  if (task.phase === 'download') return task.total ? (task.received / task.total) * 0.85 : 0.1;
  if (task.phase === 'verify') return 0.88;
  if (task.phase === 'install') return 0.94;
  return 1;
}

function card(app) {
  const meta = APP_META[app.id] || {};
  const st = statusOf(app.id);
  const inst = state.installed[app.id];
  const rel = state.releases[app.id];
  const task = state.tasks[app.id];
  const act = primaryAction(app.id);
  const tagline = meta.tagline?.[language()] ?? '';
  const latest = rel ? `${esc(rel.version)}<span class="when">${esc(formatDate(rel.published_at, { month: 'short', day: 'numeric' }))}</span>` : '—';
  const installedText = inst ? esc(inst.version || t('label.unknownVersion')) : '—';
  const secondary =
    st === 'update' && inst?.path && !task
      ? `<button type="button" class="btn quiet" data-act="open" data-id="${app.id}">${esc(t('action.open'))}</button>`
      : '';

  return `
  <article class="card status-${st}${task ? ' working' : ''}" style="--app:${meta.color || '#1d5c63'}" data-id="${app.id}">
    <div class="tab"><span>${esc(t(`drawer.${meta.drawer || 'all'}`))}</span></div>
    <div class="card-body">
      <div class="card-top">
        <img class="app-icon" src="app-icons/${app.id}.png" alt="" width="56" height="56">
        <div class="titles">
          <h3>${esc(app.name)}</h3>
          <p class="byline">${esc(app.publisher)}${app.maker === 'partner' ? ` · <span class="partner">${esc(t('label.partner'))}</span>` : ''}</p>
        </div>
        <span class="pill pill-${st}">${esc(t(`status.${st}`))}</span>
      </div>
      <p class="tagline">${esc(tagline)}</p>
      <dl class="versions">
        <div><dt>${esc(t('label.installed'))}</dt><dd class="mono">${installedText}</dd></div>
        <div><dt>${esc(t('label.latest'))}</dt><dd class="mono">${latest}</dd></div>
      </dl>
      <div class="actions">
        <button type="button" class="btn ${act.kind || 'quiet'}" ${act.action ? `data-act="${act.action}"` : 'disabled'} data-id="${app.id}">${esc(act.label)}</button>
        ${secondary}
        <button type="button" class="btn ghost more" data-act="details" data-id="${app.id}">${esc(t('action.details'))}</button>
      </div>
    </div>
    <div class="progress" aria-hidden="true"><span style="transform:scaleX(${progressValue(task)})"></span></div>
  </article>`;
}

function renderCabinet() {
  const list = visibleApps();
  $('#cabinet').innerHTML = list.map(card).join('');
  const empty = $('#empty');
  empty.hidden = list.length > 0;
  empty.textContent = t('empty.filter');
}

function renderHeader() {
  const now = new Date();
  $('#today').textContent = new Intl.DateTimeFormat(locale(), { month: 'long', day: 'numeric', weekday: 'long' }).format(now);
  $('#greeting').textContent = t(greetingKey(now.getHours()));
  if (state.loading && !Object.keys(state.releases).length) {
    $('#summary').textContent = t('summary.loading');
  } else {
    const total = apps().length;
    const installed = countFor('installed');
    const updates = countFor('updates');
    $('#summary').textContent =
      t('summary.text', { total, installed }) +
      (updates ? t('summary.updates', { n: updates }) : installed ? t('summary.allFresh') : '');
  }
  const updates = apps().filter((a) => statusOf(a.id) === 'update' && !state.tasks[a.id]);
  $('#update-all').hidden = updates.length < 1 || !state.env?.can_install;
  $('#update-count').textContent = updates.length;
  $('#refresh').classList.toggle('spinning', state.loading);

  const self = state.releases['chojema-office'];
  const banner = $('#self-update');
  if (self && state.env && isNewer(self.version, state.env.version)) {
    banner.hidden = false;
    $('#self-update-text').textContent = t('self.available', { version: self.version, current: state.env.version });
    $('#self-update-run').hidden = !self.asset || !state.env.can_install;
  } else {
    banner.hidden = true;
  }
}

function renderSheet() {
  const id = state.sheetId;
  const sheet = $('#sheet');
  if (!id) {
    sheet.classList.remove('open');
    sheet.setAttribute('aria-hidden', 'true');
    return;
  }
  const isSelf = id === 'chojema-office';
  const app = isSelf
    ? { id, name: 'Chojema Office', publisher: 'Chulwoo Park', maker: 'chojema', repo_url: 'https://github.com/chojema/chojema-office', releases_url: 'https://github.com/chojema/chojema-office/releases' }
    : apps().find((a) => a.id === id);
  if (!app) return;
  const meta = APP_META[id] || { color: '#1d5c63' };
  const rel = state.releases[id];
  const inst = state.installed[id];
  const err = state.errors[id];
  const act = isSelf ? null : primaryAction(id);
  const facts = [];
  facts.push([t('label.publisher'), esc(app.publisher) + (app.maker === 'partner' ? ` <span class="partner">${esc(t('label.partner'))}</span>` : '')]);
  if (!isSelf) facts.push([t('label.installed'), inst ? `<span class="mono">${esc(inst.version || t('label.unknownVersion'))}</span>` : '—']);
  if (rel) {
    facts.push([t('label.latest'), `<span class="mono">${esc(rel.version)}</span>`]);
    facts.push([t('label.released'), esc(formatDate(rel.published_at))]);
    if (rel.asset) facts.push([t('label.file'), `<span class="mono small">${esc(rel.asset)}</span>${rel.asset_size ? ` · ${esc(formatBytes(rel.asset_size))}` : ''}`]);
    facts.push([t('label.checksum'), esc(rel.has_checksums ? t('label.checksumYes') : t('label.checksumNo'))]);
  }
  if (inst?.path) facts.push([t('label.location'), `<span class="mono small">${esc(inst.path)}</span>`]);

  $('#sheet-body').innerHTML = `
    <header class="sheet-head" style="--app:${meta.color}">
      <img src="${isSelf ? 'favicon.png' : `app-icons/${id}.png`}" alt="" width="72" height="72">
      <div>
        <h2 id="sheet-title">${esc(app.name)}</h2>
        <p>${esc(isSelf ? t('about.lead') : meta.tagline?.[language()] ?? '')}</p>
      </div>
    </header>
    <div class="sheet-actions">
      ${act ? `<button type="button" class="btn ${act.kind || 'quiet'}" ${act.action ? `data-act="${act.action}"` : 'disabled'} data-id="${id}">${esc(act.label)}</button>` : ''}
      ${!isSelf && statusOf(id) === 'update' && inst?.path ? `<button type="button" class="btn quiet" data-act="open" data-id="${id}">${esc(t('action.open'))}</button>` : ''}
      <button type="button" class="btn ghost" data-url="${esc(app.releases_url)}">${esc(t('action.releases'))}</button>
      <button type="button" class="btn ghost" data-url="${esc(app.repo_url)}">${esc(t('action.repo'))}</button>
    </div>
    <dl class="facts">${facts.map(([k, v]) => `<div><dt>${esc(k)}</dt><dd>${v}</dd></div>`).join('')}</dl>
    ${err && !rel ? `<p class="sheet-error">${esc(errorText(err))}</p>` : ''}
    ${rel ? `<section class="notes"><h3>${esc(t('action.notes'))} · ${esc(rel.name)}</h3><div class="md">${renderMarkdown(rel.body)}</div></section>` : ''}
  `;
  sheet.classList.add('open');
  sheet.setAttribute('aria-hidden', 'false');
}

function renderSettings() {
  const langChoice = store.get('co.lang', 'auto');
  document.querySelectorAll('[data-lang]').forEach((b) => b.classList.toggle('on', b.dataset.lang === (langChoice === 'auto' ? language() : langChoice)));
  const theme = store.get('co.theme', 'auto');
  document.querySelectorAll('[data-theme-choice]').forEach((b) => b.classList.toggle('on', b.dataset.themeChoice === theme));
}

function render() {
  renderNav();
  renderHeader();
  renderCabinet();
  renderSettings();
  if (state.sheetId) renderSheet();
}

// ---------------------------------------------------------------- toasts

function toast(message, kind = 'info', ms = 4200) {
  const el = document.createElement('div');
  el.className = `toast ${kind}`;
  el.textContent = message;
  $('#toasts').appendChild(el);
  requestAnimationFrame(() => el.classList.add('in'));
  setTimeout(() => {
    el.classList.remove('in');
    setTimeout(() => el.remove(), 300);
  }, ms);
}

// ---------------------------------------------------------------- actions

async function loadInstalled() {
  try {
    state.installed = await invoke('installed_apps');
  } catch (e) {
    console.error(e);
  }
}

async function refresh() {
  state.loading = true;
  render();
  await loadInstalled();
  render();
  let failures = 0;
  let rateLimited = false;
  try {
    const results = await invoke('refresh_releases');
    for (const [id, r] of Object.entries(results)) {
      if (r.state === 'ok') {
        const { state: _s, ...view } = r;
        state.releases[id] = view;
        delete state.errors[id];
      } else {
        state.errors[id] = r.error;
        if (r.error?.code !== 'no_release') failures += 1;
        if (r.error?.code === 'rate_limited') rateLimited = true;
      }
    }
  } catch (e) {
    failures = apps().length;
    console.error(e);
  }
  state.loading = false;
  const offline = $('#offline');
  offline.hidden = failures === 0;
  $('#offline-text').textContent = rateLimited ? t('offline.rate') : t('offline.text');
  render();
}

async function install(id, { quiet = false } = {}) {
  const app = apps().find((a) => a.id === id);
  if (!app || state.tasks[id]) return false;
  const wasInstalled = Boolean(state.installed[id]);
  state.tasks[id] = { phase: 'download', received: 0, total: 0 };
  render();
  try {
    const outcome = await invoke('install_app', { id, silent: true });
    delete state.tasks[id];
    if (outcome.installed) state.installed[id] = outcome.installed;
    else await loadInstalled();
    const version = state.installed[id]?.version || state.releases[id]?.version || '';
    toast(wasInstalled ? t('toast.updated', { name: app.name, version }) : t('toast.installed', { name: app.name }), 'ok');
    if (outcome.verified === 'unchecked' && !quiet) toast(t('toast.unchecked'), 'warn', 6000);
    return true;
  } catch (e) {
    delete state.tasks[id];
    toast(`${app.name}: ${errorText(e)}`, e?.code === 'cancelled' ? 'warn' : 'error', 6000);
    return false;
  } finally {
    render();
  }
}

async function updateAll() {
  const ids = apps().filter((a) => statusOf(a.id) === 'update').map((a) => a.id);
  for (const id of ids) {
    // One at a time: installers cannot run side by side.
    // eslint-disable-next-line no-await-in-loop
    await install(id, { quiet: true });
  }
}

async function openApp(id) {
  const app = apps().find((a) => a.id === id);
  try {
    await invoke('launch_app', { id });
    toast(t('toast.opened', { name: app?.name ?? id }), 'info', 2400);
  } catch (e) {
    toast(`${app?.name ?? id}: ${errorText(e)}`, 'error');
    await loadInstalled();
    render();
  }
}

async function openUrl(url) {
  try {
    await invoke('open_external', { url });
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

function runAction(act, id) {
  const app = apps().find((a) => a.id === id);
  switch (act) {
    case 'install':
      return install(id);
    case 'open':
      return openApp(id);
    case 'details':
      state.sheetId = id;
      return renderSheet();
    case 'releases':
      return app && openUrl(app.releases_url);
    default:
      return undefined;
  }
}

// ---------------------------------------------------------------- palette

let paletteIndex = 0;

function paletteItems(query) {
  const q = query.trim().toLowerCase();
  return apps()
    .map((app) => {
      const meta = APP_META[app.id] || {};
      const hay = [app.name, meta.tagline?.ko, meta.tagline?.en, t(`drawer.${meta.drawer}`), app.publisher].join(' ').toLowerCase();
      let score = 0;
      if (!q) score = 1;
      else if (app.name.toLowerCase().startsWith(q)) score = 3;
      else if (app.name.toLowerCase().includes(q)) score = 2;
      else if (hay.includes(q)) score = 1;
      return { app, score };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .map((x) => x.app);
}

function renderPalette() {
  const items = paletteItems($('#palette-input').value);
  paletteIndex = Math.max(0, Math.min(paletteIndex, items.length - 1));
  $('#palette-list').innerHTML = items.length
    ? items
        .map((app, i) => {
          const act = primaryAction(app.id);
          return `<li role="option" aria-selected="${i === paletteIndex}" data-id="${app.id}" class="${i === paletteIndex ? 'sel' : ''}">
            <img src="app-icons/${app.id}.png" alt="" width="28" height="28">
            <span class="p-name">${esc(app.name)}</span>
            <span class="p-tag">${esc(APP_META[app.id]?.tagline?.[language()] ?? '')}</span>
            <span class="p-act">${esc(act.label)}</span>
          </li>`;
        })
        .join('')
    : `<li class="none">${esc(t('palette.none'))}</li>`;
}

function openPalette() {
  const p = $('#palette');
  p.hidden = false;
  const input = $('#palette-input');
  input.placeholder = t('palette.placeholder');
  input.value = '';
  paletteIndex = 0;
  renderPalette();
  input.focus();
}

function closePalette() {
  $('#palette').hidden = true;
}

function runPalette(id) {
  closePalette();
  const act = primaryAction(id);
  if (act.action) runAction(act.action, id);
  else runAction('details', id);
}

// ---------------------------------------------------------------- about

const MAKER_LINKS = [
  ['link.blog', 'https://cantips.com', 'cantips.com'],
  ['link.youtube', 'https://www.youtube.com/@cantips', '@cantips'],
  ['link.x', 'https://twitter.com/chojema', '@chojema'],
  ['link.instagram', 'https://www.instagram.com/chojema', '@chojema'],
  ['link.github', 'https://github.com/chojema', 'chojema'],
  ['link.home', 'http://ilovemis.com/', 'ilovemis.com'],
];

function linkTile(label, url, sub) {
  return `<button type="button" class="link-tile" data-url="${esc(url)}"><strong>${esc(label)}</strong><span>${esc(sub)}</span></button>`;
}

function openAbout() {
  $('#about-body').innerHTML = `
    <div class="about-head">
      <img src="favicon.png" alt="" width="56" height="56">
      <div><h2>${esc(t('about.title'))}</h2><p class="mono">v${esc(state.env?.version ?? '')}</p></div>
    </div>
    <p>${esc(t('about.lead'))}</p>
    <p><strong>${esc(t('about.author'))}</strong><br>${esc(t('about.license'))}</p>
    <h3>${esc(t('about.links'))}</h3>
    <div class="link-grid wide">
      ${linkTile(t('about.repo'), 'https://github.com/chojema/chojema-office', 'github.com/chojema/chojema-office')}
      ${linkTile(t('about.appPark'), 'https://chojema.github.io/AppPark/', t('about.appParkText'))}
    </div>
    <h3>${esc(t('about.maker'))} · Chulwoo Park</h3>
    <div class="link-grid">
      ${MAKER_LINKS.map(([key, url, sub]) => linkTile(t(key), url, sub)).join('')}
    </div>
    <h3>${esc(t('about.partners'))}</h3>
    <p>${esc(t('about.partnersText'))}</p>
    <p class="muted">${esc(t('about.inspired'))}</p>
    <p class="muted">${esc(t('about.data'))}</p>`;
  $('#about').showModal();
}

// ---------------------------------------------------------------- settings

function applyTheme() {
  document.documentElement.dataset.theme = store.get('co.theme', 'auto');
}

function applyLanguage() {
  const choice = store.get('co.lang', 'auto');
  setLanguage(choice === 'auto' ? detectLanguage() : choice);
  applyStatic();
  if (state.env) $('#app-version').textContent = `v${state.env.version}`;
}

async function loadAutostart() {
  try {
    $('#autostart').checked = await invoke('autostart_enabled');
    $('#autostart-row').hidden = false;
  } catch (e) {
    console.warn('autostart unavailable', e);
  }
}

async function changeAutostart(e) {
  const box = e.target;
  const want = box.checked;
  box.disabled = true;
  try {
    box.checked = await invoke('set_autostart', { enabled: want });
    toast(t(box.checked ? 'toast.autostartOn' : 'toast.autostartOff'), 'info');
  } catch (err) {
    box.checked = !want;
    toast(errorText(err), 'error');
  } finally {
    box.disabled = false;
  }
}

// ---------------------------------------------------------------- wiring

function wire() {
  document.addEventListener('click', (e) => {
    const urlEl = e.target.closest('[data-url]');
    if (urlEl) {
      e.preventDefault();
      openUrl(urlEl.dataset.url);
      return;
    }
    const nav = e.target.closest('.drawer');
    if (nav) {
      const key = nav.dataset.key;
      if (DRAWERS.includes(key)) {
        state.drawer = key;
        state.view = null;
        store.set('co.drawer', key);
      } else {
        state.view = state.view === key ? null : key;
      }
      render();
      return;
    }
    const actEl = e.target.closest('[data-act]');
    if (actEl && !actEl.disabled) {
      runAction(actEl.dataset.act, actEl.dataset.id);
      return;
    }
    const cardEl = e.target.closest('.card');
    if (cardEl && !e.target.closest('button')) {
      state.sheetId = cardEl.dataset.id;
      renderSheet();
      return;
    }
    if (e.target.closest('[data-close-sheet]')) {
      state.sheetId = null;
      renderSheet();
      return;
    }
    if (e.target.closest('[data-close-palette]')) closePalette();
    const li = e.target.closest('#palette-list li[data-id]');
    if (li) runPalette(li.dataset.id);
    const lang = e.target.closest('[data-lang]');
    if (lang) {
      store.set('co.lang', lang.dataset.lang);
      applyLanguage();
      render();
    }
    const theme = e.target.closest('[data-theme-choice]');
    if (theme) {
      store.set('co.theme', theme.dataset.themeChoice);
      applyTheme();
      renderSettings();
    }
  });

  $('#refresh').addEventListener('click', () => !state.loading && refresh());
  $('#update-all').addEventListener('click', updateAll);
  $('#palette-open').addEventListener('click', openPalette);
  $('#about-open').addEventListener('click', openAbout);
  $('#autostart').addEventListener('change', changeAutostart);
  $('#self-update-notes').addEventListener('click', () => {
    state.sheetId = 'chojema-office';
    renderSheet();
  });
  $('#self-update-run').addEventListener('click', async () => {
    toast(t('toast.selfUpdating'), 'info', 8000);
    try {
      await invoke('update_self');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  });

  $('#palette-input').addEventListener('input', () => {
    paletteIndex = 0;
    renderPalette();
  });
  $('#palette-input').addEventListener('keydown', (e) => {
    const items = paletteItems(e.target.value);
    if (e.key === 'ArrowDown') {
      paletteIndex = Math.min(items.length - 1, paletteIndex + 1);
      renderPalette();
      e.preventDefault();
    } else if (e.key === 'ArrowUp') {
      paletteIndex = Math.max(0, paletteIndex - 1);
      renderPalette();
      e.preventDefault();
    } else if (e.key === 'Enter' && items[paletteIndex]) {
      runPalette(items[paletteIndex].id);
    }
  });

  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      if ($('#palette').hidden) openPalette();
      else closePalette();
    } else if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r')) {
      e.preventDefault();
      if (!state.loading) refresh();
    } else if (e.key === 'Escape') {
      if (!$('#palette').hidden) closePalette();
      else if (state.sheetId) {
        state.sheetId = null;
        renderSheet();
      }
    }
  });

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', renderSettings);
  // Re-check installed apps when the window comes back to the front (the user may have installed
  // or removed something by hand).
  window.addEventListener('focus', async () => {
    if (state.loading || Object.keys(state.tasks).length) return;
    await loadInstalled();
    render();
  });
  setInterval(renderHeader, 60 * 1000);

  tauri.event.listen('task-progress', ({ payload }) => {
    if (!payload || !state.tasks[payload.id]) return;
    state.tasks[payload.id] = { phase: payload.phase, received: payload.received, total: payload.total };
    // Cheap update: only the affected card's progress and label.
    const cardEl = document.querySelector(`.card[data-id="${CSS.escape(payload.id)}"]`);
    if (cardEl) {
      cardEl.querySelector('.progress span').style.transform = `scaleX(${progressValue(state.tasks[payload.id])})`;
      const btn = cardEl.querySelector('.actions .btn');
      if (btn) btn.textContent = phaseLabel(state.tasks[payload.id]);
    }
  });
}

async function start() {
  applyTheme();
  applyLanguage();
  wire();
  try {
    state.env = await invoke('environment');
  } catch (e) {
    console.error(e);
    return;
  }
  $('#app-version').textContent = `v${state.env.version}`;
  if (!state.env.can_install) toast(t('unsupported.os'), 'warn', 8000);
  loadAutostart();
  try {
    state.releases = await invoke('cached_releases');
  } catch {
    state.releases = {};
  }
  render();
  await refresh();
}

start();
