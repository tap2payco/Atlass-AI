// Atlas app: a client of the same agent and saved conversations as
// `atlas agent`. Talks to `atlas-cli server` (started by `atlas ui`).
'use strict';

const $ = (sel) => document.querySelector(sel);
const API = location.protocol.startsWith('http') ? location.origin : 'http://localhost:8080';

const state = {
  project: '',          // absolute project folder
  projectName: '',
  session: null,        // saved conversation id
  access: 'read',       // 'read' | 'write'
  busy: false,
  steps: new Map(),     // call_id -> { el, tool, args, output, isError, effect, started }
  reviewArgs: new Map(), // call_id -> arguments shown in its review card
  selectedStep: null,
  terminals: [],        // { id, shell, view, term, fit, exited, abort }
  activeTerminal: null,
};

const store = {
  get(key) { try { return localStorage.getItem(key); } catch { return null; } },
  set(key, value) { try { localStorage.setItem(key, value); } catch {} },
};

// ── Helpers ────────────────────────────────────────────────────────────────

function escapeHtml(text) {
  return String(text ?? '')
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

function icon(name) { return `<svg aria-hidden="true"><use href="#i-${name}"/></svg>`; }

function toast(message) {
  const el = $('#toast');
  el.textContent = message;
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => { el.hidden = true; }, 3200);
}

function relativeTime(seconds) {
  if (!seconds) return '';
  const diff = Date.now() / 1000 - seconds;
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} h ago`;
  if (diff < 7 * 86400) return `${Math.floor(diff / 86400)} d ago`;
  return new Date(seconds * 1000).toLocaleDateString();
}

function clock() {
  return new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

async function api(path, options = {}) {
  const res = await fetch(API + path, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...(options.headers || {}) },
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || `${res.status} ${res.statusText}`);
  return data;
}

// Markdown subset: fenced code, inline code, bold, headings, lists, links.
// Model output is untrusted, so everything is escaped first.
function renderMarkdown(text) {
  const blocks = [];
  let src = String(text ?? '').replace(/```([\w+-]*)\n([\s\S]*?)```/g, (_, lang, code) => {
    blocks.push({ lang, code: code.replace(/\n$/, '') });
    return `\u0000${blocks.length - 1}\u0000`;
  });
  src = escapeHtml(src);
  const inline = (s) => s
    .replace(/`([^`\n]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*\n]+)\*\*/g, '<b>$1</b>')
    .replace(/\[([^\]\n]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');

  const out = [];
  let list = null;
  const closeList = () => { if (list) { out.push(`</${list}>`); list = null; } };
  for (const line of src.split('\n')) {
    const block = line.match(/^\u0000(\d+)\u0000$/);
    if (block) {
      closeList();
      const { lang, code } = blocks[Number(block[1])];
      out.push(`<div class="codeblock"><div class="codeblock-head"><span>${escapeHtml(lang || 'text')}</span>` +
        `<button class="icon-btn small copy" aria-label="Copy code">${icon('copy')}</button></div>` +
        `<pre><code>${escapeHtml(code)}</code></pre></div>`);
      continue;
    }
    const bullet = line.match(/^\s*[-*]\s+(.*)$/);
    const numbered = line.match(/^\s*\d+[.)]\s+(.*)$/);
    const heading = line.match(/^(#{1,4})\s+(.*)$/);
    if (bullet || numbered) {
      const kind = bullet ? 'ul' : 'ol';
      if (list !== kind) { closeList(); out.push(`<${kind}>`); list = kind; }
      out.push(`<li>${inline((bullet || numbered)[1])}</li>`);
    } else if (heading) {
      closeList();
      out.push(`<h${heading[1].length > 2 ? 4 : 3}>${inline(heading[2])}</h${heading[1].length > 2 ? 4 : 3}>`);
    } else if (line.trim() === '') {
      closeList();
      out.push('');
    } else {
      closeList();
      out.push(`<p>${inline(line)}</p>`);
    }
  }
  closeList();
  // Leftover placeholders (a code block inside a paragraph line).
  return out.join('\n').replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${escapeHtml(blocks[Number(i)].code)}</code>`);
}

// ── Tools: how each call is described ──────────────────────────────────────

const TOOL_INFO = {
  read_file: { label: 'Read', icon: 'read', effect: 'read' },
  list_files: { label: 'Listed', icon: 'list', effect: 'read' },
  search_files: { label: 'Searched', icon: 'search', effect: 'read' },
  write_file: { label: 'Write', icon: 'edit', effect: 'write' },
  edit_file: { label: 'Edit', icon: 'edit', effect: 'write' },
  run_command: { label: 'Run', icon: 'run', effect: 'run' },
};

