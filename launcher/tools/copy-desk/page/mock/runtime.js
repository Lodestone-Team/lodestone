// Mock app runtime for the Lodestone Copy Desk.
//
// A mock app is a set of screens (`routes`) and switchable UI states (`states`) that
// render real, editable strings. Screen modules call `Mock.register(appKey, def)`;
// the runtime draws the app's window, a toolbar to switch apps, pages and states,
// and handles in-place editing and saving through the Copy Desk (window.CopyDesk).
//
// API for screen modules:
//   const { h, t, tx } = Mock;
//   h(tag, attrs, ...children)   tag "div.class1.class2", attrs {class, style:{}, onclick, title, ...}
//   t(id, vars)                  editable string; vars fill {placeholders}; vars.count picks plural
//   tx(text)                     plain, non-editable text (sample data: names, versions)
//   ctx.state.<key>              the current value of a declared state
//   ctx.go(route, params)        navigate; ctx.params has the route's params
//   ctx.open(dialog, params)     open a dialog route (rendered over the page); ctx.close()
//   ctx.set(key, value)          change a state from inside the app (e.g. a toggle)
//
// def = {
//   name: "Lodestone Launcher",
//   css: "...",                         // scoped by the module itself (prefix classes)
//   size: [1280, 800],                  // window size in px
//   initial: "home",
//   states: { key: { label, options: [[value, label], ...], default } },
//   routes: { name: { title, render(ctx) -> Element, dialog: false } },
//   frame(ctx, page) -> Element         // optional: the window chrome around the page
// }
(function () {
  const apps = {};
  const run = { app: null, route: null, params: {}, dialog: null, dparams: {}, states: {}, editing: true, history: [], future: [] };
  const missing = new Set();

  function h(spec, attrs, ...kids) {
    const [tagAndId, ...classes] = String(spec || "div").split(".");
    const [tag, id] = tagAndId.split("#");
    const el = document.createElement(tag || "div");
    if (id) el.id = id;
    if (classes.length) el.className = classes.join(" ");
    if (attrs && (attrs instanceof Node || typeof attrs !== "object" || Array.isArray(attrs))) { kids.unshift(attrs); attrs = null }
    if (attrs) for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === false) continue;
      if (k === "class") el.className = (el.className ? el.className + " " : "") + v;
      else if (k === "style" && typeof v === "object") Object.assign(el.style, v);
      else if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === "html") el.innerHTML = v;
      else el.setAttribute(k, v === true ? "" : v);
    }
    const add = (c) => { if (c == null || c === false) return; if (Array.isArray(c)) c.forEach(add); else el.append(c instanceof Node ? c : document.createTextNode(String(c))) };
    kids.forEach(add);
    return el;
  }
  const tx = (text) => document.createTextNode(String(text));

  function pickIndex(s, vars) {
    if (s.forms) return (vars && Number(vars.count) === 1) ? 0 : 1;
    if (s.variants) { const mac = s.variants.findIndex(v => v.when === "macOS"); return mac >= 0 ? mac : 0 }
    return 0;
  }
  function fillInto(el, template, vars) {
    el.textContent = "";
    template.split(/(\{[a-zA-Z_][a-zA-Z0-9_]*\})/).forEach(part => {
      if (!part) return;
      const m = part.match(/^\{([a-zA-Z_][a-zA-Z0-9_]*)\}$/);
      if (m) {
        const c = document.createElement("span"); c.className = "mk-chip"; c.contentEditable = "false"; c.dataset.ph = part;
        const v = vars && vars[m[1]];
        c.textContent = v == null ? m[1] : (typeof v === "number" ? v.toLocaleString() : String(v));
        c.title = `Filled in by the app: ${part}`; el.append(c);
      } else el.append(document.createTextNode(part));
    });
  }
  function t(id, vars) {
    const D = window.CopyDesk; const s = D.byId(id);
    if (!s) { missing.add(id); return h("span.mk-missing", { title: "No such string: " + id }, "⟨" + id.split("/").pop() + "⟩") }
    const idx = pickIndex(s, vars);
    const el = document.createElement("span"); el.className = "mk-str"; el.dataset.sid = id; el.dataset.idx = idx;
    el._vars = vars || {};
    fillInto(el, D.current(s)[idx], vars);
    if (D.isEdited(s)) el.classList.add("mk-changed");
    if (run.editing) el.contentEditable = "true";
    el.spellcheck = true;
    return el;
  }
  function serialize(el) {
    let out = "";
    el.childNodes.forEach(n => {
      if (n.nodeType === 3) out += n.textContent;
      else if (n.dataset && n.dataset.ph) out += n.dataset.ph;
      else if (n.nodeName === "BR") out += "\n";
      else if (n.nodeName === "DIV" || n.nodeName === "P") out += (out ? "\n" : "") + serialize(n);
      else out += n.textContent;
    });
    return out.replace(/ /g, " ");
  }

  // Editing: in edit mode a click on a string edits it instead of pressing what it
  // sits in; double-click edits in browse mode too.
  function wireEditing(root) {
    root.addEventListener("click", (e) => {
      const str = e.target.closest && e.target.closest(".mk-str");
      if (str && run.editing) { e.preventDefault(); e.stopPropagation(); select(str) }
    }, true);
    root.addEventListener("dblclick", (e) => {
      const str = e.target.closest && e.target.closest(".mk-str");
      if (str && !run.editing) { e.preventDefault(); e.stopPropagation(); str.contentEditable = "true"; str.focus(); select(str) }
    }, true);
    root.addEventListener("input", (e) => {
      const str = e.target.closest && e.target.closest(".mk-str"); if (!str) return;
      const D = window.CopyDesk; const s = D.byId(str.dataset.sid); const idx = +str.dataset.idx;
      const v = serialize(str); D.applyEdit(s, idx, v);
      str.classList.toggle("mk-changed", v !== D.texts(s)[idx]);
      root.querySelectorAll(`.mk-str[data-sid="${CSS.escape(str.dataset.sid)}"]`).forEach(o => { if (o !== str && +o.dataset.idx === idx) fillInto(o, v, o._vars) });
      renderInspector(s, true);
    });
    root.addEventListener("keydown", (e) => {
      const str = e.target.closest && e.target.closest(".mk-str"); if (!str) return;
      if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); str.blur() }
      if (e.key === "Escape") str.blur();
      e.stopPropagation();
    }, true);
    root.addEventListener("paste", (e) => {
      const str = e.target.closest && e.target.closest(".mk-str"); if (!str) return;
      e.preventDefault(); document.execCommand("insertText", false, (e.clipboardData || window.clipboardData).getData("text/plain"));
    }, true);
    root.addEventListener("focusin", (e) => { const str = e.target.closest && e.target.closest(".mk-str"); if (str) select(str) });
  }
  function select(str) {
    document.querySelectorAll(".mk-str.mk-selected").forEach(n => n.classList.remove("mk-selected"));
    str.classList.add("mk-selected");
    renderInspector(window.CopyDesk.byId(str.dataset.sid));
  }

  // ----- rendering -----
  const ctx = {
    get state() { return run.states[run.app] },
    get params() { return run.params },
    get dparams() { return run.dparams },
    get route() { return run.route },
    go(route, params) { run.history.push([run.route, run.params]); run.future = []; run.route = route; run.params = params || {}; run.dialog = null; render() },
    back() { const p = run.history.pop(); if (!p) return; run.future.push([run.route, run.params]); [run.route, run.params] = p; run.dialog = null; render() },
    forward() { const n = run.future.pop(); if (!n) return; run.history.push([run.route, run.params]); [run.route, run.params] = n; run.dialog = null; render() },
    canBack() { return run.history.length > 0 }, canForward() { return run.future.length > 0 },
    open(dialog, params) { run.dialog = dialog; run.dparams = params || {}; render() },
    close() { run.dialog = null; render() },
    set(key, value) { run.states[run.app][key] = value; render(); renderStates() },
    t, h, tx,
  };

  function render() {
    const def = apps[run.app]; if (!def) return;
    const win = document.getElementById("mk-window"); if (!win) return;
    const scroll = Array.from(win.querySelectorAll("[data-mk-scroll]")).map(n => [n.dataset.mkScroll, n.scrollTop]);
    win.innerHTML = "";
    win.style.width = def.size[0] + "px"; win.style.height = def.size[1] + "px";
    const routeDef = def.routes[run.route] || def.routes[def.initial];
    let page;
    try { page = routeDef.render(ctx) } catch (err) { page = h("div.mk-error", "This screen failed to draw: " + err.message); console.error(err) }
    const framed = def.frame ? def.frame(ctx, page) : page;
    win.append(framed);
    if (run.dialog && def.routes[run.dialog]) {
      let dlg; try { dlg = def.routes[run.dialog].render(ctx) } catch (err) { dlg = h("div.mk-error", "This dialog failed to draw: " + err.message); console.error(err) }
      win.append(h("div.mk-backdrop", { onclick: (e) => { if (e.target === e.currentTarget) ctx.close() } }, dlg));
    }
    scroll.forEach(([k, top]) => { const n = win.querySelector(`[data-mk-scroll="${k}"]`); if (n) n.scrollTop = top });
    fit();
    renderPages(); renderStates();
    const note = document.getElementById("mk-missing");
    if (note) { note.hidden = !missing.size; note.textContent = missing.size ? `${missing.size} string ids not found (shown as ⟨…⟩)` : ""; note.title = [...missing].join("\n") }
  }
  function fit() {
    const def = apps[run.app]; const win = document.getElementById("mk-window"); const area = document.getElementById("mk-area");
    if (!def || !win || !area) return;
    const zoomSel = document.getElementById("mk-zoom"); const z = zoomSel ? zoomSel.value : "fit";
    const avail = area.clientWidth - 32;
    const scale = z === "fit" ? Math.min(1, avail / def.size[0]) : parseFloat(z);
    win.style.transform = `scale(${scale})`; win.style.transformOrigin = "top left";
    win.parentElement.style.width = def.size[0] * scale + "px"; win.parentElement.style.height = def.size[1] * scale + "px";
  }
  function renderPages() {
    const box = document.getElementById("mk-pages"); if (!box) return;
    const def = apps[run.app]; box.innerHTML = "";
    const groups = {};
    for (const [name, r] of Object.entries(def.routes)) { const g = r.group || (r.dialog ? "Dialogs" : "Pages"); (groups[g] = groups[g] || []).push([name, r]) }
    for (const [g, list] of Object.entries(groups)) {
      box.append(h("h3", g));
      list.forEach(([name, r]) => box.append(h("button", {
        "aria-current": (r.dialog ? run.dialog === name : run.route === name && !run.dialog) ? "true" : "false",
        onclick: () => { if (r.dialog) ctx.open(name, r.defaultParams); else { run.dialog = null; ctx.go(name, r.defaultParams) } },
      }, r.title || name)));
    }
  }
  function renderStates() {
    const box = document.getElementById("mk-states"); if (!box) return;
    const def = apps[run.app]; box.innerHTML = "";
    const st = run.states[run.app];
    for (const [key, sd] of Object.entries(def.states || {})) {
      const id = "mk-state-" + key;
      const sel = h("select", { id, onchange: (e) => { st[key] = e.target.value; render() } },
        sd.options.map(([v, label]) => h("option", { value: v, selected: String(st[key]) === String(v) }, label)));
      box.append(h("label.mk-state", { for: id }, h("span", sd.label), sel));
    }
  }
  function renderInspector(s, soft) {
    const box = document.getElementById("mk-inspector"); if (!box) return;
    if (!s) return;
    if (soft && box.dataset.sid === s.id) { box.querySelectorAll("textarea").forEach((ta, i) => { if (document.activeElement !== ta) ta.value = window.CopyDesk.current(s)[i] }); return }
    box.dataset.sid = s.id; box.innerHTML = "";
    const card = window.CopyDesk.card(s);
    card.querySelectorAll("textarea").forEach((ta, i) => ta.addEventListener("input", () => {
      document.querySelectorAll(`#mk-window .mk-str[data-sid="${CSS.escape(s.id)}"]`).forEach(o => { if (+o.dataset.idx === i) fillInto(o, ta.value, o._vars) });
    }));
    box.append(h("h2", "Selected text"), card);
  }

  function mount(container) {
    container.innerHTML = "";
    const appKeys = Object.keys(apps);
    if (!run.app) run.app = appKeys[0];
    for (const k of appKeys) {
      if (!run.states[k]) { run.states[k] = {}; for (const [key, sd] of Object.entries(apps[k].states || {})) run.states[k][key] = sd.default ?? sd.options[0][0] }
    }
    if (!run.route) run.route = apps[run.app].initial;
    const appBtns = h("div.mk-apps", appKeys.map(k => h("button", { "aria-pressed": run.app === k ? "true" : "false", onclick: () => { run.app = k; run.route = apps[k].initial; run.params = {}; run.dialog = null; run.history = []; run.future = []; mount(container) } }, apps[k].name)));
    const editToggle = h("label.mk-toggle", h("input", { type: "checkbox", id: "mk-edit", checked: run.editing, onchange: (e) => { run.editing = e.target.checked; render() } }), " Edit text (off: click through the app; double-click any text to edit)");
    const zoom = h("label.mk-toggle", "Zoom ", h("select", { id: "mk-zoom", onchange: fit }, [["fit", "Fit"], ["0.75", "75%"], ["1", "100%"]].map(([v, l]) => h("option", { value: v }, l))));
    container.append(
      h("aside.mk-side", h("div.mk-sidehead", appBtns), h("div.mk-scroll", h("h2", "States"), h("div#mk-states"), h("div#mk-pages"))),
      h("section.mk-main", h("div.mk-bar", editToggle, zoom, h("span#mk-missing.mk-warn", { hidden: true })), h("div#mk-area.mk-area", h("div.mk-holder", h("div#mk-window.mk-window")))),
      h("aside#mk-inspector.mk-inspector", h("h2", "How it works"), h("p.mk-help", "Click any text in the app to edit it. Turn off Edit text to click through the app and open its pages and dialogs; you can still double-click text to edit it. Use States to switch the app between situations: signed out, no instances, a server running, an error. Grey chips are filled in by the app."))
    );
    const win = document.getElementById("mk-window");
    wireEditing(win);
    render();
  }
  window.addEventListener("resize", () => { clearTimeout(window.__mkfit); window.__mkfit = setTimeout(fit, 150) });

  async function loadFonts(base) {
    const faces = [["Satoshi", "Satoshi-Regular.otf", 400], ["Satoshi", "Satoshi-Medium.otf", 500], ["Satoshi", "Satoshi-Bold.otf", 700],
      ["Clash Grotesk", "ClashGrotesk-Medium.otf", 500], ["Clash Grotesk", "ClashGrotesk-Semibold.otf", 600],
      ["Chillax", "Chillax-Medium.otf", 500], ["Chillax", "Chillax-Semibold.otf", 600], ["JetBrains Mono", "JetBrainsMono-Regular.ttf", 400]];
    await Promise.all(faces.map(async ([fam, file, weight]) => {
      try { const buf = await (await fetch(base + "fonts/" + file)).arrayBuffer(); const f = new FontFace(fam, buf, { weight: String(weight) }); await f.load(); document.fonts.add(f) } catch {}
    }));
  }

  window.Mock = { register(key, def) { apps[key] = def }, mount, render, h, t, tx, ctx, loadFonts, apps, missing };
})();
