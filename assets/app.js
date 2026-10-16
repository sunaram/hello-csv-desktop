(function () {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const mac = /Mac/.test(navigator.platform);
  const send = (m) => window.ipc.postMessage(m);
  const MAXALL = 10000;
  const api = (u) => fetch(u).then((r) => r.json());

  // document state
  let headers = [], rows = [], stream = false, total = 0, indexing = false, delim = ',', hdrOn = true, loaded = false;
  let pageRows = [];                 // streaming: cells of the page on screen
  let view = [];                     // in-memory: row indices after filter + sort
  let matches = [], searching = false, searchId = 0;
  let edits = new Map(), editCols = new Map(), memMods = new WeakMap();
  let undo = [], redo = [], dirty = false;
  let page = 1, pageSize = 50, sortCol = -1, sortDir = 1, query = '';
  let sel = null, editing = null, cur = new Map(), pollTimer = 0, sortTimer = 0, loadId = 0;

  const el = {};
  ['q', 'qx', 'qind', 'qtxt', 'save', 'revert', 'addrow', 'delrow', 'addcol', 'dirty', 'hdr', 'dl', 'cmd', 'ctsv', 'stats',
    'addr', 'bar', 'wrap', 'ovl', 'ovlmsg', 'th', 'tb', 'info', 'ps', 'p0', 'p1', 'p2', 'p3', 'pinfo', 'toast'].forEach((i) => (el[i] = $(i)));

  const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const letter = (i) => { let s = ''; for (i++; i > 0; i = Math.floor((i - 1) / 26)) s = String.fromCharCode(65 + ((i - 1) % 26)) + s; return s; };
  const ps = () => (pageSize < 0 ? MAXALL : pageSize);
  const recs = () => (stream ? (searching ? matches.length : total) : view.length);
  const maxPage = () => Math.max(1, Math.ceil(recs() / ps()));
  const cmp = (a, b) => {
    a = (a || '').trim(); b = (b || '').trim();
    const x = +a, y = +b;
    if (a !== '' && b !== '' && !isNaN(x) && !isNaN(y)) return x - y;
    return a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
  };

  let toastTimer = 0;
  function toast(m) {
    el.toast.textContent = m; el.toast.classList.add('on');
    clearTimeout(toastTimer); toastTimer = setTimeout(() => el.toast.classList.remove('on'), 2200);
  }
  function overlay(msg, spin) {
    el.ovlmsg.textContent = msg;
    el.ovl.firstChild.style.display = spin ? '' : 'none';
    el.ovl.classList.remove('h');
  }
  const hideOverlay = () => el.ovl.classList.add('h');

  function setDirty(d) {
    if (d === dirty) return;
    dirty = d;
    el.save.disabled = el.revert.disabled = !d;
    el.save.classList.toggle('on', d);
    el.dirty.style.display = d ? 'inline' : 'none';
    send('dirty:' + (d ? 1 : 0));
  }

  // ---- data access -------------------------------------------------------------------------------------------
  function cellVal(r, c) {
    const row = stream ? cur.get(r) : rows[r];
    return (row && row[c]) || '';
  }
  function isMod(r, c) {
    const s = stream ? editCols.get(r) : memMods.get(rows[r]);
    return !!s && s.has(c);
  }
  function setCell(r, c, v, mark) {
    if (!stream) {
      rows[r][c] = v;
      if (mark) { const o = rows[r]; if (!memMods.has(o)) memMods.set(o, new Set()); memMods.get(o).add(c); }
      return;
    }
    const row = edits.get(r) || (cur.get(r) || []).slice();
    row[c] = v;
    edits.set(r, row); cur.set(r, row);
    if (mark) { if (!editCols.has(r)) editCols.set(r, new Set()); editCols.get(r).add(c); }
  }

  function computeView() {
    if (stream) return;
    const q = query.trim().toLowerCase();
    const v = [];
    for (let i = 0; i < rows.length; i++) if (!q || rows[i].some((c) => c.toLowerCase().includes(q))) v.push(i);
    if (sortCol >= 0) v.sort((a, b) => cmp(rows[a][sortCol], rows[b][sortCol]) * sortDir);
    view = v;
  }

  // the rows on screen as {r: row number, c: cells}
  function disp() {
    const n = ps(), from = (page - 1) * n;
    let list;
    const byCol = (a, b) => cmp(a.c[sortCol], b.c[sortCol]) * sortDir;
    if (!stream) list = view.slice(from, from + n).map((i) => ({ r: i, c: rows[i] }));
    else if (searching) {
      let m = matches.map((x) => ({ r: x.r, c: edits.get(x.r) || x.c }));
      if (sortCol >= 0) m.sort(byCol);
      list = m.slice(from, from + n);
    } else {
      list = pageRows.map((c, k) => ({ r: from + k, c: edits.get(from + k) || c }));
      if (sortCol >= 0) list.sort(byCol);
    }
    cur = new Map(list.map((x) => [x.r, x.c]));
    return list;
  }

  // ---- rendering ---------------------------------------------------------------------------------------------
  function hl(text) {
    const q = query.trim().toLowerCase();
    if (!q || !text) return esc(text);
    const lo = text.toLowerCase();
    let out = '', i = 0, j;
    while ((j = lo.indexOf(q, i)) >= 0) { out += esc(text.slice(i, j)) + '<mark>' + esc(text.slice(j, j + q.length)) + '</mark>'; i = j + q.length; }
    return out + esc(text.slice(i));
  }

  function renderHeader() {
    let h = '<tr><th class="ri">#</th>';
    headers.forEach((t, i) => {
      h += '<th data-c="' + i + '" title="Click to sort, double-click to rename"><div class="hc"><span><span class="cl">' + letter(i) + '</span><span class="ht">' +
        esc(t) + '</span></span><span class="sa">' + (sortCol === i ? (sortDir > 0 ? '▲' : '▼') : '') + '</span></div></th>';
    });
    el.th.innerHTML = h + '</tr>';
  }

  function renderBody() {
    editing = null;
    const list = disp();
    if (!list.length) {
      el.tb.innerHTML = '';
      overlay(searching || query.trim() ? 'No rows match your search.' : 'This file is empty.', false);
      renderFooter();
      return;
    }
    hideOverlay();
    let h = '';
    for (const { r, c } of list) {
      h += '<tr><td class="ri">' + (r + 1) + '</td>';
      for (let k = 0; k < headers.length; k++) {
        const v = c[k] || '';
        const cls = (isMod(r, k) ? 'mod ' : '') + (sel && sel.r === r && sel.c === k ? 'sel' : '');
        h += '<td' + (cls ? ' class="' + cls.trim() + '"' : '') + ' data-r="' + r + '" data-c="' + k + '" title="' + esc(v) + '">' + hl(v) + '</td>';
      }
      h += '</tr>';
    }
    el.tb.innerHTML = h;
    renderFooter();
  }

  function renderFooter() {
    const mp = maxPage();
    if (page > mp) page = mp;
    el.pinfo.textContent = 'Page ' + page + ' of ' + mp.toLocaleString();
    el.p0.disabled = el.p1.disabled = page <= 1;
    el.p2.disabled = el.p3.disabled = page >= mp;
    let s = (stream ? total : rows.length).toLocaleString() + ' rows × ' + headers.length + ' cols';
    if (indexing) s += ' (indexing…)';
    if (stream && searching) s += ' · ' + matches.length.toLocaleString() + ' matches';
    else if (!stream && query.trim()) s += ' · ' + view.length.toLocaleString() + ' shown';
    el.stats.textContent = s;
    el.delrow.disabled = !sel || stream;
  }

  const render = () => { renderHeader(); renderBody(); };

  // ---- loading -----------------------------------------------------------------------------------------------
  async function load(fresh) {
    const id = ++loadId;
    if (fresh) { hdrOn = true; el.hdr.checked = true; el.dl.value = 'auto'; el.q.value = ''; query = ''; el.qx.style.display = 'none'; }
    overlay('Loading…', true);
    clearInterval(pollTimer);
    searchId++; searching = false; matches = []; el.qind.style.display = 'none';
    let j;
    try { j = await api('/__open?hdr=' + (hdrOn ? 1 : 0) + '&delim=' + encodeURIComponent(el.dl.value === 'auto' ? '' : el.dl.value)); }
    catch (e) { return overlay('Cannot load the file: ' + e, false); }
    if (id !== loadId) return;
    if (j.err) { loaded = false; headers = []; rows = []; el.th.innerHTML = el.tb.innerHTML = ''; return overlay(j.err === 'No file open' ? 'Drop a CSV or TSV file here, or press Ctrl+O to open one.' : j.err, false); }
    loaded = true;
    headers = j.headers; delim = j.delim; stream = j.stream; total = j.total; hdrOn = j.hdr;
    rows = stream ? [] : j.rows; pageRows = stream ? j.rows : [];
    edits = new Map(); editCols = new Map(); memMods = new WeakMap();
    undo = []; redo = []; setDirty(false);
    page = 1; sortCol = -1; sel = null; indexing = stream;
    el.addr.textContent = '-'; el.bar.value = ''; el.bar.disabled = true;
    el.info.textContent = j.name + ' · ' + (j.size / 1048576 >= 1 ? (j.size / 1048576).toFixed(1) + ' MB' : Math.ceil(j.size / 1024) + ' KB') +
      ' · delimiter ' + ({ ',': 'comma', ';': 'semicolon', '\t': 'tab', '|': 'pipe' }[delim] || delim) + (stream ? ' · streaming mode (large file)' : '');
    if (query.trim() && stream) startSearch(); else computeView();
    render();
    if (stream) {
      pollTimer = setInterval(async () => {
        try {
          const s = await api('/__status');
          total = s.total;
          if (s.done) { indexing = false; clearInterval(pollTimer); }
          renderFooter();
        } catch (e) { /* ignore */ }
      }, 400);
    }
  }

  async function fetchPage(n) {
    page = n;
    overlay('Loading page ' + n + '…', true);
    const j = await api('/__page?p=' + n + '&n=' + ps());
    if (j.err) return overlay(j.err, false);
    if (page !== n) return;
    pageRows = j.rows; total = j.total;
    renderBody();
    el.wrap.scrollTop = 0;
  }

  function goPage(n) {
    n = Math.max(1, Math.min(maxPage(), n));
    if (n === page) return;
    if (editing) commit();
    if (stream && !searching) fetchPage(n);
    else { page = n; renderBody(); el.wrap.scrollTop = 0; }
  }

  // ---- search ------------------------------------------------------------------------------------------------
  let qTimer = 0;
  function onQuery() {
    query = el.q.value;
    el.qx.style.display = query ? 'block' : 'none';
    clearTimeout(qTimer);
    qTimer = setTimeout(applyQuery, stream ? 250 : 120);
  }
  function applyQuery() {
    page = 1;
    if (!stream) { computeView(); renderBody(); return; }
    if (query.trim()) startSearch();
    else { searchId++; searching = false; matches = []; el.qind.style.display = 'none'; fetchPage(1); }
  }
  async function startSearch() {
    const id = ++searchId, q = query.trim();
    searching = true; matches = []; page = 1;
    el.qind.style.display = 'inline-flex'; el.qtxt.textContent = 'Scanning…';
    renderBody();
    let off = 0, row = 0;
    while (id === searchId) {
      let j;
      try { j = await api('/__search?q=' + encodeURIComponent(q) + '&off=' + off + '&row=' + row); } catch (e) { break; }
      if (id !== searchId) return;
      if (j.err) { toast(j.err); break; }
      for (const m of j.m) matches.push({ r: m[0], c: m[1] });
      off = j.off; row = j.row;
      el.qind.firstChild.style.display = j.done ? 'none' : '';
      el.qtxt.textContent = j.done ? matches.length.toLocaleString() + ' matches' : Math.round(j.ratio * 100) + '% · ' + matches.length.toLocaleString() + ' matches';
      if (!editing && (j.m.length || j.done)) renderBody();
      if (j.done) break;
      await new Promise((r) => setTimeout(r, 0));
    }
  }

  // ---- selection and editing ---------------------------------------------------------------------------------
  const tdAt = (r, c) => el.tb.querySelector('td[data-r="' + r + '"][data-c="' + c + '"]');

  function select(r, c, td) {
    const prev = el.tb.querySelector('td.sel');
    if (prev) prev.classList.remove('sel');
    sel = { r, c };
    td = td || tdAt(r, c);
    if (td) { td.classList.add('sel'); td.scrollIntoView({ block: 'nearest', inline: 'nearest' }); }
    el.addr.textContent = letter(c) + (r + 1);
    el.bar.disabled = false; el.bar.value = cellVal(r, c);
    el.delrow.disabled = stream;
    el.info.textContent = 'Selected ' + letter(c) + (r + 1) + ' (' + (headers[c] || '') + ')';
  }

  function move(dr, dc) {
    if (!sel) return;
    const list = [...cur.keys()];
    const i = list.indexOf(sel.r) + dr, c = sel.c + dc;
    if (c < 0 || c >= headers.length || i < 0) return;
    if (i >= list.length) { if (page < maxPage() && dr > 0) goPage(page + 1); return; }
    select(list[i], c);
  }

  function startEdit(td, ch) {
    if (editing) commit();
    const r = +td.dataset.r, c = +td.dataset.c;
    editing = { td, r, c, old: cellVal(r, c) };
    td.classList.add('ed');
    td.textContent = ch == null ? editing.old : ch;
    td.contentEditable = 'true';
    td.focus();
    const rg = document.createRange();
    rg.selectNodeContents(td);
    if (ch != null) rg.collapse(false);
    const s = getSelection(); s.removeAllRanges(); s.addRange(rg);
  }

  function commit() {
    if (!editing) return;
    const { td, r, c, old } = editing;
    editing = null;
    const v = td.innerText.replace(/\r?\n$/, '');
    td.contentEditable = 'false'; td.classList.remove('ed');
    getSelection().removeAllRanges();
    if (v !== old) pushEdit(r, c, old, v);
    td.innerHTML = hl(cellVal(r, c));
    td.title = cellVal(r, c);
    td.classList.toggle('mod', isMod(r, c));
    if (sel && sel.r === r && sel.c === c) el.bar.value = cellVal(r, c);
    el.wrap.focus({ preventScroll: true });
  }
  function cancel() {
    if (!editing) return;
    const { td, r, c } = editing;
    editing = null;
    td.contentEditable = 'false'; td.classList.remove('ed');
    td.innerHTML = hl(cellVal(r, c));
    el.wrap.focus({ preventScroll: true });
  }

  function pushEdit(r, c, old, v) {
    setCell(r, c, v, true);
    undo.push({ u: () => setCell(r, c, old, true), r: () => setCell(r, c, v, true) });
    redo = [];
    setDirty(true);
  }

  function doUndo() { if (undo.length) { const a = undo.pop(); redo.push(a); a.u(); afterHistory(); } }
  function doRedo() { if (redo.length) { const a = redo.pop(); undo.push(a); a.r(); afterHistory(); } }
  function afterHistory() {
    setDirty(undo.length > 0);
    computeView(); render();
    if (sel) select(sel.r, sel.c);
  }

  // cell bar edits the selected cell live; one undo step per focus
  let barOld = '';
  el.bar.addEventListener('focus', () => { barOld = sel ? cellVal(sel.r, sel.c) : ''; });
  el.bar.addEventListener('input', () => {
    if (!sel) return;
    setCell(sel.r, sel.c, el.bar.value, true);
    const td = tdAt(sel.r, sel.c);
    if (td) { td.textContent = el.bar.value; td.title = el.bar.value; td.classList.add('mod'); }
    setDirty(true);
  });
  el.bar.addEventListener('change', () => {
    if (!sel) return;
    const r = sel.r, c = sel.c, v = el.bar.value, old = barOld;
    if (v !== old) { undo.push({ u: () => setCell(r, c, old, true), r: () => setCell(r, c, v, true) }); redo = []; barOld = v; }
  });
  el.bar.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === 'Escape') { el.bar.blur(); el.wrap.focus(); } });

  el.tb.addEventListener('click', (e) => {
    const td = e.target.closest('td');
    if (!td || !td.dataset.r || td.classList.contains('ed')) return;
    if (editing) commit();
    select(+td.dataset.r, +td.dataset.c, td);
    el.wrap.focus({ preventScroll: true });
  });
  el.tb.addEventListener('dblclick', (e) => {
    const td = e.target.closest('td');
    if (!td || !td.dataset.r) return;
    select(+td.dataset.r, +td.dataset.c, td);
    startEdit(td);
  });
  el.tb.addEventListener('keydown', (e) => {
    if (!editing) return;
    if (e.key === 'Enter') { e.preventDefault(); commit(); move(1, 0); }
    else if (e.key === 'Escape') { e.preventDefault(); cancel(); }
    else if (e.key === 'Tab') {
      e.preventDefault();
      const { c } = editing; commit();
      move(0, e.shiftKey ? -1 : 1);
      if (sel && sel.c !== c) { const t = tdAt(sel.r, sel.c); if (t) startEdit(t); }
    }
  });
  el.tb.addEventListener('focusout', (e) => { if (editing && e.target === editing.td) commit(); });

  // ---- header: sort and rename ---------------------------------------------------------------------------------
  el.th.addEventListener('click', (e) => {
    const th = e.target.closest('th[data-c]');
    if (!th || e.target.isContentEditable) return;
    const c = +th.dataset.c;
    clearTimeout(sortTimer);
    sortTimer = setTimeout(() => {   // delayed so a double-click (rename) does not also sort
      if (sortCol === c) { if (sortDir > 0) sortDir = -1; else { sortCol = -1; sortDir = 1; } } else { sortCol = c; sortDir = 1; }
      page = 1; computeView(); render();
    }, 220);
  });
  el.th.addEventListener('dblclick', (e) => {
    clearTimeout(sortTimer);
    const th = e.target.closest('th[data-c]');
    if (!th) return;
    const c = +th.dataset.c, t = th.querySelector('.ht'), old = headers[c];
    t.contentEditable = 'true'; t.focus();
    const rg = document.createRange(); rg.selectNodeContents(t);
    const s = getSelection(); s.removeAllRanges(); s.addRange(rg);
    const done = (ok) => {
      t.contentEditable = 'false';
      const v = t.innerText.trim() || 'Column ' + (c + 1);
      if (ok && v !== old) {
        headers[c] = v;
        undo.push({ u: () => (headers[c] = old), r: () => (headers[c] = v) }); redo = [];
        hdrEdited = true; setDirty(true);
      }
      renderHeader();
    };
    t.addEventListener('keydown', (ev) => { if (ev.key === 'Enter') { ev.preventDefault(); done(true); } else if (ev.key === 'Escape') { ev.preventDefault(); done(false); } });
    t.addEventListener('blur', () => done(true), { once: true });
  });
  let hdrEdited = false;

  // ---- structure edits (small files) -------------------------------------------------------------------------
  const bigOnly = () => { toast('Adding rows or columns is only supported for files under 5 MB.'); };

  el.addrow.onclick = () => {
    if (!loaded) return;
    if (stream) return bigOnly();
    if (editing) commit();
    const row = new Array(headers.length).fill('');
    const at = sel ? sel.r + 1 : rows.length;
    const ins = () => rows.splice(at, 0, row), rem = () => rows.splice(at, 1);
    ins(); undo.push({ u: rem, r: ins }); redo = [];
    memMods.set(row, new Set(headers.keys()));
    setDirty(true); computeView();
    const pos = view.indexOf(at);
    if (pos >= 0) { page = Math.floor(pos / ps()) + 1; }
    render();
    const td = pos >= 0 && headers.length ? tdAt(at, 0) : null;
    if (td) { select(at, 0, td); startEdit(td); }
  };
  el.delrow.onclick = () => {
    if (!sel || stream) return;
    if (editing) commit();
    const at = sel.r, row = rows[at];
    const rem = () => rows.splice(at, 1), ins = () => rows.splice(at, 0, row);
    rem(); undo.push({ u: ins, r: rem }); redo = [];
    sel = null; el.addr.textContent = '-'; el.bar.value = ''; el.bar.disabled = true;
    setDirty(true); computeView(); render();
  };
  el.addcol.onclick = () => {
    if (!loaded) return;
    if (stream) return bigOnly();
    if (editing) commit();
    const name = 'Column ' + (headers.length + 1);
    const add = () => { headers.push(name); rows.forEach((r) => r.push('')); };
    const rem = () => { headers.pop(); rows.forEach((r) => r.pop()); };
    add(); undo.push({ u: rem, r: add }); redo = [];
    setDirty(true); render();
  };

  // ---- saving ------------------------------------------------------------------------------------------------
  function fmtField(v) {
    v = String(v == null ? '' : v).replace(/[\u0000\u0001\u001e]/g, '');
    return /["\r\n]/.test(v) || v.includes(delim) || /^\s|\s$/.test(v) ? '"' + v.replace(/"/g, '""') + '"' : v;
  }
  const fmtRow = (r, n) => { const o = []; for (let i = 0; i < n; i++) o.push(fmtField(r[i])); return o.join(delim); };

  window.__save = function () {
    if (!dirty || !loaded) return;
    if (editing) commit();
    el.save.disabled = true; el.save.textContent = 'Saving…';
    const n = headers.length;
    if (!stream) {
      const lines = rows.map((r) => fmtRow(r, n));
      if (hdrOn) lines.unshift(fmtRow(headers, n));
      send('save\u001e' + lines.join('\n') + '\n');
    } else {
      const items = [];
      edits.forEach((r, k) => items.push(k + '\u0001' + fmtRow(r, n)));
      send('delta\u001e' + (hdrOn && hdrEdited ? fmtRow(headers, n) : '') + '\u001e' + items.join('\u001e'));
    }
  };
  window.__saved = function (ok, msg) {
    el.save.textContent = 'Save';
    if (!ok) { el.save.disabled = false; return toast('Save failed: ' + msg); }
    edits = new Map(); editCols = new Map(); memMods = new WeakMap(); undo = []; redo = []; hdrEdited = false;
    setDirty(false);
    toast('Saved');
    if (stream) load(); else renderBody();
  };
  window.__load = function (fresh) { load(fresh); };
  window.__ext = function () {
    if (!dirty) { load(); toast('File changed on disk, reloaded'); }
    else el.info.textContent = 'The file changed on disk. Save to overwrite it, or Revert to reload.';
  };
  window.__theme = function (t) { document.documentElement.setAttribute('data-theme', t); };
  window.__find = function () { el.q.focus(); el.q.select(); };

  el.save.onclick = window.__save;
  el.revert.onclick = () => { if (dirty && confirm('Discard all unsaved edits and reload the file?')) load(); };

  // ---- toolbar / footer ----------------------------------------------------------------------------------------
  el.q.addEventListener('input', onQuery);
  el.qx.onclick = () => { el.q.value = ''; onQuery(); clearTimeout(qTimer); applyQuery(); el.q.focus(); };
  const guard = () => !dirty || confirm('You have unsaved changes. Discard them and re-parse the file?');
  el.hdr.onchange = () => { if (guard()) { hdrOn = el.hdr.checked; load(); } else el.hdr.checked = hdrOn; };
  el.dl.onchange = () => { if (guard()) load(); else el.dl.value = 'auto'; };
  el.ps.onchange = () => { pageSize = +el.ps.value; page = 1; if (stream && !searching) fetchPage(1); else renderBody(); };
  el.p1.onclick = () => goPage(1);
  el.p0.onclick = () => goPage(page - 1);
  el.p2.onclick = () => goPage(page + 1);
  el.p3.onclick = () => goPage(maxPage());

  function copy(text, what) {
    const fb = () => {
      const t = document.createElement('textarea');
      t.value = text; document.body.appendChild(t); t.select();
      try { document.execCommand('copy'); toast(what + ' copied'); } catch (e) { toast('Copy failed'); }
      t.remove();
    };
    if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(text).then(() => toast(what + ' copied'), fb); else fb();
  }
  function viewRows() { return disp().map((x) => x.c); }
  function allViewRows() { return stream ? (searching ? matches.map((m) => edits.get(m.r) || m.c) : pageRows) : view.map((i) => rows[i]); }
  el.cmd.onclick = () => {
    if (!loaded) return;
    const e = (s) => String(s || '').replace(/\|/g, '\\|').replace(/\r?\n/g, ' ');
    const out = ['| ' + headers.map(e).join(' | ') + ' |', '| ' + headers.map(() => '---').join(' | ') + ' |'];
    allViewRows().forEach((r) => out.push('| ' + headers.map((_, i) => e(r[i])).join(' | ') + ' |'));
    copy(out.join('\n'), 'Markdown table');
  };
  el.ctsv.onclick = () => {
    if (!loaded) return;
    const f = (v) => { v = String(v || ''); return /[\t\n"]/.test(v) ? '"' + v.replace(/"/g, '""') + '"' : v; };
    const out = [headers.map(f).join('\t')];
    allViewRows().forEach((r) => out.push(headers.map((_, i) => f(r[i])).join('\t')));
    copy(out.join('\n'), 'TSV');
  };

  // ---- keyboard ----------------------------------------------------------------------------------------------
  const keys = { o: 'open', r: 'reload', e: 'editor', w: 'quit', '=': 'zin', '+': 'zin', '-': 'zout', '0': 'zreset' };
  addEventListener('keydown', (e) => {
    const k = e.key.toLowerCase(), mod = e.ctrlKey || e.metaKey;
    if (e.key === 'F5') { e.preventDefault(); send('reload'); return; }
    if (mod && k === 's') { e.preventDefault(); window.__save(); return; }
    if (mod && k === 'f') { e.preventDefault(); window.__find(); return; }
    if (mod && !mac && keys[k]) { e.preventDefault(); send(keys[k]); return; }
    const tag = document.activeElement && document.activeElement.tagName;
    if (editing || tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA' || (document.activeElement && document.activeElement.isContentEditable)) return;
    if (mod && k === 'z' && !e.shiftKey) { e.preventDefault(); doUndo(); return; }
    if (mod && (k === 'y' || (k === 'z' && e.shiftKey))) { e.preventDefault(); doRedo(); return; }
    if (!sel) return;
    if (mod && k === 'c') { e.preventDefault(); copy(cellVal(sel.r, sel.c), 'Cell'); return; }
    const arrows = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] };
    if (arrows[e.key]) { e.preventDefault(); move(...arrows[e.key]); return; }
    if (e.key === 'PageDown') { e.preventDefault(); goPage(page + 1); return; }
    if (e.key === 'PageUp') { e.preventDefault(); goPage(page - 1); return; }
    const td = tdAt(sel.r, sel.c);
    if (!td) return;
    if (e.key === 'Enter' || e.key === 'F2') { e.preventDefault(); startEdit(td); }
    else if (e.key === 'Delete') { e.preventDefault(); const old = cellVal(sel.r, sel.c); if (old) { pushEdit(sel.r, sel.c, old, ''); td.textContent = ''; td.classList.add('mod'); el.bar.value = ''; } }
    else if (e.key.length === 1 && !mod && !e.altKey) { e.preventDefault(); startEdit(td, e.key); }
  });


  // update check for non-Store installs (window.__upd is only set by the app for those): asks GitHub for the latest release
  const U = window.__upd;
  const store = (k, v) => { try { if (v === undefined) return localStorage.getItem(k); localStorage.setItem(k, v); } catch (e) { /* ignore */ } };
  const newer = (a, b) => { a = a.split('.'); b = b.split('.'); for (let i = 0; i < 3; i++) { const d = (+a[i] || 0) - (+b[i] || 0); if (d) return d > 0; } return false; };
  function note(html, hold) {
    const n = $('upd') || document.body.appendChild(Object.assign(document.createElement('div'), { id: 'upd' }));
    n.innerHTML = html + '<button title="Dismiss">✕</button>';
    n.lastChild.onclick = () => { n.remove(); if (hold) store('upd_skip', hold); };
    if (!hold) setTimeout(() => n.remove(), 6000);
  }
  window.__checkUpdate = function (manual) {
    if (!U) return;
    fetch('https://api.github.com/repos/sunaram/hello-csv-desktop/releases/latest', { headers: { Accept: 'application/vnd.github+json' } })
      .then((r) => { if (!r.ok) throw 0; return r.json(); })
      .then((j) => {
        const v = String(j.tag_name || '').replace(/^v/, '');
        store('upd_t', String(Date.now()));
        if (v && newer(v, U.v)) {
          if (manual || store('upd_skip') !== v) note('Version ' + v + ' is available (you have ' + U.v + '). <a href="' + j.html_url + '">Download</a>', v);
        } else if (manual) note('You have the latest version (' + U.v + ').');
      })
      .catch(() => { if (manual) note('Could not check for updates.'); });
  };
  // automatic check at most once a day
  if (U && U.auto && Date.now() - (+store('upd_t') || 0) > 864e5) setTimeout(window.__checkUpdate, 1500);

  // ---- start ---------------------------------------------------------------------------------------------------
  load(true);
})();