function toolInfo(name) {
  return TOOL_INFO[name] || { label: name, icon: 'spark', effect: 'read' };
}

function effectClass(effect) {
  if (effect === 'writes_workspace' || effect === 'write') return 'write';
  if (effect === 'runs_processes' || effect === 'run') return 'run';
  return 'read';
}

function targetOf(args) {
  if (!args || typeof args !== 'object') return '';
  const value = args.path ?? args.command ?? args.query ?? args.pattern;
  if (value !== undefined) return String(value);
  const text = JSON.stringify(args);
  return text === '{}' ? '' : text;
}

function lineCount(text) {
  if (!text) return 0;
  return String(text).replace(/\n$/, '').split('\n').length;
}

// A small line diff for the review card: common prefix and suffix as
// context, the rest as removed then added.
function diffHtml(oldText, newText) {
  const a = String(oldText ?? '').split('\n');
  const b = String(newText ?? '').split('\n');
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length - 1, endB = b.length - 1;
  while (endA >= start && endB >= start && a[endA] === b[endB]) { endA--; endB--; }
  const rows = [];
  const ctx = (line) => rows.push(`<div class="ctx">  ${escapeHtml(line)}</div>`);
  a.slice(Math.max(0, start - 2), start).forEach(ctx);
  a.slice(start, endA + 1).forEach((l) => rows.push(`<div class="del">${escapeHtml(l)}</div>`));
  b.slice(start, endB + 1).forEach((l) => rows.push(`<div class="add">${escapeHtml(l)}</div>`));
  a.slice(endA + 1, endA + 3).forEach(ctx);
  return `<div class="diff">${rows.join('')}</div>`;
}

function changePreview(tool, args) {
  if (tool === 'edit_file') return diffHtml(args.old_text, args.new_text);
  if (tool === 'write_file') {
    const lines = String(args.content ?? '').split('\n');
    const shown = lines.slice(0, 400).map((l) => `<div class="add">${escapeHtml(l)}</div>`).join('');
    const more = lines.length > 400 ? `<div class="ctx">  … ${lines.length - 400} more lines</div>` : '';
    return `<div class="diff">${shown}${more}</div>`;
  }
  if (tool === 'run_command') return `<div class="diff"><div class="ctx">$ ${escapeHtml(args.command ?? '')}</div></div>`;
  return `<div class="diff"><div class="ctx">${escapeHtml(JSON.stringify(args, null, 2))}</div></div>`;
}

// ── Theme ──────────────────────────────────────────────────────────────────

const THEMES = ['system', 'light', 'dark'];
function applyTheme(theme) {
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  const btn = $('#theme-btn');
  btn.innerHTML = icon(theme === 'light' ? 'sun' : theme === 'dark' ? 'moon' : 'monitor');
  btn.setAttribute('aria-label', `Theme: ${theme}. Click to change.`);
  btn.title = `Theme: ${theme}`;
  store.set('atlas.theme', theme);
  for (const t of state.terminals) if (t.term) t.term.options.theme = terminalTheme();
}

// ── Project and conversations ──────────────────────────────────────────────

function setProject(path, name) {
  state.project = path;
  state.projectName = name || path.split(/[\\/]/).filter(Boolean).pop() || path;
  $('#project-name').textContent = state.projectName;
  $('#project-path').textContent = path;
  $('#project-path').title = path;
  $('#empty-project').textContent = state.projectName;
  document.title = `${state.projectName} · Atlas`;
  store.set('atlas.project', path);
}

async function loadSessions() {
  const list = $('#history-list');
  try {
    const data = await api(`/api/sessions?path=${encodeURIComponent(state.project)}`);
    if (!data.sessions.length) {
      list.innerHTML = '<li class="history-empty">No saved conversations yet.</li>';
      return;
    }
    list.innerHTML = data.sessions.map((s) => `
      <li><button class="history-item" data-id="${escapeHtml(s.id)}" aria-current="${s.id === state.session}">
        <span class="history-title">${escapeHtml(s.title || 'Untitled')}</span>
        <span class="history-when">${escapeHtml(relativeTime(s.updated_at))} · ${s.message_count} messages</span>
      </button></li>`).join('');
  } catch (err) {
    list.innerHTML = `<li class="history-empty">Couldn't load conversations: ${escapeHtml(err.message)}</li>`;
  }
}

