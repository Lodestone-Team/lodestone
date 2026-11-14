// Lodestone Launcher mock for the Copy Desk. Plain browser JS, registered with Mock.
// Strings: every piece of UI copy goes through t() via the short helpers below
// (sh = shell, sv = servers, fl = friend list, ct = content, is = instance settings,
// im = instances, wd = worlds, so = shared options, cm = common, fm = format; a "p"
// suffix is the file's plural enum). L(fn, "KeyA KeyB") renders a list of keys.
(function () {
  if (!window.Mock) return;
  const SRC = (document.currentScript && document.currentScript.textContent) || "";
  const { h, t, tx } = Mock;
  const K = (p) => (k, v) => t(p + k, v);
  const PFX = {
    sh: "launcher/shell/ShellMsg/", shp: "launcher/shell/ShellPlural/",
    sv: "launcher/servers/ServersMsg/", svp: "launcher/servers/ServersPlural/",
    fl: "launcher/friend_list/FriendListMsg/", flp: "launcher/friend_list/FriendListPlural/",
    ct: "launcher/content/ContentMsg/", ctp: "launcher/content/ContentPlural/",
    is: "launcher/instance_settings/InstanceSettingsMsg/", isp: "launcher/instance_settings/InstanceSettingsPlural/",
    im: "launcher/instances/InstancesMsg/", imp: "launcher/instances/InstancesPlural/",
    wd: "launcher/worlds/WorldsMsg/", wdp: "launcher/worlds/WorldsPlural/",
    so: "launcher/shared_options/SharedOptionsMsg/", sop: "launcher/shared_options/SharedOptionsPlural/",
    cm: "shared/common/CommonMsg/", cmp: "shared/common/CommonPlural/",
    fm: "shared/format/FormatMsg/", fmp: "shared/format/FormatPlural/",
  };
  const sh = K(PFX.sh), shp = K(PFX.shp), sv = K(PFX.sv), svp = K(PFX.svp), fl = K(PFX.fl), flp = K(PFX.flp),
    ct = K(PFX.ct), ctp = K(PFX.ctp), is = K(PFX.is), isp = K(PFX.isp), im = K(PFX.im), imp = K(PFX.imp),
    wd = K(PFX.wd), wdp = K(PFX.wdp), so = K(PFX.so), sop = K(PFX.sop), cm = K(PFX.cm), cmp = K(PFX.cmp),
    fm = K(PFX.fm), fmp = K(PFX.fmp);
  const FN_NAME = new Map([[sh, "sh"], [shp, "shp"], [sv, "sv"], [svp, "svp"], [fl, "fl"], [flp, "flp"], [ct, "ct"], [ctp, "ctp"], [is, "is"], [isp, "isp"], [im, "im"], [imp, "imp"], [wd, "wd"], [wdp, "wdp"], [so, "so"], [sop, "sop"], [cm, "cm"], [cmp, "cmp"], [fm, "fm"], [fmp, "fmp"]]);
  const keysOf = (s) => s.trim().split(/\s+/);
  // L(fn, "A B C", wrap) -> array of wrap(fn(key), key)
  const L = (fn, keys, wrap) => keysOf(keys).map((k) => (wrap ? wrap(fn(k), k) : fn(k)));
  const sep = () => h("span.lx-dot", " \u00b7 ");
  const join = (...parts) => { const out = []; parts.flat().filter(Boolean).forEach((p, i) => { if (i) out.push(sep()); out.push(p) }); return out };
  const ui = { menu: null, wgs: "minimize", ctab: "versions", hover: null };
  const rerender = () => Mock.render();
  const toggleMenu = (name) => (e) => { e.stopPropagation(); ui.menu = ui.menu === name ? null : name; rerender() };

  // ---------- icons ----------
  const P = {
    chevL: '<path d="M15 5l-7 7 7 7"/>', chevR: '<path d="M9 5l7 7-7 7"/>', caret: '<path d="M6 9l6 6 6-6" fill="currentColor"/>',
    plus: '<path d="M12 5v14M5 12h14"/>', x: '<path d="M6 6l12 12M18 6L6 18"/>', check: '<path d="M5 12.5l4.5 4.5L19 7"/>',
    search: '<circle cx="11" cy="11" r="6"/><path d="M16 16l4 4"/>', dots: '<circle cx="6" cy="12" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="18" cy="12" r="1.3" fill="currentColor"/>',
    dl: '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>', ul: '<path d="M12 20V9M7 13l5-5 5 5M5 4h14"/>', gear: '<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/>',
    copy: '<rect x="8" y="8" width="11" height="12" rx="1.5"/><path d="M5 16V5a1 1 0 011-1h9"/>', ext: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 01-1 1H5a1 1 0 01-1-1V7a1 1 0 011-1h5"/>',
    trash: '<path d="M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13"/>', user: '<circle cx="12" cy="8" r="4"/><path d="M4 21c1-4 4-6 8-6s7 2 8 6"/>',
    users: '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c.8-3.5 3.3-5 6.5-5s5.7 1.5 6.5 5"/><circle cx="17" cy="9" r="2.5"/><path d="M17 14c2.5 0 4 1.5 4.5 4"/>',
    server: '<rect x="4" y="4" width="16" height="7" rx="1.5"/><rect x="4" y="13" width="16" height="7" rx="1.5"/><path d="M8 7.5h.01M8 16.5h.01"/>',
    warn: '<path d="M12 3l10 18H2z" fill="currentColor" stroke="none"/><path d="M12 10v5M12 17.5v.5" stroke="#1d1e21"/>', refresh: '<path d="M20 11a8 8 0 10-2.3 5.7M20 4v7h-7"/>',
    play: '<path d="M8 5l11 7-11 7z" fill="currentColor" stroke="none"/>', folder: '<path d="M3 6a1 1 0 011-1h5l2 2h9a1 1 0 011 1v10a1 1 0 01-1 1H4a1 1 0 01-1-1z"/>',
    back: '<path d="M19 12H5M11 6l-6 6 6 6"/>', link: '<path d="M10 14a4 4 0 005.6 0l3-3a4 4 0 00-5.6-5.6l-1 1M14 10a4 4 0 00-5.6 0l-3 3a4 4 0 005.6 5.6l1-1"/>',
    cube: '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M4 7.5l8 4.5 8-4.5M12 12v9"/>', lan: '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" fill="currentColor"/>',
    list: '<path d="M4 6h16M4 12h16M4 18h16"/>', palette: '<path d="M12 3a9 9 0 100 18c1 0 1.5-.8 1.5-1.5 0-1-.8-1.3-.8-2.3 0-.8.7-1.2 1.5-1.2H17a4 4 0 004-4C21 7 17 3 12 3z"/><circle cx="7.5" cy="11" r="1" fill="currentColor"/><circle cx="10" cy="7" r="1" fill="currentColor"/><circle cx="15" cy="7.5" r="1" fill="currentColor"/>',
    drive: '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 14h.01M11 14h.01"/>', shield: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/>',
    key: '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M17 6l3 3"/>', image: '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 16l-5-5-9 8"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none"/>', power: '<path d="M12 3v8M6.3 7a8 8 0 1011.4 0"/>',
    restart: '<path d="M4 12a8 8 0 0113.7-5.7L20 9M20 4v5h-5M20 12a8 8 0 01-13.7 5.7L4 15M4 20v-5h5"/>', eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    signout: '<path d="M10 5H5v14h5M15 8l4 4-4 4M19 12H9"/>', pencil: '<path d="M4 20l4-1L19 8l-3-3L5 16z"/>', clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  };
  const ic = (name, cls) => h("span.lx-ic" + (cls ? "." + cls : ""), { html: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">${P[name] || ""}</svg>` });
  const logoMark = (big) => h("span.lx-lmark" + (big ? ".lx-lmark-big" : ""), { html: '<svg viewBox="0 0 24 24"><path d="M5 3h5v13h9v5H5z" fill="currentColor"/><path d="M12 3h7v10h-7z" fill="currentColor" opacity=".55"/></svg>' });
  const grass = (cls) => h("span.lx-grass" + (cls ? "." + cls : ""));
  const avatar = (kind, cls) => h("span.lx-av" + (kind === "offline" ? ".lx-av-off" : "") + (cls ? "." + cls : ""), kind === "offline" ? ic("user") : [h("i.lx-av-hair"), h("i.lx-av-eye.l"), h("i.lx-av-eye.r"), h("i.lx-av-mouth")]);

  // ---------- components ----------
  const btn = (label, opts = {}) => h("button.lx-btn" + (opts.kind ? ".lx-btn-" + opts.kind : "") + (opts.sm ? ".lx-sm" : "") + (opts.cls ? "." + opts.cls : ""),
    { onclick: opts.on, title: opts.title, disabled: opts.disabled }, opts.icon ? ic(opts.icon) : null, label);
  const pbtn = (label, on, icon) => btn(label, { kind: "p", on, icon });
  const gbtn = (label, on, icon) => btn(label, { kind: "g", on, icon });
  const dbtn = (label, on, icon) => btn(label, { kind: "d", on, icon });
  const iconBtn = (name, on, title) => h("button.lx-ibtn", { onclick: on, title }, ic(name));
  const toggle = (on, cb) => h("span.lx-tg" + (on ? ".on" : ""), { onclick: cb }, h("i"));
  const seg = (items, active, onPick) => h("div.lx-seg", items.map(([key, label, count]) => h("button" + (key === active ? ".on" : ""), { onclick: () => onPick && onPick(key) }, label, count != null ? h("span.lx-segn", tx(count)) : null)));
  const field = (value, placeholder, opts = {}) => h("div.lx-input" + (opts.cls ? "." + opts.cls : "") + (opts.err ? ".lx-err" : ""), opts.icon ? ic(opts.icon) : null,
    value != null ? h("span.lx-val", typeof value === "string" ? tx(value) : value) : h("span.lx-ph", placeholder), opts.right || null);
  const select = (value, opts = {}) => h("div.lx-select" + (opts.cls ? "." + opts.cls : ""), { onclick: opts.on }, h("span", value), ic("caret"));
  const badge = (label, kind) => h("span.lx-badge" + (kind ? ".lx-b-" + kind : ""), label);
  const dot = (kind) => h("span.lx-sdot.lx-sd-" + (kind || "off"));
  const spinner = () => h("span.lx-spin", h("i"), h("i"), h("i"));
  const note = (body, kind, extra) => h("div.lx-note.lx-n-" + (kind || "info"), kind === "warn" || kind === "err" ? ic("warn") : kind === "ok" ? ic("check") : ic("clock"), h("div.lx-notebody", body), extra || null);
  const toast = (title, body, kind, actions) => h("div.lx-toast" + (kind ? ".lx-t-" + kind : ""), kind ? h("span.lx-toast-ic", ic(kind === "ok" ? "check" : kind === "err" || kind === "warn" ? "warn" : "clock")) : null,
    h("div.lx-toast-txt", title ? h("div.lx-toast-title", title) : null, body ? h("div.lx-toast-body", body) : null, actions ? h("div.lx-toast-acts", actions) : null));
  const card = (...kids) => h("div.lx-card", ...kids);
  const srow = (label, hint, control, opts = {}) => h("div.lx-srow" + (opts.cls ? "." + opts.cls : ""), h("div.lx-srow-l", h("div.lx-srow-label", label), hint ? h("div.lx-srow-hint", hint) : null, opts.below || null), control ? h("div.lx-srow-c", control) : null);
  const section = (title, ...kids) => h("div.lx-sec", h("h3.lx-h3", title), ...kids);
  const caps = (el) => h("span.lx-caps", el);
  const progress = (pct) => h("div.lx-prog", h("i", { style: { width: pct + "%" } }));
  const menu = (items, cls) => h("div.lx-menu" + (cls ? "." + cls : ""), { onclick: (e) => e.stopPropagation() }, items.map((it) => it === "-" ? h("div.lx-menu-sep") : it && it.nodeType ? it :
    h("button.lx-menu-item" + (it.danger ? ".lx-danger" : ""), { onclick: it.on || (() => { ui.menu = null; rerender() }) }, it.icon ? ic(it.icon) : null, h("span", it.label), it.right || null)));
  const empty = (icon, title, hint, actions) => h("div.lx-empty", icon ? ic(icon) : null, title ? h("div.lx-empty-t", title) : null, hint ? h("div.lx-empty-h", hint) : null, actions ? h("div.lx-empty-a", actions) : null);
  const modIcon = (hue) => h("span.lx-modic", { style: { background: `hsl(${hue} 70% 62%)`, boxShadow: `inset 0 0 0 5px hsl(${hue} 55% 48%)` } });
  // A dialog: title, close, body, footer actions.
  const dialog = (ctx, title, body, actions, opts = {}) => h("div.lx-dlg" + (opts.wide ? ".lx-dlg-wide" : "") + (opts.narrow ? ".lx-dlg-narrow" : "") + (opts.cls ? "." + opts.cls : ""), { onclick: (e) => e.stopPropagation() },
    h("div.lx-dlg-head", h("h2.lx-dlg-title", title), opts.headRight || null, opts.noClose ? null : iconBtn("x", () => ctx.close(), "")),
    h("div.lx-dlg-body", body), actions && actions.length ? h("div.lx-dlg-foot", actions) : null);
  const confirmDlg = (ctx, title, body, primary, opts = {}) => dialog(ctx, title, body, [
    opts.danger ? dbtn(primary, () => ctx.close()) : pbtn(primary, () => ctx.close()),
    ...(opts.extra || []), btn(opts.cancel || cm("Cancel"), { on: () => ctx.close() })], { narrow: true });

  // ---------- styles ----------
  const CSS = `
.lx-root,.lx-dlg{--g300:#E3E3E4;--g400:#A5A5AC;--g500:#767A82;--g550:#67686A;--g600:#44464B;--g700:#36393F;--g750:#303338;--g800:#2B2D32;--g850:#26282C;--g875:#212327;--g900:#1D1E21;--blue:#1D8EB2;--blue2:#59B2F3;--green:#3DAE5E;--green2:#6DD277;--red:#FF5C5C;--red3:#CF4545;--yellow:#EFB440;}
.lx-root{position:relative;width:100%;height:100%;display:grid;grid-template-rows:46px minmax(0,1fr);background:var(--g875);color:var(--g300);font-family:Satoshi,system-ui,sans-serif;font-size:14px;line-height:1.35;overflow:hidden;-webkit-font-smoothing:antialiased}
.lx-root *,.lx-dlg,.lx-dlg *{box-sizing:border-box}
.lx-root button,.lx-dlg button{font:inherit;color:inherit;cursor:pointer}
.lx-ic{display:inline-flex;width:16px;height:16px;flex:none;vertical-align:-3px}.lx-ic svg{width:100%;height:100%}
.lx-top{display:grid;grid-template-columns:234px 1fr 234px;align-items:center;background:var(--g850);border-bottom:1px solid var(--g800)}
.lx-top.lx-dev{border-bottom-color:#a27a2c}
.lx-arrows{display:flex;gap:6px;padding-left:90px}.lx-arrows button{background:none;border:0;color:var(--g400);padding:4px;border-radius:4px}.lx-arrows button:disabled{color:var(--g600);cursor:default}.lx-arrows .lx-ic{width:20px;height:20px}
.lx-logo{justify-self:center;display:flex;align-items:center;gap:8px;font-family:"Clash Grotesk";font-weight:600;font-size:24px;letter-spacing:-.01em;color:#eee}
.lx-lmark{display:inline-flex;width:16px;height:16px;color:#3fd0b7}.lx-lmark svg{width:100%;height:100%}.lx-lmark-big{width:42px;height:42px}
.lx-devb{background:var(--yellow);color:#2a2000;font-family:"Clash Grotesk";font-weight:600;font-size:14px;padding:1px 6px;border-radius:4px}
.lx-upd{justify-self:end;padding-right:12px;display:flex;align-items:center;gap:8px;font-size:12.5px}
.lx-upd .lx-pill{background:rgba(29,142,178,.18);color:var(--blue2);border:1px solid rgba(89,178,243,.4);padding:2px 8px;border-radius:999px}
.lx-body{display:grid;grid-template-columns:234px minmax(0,1fr);min-height:0}
.lx-side{background:var(--g850);border-right:1px solid var(--g800);display:flex;flex-direction:column;min-height:0;position:relative}
.lx-side-scroll{flex:1;overflow:auto;padding:12px 10px}
.lx-acct{display:flex;align-items:center;gap:10px;border:1px solid var(--g700);border-radius:8px;padding:9px 10px;cursor:pointer;position:relative}
.lx-acct-n{font-weight:700;font-size:15px}.lx-acct-s{font-size:12.5px;color:var(--g400)}.lx-acct .lx-caretic{margin-left:auto;color:var(--g400)}
.lx-av{width:34px;height:34px;border-radius:4px;background:#d9a066;position:relative;flex:none;overflow:hidden;display:inline-block}
.lx-av .lx-av-hair{position:absolute;left:0;right:0;top:0;height:30%;background:#3aa6c9}.lx-av .lx-av-eye{position:absolute;top:46%;width:16%;height:12%;background:#2b2b55}.lx-av .lx-av-eye.l{left:18%}.lx-av .lx-av-eye.r{right:18%}.lx-av .lx-av-mouth{position:absolute;bottom:18%;left:30%;width:40%;height:9%;background:#7a3d2a}
.lx-av-off{background:var(--g700);color:var(--g400);display:inline-flex;align-items:center;justify-content:center}.lx-av-sm{width:22px;height:22px}
.lx-sidecap{font-size:12.5px;text-transform:uppercase;color:var(--g500);letter-spacing:.02em;margin:18px 4px 8px}
.lx-grp{display:flex;align-items:center;gap:6px;font-size:12.5px;text-transform:uppercase;color:var(--g400);margin:12px 4px 6px;font-weight:500}.lx-grp .lx-n{margin-left:auto;color:var(--g550)}
.lx-grp .lx-tri{width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-top:6px solid var(--g400)}
.lx-irow{display:flex;align-items:center;gap:10px;padding:7px 10px;border-radius:6px;border:1px solid transparent;cursor:pointer;position:relative}
.lx-irow:hover{background:var(--g800)}.lx-irow.on{background:var(--g800);border-color:var(--g700)}
.lx-irow-n{font-size:15px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.lx-irow-s{font-size:12.5px;color:var(--g400)}
.lx-irow .lx-iplay{position:absolute;left:8px;top:50%;transform:translateY(-50%);width:32px;height:32px;border-radius:5px;background:rgba(61,174,94,.92);display:none;align-items:center;justify-content:center;color:#fff}
.lx-irow:hover .lx-iplay{display:flex}.lx-irow .lx-iplay .lx-ic{width:14px;height:14px}
.lx-irow .lx-iprog{position:absolute;left:10px;right:10px;bottom:2px}
.lx-grass{width:30px;height:30px;flex:none;display:inline-block;background:linear-gradient(160deg,#6fb64a 0 38%,#4b8a31 38% 44%,#8a5b3a 44% 100%);clip-path:polygon(50% 0,100% 25%,100% 75%,50% 100%,0 75%,0 25%)}
.lx-grass.lx-big{width:44px;height:44px}
.lx-noinst{border:2px dashed var(--g700);border-radius:8px;padding:22px 10px;text-align:center;color:var(--g500);font-style:italic}
.lx-sidelink{display:flex;align-items:center;gap:8px;padding:7px 8px;border-radius:6px;color:var(--g400);background:none;border:0;width:100%;text-align:left;font-size:15px}
.lx-sidelink:hover{background:var(--g800);color:var(--g300)}.lx-sidelink.on{background:var(--g800);color:var(--g300)}
.lx-side-foot{border-top:1px solid var(--g800);padding:10px}.lx-side-foot .lx-sidelink{color:var(--g300);font-size:16px}.lx-side-foot .lx-lmark{width:18px;height:18px}
.lx-ver{font-size:11px;color:var(--g550);padding:6px 8px 0}
.lx-main{display:flex;flex-direction:column;min-height:0;min-width:0;position:relative}
.lx-page{flex:1;overflow:auto;padding:26px 34px 30px}
.lx-h1{font-family:Satoshi;font-weight:700;font-size:31px;margin:0;letter-spacing:-.01em;line-height:1.15}
.lx-h1-chillax{font-family:Chillax;font-weight:600;font-size:30px}
.lx-sub{color:var(--g400);font-size:15px;margin-top:4px}
.lx-dot{color:var(--g500)}
.lx-tabs{display:flex;gap:20px;border-bottom:2px solid var(--g800);margin:20px 0 24px}
.lx-tabs button{background:none;border:0;padding:0 0 4px;font-size:18px;color:var(--g400);border-bottom:2px solid transparent;margin-bottom:-2px}
.lx-tabs button.on{color:var(--blue2);border-bottom-color:var(--blue2)}
.lx-hero{position:relative;border:1px solid var(--g700);border-radius:10px;overflow:hidden;height:470px;background:radial-gradient(circle at 75% 45%,#3e4f78 0,#24304a 35%,#1a1e28 70%);display:flex;flex-direction:column;justify-content:flex-end;padding:38px}
.lx-hero::after{content:"";position:absolute;right:40px;top:40px;width:520px;height:380px;background:conic-gradient(from 210deg at 50% 60%,#2a2c33,#3b3e47,#2a2c33,#1f2127,#2a2c33);clip-path:polygon(50% 8%,96% 40%,96% 62%,50% 96%,4% 62%,4% 40%);opacity:.85}
.lx-hero>*{position:relative;z-index:1;max-width:460px}
.lx-hero-t{font-weight:700;font-size:34px;margin:0 0 12px}.lx-hero-p{color:var(--g300);font-size:16.5px;opacity:.9}
.lx-hero .lx-btn{margin-top:18px;align-self:flex-start}
.lx-playbar{flex:none;height:110px;border-top:1px solid var(--g800);display:flex;align-items:center;justify-content:center;gap:16px;position:relative}
.lx-play{width:300px;height:68px;border-radius:6px;border:2px solid transparent;background:var(--green);color:#fff;font-family:"Clash Grotesk";font-weight:600;font-size:30px;text-transform:uppercase;display:flex;align-items:center;justify-content:center;gap:12px;letter-spacing:.02em}
.lx-play.lx-off{background:#2c4a36;color:#7e9a86;border-color:#3a6a48;cursor:default}
.lx-play.lx-playing{background:transparent;border-color:var(--green);color:var(--green2);justify-content:space-between;padding:0 12px 0 22px;font-size:24px}
.lx-play.lx-playing .lx-btn{font-family:Satoshi;font-size:14px;text-transform:none;color:var(--g300)}
.lx-play.lx-launch{background:#2f7a48;font-size:24px}
.lx-play-sub{position:absolute;left:50%;transform:translateX(-50%);bottom:6px;font-size:12.5px;color:var(--g400);white-space:nowrap}
.lx-play-prog{width:300px;display:grid;gap:5px}.lx-play-prog .lx-pl{display:flex;justify-content:space-between;font-size:13px;color:var(--g400)}
.lx-play-note{position:absolute;left:24px;font-size:12.5px;color:var(--g400);max-width:250px}
.lx-btn{display:inline-flex;align-items:center;gap:6px;background:transparent;border:1px solid var(--g600);color:var(--g300);border-radius:4px;padding:5px 10px;font-size:14px;white-space:nowrap;line-height:1.2}
.lx-btn:hover{background:var(--g750)}.lx-btn:disabled{opacity:.5;cursor:default}
.lx-btn-p{background:var(--blue);border-color:var(--blue);color:#fff}.lx-btn-p:hover{background:#2499be}
.lx-btn-g{border-color:transparent;color:var(--g400)}.lx-btn-g:hover{color:var(--g300);background:transparent}
.lx-btn-d{color:var(--red);border-color:rgba(255,92,92,.55)}
.lx-btn-ok{background:var(--green);border-color:var(--green);color:#fff}
.lx-btn.lx-sm{padding:3px 7px;font-size:13px}
.lx-ibtn{display:inline-flex;align-items:center;justify-content:center;width:30px;height:30px;background:transparent;border:1px solid var(--g600);border-radius:4px;color:var(--g400);flex:none}
.lx-tg{display:inline-block;width:50px;height:26px;border-radius:13px;background:var(--g600);position:relative;cursor:pointer;flex:none}.lx-tg i{position:absolute;top:4px;left:4px;width:18px;height:18px;border-radius:50%;background:#eee;transition:left .15s}
.lx-tg.on{background:var(--green)}.lx-tg.on i{left:28px}
.lx-seg{display:inline-flex;background:var(--g900);border:1px solid var(--g700);border-radius:7px;padding:3px;gap:2px}
.lx-seg button{background:none;border:0;padding:6px 13px;border-radius:5px;color:var(--g400);font-size:15px;display:inline-flex;gap:6px}.lx-seg button.on{background:var(--g700);color:var(--g300)}
.lx-segn{opacity:.9}
.lx-input{display:flex;align-items:center;gap:8px;background:var(--g900);border:1px solid var(--g600);border-radius:5px;padding:8px 11px;min-height:38px;font-size:15px;min-width:0}
.lx-input.lx-err{border-color:var(--red3)}.lx-ph{color:var(--g500)}.lx-val{color:var(--g300);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;flex:1}
.lx-input .lx-ic{color:var(--g400)}.lx-input>.lx-ph{flex:1}
.lx-select{display:inline-flex;align-items:center;gap:10px;justify-content:space-between;background:var(--g850);border:1px solid var(--g700);border-radius:5px;padding:7px 12px;font-size:15px;cursor:pointer;min-width:120px}.lx-select .lx-ic{width:12px;height:12px;color:var(--g400)}
.lx-badge{display:inline-flex;align-items:center;font-size:12px;padding:1px 7px;border-radius:4px;border:1px solid var(--g600);color:var(--g400);white-space:nowrap;vertical-align:2px}
.lx-b-red{color:var(--red);border-color:rgba(255,92,92,.6);background:rgba(174,50,50,.25)}.lx-b-green{color:var(--green2);border-color:rgba(109,210,119,.55);background:rgba(97,174,50,.15)}
.lx-b-blue{color:var(--blue2);border-color:rgba(89,178,243,.55);background:rgba(60,133,186,.18)}.lx-b-yellow{color:var(--yellow);border-color:rgba(239,180,64,.55);background:rgba(174,139,50,.18)}
.lx-sdot{display:inline-block;width:9px;height:9px;border-radius:50%;flex:none}.lx-sd-on{background:#2AF588}.lx-sd-off{background:var(--g550)}.lx-sd-warn{background:var(--yellow)}.lx-sd-err{background:var(--red)}
.lx-spin{display:inline-flex;gap:3px;align-items:center}.lx-spin i{width:5px;height:5px;border-radius:50%;background:var(--g500)}.lx-spin i:nth-child(2){background:var(--g400)}.lx-spin i:nth-child(3){background:var(--g300)}
.lx-note{display:flex;gap:10px;align-items:flex-start;padding:10px 14px;border-radius:7px;font-size:14px;border:1px solid var(--g700);background:var(--g850);margin:10px 0}
.lx-note>.lx-ic{margin-top:1px}.lx-notebody{flex:1;min-width:0}.lx-n-warn{color:var(--yellow);border-color:rgba(239,180,64,.35);background:rgba(174,139,50,.12)}
.lx-n-err{color:#ff8a8a;border-color:rgba(255,92,92,.35);background:rgba(174,50,50,.14)}.lx-n-ok{color:var(--green2);border-color:rgba(109,210,119,.3);background:rgba(97,174,50,.1)}.lx-n-info{color:var(--g300)}
.lx-n-info>.lx-ic{color:var(--blue2)}
.lx-toasts{position:absolute;right:18px;bottom:18px;display:grid;gap:10px;z-index:30;width:380px}
.lx-toast{display:flex;gap:10px;background:var(--g800);border:1px solid var(--g600);border-radius:9px;padding:12px 14px;box-shadow:0 10px 30px rgba(0,0,0,.4);font-size:14px}
.lx-toast-title{font-weight:700;margin-bottom:2px}.lx-toast-body{color:var(--g400)}.lx-toast-acts{display:flex;gap:8px;margin-top:10px}.lx-toast-ic{color:var(--blue2)}.lx-t-err .lx-toast-ic{color:var(--red)}.lx-t-ok .lx-toast-ic{color:var(--green2)}.lx-t-warn .lx-toast-ic{color:var(--yellow)}
.lx-card{background:var(--g850);border:1px solid var(--g700);border-radius:9px;overflow:hidden}
.lx-card>*+*{border-top:1px solid var(--g700)}
.lx-srow{display:flex;align-items:center;gap:16px;padding:14px 18px;min-height:56px}.lx-srow-l{flex:1;min-width:0}.lx-srow-label{font-size:16px;font-weight:500}.lx-srow-hint{color:var(--g400);font-size:13.5px;margin-top:2px}
.lx-srow-c{display:flex;align-items:center;gap:8px;flex:none;max-width:60%}.lx-srow-c .lx-input{min-width:300px}
.lx-sec{margin:0 0 28px}.lx-h3{font-family:Chillax;font-weight:600;font-size:19px;margin:0 0 10px;color:var(--g300)}
.lx-h3-sub{color:var(--g400);font-size:14px;margin:-6px 0 10px}
.lx-caps{text-transform:uppercase;letter-spacing:.03em;font-size:12.5px;color:var(--g400);font-weight:500}
.lx-prog{height:6px;border-radius:3px;background:var(--g700);overflow:hidden}.lx-prog i{display:block;height:100%;background:var(--blue2)}
.lx-menu{position:absolute;z-index:40;background:var(--g850);border:1px solid var(--g600);border-radius:8px;padding:6px;box-shadow:0 12px 30px rgba(0,0,0,.45);min-width:220px}
.lx-menu-item{display:flex;align-items:center;gap:10px;width:100%;background:none;border:0;padding:8px 10px;border-radius:5px;text-align:left;font-size:15px}.lx-menu-item:hover{background:var(--g750)}
.lx-menu-item.lx-danger{color:var(--red)}.lx-menu-sep{height:1px;background:var(--g700);margin:5px 4px}.lx-menu-item .lx-mright{margin-left:auto;font-size:12.5px;color:var(--g500);display:flex;gap:6px;align-items:center}
.lx-empty{border:2px dashed var(--g700);border-radius:9px;padding:30px;text-align:center;color:var(--g400);display:grid;justify-items:center;gap:8px}.lx-empty>.lx-ic{width:26px;height:26px}.lx-empty-t{font-size:16px;color:var(--g300)}.lx-empty-h{font-size:13.5px;max-width:520px}.lx-empty-a{display:flex;gap:8px;margin-top:6px}
.lx-modic{width:42px;height:42px;border-radius:5px;flex:none;display:inline-block}
.lx-row{display:flex;align-items:center;gap:14px;padding:12px 18px;min-height:64px}.lx-row-main{flex:1;min-width:0}.lx-row-t{font-size:16px;display:flex;align-items:center;gap:8px;flex-wrap:wrap}.lx-row-s{color:var(--g400);font-size:13.5px;margin-top:2px}
.lx-row-a{display:flex;align-items:center;gap:8px;flex:none}.lx-row-v{color:var(--g400);font-size:14px}
.lx-tbar{display:flex;align-items:center;gap:10px;margin-bottom:16px}.lx-grow{flex:1}
.lx-subh{display:flex;align-items:center;gap:10px;font-size:14px;color:var(--g400);margin:20px 2px 10px}
.lx-dlg{-webkit-font-smoothing:antialiased;line-height:1.35;position:relative;background:var(--g850);border:1px solid var(--g600);border-radius:18px;padding:38px 48px 34px;width:640px;max-height:92%;display:flex;flex-direction:column;box-shadow:0 20px 60px rgba(0,0,0,.5);color:var(--g300);font-family:Satoshi;font-size:15px;text-align:left}
.lx-dlg-vis .lx-dlg-body{overflow:visible}.lx-acctmenu{top:calc(100% + 6px);left:0;right:0}.lx-rmenu{top:calc(100% + 4px);right:0}.lx-dlg-xl{width:1180px}
.lx-dlg-wide{width:960px}.lx-dlg-narrow{width:560px}
.lx-dlg-head{display:flex;align-items:center;gap:14px;margin-bottom:20px}.lx-dlg-title{font-family:Satoshi;font-weight:700;font-size:31px;margin:0;flex:1;line-height:1.15}
.lx-dlg-body{overflow:auto;min-height:0;display:grid;gap:14px;align-content:start}.lx-dlg-body p{margin:0;color:var(--g300)}.lx-dlg-body .lx-muted,.lx-muted{color:var(--g400)}
.lx-dlg-foot{display:flex;gap:10px;margin-top:24px;align-items:center}.lx-dlg-foot .lx-btn{font-size:15px;padding:7px 12px}
.lx-dlg .lx-ibtn{width:34px;height:34px}
.lx-flabel{font-size:14px;color:var(--g400);margin-bottom:6px}.lx-form{display:grid;grid-template-columns:150px 1fr;gap:14px 16px;align-items:center}
.lx-code{font-family:"JetBrains Mono";font-size:13px;line-height:1.75;background:var(--g900);border:1px solid var(--g700);border-radius:7px;padding:12px 14px;white-space:pre;overflow:auto;color:var(--g300)}
.lx-code-err{color:#ff7272;border-color:rgba(255,92,92,.55);white-space:pre-wrap}
.lx-mono{font-family:"JetBrains Mono";font-size:13px}
.lx-check{display:flex;align-items:flex-start;gap:9px;font-size:14.5px;cursor:pointer}.lx-check>i{width:17px;height:17px;border-radius:4px;border:1px solid var(--g500);flex:none;margin-top:1px;display:inline-flex;align-items:center;justify-content:center}.lx-check.on>i{background:var(--blue);border-color:var(--blue)}.lx-check.on>i::after{content:"";width:8px;height:4px;border-left:2px solid #fff;border-bottom:2px solid #fff;transform:rotate(-45deg) translate(1px,-1px)}
.lx-radio{display:flex;gap:9px;align-items:center;font-size:14.5px}.lx-radio>i{width:16px;height:16px;border-radius:50%;border:1px solid var(--g500)}.lx-radio.on>i{border:5px solid var(--blue)}
.lx-facts{display:grid;grid-template-columns:150px 1fr;gap:9px 14px;font-size:14px}.lx-facts>:nth-child(odd){color:var(--g400)}
.lx-scard{background:var(--g900);border:1px solid var(--g700);border-radius:9px;padding:16px 18px;display:grid;gap:12px}
.lx-steps{display:grid;gap:0}.lx-step{display:grid;grid-template-columns:26px 1fr;gap:12px;position:relative;padding-bottom:14px}
.lx-step::before{content:"";position:absolute;left:12px;top:24px;bottom:2px;width:1px;background:var(--g600)}.lx-step:last-child::before{display:none}
.lx-step-n{width:24px;height:24px;border-radius:50%;border:1px solid var(--g500);color:var(--g400);display:flex;align-items:center;justify-content:center;font-size:13px}
.lx-step.done .lx-step-n{background:var(--green);border-color:var(--green);color:#fff}.lx-step.next .lx-step-n{border:2px solid var(--blue2);color:var(--blue2)}
.lx-step-t{font-size:16px;color:var(--g400);padding-top:1px}.lx-step.next .lx-step-t{color:var(--g300);font-weight:500}.lx-step-d{color:var(--g400);font-size:13.5px;margin-top:6px}.lx-step-a{margin-top:10px;display:flex;gap:8px}
.lx-hostsec{background:var(--g850);border:1px solid var(--g700);border-radius:10px;margin-bottom:18px;overflow:visible}
.lx-hostsec-h{display:flex;align-items:center;gap:14px;padding:16px 18px}.lx-hostsec-h>.lx-ic{width:20px;height:20px;color:var(--g400)}.lx-hostsec-t{font-family:Chillax;font-weight:600;font-size:19px}.lx-hostsec-s{color:var(--g400);font-size:14px}
.lx-hostsec>*+*{border-top:1px solid var(--g700)}
.lx-pagehead{display:flex;align-items:flex-start;gap:16px;margin-bottom:18px}.lx-pagehead .lx-grow>.lx-sub{margin-top:0}
.lx-areah{display:flex;align-items:flex-end;gap:12px;margin:22px 0 10px}.lx-areah-t{font-family:Chillax;font-weight:600;font-size:20px}.lx-areah-s{color:var(--g400);font-size:14px}
.lx-core{display:flex;align-items:center;gap:14px;padding:16px 18px}.lx-core-local{background:linear-gradient(180deg,rgba(46,160,120,.12),rgba(46,160,120,.03))}
.lx-corebox{border:1px solid var(--g700);border-radius:10px;overflow:visible;margin-bottom:14px;background:var(--g850);position:relative}.lx-corebox.lx-local{border-color:#2f7d63}
.lx-corebox>*+*{border-top:1px solid var(--g700)}.lx-corebox.lx-local>*+*{border-top-color:#2b5a4b}
.lx-core-k{font-size:12px;color:#3fd0b7}.lx-core-n{font-family:Chillax;font-weight:600;font-size:19px}.lx-core-s{color:var(--g400);font-size:13.5px}
.lx-srvrow{display:flex;align-items:center;gap:14px;padding:12px 18px}.lx-srvrow-n{font-size:16px;display:flex;gap:8px;align-items:center}
.lx-pill-tag{display:inline-flex;align-items:center;gap:6px;font-size:12px;border-radius:999px;padding:1px 4px 1px 9px;color:#3fd0b7;border:1px solid rgba(63,208,183,.5)}.lx-pill-tag button{background:none;border:0;color:inherit;padding:0 3px;display:inline-flex;gap:4px;align-items:center;font-size:12px}
.lx-console{background:var(--g900);border:1px solid var(--g700);border-radius:8px;font-family:"JetBrains Mono";font-size:13.5px;line-height:1.7;padding:12px 14px;white-space:pre;overflow:hidden;flex:1;min-height:0;position:relative}
.lx-c-warn{color:var(--yellow)}.lx-c-err{color:#ff6b6b}.lx-c-ok{color:var(--green2)}.lx-c-blue{color:var(--blue2)}.lx-c-dim{color:var(--g500)}
.lx-sdlg{width:1180px;height:720px;padding:0;display:grid;grid-template-columns:254px 1fr;overflow:hidden}
.lx-snav{background:var(--g850);border-right:1px solid var(--g700);padding:26px 14px 18px;display:flex;flex-direction:column}
.lx-snav-acct{display:flex;gap:12px;align-items:center;padding:0 8px 18px;cursor:pointer}.lx-snav-acct .lx-av{width:40px;height:40px}
.lx-snav-g{font-size:14px;color:var(--g400);margin:14px 10px 8px}
.lx-snav button{display:flex;align-items:center;gap:10px;width:100%;background:none;border:0;padding:8px 10px;border-radius:6px;font-size:16px;color:var(--g300);text-align:left}.lx-snav button.on{background:var(--g750)}.lx-snav button .lx-ic{color:var(--g400)}
.lx-smain{overflow:auto;padding:38px 46px 40px;position:relative;background:var(--g850)}
.lx-smain .lx-card{max-width:800px;background:var(--g850)}.lx-smain .lx-sec{max-width:800px}
.lx-sclose{position:absolute;top:14px;right:16px;z-index:2}
.lx-stitle{font-family:Chillax;font-weight:600;font-size:24px;margin:0 0 18px}
.lx-swatches{display:flex;gap:7px;align-items:center}.lx-swatches i{width:20px;height:20px;border-radius:50%;display:inline-block}
.lx-chip{border:1px solid var(--g400);border-radius:999px;padding:1px 8px;font-size:13px}
.lx-keycap{display:inline-flex;align-items:center;justify-content:center;min-width:34px;padding:3px 8px;border:1px solid var(--g600);border-bottom-width:3px;border-radius:5px;background:var(--g800);font-size:13px;margin:3px}
.lx-grid2{display:grid;grid-template-columns:1fr 1fr;gap:12px}.lx-grid3{display:grid;grid-template-columns:repeat(3,1fr);gap:12px}.lx-grid5{display:grid;grid-template-columns:repeat(5,1fr);gap:10px}
.lx-lcard{border:1px solid var(--g700);border-radius:8px;padding:14px;background:var(--g800);cursor:pointer;display:grid;gap:6px;align-content:start}.lx-lcard.on{border-color:var(--blue2);background:#203a4a}.lx-lcard-l{width:28px;height:28px;border-radius:5px;display:flex;align-items:center;justify-content:center;font-size:13px;font-weight:700}
.lx-lcard-d{color:var(--g400);font-size:13.5px}
.lx-mgroup{margin-bottom:26px}.lx-mgroup h3{font-family:Chillax;font-weight:600;font-size:18px;margin:0 0 4px}.lx-mgroup .lx-mg-d{color:var(--g500);font-size:13px;margin-bottom:10px}
.lx-mlist{display:grid;gap:8px}.lx-mlist .lx-toast{box-shadow:none;max-width:640px}
.lx-mkey{font-family:"JetBrains Mono";font-size:10.5px;color:var(--g550);margin-top:3px}
.lx-shot{aspect-ratio:16/10;border-radius:6px;background:linear-gradient(180deg,#7fb8e8 0 45%,#5d9440 45% 52%,#6b6f73 52%);border:1px solid var(--g700)}
.lx-world-ic{width:56px;height:56px;border-radius:4px;background:linear-gradient(180deg,#8cc0ec 0 50%,#5d9440 50% 58%,#7a7e83 58%);flex:none}
.lx-popover{position:absolute;z-index:40}
.lx-unsaved{position:absolute;left:50%;transform:translateX(-50%);bottom:124px;display:flex;align-items:center;gap:14px;background:var(--g800);border:1px solid var(--g600);border-radius:10px;padding:10px 14px 10px 18px;box-shadow:0 10px 30px rgba(0,0,0,.4);z-index:20}
.lx-snavcol{display:grid;grid-template-columns:200px 1fr;gap:36px;align-items:start}
.lx-snavcol-nav{display:grid;gap:2px;position:sticky;top:0}.lx-snavcol-nav button{background:none;border:0;text-align:left;padding:8px 12px;border-radius:6px;color:var(--g400);font-size:15.5px}.lx-snavcol-nav button.on{background:var(--g800);color:var(--g300)}
.lx-perf{width:520px}.lx-perf .lx-facts{grid-template-columns:170px 1fr}
.lx-spark{height:26px;background:linear-gradient(180deg,transparent 60%,rgba(89,178,243,.2));border-bottom:1px solid var(--blue2);border-radius:2px}
.lx-skin{border:1px solid var(--g700);border-radius:8px;overflow:hidden;cursor:pointer}.lx-skin.on{border-color:var(--blue2)}.lx-skin-p{height:70px}.lx-skin-n{padding:7px 10px;font-size:14px}
`;
  if (!document.getElementById("lx-style")) { const st = document.createElement("style"); st.id = "lx-style"; st.textContent = CSS; document.head.append(st) }

  // ---------- sample data ----------
  const INST = [
    { id: "smp", name: "Survival SMP", ver: "1.20.1", loader: "Fabric", lver: "0.16.14", grp: "servers", linked: true, played: "just" },
    { id: "atm", name: "All the mods", ver: "1.20.1", loader: "Forge", lver: "47.4.28", grp: "modded", played: "on", modpack: true },
    { id: "old", name: "Old school", ver: "1.8.9", grp: "Survival", played: "ago" },
    { id: "surv", name: "Survival 1.20.1", ver: "1.20.1", loader: "Fabric", lver: "0.16.14", grp: "Survival", played: null },
    { id: "latest", name: "Latest release", ver: "26.3", grp: "vanilla", played: "yesterday" },
  ];
  const instById = (id) => INST.find((i) => i.id === id) || INST[1];
  const loaderFull = (i) => i.loader ? i.loader + " " + i.lver : null;
  const gameLine = (i) => i.loader ? "Minecraft " + i.ver + " with " + loaderFull(i) : "Minecraft " + i.ver;
  const playedText = (i) => i.played === "on" ? fm("PlayedOn", { date: "Nov 7, 2025" }) : i.played === "ago" ? fm("PlayedAgo", { ago: "12 days ago" }) :
    i.played === "yesterday" ? fm("PlayedYesterday") : i.played === "just" ? fm("PlayedJustNow") : fm("NotPlayedYet");
  const ACCTS = { microsoft: { name: "Lodestoner", kind: "microsoft" }, offline: { name: "Dev_Steve", kind: "offline" }, launcher: { name: "Lodestoner", kind: "launcher" } };

  const isInst = (ctx) => ctx.route === "instance" || ctx.route.startsWith("inst-");
  // ---------- frame ----------
  function topBar(ctx) {
    const S = ctx.state;
    const upd = S.update;
    return h("div.lx-top" + (S.build === "dev" ? ".lx-dev" : ""),
      h("div.lx-arrows",
        h("button", { disabled: !ctx.canBack(), onclick: () => ctx.back(), title: "" }, ic("chevL")),
        h("button", { disabled: !ctx.canForward(), onclick: () => ctx.forward() }, ic("chevR"))),
      h("div.lx-logo", logoMark(), tx("Lodestone"), S.build === "dev" ? h("span.lx-devb", sh("DevBadge")) : null),
      h("div.lx-upd",
        upd === "available" ? [h("span.lx-pill", sh("UpdateAvailableBadge")), btn(sh("DownloadUpdate"), { sm: true, kind: "p", on: () => ctx.set("update", "ready") })] :
          upd === "ready" ? [h("span.lx-pill", sh("UpdateReadyBadge")), btn(sh("RestartToUpdate"), { sm: true, kind: "p" })] :
            upd === "downloading" ? h("span.lx-muted", sh("UpdateDownloading", { build: 1412, percent: "42%" })) : null));
  }
  function accountBox(ctx) {
    const a = ctx.state.account;
    let av, name, sub;
    if (a === "signedOut") { av = avatar("offline"); name = sh("NoAccount"); sub = sh("AddMinecraftAccount") }
    else if (a === "offline") { av = avatar("offline"); name = tx("Dev_Steve"); sub = sh("OfflineAccount") }
    else {
      av = avatar(); name = tx("Lodestoner");
      sub = a === "launcher" ? sh("LauncherIdentity") : a === "expired" ? sh("SignInAgain") : a === "signingIn" ? sh("SigningIn") : a === "noNetwork" ? sh("OfflineCouldntConnect") : sh("MicrosoftAccount");
    }
    return h("div.lx-acct", { onclick: toggleMenu("acct") }, av, h("div", h("div.lx-acct-n", name), h("div.lx-acct-s", sub)), h("span.lx-caretic", ic("caret")),
      ui.menu === "acct" ? menu([
        ...(a === "signedOut" ? [] : [
          { label: tx("Dev_Steve"), icon: "user", right: h("span.lx-mright", sh("Offline"), a === "offline" ? ic("check") : null), on: () => { ui.menu = null; ctx.set("account", "offline") } },
          { label: tx("Lodestoner"), icon: "user", right: h("span.lx-mright", tx("Microsoft"), a !== "offline" ? ic("check") : null), on: () => { ui.menu = null; ctx.set("account", "microsoft") } }, "-"]),
        { label: sh("AddMicrosoftAccount"), icon: "plus", on: () => { ui.menu = null; ctx.open("signin") } },
        { label: sh("AddOfflineAccount"), icon: "plus", on: () => { ui.menu = null; ctx.open("offlineAccount") } },
        ...(a === "signedOut" ? [] : ["-", { label: sh("RemoveNamed", { name: a === "offline" ? "Dev_Steve" : "Lodestoner" }), icon: "signout", danger: true, on: () => { ui.menu = null; ctx.open("removeAccount") } }]),
      ], "lx-acctmenu") : null);
  }
  function instRow(ctx, i, sel, opts = {}) {
    const sub = opts.sub || tx(i.loader ? i.ver + " \u00b7 " + i.loader : i.ver);
    const preparing = ctx.state.game === "preparing" && i.id === (ctx.params.id || "atm") && isInst(ctx);
    return h("div.lx-irow" + (sel ? ".on" : ""), { onclick: () => ctx.go("instance", { id: i.id, tab: ctx.params.tab || "play" }) },
      grass(), h("div", { style: { minWidth: 0 } }, h("div.lx-irow-n", tx(i.name)), h("div.lx-irow-s", sub)),
      h("span.lx-iplay", { title: "", onclick: (e) => { e.stopPropagation(); ctx.go("instance", { id: i.id, tab: "play" }); ctx.set("game", "preparing") } }, ic("play")),
      preparing ? h("div.lx-iprog", progress(34)) : null);
  }
  function sidebar(ctx) {
    const S = ctx.state;
    const selId = isInst(ctx) ? (ctx.params.id || "atm") : null;
    const kids = [accountBox(ctx)];
    if (S.instances === "none") {
      kids.push(h("div.lx-sidecap", sh("Instances")), h("div.lx-noinst", h("div", { style: { fontSize: "26px", fontStyle: "normal" } }, "\u26f6"), sh("NoInstancesYet")));
    } else {
      kids.push(h("div.lx-sidecap", sh("LastPlayed")), instRow(ctx, INST[1], false, { sub: fm("ShortDateWithYear", { month: "Nov", day: 7, year: 2025 }) }));
      kids.push(h("div.lx-sidecap", sh("Instances")));
      const groups = S.groups === "off" ? [[null, INST]] : [
        [sh("GroupServers"), INST.filter((i) => i.grp === "servers")],
        [sh("GroupModded", { loader: "Forge" }), INST.filter((i) => i.grp === "modded")],
        [tx("Survival"), INST.filter((i) => i.grp === "Survival")],
        [sh("Vanilla"), INST.filter((i) => i.grp === "vanilla")],
      ];
      groups.forEach(([label, list]) => {
        if (label) kids.push(h("div.lx-grp", h("span.lx-tri"), label, h("span.lx-n", tx(list.length))));
        list.forEach((i) => kids.push(instRow(ctx, i, i.id === selId)));
      });
      if (S.instances === "importing") kids.push(h("div.lx-irow", grass(), h("div", h("div.lx-irow-n", tx("Fabulously Optimized")), h("div.lx-irow-s", sh("ImportProgress", { percent: "45%", label: "Downloading mods" })), h("div", { style: { marginTop: "4px" } }, progress(45)))));
    }
    kids.push(h("div", { style: { height: "8px" } }),
      h("button.lx-sidelink", { onclick: () => ctx.open("newInstance", { mode: "version" }) }, ic("plus"), sh("NewInstance")),
      h("button.lx-sidelink", { onclick: () => ctx.open("import") }, ic("dl"), sh("ImportInstances")));
    return h("div.lx-side", h("div.lx-side-scroll", { "data-mk-scroll": "lx-side" }, kids),
      h("div.lx-side-foot",
        h("button.lx-sidelink" + (ctx.route === "host" || ctx.route === "server" ? ".on" : ""), { onclick: () => ctx.go("host") }, logoMark(), sh("Servers")),
        h("button.lx-sidelink", { onclick: () => ctx.open("settings", { page: "general" }) }, ic("gear"), sh("Settings")),
        h("div.lx-ver", sh("AppVersion", { version: "0.1.0" }))));
  }
  function toastArea(ctx) {
    const k = ctx.state.toast;
    if (!k || k === "none") return null;
    const close = () => ctx.set("toast", "none");
    const T = {
      narrator: () => toast(sh("NarratorOfferTitle"), sh("NarratorOfferBody", {}), "info", [pbtn(sh("NarratorOfferAccept"), close), btn(sh("NarratorOfferDismiss"), { on: close })]),
      undo: () => toast(null, sh("Undid", { change: "Language \u2192 System" }), "info", [btn(sh("Redo"), { sm: true })]),
      redo: () => toast(null, sh("Redid", { change: "Zoom \u2192 110%" }), "info"),
      settingsOf: () => toast(null, sh("Undid", { change: "Settings of All the mods" }), "info"),
      crashed: () => toast(sh("CrashTitle", { name: "All the mods" }), sh("MinecraftExitCode", { code: 1 }), "err", [pbtn(sh("OpenCrashReport"), () => ctx.open("crash"))]),
      signInExpired: () => toast(null, sh("SignInExpired", { name: "Lodestoner" }), "warn", [pbtn(sh("SignInAgain"), () => ctx.open("signin"))]),
      addAccount: () => toast(null, sh("AddAccountFirst"), "warn"),
      ready: () => toast(sv("ReadyTitle", { name: "Survival SMP" }), sv("ReadyBody", { version: "1.20.1", loader: "Fabric 0.16.14" }), "ok"),
      shareReady: () => toast(im("ShareCodeReady", { name: "Survival 1.20.1" }), imp("ShareCodeInstalled", { count: 14 }), "ok"),
      switched: () => toast(im("SwitchedTo", { loader: "Fabric" }), im("SwitchedToMessage", { name: "Latest release", loader: "Fabric", version: "0.16.14" }), "ok"),
      verified: () => toast(im("GameFilesVerified"), im("EveryFileIntact", { summary: "Checked 4,091 game files in 45 ms." }), "ok"),
      copiedKey: () => toast(sv("OwnerKeyCopied", { name: "Peter's Lodestone Core" }), sv("OwnerKeyCopiedHint"), "ok"),
      copiedInvite: () => toast(sv("InviteLinkCopied", { name: "Survival SMP" }), sv("InviteLinkCopiedHint"), "ok"),
      skinCopied: () => toast(null, sh("SkinCopied", { name: "Glass" }), "info"),
      firstTime: () => toast(null, sh("FirstTimeSetupReset"), "ok"),
      synced: () => toast(null, sh("SyncedFrom", { name: "Survival 1.20.1" }), "ok"),
      copiedValues: () => toast(null, shp("CopiedValues", { count: 38, name: "Survival 1.20.1" }), "ok"),
    };
    return h("div.lx-toasts", (T[k] || (() => null))());
  }
  function frame(ctx, page) {
    const root = h("div.lx-root", { onclick: () => { if (ui.menu) { ui.menu = null; rerender() } } },
      topBar(ctx), h("div.lx-body", sidebar(ctx), page), toastArea(ctx));
    return root;
  }

  // ---------- instance page ----------
  const TABS = [["play", "Play"], ["content", "TabContent"], ["worlds", "TabWorlds"], ["settings", "Settings"], ["logs", "TabLogs"]];
  function playBar(ctx, inst) {
    const g = ctx.state.game;
    if (!inst) return h("div.lx-playbar", h("button.lx-play.lx-off", sh("Play")));
    if (ctx.state.account === "signedOut") return h("div.lx-playbar", h("button.lx-play.lx-off", sh("Play")), h("div.lx-play-sub", sh("AddAccountFirst")));
    if (g === "preparing") return h("div.lx-playbar",
      h("div.lx-play-note", im("PreparingNote", { version: inst.ver, loader: inst.loader || "Vanilla" }), inst.linked ? h("div", im("JoiningNote", { address: "127.0.0.1:25565" })) : null),
      h("div.lx-play-prog", h("div.lx-pl", h("span", im("StageAssets")), h("span", fm("Percent", { value: 34 }))), progress(34),
        h("div.lx-pl", h("span", im("ProgressDetail", { files_done: 812, files_total: 3904, done: 212, total: 514 })))),
      btn(cm("Cancel"), { on: () => ctx.set("game", "stopped") }));
    if (g === "launching") return h("div.lx-playbar", h("button.lx-play.lx-launch", spinner(), sh("Launching")), h("div.lx-play-sub", im("StageLaunching")));
    if (g === "playing") return h("div.lx-playbar", h("div.lx-play.lx-playing", h("span", dot("on"), " ", sh("Playing")), btn(cm("Stop"), { on: () => ctx.set("game", "stopped") })));
    if (g === "stopping") return h("div.lx-playbar", h("button.lx-play.lx-launch", spinner(), im("StoppingNote")));
    if (g === "verifying") return h("div.lx-playbar", h("div.lx-play-prog", h("div.lx-pl", h("span", im("VerifyingNote", { version: inst.ver, loader: inst.loader || "Vanilla" }))), progress(62)), btn(cm("Cancel")));
    if (g === "modpackUpdating") return h("div.lx-playbar", h("button.lx-play.lx-off", sh("Play")), h("div.lx-play-sub", im("ModpackUpdating"), " \u2014 ", im("WaitForUpdate")));
    return h("div.lx-playbar",
      inst.id === "old" ? h("div.lx-play-note", im("RosettaNote")) : null,
      h("button.lx-play", { onclick: () => ctx.set("game", "preparing") }, sh("Play")),
      inst.linked ? h("div.lx-play-sub", sh("AndJoin", { server: "Survival SMP" }), " \u00b7 ", h("a", { style: { color: "var(--blue2)", cursor: "pointer" } }, sh("PlayWithoutJoining"))) : null);
  }
  function instHeader(ctx, inst) {
    if (!inst) return [h("h1.lx-h1", sh("NoInstanceSelected")), h("div.lx-sub", sh("NoInstanceSelectedHint"))];
    const parts = [sh("MinecraftVersion", { version: inst.ver })];
    if (inst.loader) parts.push(tx(loaderFull(inst)));
    if (inst.played === "on") parts.push(fm("HoursMinutes", { hours: 5, minutes: 0 }));
    if (inst.played === "just") parts.push(fm("Minutes", { minutes: 42 }));
    if (inst.played === "yesterday") parts.push(fm("LessThanAMinute"));
    parts.push(playedText(inst));
    return [h("h1.lx-h1", tx(inst.name)), h("div.lx-sub", join(parts)),
      inst.linked ? h("div.lx-sub", ctx.state.linkedCore === "removed" ? sh("LinkedToRemoved", { server: "Survival SMP", core: "Peter's Lodestone Core" }) : sh("LinkedTo", { server: "Survival SMP", core: "Peter's Lodestone Core" })) : null,
      inst.id === "old" ? h("div.lx-muted", { style: { fontSize: "12px" } }, sh("RunsUnderRosetta")) : null];
  }
  function instancePage(ctx) {
    const none = ctx.state.instances === "none" || ctx.route === "home";
    const inst = none ? null : instById(ctx.params.id || "atm");
    const tab = none ? (ctx.params.tab || "play") : (ctx.params.tab || "play");
    const go = (t2) => none ? ctx.go("home", { tab: t2 }) : ctx.go("instance", { id: inst.id, tab: t2 });
    let body;
    if (none) {
      body = tab === "play" ? h("div.lx-hero", h("h2.lx-hero-t", sh("CreateFirstInstance")), h("div.lx-hero-p", sh("InstanceExplainer")), pbtn(sh("NewInstance"), () => ctx.open("newInstance", { mode: "version" }), "plus")) :
        empty(null, { content: sh("ContentPlaceholder"), worlds: sh("WorldsPlaceholder"), settings: sh("SettingsPlaceholder"), logs: sh("LogsPlaceholder") }[tab]);
    } else body = ({ play: playTab, content: contentTab, worlds: worldsTab, settings: settingsTab, logs: logsTab }[tab] || playTab)(ctx, inst);
    return h("div.lx-main",
      h("div.lx-page", { "data-mk-scroll": "lx-page-" + tab },
        instHeader(ctx, inst),
        h("div.lx-tabs", TABS.map(([k, s]) => h("button" + (tab === k ? ".on" : ""), { onclick: () => go(k) }, sh(s)))),
        body),
      tab === "settings" && ctx.state.instSettings === "unsaved" ? h("div.lx-unsaved", cm("UnsavedChanges"), btn(cm("Discard"), { on: () => ctx.set("instSettings", "saved") }), pbtn(cm("Save"), () => ctx.set("instSettings", "saved"))) : null,
      playBar(ctx, inst));
  }
  function playTab(ctx, inst) {
    const g = ctx.state.game;
    return [
      g === "crashed" ? note([sh("MinecraftExitCode", { code: 1 }), " "], "err", btn(sh("OpenCrashReport"), { sm: true, on: () => ctx.open("crash") })) : null,
      ctx.state.account === "expired" ? note(sh("SignInExpired", { name: "Lodestoner" }), "warn", btn(sh("SignInAgain"), { sm: true, on: () => ctx.open("signin") })) : null,
      h("div.lx-hero", h("h2.lx-hero-t", tx(inst.name)),
        h("div.lx-hero-p", inst.played ? sh("HeroPlayed", { game: gameLine(inst), time: "5 h 0 min" }) : sh("HeroNotPlayed", { game: gameLine(inst) }))),
    ];
  }
  function logsTab(ctx, inst) {
    const st = ctx.state.logs;
    const lvl = (l) => h("span", { class: l === "WARN" ? "lx-c-warn" : l === "ERROR" ? "lx-c-err" : l === "DEBUG" ? "lx-c-dim" : "" }, tx(l.padEnd(6)));
    const L1 = (time, level, src, msg, cls) => h("div", h("span.lx-c-dim", tx(time + "  ")), lvl(level), h("span.lx-c-dim", tx(src.padEnd(14))), h("span", { class: cls || "" }, msg));
    const launcher = (m) => L1("02:20:00", "INFO", "Launcher", m, "lx-c-blue");
    const lines = [
      launcher(im("PreparingNote", { version: inst.ver, loader: inst.loader || "Vanilla" })),
      launcher(im("AutoMemoryNote", { memory: "2 GB \u2013 6 GB" })),
      launcher(im("AutoJvmArgsNote")),
      launcher(im("AutoVideoNote", { distance: 12 })),
      launcher(im("AutoControlsNote", { source: "Survival 1.20.1" })),
      launcher(isp("SharedWritten", { count: 42 })),
      launcher(im("StartingJavaNote", { command: "/\u2026/runtimes/java-runtime-gamma/bin/java -Xss1M -Xmx6G \u2026" })),
      L1("02:20:02", "INFO", "main", tx("Setting user: Lodestoner")),
      L1("02:20:03", "WARN", "Render thread", tx("Missing sound for event: minecraft:item.goat_horn.play"), "lx-c-warn"),
      L1("02:20:05", "INFO", "Render thread", tx("Sound engine started")),
      L1("02:20:06", "DEBUG", "Render thread", tx("Narrator library for arm64 successfully loaded")),
      L1("02:20:08", "ERROR", "Render thread", tx("Unreported exception thrown!"), "lx-c-err"),
      h("div.lx-c-err", tx("                              java.lang.IllegalStateException: Lock is no longer valid")),
      L1("02:20:08", "INFO", "Render thread", tx("Stopping!")),
      launcher(isp("SharedEmitted", { count: 2 })),
      launcher(im("ExitedWithCode", { code: 1 })),
    ];
    return [
      h("div.lx-tbar", h("div", { style: { width: "300px" } }, field(st === "nomatch" ? "goat_hornz" : null, im("SearchLog"))),
        btn(im("LevelDebug"), { cls: "lx-lv" }), btn(im("LevelInfo")), h("button.lx-btn", { style: { color: "var(--yellow)", borderColor: "rgba(239,180,64,.6)" } }, im("LevelWarn")),
        h("button.lx-btn", { style: { color: "var(--red)", borderColor: "rgba(255,92,92,.6)" } }, im("LevelError")), h("span.lx-grow"), btn(im("CopyLog"), { icon: "copy" })),
      st === "empty" ? h("div.lx-console", { style: { height: "380px", whiteSpace: "normal", color: "var(--g500)" } }, im("LogEmpty")) :
        st === "nomatch" ? h("div.lx-console", { style: { height: "380px", whiteSpace: "normal", color: "var(--g500)" } }, im("NoLinesMatch")) :
          h("div.lx-console", { style: { height: "400px" } }, lines, h("div", { style: { position: "absolute", right: "14px", bottom: "10px" } }, btn(im("JumpToLatest"), { sm: true }))),
      h("div.lx-subh", { style: { fontSize: "12.5px" } }, st === "empty" ? null : imp("LinesShownFollowing", { shown: 16, count: 16 }), st === "nomatch" ? imp("LinesShown", { shown: 0, count: 16 }) : null),
    ];
  }

  // ---------- Content tab ----------
  const MODS = [
    ["Fabric API", "0.92.2+1.20.1", "fabric-api-0.92.2+1.20.1.jar", 208, 45, { update: "0.92.6+1.20.1" }],
    ["Homemade Tweaks", "1.0.0", "homemade-tweaks.jar", 661, 25, { notOn: true }],
    ["Iris Shaders", "1.7.6+1.20.1", "iris-1.7.6+mc1.20.1.jar", 198, 260],
    ["Lithium", "mc1.20.1-0.11.2", "lithium-fabric-0.11.2.jar", 193, 140],
    ["Mod Menu", "7.2.2", "modmenu-7.2.2.jar.disabled", 196, 280, { off: true }],
    ["Sodium", "mc1.20.1-0.5.13", "sodium-fabric-0.5.13+mc1.20.1.jar", 198, 200, { cf: true }],
  ];
  const PACKS = [["Faithful 32x", "1.20.1", "Faithful 32x.zip", 180, 30], ["Fresh Animations", "1.9.1", "FreshAnimations_v1.9.1.zip", 220, 120], ["My pack", null, "my-pack", 0, 200, { folder: true }]];
  const SHADERS = [["Complementary Reimagined", "r5.2.2", "ComplementaryReimagined_r5.2.2.zip", 190, 20], ["BSL Shaders", "v8.2.09", "BSL_v8.2.09.zip", 170, 210]];
  function contentRow(ctx, m, kind) {
    const [name, ver, file, bytes, hue, o = {}] = m;
    return h("div.lx-row", modIcon(hue),
      h("div.lx-row-main", h("div.lx-row-t", tx(name), ver ? h("span.lx-row-v", tx(ver)) : null, o.notOn ? badge(ct("NotOnModrinth")) : null, o.cf ? badge(ct("CurseForgeTab"), "yellow") : null),
        h("div.lx-row-s", o.folder ? ct("ItemFolder", { name: file }) : [tx(file), sep(), fmp("Bytes", { count: bytes })])),
      h("div.lx-row-a", o.update ? pbtn(ct("UpdateTo", { version: o.update }), null, "ul") : null, o.notOn ? null : iconBtn("ext"), toggle(!o.off), iconBtn("trash")));
  }
  function contentTab(ctx, inst) {
    const S = ctx.state, p = ctx.params;
    const kind = p.kind || "mods", view = p.view === "browse" ? "modrinth" : p.view || "installed";
    const goC = (extra) => ctx.go("instance", Object.assign({ id: inst.id, tab: "content", kind, view }, extra));
    const target = inst.ver + " / " + (inst.loader || "Vanilla");
    const vanilla = !inst.loader || S.content === "vanilla";
    const head = h("div.lx-tbar",
      seg([["mods", ct("KindMods"), 6], ["rp", ct("KindResourcePacks"), 3], ["shaders", ct("KindShaders"), 2]], kind, (k) => goC({ kind: k, view: "installed" })),
      h("div.lx-grow", field(view === "modrinth" ? "sodium" : null, ct(view === "modrinth" ? "SearchModrinth" : view === "curseforge" ? "SearchCurseForge" : "SearchInstalled"), { icon: "search" })),
      kind === "mods" && view === "installed" ? pbtn(ct("UpdateAll", { count: 1 }), null, "ul") : null,
      h("div", { style: { position: "relative" } }, iconBtn("dots", toggleMenu("content")),
        ui.menu === "content" ? menu([{ label: ct("CheckForUpdates"), icon: "refresh" }, { label: ct("AddFile"), icon: "plus" }, { label: ct("AddFolder"), icon: "folder" }, "-", { label: cm("OpenFolder"), icon: "folder" }], "lx-rmenu") : null));
    const out = [head];
    if (S.content === "gameRunning") out.push(note(ct("GameRunning"), "warn"));
    if (S.content === "installing") out.push(h("div.lx-scard", h("div", ct("CheckingProject", { name: "Sodium" }), " \u2014 ", ct("CheckingDependencies")), progress(55),
      h("div.lx-muted", ct("FilesProgress", { done: 3, total: 5 }), sep(), ct("BytesProgress", { done: "1.2 MB", total: "3.4 MB" }))));
    if (S.content === "installed") out.push(note([ct("InstalledMany", { names: "Iris, Sodium", last: "Fabric API" }), " ", ct("Replaced", { files: "sodium-fabric-0.5.11.jar" }), " ", ct("Skipped", { name: "Mod Menu", reason: "already installed" })], "ok"));
    if (view === "project") return out.concat(projectView(ctx, inst, goC));
    // Installed, Modrinth or CurseForge, one at a time; the platforms get their tools beside the tabs.
    out.push(h("div.lx-tbar", seg([["installed", ct("ViewInstalled"), kind === "mods" ? 6 : kind === "rp" ? 3 : 2], ["modrinth", ct("ViewModrinth"), "248"], ["curseforge", ct("ViewCurseForge"), "31"]], view, (v) => goC({ view: v })),
      h("span.lx-grow"),
      view === "modrinth" ? [btn(S.content === "filtered" ? ct("FiltersCount", { count: 2 }) : ct("Filters"), { sm: true }), h("label.lx-muted", { style: { display: "inline-flex", gap: "6px", alignItems: "center" } }, toggle(true), ct("ShowInstalled")), select(ct("SortRelevance")), h("span.lx-badge.lx-b-blue", { style: { fontSize: "13.5px", padding: "4px 10px", borderRadius: "999px" } }, "\u2713 ", ct("OnlyTarget", { target }))] : null));
    const backToTop = () => h("div.lx-tbar", { style: { justifyContent: "center", marginTop: "12px" } }, btn(ct("BackToTop"), { sm: true }));
    if (vanilla && kind !== "rp") {
      const m = kind === "mods";
      out.push(h("div.lx-scard", h("div.lx-row-t", m ? ct("ModsNeedLoader") : ct("ShadersNeedLoader")),
        h("div.lx-muted", m ? ct("VanillaSwitchMods", { loader: "Fabric" }) : ct("VanillaSwitchShaders", { loader: "Fabric" })),
        h("div", { style: { display: "flex", gap: "8px" } }, pbtn(ct("SwitchTo", { loader: "Fabric" }), () => ctx.open("switchLoader")), btn(ct("NewFabricInstance"), { on: () => ctx.open("newInstance", { mode: "version", loader: "fabric" }) }))));
      return out;
    }
    if (S.content === "loading") { out.push(empty(null, spinner(), ct("ReadingFolder", { folder: kind === "mods" ? "mods" : kind === "rp" ? "resourcepacks" : "shaderpacks" }))); return out }
    if (S.content === "readFailed") { out.push(note(ct("CouldntReadFolder", { folder: "mods", error: "Permission denied (os error 13)" }), "err")); return out }
    if (kind === "shaders") out.push(h("div.lx-scard", { style: { marginBottom: "14px" } },
      h("div.lx-row-t", ct("ShaderModUsingPack", { shader_mod: "Iris", pack: "Complementary Reimagined" })),
      h("div.lx-muted", ct("ShaderModPickPack", { shader_mod: "Iris", file: "config/iris.properties" }))));
    if (S.content === "empty") {
      out.push(h("div.lx-subh", ct("InstalledCount", { count: 0 })), empty(kind === "mods" ? "cube" : "image", kind === "mods" ? ct("NoModsYet") : kind === "rp" ? ct("NoResourcePacksYet") : ct("NoShaderPacksYet")));
    } else if (view !== "installed") {
      // the platform views come below
    } else {
      const list = kind === "mods" ? MODS : kind === "rp" ? PACKS : SHADERS;
      if (kind === "rp") {
        out.push(h("div.lx-subh", ct("EnabledCount", { count: 2 })), card(list.slice(0, 2).map((m) => contentRow(ctx, m, kind))),
          h("div.lx-subh", ct("AvailableCount", { count: 1 })), card(list.slice(2).map((m) => contentRow(ctx, m, kind))));
      } else {
        out.push(h("div.lx-subh", ct("InstalledCount", { count: list.length })), card(
          kind === "shaders" ? h("div.lx-row", h("span.lx-modic", { style: { background: "var(--g700)" } }), h("div.lx-row-main", h("div.lx-row-t", ct("ShadersOff")), h("div.lx-row-s", ct("NormalLook"))), h("span.lx-radio", h("i"))) : null,
          list.map((m) => contentRow(ctx, m, kind))));
      }
      if (S.content === "upToDate") out.push(note(kind === "mods" ? ct("AllModsUpToDate") : kind === "rp" ? ct("AllResourcePacksUpToDate") : ct("AllShaderPacksUpToDate"), "ok"));
      if (S.content === "curseforge") out.push(blockedCard());
    }
    if (view === "installed") {
      // "Looking for something new?", with a button to each platform's view.
      out.push(h("div.lx-tbar", { style: { justifyContent: "center", padding: "16px 0" } }, h("span.lx-muted", ct("LookingForMore")),
        btn(ct("BrowseModrinth"), { sm: true, icon: "search", on: () => goC({ view: "modrinth" }) }), btn(ct("BrowseCurseForge"), { sm: true, icon: "search", on: () => goC({ view: "curseforge" }) })));
      return out;
    }
    if (view === "curseforge") {
      out.push(card(h("div.lx-row", modIcon(15), h("div.lx-row-main", h("div.lx-row-t", tx("Sodium Plus"), badge(ct("WebsiteDownload"), "yellow")), h("div.lx-row-s", ct("ByAuthor", { author: "someone" }), sep(), ctp("Downloads", { count: 1 }))), h("div.lx-row-a", btn(ct("GetOnCurseForge"), { icon: "ext" })))), backToTop());
      return out;
    }
    // Modrinth's results
    if (S.content === "offline") out.push(empty("search", ct("BrowsingNeedsNetwork")));
    else if (S.content === "searching") out.push(empty(null, spinner(), ct("SearchingModrinth")));
    else if (S.content === "nothing") out.push(empty("search", ct("NothingMatches")));
    else {
      const R = [["Sodium", "jellysquid3", "98.4M", 200, true], ["Sodium Extra", "FlashyReese", "21.3M", 210], ["Reese's Sodium Options", "FlashyReese", "18.9M", 30], ["Sodium Dynamic Lights", "Lambda", "1.2M", 50]];
      out.push(card(R.map(([n, a, d, hue, inst2]) => h("div.lx-row", { style: { cursor: "pointer" }, onclick: () => goC({ view: "project" }) }, modIcon(hue),
        h("div.lx-row-main", h("div.lx-row-t", tx(n), n === "Sodium Dynamic Lights" ? badge(ct("OptionalOnServers")) : null),
          h("div.lx-row-s", ct("ByAuthor", { author: a }), sep(), ct("DownloadsCompact", { count: d }), sep(), ctp("Follows", { count: 3120 }))),
        h("div.lx-row-a", inst2 ? badge(ct("AlreadyInstalled"), "green") : pbtn(cm("Install"), null, "dl"))))));
      out.push(h("div.lx-tbar", { style: { justifyContent: "center", marginTop: "12px" } }, btn(cm("Previous"), { sm: true }), h("span.lx-muted", ct("PageOf", { page: 1, pages: 13 })), btn(cm("Next"), { sm: true })), backToTop());
    }
    return out;
  }
  function blockedCard() {
    return h("div.lx-scard", { style: { marginTop: "14px", borderColor: "rgba(239,180,64,.4)" } },
      h("div.lx-row-t", ct("BlockedTitle")), h("div.lx-muted", ct("BlockedLead", { folder: "~/Downloads" })),
      card(h("div.lx-row", modIcon(30), h("div.lx-row-main", h("div.lx-row-t", tx("Xaero's Minimap")), h("div.lx-row-s", ct("BlockedWaiting", { file: "Xaeros_Minimap_24.2.0_Fabric_1.20.jar" }))), btn(ct("BlockedOpen"), { icon: "ext" })),
        h("div.lx-row", modIcon(90), h("div.lx-row-main", h("div.lx-row-t", tx("JourneyMap")), h("div.lx-row-s", tx("journeymap-1.20.1-5.9.18-fabric.jar"))), badge(ct("BlockedAdded"), "green"))),
      h("div", { style: { display: "flex", gap: "8px" } }, pbtn(ct("BlockedOpenAll")), btn(ct("BlockedChangeFolder")), gbtn(ct("BlockedSkip"))),
      h("div.lx-muted", { style: { fontSize: "13px" } }, ct("BlockedNoFolder")));
  }
  function projectView(ctx, inst, goC) {
    const desc = ui.ctab === "description";
    return [
      h("div", { style: { margin: "4px 0 14px" } }, gbtn(cm("Back"), () => goC({ view: "modrinth" }), "back")),
      h("div", { style: { display: "flex", gap: "18px", alignItems: "flex-start" } }, h("span.lx-modic", { style: { width: "84px", height: "84px", background: "hsl(200 70% 62%)", boxShadow: "inset 0 0 0 12px hsl(200 55% 48%)" } }),
        h("div.lx-grow", h("div", { style: { fontFamily: "Chillax", fontWeight: 600, fontSize: "24px" } }, tx("Sodium")),
          h("div", { style: { color: "var(--g300)", fontSize: "15px", margin: "4px 0 6px" } }, tx("The fastest and most compatible rendering optimization mod for Minecraft.")),
          h("div.lx-muted", ct("DownloadsCompact", { count: "98.4M" }), sep(), ct("FollowersCompact", { count: "65.6k" }), sep(), ct("FollowsCompact", { count: "65.6k" }), " ", badge(tx("optimization"), "blue"), " ", badge(tx("fabric"), "blue"))),
        btn(ct("ViewOnModrinth"), { icon: "ext" })),
      h("div", { style: { margin: "18px 0 14px" } }, seg([["versions", ct("Versions")], ["description", ct("Description")]], desc ? "description" : "versions", (k) => { ui.ctab = k; rerender() })),
      desc ? h("div", { style: { maxWidth: "760px", color: "var(--g300)", display: "grid", gap: "10px" } }, h("p", tx("Sodium is a powerful rendering engine for Minecraft which greatly improves frame rates and micro-stutter, while fixing many graphical issues.")), h("p.lx-muted", sh("RestOnModrinth")), h("div", btn(ct("ViewOnModrinth"), { icon: "ext" }))) : [
        h("div.lx-tbar", h("span", { style: { fontFamily: "Chillax", fontWeight: 600, fontSize: "19px" } }, ct("Versions")), toggle(false), h("span.lx-muted", ct("ShowAllVersions")), toggle(true), h("span.lx-muted", ct("AlsoInstallDependencies"))),
        ctx.state.content === "noFit" ? note([ct("NoVersionFits", { project: "Sodium", show_all: "Show versions\u2026" }), " "], "warn", h("div", { style: { display: "flex", gap: "6px" } }, btn(ct("ShowVersionsShort"), { sm: true }), btn(ct("InstallAnyway"), { sm: true }))) : null,
        card(
          h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", tx("mc1.20.1-0.5.13"), badge(ct("Release"), "green")), h("div.lx-row-s", tx("1.20.1 \u00b7 fabric, quilt \u00b7 Mar 10, 2025 \u00b7 1.1 MB"))), badge(ct("InstalledBadge"), "green")),
          h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", tx("mc1.20.1-0.5.12-beta"), badge(ct("Beta"), "yellow")), h("div.lx-row-s", tx("1.20.1 \u00b7 fabric \u00b7 Feb 2, 2025 \u00b7 1.1 MB"))), pbtn(cm("Install"), null, "dl")),
          h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", tx("mc1.20.1-0.5.0-alpha"), badge(ct("Alpha"), "red")), h("div.lx-row-s", tx("1.20.1 \u00b7 fabric \u00b7 Aug 1, 2024 \u00b7 1.0 MB"))), btn(ct("InstallItAnyway"))),
          h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", tx("Iris 1.7.6"), badge(ct("Release"), "green")), h("div.lx-row-s", tx("Shader pack needs a shader mod"))), pbtn(ct("InstallWith", { shader_mod: "Iris" }), null, "dl"))),
        h("div.lx-muted", { style: { marginTop: "10px", fontSize: "13.5px" } }, ctp("MoreVersions", { count: 37 }))],
    ];
  }

  // ---------- Worlds tab ----------
  const WORLDS = [["Survival Island", "New World", "Today, 03:05", "Survival", false, "1.21.11", "40 MB", 2], ["Castle Build", "Castle", "2026-11-08", "Creative", true, "1.21.11", "117 MB", 0],
    ["One Life", "Hardcore run", "2026-11-01", "Hardcore", false, "1.20.1", "22 MB", 0, true], ["Sky test", "skytest", "2026-10-06", "Adventure", true, "24w14a", "6.3 MB", 0, false, true], ["Spectate", "spec", "2026-09-30", "Spectator", false, "1.20.1", "2.1 MB", 0]];
  function worldsTab(ctx, inst) {
    const S = ctx.state, sec = ctx.params.sec || "worlds";
    const out = [h("div.lx-tbar", seg([["worlds", wd("Worlds"), 5], ["shots", wd("Screenshots"), 6]], sec, (k) => ctx.go("instance", { id: inst.id, tab: "worlds", sec: k })))];
    if (sec === "shots") {
      if (S.worlds === "empty") return out.concat(empty("image", wd("NoScreenshotsYet"), wd("NoScreenshotsHint")));
      if (S.worlds === "reading") return out.concat(empty(null, spinner(), wd("ReadingScreenshots")));
      return out.concat(h("div.lx-subh", wdp("ScreenshotsSummary", { count: 6 })), h("div.lx-grid3", [0, 1, 2, 3, 4, 5].map((n) => h("div.lx-shot", { style: { cursor: "pointer", filter: `hue-rotate(${n * 25}deg)` }, onclick: () => ctx.open("screenshot") }))));
    }
    out.push(h("div.lx-tbar", btn(wd("ImportWorld"), { icon: "ul", on: () => {} }), gbtn(wd("OpenSavesFolder")), h("span.lx-grow"), gbtn(cm("Refresh"))));
    if (S.worlds === "gameRunning") out.push(note(wd("GameRunning"), "warn"));
    if (S.worlds === "task") out.push(h("div.lx-scard", h("div", wd("BackingUp", { name: "Survival Island" })), progress(40), h("div", btn(cm("Cancel"), { sm: true }))));
    if (S.worlds === "done") out.push(note(wd("BackedUp", { name: "Survival Island", size: "40 MB" }), "ok"));
    if (S.worlds === "empty") return out.concat(empty("cube", wd("NoWorldsYet"), wd("NoWorldsHint")));
    if (S.worlds === "reading") return out.concat(empty(null, spinner(), wd("ReadingWorlds")));
    const rows = [];
    WORLDS.forEach(([name, folder, played, mode, cheats, ver, size, backups, hard, snap], idx) => {
      rows.push(h("div.lx-row", h("span.lx-world-ic"),
        h("div.lx-row-main", h("div.lx-row-t", tx(name), hard ? badge(wd("Hardcore"), "red") : null, h("span.lx-row-v", tx(folder))),
          h("div.lx-row-s", join(wd("PlayedAt", { time: played }), mode === "Hardcore" ? wd("Hardcore") : wd(mode), cheats ? wd("Cheats") : null, snap ? wd("Snapshot", { version: ver }) : tx(ver), tx(size)))),
        h("div.lx-row-a", btn(wd("BackUp"), { icon: "dl", sm: true }), gbtn([wd("Backups"), backups ? " " + backups : ""]), gbtn(cm("Rename")), gbtn(wd("Export")), gbtn(cm("OpenFolder")), btn(wd("DeleteWorld"), { kind: "d", sm: true, on: () => ctx.open("deleteWorld") }))));
      if (idx === 0) rows.push(h("div", { style: { padding: "8px 18px 12px 88px", background: "var(--g875)" } },
        h("div.lx-caps", { style: { margin: "4px 0 8px" } }, wd("Backups")),
        ["2026-11-10 03:04", "2026-11-09 21:40"].map((b) => h("div", { style: { display: "flex", alignItems: "center", gap: "8px", padding: "6px 0" } }, h("span.lx-mono.lx-grow", tx("Survival Island " + b + ".zip")), h("span.lx-muted", tx("39 MB")),
          btn(wd("RestoreAsCopy"), { sm: true }), btn(wd("ReplaceWorld"), { sm: true, on: () => ctx.open("replaceWorld") }), gbtn(cm("Delete")))),
        h("div.lx-muted", { style: { fontSize: "13px", marginTop: "6px" } }, wd("NoBackupsYet"))));
    });
    out.push(card(rows));
    return out;
  }

  // ---------- Settings tab ----------
  const ISEC = [["general", "General"], ["game", "GameAndLoader"], ["modpack", "Modpack"], ["linked", "LinkedServer"], ["java", "JavaAndMemory"], ["launch", "LaunchOptions"], ["shared", "SharedGameSettings"], ["snapshots", "InstanceSnapshots"], ["maint", "Maintenance"]];
  const COLORS = ["#59B2F3", "#22c7e0", "#6DD277", "#9be060", "#EFB440", "#f08a40", "#e35d6a", "#ee3fd0", "#a98bf5", "#f2f2f2"];
  const swatches = () => h("div.lx-swatches", h("span.lx-chip", sh("SkinUseSkins")), COLORS.map((c) => h("i", { style: { background: c } })));
  const LOADERS = [["vanilla", "Vanilla", "AboutVanilla", "V", "#4caf50"], ["fabric", "Fabric", "AboutFabric", "F", "#dbd0b4"], ["quilt", "Quilt", "AboutQuilt", "Q", "#8b4fd8"], ["forge", "Forge", "AboutForge", "F", "#2f3a6a"], ["neoforge", "NeoForge", "AboutNeoForge", "N", "#e07a2c"]];
  const loaderCards = (sel, on) => h("div.lx-grid5", LOADERS.map(([k, n, about, l, c]) => h("div.lx-lcard" + (k === sel ? ".on" : ""), { onclick: () => on && on(k) },
    h("span.lx-lcard-l", { style: { background: c, color: k === "fabric" ? "#333" : "#fff" } }, tx(l)), h("b", k === "vanilla" ? is("Vanilla") : tx(n)), h("div.lx-lcard-d", is(about)))));
  function settingsTab(ctx, inst) {
    const active = ctx.params.sec || "general";
    const secs = ISEC.filter(([k]) => (k !== "modpack" || inst.modpack) && (k !== "linked" || inst.linked));
    const content = {
      general: () => card(
        srow(is("Name"), null, field(inst.name)),
        srow(is("Group"), null, inst.grp === "Survival" ? field("Survival") : field(null, is("NoGroup"))),
        srow(is("Icon"), null, [grass("lx-big"), btn(cm("Change"), { on: () => ctx.open("iconPicker") })]),
        srow(is("PlayBackground"), null, [h("span", { style: { width: "80px", height: "45px", borderRadius: "4px", background: "radial-gradient(circle at 70% 50%,#3e4f78,#1a1e28)" } }), btn(cm("Change"), { on: () => ctx.open("backgroundPicker") })]),
        srow(sh("SkinPlayColor"), null, swatches()), srow(sh("SkinAccent"), null, swatches())),
      game: () => [card(
        srow(is("MinecraftVersion"), is("GameVersionHint"), h("span.lx-row-v", tx(inst.ver))),
        h("div", { style: { padding: "14px 18px" } }, h("div.lx-flabel", is("ModLoader")), loaderCards((inst.loader || "vanilla").toLowerCase(), () => ctx.open("switchLoader"))),
        srow(is("LoaderVersion"), null, h("div", { style: { position: "relative" } }, select(h("span", tx(inst.lver || "0.16.14"), " ", badge(is("Recommended"), "green")), { on: toggleMenu("lver") }),
          ui.menu === "lver" ? menu([{ label: tx("0.16.14"), right: h("span.lx-mright", is("Recommended")) }, { label: tx("0.17.0-beta.2"), right: h("span.lx-mright", is("Beta")) }, { label: tx("0.16.13") }], "lx-rmenu") : null)),
        srow(is("PrereleaseUpdates"), is("PrereleaseUpdatesHint"), toggle(false))),
        ctx.state.instSettings === "loaderChanged" ? note(isp("LoaderNextPlayModsOff", { loader: "Fabric 0.16.9", count: 2 }), "info") : note(is("LoaderInstallsNextPlay", { loader: "Fabric 0.16.14" }), "info")],
      modpack: () => card(srow(tx("Fabulously Optimized 6.1.0"), is("FromModrinth"), [gbtn(is("ViewOnModrinth"), null, "ext"), ctx.state.modpack === "checking" ? h("span.lx-muted", spinner(), " ", is("CheckingModrinth")) : ctx.state.modpack === "upToDate" ? h("span.lx-muted", is("UpToDate")) : ctx.state.modpack === "offline" ? h("span.lx-muted", is("ModrinthUnreachable")) : pbtn(is("UpdateTo", { version: "6.2.0" }), () => ctx.open("modpackUpdate"))]),
        srow(is("CheckForUpdates"), null, btn(is("CheckForUpdates"), { sm: true }))),
      linked: () => card(srow(tx("Survival SMP"), ctx.state.linkedCore === "removed" ? is("LinkedServerCoreRemoved", { core: "Peter's Lodestone Core" }) : is("LinkedServerHint", { core: "Peter's Lodestone Core" }),
        [ctx.state.linkedCore === "removed" ? badge(is("CoreRemoved"), "yellow") : null, ctx.state.instSettings === "unlinked" ? badge(is("Unlinked")) : btn(is("Unlink"), { on: () => ctx.set("instSettings", "unlinked") })],
        { below: ctx.state.instSettings === "unlinked" ? h("div.lx-srow-hint", is("UnlinkNote")) : h("div.lx-srow-hint", is("FromServerMods", { server: "Survival SMP" })) })),
      java: () => [h("div.lx-h3-sub", is("JavaMemoryLead")), card(
        srow(is("Memory"), [is("MemoryChosenAutomatically"), " ", is("TotalMemory", { computer: "This Mac", size: "16 GB" })], [h("span.lx-row-v", is("MemoryRange", { min: "2 GB", max: "6 GB" })), btn(is("ChangeSetting"), { sm: true })]),
        srow(tx("Java"), is("JavaFound", { major: 17, version: "17.0.12" }), [h("span.lx-row-v", is("MojangJavaVersion", { major: 17 })), btn(cm("Change"), { sm: true }), btn(is("Check"), { sm: true })])),
        ctx.state.instSettings === "javaOld" ? note(is("JavaTooOld", { major: 8, version: "1.8.0_402", game: "1.20.1", needed: 17 }), "err") : null],
      launch: () => card(
        srow(is("Window"), null, [h("span.lx-row-v", is("WindowDefaultSize")), btn(is("ChangeSetting"), { sm: true })]),
        srow(is("Fullscreen"), null, toggle(false)),
        srow(is("JvmArguments"), null, [h("span.lx-row-v", is("NoArguments")), btn(is("ChangeSetting"), { sm: true })]),
        srow(is("GameArguments"), null, [h("span.lx-row-v", is("NoArguments")), btn(is("ChangeSetting"), { sm: true })]),
        srow(is("EnvironmentVariables"), null, [h("span.lx-row-v", is("NoVariables")), btn(is("ChangeSetting"), { sm: true })])),
      shared: () => [h("div.lx-h3-sub", is("SharedLead")),
        ctx.state.game === "playing" ? note(is("StopGameToChange"), "warn") : null,
        card(srow(is("UseSharedSettings"), is("SharingFrom", { name: "Survival 1.20.1" }), toggle(true)),
          srow(is("MainInstance"), is("MainInstanceHint"), btn(is("MakeMain"), { sm: true, on: () => ctx.open("mainSwitch") })),
          ...[["Controls", 12, 2], ["Video", 9, 0], ["Sound", 14, 0], ["Accessibility", 20, 1], ["Language", 3, 0]].map(([c, s, o]) =>
            srow(is("Category" + c), [is("Category" + c + "Description"), sep(), isp("SharedCount", { count: s }), o ? [sep(), isp("OverriddenCount", { count: o })] : null], gbtn(is("Show"))))),
        h("div.lx-scard", { style: { marginTop: "12px" } }, h("div", isp("ChangedInGame", { count: 2, names: "FOV, Brightness" })), h("div", { style: { display: "flex", gap: "8px" } }, btn(is("KeepForInstance"), { sm: true }), gbtn(is("Dismiss"))))],
      snapshots: () => [h("div.lx-h3-sub", is("SnapshotsHint")), card(
        h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", tx("Before 1.21 update")), h("div.lx-row-s", tx("Nov 8, 2026"), sep(), is("SnapshotSizes", { total: "412 MB", new: "38 MB" }))), h("div.lx-row-a", btn(is("RevertEllipsis"), { sm: true, on: () => ctx.open("snapshotRevert") }), btn(is("ShareEllipsis"), { sm: true, on: () => ctx.open("snapshotShare") }), gbtn(cm("Delete"), () => ctx.open("snapshotDelete")))),
        h("div.lx-row", h("div.lx-row-main", h("div.lx-row-t", is("BeforeGoingBack", { name: "Before 1.21 update" })), h("div.lx-row-s", tx("Nov 9, 2026"), sep(), is("SnapshotSizes", { total: "410 MB", new: "2 MB" }))), h("div.lx-row-a", btn(is("RevertEllipsis"), { sm: true }), btn(is("ShareEllipsis"), { sm: true }), gbtn(cm("Delete"))))),
        h("div", { style: { marginTop: "10px", display: "flex", gap: "8px" } }, btn(is("TakeSnapshotEllipsis"), { on: () => ctx.open("snapshotTake") })),
        ctx.state.instSettings === "noSnapshots" ? h("div.lx-muted", is("NoSnapshots")) : null],
      maint: () => card(
        srow(is("VerifyGameFiles"), is("VerifyHint"), btn(is("Verify"), { on: () => ctx.set("game", "verifying") }), { below: ctx.state.game === "verifying" ? h("div.lx-srow-hint", is("VerifyStarted")) : null }),
        srow(is("Export"), is("ExportHint"), btn(is("ExportButton"))),
        srow(is("Duplicate"), null, btn(is("Duplicate"))),
        srow(is("HostOnCore"), is("HostHint"), btn(is("HostButton"), { on: () => ctx.open("hostOnCore") })),
        srow(is("ShareCodeRow"), null, btn(is("ShareButton"), { on: () => ctx.open("shareCode") })),
        srow(is("DeleteInstance"), is("DeleteHint"), btn(is("DeleteButton"), { kind: "d", on: () => ctx.open("deleteInstance") }))),
    };
    return h("div.lx-snavcol",
      h("div.lx-snavcol-nav", secs.map(([k, s]) => h("button" + (k === active ? ".on" : ""), { onclick: () => ctx.go("instance", { id: inst.id, tab: "settings", sec: k }) }, k === "snapshots" ? is("InstanceSnapshots") : is(s)))),
      h("div", secs.map(([k, s]) => section(k === "snapshots" ? is("InstanceSnapshots") : is(s), content[k]()))));
  }

  // ---------- Lodestone Host page ----------
  const SERVERS = [
    { name: "Survival SMP", ver: "1.20.1", loader: "Fabric 0.16.14", state: "running", players: [3, 20], shared: true, linked: true },
    { name: "Creative world", ver: "1.21.1", loader: null, state: "stopped" },
    { name: "All the Mods", ver: "1.20.1", loader: "Forge 1.20.1-47.4.28", state: "starting", busy: "BusyStarting" },
    { name: "Old hub", ver: "1.8.9", loader: null, state: "crashed" },
    { name: "Lobby", ver: "1.21.1", loader: "Paper", state: "stopping", busy: "BusyStopping" },
    { name: "Test", ver: "1.21.1", loader: "Other", state: "unknown" },
  ];
  const stateLabel = (s) => sv({ running: "StateRunning", starting: "StateStarting", stopping: "StateStopping", stopped: "StateStopped", crashed: "StateCrashed", unknown: "StateUnknown", hidden: "StateHidden" }[s]);
  function serverRow(ctx, s, opts = {}) {
    const busy = s.busy ? h("span.lx-muted", spinner(), " ", sv(s.busy, s.busyVars)) : null;
    const actions = busy || [
      opts.owner && s.state === "running" ? [btn(cm("Stop"), { sm: true }), btn(cm("Restart"), { sm: true })] : null,
      opts.owner && (s.state === "stopped" || s.state === "crashed") ? btn(cm("Start"), { sm: true, kind: "p" }) : null,
      !opts.owner && s.state === "stopped" && opts.signedIn ? btn(sv("StartAndJoin"), { sm: true, kind: "p" }) : null,
      s.linked ? btn(sv("Play"), { sm: true, kind: "p", on: () => ctx.go("instance", { id: "smp", tab: "play" }) }) : btn(sv("CreateInstance"), { sm: true, icon: "plus", on: () => ctx.open("advisory") }),
    ];
    return h("div.lx-srvrow", dot(s.state === "running" ? "on" : s.state === "crashed" ? "err" : s.state === "starting" ? "warn" : "off"),
      h("div.lx-grow", h("div.lx-srvrow-n", h("a", { style: { cursor: "pointer" }, onclick: () => ctx.go("server", { name: s.name }) }, tx(s.name)),
        s.shared ? h("span.lx-pill-tag", sv("SharedTag"), h("button", { onclick: () => ctx.set("toast", "copiedInvite") }, ic("copy"), sv("CopyInviteLink"))) : null),
      h("div.lx-row-s", join(tx(s.ver), s.loader === "Other" ? sv("LoaderOther") : s.loader ? tx(s.loader) : null, s.players ? sv("PlayersOfMax", { count: s.players[0], max: s.players[1] }) : null, s.state !== "running" ? stateLabel(s.state) : null, s.hiddenAddr ? sv("AddressHidden") : null))),
      h("div.lx-row-a", actions));
  }
  function coreMenu(ctx, owner) {
    return menu([
      owner ? { label: sv("CopyOwnerApiKey"), icon: "key", on: () => { ui.menu = null; ctx.set("toast", "copiedKey") } } : { label: cm("SignIn"), icon: "user", on: () => { ui.menu = null; ctx.open("addServer", { signin: true }) } },
      { label: cm("Rename"), icon: "pencil" }, owner ? null : { label: cm("SignOut"), icon: "signout" }, { label: cm("Remove"), icon: "trash", danger: true }, "-",
      h("div", { style: { padding: "6px 10px", fontSize: "12.5px", color: "var(--g500)", display: "flex", gap: "6px", alignItems: "center" } }, ic("shield"), sv("CertificatePinned")),
    ].filter(Boolean), "lx-rmenu");
  }
  function localCore(ctx) {
    const st = ctx.state.localCore;
    if (st === "scanning") return h("div.lx-corebox", h("div.lx-core.lx-muted", spinner(), " ", sv("ScanningLocal")));
    if (st === "none") return h("div.lx-corebox",
      h("div.lx-core.lx-muted", sv("NoLocalCores")),
      h("div.lx-core", logoMark(true), h("div.lx-grow", h("div", { style: { fontWeight: 700, fontSize: "16px" } }, sv("HostOfferTitle")), h("div.lx-muted", sv("HostOfferBody"))),
        h("div", { style: { display: "grid", justifyItems: "center", gap: "6px" } }, pbtn(sv("HostOfferDownload"), null, "dl"), gbtn(sv("HostOfferCoreOnly"))), iconBtn("x")));
    const claimed = st === "claimed";
    const head = h("div.lx-core.lx-core-local", logoMark(true),
      h("div.lx-grow", h("div.lx-core-k", cm("ThisComputerStart")), h("div.lx-core-n", tx("Peter's Lodestone Core")),
        h("div.lx-core-s", tx("https://127.0.0.1:16662"), claimed ? [sep(), sv("SignedInAs", { username: "owner_k3m9x" })] : null),
        h("div.lx-core-s", st === "direct" ? sv("StartedDirectly") : sv("StartedBy", { who: "Lodestone Host" }))),
      claimed ? [h("span.lx-muted", sv("TrustedForMods")), toggle(true)] : pbtn(sv("ConnectAsOwner"), () => ctx.set("localCore", "claimed")),
      h("div", { style: { position: "relative" } }, iconBtn("dots", toggleMenu("core-local"), ""), ui.menu === "core-local" ? coreMenu(ctx, claimed) : null));
    const parts = [head];
    if (st === "direct") parts.push(h("div.lx-core", h("div.lx-grow.lx-muted", sv("DirectNote")), btn(sv("GetLodestoneCli"), { icon: "ext" })));
    if (st === "claimFailed") parts.push(h("div", { style: { padding: "0 18px" } }, note(sv("ClaimRefused"), "err")));
    if (claimed) {
      parts.push(...SERVERS.map((s) => serverRow(ctx, s, { owner: true })));
      parts.push(h("div", { style: { padding: "12px 18px" } }, h("div.lx-caps", sv("SharedWithFriends")), h("div.lx-muted", { style: { fontSize: "13px" } }, sv("SharedWithFriendsHint"))));
    } else if (st !== "direct") {
      parts.push(serverRow(ctx, { name: "Survival SMP", ver: "1.20.1", loader: "Fabric 0.16.14", state: "running", players: [3, 20], shared: true }, {}),
        h("div", { style: { padding: "8px 18px 12px" } }, h("div.lx-muted", { style: { fontSize: "13px" } }, sv("ConnectToJoinDirectly"))));
    }
    return h("div.lx-corebox.lx-local", parts);
  }
  function friendsSection(ctx) {
    const st = ctx.state.friends;
    const head = h("div.lx-hostsec-h", ic("users"), h("div.lx-grow", h("div.lx-hostsec-t", fl("Title")), h("div.lx-hostsec-s", fl("Hint"))),
      st === "refreshing" ? h("span.lx-muted", spinner(), " ", fl("Refreshing")) : gbtn(cm("Refresh"), null, "refresh"), btn(fl("AddFriend"), { icon: "plus", on: () => ctx.open("addFriend") }));
    const parts = [head];
    if (st === "error") { parts.push(h("div", { style: { padding: "4px 18px" } }, note([h("b", fl("OpenFailed")), h("div", tx("database is locked"))], "err", btn(cm("TryAgain"), { sm: true })))); return h("div.lx-hostsec", parts) }
    if (st === "linkWaiting") parts.push(h("div", { style: { padding: "14px 18px" } }, h("div.lx-caps", fl("LinksTitle")),
      h("div.lx-srvrow", { style: { padding: "8px 0" } }, ic("link"), h("div.lx-grow", h("div.lx-mono", tx("https://connect.lodestone.cc/f/9fK2\u2026")), h("div.lx-row-s", fl("LinkWaiting", { time: "Tomorrow at 14:05" }))), btn(cm("Copy"), { sm: true, icon: "copy" }), btn(fl("CheckAnswers"), { sm: true }), gbtn(fl("CancelLink")))));
    if (st === "reply") parts.push(h("div", { style: { padding: "14px 18px" } },
      h("div.lx-scard", h("b", fl("ReplyFrom", { name: "Alex" })), h("div.lx-mono", fl("SafetyCode", { code: "P4XN-7TQE-K2MW-9RBD" })), h("div.lx-muted", fl("PlaysAs", { name: "AlexCrafts" })), note(fl("SameNameAs", { names: "Alex" }), "warn"), h("div.lx-muted", flp("AnsweredTimes", { count: 2 })),
        h("div", { style: { display: "flex", gap: "8px" } }, pbtn(fl("Accept")), btn(fl("Ignore"))))));
    // no Microsoft account: asked for a name
    if (st === "noName") parts.push(h("div.lx-srvrow", ic("user"), h("div.lx-grow", h("div.lx-muted", fl("You")), h("div.lx-muted", fl("NoNameYet"))), btn(fl("ChooseName"), { sm: true, on: () => ctx.open("friendName") })));
    // signed in with Microsoft: the Minecraft name, without asking
    if (st === "empty") parts.push(h("div.lx-srvrow", ic("user"), h("div.lx-grow", h("div", h("span.lx-muted", { style: { fontSize: "12.5px" } }, fl("You")), " ", h("b", tx("Lodestoner")), " ", h("span.lx-muted", fl("MinecraftNameNote")))), gbtn(fl("EditName"), () => ctx.open("friendName"))));
    if (st === "noName" || st === "empty") parts.push(h("div.lx-srvrow", h("div.lx-grow.lx-muted", fl("NoFriendsYet"))));
    else {
      parts.push(h("div.lx-srvrow", ic("user"), h("div.lx-grow", h("div", h("span.lx-muted", { style: { fontSize: "12.5px" } }, fl("You")), " ", h("b", tx("Peter"))), h("div.lx-mono.lx-muted", fl("YourSafetyCode", { code: "K7Q2-M9XD-4TRW-H3BE" }))),
        gbtn(fl("EditName"), () => ctx.open("friendName")), btn(fl("Retire"), { kind: "d", sm: true, on: () => ctx.open("retire") })),
        h("div.lx-srvrow", { style: { paddingLeft: "44px" } }, ic("server"), h("div.lx-grow.lx-muted", fl("SharingFrom", { core: "Peter's PC" })), gbtn(fl("RetireCore"), () => ctx.open("retireCore"))));
      if (st === "list" || st === "play" || st === "share" || st === "compare") parts.push(...friendRows(ctx, st));
      if (st === "list") parts.push(h("div", { style: { padding: "4px 18px 10px" } }, note(fl("RefreshProblem", { error: "Sam: timed out" }), "warn")));
    }
    return h("div.lx-hostsec", parts);
  }
  function friendRows(ctx, st) {
    const fmenu = (n) => h("div", { style: { position: "relative" } }, iconBtn("dots", toggleMenu("friend-" + n)), ui.menu === "friend-" + n ? menu([
      { label: fl("CompareCodes"), icon: "shield", on: () => { ui.menu = null; ctx.open("verify") } }, { label: cm("Rename"), icon: "pencil", on: () => { ui.menu = null; ctx.open("renameFriend") } },
      { label: n === "Jo" ? fl("StopHiding") : fl("HideFromThem"), icon: "eye" }, "-", { label: fl("RemoveFriend"), icon: "trash", danger: true, on: () => { ui.menu = null; ctx.open("removeFriend") } }], "lx-rmenu") : null);
    return [
      h("div.lx-srvrow", dot("on"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Alex"), badge(fl("Verified"), "green")), h("div.lx-row-s", fl("OnlineOn", { cores: "Alex's PC, Alex's laptop" }))), gbtn(fl("ShowServers")), fmenu("Alex")),
      h("div", { style: { padding: "6px 18px 12px 46px", background: "var(--g875)" } }, h("div.lx-caps", { style: { margin: "4px 0" } }, fl("Servers")),
        h("div.lx-srvrow", { style: { padding: "6px 0" } }, h("span.lx-grow", tx("Alex's PC"), " ", badge(fl("CoreOnline"), "green")), null),
        h("div.lx-srvrow", { style: { padding: "6px 0" } }, dot("on"), h("div.lx-grow", h("div", tx("Creative build")), h("div.lx-row-s", tx("1.21.4"), sep(), svp("Players", { count: 3 }))), pbtn(sv("Play"), () => ctx.open("joinFriend"))),
        h("div.lx-srvrow", { style: { padding: "6px 0" } }, h("span.lx-grow", tx("Alex's laptop"), " ", badge(fl("CoreOffline")))),
        h("div.lx-muted", { style: { fontSize: "13px" } }, fl("LoadingServers")), h("div.lx-muted", { style: { fontSize: "13px" } }, fl("NoServersShared")), h("div.lx-muted", { style: { fontSize: "13px", color: "#ff8a8a" } }, fl("ServersFailed", { error: "connection reset" }))),
      h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Sammy"), badge(fl("NotVerified"))), h("div.lx-row-s", fl("TheirName", { name: "Sam" }), sep(), fl("NotSharingNow"))), btn(fl("CompareCodes"), { sm: true, on: () => ctx.open("verify") }), fmenu("Sam")),
      h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Robin")), h("div.lx-row-s", fl("WaitingFor", { name: "Robin" }))), fmenu("Robin")),
      h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Jo"), badge(fl("Hidden"))), h("div.lx-row-s", fl("SharesNothing"))), fmenu("Jo")),
      h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Alex"), badge(fl("NotVerified"))), h("div.lx-row-s", { style: { color: "var(--yellow)" } }, fl("NameLike", { names: "Alex" }))), fmenu("Alex2")),
      h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Kim"), badge(fl("Retired"), "red"))), fmenu("Kim")),
    ];
  }
  function hostPage(ctx) {
    const S = ctx.state;
    const banners = {
      signedOut: () => note([h("b", sv("SignedOutOf", { name: "Alex's server box" })), h("div", sv("KeyStillValid", { id: "a1b2c3" }))], "info", gbtn(sv("Dismiss"))),
      untrusted: () => note([h("b", sv("NoLongerTrusted", { name: "Alex's server box" })), h("div", sv("NoLongerTrustedCertificateChanged"))], "warn", gbtn(sv("Dismiss"))),
      readFailed: () => note([h("b", sv("ReadFailed")), h("div", tx("servers.json: expected value at line 1"))], "err", gbtn(sv("Dismiss"))),
      joinFailed: () => note([h("b", sv("JoinFailed", { name: "Skyblock" })), h("div", sv("ServerNotShown"))], "err", gbtn(sv("Dismiss"))),
      notRunning: () => note([h("b", sv("NotRunning", { name: "Skyblock" })), h("div", sv("AskOwnerToStart"))], "warn", gbtn(sv("Dismiss"))),
      createdWithoutMods: () => note([h("b", sv("CreatedWithoutMods", { name: "Skyblock" })), h("div", sv("CreatedWithoutModsBody"))], "info", gbtn(sv("Dismiss"))),
      connected: () => note([h("b", sv("ConnectedAsOwner", { name: "Peter's Lodestone Core" })), h("div", sv("ConnectedMadeOwner", { username: "owner_k3m9x" }), " ", sv("ConnectedOwnerKey"))], "ok", gbtn(sv("Dismiss"))),
    };
    const other = S.otherCores;
    return h("div.lx-main", h("div.lx-page", { "data-mk-scroll": "lx-host" },
      h("div.lx-pagehead", h("div.lx-grow", h("h1.lx-h1.lx-h1-chillax", sv("ServersTitle")))),
      banners[S.hostMsg] ? banners[S.hostMsg]() : null,
      h("div.lx-areah", h("div.lx-grow", h("div.lx-areah-t", sv("OnThisComputer"))), gbtn(sv("Rescan"))),
      localCore(ctx),
      friendsSection(ctx),
      h("div.lx-hostsec", h("div.lx-hostsec-h", ic("user"), h("div.lx-grow", h("div.lx-hostsec-t", sv("FriendsTitle"), S.relay === "dev" ? h("span", { style: { color: "var(--yellow)", fontFamily: "Satoshi", fontSize: "13px", marginLeft: "10px" } }, sh("ConnectServiceInUse", { service: "connect-dev.lodestone.cc" })) : null), h("div.lx-hostsec-s", sv("FriendsHint"))),
        btn(sv("JoinAFriend"), { icon: "plus", on: () => ctx.open("joinFriend") })),
        S.friendServers === "some" ? [
          h("div.lx-srvrow", dot("on"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Creative build"), badge(sv("FriendPlayingNow"), "green")), h("div.lx-row-s", sv("SharedBy", { name: "Alex's Lodestone" }), sep(), sv("FriendAddress", { address: "127.0.0.1:36017" }))), pbtn(sv("Play"), () => ctx.open("joinFriend"))),
          h("div.lx-srvrow", dot("off"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Sam's survival")), h("div.lx-row-s", sv("SharedBy", { name: "Sam's Core" }))), btn(sv("FriendCheck"), { sm: true }), h("span.lx-muted", sv("FriendChecking")), gbtn(sv("StopTrusting"))),
          h("div.lx-srvrow", dot("on"), h("div.lx-grow", h("div.lx-srvrow-n", tx("Survival SMP")), h("div.lx-row-s", sv("SharedBy", { name: "Peter's Lodestone Core" }), sep(), sv("DirectConnection"))), pbtn(sv("Play")))]
          : null),
      h("div.lx-areah", h("div.lx-grow", h("div.lx-areah-t", sv("OtherCoresTitle")), h("div.lx-areah-s", sv("OtherCoresHint"))), gbtn(sv("AddServer"), () => ctx.open("addServer"), "plus")),
      other === "none" ? empty("server", sv("NoOtherCores"), sv("NoServersHint"), [pbtn(sv("AddServer"), () => ctx.open("addServer"), "plus")]) : [
        h("div.lx-corebox", h("div.lx-core", h("span.lx-ibtn", { style: { width: "38px", height: "38px", border: 0, background: "var(--g700)" } }, ic("server")),
          h("div.lx-grow", h("div.lx-core-n", tx("Alex's server box")), h("div.lx-core-s", tx("https://192.168.1.42:16662"), sep(), other === "signedIn" ? sv("SignedInAs", { username: "peter" }) : sv("NotSignedIn"))),
          h("span.lx-muted", sv("TrustedForMods")), toggle(false, () => ctx.open("trust")), other === "signedIn" ? null : btn(cm("SignIn"), { icon: "user", on: () => ctx.open("addServer", { signin: true }) }),
          h("div", { style: { position: "relative" } }, iconBtn("dots", toggleMenu("core-alex")), ui.menu === "core-alex" ? coreMenu(ctx, false) : null)),
          other === "certificate" ? h("div", { style: { padding: "4px 18px" } }, note(sv("CertUntrustedWarning", { computer: "this Mac" }), "warn", btn(sv("ReviewCertificate"), { sm: true, on: () => ctx.open("certificate") }))) :
            other === "certChanged" ? h("div", { style: { padding: "4px 18px" } }, note(sv("CertChangedWarning"), "err", btn(sv("ReviewNewCertificate"), { sm: true, on: () => ctx.open("certificate", { changed: true }) }))) :
              other === "notListed" ? h("div.lx-srvrow.lx-muted", sv("NotListedPublicly")) : other === "noPublic" ? h("div.lx-srvrow.lx-muted", sv("NoPublicServers")) : other === "noVisible" ? h("div.lx-srvrow.lx-muted", sv("NoVisibleServers")) :
                [serverRow(ctx, { name: "Skyblock", ver: "1.21.1", loader: "Fabric 0.16.14", state: "running", players: [1, 10] }, {}),
                  serverRow(ctx, { name: "Minigames", ver: "1.21.1", loader: "Paper", state: other === "signedIn" ? "stopped" : "hidden", hiddenAddr: true, busy: other === "busy" ? "BusyCreatingFiles" : null, busyVars: { done: 12, total: 48 } }, { signedIn: other === "signedIn" })]),
        h("div.lx-corebox", h("div.lx-core", h("span.lx-ibtn", { style: { width: "38px", height: "38px", border: 0, background: "var(--g700)" } }, ic("server")),
          h("div.lx-grow", h("div.lx-core-n", tx("Old laptop")), h("div.lx-core-s", tx("https://old-laptop.local:16662"), sep(), sv("NotSignedIn"))), h("span.lx-muted", sv("TrustedForMods")), toggle(false), btn(cm("SignIn"), { icon: "user" }), iconBtn("dots")),
          h("div", { style: { padding: "4px 18px" } }, note(sv("Offline"), "warn")))],
      h("div.lx-hostsec", { style: { marginTop: "18px" } }, h("div.lx-hostsec-h", ic("lan"), h("div.lx-grow", h("div.lx-hostsec-t", sv("OnYourNetwork")), h("div.lx-hostsec-s", sv("OnYourNetworkHint")))),
        S.lan === "looking" ? h("div.lx-srvrow.lx-muted", spinner(), " ", sv("LanLooking")) : S.lan === "none" ? h("div.lx-srvrow.lx-muted", sv("LanNoneMac")) : S.lan === "failed" ? h("div.lx-srvrow", note(sv("LanListenFailed", { error: "Address already in use" }), "err")) :
          [h("div.lx-srvrow", dot("on"), h("div.lx-grow", h("div", tx("Steve's world")), h("div.lx-row-s", tx("192.168.1.31:51234"))), btn(sv("Join"), { sm: true, kind: "p", on: () => ctx.open("joinLan") })),
            h("div.lx-srvrow", dot("on"), h("div.lx-grow", h("div", sv("LanWorld")), h("div.lx-row-s", tx("192.168.1.40:60311"))), btn(sv("Join"), { sm: true, kind: "p", on: () => ctx.open("joinLan") }))])));
  }

  // ---------- a server's page ----------
  function serverPage(ctx) {
    const S = ctx.state, st = S.serverConsole;
    const name = ctx.params.name || "Survival SMP";
    const lines = [["12:00:01", "[main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.16.14"], ["12:00:02", "[main/INFO]: Loaded 42 mods", "ok"], ["12:00:04", "[Server thread/INFO]: Starting minecraft server version 1.20.1"],
      ["12:00:09", "[Server thread/WARN]: Can't keep up! Is the server overloaded?", "warn"], ["12:00:10", "[Server thread/INFO]: Done (6.214s)! For help, type \"help\""], ["12:01:12", "[Server thread/INFO]: Steve joined the game"],
      ["12:01:30", "[Server thread/INFO]: <Steve> hi everyone!"], ["12:03:20", "[Server thread/ERROR]: Couldn't save chunk [12, -4]: disk is full", "err"], ["12:04:03", "[Server thread/INFO]: <Lodestoner> hello from the launcher"]];
    let consoleBody;
    if (st === "notAllowed") consoleBody = h("div.lx-console", { style: { whiteSpace: "normal" } }, h("div", { style: { fontFamily: "Satoshi", fontSize: "16px" } }, sv("ConsoleNotShown")), h("div.lx-muted", { style: { fontFamily: "Satoshi" } }, sv("ConsoleNotAllowedAtSignIn", { computer: "this Mac" })), h("div.lx-muted", { style: { fontFamily: "Satoshi" } }, sv("ConsoleKeyCantRead", { computer: "this Mac", option: "Read the console" })), h("div.lx-muted", { style: { fontFamily: "Satoshi" } }, sv("ConsoleKeyCantReadShort", { computer: "this Mac" })));
    else if (st === "empty") consoleBody = h("div.lx-console.lx-c-dim", sv("OutputAppearsHere"));
    else if (st === "eula") consoleBody = h("div.lx-console", { style: { whiteSpace: "normal", fontFamily: "Satoshi" } }, h("div.lx-scard", h("b", sv("EulaNotAccepted")), h("div.lx-muted", sv("EulaNotAcceptedBody", { url: "https://aka.ms/MinecraftEULA" })),
      h("div", { style: { display: "flex", gap: "8px" } }, pbtn(sv("AcceptEula")), btn(sv("ReadEula"), { icon: "ext" }))), note(sv("EulaAccepted"), "ok"), note(sv("EulaAcceptFailed", { error: "403 Forbidden" }), "err"));
    else consoleBody = h("div.lx-console", lines.map(([tm, l, c]) => h("div", { class: c ? "lx-c-" + c : "" }, tx(`[${tm}] ${l}`))), h("div", { style: { position: "absolute", right: "12px", bottom: "10px" } }, btn(sv("JumpToLatest"), { sm: true })));
    const acct = S.account === "offline" ? h("div.lx-muted", sv("OfflineCantLink")) : S.account === "signedOut" ? h("div.lx-muted", sv("SignInMicrosoftToLink")) :
      st === "linked" ? [h("div", sv("LinkedTo", { account: "Lodestoner", user: "peter" })), btn(sv("Unlink"), { sm: true })] :
        st === "tooOld" ? h("div.lx-muted", sv("CoreTooOldToLink")) :
          [pbtn(sv("LinkAccount", { account: "Lodestoner", user: "peter" })), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sv("LinkCheckHint")), h("div.lx-muted", { style: { fontSize: "13px" } }, sv("YourLodestoneUser"))];
    return h("div.lx-main", h("div.lx-page", { style: { display: "flex", flexDirection: "column" } },
      h("div.lx-pagehead", h("div.lx-grow", h("h1.lx-h1.lx-h1-chillax", tx(name)), h("div.lx-sub", tx("Peter's Lodestone Core \u00b7 1.20.1 \u00b7 Fabric 0.16.14"))),
        h("span.lx-muted", { style: { display: "flex", gap: "8px", alignItems: "center" } }, dot(st === "eula" ? "off" : "on"), st === "eula" ? sv("StateStopped") : [sv("StateRunning"), " \u00b7 ", sv("PlayersOfMax", { count: 3, max: 20 })]),
        st === "eula" ? btn(cm("Start"), { kind: "p", disabled: true }) : [btn(cm("Stop")), btn(cm("Restart")), btn(cm("Kill"), { kind: "d" })]),
      h("div", { style: { display: "grid", gridTemplateColumns: "1fr 330px", gap: "24px", flex: 1, minHeight: 0 } },
        h("div", { style: { display: "flex", flexDirection: "column", minHeight: "560px" } },
          h("div.lx-tbar", { style: { marginBottom: "8px" } }, h("b", { style: { fontSize: "16px" } }, sv("Console")),
            st === "connecting" ? h("span.lx-muted", spinner(), " ", sv("Connecting")) : st === "reconnecting" ? h("span", { style: { color: "var(--yellow)" } }, sv("Reconnecting", { reason: "connection reset by peer" })) : h("span.lx-muted", dot("on"), " ", sv("Live"))),
          consoleBody,
          h("div.lx-tbar", { style: { marginTop: "12px", marginBottom: "4px" } }, h("div.lx-grow", field(null, sv("ChatPlaceholder"))), pbtn(sv("Send"))),
          st === "cantWrite" ? h("div.lx-muted", { style: { fontSize: "13.5px", color: "var(--yellow)" } }, sv("ConsoleKeyCantWrite", { computer: "this Mac", option: "Send chat and commands" })) : h("div.lx-muted", { style: { fontSize: "13.5px" } }, sv("ChatHint"))),
        h("div", h("div.lx-scard", { style: { background: "var(--g850)" } }, h("b", { style: { fontSize: "16px" } }, sv("MinecraftAccount")), acct)))));
  }

  // ---------- the Settings dialog ----------
  const SPAGES = [["defaults", "InstanceDefaults", "cube"], ["shared", "SharedGameSettings", "copy"], ["general", "General", "list"], ["skin", "Skin", "palette"], ["storage", "Storage", "drive"], ["accounts", "Accounts", "user"], ["developer", "Developer", "gear"]];
  // Shared options shown per category: [option key, value]. Values: so key, plain text, or binding key.
  const OPTS = {
    Controls: [["MouseSensitivity", "100%"], ["InvertYMouse", "=ValueOff"], ["InvertXMouse", "=ValueOff"], ["RawMouseInput", "=ValueOn"], ["MouseWheelSensitivity", "1.0"], ["DiscreteMouseScroll", "=ValueOff"], ["ToggleCrouch", "=ValueOff"], ["ToggleSprint", "=ValueOff"], ["ToggleAttack", "=ValueOff"], ["ToggleUse", "=ValueOff"], ["SprintWindow", "7"], ["AutoJump", "=ValueOff"],
      ["KeyKeyAttack", "#KeyMouseLeft"], ["KeyKeyUse", "#KeyMouseRight"], ["KeyKeyPickItem", "#KeyMouseMiddle"], ["KeyKeyForward", "#W"], ["KeyKeyLeft", "#A"], ["KeyKeyBack", "#S"], ["KeyKeyRight", "#D"], ["KeyKeyJump", "#KeyKeyboardSpace"], ["KeyKeySneak", "#KeyKeyboardLeftShift"], ["KeyKeySprint", "#KeyKeyboardLeftControl"],
      ["KeyKeyDrop", "#Q"], ["KeyKeyInventory", "#E"], ["KeyKeySwapOffhand", "#F"], ["KeyKeyChat", "#T"], ["KeyKeyCommand", "#KeyKeyboardSlash"], ["KeyKeyPlayerlist", "#KeyKeyboardTab"], ["KeyKeySocialInteractions", "#P"], ["KeyKeyFriends", "#KeyKeyboardUnknown"], ["KeyKeyAdvancements", "#L"], ["KeyKeyQuickActions", "#G"],
      ["KeyKeyScreenshot", "#KeyKeyboardF2"], ["KeyKeyTogglePerspective", "#KeyKeyboardF5"], ["KeyKeySmoothCamera", "#KeyKeyboardUnknown"], ["KeyKeyFullscreen", "#KeyKeyboardF11"], ["KeyKeySpectatorOutlines", "#KeyKeyboardUnknown"], ["KeyKeySpectatorHotbar", "#KeyMouseMiddle"], ["KeyKeyToggleGui", "#KeyKeyboardF1"], ["KeyKeyToggleSpectatorShaderEffects", "#KeyKeyboardF4"],
      ["KeyKeySaveToolbarActivator", "#C"], ["KeyKeyLoadToolbarActivator", "#X"], ["KeyKeyHotbar1", "#1"], ["KeyKeyHotbar2", "#2"], ["KeyKeyHotbar3", "#3"], ["KeyKeyHotbar4", "#4"], ["KeyKeyHotbar5", "#5"], ["KeyKeyHotbar6", "#6"], ["KeyKeyHotbar7", "#7"], ["KeyKeyHotbar8", "#8"], ["KeyKeyHotbar9", "#9"],
      ["KeyKeyStreamStartStop", "#KeyKeyboardUnknown"], ["KeyKeyStreamPauseUnpause", "#KeyKeyboardUnknown"], ["KeyKeyStreamCommercial", "#KeyKeyboardUnknown"], ["KeyKeyStreamToggleMic", "#KeyKeyboardUnknown"], ["KeyKeyDebugOverlay", "#KeyKeyboardF3"], ["KeyKeyDebugModifier", "#KeyKeyboardF3"]],
    Video: [["Fov", "90"], ["RenderDistance", "!12"], ["SimulationDistance", "!8"], ["GuiScale", "=ValueAuto"], ["Gamma", "50%"], ["Fullscreen", "=ValueOff"], ["EnableVsync", "=ValueOn"], ["MaxFps", "=ValueUnlimited"], ["Particles", "=ValueAll"], ["GraphicsPreset", "=ValueFancy"], ["Ao", "=ValueOn"], ["BobView", "=ValueOn"],
      ["EntityShadows", "=ValueOn"], ["MipmapLevels", "4"], ["BiomeBlendRadius", "5\u00d75"], ["EntityDistanceScaling", "100%"], ["CloudRange", "!64"], ["ImprovedTransparency", "=ValueOff"], ["CutoutLeaves", "=ValueOn"], ["Vignette", "=ValueOn"], ["WeatherRadius", "10"], ["ChunkSectionFadeInTime", "0.75 s"], ["TextureFiltering", "=ValueRgss"], ["MaxAnisotropyBit", "=ValueAnisotropic"]],
    Sound: [["SoundCategoryMaster", "100%"], ["SoundCategoryMusic", "40%"], ["SoundCategoryRecord", "100%"], ["SoundCategoryWeather", "100%"], ["SoundCategoryBlock", "100%"], ["SoundCategoryHostile", "100%"], ["SoundCategoryNeutral", "100%"], ["SoundCategoryPlayer", "100%"], ["SoundCategoryAmbient", "100%"], ["SoundCategoryVoice", "100%"], ["SoundCategoryUi", "70%"], ["ShowSubtitles", "=ValueOff"], ["DirectionalAudio", "=ValueOff"]],
    Accessibility: [["Narrator", "=ValueOff"], ["HighContrast", "=ValueOff"], ["HighContrastBlockOutline", "=ValueOff"], ["TextBackgroundOpacity", "50%"], ["BackgroundForChatOnly", "=ValueOn"], ["ChatVisibility", "=ValueShown"], ["ChatColors", "=ValueOn"], ["ChatLinks", "=ValueOn"], ["ChatLinksPrompt", "=ValueOn"], ["ChatOpacity", "100%"], ["ChatScale", "100%"], ["ChatWidth", "320 px"],
      ["ChatHeightFocused", "180 px"], ["ChatHeightUnfocused", "90 px"], ["ChatLineSpacing", "0"], ["ChatDelay", "0 s"], ["AutoSuggestions", "=ValueOn"], ["HideMatchedNames", "=ValueOn"], ["OnlyShowSecureChat", "=ValueOff"], ["SaveChatDrafts", "=ValueOff"], ["NotificationDisplayTime", "1\u00d7"], ["ScreenEffectScale", "100%"], ["FovEffectScale", "100%"],
      ["DarknessEffectScale", "100%"], ["DamageTiltStrength", "100%"], ["GlintSpeed", "50%"], ["GlintStrength", "75%"], ["HideLightningFlashes", "=ValueOff"], ["DarkMojangStudiosBackground", "=ValueOff"], ["MenuBackgroundBlurriness", "5"], ["PanoramaScrollSpeed", "1\u00d7"], ["HideSplashTexts", "=ValueOff"], ["NarratorHotkey", "=ValueOn"], ["OnboardAccessibility", "=ValueOff"], ["RotateWithMinecart", "=ValueOff"], ["AllowCursorChanges", "=ValueOn"]],
    Language: [["Lang", "English (US)"], ["ForceUnicodeFont", "=ValueOff"], ["JapaneseGlyphVariants", "=ValueOff"]],
  };
  const optVal = (v) => v[0] === "=" ? so(v.slice(1)) : v[0] === "!" ? sop("Chunks", { count: +v.slice(1) }) : v[0] === "#" ? (v.length <= 3 ? h("span.lx-keycap", tx(v.slice(1))) : h("span.lx-keycap", so("Binding" + v.slice(1)))) : tx(v);
  function sharedOptionsList(ctx, withTags) {
    return Object.entries(OPTS).map(([cat, rows]) => h("div.lx-sec", h("div.lx-tbar", { style: { marginBottom: "6px" } }, h("b", sh("Category" + cat)), h("span.lx-muted", sh("Category" + cat + "Hint")), h("span.lx-grow"), h("span.lx-muted", shp("Values", { count: rows.length }))),
      card(rows.map(([k, v], i) => h("div.lx-srow", { style: { minHeight: "40px", padding: "7px 18px" } }, h("div.lx-srow-l", h("span", so("Option" + k))),
        withTags && i === 1 ? badge(is("ThisInstance"), "blue") : null, withTags && i === 2 ? badge(is("Shared")) : null, h("div.lx-srow-c", optVal(v)))))));
  }
  function settingsDialog(ctx) {
    const S = ctx.state, page = ctx.dparams.page || "general";
    const set = (p) => ctx.open("settings", { page: p });
    const acct = S.account === "offline" ? ["offline", "Dev_Steve"] : ["", "Lodestoner"];
    const nav = h("div.lx-snav",
      h("div.lx-snav-acct", { onclick: () => set("accounts") }, avatar(acct[0]), h("div", h("div", { style: { fontWeight: 700, fontSize: "16px" } }, S.account === "signedOut" ? sh("NoAccount") : tx(acct[1])), h("div.lx-muted", sh("ManageAccounts")))),
      h("div.lx-snav-g", sh("Instances")), SPAGES.slice(0, 2).map(([k, s, i]) => h("button" + (k === page ? ".on" : ""), { onclick: () => set(k) }, ic(i), sh(s))),
      h("div.lx-snav-g", sh("GroupLauncher")), SPAGES.slice(2).map(([k, s, i]) => h("button" + (k === page ? ".on" : ""), { onclick: () => set(k) }, ic(i), sh(s))),
      h("div.lx-grow"), h("div.lx-ver", sh("AppVersion", { version: "0.1.0" })));
    const pages = { general: settingsGeneral, defaults: settingsDefaults, shared: settingsShared, skin: settingsSkin, storage: settingsStorage, accounts: settingsAccounts, developer: settingsDeveloper };
    return h("div.lx-dlg.lx-sdlg", { onclick: (e) => e.stopPropagation() }, nav,
      h("div.lx-smain", { "data-mk-scroll": "lx-settings-" + page }, h("div.lx-sclose", iconBtn("x", () => ctx.close())),
        h("h2.lx-stitle", sh(SPAGES.find((p) => p[0] === page)[1])), pages[page](ctx),
        S.settingsUnsaved === "yes" && page === "defaults" ? h("div.lx-unsaved", { style: { position: "sticky", bottom: "0", left: "auto", transform: "none", margin: "20px auto 0", width: "fit-content" } }, sh("UnsavedInDefaults"), btn(cm("Discard")), pbtn(cm("Save"))) : null));
  }
  function settingsGeneral(ctx) {
    const S = ctx.state, w = ui.wgs;
    const upd = S.update;
    const status = upd === "dev" ? srow(sh("DevelopmentBuild"), sh("UpdatesUnavailable")) :
      srow(sh("BuildNumberCommit", { build: 1398, commit: "605928a" }),
        upd === "checking" ? [spinner(), " ", sh("UpdateChecking")] : upd === "available" ? sh("UpdateAvailable", { build: 1412 }) : upd === "downloading" ? sh("UpdateDownloading", { build: 1412, percent: "42%" }) :
          upd === "ready" ? (S.game === "playing" ? sh("UpdateReadyPlaying", { build: 1412 }) : sh("UpdateReady", { build: 1412 })) : upd === "failed" ? sh("UpdateFailed", { error: "GitHub answered 503" }) : upd === "notChecked" ? sh("UpdateNotChecked") : sh("UpToDate"),
        upd === "available" ? pbtn(sh("DownloadUpdate"), () => ctx.set("update", "downloading")) : upd === "ready" ? pbtn(sh("RestartToUpdate")) : btn(sh("CheckForUpdates"), { on: () => ctx.set("update", "checking") }));
    return [
      h("div.lx-sec", card(
        srow(sh("Language"), null, h("div", { style: { position: "relative" } }, select(sh("LanguageSystemIs", { language: "English" }), { on: toggleMenu("lang") }),
          ui.menu === "lang" ? menu([{ label: sh("LanguageSystem") }, { label: tx("English") }, { label: tx("Deutsch") }, { label: tx("\u7b80\u4f53\u4e2d\u6587") }], "lx-rmenu") : null)),
        srow(sh("Zoom"), tx("\u2318+  \u2318\u2212  \u23180"), [btn(tx("\u2212"), { sm: true }), h("span", { style: { width: "50px", textAlign: "center" } }, fm("Percent", { value: 100 })), btn(tx("+"), { sm: true })]),
        srow(sh("GroupAutomatically"), sh("GroupAutomaticallyHint"), toggle(S.groups !== "off", () => ctx.set("groups", S.groups === "off" ? "on" : "off"))),
        srow(sh("WhenAGameStarts"), w === "stay" ? sh("AfterLaunchStayHint") : w === "quit" ? sh("AfterLaunchQuitHint") : null,
          seg([["stay", sh("AfterLaunchStay")], ["minimize", sh("AfterLaunchMinimize")], ["quit", sh("AfterLaunchQuit")]], w, (k) => { ui.wgs = k; rerender() })))),
      section(sh("UpdatesSection"), card(
        srow(sh("UpdateAutomatically"), sh("UpdateAutomaticallyHint"), toggle(upd !== "dev")),
        srow(sh("BetaBuilds"), sh("BetaBuildsHint"), toggle(false)),
        S.build === "dev" ? srow(sh("DeveloperBuilds"), sh("DeveloperBuildsHint"), toggle(true)) : null,
        status, upd !== "dev" ? srow(sh("BuildNumber", { build: 1398 }), null) : null)),
      h("div.lx-muted", { style: { fontSize: "13px" } }, sh("ViewMenu"), " \u00b7 ", sh("ZoomIn"), " \u00b7 ", sh("ZoomOut"), " \u00b7 ", sh("ActualSize"), " \u00b7 ", sh("EditMenu"), " \u00b7 ", sh("Undo"), " \u00b7 ", sh("Redo"), " \u00b7 ", sh("QuitApp", { app: "Lodestone Launcher" }), " \u00b7 ", cm("MinimizeWindow"), " \u00b7 ", cm("MaximizeWindow"), " \u00b7 ", cm("RestoreWindow")),
    ];
  }
  function settingsDefaults(ctx) {
    return [h("div.lx-h3-sub", sh("DefaultsLead")), h("div.lx-sec", card(
      srow(sh("AutoConfigure"), sh("AutoConfigureHint"), toggle(true)),
      srow(sh("Memory"), [sh("MemoryDefaultHint", { min: "512 MB", max: "4 GB" }), " ", is("TotalMemory", { computer: "This Mac", size: "16 GB" })], [field("512", null, { cls: "lx-num" }), h("span.lx-muted", sh("MegabytesUnit")), tx("\u2013"), field("4096"), h("span.lx-muted", sh("MegabytesUnit"))],
        { below: ctx.state.settingsUnsaved === "error" ? h("div", { style: { color: "#ff8a8a", fontSize: "13.5px", marginTop: "4px" } }, sh("MemoryError", { error: "" }), is("MemoryMinAboveMax"), " ", is("MemoryFormat"), " ", is("MemoryTooHigh", { computer: "this Mac", os: "macOS" })) : null }),
      srow(sh("Window"), null, [field("854"), tx("\u00d7"), field("480"), h("span.lx-muted", sh("Fullscreen")), toggle(false)], { below: ctx.state.settingsUnsaved === "error" ? h("div", { style: { color: "#ff8a8a", fontSize: "13.5px" } }, is("WindowSizeInvalid")) : h("div.lx-srow-hint", is("WindowFullscreen", { size: "854 \u00d7 480" })) }),
      srow(sh("JvmArguments"), null, field(null, is("NoArguments"))),
      srow(sh("GameArguments"), null, field(null, is("NoArguments"))),
      srow(sh("EnvironmentVariables"), ctx.state.settingsUnsaved === "error" ? h("span", { style: { color: "#ff8a8a" } }, is("EnvInvalid", { error: "missing '='" })) : null, field(null, is("EnvPlaceholder")))))];
  }
  function settingsShared(ctx) {
    return [h("div.lx-h3-sub", sh("SharedLead")),
      section(sh("Source"), card(
        srow(sh("SyncFrom"), ctx.state.sharedMain === "none" ? sh("SyncFromNoneHint") : sh("SyncFromHint"), h("div", { style: { position: "relative" } }, select(ctx.state.sharedMain === "none" ? h("span", sh("NoInstance"), " ", h("span.lx-muted", sh("EditHereOnly"))) : tx("Survival 1.20.1"), { on: toggleMenu("main") }),
          ui.menu === "main" ? menu([{ label: tx("Survival 1.20.1") }, { label: tx("All the mods") }, "-", { label: sh("NoInstance"), right: h("span.lx-mright", sh("EditHereOnly")) }], "lx-rmenu") : null)),
        srow(sh("StartFromInstance"), null, btn(sh("CopyFromInstance"))),
        srow(sh("ClearShared"), sh("ClearSharedHint"), btn(sh("ClearEllipsis"))))),
      ctx.state.sharedMain === "none" ? note(sh("SyncedFromNone"), "info") : null,
      section(sh("WhatsShared"), h("div.lx-h3-sub", sh("WhatsSharedLead")), card(["Controls", "Video", "Sound", "Accessibility", "Language"].map((c, i) => srow(sh("Category" + c), [sh("Category" + c + "Hint"), sep(), shp("Values", { count: [61, 24, 13, 37, 3][i] })], toggle(i !== 2))))),
      section(sh("NarratorSection"), card(srow(sh("KeepNarratorOff"), sh("KeepNarratorOffHint"), toggle(true))), note(sh("NarratorKeptOff"), "ok"), h("div.lx-muted", sh("NarratorNotShared", { category: "Accessibility & chat" })), h("div.lx-muted", sh("NarratorPerInstance"))),
      section(sh("ServerList"), card(srow(sh("ServerList"), sh("ServerListHint"), toggle(false)))),
      h("div.lx-tbar", h("b", { style: { fontFamily: "Chillax", fontSize: "19px" } }, sh("WhatsShared")), h("span.lx-grow"), gbtn(sh("Hide")), gbtn(sh("Show")), btn(sh("ClearEveryShared"), { sm: true })),
      h("div.lx-muted", { style: { marginBottom: "10px" } }, sh("NoValuesYet"), " \u00b7 ", h("span.lx-input", { style: { display: "inline-flex", minHeight: "26px", padding: "2px 8px" } }, h("span.lx-ph", sh("Value"))), " \u00b7 ", sh("SharedCleared")),
      sharedOptionsList(ctx, false)];
  }
  function settingsSkin(ctx) {
    const swatch = (c) => h("i", { style: { display: "inline-block", width: "22px", height: "22px", borderRadius: "5px", background: c, border: "1px solid var(--g600)" } });
    const skins = [["Default", "linear-gradient(135deg,#26282C,#212327)"], ["Glass", "linear-gradient(135deg,#3b5876,#1c2633)"], ["Neon", "linear-gradient(135deg,#ff2bd6,#2b1d6b)"], ["Grass block", "linear-gradient(180deg,#6fb64a 30%,#8a5b3a 30%)"]];
    return [
      section(sh("SkinGallery"), h("div.lx-grid3", { style: { gridTemplateColumns: "repeat(4,1fr)", maxWidth: "800px" } }, skins.map(([n, bg], i) => h("div.lx-skin" + (i === 0 ? ".on" : ""), h("div.lx-skin-p", { style: { background: bg } }), h("div.lx-skin-n", tx(n))))),
        h("div.lx-tbar", { style: { marginTop: "12px" } }, btn(sh("SkinNew")), btn(sh("SkinImport"), { on: toggleMenu("skinimp") }), btn(sh("SkinExport")), btn(sh("SkinDelete"), { kind: "d" }), h("span.lx-grow"), pbtn(sh("SkinPopOut"), () => ctx.open("skinEditor"))),
        ui.menu === "skinimp" ? menu([{ label: sh("SkinImportZip") }, { label: sh("SkinImportFolder") }]) : null,
        h("div.lx-muted", { style: { fontSize: "13px" } }, sh("SkinSummary", { surfaces: 12, fonts: 2, banners: 4 }))),
      section(sh("SkinSidebar"), card(srow(sh("SkinImage"), null, [btn(sh("SkinChoose")), gbtn(sh("SkinRemove"))]), srow(sh("SkinFit"), null, seg([["cover", sh("SkinFitCover")], ["tile", sh("SkinFitTile")]], "cover")),
        srow(sh("SkinTint"), null, swatch("#1d3b5a")), srow(sh("SkinTintStrength"), null, tx("40%")), srow(sh("SkinBlur"), null, tx("8 px")))),
      section(sh("SkinMaterial"), card(srow(sh("SkinMaterial"), sh("SkinMaterialHint"), seg([["t", sh("SkinMaterialTranslucent")], ["b", sh("SkinMaterialBlur")], ["f", sh("SkinMaterialFrost")]], "b")),
        srow(sh("SkinMaterialOpacity"), null, tx("70%")), srow(sh("SkinMaterialBlurAmount"), null, tx("24 px")), srow(sh("SkinMaterialFrostVeil"), null, tx("20%")), srow(sh("SkinMaterialGrain"), null, tx("5%")))),
      section(sh("SkinBanner"), card(srow(sh("SkinBannerInSidebar"), null, toggle(true)), srow(sh("SkinHero"), sh("SkinHeroHint"), [btn(sh("SkinChoose")), gbtn(sh("SkinRemove"))]))),
      section(sh("SkinPlayButton"), card(srow(sh("SkinColor"), null, swatch("#3DAE5E")), srow(sh("SkinFadeTo"), null, [swatch("#2f7a48"), gbtn(sh("SkinNone"))]),
        srow(sh("SkinShape"), null, seg([["r", sh("SkinShapeRounded")], ["p", sh("SkinShapePill")], ["x", sh("SkinShapePixel")]], "r")), srow(sh("SkinLabel"), null, swatch("#ffffff")), srow(sh("SkinGlow"), null, toggle(false)))),
      section(sh("SkinColors"), card(srow(sh("SkinAccent"), sh("SkinAdjusted"), [swatch("#59B2F3"), gbtn(sh("SkinDefault"))]), srow(sh("SkinProgress"), null, swatch("#59B2F3"))),
        h("div", { style: { marginTop: "10px" } }, btn(sh("SkinReset")))),
      h("div.lx-muted", sh("SkinSafeMode"), " \u00b7 ", sh("SkinCouldntLoad", { error: "skin.toml: unknown field `colour`" }), " \u00b7 ", sh("SkinImporting"), " \u00b7 ", sh("SkinExporting"), " \u00b7 ", sh("SkinImported", { name: "Neon" }), " \u00b7 ", sh("SkinExported", { path: "~/Desktop/Neon.zip" }), " \u00b7 ", sh("SkinCopyName", { name: "Glass" }), " \u00b7 ", sh("SkinDeleteConfirm", { name: "Neon" }), " \u00b7 ", sh("SkinImportButton"), " \u00b7 ", sh("SkinUseImage")),
    ];
  }
  function settingsStorage(ctx) {
    const st = ctx.state.storage;
    const bar = h("div", { style: { display: "flex", height: "12px", borderRadius: "3px", overflow: "hidden", margin: "10px 0" } }, [["#59B2F3", 44], ["#6DD277", 38], ["#8b4fd8", 12], ["#EFB440", 6]].map(([c, w]) => h("i", { style: { background: c, width: w + "%" } })));
    const leg = (c, l, v) => h("span", { style: { display: "inline-flex", alignItems: "center", gap: "6px", marginRight: "16px" } }, h("i", { style: { width: "9px", height: "9px", borderRadius: "50%", background: c } }), l, " ", tx(v));
    const counting = st === "counting";
    const size = (v) => counting ? h("span.lx-muted", sh("StorageCounting")) : tx(v);
    return [
      st === "gameRunning" ? note(sh("StorageStopGame"), "warn") : null,
      st === "error" ? note(sh("StorageCouldntCount", { error: "Permission denied" }), "err") : null,
      st === "freed" ? note([sh("StorageFreed", { size: "1.2 GB" }), " ", sh("StorageCouldntRemove", { error: "a file is in use" })], "ok") : null,
      section(sh("StorageDataFolder"), card(srow(tx("~/Library/Application Support/Lodestone Launcher"), sh("StorageDataFolderHint"), [gbtn(sh("StorageOpenFolder")), btn(sh("StorageMoveEllipsis"), { on: () => ctx.open("storageMove") }), btn(sh("StorageMoveBackEllipsis"), { sm: true })]))),
      h("div.lx-sec", card(h("div", { style: { padding: "16px 18px" } }, h("div", { style: { fontSize: "24px" } }, counting ? sh("StorageCounting") : fm("Gigabytes", { size: "3.9" })), bar,
        leg("#59B2F3", sh("StorageInstances"), "1.7 GB"), leg("#6DD277", sh("StorageGameFiles"), "1.5 GB"), leg("#8b4fd8", sh("StorageRuntimes"), "452 MB"), leg("#EFB440", sh("StorageCache"), "234 MB"), leg("#767A82", sh("StorageOther"), "400 KB")))),
      section(sh("StorageInstances"), ctx.state.instances === "none" ? card(srow(sh("StorageNoInstances"), null)) : card(
        [["All the mods", "1.0 GB", "412 MB", "520 MB", "111 MB"], ["Survival 1.20.1", "368 MB", "238 MB", "84 MB", "46 MB"], ["Old school", "69 MB", "46 MB", null, "23 MB"]].map(([n, tot, w, c, o]) =>
          srow(tx(n), join(sh("StorageWorlds", { size: w }), c ? sh("StorageContent", { size: c }) : null, sh("StorageOtherFiles", { size: o })), [size(tot), gbtn(sh("StorageOpen")), btn(sh("StorageDelete"), { sm: true, on: () => ctx.open("deleteInstance") })])))),
      section(sh("StorageGameFiles"), card(srow(sh("StorageVersions"), null, size("820 MB")), srow(sh("StorageLibraries"), null, size("410 MB")), srow(sh("StorageAssets"), null, size("290 MB")),
        srow(sh("StorageUnused"), null, [size("312 MB"), btn(sh("StorageRemoveEllipsis"), { sm: true, on: () => ctx.open("storageConfirm", { which: "GameFiles" }) })]))),
      section(sh("StorageRuntimes"), card(srow(sh("StorageJava", { major: 21 }), null, size("190 MB")), srow(sh("StorageJava", { major: 8 }), badge(sh("StorageUnusedTag")), [size("98 MB"), btn(sh("StorageRemoveEllipsis"), { sm: true, on: () => ctx.open("storageConfirm", { which: "Runtimes" }) })]))),
      section(sh("StorageCacheAndLogs"), card(srow(sh("StorageCache"), null, [size("234 MB"), btn(sh("StorageClearEllipsis"), { sm: true, on: () => ctx.open("storageConfirm", { which: "Cache" }) })]),
        srow(sh("StorageLogs"), sh("StorageOldLogs", { size: "38 MB" }), [size("52 MB"), btn(sh("StorageDeleteOld"), { sm: true, on: () => ctx.open("storageConfirm", { which: "OldLogs" }) }), btn(sh("StorageDeleteAll"), { sm: true, on: () => ctx.open("storageConfirm", { which: "AllLogs" }) })]),
        srow(sh("StorageLeftovers"), null, [size("120 MB"), btn(sh("StorageRemoveEllipsis"), { sm: true, on: () => ctx.open("storageConfirm", { which: "Leftovers" }) })]),
        srow(sh("StorageSnapshots"), sh("StorageSnapshotsHint", { count: 4, size: "80 MB" }), [size("640 MB"), btn(sh("StorageRemoveEllipsis"), { sm: true, on: () => ctx.open("storageConfirm", { which: "Snapshots" }) })]))),
      h("div.lx-muted", { style: { fontSize: "13px" } }, sh("StorageNoRuntimes"), " \u00b7 ", sh("StorageRemove"), " \u00b7 ", sh("StorageClear"), " \u00b7 ", sh("StorageDeleteConfirm"), " \u00b7 ", sh("StorageMoveHere"), " \u00b7 ", sh("StorageMoveCantUse", { error: "the folder isn't empty" }), " \u00b7 ", sh("StorageMoveStopGame")),
    ];
  }
  function settingsAccounts(ctx) {
    const a = ctx.state.account;
    const row = (name, kind, inUse) => h("div.lx-row", avatar(kind === "offline" ? "offline" : ""), h("div.lx-row-main", h("div.lx-row-t", tx(name), inUse ? badge(sh("InUse"), "green") : null, kind === "launcher" ? badge(sh("LauncherIdentity"), "yellow") : null),
      h("div.lx-row-s", kind === "offline" ? sh("OfflineNote") : sh("MicrosoftAccount"))),
      h("div.lx-row-a", kind === "expired" ? pbtn(sh("SignInAgain"), () => ctx.open("signin")) : null, inUse ? null : btn(sh("Use"), { sm: true }), gbtn(cm("Remove"), () => ctx.open("removeAccount"))));
    return [
      a === "signedOut" ? h("div.lx-sec", empty("user", sh("NoAccount"), sh("AddAccountFirst"))) : h("div.lx-sec", card(
        row("Lodestoner", a === "launcher" ? "launcher" : a === "expired" ? "expired" : "microsoft", a !== "offline"), row("Dev_Steve", "offline", a === "offline"))),
      section(sh("AddAnAccount"), h("div", { style: { display: "flex", gap: "8px" } }, btn(sh("AddMicrosoftEllipsis"), { icon: "plus", on: () => ctx.open("signin") }), btn(sh("AddOfflineEllipsis"), { icon: "plus", on: () => ctx.open("offlineAccount") }))),
      h("div.lx-muted", { style: { fontSize: "13px" } }, sh("AccountsTooNew"), " ", sh("AccountsCorruptMoved", { path: "~/\u2026/accounts.json.broken" })),
    ];
  }
  function settingsDeveloper(ctx) {
    const relay = ctx.state.relay;
    return [
      section(sh("CommandLineSection"), h("div.lx-h3-sub", sh("CommandLineLead")), card(
        srow(sh("AllowCli"), sh("AllowCliHint"), toggle(true)),
        srow(sh("CliAccess"), sh("CliAccessHint"), seg([["full", sh("CliFullControl")], ["ro", sh("CliReadOnly")]], "full")),
        srow(sh("DebugTraces"), sh("DebugTracesHint"), toggle(true)),
        srow(sh("CliGameLogs"), sh("CliGameLogsHint"), toggle(true)),
        srow(sh("SocketPath"), ctx.state.cli === "closed" ? sh("SocketClosed") : ctx.state.cli === "error" ? h("span", { style: { color: "#ff8a8a" } }, sh("SocketError", { error: "Address already in use" })) : tx("/Users/you/Library/Application Support/Lodestone Launcher/control.sock"), [btn(sh("CopyPath"), { sm: true }), btn(sh("OpenDataFolder"), { sm: true })]),
        h("div", { style: { padding: "12px 18px" } }, h("div.lx-srow-label", sh("ConnectedClients")),
          ctx.state.cli === "clients" ? [h("div.lx-row", { style: { padding: "8px 0" } }, h("div.lx-row-main", h("div", tx("lodestone-launcher-cli")), h("div.lx-row-s", sh("ClientSince", { time: "Today, 14:30" }), sep(), sh("ClientPid", { pid: 48213 }), sep(), shp("Subscriptions", { count: 2 }))), btn(sh("Disconnect"), { sm: true })),
            h("div.lx-row", { style: { padding: "8px 0" } }, h("div.lx-row-main", h("div", sh("UnnamedClient")), h("div.lx-row-s", sh("ClientSince", { time: "Today, 14:41" }), sep(), sh("ClientPid", { pid: 48990 }), sep(), shp("Subscriptions", { count: 1 }))), btn(sh("Disconnect"), { sm: true }))] :
            h("div.lx-srow-hint", sh("NoClients"))))),
      section(sh("CodingAgentsSection"), h("div.lx-h3-sub", sh("CodingAgentsLead")), card(srow(sh("AgentSkills"), tx("launcher-instances, launcher-content, launcher-trace")), srow(sh("AgentPrompt"), sh("AgentPromptHint"), btn(sh("CopyPrompt"), { icon: "copy" })))),
      section(sh("ConnectSection"), card(srow(sh("ConnectService"), sh("ConnectServiceHint"), [field(relay === "default" ? null : relay === "error" ? "http://relay.example.com/x" : "https://connect-dev.lodestone.cc", tx("https://connect.lodestone.cc"), { err: relay === "error" }), btn(cm("Save"), { sm: true, kind: "p" }), gbtn(cm("Reset"))],
        { below: relay === "dev" ? h("div", { style: { color: "var(--yellow)", fontSize: "13.5px", marginTop: "8px" } }, sh("ConnectServiceInUse", { service: "connect-dev.lodestone.cc" })) :
          relay === "error" ? h("div", { style: { color: "#ff8a8a", fontSize: "13.5px", marginTop: "8px", display: "grid" } }, sh("ConnectServiceInsecure"), sh("ConnectServiceScheme"), sh("ConnectServiceExtra"), sh("ConnectServiceAddress")) :
            relay === "env" ? h("div", { style: { color: "var(--yellow)", fontSize: "13.5px", marginTop: "8px" } }, sh("ConnectServiceEnv", { var: "LODESTONE_CONNECT_URL", service: "http://127.0.0.1:8787" })) : null }))),
      section(sh("SignInSection"), card(srow(sh("SignInAsLauncher"), sh("SignInAsLauncherHint"), toggle(ctx.state.account === "launcher", () => ctx.state.account === "launcher" ? ctx.open("turnOffLauncher") : ctx.open("launcherIdentity"))))),
      section(sh("OfflineAccountsSetting"), card(srow(sh("OfflineAccountsSetting"), [sh("OfflineAccountsSettingHint", { add: "Add offline account" }), " ", sh("OfflineAccountsAlwaysOn")], toggle(true)))),
      section(sh("FirstTimeSetupSection"), card(srow(sh("ResetFirstTimeSetup"), sh("ResetFirstTimeSetupHint"), btn(cm("Reset"), { on: () => ctx.set("toast", "firstTime") })))),
      section(sh("DiagnosticsSection"), card(srow(sh("PerfMonitor"), sh("PerfMonitorHint"), btn(sh("PerfOpen"), { on: () => ctx.open("perf") })))),
    ];
  }

  // ---------- dialogs ----------
  const codeBox = (code, copy) => h("div.lx-scard", { style: { flexDirection: "row", display: "flex", alignItems: "center", justifyContent: "center", position: "relative", padding: "18px" } },
    h("span", { style: { fontFamily: "JetBrains Mono", fontSize: "36px", fontWeight: 700, letterSpacing: ".04em" } }, tx(code)), copy ? h("span", { style: { position: "absolute", right: "14px" } }, iconBtn("copy", null, "")) : null);
  const waitBox = (label) => h("div.lx-scard", { style: { display: "flex", flexDirection: "row", alignItems: "center", gap: "10px", background: "var(--g800)" } }, spinner(), label);
  const check = (label, on) => h("div.lx-check" + (on ? ".on" : ""), h("i"), h("span", label));
  const AUTH_ERRORS = ["AuthCancelled", "AuthTimedOut", "AuthStateMismatch", "AuthDeclined", "AuthMicrosoft", "AuthCodeExpired", "AuthNoXbox", "AuthRegion", "AuthAdult", "AuthChild", "AuthXbox", "AuthNotApproved", "AuthNotOwned", "AuthNoProfile", "AuthRateLimited", "AuthNetwork", "AuthUnexpected", "AuthListener", "AuthKeychain", "AuthInvalidLink"];
  // uses: sh("AuthCancelled") sh("AuthCancelledBody") sh("AuthTimedOut") sh("AuthTimedOutBody") sh("AuthStateMismatch") sh("AuthStateMismatchBody") sh("AuthDeclined") sh("AuthDeclinedBody") sh("AuthMicrosoft") sh("AuthMicrosoftBody") sh("AuthCodeExpired") sh("AuthCodeExpiredBody") sh("AuthNoXbox") sh("AuthNoXboxBody") sh("AuthRegion") sh("AuthRegionBody") sh("AuthAdult") sh("AuthAdultBody") sh("AuthChild") sh("AuthChildBody") sh("AuthXbox") sh("AuthXboxBody") sh("AuthNotApproved") sh("AuthNotApprovedBody") sh("AuthNotOwned") sh("AuthNotOwnedBody") sh("AuthNoProfile") sh("AuthNoProfileBody") sh("AuthRateLimited") sh("AuthRateLimitedBody") sh("AuthNetwork") sh("AuthNetworkBody") sh("AuthUnexpected") sh("AuthUnexpectedBody") sh("AuthListener") sh("AuthListenerBody") sh("AuthKeychain") sh("AuthKeychainBody") sh("AuthInvalidLink") sh("AuthNotAUrlBody")
  const AUTH_VARS = { code: "AADSTS50126", status: 500, step: "Xbox Live sign-in", keychain: "macOS Keychain" };
  function signInDialog(ctx) {
    const st = ctx.state.signin;
    const back = btn(cm("Back"), { on: () => ctx.set("signin", "start") });
    let body, acts;
    const idNote = ctx.state.account === "launcher" ? note(sh("LauncherIdentityNote"), "warn") : null;
    if (st === "start") { body = [idNote, h("p", sh("SignInBrowserBody")), h("p.lx-muted", sh("KeepsSignIn", { keychain: "macOS Keychain" }))]; acts = [pbtn(cm("SignIn"), () => ctx.set("signin", "browser")), gbtn(sh("UseCodeInstead"), () => ctx.set("signin", "codeStart"))] }
    else if (st === "codeStart") { body = [idNote, h("p", sh("SignInCodeBody")), h("p.lx-muted", sh("KeepsSignIn", { keychain: "macOS Keychain" }))]; acts = [pbtn(sh("GetCode"), () => ctx.set("signin", "code")), back] }
    else if (st === "browser") { body = [waitBox(sh("WaitingForBrowser")), h("p.lx-muted", sh("FinishInBrowser"))]; acts = [btn(sh("OpenBrowserAgain")), gbtn(sh("UseCodeInstead"), () => ctx.set("signin", "codeStart")), back] }
    else if (st === "gettingCode") { body = [waitBox(sh("GettingCode"))]; acts = [back] }
    else if (st === "code" || st === "copied") { body = [h("p", sh("GoToHost", { host: "microsoft.com/link" })), codeBox("F7K2-9QXM", true), waitBox(st === "copied" ? sh("CodeCopiedWaiting") : sh("WaitingForCode"))]; acts = [pbtn(sh("CopyCodeAndOpen", { host: "microsoft.com/link" }), () => ctx.set("signin", "copied"), "copy"), btn(sh("CopySignInCode")), btn(cm("Back"), { on: () => ctx.open("stopSignIn") })] }
    else if (st === "signing") { body = [waitBox(sh("SigningInToMinecraft"))]; acts = [] }
    else {
      const k = st.replace(/^err:/, "");
      const bodyKey = k === "AuthInvalidLink" ? "AuthNotAUrlBody" : k + "Body";
      body = [note([h("b", sh(k)), h("div", sh(bodyKey, AUTH_VARS)), k === "AuthMicrosoft" ? h("div.lx-muted", sh("AuthMicrosoftSaid", { description: "The user account is disabled.", code: "AADSTS50057" })) : null,
        k === "AuthInvalidLink" ? [h("div", sh("AuthWrongPageBody", { url: "https://login.live.com/oauth20_desktop.srf" })), h("div", sh("AuthNoCodeBody"))] : null], "err"),
        k === "AuthNotApproved" ? h("p.lx-muted", sh("ExpectedUntilApproved")) : null];
      acts = [pbtn(cm("TryAgain"), () => ctx.set("signin", "start")), gbtn(sh("UseCodeInstead"), () => ctx.set("signin", "codeStart"))];
    }
    return dialog(ctx, sh("SignInWithMicrosoft"), body, acts);
  }
  function crashDialog(ctx) {
    const st = ctx.state.crash;
    const modRow = (n, ev, off) => h("div", { style: { display: "flex", alignItems: "center", gap: "12px" } }, h("div.lx-grow", h("b", tx(n)), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sh(ev))), btn(off ? sh("TurnOnMod") : sh("TurnOffMod")));
    const head = [h("p", sh("MinecraftCrashed", { description: "Rendering screen" })), h("div.lx-code.lx-code-err", tx('java.lang.NullPointerException: Cannot invoke\n"net.minecraft.class_1799.method_7909()" because "stack" is null'))];
    let mid;
    if (st === "cause") mid = [caps(sh("LikelyCause")), h("p", sh("CauseOutOfMemory")), h("div.lx-muted", { style: { fontSize: "13px", display: "grid", gap: "6px" } }, sh("CauseWrongJava"), sh("CauseNatives"), sh("CauseJvmOptions"), sh("CauseGraphics"), sh("CauseModPatch"))];
    else if (st === "one") mid = [caps(sh("ProbablyCausedBy")), modRow("Zenith", "EvidenceCrashedInItsCode"), modRow("Mixin Trace", "EvidenceFailedToPatch", true), h("p.lx-muted", sh("ModTurnedOff"))];
    else if (st === "stopped") mid = [h("p", sh("MinecraftStopped"))];
    else mid = [caps(sh("ModsInvolved")), h("p.lx-muted", sh("ModsInvolvedHint")), modRow("Zenith", "EvidencePatchedCrashingCode"), modRow("FancyMenu", "EvidencePatchedCrashingCode"), modRow("Sodium", "EvidenceInStack")];
    return dialog(ctx, sh("CrashTitle", { name: "All the mods" }), [st === "stopped" ? null : head, mid], [pbtn(im("FindModButton"), () => ctx.open("modFinder")), btn(sh("OpenCrashReport")), btn(sh("CopyLog"))], { narrow: true });
  }
  function modFinderDialog(ctx) {
    const st = ctx.state.finder;
    const prog = h("div.lx-muted", { style: { fontSize: "13.5px" } }, imp("FinderModsLeft", { count: 12 }), sep(), imp("FinderModsOff", { count: 6 }), sep(), imp("FinderTriesLeft", { count: 3 }));
    const B = {
      intro: () => [[h("p", im("FinderIntro")), h("p.lx-muted", im("FinderAutomatic")), h("p.lx-muted", im("FinderManual"))], [pbtn(im("FinderStart"), () => ctx.set("finder", "try")), btn(cm("Cancel"), { on: () => ctx.close() })]],
      reading: () => [[waitBox(im("FinderReading"))], [btn(im("FinderStop"))]],
      try: () => [[h("b", im("FinderTry", { number: 2 })), waitBox(im("FinderWaiting")), h("div.lx-muted", im("FinderStarting"), " \u00b7 ", im("FinderSettling"), " \u00b7 ", im("FinderStopping")), prog], [btn(im("FinderStop"), { on: () => ctx.set("finder", "stopped") })]],
      play: () => [[h("b", im("FinderTry", { number: 3 })), h("p", im("FinderPlay")), prog], [pbtn(im("FinderDidntCrash")), btn(im("FinderStop"))]],
      ask: () => [[h("b", im("FinderTry", { number: 3 })), h("p", im("FinderAsk")), prog], [btn(im("FinderItCrashed")), btn(im("FinderDidntCrash")), gbtn(im("FinderStop"))]],
      found: () => [[note(h("b", im("FinderFound")), "ok"), h("p", im("FinderFoundOne", { name: "Zenith" })), h("p.lx-muted", im("FinderAlsoOff", { names: "Zenith Attributes" })), h("p.lx-muted", im("FinderFoundGroup", { names: "Create, Flywheel" }))], [pbtn(im("FinderKeepOff")), btn(im("FinderPutBack"))]],
      notFound: () => [[h("p", im("FinderNotFound"))], [btn(cm("Close"), { on: () => ctx.close() })]],
      refused: () => [[h("p", im("FinderRefused"))], [btn(cm("Close"), { on: () => ctx.close() })]],
      failed: () => [[note([im("FinderFailed"), " ", im("FinderPutBackFailed", { error: "file is in use" })], "err")], [btn(cm("Close"), { on: () => ctx.close() })]],
      stopped: () => [[h("p", im("FinderStopped"))], [btn(cm("Close"), { on: () => ctx.close() })]],
    };
    const [body, acts] = (B[st] || B.intro)();
    return dialog(ctx, im("FinderTitle", { name: "All the mods" }), body, acts, { narrow: true });
  }
  function newInstanceDialog(ctx) {
    const mode = ctx.dparams.mode || "version", st = ctx.state.newinst, loader = ctx.dparams.loader || "vanilla";
    const setM = (m) => ctx.open("newInstance", Object.assign({}, ctx.dparams, { mode: m }));
    const head = seg([["version", is("MinecraftVersion")], ["modpack", is("Modpack")], ["code", is("ShareCodeMode")]], mode, setM);
    let body, acts;
    if (mode === "version") {
      const list = ctx.dparams.list;
      body = [h("div.lx-form", h("span.lx-muted", is("Name")), field(null, is("MyInstance")),
        h("span.lx-muted", tx("Minecraft")), h("div", { style: { position: "relative" } },
          st === "loading" ? h("span.lx-muted", spinner(), " ", is("LoadingVersionList")) :
            st === "failed" ? h("div", { style: { display: "flex", gap: "8px", alignItems: "center" } }, h("span", { style: { color: "#ff8a8a" } }, is("CouldntLoadVersions", { error: "network unreachable" })), btn(cm("TryAgain"), { sm: true })) :
              select(h("span", h("b", tx("26.3")), " ", h("span.lx-muted", is("KindRelease"), " \u00b7 Sep 15, 2026")), { on: () => ctx.open("newInstance", Object.assign({}, ctx.dparams, { list: !list })) }),
          list ? h("div.lx-menu", { style: { top: "44px", left: 0, width: "460px", padding: "10px" } }, field(null, is("SearchVersions"), { icon: "search" }),
            h("div", { style: { display: "flex", gap: "12px", margin: "10px 2px" } }, check(is("Releases"), true), check(is("Snapshots")), check(is("OldVersions"))),
            [["26.3", "KindRelease"], ["26.3-pre2", "KindSnapshot"], ["b1.7.3", "KindBeta"], ["a1.2.6", "KindAlpha"], ["3D Shareware v1.34", "KindOther"]].map(([v, k]) => h("div.lx-menu-item", h("b", tx(v)), h("span.lx-mright", is(k)))),
            h("div.lx-muted", { style: { padding: "6px 10px" } }, is("NoVersionsMatch"))) : null)),
        h("div.lx-flabel", is("ModLoader")), loaderCards(loader, (k) => ctx.open("newInstance", Object.assign({}, ctx.dparams, { loader: k }))),
        loader !== "vanilla" ? h("div.lx-form", h("span.lx-muted", is("LoaderVersion")), st === "loaderLoading" ? h("span.lx-muted", spinner(), " ", is("LoadingLoaderVersions", { loader: "Fabric" })) :
          st === "noLoader" ? h("span.lx-muted", is("NoVersionsFor", { game: "26.3" })) : h("div", select(h("span", tx("0.16.14"), " ", badge(is("Recommended"), "green"))), " ", h("span.lx-muted", is("ChooseLoaderVersion", { loader: "Fabric" }), " \u00b7 ", is("ChooseVersion")))) : null,
        st === "noVersion" ? h("div", { style: { color: "#ff8a8a" } }, is("ChooseGameVersion")) : null];
      acts = [gbtn(is("ImportMrpack"), null, "dl"), h("span.lx-grow"), pbtn(is("CreateInstance"), () => { ctx.close(); ctx.set("toast", "none") }), btn(cm("Cancel"), { on: () => ctx.close() })];
    } else if (mode === "modpack") {
      const R = [["Fabulously Optimized", "robotkoer", "6.4M", 200], ["Cobblemon Official", "Cobblemon", "2.1M", 0], ["Better MC [FABRIC]", "SharkieRealm", "1.9M", 120]];
      body = [h("div.lx-tbar", h("div.lx-grow", field(null, ctx.state.newinst === "curseforge" ? im("SearchModpacksCurseForge") : im("SearchModpacks"), { icon: "search" })), seg([["m", ct("ModrinthTab")], ["c", ct("CurseForgeTab")]], ctx.state.newinst === "curseforge" ? "c" : "m", (k) => ctx.set("newinst", k === "c" ? "curseforge" : "ok")),
        h("div", { style: { position: "relative" } }, select(im("SortRelevance"), { on: toggleMenu("mpsort") }), ui.menu === "mpsort" ? menu([{ label: im("SortRelevance") }, { label: im("SortDownloads") }, { label: im("SortFollows") }, { label: im("SortUpdated") }], "lx-rmenu") : null)),
      h("div", { style: { display: "grid", gridTemplateColumns: "1fr 1fr", gap: "16px", minHeight: "300px" } },
        st === "loading" ? empty(null, spinner(), im("SearchingModrinth")) : st === "nothing" ? empty("search", im("NoModpacksMatch")) : st === "notModpack" ? note(im("LinkNotModpack", { kind: "mod" }), "warn") :
          card(R.map(([n, a, d, hue], i) => h("div.lx-row" + (i === 0 ? ".on" : ""), { style: i === 0 ? { background: "var(--g750)" } : {} }, modIcon(hue), h("div.lx-row-main", h("div.lx-row-t", tx(n)), h("div.lx-row-s", im("ByAuthor", { author: a, downloads: d })))))),
        h("div", seg([["v", im("Versions")], ["d", im("Description")]], "v"), h("div", { style: { marginTop: "10px" } }, st === "pick" ? h("div.lx-muted", im("PickModpack")) : st === "loadingPack" ? h("div.lx-muted", spinner(), " ", im("LoadingModpack"), " \u00b7 ", im("LoadingVersions")) : st === "noVersions" ? h("div.lx-muted", im("NoVersions")) :
          card(["6.4.0", "6.3.2", "6.3.1"].map((v, i) => h("div.lx-row", { style: { minHeight: "46px" } }, h("span.lx-radio" + (i === 0 ? ".on" : ""), h("i")), h("div.lx-row-main", tx("Fabulously Optimized " + v), h("div.lx-row-s", tx("1.21.4 \u00b7 Fabric"))))))))),
      h("div.lx-muted", { style: { fontSize: "13px" } }, im("KindMod"), " / ", im("KindResourcePack"), " / ", im("KindShaderPack"), " / ", im("KindDataPack"), " / ", im("KindPlugin"), " \u00b7 ", is("PickModpack"))];
      acts = [gbtn(is("ImportMrpack"), null, "dl"), h("span.lx-grow"), pbtn(is("InstallModpack")), btn(cm("Cancel"), { on: () => ctx.close() })];
    } else {
      const err = { notCode: "CodeNotACode", newer: "CodeNewer", tooLarge: "CodeTooLarge", damaged: "CodeDamaged" }[st];
      // uses: is("CodeNotACode") is("CodeNewer") is("CodeTooLarge") is("CodeDamaged")
      body = [h("div.lx-form", h("span.lx-muted", is("Name")), field(null, is("MyInstance"))), h("div", { style: { minHeight: "90px" } }, field(st === "ok" || st === "pack" ? "LSC1:eJyNkM1qwzAQhF9F\u2026" : null, is("PasteShareCode"), { err: !!err, cls: "lx-mono" })),
        err ? h("div", { style: { color: "#ff8a8a" } }, is(err)) : null,
        st === "pack" ? h("div.lx-scard", h("b", is("CodePackPreview", { pack: "Gaycraft 5.0" }), " ", h("span.lx-muted", isp("CodePackChanges", { count: 3 }))), h("div.lx-muted", tx("Minecraft 1.21.1 \u00b7 Fabric 0.16.9"))) :
          h("div.lx-scard", h("b", tx("Survival 1.20.1")), h("div.lx-muted", tx("Minecraft 1.20.1 \u00b7 Fabric 0.16.14 \u00b7 14 mods")))];
      acts = [h("span.lx-grow"), pbtn(is("CreateInstance")), btn(cm("Cancel"), { on: () => ctx.close() })];
    }
    return dialog(ctx, is("NewInstance"), body, acts, { wide: true, headRight: head, cls: "lx-dlg-vis" });
  }
  function shareCodeDialog(ctx) {
    const st = ctx.state.shareCode;
    if (st === "reading") return dialog(ctx, is("ShareTitle", { name: "Survival 1.20.1" }), [waitBox(is("ShareReading"))], []);
    const pack = st === "pack";
    return dialog(ctx, is("ShareTitle", { name: pack ? "All the mods" : "Survival 1.20.1" }), [
      h("p", pack ? is("ShareIntroPack") : is("ShareIntro")),
      pack ? h("div.lx-scard", h("b", is("ShareBasedOnPack", { pack: "Gaycraft 5.0" })), h("div.lx-muted", is("ShareChanges", { changes: "" }), isp("ShareAddedCount", { count: 2 }), ", ", isp("ShareRemovedCount", { count: 1 }), ", ", isp("ShareChangedCount", { count: 1 }), ", ", isp("ShareToggledCount", { count: 3 })), h("div.lx-muted", is("ShareNoChanges"))) : null,
      h("div.lx-code", { style: { whiteSpace: "pre-wrap", wordBreak: "break-all" } }, tx("LSC1:eJyNkM1qwzAQhF9F6F4kO0oDpVBDoAQK7aGXHhZpbQtkScgrN3n7rp0mTktLz5r5ZnZ2BTK\u2026")),
      h("div", { style: { display: "flex", gap: "8px" } }, pbtn(is("CopyCode"), null, "copy"), h("span.lx-muted", is("CodeCopied"))),
      note([h("b", isp("ShareNotShared", { count: 3 })), h("div", pack ? is("ShareNotSharedBodyPack") : is("ShareNotSharedBody")),
        h("div", { style: { display: "grid", gap: "4px", marginTop: "8px", color: "var(--g300)" } },
          h("div", tx("journeymap-1.20.1.jar"), " \u2014 ", is("ShareFromCurseForge")), h("div", tx("homemade-tweaks.jar"), " \u2014 ", is("ShareAddedByHand")), h("div", tx("custom-pack.zip"), " \u2014 ", is("ShareNotOnModrinth")),
          pack ? h("div", tx("config/sodium-options.json"), " \u2014 ", is("ShareChangedSincePack")) : h("div", tx("lithium.jar"), " \u2014 ", is("ShareUnchecked")), h("div.lx-muted", is("ShareMore", { count: 4 })))], "warn"),
    ], [btn(cm("Done"), { on: () => ctx.close() })]);
  }
  function addServerDialog(ctx) {
    const signIn = ctx.dparams.signin;
    const err = ctx.state.addServer;
    return dialog(ctx, signIn ? sv("SignInTitle") : sv("AddServer"), [
      h("p.lx-muted", signIn ? sv("SignInIntro") : sv("AddIntro")),
      signIn ? null : [h("div.lx-flabel", sv("Address")), field(null, sv("AddressPlaceholder"), { err: err === "address" })],
      err === "address" ? h("div", { style: { color: "#ff8a8a" } }, sv("EnterAddress")) : null,
      signIn ? null : h("b", sv("SignInOptional")),
      h("div.lx-grid2", field(null, signIn ? sv("Username") : sv("UsernameOptional")), field(null, sv("Password"))),
      err === "password" ? h("div", { style: { color: "#ff8a8a" } }, sv("EnterPassword"), " ", sv("EnterUsernamePassword")) : null,
      err === "bad" ? note(sv("CoreBadCredentials"), "err") : null,
      h("b", sv("ConsoleAccess")), check(sv("ReadConsole"), true), check(sv("SendChat")), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sv("ConsoleAccessHint")),
      h("div", { style: { display: "flex", alignItems: "center", gap: "10px" } }, sv("KeyExpiresAfter", { computer: "this Mac" }), seg([["90", sv("KeyDays90")], ["y", sv("KeyYear")], ["n", sv("KeyNever")]], err === "never" ? "n" : "90")),
      err === "never" ? note(sv("NeverExpiresWarning"), "warn") : null,
      h("div.lx-muted", { style: { fontSize: "13px" } }, sv("PasswordNotKept")),
    ], [pbtn(signIn ? cm("SignIn") : cm("Add"), () => signIn ? ctx.close() : ctx.open("trust")), btn(cm("Cancel"), { on: () => ctx.close() })]);
  }
  function joinFriendDialog(ctx) {
    const st = ctx.state.join;
    if (st === "paste" || st === "invalid" || st === "empty") return dialog(ctx, sv("JoinFriendTitle"), [h("p.lx-muted", sv("JoinFriendIntro")), field(st === "invalid" ? "https://example.com/abc" : null, sv("InvitePlaceholder"), { err: st !== "paste" }),
      st === "invalid" ? h("div", { style: { color: "#ff8a8a" } }, sv("NotAnInvite")) : st === "empty" ? h("div", { style: { color: "#ff8a8a" } }, sv("PasteInviteFirst")) : null], [pbtn(cm("Continue"), () => ctx.set("join", "connecting")), btn(cm("Cancel"), { on: () => ctx.close() })]);
    if (st === "connecting" || st === "slow" || st === "reading") return dialog(ctx, sv("JoinFriendTitle"), [h("b", st === "reading" ? sv("ConnectingTo", { name: "Creative build" }) : sv("ConnectingToFriend")), waitBox(st === "slow" ? sv("StageFindingSlow") : st === "reading" ? sv("StageReading") : sv("StageFinding"))], [btn(cm("Cancel"), { on: () => ctx.close() })]);
    const F = { failed: ["FriendJoinFailed", "FriendJoinFailedBody"], noPath: ["FriendNoPath", "FriendNoPathBody"], stopped: ["FriendServerStopped", "FriendServerStoppedBody"], ended: ["FriendLinkEnded", "FriendLinkEndedBody"], notSharing: ["FriendNotSharing", "FriendNotSharingBody"],
      wrongHost: ["FriendWrongHost", "FriendWrongHostBody"], offline: ["FriendOffline", "FriendOfflineBody"], service: ["FriendServiceUnreachable", "FriendServiceUnreachableBody"], tunnel: ["FriendTunnelEnded", "FriendTunnelEndedBody"] };
    // uses: sv("FriendJoinFailed") sv("FriendJoinFailedBody") sv("FriendNoPath") sv("FriendNoPathBody") sv("FriendServerStopped") sv("FriendServerStoppedBody") sv("FriendLinkEnded") sv("FriendLinkEndedBody") sv("FriendNotSharing") sv("FriendNotSharingBody") sv("FriendWrongHost") sv("FriendWrongHostBody") sv("FriendOffline") sv("FriendOfflineBody") sv("FriendServiceUnreachable") sv("FriendServiceUnreachableBody") sv("FriendTunnelEnded") sv("FriendTunnelEndedBody")
    if (F[st]) return dialog(ctx, sv("JoinFriendTitle"), [note([h("b", sv(F[st][0], { name: "Creative build" })), h("div", sv(F[st][1], { detail: "connection refused" }))], st === "stopped" ? "warn" : "err")], [pbtn(cm("TryAgain"), () => ctx.set("join", "connecting")), btn(cm("Cancel"), { on: () => ctx.close() })]);
    const mods = st === "readyMods" || st === "readyLinked";
    return dialog(ctx, sv("JoinFriendTitle"), [
      h("div.lx-scard", h("div", { style: { display: "flex", alignItems: "center", gap: "8px", fontFamily: "Chillax", fontWeight: 600, fontSize: "19px" } }, dot("on"), tx("Creative build")), h("div.lx-muted", sv("SharedBy", { name: "Alex's Lodestone" })),
        h("div.lx-facts", sv("FactVersion"), tx("1.21.4"), sv("FactMods"), mods ? sv("FriendHasMods") : sv("FriendNoMods"), sv("FactStatus"), h("span", sv("StateRunning"), " \u00b7 ", sv("PlayersOfMax", { count: 3, max: 20 })),
          sv("FactConnection"), st === "readyLocal" ? sv("DirectConnection") : sv("ConnectedVia", { how: "over your local network", ms: 18 }),
          sv("Address"), h("span", tx("127.0.0.1:36017 "), btn(cm("Copy"), { sm: true, icon: "copy" })),
          sv("FactInstance"), st === "readyLinked" ? sv("PlaysWithLinked", { name: "Creative build" }) : mods ? sv("MakesInstanceWithMods") : st === "readyInstance" ? sv("PlaysWithInstance", { name: "Latest release" }) : sv("MakesInstance"))),
      st === "portTaken" ? note([h("b", sv("FriendPortTaken", { port: 25565 })), h("div", sv("FriendPortTakenBody", { address: "127.0.0.1:36018", port: 25565 }))], "warn") : null,
      st === "starting" ? waitBox(sv("FriendStarting")) : null, st === "busy" ? note(sv("FriendInstanceBusy"), "warn") : null,
      h("div.lx-muted", { style: { fontSize: "13px" } }, sv("ViaSameComputer"), " \u00b7 ", sv("ViaLocalNetwork"), " \u00b7 ", sv("ViaIpv6"), " \u00b7 ", sv("ViaInternet")),
    ], [pbtn(sv("Play"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })]);
  }
  function addFriendDialog(ctx) {
    const st = ctx.state.addFriend;
    const errs = { notLink: "NotAFriendLink", paste: "PasteFirst", other: "OtherService", already: "AlreadyFriends", needName: "NeedNameFirst" };
    // uses: fl("NotAFriendLink") fl("PasteFirst") fl("OtherService") fl("AlreadyFriends") fl("NeedNameFirst")
    if (st === "preview" || st === "added") return dialog(ctx, fl("AddTitle"), [h("b", fl("PreviewFrom", { name: "Alex" })), h("div.lx-muted", fl("PlaysAs", { name: "AlexCrafts" })), h("div.lx-mono", fl("SafetyCode", { code: "P4XN-7TQE-K2MW-9RBD" })), h("p.lx-muted", fl("PreviewCompare", { name: "Alex" })), st === "added" ? note(fl("Added", { name: "Alex" }), "ok") : null],
      [pbtn(fl("AddFriend"), () => ctx.set("addFriend", "added")), btn(cm("Cancel"), { on: () => ctx.close() })]);
    return dialog(ctx, fl("AddTitle"), [
      h("b", fl("SendLinkTitle")), h("p.lx-muted", fl("SendLinkBody")),
      st === "made" ? [h("div.lx-tbar", h("div.lx-grow", field("https://connect.lodestone.cc/f/9fK2xQ7\u2026", null, { cls: "lx-mono" })), btn(cm("Copy"), { icon: "copy" })), h("div.lx-muted", fl("LinkWorksOnce"))] : h("div", pbtn(fl("MakeLink"), () => ctx.set("addFriend", "made"))),
      h("b", { style: { marginTop: "10px" } }, fl("OpenLinkTitle")),
      h("div.lx-tbar", h("div.lx-grow", field(null, fl("LinkPlaceholder"), { err: !!errs[st] })), btn(fl("LookUp"), { on: () => ctx.set("addFriend", "preview") })),
      errs[st] ? h("div", { style: { color: "#ff8a8a" } }, fl(errs[st], { host: "connect.example.org", name: "Alex" })) : null,
    ], [btn(cm("Close"), { on: () => ctx.close() })]);
  }
  const DIALOG_SIMPLE = {
    friendName: (ctx) => dialog(ctx, fl("NameTitle"), [h("p.lx-muted", fl("NameBody")), field(null, fl("NamePlaceholder")), ctx.state.addFriend === "needName" ? h("div", { style: { color: "#ff8a8a" } }, fl("NameEmpty")) : null], [pbtn(cm("Save"), () => ctx.close()), gbtn(fl("UseMinecraftName"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    verify: (ctx) => dialog(ctx, fl("VerifyTitle", { name: "Sammy" }), [h("p.lx-muted", fl("VerifyBody", { name: "Sammy" })), h("div.lx-grid2", h("div.lx-scard", h("div.lx-muted", fl("YourCode")), h("div.lx-mono", { style: { fontSize: "18px" } }, tx("K7Q2-M9XD-4TRW-H3BE"))), h("div.lx-scard", h("div.lx-muted", fl("TheirCode", { name: "Sammy" })), h("div.lx-mono", { style: { fontSize: "18px" } }, tx("P4XN-7TQE-K2MW-9RBD"))))],
      [pbtn(fl("CodesMatch"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })]),
    renameFriend: (ctx) => dialog(ctx, fl("RenameTitle", { name: "Sammy" }), [h("p.lx-muted", fl("RenameBody", { name: "Sam" })), field("Sammy", fl("NicknamePlaceholder"))], [pbtn(cm("Save"), () => ctx.close()), gbtn(fl("UseTheirName")), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    retire: (ctx) => dialog(ctx, fl("RetireTitle"), [h("p", fl("RetireBody")), note(fl("RetireWarning"), "warn"), check(fl("RetireUnderstood"))], [dbtn(fl("RetireConfirm")), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    retireCore: (ctx) => dialog(ctx, fl("RetireCoreTitle", { name: "Peter's PC" }), [h("p", fl("RetireCoreBody", { name: "Peter's PC" })), note(fl("RetireCoreWarning"), "warn")], [dbtn(fl("RetireCoreConfirm")), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    removeFriend: (ctx) => confirmDlg(ctx, fl("RemoveTitle", { name: "Sammy" }), h("p", fl("RemoveBody")), fl("RemoveFriend"), { danger: true }),
    offlineAccount: (ctx) => dialog(ctx, sh("AddOfflineAccount"), [h("p.lx-muted", sh("OfflineDialogBody")), h("div.lx-flabel", sh("Username")), field(null, tx("Dev_Steve")), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sh("UsernameRule")), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sh("OfflineNote"))],
      [pbtn(sh("AddAccount"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    removeAccount: (ctx) => confirmDlg(ctx, sh("RemoveAccountTitle", { name: "Lodestoner" }), h("p", sh("RemoveAccountBody", { keychain: "Keychain" })), sh("RemoveAccount"), { danger: true }),
    stopSignIn: (ctx) => dialog(ctx, sh("StopSigningInTitle"), h("p", sh("StopSigningInBody")), [dbtn(sh("StopSigningIn"), () => ctx.close()), btn(sh("KeepWaiting"), { on: () => ctx.open("signin") })], { narrow: true }),
    launcherIdentity: (ctx) => dialog(ctx, sh("SignInAsLauncherTitle"), ["WhatThisDoes", "Risks", "WhyHere", "WhenSignIn", "WhatLauncherGets", "HowToUndo"].map((k) => h("div", h("b", sh(k)), h("p.lx-muted", sh(k + "Body", { add: "Add Microsoft account", keychain: "macOS Keychain", badge: "Launcher identity" })))),
      // uses: sh("WhatThisDoes") sh("WhatThisDoesBody") sh("Risks") sh("RisksBody") sh("WhyHere") sh("WhyHereBody") sh("WhenSignIn") sh("WhenSignInBody") sh("WhatLauncherGets") sh("WhatLauncherGetsBody") sh("HowToUndo") sh("HowToUndoBody")
      [dbtn(sh("TurnItOn"), () => { ctx.close(); ctx.set("account", "launcher") }), btn(cm("Cancel"), { on: () => ctx.close() })]),
    turnOffLauncher: (ctx) => dialog(ctx, sh("TurnOffLauncherTitle"), [h("p", sh("TurnOffLauncherBody")), h("p", sh("TurnOffLauncherAccounts", { accounts: "Lodestoner is signed in as the Minecraft Launcher." })), h("p.lx-muted", sh("OneLauncherAccount", { name: "Lodestoner" })), h("p.lx-muted", shp("LauncherAccounts", { count: 2, names: "Lodestoner, Alt" }))],
      [dbtn(sh("TurnOffAndSignOut"), () => { ctx.close(); ctx.set("account", "microsoft") }), btn(sh("TurnOffOnly"), { on: () => ctx.close() }), btn(cm("Cancel"), { on: () => ctx.close() })]),
    deleteInstance: (ctx) => confirmDlg(ctx, sh("DeleteNamed", { name: "All the mods" }), [h("p", sh("DeleteInstanceBody", { trash: "the Trash" })), ctx.state.game === "playing" ? note(im("StopBeforeDeleting"), "warn") : null], cm("MoveToTrash"), { danger: true }),
    deleteWorld: (ctx) => confirmDlg(ctx, sh("DeleteNamed", { name: "Survival Island" }), [h("p", sh("DeleteWorldBody", { trash: "the Trash" })), h("p.lx-muted", cm("TheTrash"))], cm("MoveToTrash"), { danger: true }),
    replaceWorld: (ctx) => confirmDlg(ctx, sh("ReplaceWorldTitle", { name: "Survival Island" }), h("p", sh("ReplaceWorldBody", { name: "Survival Island", when: "today, 03:04" })), sh("BackUpAndReplace"), { danger: true }),
    switchLoader: (ctx) => dialog(ctx, ctx.state.switchTo === "vanilla" ? sh("SwitchToVanilla") : sh("SwitchToLoader", { loader: "Fabric" }), [
      h("p", ctx.state.switchTo === "vanilla" ? sh("ModsWontLoadVanilla") : sh("ModsWontLoadOn", { loader: "Fabric" })),
      card(h("div.lx-row", { style: { minHeight: "44px" } }, h("div.lx-row-main", tx("Create")), h("span.lx-muted", sh("ModMadeFor", { loader: "Forge" }))), h("div.lx-row", { style: { minHeight: "44px" } }, h("div.lx-row-main", tx("JEI")), h("span.lx-muted", sh("ModMadeFor", { loader: "Forge" })))),
      h("p.lx-muted", shp("AndMore", { count: 3 })), h("p.lx-muted", sh("TurningOffRenames")), note(sh("WorldsAtRisk"), "warn"), h("p.lx-muted", im("NoModLoaderFor", { version: "1.2.5" })),
      ctx.state.instSettings === "error" ? note([is("CouldntTurnModsOff", { error: "file in use" }), " ", im("CouldntSwitchLoader")], "err") : null],
      [pbtn(sh("TurnOffAndSave"), () => { ctx.close(); ctx.set("toast", "switched") }), btn(sh("SaveAndKeep"), { on: () => ctx.close() }), btn(cm("Cancel"), { on: () => ctx.close() })]),
    mainOffer: (ctx) => dialog(ctx, sh("MainOfferTitle", { name: "Survival 1.20.1" }), [h("p", sh("MainOfferBody")), h("p.lx-muted", sh("MainChangeLater"))], [pbtn(sh("MakeItMain"), () => { ctx.close(); ctx.set("toast", "synced") }), btn(cm("NotNow"), { on: () => ctx.close() })], { narrow: true }),
    mainSwitch: (ctx) => dialog(ctx, sh("MainOfferTitle", { name: "All the mods" }), [h("p", sh("MainSwitchBody", { current: "Survival 1.20.1", name: "All the mods" })), h("p.lx-muted", sh("MainChangeLater")), h("p.lx-muted", is("IsMainInstance"), " \u00b7 ", is("SharingMain"), " \u00b7 ", is("SharingOff"), " \u00b7 ", is("SharingNoMain"))],
      [pbtn(sh("MakeNameMain", { name: "All the mods" }), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    unsaved: (ctx) => dialog(ctx, sh("SaveChangesTitle"), [h("p", ctx.state.unsavedWhere === "defaults" ? sh("UnsavedDefaultsBody") : ctx.state.unsavedWhere === "this" ? sh("UnsavedThisInstanceBody") : sh("UnsavedInstanceBody", { name: "All the mods" }))],
      [pbtn(cm("Save"), () => ctx.close()), btn(cm("Discard"), { on: () => ctx.close() }), btn(cm("KeepEditing"), { on: () => ctx.close() })], { narrow: true }),
    trust: (ctx) => dialog(ctx, sv("TrustTitle", { name: "Alex's server box" }), [h("p", sv("CoreAt", { name: "Alex's server box", address: "192.168.1.42:16662" })), h("p", sv("TrustExplanation", { computer: "this Mac" })), h("p.lx-muted", sv("TrustAfterAdd", { setting: "Trusted for mods" })), h("p.lx-muted", sv("TrustLater"))],
      [pbtn(sv("TrustThisCore"), () => ctx.close()), btn(cm("NotNow"), { on: () => ctx.close() })]),
    certificate: (ctx) => { const ch = ctx.dparams.changed; return dialog(ctx, ch ? sv("CertChangedTitle", { name: "Alex's server box" }) : sv("CertTrustTitle"), [
      h("p", ch ? sv("CertChangedIntro", { address: "192.168.1.42:16662" }) : sv("CertUntrustedIntro", { os: "macOS", name: "Alex's server box", address: "192.168.1.42:16662" })),
      ch ? [caps(sv("FingerprintTrustedBefore")), h("div.lx-code", tx("4A:1F:9C:\u2026:E2:07")), caps(sv("FingerprintPresented")), h("div.lx-code", tx("B3:77:0D:\u2026:19:AC"))] : [caps(sv("FingerprintCaption")), h("div.lx-code", tx("B3:77:0D:5E:21:C4:8A:90:\u2026:19:AC"))],
      h("p.lx-muted", sv("FingerprintHelp")), h("p.lx-muted", ch ? sv("CertIfMatchChanged", { host: "192.168.1.42" }) : sv("CertIfMatch", { host: "192.168.1.42" }))],
      [pbtn(ch ? sv("TrustNewCertificate") : sv("TrustCertificate"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })]) },
    advisory: (ctx) => dialog(ctx, sv("AdvisoryTitle", { computer: "this Mac" }), [
      h("p", ctx.state.advisory === "update" ? sv("AdvisoryUpdate", { server: "Skyblock", core: "Alex's server box" }) : sv("AdvisoryNew", { server: "Skyblock", core: "Alex's server box" })),
      ctx.state.advisory === "offline" ? note(sv("ModrinthUnreachable"), "warn") : null,
      h("b", sv("UploadedNotChecked", { count: 2 })), card(h("div.lx-row", { style: { minHeight: "40px" } }, h("span.lx-mono", tx("skyblock-core-1.4.jar"))), h("div.lx-row", { style: { minHeight: "40px" } }, h("span.lx-mono", tx("alex-utils.jar")))),
      h("b", sv("AlsoOnModrinth", { count: 12 })), h("div.lx-muted", tx("Fabric API, Sodium, Lithium, \u2026"), " ", svp("AndMore", { count: 9 })),
      h("p.lx-muted", sv("AlsoFilesScripts", { files: "14 config files, 2 script files" }), " ", sv("AlsoFiles", { files: "" }), svp("ConfigFiles", { count: 14 }), ", ", svp("ScriptFiles", { count: 2 }), ", ", svp("OtherFiles", { count: 1 })),
      h("b", sv("OnlyIfYouTrust", { core: "Alex's server box" })), h("p.lx-muted", sv("AdvisoryTrustNote", { button: "Trust Alex's server box and install", explanation: "" }))],
      [btn(sv("InstallThisTime"), { on: () => ctx.close() }), pbtn(sv("TrustAndInstall", { core: "Alex's server box" }), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })]),
    differentMods: (ctx) => dialog(ctx, sv("DifferentMods", { name: "Survival SMP" }), [
      h("p", sv("DifferentModsBody", { changes: "" }), svp("ModsAdded", { count: 2 }), ", ", svp("ModsUpdated", { count: 1 }), ", ", svp("ModsRemoved", { count: 1 }), ", ", svp("OtherFiles", { count: 3 })),
      card([sv("ChangeNew", { name: "Create 0.5.1" }), sv("ChangeChanged", { name: "Sodium 0.5.13" }), sv("ChangeRemoved", { name: "OptiFabric" }), sv("ChangeConfig", { name: "config/create.toml" })].map((x) => h("div.lx-row", { style: { minHeight: "40px" } }, x))), h("div.lx-muted", sv("NoChanges"))],
      [pbtn(sv("UpdateAndPlay"), () => ctx.close()), btn(sv("PlayAnyway"), { on: () => ctx.close() }), btn(cm("Cancel"), { on: () => ctx.close() })]),
    hostOnCore: (ctx) => { const st = ctx.state.host; return dialog(ctx, sv("HostTitle"), st === "done" ? [note(svp("HostDone", { name: "All the mods", core: "Peter's Lodestone Core", port: 25566, count: 42 }), "ok"), h("p.lx-muted", svp("HostDoneRelinked", { name: "All the mods", core: "Peter's Lodestone Core", port: 25566, count: 42, server: "Old SMP" }))] :
      st === "progress" ? [waitBox(sv("StageUploading", { size: "3.2 MB" })), h("div.lx-muted", sv("StageExporting"), " \u00b7 ", sv("StageSigningIn"), " \u00b7 ", sv("StageCreating"))] : [
        h("p.lx-muted", sv("HostIntro", { name: "All the mods" })),
        h("div.lx-form", h("span.lx-muted", sv("CoreLabel")), select(tx("Peter's Lodestone Core")), h("span.lx-muted", sv("ServerName")), field("All the mods"), h("span.lx-muted", sv("Memory")), field("4096 MB")),
        h("div.lx-muted", sv("GamePortHint", { port: 25565 })), check(sv("ShowOnServersPage"), true),
        h("div", { style: { display: "flex", alignItems: "center", gap: "10px" } }, check(sv("EulaAgree", { url: "https://aka.ms/MinecraftEULA" })), gbtn(sv("ReadEula"), null, "ext")),
        st === "errors" ? note([sv("NameTheServer"), " ", sv("AgreeEulaFirst"), " ", sv("HostNeedsPassword", { computer: "this Mac" }), " ", sv("AddCoreFirst"), " ", sv("CoreGone"), " ", sv("InstanceGone"), " ", sv("PackNotKept"), " ", sv("SignedOutRightAway")], "err") : null],
      [st === "done" ? btn(cm("Done"), { on: () => ctx.close() }) : pbtn(sv("CreateServer"), () => ctx.set("host", "progress")), st === "done" ? null : btn(cm("Cancel"), { on: () => ctx.close() })]) },
    joinLan: (ctx) => dialog(ctx, sv("JoinWorld", { world: "Steve's world" }), [h("p.lx-muted", sv("PickInstance", { address: "192.168.1.31:51234" })),
      ctx.state.instances === "none" ? h("p", sv("NoInstancesYet")) : card(INST.slice(1, 4).map((i, n) => h("div.lx-row", { style: { minHeight: "48px" } }, h("span.lx-radio" + (n === 0 ? ".on" : ""), h("i")), grass(), h("div.lx-row-main", tx(i.name), h("div.lx-row-s", tx(i.ver))))))],
      [pbtn(sv("Join"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    modpackUpdate: (ctx) => dialog(ctx, sh("UpdateTitle", { name: "All the mods" }), [
      h("p", sh("ModpackSummary", { pack: "Fabulously Optimized", from: "6.1.0", to: "6.2.0", game: "1.21.4", loader: "Fabric 0.16.9", size: "38 MB" })),
      ...[["UpdatedFiles", 12], ["AddedFiles", 3], ["RemovedFiles", 1], ["ReplacedOwnFiles", 1], ["KeptFiles", 2], ["WrittenFiles", 8]].map(([k, n]) => h("div", h("b", sh(k, { count: n })), h("div.lx-muted.lx-mono", tx(k === "RemovedFiles" ? "mods/old-mod.jar" : "mods/sodium-fabric-0.6.0.jar, \u2026")))),
      // uses: sh("UpdatedFiles") sh("AddedFiles") sh("RemovedFiles") sh("ReplacedOwnFiles") sh("KeptFiles") sh("WrittenFiles")
      h("p.lx-muted", sh("NoFilesChange")), h("p.lx-muted", sh("ModpackUpdateSafety")), check(sh("BackUpEveryWorld"), true)],
      [pbtn(cm("Update"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })]),
    storageConfirm: (ctx) => { const w = ctx.state.storageWhich !== "fromButton" ? ctx.state.storageWhich : (ctx.dparams.which || "Cache"); const M = { GameFiles: ["StorageCleanGameFilesTitle", "StorageCleanGameFilesBody", "StorageRemove"], Runtimes: ["StorageCleanRuntimesTitle", "StorageCleanRuntimesBody", "StorageRemove"], Cache: ["StorageClearCacheTitle", "StorageClearCacheBody", "StorageClear"],
      OldLogs: ["StorageDeleteOldLogsTitle", "StorageDeleteOldLogsBody", "StorageDeleteConfirm"], AllLogs: ["StorageDeleteAllLogsTitle", "StorageDeleteAllLogsBody", "StorageDeleteConfirm"], Leftovers: ["StorageCleanLeftoversTitle", "StorageCleanLeftoversBody", "StorageRemove"], Snapshots: ["StorageCleanSnapshotsTitle", "StorageCleanSnapshotsBody", "StorageRemove"] }[w];
      // uses: sh("StorageCleanGameFilesTitle") sh("StorageCleanGameFilesBody") sh("StorageCleanRuntimesTitle") sh("StorageCleanRuntimesBody") sh("StorageClearCacheTitle") sh("StorageClearCacheBody") sh("StorageDeleteOldLogsTitle") sh("StorageDeleteOldLogsBody") sh("StorageDeleteAllLogsTitle") sh("StorageDeleteAllLogsBody") sh("StorageCleanLeftoversTitle") sh("StorageCleanLeftoversBody") sh("StorageCleanSnapshotsTitle") sh("StorageCleanSnapshotsBody")
      return confirmDlg(ctx, sh(M[0]), h("p", sh(M[1], { size: "312 MB" })), sh(M[2]), { danger: true }) },
    storageMove: (ctx) => { const st = ctx.state.storageMove; if (st === "moving") return dialog(ctx, sh("StorageMoving"), [h("p.lx-muted", sh("StorageMovingTo", { to: "/Volumes/Games/Lodestone" })), progress(40), h("div.lx-muted", sh("StorageMovingProgress", { done: "1.6 GB", total: "3.9 GB" }), " \u00b7 ", sh("StorageMovingPreparing")), h("p.lx-muted", sh("StorageMovingNote"))], [btn(cm("Cancel"))], { narrow: true, noClose: true });
      if (st === "failed") return dialog(ctx, sh("StorageMoveFailedTitle"), [h("p", sh("StorageMoveFailedBody", { from: "~/Library/Application Support/Lodestone Launcher", error: "The disk is full." })), h("p.lx-muted", sh("StorageMoveRestartFailed"))], [btn(cm("Ok"), { on: () => ctx.close() })], { narrow: true });
      if (st === "back") return confirmDlg(ctx, sh("StorageMoveBackTitle"), [h("p", sh("StorageMoveBackBody", { to: "~/Library/Application Support/Lodestone Launcher" })), h("p.lx-muted", sh("StorageMoveSize", { size: "3.9 GB" }))], sh("StorageMoveConfirm"));
      return confirmDlg(ctx, sh("StorageMoveTitle"), [h("p", sh("StorageMoveBody", { to: "/Volumes/Games/Lodestone" })), h("p.lx-muted", sh("StorageMoveSize", { size: "3.9 GB" }))], sh("StorageMoveConfirm")) },
    dataFolderMissing: (ctx) => dialog(ctx, sh("DataFolderMissingTitle"), h("p", sh("DataFolderMissingBody", { path: "/Volumes/Games/Lodestone" })), [btn(cm("TryAgain")), btn(sh("DataFolderUseDefault")), gbtn(sh("DataFolderQuit"))], { narrow: true, noClose: true }),
    import: (ctx) => { const st = ctx.state.importState;
      const L2 = [["Prism Launcher", [["Fabric 1.20.1", "1.2 GB", true], ["Vanilla+", "220 MB", true], ["Old Forge", "3.1 GB", false]]], ["CurseForge", [["RLCraft", "2.4 GB", false]]]];
      if (st === "looking") return dialog(ctx, im("ImportTitle"), [waitBox(im("LookingForLaunchers"))], [btn(im("ChooseFolder")), btn(cm("Cancel"), { on: () => ctx.close() })]);
      if (st === "none") return dialog(ctx, im("ImportTitle"), [empty("folder", im("NoLaunchersFound")), h("p.lx-muted", im("NoInstances"))], [pbtn(im("ChooseFolder")), btn(cm("Cancel"), { on: () => ctx.close() })]);
      if (st === "progress") return dialog(ctx, im("ImportTitle"), [card(
        h("div.lx-row", grass(), h("div.lx-row-main", tx("Fabric 1.20.1"), h("div.lx-row-s", im("ImportedNote", { note: "" }), im("LinksLeftOut")))), h("div.lx-row", grass(), h("div.lx-row-main", tx("Vanilla+"), h("div.lx-row-s", im("Copying", { done: "120 MB", total: "220 MB" })), progress(55))),
        h("div.lx-row", grass(), h("div.lx-row-main", tx("Old Forge"), h("div.lx-row-s", im("MatchingModrinth")))), h("div.lx-row", grass(), h("div.lx-row-main", tx("RLCraft"), h("div.lx-row-s", im("FindingVersion")))), h("div.lx-row", grass(), h("div.lx-row-main", tx("Skyfactory"), h("div.lx-row-s", im("Waiting")))),
        h("div.lx-row", grass(), h("div.lx-row-main", tx("Done one"), h("div.lx-row-s", im("Imported"))))),
        h("p.lx-muted", im("Cancelled"), " \u00b7 ", imp("ProblemAndMore", { problem: "level.dat is damaged", count: 2 }))], [btn(cm("Close"), { on: () => ctx.open("stopImporting") })]);
      return dialog(ctx, im("ImportTitle"), [h("p.lx-muted", im("CheckToImport")), ...L2.map(([l, is2]) => h("div", h("b", tx(l)), card(is2.map(([n, s, on]) => h("div.lx-row", { style: { minHeight: "44px" } }, check(tx(n), on), h("span.lx-grow"), h("span.lx-muted", tx(s))))))),
        check(im("LeaveWorldsOutSize", { size: "640 MB" })), h("div.lx-muted", im("LeaveWorldsOut")), h("div.lx-muted", imp("InstancesToCopy", { count: 2, size: "1.4 GB" }))],
        [pbtn(imp("ImportInstances", { count: 2 }), () => ctx.set("importState", "progress")), btn(im("ChooseFolder")), btn(cm("Cancel"), { on: () => ctx.close() })]) },
    stopImporting: (ctx) => dialog(ctx, im("StopImportingTitle"), h("p", im("StopImportingBody")), [dbtn(im("StopImporting"), () => ctx.close()), btn(im("KeepImporting"), { on: () => ctx.open("import") })], { narrow: true }),
    finderInterrupted: (ctx) => dialog(ctx, im("FinderInterruptedTitle", { name: "All the mods" }), h("p", im("FinderInterrupted", { name: "All the mods" })), [pbtn(im("FinderStart"), () => ctx.open("modFinder")), btn(im("FinderPutBack"), { on: () => ctx.close() })], { narrow: true }),
    snapshotTake: (ctx) => dialog(ctx, is("TakeSnapshotTitle"), [h("div.lx-flabel", is("SnapshotName")), field("Before 1.21 update"), h("b", is("WhatToInclude")), h("div.lx-grid2", L(is, "PartMods PartResourcePacks PartShaderPacks PartConfigs PartWorlds PartScreenshots PartSettings PartOther", (el, k) => check(el, k !== "PartScreenshots"))),
      ctx.state.game === "playing" ? note(is("SnapshotGameRunning"), "warn") : null, ctx.state.instSettings === "error" ? note(is("ChooseSomething"), "err") : null, h("div.lx-muted", is("TakingSnapshot"), " \u00b7 ", is("SnapshotTaken"))], [pbtn(is("TakeSnapshot"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    snapshotRevert: (ctx) => dialog(ctx, is("RevertTitle", { name: "Before 1.21 update" }), [h("p", is("RevertBody")), h("b", is("WhatToRestore")), h("div.lx-grid2", L(is, "PartMods PartConfigs PartWorlds PartSettings", (el) => check(el, true))), h("div.lx-muted", is("RevertingSnapshot"), " \u00b7 ", is("SnapshotReverted"))],
      [dbtn(is("Revert"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    snapshotShare: (ctx) => dialog(ctx, is("ShareSnapshotTitle", { name: "Before 1.21 update" }), [waitBox(is("PreparingFile")), h("div.lx-scard", { style: { display: "flex", flexDirection: "row", alignItems: "center", gap: "10px" } }, ic("folder"), h("span.lx-mono", tx("Before 1.21 update.lsnap"))), h("p.lx-muted", is("DragToShare"))],
      [btn(is("SaveAsEllipsis")), btn(is("ShowInFolder")), btn(cm("Done"), { on: () => ctx.close() })], { narrow: true }),
    snapshotDelete: (ctx) => confirmDlg(ctx, is("DeleteSnapshotTitle", { name: "Before 1.21 update" }), h("p", is("DeleteSnapshotBody")), cm("Delete"), { danger: true }),
    snapshotImport: (ctx) => dialog(ctx, is("ImportSnapshotTitle"), [waitBox(is("ReadingSnapshot")), h("div.lx-scard", h("b", is("SnapshotFrom", { name: "Survival 1.20.1", version: "1.20.1" }))), note(is("SnapshotModsWarning", { count: 14 }), "warn"), note(is("SnapshotLaunchSettingsWarning"), "warn"), h("div.lx-muted", is("ImportingSnapshot"))],
      [pbtn(is("CreateInstanceButton"), () => ctx.close()), btn(cm("Cancel"), { on: () => ctx.close() })], { narrow: true }),
    iconPicker: (ctx) => dialog(ctx, is("Icon"), [seg([["b", is("BlocksAndItems")], ["e", is("Emoji")]], "b"), h("div", { style: { display: "grid", gridTemplateColumns: "repeat(10,1fr)", gap: "8px" } }, Array.from({ length: 30 }, (_, n) => h("span.lx-grass", { style: { filter: `hue-rotate(${n * 12}deg)`, width: "36px", height: "36px" } })))],
      [btn(is("UploadImage")), gbtn(is("UseDefault")), h("span.lx-grow"), btn(cm("Done"), { on: () => ctx.close() })]),
    backgroundPicker: (ctx) => { const st = ctx.state.bg; return dialog(ctx, is("PlayBackground"), [seg([["d", is("BackgroundDefault")], ["i", is("BackgroundImage")], ["s", is("BackgroundSlideshow")]], st === "slideshow" ? "s" : "i"),
      st === "slideshow" ? [h("div", { style: { display: "flex", gap: "16px" } }, check(is("SlideshowThisInstance"), true), check(is("SlideshowAllInstances")), check(is("SlideshowOwnImages"))), h("div.lx-muted", is("SlideshowNoScreenshots")), h("div.lx-muted", is("SlideshowNoImages")), h("div", btn(is("AddImages")), " ", btn(is("AddToSlideshow")))] :
        [h("b", is("YourScreenshots")), h("div.lx-grid3", [0, 1, 2].map((n) => h("div.lx-shot", { style: { filter: `hue-rotate(${n * 40}deg)` } }))), h("div.lx-muted", is("FindingScreenshots"), " \u00b7 ", is("NoScreenshots")),
          h("b", sh("SkinBanners")), h("div.lx-grid3", [0, 1].map(() => h("div.lx-shot", { style: { background: "radial-gradient(circle,#3e4f78,#1a1e28)" } }))), h("div.lx-muted", sh("SkinBannerMissing"))],
      note(is("SavedSeePlayTab"), "ok")], [btn(is("ChooseFile")), gbtn(is("UseDefault")), h("span.lx-grow"), btn(cm("Done"), { on: () => ctx.close() })]) },
    screenshot: (ctx) => dialog(ctx, tx("2026-11-09_21.40.12.png"), [h("div.lx-shot", { style: { height: "380px", aspectRatio: "auto" } }), h("div.lx-muted", wd("Position", { index: 3, count: 6 }))],
      [btn(cm("Previous"), { sm: true }), btn(cm("Next"), { sm: true }), h("span.lx-grow"), btn(wd("CopyImage"), { icon: "copy" }), btn(cm("OpenImage")), btn(cm("ShowInFileManager")), dbtn(cm("MoveToTrash"))], { wide: true }),
    perf: (ctx) => dialog(ctx, sh("PerfMonitor"), [
      h("b", sh("PerfLauncher")), h("div.lx-facts", sh("PerfMemory"), tx("212 MB"), sh("PerfCpu"), sh("PerfCpuValue", { percent: "12.5%", cores: 10 }), tx(""), sh("PerfCpuValueAll", { percent: "1.3%", cores: 10, one: "12.5%" }), sh("PerfThreads"), tx("38"), sh("PerfOpenFiles"), tx("114"), sh("PerfHandles"), sh("PerfUnknown")),
      h("div.lx-spark"), h("div.lx-muted", { style: { fontSize: "12px" } }, sh("PerfLastMinute")),
      h("b", sh("PerfRendering")), h("div.lx-facts", sh("PerfFps"), tx("60"), sh("PerfFrameTime"), sh("PerfFrameStats", { avg: "4.1 ms", p95: "7.9 ms", max: "16.7 ms" })), h("div.lx-muted", sh("PerfIdle")), h("div.lx-muted", sh("PerfBackground")), h("div.lx-muted", { style: { fontSize: "13px" } }, sh("PerfFramesNote")),
      h("b", sh("PerfGame")), h("div.lx-muted", sh("PerfNoGame")),
      h("b", sh("PerfInternals")), h("div.lx-facts", sh("PerfImages"), shp("PerfEntries", { count: 18 }), sh("PerfContentCache"), shp("PerfEntries", { count: 42 }), sh("PerfJobs"), tx("0"), sh("PerfDownloads"), sh("PerfRate", { rate: "2.1 MB" }), sh("PerfCliClients"), tx("1"), sh("PerfTraceBus"), sh("PerfTraceValue", { subscribers: 1, rate: "3.5" })),
      h("div.lx-muted", { style: { fontSize: "13px" } }, sh("PerfSelfNote"))], [btn(sh("PerfCopy"), { icon: "copy" }), h("span.lx-muted", sh("PerfCopied")), h("span.lx-muted", sh("PerfOff"))], { cls: "lx-perf" }),
  };
  function skinEditorDialog(ctx) {
    const surf = "Window TopBar Sidebar Main Hero PlayBar Card SettingCard Row Dialog SettingsNav Popover Input ButtonPrimary ButtonSecondary ButtonGhost ButtonDanger Toggle Tab Pill PlayButton Progress Toast";
    return dialog(ctx, sh("SkinEditorTitle"), h("div", { style: { display: "grid", gridTemplateColumns: "220px 1fr 1fr", gap: "22px" } },
      h("div", h("div.lx-caps", sh("SkinSurfaces")), h("div", { style: { display: "grid", gap: "2px", marginTop: "6px", maxHeight: "520px", overflow: "auto" } }, L(sh, "SkinSurfaceWindow SkinSurfaceTopBar SkinSurfaceSidebar SkinSurfaceMain SkinSurfaceHero SkinSurfacePlayBar SkinSurfaceCard SkinSurfaceSettingCard SkinSurfaceRow SkinSurfaceDialog SkinSurfaceSettingsNav SkinSurfacePopover SkinSurfaceInput SkinSurfaceButtonPrimary SkinSurfaceButtonSecondary SkinSurfaceButtonGhost SkinSurfaceButtonDanger SkinSurfaceToggle SkinSurfaceTab SkinSurfacePill SkinSurfacePlayButton SkinSurfaceProgress SkinSurfaceToast", (el, k) => h("div.lx-menu-item" + (k === "SkinSurfaceSidebar" ? "" : ""), { style: k === "SkinSurfaceSidebar" ? { background: "var(--g750)" } : {} }, el)))),
      h("div", { style: { display: "grid", gap: "10px", alignContent: "start" } },
        h("div.lx-caps", sh("SkinFill")), seg([["c", sh("SkinFillColour")], ["g", sh("SkinFillGradient")], ["i", sh("SkinImage")]], "g"),
        h("div.lx-form", { style: { gridTemplateColumns: "120px 1fr" } }, h("span.lx-muted", sh("SkinOpacity")), tx("90%"), h("span.lx-muted", tx("")), seg([["l", sh("SkinGradientLinear")], ["r", sh("SkinGradientRadial")]], "l"), h("span.lx-muted", sh("SkinAngle")), tx("135\u00b0"),
          h("span.lx-muted", sh("SkinStops")), h("div", tx("#26282C \u2192 #1D1E21 "), gbtn(sh("SkinAddStop")), gbtn(sh("SkinRemoveStop"))), h("span.lx-muted", sh("SkinStopAt")), tx("40%"),
          h("span.lx-muted", sh("SkinBuiltinImages")), tx("\u2014"), h("span.lx-muted", sh("SkinBorder")), tx("#36393F"), h("span.lx-muted", sh("SkinBorderWidth")), tx("1 px"), h("span.lx-muted", sh("SkinRadius")), tx("8 px"),
          h("span.lx-muted", sh("SkinTextColour")), tx("#E3E3E4"), h("span.lx-muted", sh("SkinMutedColour")), tx("#A5A5AC")), h("div.lx-muted", sh("SkinTextElsewhere")),
        h("div.lx-caps", sh("SkinTokens")), h("div.lx-form", { style: { gridTemplateColumns: "120px 1fr" } }, L(sh, "SkinTokenText SkinTokenSecondary SkinTokenMuted SkinTokenFaint SkinTokenBorder", (el) => [h("span.lx-muted", el), tx("#E3E3E4")])),
        h("div.lx-caps", sh("SkinFonts")), h("div.lx-form", { style: { gridTemplateColumns: "150px 1fr" } }, L(sh, "SkinFontHeading SkinFontBody SkinFontSidebar SkinFontButton SkinFontPlay SkinFontTab SkinFontMono", (el) => [h("span.lx-muted", el), h("span", sh("SkinUseLaunchers"))])), h("div", btn(sh("SkinFontAdd")))),
      h("div", { style: { display: "grid", gap: "10px", alignContent: "start" } },
        h("div.lx-caps", sh("SkinBannerSet")), h("div.lx-muted", sh("SkinBannersLead")), h("div.lx-muted", sh("SkinBannersNone")), h("div", btn(sh("SkinBannerAdd"))),
        h("div.lx-caps", sh("SkinPlayButtonSection")), h("div.lx-muted", sh("SkinScrim")),
        h("div.lx-caps", sh("SkinReadability")), h("div.lx-muted", { style: { fontSize: "13.5px" } }, sh("SkinChecksNote")),
        card(h("div.lx-row", { style: { minHeight: "44px", fontSize: "13.5px" } }, h("div.lx-row-main", sh("SkinCheckShort", { surface: "Sidebar", role: "muted text", ratio: "3.1", target: "4.5" })), btn(sh("SkinApplyFix"), { sm: true })),
          h("div.lx-row", { style: { minHeight: "44px", fontSize: "13.5px" } }, h("div.lx-row-main", sh("SkinCheckShort", { surface: "Cards", role: "label", ratio: "2.2", target: "3" }), h("div.lx-row-s", sh("SkinCheckNoFix"))))),
        h("div.lx-muted", { style: { fontSize: "13px" } }, sh("SkinRoleText"), " \u00b7 ", sh("SkinRoleMuted"), " \u00b7 ", sh("SkinRoleFaint"), " \u00b7 ", sh("SkinRoleLabel")),
        h("div", btn(sh("SkinFixAll"), { sm: true })), note(sh("SkinChecksPass", { count: 46 }), "ok"),
        h("div.lx-caps", sh("SkinPreviewIn")), seg([["s", sh("SkinPreviewSettings")], ["m", sh("SkinPreviewMenu")], ["n", sh("SkinPreviewNotice")]], "n"),
        toast(sh("SkinPreviewNotice"), sh("SkinPreviewNoticeText"), "info"), h("div", sh("SkinSampleText")), h("div.lx-muted", sh("SkinSampleMuted")))),
      [pbtn(sh("SkinSaveAsNew")), btn(cm("Close"), { on: () => ctx.close() })], { wide: true, cls: "lx-dlg-xl" });
  }

  // ---------- Messages: notices, progress lines and errors that only show in passing ----------
  const MP = (title, sub, groups) => (ctx) => h("div.lx-main", h("div.lx-page", { "data-mk-scroll": "lx-msg-" + ctx.route }, h("h1.lx-h1", title), sub ? h("div.lx-sub", { style: { marginBottom: "22px" } }, sub) : h("div", { style: { height: "18px" } }), groups()));
  const tl = (el, kind) => toast(null, el, kind || "info");
  const te = (title, body) => toast(title, body, "err");
  const prog = (label, pct) => h("div.lx-scard", { style: { maxWidth: "640px" } }, h("div", label), progress(pct || 40));
  const MSG_PAGES = {
    shell: ["Messages \u203a Window, accounts, sign-in", () => [
      mgroup(sh("Undo"), null, [tl(sh("Undid", { change: "" })), tl(sh("ChangeTo", { setting: "Language", value: "System" })), tl(sh("SettingsOf", { name: "All the mods" })), tl(shp("CopiedValues", { count: 1, name: "Survival 1.20.1" }), "ok")]),
      mgroup(sh("Instances"), null, [
        h("div.lx-irow", { style: { maxWidth: "260px", background: "var(--g800)" } }, grass(), h("div", h("div.lx-irow-n", tx("All the mods")), h("div.lx-irow-s", sh("PlayedInTotal", { when: "Played 2 hours ago", time: "5 h 0 min" })))),
        tl(sh("PlayNamed", { name: "All the mods" })), tl(sh("MinecraftWith", { version: "1.20.1", loader: "Fabric 0.16.14" })), tl(sh("GroupImported")), tl(sh("NoInstances")), tl(sh("GoBack")), tl(sh("GoForward")), tl(sh("LauncherClosing"), "warn"),
        tl(shp("Values", { count: 1 })), tl(shp("AndMore", { count: 1 })), tl(shp("Subscriptions", { count: 1 })), tl(shp("PerfEntries", { count: 1 })), tl(shp("LauncherAccounts", { count: 1, names: "Lodestoner" }))]),
      mgroup(sh("CrashTitle", { name: "All the mods" }), null, L(sh, "EvidenceFailedToPatch EvidenceCrashedInItsCode EvidencePatchedCrashingCode EvidenceInStack", (el) => tl(el))),
      mgroup(sh("Accounts"), null, [te(sh("AuthKeychain"), sh("AuthSignInExpiredBody")), te(null, sh("AccountsCorrupt")), te(null, sh("CouldntReadAccounts", { error: "permission denied" })), te(null, sh("CouldntSaveSettings", { error: "disk full" })),
        te(null, sh("CouldntSaveAccounts", { error: "disk full" })), te(null, sh("CouldntRemoveSignIn", { name: "Lodestoner", error: "item not found" }))]),
      mgroup(sh("SignInWithMicrosoft"), tx("Steps named in \u201cSomething went wrong during {step}\u201d."), L(sh, "StepMicrosoftToken StepDeviceCode StepXboxLive StepXsts StepMinecraftLogin StepEntitlements StepProfile", (el) => tl(el))),
    ]],
    instances: ["Messages \u203a Playing, modpacks, import", () => [
      mgroup(sh("Play"), tx("The line under the Play button while a game gets ready, and the console's notes."), [
        ...L(im, "StagePreparing StageJava StageLibraries StageLoader StageProcessors StageNatives StageStarting", (el) => prog(el, 30)),
        prog(imp("DownloadingFiles", { count: 38 }), 50),
        tl(im("AutoMemoryLimitedNote", { wanted: "10 GB" }), "warn"), tl(im("AutoLowMemoryNote", { have: "2 GB", recommended: "6 GB" }), "warn"), tl(im("AutoVideoNoDistanceNote")), tl(im("AutoControlsLauncherNote")),
        tl(im("StoppedNote")), tl(im("EndedBySignal")), tl(im("CancellingNote")), tl(im("TitleAndMessage", { title: "Couldn't start Java", message: "No such file" })), tl(im("TwoSentences", { first: "Imported.", second: "Links were left out." })),
        tl(isp("SharedWrittenNewFile", { count: 42 })), tl(isp("SharedChangedNote", { count: 2 })), tl(is("SharedNotApplied", { error: "options.txt is read-only" }), "warn"), tl(is("SharedNotReadBack", { error: "options.txt is missing" }), "warn")]),
      mgroup(im("CouldntStartMinecraft"), null, [te(im("RosettaNeeded"), im("RosettaMissing")), toast(im("RosettaNoticeTitle", { version: "1.12.2" }), im("RosettaNoticeBody")), te(im("CouldntStartMinecraft"), im("NoNativeJava8")), te(im("CouldntStartMinecraft"), tx("\u2026")), te(im("CouldntStartJava"), tx("No such file or directory")), te(im("CouldntSignIn"), sh("AuthNetworkBody")),
        te(im("CouldntVerify"), tx("\u2026")), toast(im("GameFilesVerified"), imp("FilesRepaired", { count: 3, summary: "" }), "ok"), tl(imp("CheckSummary", { count: 4091, ms: 45, unchanged: 4091, hashed: 0, downloaded: 0 })),
        tl(im("FilesBusy"), "warn"), tl(im("StopGameFirst"), "warn"), te(null, im("CouldntSavePlayed", { error: "disk full" })), te(null, im("CouldntSavePlayTime", { error: "disk full" })), te(null, imp("BrokenFolders", { count: 2, path: "instances/old" }))]),
      mgroup(im("ModpackExported"), null, [toast(im("ModpackExported"), [imp("FilesSaved", { count: 1, path: "~/Desktop" }), " ", imp("CurseForgeLeftOut", { count: 2, names: "JourneyMap, Xaero's" }), " ", imp("FilesLeftOut", { count: 2, name: "secret.txt" })], "ok"), te(im("CouldntExportModpack"), tx("\u2026"))]),
      mgroup(im("Modpack"), null, [prog(im("ReadingModpack")), prog(im("DownloadingName", { name: "Fabulously Optimized" }), 60), prog(im("InstallingName", { name: "Fabric 0.16.9" }), 70), prog(im("CopyingConfigs"), 80), prog(im("RecordingFiles"), 85), prog(im("SavingSnapshot"), 90), prog(im("BackingUp", { name: "Survival Island" }), 30), prog(im("UpdatingTo", { version: "6.2.0" }), 50),
        toast(im("ModpackReady", { name: "Fabulously Optimized" }), [im("ModpackReadyMessage", { version: "1.21.4", loader: "Fabric 0.16.9" }), " ", imp("PackFilesToDownload", { count: 2 }), " ", imp("PackFilesLeftOut", { count: 1, path: "overrides/x" })], "ok"),
        toast(im("ModpackUpdated"), [im("ModpackUpdatedMessage", { name: "Survival", pack: "Fabulously Optimized", version: "6.1.0", path: "backups/" }), " ", imp("WorldsBackedUp", { count: 2 })], "ok"),
        te(im("CouldntImportModpack"), tx("\u2026")), te(im("CouldntUpdateModpack"), im("UpdatedButNotSaved", { error: "disk full" })), te(im("NotAModpack"), im("NotMrpack", { name: "notes.zip" })), te(im("NotAModpack"), im("CurseForgeUnreachable")), te(im("NotAModpack"), im("CurseForgePackOnWebsite"))]),
      mgroup(im("ImportTitle"), null, [tl(im("ImportFromHere")), tl(im("Import")), tl(im("NewestRelease")), tl(im("NewestSnapshot")), tl(im("Vanilla"))]),
      mgroup(is("ShareCodeRow"), null, [toast(im("ShareCodeReady", { name: "Survival" }), [imp("ShareCodeInstalled", { count: 1 }), " ", im("ShareCodeNotInstalled", { names: "JourneyMap" })], "ok"), te(im("ShareCodeContentFailed"), im("ShareCodeOffline"))]),
    ]],
    content: ["Messages \u203a Content", () => [
      mgroup(ct("KindMods"), tx("Notices above the list, and install results."), [tl(ct("StopGameFirst"), "warn"), tl(ct("WaitForInstall"), "warn"), te(null, ct("CouldntCheckUpdates", { error: "Modrinth answered 503" })), tl(ct("NothingInstalled")), tl(ct("InstalledOne", { name: "Sodium" }), "ok"),
        tl(ct("AddedFiles", { files: "a.jar, b.jar" }), "ok"), te(null, ct("NotAddedFiles", { files: "c.txt: not a mod" })), te(null, ct("AddedAndNotAdded", { added: "a.jar", failed: "c.txt: not a mod" })), te(null, ct("FileError", { name: "c.txt", error: "not a mod" })),
        tl(ct("LinkNotInstallable", { kind: "modpack" }), "warn"), te(null, ct("SomeFilesNotMatched", { error: "timed out" })), tl(ct("AddedDownloads", { names: "JourneyMap" }), "ok"), te(null, ct("CouldntAddDownload", { name: "JourneyMap", error: "damaged zip" }))]),
      mgroup(ct("Versions"), tx("Version tags, sort orders and the file dialog's buttons."), [h("div", L(ct, "Compatible OlderVersion NewerVersion NotOnEitherPlatform Vanilla", (el) => [badge(el), " "])),
        h("div", L(ct, "SortRelevance SortDownloads SortFollows SortNewest SortUpdated SortPopular SortName", (el) => [h("span.lx-keycap", el)])),
        h("div", L(ct, "AddToMods AddToResourcePacks AddToShaders WatchFolderPrompt ViewOnCurseForge", (el) => [btn(el, { sm: true }), " "])),
        h("div.lx-muted", ctp("Followers", { count: 1 })), empty(null, spinner(), ct("SearchingCurseForge")), empty("search", ct("NothingOnCurseForge")), empty(null, spinner(), [ct("LoadingProject"), " \u00b7 ", ct("LoadingVersions")])]),
      mgroup(ct("KindShaders"), null, [h("div.lx-scard", h("b", ct("ShadersNeedShaderMod")), h("div.lx-muted", ct("ShaderModRunsOn", { shader_mod: "Iris", loader: "Fabric" }), " ", ct("ShaderModComesWith", { dependency: "Sodium", shader_mod: "Iris" })), h("div", pbtn(ct("InstallShaderMod", { shader_mod: "Iris", version: "1.7.6" }))), h("div.lx-muted", ct("OptiFineNote", { add_file: "Add file\u2026", mods: "Mods" }))),
        tl(ct("LookingForShaderMod")), te(null, ct("CouldntAskModrinth", { error: "offline" })), tl(ct("ShaderModUnavailable", { reason: "No version for 1.8.9." })), tl(ct("ShaderModUsingBuiltIn", { shader_mod: "Iris" })), tl(ct("ShaderModInstalledOff", { shader_mod: "Iris" }))]),
      mgroup(sh("Vanilla"), null, [tl(ct("VanillaNoLoaderMods", { version: "1.2.5" })), tl(ct("VanillaNoLoaderShaders", { version: "1.2.5" })), tl(ct("VanillaCheckFailedMods", { error: "offline" }), "warn"), tl(ct("VanillaCheckFailedShaders", { error: "offline" }), "warn"), tl(ct("VanillaCheckingMods", { version: "26.3" })), tl(ct("VanillaCheckingShaders", { version: "26.3" }))]),
    ]],
    instSettings: ["Messages \u203a Instance settings", () => [
      mgroup(is("General"), null, [te(null, is("NameMissing")), tl(is("UseAsIcon")), tl(is("UseAsBackground")), tl(is("Import")), tl(is("MemoryUnit"))]),
      mgroup(is("Group"), tx("Automatic sidebar groups."), L(is, "AutoGroupServers AutoGroupFabric AutoGroupForge AutoGroupNeoForge AutoGroupQuilt AutoGroupVanilla AutoGroupImported", (el) => tl(el))),
      mgroup(is("GameAndLoader"), null, [tl(is("LoaderInstallsNextStart", { loader: "Fabric 0.16.9" })), tl(isp("LoaderNextStartModsOff", { loader: "Fabric 0.16.9", count: 1 })), tl(is("SavedNextStart"), "ok")]),
      mgroup(is("Modpack"), null, [tl(is("FromMrpack")), prog(is("DownloadingNewVersion"), 30), tl(is("StopGameFirst"), "warn"), te(null, is("CouldntPrepareUpdate", { error: "offline" }))]),
      mgroup(is("JavaAndMemory"), null, [tl(is("Checking")), tl(is("MojangJava")), tl(is("UseThisJava"))]),
      mgroup(is("Maintenance"), null, [btn(is("Exporting"), { disabled: true }), btn(is("Duplicating"), { disabled: true })]),
      mgroup(is("SharedGameSettings"), tx("Tags and buttons on a shared value."), [h("div", badge(is("NotShared")), " ", badge(is("NotSharedYet")), " ", badge(is("NotInOptions")), " ", badge(is("Shared")), " ", badge(is("ThisInstance"), "blue")),
        h("div", btn(is("ShareAll"), { sm: true }), " ", btn(is("OverrideAll"), { sm: true }), " ", btn(is("Override"), { sm: true }), " ", btn(is("UseShared", { value: "90" }), { sm: true }), " ", gbtn(is("Hide"))),
        h("div", { style: { maxWidth: "300px" } }, field(null, is("Value"))), tl(isp("AndMore", { names: "FOV, Brightness", count: 4 }))]),
      h("div.lx-mgroup", h("h3", is("SharedGameSettings")), sharedOptionsList(ctx0(), true)),
    ]],
    servers: ["Messages \u203a Lodestone Host page", () => [
      mgroup(sv("ConnectAsOwner"), null, [h("div.lx-scard", { style: { maxWidth: "640px" } }, note(sv("ClaimFolderNotFound"), "err"), h("div", btn(sv("ChooseDataFolder")), " ", h("span.lx-muted", sv("ChooseDataFolderPrompt"))), h("div.lx-muted", tx("Lodestone Host \u2014 "), sv("ProductHostHint")), h("div.lx-muted", tx("Lodestone Core \u2014 "), sv("ProductCoreHint"))),
        ...L(sv, "ClaimNoCore ClaimNotThisCore ClaimNoResult ClaimNoCertificate", (el) => te(null, el)), te(null, sv("ClaimFiles", { error: "permission denied" })), te(null, sv("ClaimInvalid", { error: "missing field" })), te(sv("CredentialsReadFailed"), tx("\u2026")), te(sv("CredentialsSaveFailed"), tx("\u2026"))]),
      mgroup(sv("CoreMoreActions", { name: "Peter's Lodestone Core" }), null, [h("div", badge(sv("SignedIn"), "green"), " ", badge(sv("LoaderOther"))),
        ...L(sv, "BusyFetching BusyCreating BusyChecking BusyUpdating BusyStopping", (el) => h("div.lx-muted", spinner(), " ", el)), h("div.lx-muted", spinner(), " ", sv("BusyUpdatingFiles", { done: 3, total: 9 }))]),
      mgroup(sv("ServersTitle"), tx("Messages at the top of the page."), [te(sv("SaveFailed"), tx("\u2026")), toast(sv("NoLongerTrusted", { name: "Alex's server box" }), sv("NoLongerTrustedIdChanged", { address: "192.168.1.42:16662" }), "warn"),
        toast(sv("NoLongerTrusted", { name: "Alex's server box" }), sv("NoLongerTrustedOtherCore", { address: "192.168.1.42:16662" }), "warn"), toast(sv("NoLongerTrusted", { name: "Alex's server box" }), sv("NoLongerTrustedNewCertificate"), "warn"),
        te(sv("KeySaveFailed", { keychain: "Keychain" }), sv("KeySaveFailedError", { keychain: "Keychain", error: "denied" })), tl(sv("CheckCertificate"), "warn"), tl(sv("CertificateNotTrusted"), "warn"), te(sv("CreateFailed"), tx("\u2026")), te(sv("LinkFailed"), tx("\u2026")),
        te(sv("CheckModsFailed"), sv("CoreRemoved", { address: "192.168.1.42:16662" })), te(sv("UpdateModsFailed"), sv("StopGameFirst")), te(sv("UpdateInstanceFailed"), sv("InstanceNotSaved", { error: "disk full" })), te(sv("JoinFailed", { name: "Skyblock" }), sv("BadJoinAddress")),
        te(sv("StartFailed", { name: "Skyblock" }), sv("StartCrashed")), te(sv("StartFailed", { name: "Skyblock" }), sv("StartStopped")), te(sv("StartFailed", { name: "Skyblock" }), sv("StartTimedOut")), te(sv("StopFailed", { name: "Skyblock" }), sv("ServerGone")),
        te(null, sv("UnlinkFailed", { error: "\u2026" })), tl(sv("Cancelled")), toast(sv("ReadyTitle", { name: "Skyblock" }), svp("ReadyBodySkipped", { version: "1.21.1", loader: "Fabric 0.16.14", count: 1 }), "ok")]),
      mgroup(tx("Lodestone Core"), tx("Why a request to a core failed."), [te(null, sv("CoreUnreachable", { detail: "connection refused" })), te(null, sv("CoreUntrusted", { host: "192.168.1.42", computer: "this Mac" })), te(null, sv("CoreCertificateChanged", { host: "192.168.1.42" })),
        ...L(sv, "CoreSignedOut CoreForbidden CoreNotFound CoreTooOld CoreTooManyRequests", (el) => te(null, el)), te(null, sv("CoreAnswered", { status: 500, message: "internal error" })), te(null, sv("CoreInvalidReply", { detail: "expected JSON" }))]),
      mgroup(sv("OnYourNetwork"), null, [tl(sv("LanNoneWindows")), tl(sv("LanNoneOther"))]),
      mgroup(sv("FriendsTitle"), null, [te(null, sv("FriendLinkMissing")), te(null, sv("FriendRemoved", { link: "https://connect.lodestone.cc/j/\u2026" })), te(sv("FriendsReadFailed"), tx("\u2026")), te(sv("FriendsSaveFailed"), tx("\u2026"))]),
      mgroup(fl("Title"), null, [te(fl("ActionFailed"), tx("\u2026"))]),
    ]],
    worlds: ["Messages \u203a Worlds", () => [
      mgroup(wd("Worlds"), null, [tl(wd("StopGameFirst"), "warn"), tl(wd("WaitForTask"), "warn"), tl(wd("Cancelled")),
        ...[wd("RestoringBackup"), wd("ReplacingWorld"), wd("MovingToTrash", { name: "Castle Build", trash: "the Trash" }), wd("MovingBackupToTrash", { trash: "the Trash" }), wd("Renaming"), wd("Exporting", { name: "Castle Build" }), wd("ImportingWorlds")].map((el) => prog(el, 45)),
        tl(wd("RestoredAsCopy", { folder: "Castle Build (2)" }), "ok"), tl(wd("Restored", { folder: "Castle Build" }), "ok"), tl(wd("RestoredWithSafety", { folder: "Castle Build", backup: "Castle Build 2026-11-10.zip", trash: "the Trash" }), "ok"), tl(wd("InTrash", { name: "Castle Build", trash: "the Trash" }), "ok"),
        tl(wd("RenamedTo", { name: "Castle" }), "ok"), tl(wd("RenamedToFolder", { name: "Castle", folder: "Castle (1)" }), "ok"), tl(wd("Exported", { name: "Castle", path: "~/Desktop", size: "117 MB" }), "ok"), tl(wd("Imported", { worlds: "Castle, Island" }), "ok"),
        te(null, wd("FileError", { name: "x.zip", error: "no level.dat" })), te(null, wd("ZipsOnly")), tl(wd("ImportWorldPrompt")), tl(wd("CopiedScreenshot"), "ok"), te(null, wd("CouldntCopy", { error: "\u2026" }))]),
    ]],
    common: ["Messages \u203a Common words and formats", () => [
      mgroup(tx("Common"), tx("Words shared by both apps."), [h("div", L(cm, "Apply Browse Copied Default Edit Loading No None Off On Open Saved Search Menu MoreActions MoveDown MoveUp Yes", (el) => [btn(el, { sm: true }), " "])),
        tl(h("span", cm("ThisComputer"), " \u00b7 ", cm("Keychain"), " \u00b7 ", cm("KeychainInFull"))), te(null, cm("FileDialogUnavailable")), tl(h("span", cmp("Instances", { count: 3 }), " \u00b7 ", cmp("Files", { count: 12 })))]),
      mgroup(tx("Format"), tx("Numbers, sizes and dates."), [tl(h("span", tx("4"), fm("GroupSeparator"), tx("091 \u00b7 1"), fm("DecimalSeparator"), tx("5 \u00b7 Fabric"), fm("ListSeparator"), tx("Quilt"))),
        tl(h("span", fm("Kilobytes", { size: 400 }), " \u00b7 ", fm("Megabytes", { size: 38 }), " \u00b7 ", fm("Terabytes", { size: 1.2 }), " \u00b7 ", fm("CompactThousands", { value: "1.2" }), " \u00b7 ", fm("CompactMillions", { value: "3.4" }))),
        tl(h("span", fm("JustNow"), " \u00b7 ", fm("Today"), " \u00b7 ", fm("Yesterday"), " \u00b7 ", fm("TodayAt", { time: "14:30" }), " \u00b7 ", fm("YesterdayAt", { time: "09:12" }), " \u00b7 ", fm("ShortDate", { month: "Mar", day: 12 }))),
        tl(h("span", L(fm, "January February March April May June July August September October November December", (el) => [el, " "]))),
        tl(h("span", fmp("MinutesAgo", { count: 5 }), " \u00b7 ", fmp("HoursAgo", { count: 2 }), " \u00b7 ", fmp("DaysAgo", { count: 3 })))]),
    ]],
  };
  let ctx0 = () => Mock.ctx;

  const KEYNAMES = "AcBack AcBookmarks AcClose AcExit AcForward AcHome AcNew AcOpen AcPrint AcProperties AcRefresh AcSave AcSearch AcStop Again AlternateErase Apostrophe Application Backslash Backspace Call Cancel CapsLock ChannelDown ChannelUp Clear ClearAgain Comma Copy Crsel CurrencySubunit CurrencyUnit Cut DecimalSeparator Delete Down End EndCall Enter Enter2 Equal Escape Execute Exsel F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24 F25 Find GraveAccent Help Home Insert International1 International2 International3 International4 International5 International6 International7 International8 International9 Keypad0 Keypad00 Keypad000 Keypad1 Keypad2 Keypad3 Keypad4 Keypad5 Keypad6 Keypad7 Keypad8 Keypad9 KeypadA KeypadAdd KeypadAmpersand KeypadAt KeypadB KeypadBackspace KeypadBinary KeypadC KeypadClear KeypadClearEntry KeypadColon KeypadComma KeypadD KeypadDecimal KeypadDivide KeypadDoubleAmpersand KeypadDoubleVerticalBar KeypadE KeypadEnter KeypadEqual KeypadEqualsAs400 KeypadExclamation KeypadF KeypadGreater KeypadHash KeypadHexadecimal KeypadLeftBrace KeypadLeftParenthesis KeypadLess KeypadMemoryAdd KeypadMemoryClear KeypadMemoryDivide KeypadMemoryMultiply KeypadMemoryRecall KeypadMemoryStore KeypadMemorySubtract KeypadMultiply KeypadOctal KeypadPercent KeypadPeriod KeypadPlusMinus KeypadPower KeypadRightBrace KeypadRightParenthesis KeypadSpace KeypadSubtract KeypadTab KeypadVerticalBar KeypadXor Lang1 Lang2 Lang3 Lang4 Lang5 Lang6 Lang7 Lang8 Lang9 Left LeftAlt LeftBracket LeftCommand LeftControl LeftMeta LeftOption LeftShift LeftWin LeftWindows MediaEject MediaFastForward MediaNextTrack MediaPause MediaPlay MediaPlayPause MediaPreviousTrack MediaRecord MediaRewind MediaSelect MediaStop Menu Minus Mode Mute New NumLock Oper Out PageDown PageUp Paste Pause Period Power PrintScreen Prior Right RightAlt RightBracket RightCommand RightControl RightMeta RightOption RightShift RightWin RightWindows Save ScrollLock Select Semicolon Separator Slash Sleep SoftLeft SoftRight Space Stop SysReq Tab ThousandsSeparator Undo Unknown Up VolumeDown VolumeUp Wake World1 World2";
  // ---------- "Other text": every launcher/shared string not placed above ----------
  // Ids this module places: literal helper calls in its own source, plus data-driven ones.
  function placedIds() {
    const out = new Set();
    const re = /\b(shp|sh|svp|sv|flp|fl|ctp|ct|isp|is|imp|im|wdp|wd|sop|so|cmp|cm|fmp|fm)\(\s*"([A-Za-z0-9_]+)"(?=\s*[,)])/g;
    let m; while ((m = re.exec(SRC))) out.add(PFX[m[1]] + m[2]);
    const reL = /\bL\(\s*(\w+)\s*,\s*"([^"]+)"/g;
    while ((m = reL.exec(SRC))) if (PFX[m[1]]) keysOf(m[2]).forEach((k) => out.add(PFX[m[1]] + k));
    Object.values(OPTS).forEach((rows) => rows.forEach(([k, v]) => { out.add(PFX.so + "Option" + k); if (v[0] === "=") out.add(PFX.so + v.slice(1)); if (v[0] === "#" && v.length > 3) out.add(PFX.so + "Binding" + v.slice(1)) }));
    Object.keys(OPTS).forEach((c) => { out.add(PFX.sh + "Category" + c); out.add(PFX.sh + "Category" + c + "Hint"); out.add(PFX.is + "Category" + c); out.add(PFX.is + "Category" + c + "Description") });
    AUTH_ERRORS.forEach((k) => out.add(PFX.sh + k));
    keysOf(KEYNAMES).forEach((k) => out.add(PFX.so + "BindingKeyKeyboard" + k));
    return out;
  }
  function otherText(ctx) {
    const D = window.CopyDesk;
    const all = (D && D.state && D.state.strings) || [];
    const placed = placedIds();
    const rest = all.filter((s) => (s.app === "launcher" || s.app === "shared") && !placed.has(s.id));
    const groups = new Map();
    rest.forEach((s) => { const g = s.file + (s.section ? " \u203a " + s.section : ""); if (!groups.has(g)) groups.set(g, []); groups.get(g).push(s) });
    return msgPage(ctx, sh("Settings"), [
      rest.length ? null : h("p.lx-muted", tx("Every Lodestone Launcher string is placed on a screen of this mock.")),
      ...Array.from(groups.entries()).map(([g, list]) => h("div.lx-mgroup", h("h3", tx(g)), h("div.lx-mlist", list.map((s) => {
        const vars = {}; if (s.forms) vars.count = 2;
        return h("div.lx-toast", h("div.lx-toast-txt", h("div", t(s.id, vars)), h("div.lx-mkey", tx(s.enum + "::" + s.key))));
      })))),
    ], true);
  }
  function msgPage(ctx, title, kids, raw) {
    return h("div.lx-main", h("div.lx-page", { "data-mk-scroll": "lx-msg-" + ctx.route }, raw ? null : null, kids));
  }
  const mgroup = (title, desc, items) => h("div.lx-mgroup", h("h3", title), desc ? h("div.lx-mg-d", desc) : null, h("div.lx-mlist", items));
  const tline = (el, kind) => toast(null, el, kind || "info");
  const terr = (title, body) => toast(title, body, "err");
  function keyNames(ctx) {
    const keys = KEYNAMES;
    return h("div.lx-main", h("div.lx-page", { "data-mk-scroll": "lx-keys" },
      h("h1.lx-h1", sh("SharedGameSettings")), h("div.lx-sub", sh("CategoryControls"), sep(), sh("CategoryControlsHint")),
      mgroup(sh("CategoryControls"), tx("Key names shown as the value of a key binding (Minecraft's own words, from options.txt)."),
        [h("div", L(so, "BindingKeyMouseLeft BindingKeyMouseRight BindingKeyMouseMiddle", (el) => h("span.lx-keycap", el)), h("span.lx-keycap", so("BindingMouse", { number: 4 }))),
          h("div", keysOf(keys).map((k) => h("span.lx-keycap", t(PFX.so + "BindingKeyKeyboard" + k))))]),
      mgroup(sh("CategoryVideo"), null, [h("div", L(so, "ValueFast ValueFancy ValueFabulous ValueCustom ValueDecreased ValueMinimal ValueNarratorAll ValueNarratorChat ValueNarratorSystem ValueCommands ValueHidden ValueNone", (el) => h("span.lx-keycap", el)))])));
  }

  // ---------- registration ----------
  // uses (data-driven keys): sh("Play") sh("TabContent") sh("TabWorlds") sh("Settings") sh("TabLogs") is("General") is("GameAndLoader") is("Modpack") is("LinkedServer") is("JavaAndMemory") is("LaunchOptions") is("SharedGameSettings") is("InstanceSnapshots") is("Maintenance")
  // uses: is("AboutVanilla") is("AboutFabric") is("AboutQuilt") is("AboutForge") is("AboutNeoForge") is("KindRelease") is("KindSnapshot") is("KindBeta") is("KindAlpha") is("KindOther")
  // uses: wd("Survival") wd("Creative") wd("Adventure") wd("Spectator") sv("StateRunning") sv("StateStarting") sv("StateStopping") sv("StateStopped") sv("StateCrashed") sv("StateUnknown") sv("StateHidden") sv("BusyStarting") sv("BusyCreatingFiles")
  // uses: sh("InstanceDefaults") sh("SharedGameSettings") sh("General") sh("Skin") sh("Storage") sh("Accounts") sh("Developer")
  const opts = (s) => keysOf(s).map((v) => [v, v.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^./, (c) => c.toUpperCase())]);
  const ST = (label, s, def) => ({ label, options: opts(s), default: def || keysOf(s)[0] });
  const states = {
    account: ST("Account", "microsoft offline launcher expired signingIn noNetwork signedOut"),
    instances: ST("Instances", "some none importing"),
    game: ST("Game", "stopped preparing launching playing stopping verifying modpackUpdating crashed"),
    update: ST("Launcher update", "none notChecked checking available downloading ready failed dev"),
    build: ST("Build", "dev release"),
    toast: { label: "Notice (toast)", options: opts("none narrator undo redo settingsOf crashed signInExpired addAccount ready shareReady switched verified copiedKey copiedInvite skinCopied firstTime synced copiedValues"), default: "none" },
    groups: ST("Sidebar groups", "on off"),
    logs: ST("Logs tab", "filled empty nomatch"),
    content: ST("Content tab", "loaded loading empty offline vanilla gameRunning installing installed upToDate curseforge searching nothing filtered noFit readFailed"),
    worlds: ST("Worlds tab", "list empty reading gameRunning task done"),
    instSettings: ST("Instance settings", "saved unsaved loaderChanged unlinked javaOld noSnapshots error"),
    modpack: ST("Modpack (instance)", "update checking upToDate offline"),
    linkedCore: ST("Linked server's core", "present removed"),
    localCore: ST("Local core", "claimed unclaimed direct claimFailed scanning none"),
    friends: ST("Friend list", "noName linkWaiting reply pending compare share play list empty refreshing error"),
    friendServers: ST("Friends' servers", "none some"),
    otherCores: ST("Other cores", "some signedIn certificate certChanged notListed noPublic noVisible busy none"),
    lan: ST("On your network", "found looking none failed"),
    hostMsg: ST("Host page message", "none signedOut untrusted readFailed joinFailed notRunning createdWithoutMods connected"),
    relay: ST("Connect service (relay)", "default dev error env"),
    serverConsole: ST("Server page", "live connecting reconnecting empty notAllowed cantWrite eula linked tooOld"),
    signin: { label: "Sign-in dialog", options: opts("start codeStart browser gettingCode code copied signing").concat(AUTH_ERRORS.map((k) => ["err:" + k, "Error: " + k.replace(/^Auth/, "")])), default: "start" },
    crash: ST("Crash dialog", "mods one cause stopped"),
    finder: ST("Mod finder", "intro reading try play ask found notFound refused failed stopped"),
    newinst: ST("New instance dialog", "ok loading failed loaderLoading noLoader noVersion curseforge nothing notModpack pick loadingPack noVersions pack notCode newer tooLarge damaged"),
    shareCode: ST("Share code dialog", "plain pack reading"),
    addServer: ST("Add server dialog", "none address password bad never"),
    join: ST("Join a friend dialog", "paste invalid empty connecting slow reading ready readyMods readyLinked readyInstance readyLocal portTaken starting busy failed noPath stopped ended notSharing wrongHost offline service tunnel"),
    addFriend: ST("Add friend dialog", "start made preview added notLink paste other already needName"),
    importState: ST("Import dialog", "list looking none progress"),
    host: ST("Host on a core dialog", "form errors progress done"),
    advisory: ST("Mods advisory", "new update offline"),
    switchTo: ST("Switch loader dialog", "loader vanilla"),
    unsavedWhere: ST("Unsaved changes dialog", "instance this defaults"),
    bg: ST("Background picker", "image slideshow"),
    storage: ST("Settings \u203a Storage", "counted counting error gameRunning freed"),
    storageWhich: ST("Storage confirm (which)", "fromButton GameFiles Runtimes Cache OldLogs AllLogs Leftovers Snapshots"),
    storageMove: ST("Move data dialog", "confirm back moving failed"),
    settingsUnsaved: ST("Settings \u203a Instance defaults", "no yes error"),
    sharedMain: ST("Shared settings main instance", "set none"),
    cli: ST("Settings \u203a Developer CLI", "none clients closed error"),
  };
  const R = {};
  const page = (title, render, group, defaultParams) => ({ title, render, group: group || "Pages", defaultParams });
  const dlg = (title, render, group, defaultParams) => ({ title, render, dialog: true, group: group || "Dialogs", defaultParams });
  R.home = page("Home (no instance selected)", instancePage, "Pages", { tab: "play" });
  R.instance = page("Instance \u203a Play", instancePage, "Instance", { id: "atm", tab: "play" });
  R["inst-new"] = page("Instance never played", instancePage, "Instance", { id: "surv", tab: "play" });
  R["inst-old"] = page("Old version (Rosetta)", instancePage, "Instance", { id: "old", tab: "play" });
  R["inst-linked"] = page("Instance linked to a server \u203a Play", instancePage, "Instance", { id: "smp", tab: "play" });
  R["inst-content"] = page("Content \u203a Mods", instancePage, "Instance", { id: "surv", tab: "content", kind: "mods" });
  R["inst-rp"] = page("Content \u203a Resource packs", instancePage, "Instance", { id: "surv", tab: "content", kind: "rp" });
  R["inst-shaders"] = page("Content \u203a Shaders", instancePage, "Instance", { id: "surv", tab: "content", kind: "shaders" });
  R["inst-browse"] = page("Content \u203a Modrinth", instancePage, "Instance", { id: "surv", tab: "content", kind: "mods", view: "modrinth" });
  R["inst-curseforge"] = page("Content \u203a CurseForge", instancePage, "Instance", { id: "surv", tab: "content", kind: "mods", view: "curseforge" });
  R["inst-project"] = page("Content \u203a A project's page", instancePage, "Instance", { id: "surv", tab: "content", kind: "mods", view: "project" });
  R["inst-vanilla"] = page("Content on a vanilla instance", instancePage, "Instance", { id: "latest", tab: "content", kind: "mods" });
  R["inst-worlds"] = page("Worlds", instancePage, "Instance", { id: "surv", tab: "worlds" });
  R["inst-shots"] = page("Worlds \u203a Screenshots", instancePage, "Instance", { id: "surv", tab: "worlds", sec: "shots" });
  R["inst-settings"] = page("Settings (instance)", instancePage, "Instance", { id: "atm", tab: "settings" });
  R["inst-settings-linked"] = page("Settings of a linked instance", instancePage, "Instance", { id: "smp", tab: "settings", sec: "linked" });
  R["inst-logs"] = page("Logs", instancePage, "Instance", { id: "atm", tab: "logs" });
  R.host = page("Lodestone Host", hostPage);
  R.server = page("A server's page (console)", serverPage, "Pages", { name: "Survival SMP" });
  SPAGES.forEach(([k, s]) => { R["settings-" + k] = dlg("Settings \u203a " + s.replace(/([a-z])([A-Z])/g, "$1 $2"), settingsDialog, "Settings dialog", { page: k }) });
  R.settings = dlg("Settings (last page)", settingsDialog, "Settings dialog", { page: "general" });
  R.perf = dlg("Performance monitor", DIALOG_SIMPLE.perf, "Settings dialog");
  R.skinEditor = dlg("Skin editor", skinEditorDialog, "Settings dialog");
  R.storageConfirm = dlg("Storage: remove/clear confirm", DIALOG_SIMPLE.storageConfirm, "Settings dialog", { which: "GameFiles" });
  R.storageMove = dlg("Storage: move data", DIALOG_SIMPLE.storageMove, "Settings dialog");
  R.dataFolderMissing = dlg("Data folder missing (at start)", DIALOG_SIMPLE.dataFolderMissing, "Settings dialog");
  R.unsaved = dlg("Save your changes?", DIALOG_SIMPLE.unsaved, "Settings dialog");
  R.signin = dlg("Sign in with Microsoft", signInDialog, "Accounts & sign-in");
  R.stopSignIn = dlg("Stop signing in?", DIALOG_SIMPLE.stopSignIn, "Accounts & sign-in");
  R.offlineAccount = dlg("Add offline account", DIALOG_SIMPLE.offlineAccount, "Accounts & sign-in");
  R.removeAccount = dlg("Remove account", DIALOG_SIMPLE.removeAccount, "Accounts & sign-in");
  R.launcherIdentity = dlg("Sign in as the Minecraft Launcher?", DIALOG_SIMPLE.launcherIdentity, "Accounts & sign-in");
  R.turnOffLauncher = dlg("Turn off Launcher sign-in", DIALOG_SIMPLE.turnOffLauncher, "Accounts & sign-in");
  R.newInstance = dlg("New instance \u203a Minecraft version", newInstanceDialog, "Instance dialogs", { mode: "version" });
  R["newInstance-fabric"] = dlg("New instance \u203a Fabric + version list", newInstanceDialog, "Instance dialogs", { mode: "version", loader: "fabric", list: true });
  R["newInstance-modpack"] = dlg("New instance \u203a Modpack", newInstanceDialog, "Instance dialogs", { mode: "modpack" });
  R["newInstance-code"] = dlg("New instance \u203a Share code", newInstanceDialog, "Instance dialogs", { mode: "code" });
  R.import = dlg("Import from another launcher", DIALOG_SIMPLE.import, "Instance dialogs");
  R.stopImporting = dlg("Stop importing?", DIALOG_SIMPLE.stopImporting, "Instance dialogs");
  R.crash = dlg("Crash dialog", crashDialog, "Instance dialogs");
  R.modFinder = dlg("Find the mod that crashes", modFinderDialog, "Instance dialogs");
  R.finderInterrupted = dlg("Finish finding the mod?", DIALOG_SIMPLE.finderInterrupted, "Instance dialogs");
  R.deleteInstance = dlg("Delete instance", DIALOG_SIMPLE.deleteInstance, "Instance dialogs");
  R.deleteWorld = dlg("Delete world", DIALOG_SIMPLE.deleteWorld, "Instance dialogs");
  R.replaceWorld = dlg("Replace world with backup", DIALOG_SIMPLE.replaceWorld, "Instance dialogs");
  R.screenshot = dlg("Screenshot viewer", DIALOG_SIMPLE.screenshot, "Instance dialogs");
  R.switchLoader = dlg("Switch loader", DIALOG_SIMPLE.switchLoader, "Instance dialogs");
  R.mainOffer = dlg("Make it your main instance?", DIALOG_SIMPLE.mainOffer, "Instance dialogs");
  R.mainSwitch = dlg("Switch main instance", DIALOG_SIMPLE.mainSwitch, "Instance dialogs");
  R.modpackUpdate = dlg("Update modpack", DIALOG_SIMPLE.modpackUpdate, "Instance dialogs");
  R.shareCode = dlg("Share code", shareCodeDialog, "Instance dialogs");
  R.snapshotTake = dlg("Take a snapshot", DIALOG_SIMPLE.snapshotTake, "Instance dialogs");
  R.snapshotRevert = dlg("Revert to a snapshot", DIALOG_SIMPLE.snapshotRevert, "Instance dialogs");
  R.snapshotShare = dlg("Share a snapshot", DIALOG_SIMPLE.snapshotShare, "Instance dialogs");
  R.snapshotDelete = dlg("Delete a snapshot", DIALOG_SIMPLE.snapshotDelete, "Instance dialogs");
  R.snapshotImport = dlg("New instance from a snapshot", DIALOG_SIMPLE.snapshotImport, "Instance dialogs");
  R.iconPicker = dlg("Icon picker", DIALOG_SIMPLE.iconPicker, "Instance dialogs");
  R.backgroundPicker = dlg("Play background picker", DIALOG_SIMPLE.backgroundPicker, "Instance dialogs");
  R.hostOnCore = dlg("Host on a Lodestone core", DIALOG_SIMPLE.hostOnCore, "Instance dialogs");
  R.addServer = dlg("Add server", addServerDialog, "Lodestone Host dialogs");
  R["addServer-signin"] = dlg("Sign in to server", addServerDialog, "Lodestone Host dialogs", { signin: true });
  R.trust = dlg("Do you trust this core?", DIALOG_SIMPLE.trust, "Lodestone Host dialogs");
  R.certificate = dlg("Trust certificate", DIALOG_SIMPLE.certificate, "Lodestone Host dialogs");
  R["certificate-changed"] = dlg("Certificate changed", DIALOG_SIMPLE.certificate, "Lodestone Host dialogs", { changed: true });
  R.advisory = dlg("Server mods will run here", DIALOG_SIMPLE.advisory, "Lodestone Host dialogs");
  R.differentMods = dlg("Server has different mods", DIALOG_SIMPLE.differentMods, "Lodestone Host dialogs");
  R.joinLan = dlg("Join a LAN world", DIALOG_SIMPLE.joinLan, "Lodestone Host dialogs");
  R.joinFriend = dlg("Join a friend's server", joinFriendDialog, "Lodestone Host dialogs");
  R.addFriend = dlg("Add a friend / friend link", addFriendDialog, "Lodestone Host dialogs");
  R.friendName = dlg("Your name", DIALOG_SIMPLE.friendName, "Lodestone Host dialogs");
  R.verify = dlg("Compare safety codes", DIALOG_SIMPLE.verify, "Lodestone Host dialogs");
  R.renameFriend = dlg("Rename a friend", DIALOG_SIMPLE.renameFriend, "Lodestone Host dialogs");
  R.retire = dlg("Retire friend identity", DIALOG_SIMPLE.retire, "Lodestone Host dialogs");
  R.retireCore = dlg("Retire a core", DIALOG_SIMPLE.retireCore, "Lodestone Host dialogs");
  R.removeFriend = dlg("Remove friend", DIALOG_SIMPLE.removeFriend, "Lodestone Host dialogs");
  Object.entries(MSG_PAGES).forEach(([k, [title, fn]]) => { R["msg-" + k] = page(title, (ctx) => h("div.lx-main", h("div.lx-page", { "data-mk-scroll": "lx-msg-" + k }, fn(ctx))), "Messages") });
  R["msg-keys"] = page("Key names (shared settings)", keyNames, "Messages");
  R["msg-other"] = page("Other text not on a screen", otherText, "Messages");

  Mock.register("launcher", { name: "Lodestone Launcher", size: [1280, 800], initial: "instance", states, routes: R, frame });
})();