function markCurrentSession() {
  for (const el of document.querySelectorAll('.history-item')) {
    el.setAttribute('aria-current', String(el.dataset.id === state.session));
  }
  $('#conv-meta').textContent = state.session ? state.session.slice(0, 8) : '';
  if (state.session) history.replaceState(null, '', `#s=${state.session}`);
  else history.replaceState(null, '', location.pathname);
}

function resetThread() {
  state.steps.clear();
  state.selectedStep = null;
  $('#thread').innerHTML = '';
  $('#thread').append(emptyTemplate.cloneNode(true));
  bindStarters();
}

function newConversation() {
  if (state.busy) return toast('Stop the running turn first.');
  state.session = null;
  $('#conv-title').textContent = 'New conversation';
  resetThread();
  markCurrentSession();
  closeNav();
  $('#input').focus();
}

async function openConversation(id) {
  if (state.busy) return toast('Stop the running turn first.');
  closeNav();
  try {
    const data = await api(`/api/session?path=${encodeURIComponent(state.project)}&id=${encodeURIComponent(id)}`);
    state.session = data.id;
    resetThread();
    $('#empty')?.remove();
    renderHistory(data.items);
    const first = data.items.find((i) => i.kind === 'user');
    $('#conv-title').textContent = first ? first.text.slice(0, 80) : 'Conversation';
    markCurrentSession();
    scrollToEnd(false);
  } catch (err) {
    toast(`Couldn't open that conversation: ${err.message}`);
  }
}

function renderHistory(items) {
  let turn = null;
  for (const item of items) {
    if (item.kind === 'user') {
      appendUser(item.text);
      turn = null;
      continue;
    }
    if (!turn) turn = startAgentTurn(false);
    if (item.kind === 'agent') turn.addText(item.text);
    else if (item.kind === 'tool_call') turn.beginStep(item.call_id, item.tool, item.arguments);
    else if (item.kind === 'tool_result') turn.endStep(item.call_id, item.output, item.is_error);
    else if (item.kind === 'compacted') turn.notice('info', `Summarised the ${Number(item.covers)} earlier messages to stay within the context window.`);
  }
}

// ── Thread rendering ───────────────────────────────────────────────────────

function scrollToEnd(smooth = true) {
  const s = $('#scroller');
  s.scrollTo({ top: s.scrollHeight, behavior: smooth ? 'smooth' : 'auto' });
}

function nearBottom() {
  const s = $('#scroller');
  return s.scrollHeight - s.scrollTop - s.clientHeight < 160;
}

function appendUser(text) {
  $('#empty')?.remove();
  const el = document.createElement('div');
  el.className = 'msg msg-user';
  el.innerHTML = `<div class="bubble">${escapeHtml(text)}</div>`;
  $('#thread').append(el);
}

// One agent turn: a header, then interleaved text blocks, step spines,
// review cards and notices, in the order they happen.
function startAgentTurn(live = true) {
  const el = document.createElement('div');
  el.className = 'msg msg-agent';
  el.innerHTML = `<div class="agent-head"><span class="brand-mark" aria-hidden="true"><svg viewBox="0 0 32 32"><path d="M9 23 16 8l7 15M11.8 17.5h8.4"/></svg></span>Atlas<span class="agent-time">${live ? clock() : ''}</span></div><div class="agent-body"></div>`;
  $('#thread').append(el);
  const body = el.querySelector('.agent-body');
  let spine = null;
  let thinking = null;

  const keepScrolled = (fn) => { const stick = nearBottom(); fn(); if (stick) scrollToEnd(); };

  const turn = {
    el,
    setThinking(on, label = 'Thinking') {
      if (on && !thinking) {
        thinking = document.createElement('div');
        thinking.className = 'thinking';
        thinking.innerHTML = `<span class="dots" aria-hidden="true"><i></i><i></i><i></i></span><span>${escapeHtml(label)}…</span>`;
        keepScrolled(() => body.append(thinking));
      } else if (!on && thinking) {
        thinking.remove();
        thinking = null;
      }
    },
    addText(text) {
      if (!text || !text.trim()) return;
      spine = null;
      const p = document.createElement('div');
      p.className = 'prose';
      p.innerHTML = renderMarkdown(text);
      keepScrolled(() => (thinking ? body.insertBefore(p, thinking) : body.append(p)));
    },
    spine() {
      if (!spine) {
        spine = document.createElement('div');
        spine.className = 'steps';
        thinking ? body.insertBefore(spine, thinking) : body.append(spine);
      }
      return spine;
    },
    beginStep(callId, tool, args, effect) {
      const info = toolInfo(tool);
      const eff = effectClass(effect || info.effect);
      let step = state.steps.get(callId);
      if (!step) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = `step e-${eff} running`;
        btn.dataset.call = callId;
        btn.innerHTML = `<span class="step-icon">${icon(info.icon)}</span><span class="step-tool">${escapeHtml(info.label)}</span>` +
          `<span class="step-target">${escapeHtml(targetOf(args))}</span><span class="step-status">running</span>`;
        keepScrolled(() => turn.spine().append(btn));
        step = { el: btn, tool, args, output: null, isError: false, effect: eff, started: performance.now() };
        state.steps.set(callId, step);
      } else {
        step.el.classList.add('running');
        step.el.querySelector('.step-status').textContent = 'running';
      }
      return step;
    },
    endStep(callId, output, isError) {
      const step = state.steps.get(callId) || turn.beginStep(callId, 'tool', {});
      step.output = output;
      step.isError = isError;
      step.el.classList.remove('running');
      step.el.classList.add(isError ? 'failed' : 'done');
      const lines = lineCount(output);
      step.el.querySelector('.step-status').textContent = isError
        ? 'failed'
        : step.effect === 'write' ? 'applied' : `${lines} line${lines === 1 ? '' : 's'}`;
      if (state.selectedStep === callId) showDetails(callId);
    },
    declineStep(callId, tool) {
      const step = state.steps.get(callId) || turn.beginStep(callId, tool, state.reviewArgs.get(callId) || {});
      step.el.classList.remove('running');
      step.el.classList.add('declined');
      step.el.querySelector('.step-status').textContent = 'declined';
    },
    review(event) {
      state.reviewArgs.set(event.call_id, event.arguments || {});
      spine = null;
      const card = document.createElement('div');
      card.className = 'review';
      card.dataset.call = event.call_id;
      const info = toolInfo(event.tool);
      const what = event.tool === 'run_command' ? 'Run this command?' : event.tool === 'write_file' ? 'Write this file?' : 'Apply this change?';
      card.innerHTML = `
        <div class="review-head">${icon('shield')}
          <div><div class="review-title">${escapeHtml(what)}</div>
          <div class="review-file">${escapeHtml(info.label)} · ${escapeHtml(targetOf(event.arguments))}</div></div>
        </div>
        <div class="review-body">${changePreview(event.tool, event.arguments || {})}</div>
        <div class="review-actions">
          <button type="button" class="btn approve">${icon('check')}Approve</button>
          <button type="button" class="btn decline">${icon('x')}Decline</button>
          <input class="review-reason" type="text" placeholder="Optional: tell Atlas why or what to do instead" aria-label="Reason for declining">
        </div>`;
      const decide = async (decision) => {
        card.querySelectorAll('button').forEach((b) => (b.disabled = true));
        const reason = card.querySelector('.review-reason').value.trim() || undefined;
        try {
          await api('/api/agent/approve', {
            method: 'POST',
            body: JSON.stringify({ session: state.session, call_id: event.call_id, decision, reason }),
          });
          card.classList.add('decided');
          const outcome = document.createElement('div');
          outcome.className = `review-outcome ${decision === 'allow' ? 'ok' : 'no'}`;
          outcome.innerHTML = decision === 'allow'
            ? `${icon('check')}Approved at ${clock()}`
            : `${icon('x')}Declined${reason ? `: ${escapeHtml(reason)}` : ''}`;
          card.append(outcome);
          turn.setThinking(true, decision === 'allow' ? 'Applying' : 'Thinking');
        } catch (err) {
          card.querySelectorAll('button').forEach((b) => (b.disabled = false));
          toast(`Couldn't send your answer: ${err.message}`);
        }
      };
      card.querySelector('.approve').addEventListener('click', () => decide('allow'));
      card.querySelector('.decline').addEventListener('click', () => decide('deny'));
      card.querySelector('.review-reason').addEventListener('keydown', (e) => { if (e.key === 'Enter') decide('deny'); });
      turn.setThinking(false);
      keepScrolled(() => body.append(card));
      card.querySelector('.approve').focus({ preventScroll: true });
    },
    notice(kind, html) {
      spine = null;
      const n = document.createElement('div');
      n.className = `notice ${kind}`;
      n.innerHTML = `${icon(kind === 'error' ? 'x' : 'spark')}<div>${html}</div>`;
      keepScrolled(() => (thinking ? body.insertBefore(n, thinking) : body.append(n)));
    },
  };
  return turn;
}

// ── Details panel ──────────────────────────────────────────────────────────

function showDetails(callId) {
  const step = state.steps.get(callId);
  if (!step) return;
  state.selectedStep = callId;
  for (const el of document.querySelectorAll('.step')) el.setAttribute('aria-current', String(el.dataset.call === callId));
  const info = toolInfo(step.tool);
  const args = step.args || {};
  let change = '';
  if (step.tool === 'edit_file' || step.tool === 'write_file') {
    change = `<h4>Change</h4><div class="diff-wrap">${changePreview(step.tool, args)}</div>`;
  }
  const output = step.output == null
    ? '<p class="sub">Still running…</p>'
    : `<pre class="out${step.isError ? ' err' : ''}">${escapeHtml(step.output || '(no output)')}</pre>`;
  $('#details').innerHTML = `
    <h3>${icon(info.icon)}${escapeHtml(info.label)}</h3>
    <p class="sub">${escapeHtml(targetOf(args)) || escapeHtml(step.tool)}</p>
    ${change}
    <h4>${step.isError ? 'Error' : 'Result'}</h4>${output}
    <h4>Arguments</h4><pre class="out">${escapeHtml(JSON.stringify(args, null, 2))}</pre>`;
  $('#details-empty').hidden = true;
  $('#details').hidden = false;
  openPanel('details');
}

// ── Side panel ─────────────────────────────────────────────────────────────

function openPanel(tab) {
  $('#panel').hidden = false;
  $('#app').classList.add('with-panel');
  selectTab(tab);
  syncPanelButtons();
}

function closePanel() {
  $('#panel').hidden = true;
  $('#app').classList.remove('with-panel');
  syncPanelButtons();
}

function selectTab(tab) {
  for (const t of document.querySelectorAll('.panel-tab')) t.setAttribute('aria-selected', String(t.dataset.tab === tab));
  $('#pane-terminal').hidden = tab !== 'terminal';
  $('#pane-details').hidden = tab !== 'details';
  syncPanelButtons();
  if (tab === 'terminal') requestAnimationFrame(fitActiveTerminal);
}

function currentTab() {
  return $('#tab-terminal').getAttribute('aria-selected') === 'true' ? 'terminal' : 'details';
}

function syncPanelButtons() {
  const open = !$('#panel').hidden;
  $('#terminal-toggle').setAttribute('aria-pressed', String(open && currentTab() === 'terminal'));
  $('#panel-toggle').setAttribute('aria-pressed', String(open && currentTab() === 'details'));
}

function togglePanel(tab) {
  if (!$('#panel').hidden && currentTab() === tab) return closePanel();
  openPanel(tab);
  if (tab === 'terminal' && !state.terminals.length) newTerminal();
}

// ── Terminal ───────────────────────────────────────────────────────────────

function terminalTheme() {
  const dark = getComputedStyle(document.documentElement).colorScheme.includes('dark');
  return dark
    ? { background: '#0a0c10', foreground: '#e6e8f0', cursor: '#8b90ff', selectionBackground: '#2a2e3f' }
    : { background: '#12141c', foreground: '#e8eaf1', cursor: '#a5a9ff', selectionBackground: '#2f3346' };
}

function fitActiveTerminal() {
  const t = state.terminals.find((x) => x.id === state.activeTerminal);
  if (!t || !t.fit || $('#panel').hidden || $('#pane-terminal').hidden) return;
  try {
    t.fit.fit();
    const { cols, rows } = t.term;
    if (cols !== t.cols || rows !== t.rows) {
      t.cols = cols; t.rows = rows;
      sendTerminal(t, { cols, rows });
    }
  } catch {}
}

async function sendTerminal(t, payload) {
  try {
    await fetch(`${API}/api/terminal/input`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: t.id, ...payload }),
    });
  } catch {}
}

// Keystrokes are batched briefly so fast typing and pastes stay in order
// and don't flood the server with one request per key.
function queueInput(t, data) {
  t.pending = (t.pending || '') + data;
  if (t.flushing) return;
  t.flushing = true;
  setTimeout(async () => {
    const chunk = t.pending; t.pending = '';
    await sendTerminal(t, { data: chunk });
    t.flushing = false;
    if (t.pending) queueInput(t, '');
  }, 8);
}

function renderTerminalTabs() {
  $('#term-tabs').innerHTML = state.terminals.map((t, i) => `
    <span class="term-tab${t.exited ? ' exited' : ''}" role="tab" aria-selected="${t.id === state.activeTerminal}" data-id="${t.id}">
      <span>${escapeHtml(t.label || `Terminal ${i + 1}`)}${t.exited ? ' (ended)' : ''}</span>
      <button class="x" data-close="${t.id}" aria-label="Close terminal">${icon('close')}</button>
    </span>`).join('');
  $('#term-empty').hidden = state.terminals.length > 0;
}

function activateTerminal(id) {
  state.activeTerminal = id;
  for (const t of state.terminals) t.view.hidden = t.id !== id;
  renderTerminalTabs();
  const t = state.terminals.find((x) => x.id === id);
  requestAnimationFrame(() => { fitActiveTerminal(); t?.term?.focus(); });
}

async function newTerminal() {
  openPanel('terminal');
  let data;
  try {
    data = await api('/api/terminal', { method: 'POST', body: JSON.stringify({ path: state.project, cols: 100, rows: 30 }) });
  } catch (err) {
    toast(`Couldn't open a terminal: ${err.message}`);
    return;
  }
  attachTerminal(data.id, data.shell);
  saveTerminalIds();
}

function attachTerminal(id, shell) {
  const view = document.createElement('div');
  view.className = 'term-view';
  $('#term-host').append(view);
  const shellName = String(shell || 'shell').split(/[\\/ ]/).filter(Boolean)[0]?.replace(/\.exe$/i, '') || 'shell';
  const t = { id, shell, label: shellName, view, term: null, fit: null, exited: false, cols: 0, rows: 0 };
  state.terminals.push(t);

  if (window.Terminal) {
    t.term = new window.Terminal({
      fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, monospace',
      fontSize: 13, lineHeight: 1.15, cursorBlink: true, scrollback: 5000,
      theme: terminalTheme(), allowProposedApi: false,
    });
    if (window.FitAddon) { t.fit = new window.FitAddon.FitAddon(); t.term.loadAddon(t.fit); }
    t.term.open(view);
    t.term.onData((d) => queueInput(t, d));
  } else {
    // xterm.js didn't load (offline): a plain line-based fallback.
    view.innerHTML = '<div class="term-fallback"><pre></pre><input type="text" aria-label="Terminal input" placeholder="Type a command and press Enter"></div>';
    const pre = view.querySelector('pre');
    const input = view.querySelector('input');
    t.write = (text) => {
      pre.textContent += text.replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '').replace(/\x1b\][^\x07]*\x07/g, '').replace(/\r(?!\n)/g, '');
      pre.scrollTop = pre.scrollHeight;
    };
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { queueInput(t, input.value + '\r'); input.value = ''; }
    });
  }
  activateTerminal(id);
  streamTerminal(t);
}

async function streamTerminal(t) {
  const decoder = new TextDecoder();
  try {
    const res = await fetch(`${API}/api/terminal/stream?id=${encodeURIComponent(t.id)}`);
    if (!res.ok || !res.body) throw new Error(`status ${res.status}`);
    const reader = res.body.getReader();
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      if (t.term) t.term.write(value);
      else t.write(decoder.decode(value, { stream: true }));
    }
  } catch {}
  t.exited = true;
  if (t.term) t.term.write('\r\n\x1b[2m[process ended]\x1b[0m\r\n');
  renderTerminalTabs();
  saveTerminalIds();
}

async function closeTerminal(id) {
  const i = state.terminals.findIndex((x) => x.id === id);
  if (i < 0) return;
  const [t] = state.terminals.splice(i, 1);
  try { await api('/api/terminal/close', { method: 'POST', body: JSON.stringify({ id }) }); } catch {}
  t.term?.dispose();
  t.view.remove();
  if (state.activeTerminal === id) {
    const next = state.terminals[Math.max(0, i - 1)];
    if (next) activateTerminal(next.id); else { state.activeTerminal = null; renderTerminalTabs(); }
  } else renderTerminalTabs();
  saveTerminalIds();
}

function saveTerminalIds() {
  try {
    sessionStorage.setItem('atlas.terminals', JSON.stringify(
      state.terminals.filter((t) => !t.exited).map((t) => ({ id: t.id, shell: t.shell }))));
  } catch {}
}

function restoreTerminals() {
  let saved = [];
  try { saved = JSON.parse(sessionStorage.getItem('atlas.terminals') || '[]'); } catch {}
  for (const s of saved) attachTerminal(s.id, s.shell);
}

// ── Running a turn ─────────────────────────────────────────────────────────

let currentTurn = null;

function setBusy(busy) {
  state.busy = busy;
  const send = $('#send');
  send.classList.toggle('stop', busy);
  send.innerHTML = icon(busy ? 'stop' : 'send');
  send.setAttribute('aria-label', busy ? 'Stop' : 'Send');
  $('#composer-hint').textContent = busy ? 'Atlas is working… Stop ends this turn.' : 'Enter to send · Shift+Enter for a new line';
  updateSendEnabled();
}

function updateSendEnabled() {
  $('#send').disabled = !state.busy && !$('#input').value.trim();
}

async function stopTurn() {
  if (!state.session) return;
  try {
    await api('/api/agent/interrupt', { method: 'POST', body: JSON.stringify({ session: state.session }) });
  } catch (err) {
    toast(`Couldn't stop: ${err.message}`);
  }
}

async function sendMessage(text) {
  text = text.trim();
  if (!text || state.busy) return;
  appendUser(text);
  if (!state.session) $('#conv-title').textContent = text.slice(0, 80);
  const turn = startAgentTurn(true);
  currentTurn = turn;
  turn.setThinking(true);
  setBusy(true);
  scrollToEnd();

  let finished = false;
  try {
    const res = await fetch(`${API}/api/agent`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt: text, path: state.project, session: state.session, access: state.access }),
    });
    if (!res.ok || !res.body) throw new Error(`the server answered ${res.status}`);
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    while (!finished) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let nl;
      while ((nl = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, nl).trim();
        buffer = buffer.slice(nl + 1);
        if (!line) continue;
        let event;
        try { event = JSON.parse(line); } catch { continue; }
        if (handleEvent(turn, event)) finished = true;
      }
    }
    if (!finished) turn.notice('error', 'The connection closed before the turn finished.');
  } catch (err) {
    turn.notice('error', `Couldn't reach Atlas at <b>${escapeHtml(API)}</b> (${escapeHtml(err.message)}). Start it with <code>atlas ui</code> and make sure Ollama is running.`);
  } finally {
    turn.setThinking(false);
    setBusy(false);
    currentTurn = null;
    loadSessions();
  }
}

// Returns true once the turn is over.
function handleEvent(turn, e) {
  switch (e.type) {
    case 'session_configured':
      state.session = e.session;
      markCurrentSession();
      return false;
    case 'agent_message':
    case 'message_delta':
      turn.setThinking(false);
      turn.addText(e.text ?? e.delta);
      turn.setThinking(true);
      return false;
    case 'tool_call_begin':
      turn.setThinking(false);
      turn.beginStep(e.call_id, e.tool, e.arguments);
      turn.setThinking(true, 'Working');
      return false;
    case 'tool_call_end':
      turn.endStep(e.call_id, e.output, e.is_error);
      return false;
    case 'approval_request':
      turn.review(e);
      return false;
    case 'tool_denied':
      turn.declineStep(e.call_id, e.tool);
      return false;
    case 'compacted':
      turn.notice('info', `Summarised the ${Number(e.covers)} earlier messages to stay within the context window.`);
      return false;
    case 'error':
      turn.setThinking(false);
      turn.notice('error', escapeHtml(e.message || 'Something went wrong.'));
      return false;
    case 'turn_complete':
      turn.setThinking(false);
      if (e.session) { state.session = e.session; markCurrentSession(); }
      if (e.cause === 'cancelled') turn.notice('info', 'Stopped.');
      else if (e.cause === 'max_turns') turn.notice('info', 'Atlas reached its step limit for this turn. Send <b>continue</b> to let it keep going.');
      else if (e.cause === 'failed') turn.notice('error', 'The turn failed. Your message is saved: send <b>continue</b> to retry.');
      return true;
    default:
      return false;
  }
}

// ── Wiring ─────────────────────────────────────────────────────────────────

let emptyTemplate;

function bindStarters() {
  for (const b of document.querySelectorAll('.starter')) {
    b.addEventListener('click', () => sendMessage(b.dataset.prompt));
  }
}

function setAccess(access) {
  state.access = access;
  for (const b of document.querySelectorAll('.seg')) b.setAttribute('aria-checked', String(b.dataset.access === access));
  $('#input').placeholder = access === 'write'
    ? 'Ask Atlas to change something. Each edit waits for your approval…'
    : 'Ask Atlas about this project…';
  store.set('atlas.access', access);
}

function openNav() { $('#app').classList.add('nav-open'); $('#scrim').hidden = false; }
function closeNav() { $('#app').classList.remove('nav-open'); $('#scrim').hidden = true; }

function autosize() {
  const el = $('#input');
  el.style.height = 'auto';
  el.style.height = Math.min(el.scrollHeight, 220) + 'px';
  updateSendEnabled();
}

async function init() {
  emptyTemplate = $('#empty').cloneNode(true);
  bindStarters();
  applyTheme(store.get('atlas.theme') || 'system');
  setAccess(store.get('atlas.access') === 'write' ? 'write' : 'read');
  setBusy(false);

  $('#theme-btn').addEventListener('click', () => {
    const now = store.get('atlas.theme') || 'system';
    applyTheme(THEMES[(THEMES.indexOf(now) + 1) % THEMES.length]);
  });
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => applyTheme(store.get('atlas.theme') || 'system'));

  for (const b of document.querySelectorAll('.seg')) b.addEventListener('click', () => setAccess(b.dataset.access));
  $('#new-chat').addEventListener('click', newConversation);
  $('#history-list').addEventListener('click', (e) => {
    const item = e.target.closest('.history-item');
    if (item) openConversation(item.dataset.id);
  });
  $('#project-btn').addEventListener('click', async () => {
    const path = prompt('Project folder to open in Atlas:', state.project);
    if (!path || path === state.project) return;
    if (state.busy) return toast('Stop the running turn first.');
    setProject(path.trim());
    newConversation();
    await loadSessions();
  });

  $('#composer').addEventListener('submit', (e) => {
    e.preventDefault();
    if (state.busy) return stopTurn();
    const text = $('#input').value;
    $('#input').value = '';
    autosize();
    sendMessage(text);
  });
  $('#input').addEventListener('input', autosize);
  $('#input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      $('#composer').requestSubmit();
    }
  });

  $('#thread').addEventListener('click', (e) => {
    const step = e.target.closest('.step');
    if (step) return showDetails(step.dataset.call);
    const copy = e.target.closest('.copy');
    if (copy) {
      navigator.clipboard?.writeText(copy.closest('.codeblock').querySelector('code').innerText);
      toast('Copied');
    }
  });

  $('#terminal-toggle').addEventListener('click', () => togglePanel('terminal'));
  $('#panel-toggle').addEventListener('click', () => togglePanel('details'));
  $('#panel-close').addEventListener('click', closePanel);
  for (const t of document.querySelectorAll('.panel-tab')) t.addEventListener('click', () => selectTab(t.dataset.tab));
  $('#term-new').addEventListener('click', newTerminal);
  $('#term-start').addEventListener('click', newTerminal);
  $('#term-tabs').addEventListener('click', (e) => {
    const close = e.target.closest('[data-close]');
    if (close) return closeTerminal(close.dataset.close);
    const tab = e.target.closest('.term-tab');
    if (tab) activateTerminal(tab.dataset.id);
  });
  new ResizeObserver(() => fitActiveTerminal()).observe($('#term-host'));

  $('#menu-btn').addEventListener('click', openNav);
  $('#sidebar-close').addEventListener('click', closeNav);
  $('#scrim').addEventListener('click', closeNav);

  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === '`') { e.preventDefault(); togglePanel('terminal'); }
    if (e.key === 'Escape' && $('#app').classList.contains('nav-open')) closeNav();
  });

  // Start-up: the project the server was opened on (or the last one used).
  try {
    const info = await api('/api/info');
    // `atlas ui --path` decides the project; the last folder is a fallback.
    if (info.project) setProject(info.project, info.project_name);
    else setProject(store.get('atlas.project') || '.', null);
    $('#model-name').textContent = info.model;
    $('#model-info').title = `Model: ${info.model}${info.context_tokens ? ` · ${info.context_tokens} token window` : ''}`;
  } catch (err) {
    $('#model-info').classList.add('offline');
    $('#model-name').textContent = 'Server offline';
    setProject(store.get('atlas.project') || '.', null);
  }
  await loadSessions();
  const fromHash = location.hash.match(/^#s=([\w-]+)/);
  if (fromHash) openConversation(fromHash[1]);
  restoreTerminals();
  autosize();
  $('#input').focus();
}

document.addEventListener('DOMContentLoaded', init);
