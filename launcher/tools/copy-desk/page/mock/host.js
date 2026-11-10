// Mock of Lodestone Host (launcher/crates/lodestone-host), for the Copy Desk.
// Every piece of UI text goes through t() (string ids from strings.json); sample data
// (server names, versions, players, files, log lines) goes through tx().
(function () {
  const { h, tx } = Mock;
  const X = Mock.ctx;

  // ---------------------------------------------------------------- strings
  // tt(id, vars): like t(), but a var may be an element (another editable string); it
  // is put in the placeholder's place as a non-editable slot.
  function tt(id, vars) {
    if (!vars) return Mock.t(id);
    const plain = {}; const slots = {};
    for (const [k, v] of Object.entries(vars)) {
      if (v instanceof Node) { slots[k] = v; plain[k] = k } else plain[k] = v;
    }
    const el = Mock.t(id, plain);
    for (const [k, node] of Object.entries(slots)) {
      const chip = el.querySelector && el.querySelector(`.mk-chip[data-ph="{${k}}"]`);
      if (chip) {
        const slot = h("span.hx-slot", { contenteditable: "false" }, node);
        slot.dataset.ph = "{" + k + "}";
        chip.replaceWith(slot);
      }
    }
    return el;
  }
  const S = (k, v) => tt("host/shell/ShellMsg/" + k, v);
  const C = (k, v) => tt("host/core/CoreMsg/" + k, v);
  const CP = (k, v) => tt("host/core/CorePlural/" + k, v);
  const I = (k, v) => tt("host/instance/InstanceMsg/" + k, v);
  const IP = (k, v) => tt("host/instance/InstancePlural/" + k, v);
  const N = (k, v) => tt("host/instances/InstancesMsg/" + k, v);
  const NP = (k, v) => tt("host/instances/InstancesPlural/" + k, v);
  const F = (k, v) => tt("host/console_files/ConsoleFilesMsg/" + k, v);
  const FP = (k, v) => tt("host/console_files/ConsoleFilesPlural/" + k, v);
  const M = (k, v) => tt("shared/common/CommonMsg/" + k, v);
  const MP = (k, v) => tt("shared/common/CommonPlural/" + k, v);
  const FM = (k, v) => tt("shared/format/FormatMsg/" + k, v);
  const FMP = (k, v) => tt("shared/format/FormatPlural/" + k, v);

  // ---------------------------------------------------------------- css
  const css = `
.hx-root,.hx-dlg{--g900:#1D1E21;--g875:#212327;--g850:#26282C;--g800:#2B2D32;--g750:#303338;--g700:#36393F;--g600:#44464B;--g550:#67686A;--g500:#767A82;--g400:#A5A5AC;--g300:#E3E3E4;
 --blue:#1D8EB2;--blue2:#59B2F3;--blue1:#69C9FF;--bluef:rgba(60,133,186,.28);--green:#6DD277;--greenf:rgba(97,174,50,.22);--red:#CF4545;--red2:#FF5C5C;--redf:rgba(174,50,50,.25);--yellow:#EFB440;--yellowf:rgba(174,139,50,.25)}
.hx-root{position:relative;width:100%;height:100%;display:flex;flex-direction:column;background:var(--g800);color:var(--g300);font-family:Satoshi,system-ui,sans-serif;font-size:14px;line-height:1.4;overflow:hidden;-webkit-font-smoothing:antialiased}
.hx-root *{box-sizing:border-box}
.hx-top{height:48px;flex:none;display:flex;align-items:center;justify-content:center;position:relative;background:var(--g800);border-bottom:1px solid var(--g700)}
.hx-top.hx-devline{border-bottom-color:#8a7350}
.hx-logo{display:flex;align-items:center;gap:6px;font-family:"Clash Grotesk",sans-serif;font-weight:600;font-size:22px;letter-spacing:-.02em;color:var(--g300)}
.hx-logomark{width:18px;height:18px;border-radius:3px;background:linear-gradient(135deg,#2EF29A,#1FBFA8);position:relative}
.hx-logomark:after{content:"";position:absolute;left:5px;top:4px;width:8px;height:8px;background:var(--g800);border-radius:1px}
.hx-dev{margin-left:16px;background:var(--yellow);color:#2a2000;font-weight:700;font-size:13px;padding:1px 7px;border-radius:5px}
.hx-bell{position:absolute;right:16px;top:9px;width:30px;height:30px;border:1px solid var(--g600);border-radius:6px;display:flex;align-items:center;justify-content:center;color:var(--g400);cursor:pointer;background:var(--g850)}
.hx-bell .hx-badge{position:absolute;top:-4px;right:-4px;background:var(--red2);color:#fff;border-radius:8px;font-size:10px;padding:0 4px;font-weight:700}
.hx-banner{flex:none;display:flex;align-items:center;justify-content:center;gap:12px;padding:6px 16px;background:rgba(239,180,64,.15);border-bottom:1px solid rgba(239,180,64,.4);font-size:13.5px}
.hx-banner.hx-red{background:rgba(207,69,69,.15);border-bottom-color:rgba(207,69,69,.4)}
.hx-body{flex:1;display:flex;min-height:0}
.hx-side{width:254px;flex:none;background:var(--g850);border-right:1px solid var(--g750);display:flex;flex-direction:column;min-height:0}
.hx-sidescroll{flex:1;overflow:auto;padding:8px}
.hx-cap{font-size:12px;text-transform:uppercase;color:var(--g500);margin:12px 8px 6px;letter-spacing:.01em}
.hx-instcard{border:1px solid var(--g750);background:var(--g800);border-radius:8px;padding:10px 12px;display:flex;flex-direction:column;gap:6px}
.hx-instcard .hx-name{display:flex;gap:8px;align-items:center;font-size:17px;font-weight:500}
.hx-meta{display:flex;align-items:center;gap:6px;color:var(--g500);font-size:13px}
.hx-dot{width:7px;height:7px;border-radius:50%;background:var(--g500);flex:none;display:inline-block}
.hx-dot.hx-run{background:var(--green)}.hx-dot.hx-err{background:var(--red2)}.hx-dot.hx-warn{background:var(--yellow)}.hx-dot.hx-off{background:var(--g500)}
.hx-chipaddr{display:inline-flex;align-items:center;gap:6px;align-self:flex-start;background:var(--bluef);color:var(--blue2);font-family:"JetBrains Mono",monospace;font-size:13px;border-radius:5px;padding:2px 7px;cursor:pointer}
.hx-power{display:flex;gap:6px;flex-wrap:wrap}
.hx-empty-sel{border:1px dashed var(--g600);border-radius:8px;height:440px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;color:var(--g500)}
.hx-row{display:flex;align-items:center;gap:9px;padding:7px 10px;border-radius:7px;cursor:pointer;color:var(--g300);font-size:15px;border:1px solid transparent}
.hx-row:hover{background:var(--g750)}
.hx-row.hx-sel{background:var(--g750);border-color:var(--g700)}
.hx-row .hx-ico{color:var(--g500)}
.hx-row.hx-sel .hx-ico{color:var(--g300)}
.hx-row.hx-dis{color:var(--g500);cursor:default}
.hx-row .hx-grow{flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.hx-muted{color:var(--g500)}.hx-faint{color:var(--g550)}.hx-italic{font-style:italic}
.hx-small{font-size:12.5px}.hx-tiny{font-size:11.5px}
.hx-foot{flex:none;border-top:1px solid var(--g750);padding:8px;display:flex;flex-direction:column;gap:2px;position:relative}
.hx-conn{display:flex;align-items:center;gap:8px;padding:4px 8px;font-size:15px;cursor:default}
.hx-page{flex:1;overflow:auto;min-width:0;position:relative}
.hx-pagein{padding:44px 48px 48px;max-width:1100px}
.hx-h1{font-family:Chillax,sans-serif;font-weight:500;font-size:38px;letter-spacing:.005em;margin:0 0 18px;display:flex;align-items:center;gap:14px}
.hx-h2{font-weight:700;font-size:21px;margin:0}
.hx-h3{font-family:Chillax,sans-serif;font-weight:500;font-size:18px;margin:0}
.hx-lead{color:var(--g500);font-style:italic;font-size:16px;margin:4px 0 16px}
.hx-sublead{color:var(--g500);font-size:13.5px;margin:3px 0 12px}
.hx-sec{margin-top:36px}
.hx-pills{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.hx-pill{display:inline-flex;align-items:center;gap:6px;padding:3px 10px;border-radius:6px;font-size:15.5px;background:var(--bluef);color:var(--blue2);font-weight:500}
.hx-pill.hx-grey{background:rgba(165,165,172,.15);color:var(--g400)}
.hx-pill.hx-green{background:var(--greenf);color:var(--green)}
.hx-pill.hx-red{background:var(--redf);color:var(--red2)}
.hx-pill.hx-yellow{background:var(--yellowf);color:var(--yellow)}
.hx-pill.hx-mono{font-family:"JetBrains Mono",monospace;font-size:14px}
.hx-pill.hx-sm{font-size:13px;padding:1px 8px}
.hx-tag{display:inline-flex;align-items:center;gap:4px;border:1px solid var(--g600);border-radius:5px;font-size:11.5px;padding:0 6px;color:var(--g400);white-space:nowrap}
.hx-tag.hx-green{border-color:rgba(109,210,119,.6);color:var(--green)}
.hx-tag.hx-yellow{border-color:rgba(239,180,64,.6);color:var(--yellow)}
.hx-tag.hx-red{border-color:rgba(255,92,92,.6);color:var(--red2)}
.hx-tag.hx-blue{border-color:rgba(89,178,243,.6);color:var(--blue2)}
.hx-btn{display:inline-flex;align-items:center;justify-content:center;gap:6px;padding:5px 11px;border-radius:5px;border:1px solid var(--g550);background:var(--g800);color:var(--g300);font-size:14px;cursor:pointer;white-space:nowrap;user-select:none;line-height:1.3}
.hx-btn:hover{background:var(--g700)}
.hx-btn.hx-primary{background:var(--blue);border-color:var(--blue);color:#fff}
.hx-btn.hx-primary:hover{background:#037AA0}
.hx-btn.hx-danger{color:var(--red2);border-color:var(--g550)}
.hx-btn.hx-dangerfill{background:var(--red);border-color:var(--red);color:#fff}
.hx-btn.hx-ghost{border-color:transparent;background:transparent}
.hx-btn.hx-sm{padding:2px 7px;font-size:13px}
.hx-btn.hx-lg{padding:7px 14px;font-size:15.5px}
.hx-btn.hx-dis{opacity:.55;cursor:default}
.hx-link{color:var(--blue2);cursor:pointer;text-decoration:none}
.hx-link.hx-u{text-decoration:underline}
.hx-card{background:var(--g850);border:1px solid var(--g750);border-radius:10px}
.hx-pad{padding:18px 20px}
.hx-rows>.hx-srow+.hx-srow{border-top:1px solid var(--g750)}
.hx-srow{display:flex;align-items:center;gap:20px;padding:14px 20px;min-height:58px}
.hx-srow .hx-st{flex:1;min-width:0}
.hx-srow .hx-stt{font-size:15.5px;font-weight:500;display:flex;align-items:center;gap:8px}
.hx-srow .hx-sth{color:var(--g500);font-size:13.5px;margin-top:2px}
.hx-srow .hx-sc{flex:none;display:flex;align-items:center;gap:8px}
.hx-srow.hx-col{flex-direction:column;align-items:stretch;gap:8px}
.hx-input{background:var(--g900);border:1px solid var(--g600);border-radius:6px;padding:8px 12px;min-height:38px;display:flex;align-items:center;gap:8px;color:var(--g300);font-size:15px;min-width:0;overflow:hidden;white-space:nowrap}
.hx-input .hx-ph{color:var(--g550)}
.hx-input.hx-mono{font-family:"JetBrains Mono",monospace;font-size:14px}
.hx-input.hx-focus{border-color:var(--blue);box-shadow:0 0 0 2px rgba(29,142,178,.35)}
.hx-input.hx-err{border-color:var(--red2)}
.hx-select{justify-content:space-between}
.hx-w300{width:300px}.hx-w360{width:356px}.hx-w240{width:240px}.hx-wfull{width:100%}
.hx-field{display:flex;flex-direction:column;gap:6px}
.hx-label{font-size:14.5px;color:var(--g400)}
.hx-err-t{color:var(--red2);font-size:13.5px}
.hx-ok-t{color:var(--green);font-size:13.5px}
.hx-warn-t{color:var(--yellow);font-size:13.5px}
.hx-toggle{width:52px;height:28px;border-radius:14px;background:var(--g550);position:relative;flex:none;cursor:pointer}
.hx-toggle:after{content:"";position:absolute;width:22px;height:22px;border-radius:50%;background:#fff;top:3px;left:3px;transition:left .15s}
.hx-toggle.hx-on{background:#3DAE5E}
.hx-toggle.hx-on:after{left:27px}
.hx-toggle.hx-dis{opacity:.45;cursor:default}
.hx-check{display:inline-flex;align-items:center;gap:9px;cursor:pointer}
.hx-box{width:18px;height:18px;border-radius:4px;border:1.5px solid var(--g500);flex:none;display:flex;align-items:center;justify-content:center}
.hx-box.hx-on{background:var(--blue);border-color:var(--blue);color:#fff}
.hx-flex{display:flex;align-items:center;gap:10px}
.hx-between{display:flex;align-items:center;justify-content:space-between;gap:12px}
.hx-col{display:flex;flex-direction:column;gap:10px}
.hx-gap4{gap:4px}.hx-gap16{gap:16px}
.hx-grow{flex:1;min-width:0}
.hx-mono{font-family:"JetBrains Mono",monospace}
.hx-hr{height:1px;background:var(--g700);margin:18px 0}
.hx-chart{background:var(--g900);border-radius:10px;padding:14px;height:230px;display:flex;position:relative}
.hx-chart .hx-plot{flex:1;border:1.5px solid var(--g500);position:relative;background-image:linear-gradient(var(--g600) 1px,transparent 1px),linear-gradient(90deg,var(--g600) 1px,transparent 1px);background-size:100% 25%,8.33% 100%}
.hx-chart .hx-axis{width:44px;display:flex;flex-direction:column;justify-content:space-between;font-family:"JetBrains Mono",monospace;font-size:12.5px;color:var(--g400);padding-bottom:20px}
.hx-chart .hx-xaxis{position:absolute;left:58px;right:14px;bottom:-2px;display:flex;justify-content:space-between;font-family:"JetBrains Mono",monospace;font-size:12.5px;color:var(--g400)}
.hx-loadbox{background:var(--g850);flex:1;border-radius:6px;display:flex;align-items:center;justify-content:center;gap:10px;color:var(--g500)}
.hx-dots{display:inline-flex;gap:3px}.hx-dots i{width:5px;height:5px;border-radius:50%;background:var(--g500);display:block}.hx-dots i:nth-child(2){background:var(--g400)}
.hx-dashed{border:1.5px dashed var(--g700);border-radius:10px;padding:14px 16px;display:flex;align-items:center;gap:10px;color:var(--g500)}
.hx-console{background:var(--g900);border:1px solid var(--g700);border-radius:10px;display:flex;flex-direction:column;height:560px;position:relative}
.hx-log{flex:1;overflow:auto;padding:14px 18px;font-family:"JetBrains Mono",monospace;font-size:13.5px;line-height:1.65;white-space:pre-wrap}
.hx-log .w{color:var(--yellow)}.hx-log .e{color:var(--red2)}.hx-log .g{color:var(--green)}
.hx-usertag{font-family:"JetBrains Mono",monospace;font-size:12px;background:var(--g750);color:var(--g400);border-radius:4px;padding:1px 6px;margin-left:8px}
.hx-cmd{margin:10px;}
.hx-files{display:flex;border:1px solid var(--g600);border-radius:8px;height:560px;overflow:hidden;position:relative}
.hx-flist{width:384px;flex:none;border-right:2px solid var(--g700);overflow:auto}
.hx-frow{display:flex;align-items:center;gap:14px;padding:12px 24px;border-bottom:1px solid var(--g700);font-size:15.5px;cursor:pointer}
.hx-frow.hx-sel{background:var(--g750)}.hx-frow.hx-open{background:var(--g900);font-style:italic}
.hx-editor{flex:1;display:flex;flex-direction:column;min-width:0;background:var(--g850)}
.hx-code{flex:1;display:flex;font-family:"JetBrains Mono",monospace;font-size:14px;line-height:1.75}
.hx-gutter{width:50px;background:var(--g900);color:var(--g500);text-align:right;padding:4px 14px 0 0}
.hx-findbar{padding:6px 8px;border-bottom:1px solid var(--g700);display:flex;flex-direction:column;gap:6px;background:var(--g850)}
.hx-menu{position:absolute;background:var(--g900);border:1px solid var(--g600);border-radius:6px;min-width:220px;box-shadow:0 8px 30px rgba(0,0,0,.4);z-index:20;padding:4px 0;font-size:14.5px}
.hx-menu .hx-mi{padding:8px 16px;cursor:pointer}
.hx-menu .hx-mi:hover{background:var(--g750)}
.hx-menu .hx-mi.hx-dis{color:var(--g500)}
.hx-menu .hx-mi.hx-red{color:var(--red2)}
.hx-menu .hx-sep{height:1px;background:var(--g600);margin:4px 0}
.hx-tabs{display:flex;gap:20px;border-bottom:2px solid var(--g700);margin-bottom:22px}
.hx-tab{padding:4px 0 6px;font-size:18px;color:var(--g500);cursor:pointer;margin-bottom:-2px}
.hx-tab.hx-on{color:var(--blue2);border-bottom:2px solid var(--blue2)}
.hx-modrow{display:flex;align-items:center;gap:14px;padding:12px 18px}
.hx-modicon{width:40px;height:40px;border-radius:8px;flex:none;background:linear-gradient(135deg,#3b6,#275);display:flex;align-items:center;justify-content:center;font-weight:700;color:#fff}
.hx-gameic{width:20px;height:20px;flex:none;border-radius:3px;display:inline-block;position:relative;overflow:hidden}
.hx-gameic.vanilla{background:linear-gradient(#5bb539 0 38%,#8b5a2b 38%)}
.hx-gameic.fabric{background:linear-gradient(135deg,#e7d6b0,#b9a37a);transform:rotate(-20deg) scale(.85);border-radius:2px}
.hx-gameic.paper{background:#ddd;clip-path:polygon(0 30%,100% 0,70% 100%,45% 60%)}
.hx-gameic.forge{background:#1d2b55}
.hx-gameic.modpack{background:linear-gradient(135deg,#e7d6b0,#b9a37a);transform:rotate(-20deg) scale(.85)}
.hx-gameic.big{width:28px;height:28px}
.hx-snav{width:230px;flex:none;display:flex;flex-direction:column;gap:2px;position:sticky;top:0;align-self:flex-start}
.hx-snav .hx-si{padding:9px 13px;border-radius:7px;color:var(--g500);font-size:16.5px;cursor:pointer}
.hx-snav .hx-si.hx-on{background:var(--g850);color:var(--g300)}
.hx-scontent{flex:1;min-width:0;display:flex;flex-direction:column;gap:34px}
.hx-shead .hx-h3{font-size:19px}
.hx-backdropin{position:absolute;inset:0;display:flex;align-items:center;justify-content:center}
.hx-dlg{background:var(--g850);border:1px solid var(--g600);border-radius:14px;box-shadow:0 20px 60px rgba(0,0,0,.5);position:relative;color:var(--g300);font-family:Satoshi,system-ui,sans-serif;font-size:14px;line-height:1.4;max-height:760px;display:flex;flex-direction:column}
.hx-dlg *{box-sizing:border-box}
.hx-dlg .hx-x{position:absolute;right:22px;top:22px;width:34px;height:34px;border:1px solid var(--g550);border-radius:5px;display:flex;align-items:center;justify-content:center;color:var(--g400);cursor:pointer;background:var(--g850);z-index:2}
.hx-dlgbody{padding:40px 58px 34px;overflow:auto;display:flex;flex-direction:column;gap:14px}
.hx-dlgtitle{font-family:Chillax,sans-serif;font-weight:600;font-size:30px;line-height:1.2;margin:0 50px 6px 0}
.hx-dlgfoot{display:flex;gap:10px;margin-top:6px}
.hx-settings{width:1180px;height:740px;flex-direction:row;overflow:hidden}
.hx-snavd{width:276px;flex:none;border-right:1px solid var(--g700);padding:28px 16px 16px;display:flex;flex-direction:column;gap:2px}
.hx-avatar{width:42px;height:42px;border-radius:50%;background:#EFA93A;color:#fff;font-weight:700;display:flex;align-items:center;justify-content:center;font-size:18px;flex:none}
.hx-avatar.hx-sm{width:36px;height:36px;font-size:15px}
.hx-avatar.hx-violet{background:#9b4d97}
.hx-sgcap{font-size:13.5px;color:var(--g400);margin:22px 12px 8px}
.hx-snavd .hx-row{font-size:18px;padding:9px 12px}
.hx-spage{flex:1;overflow:auto;padding:38px 52px 40px}
.hx-spage .hx-h2t{font-family:Chillax,sans-serif;font-weight:500;font-size:24px;margin:0 0 20px}
.hx-create{width:1000px;height:740px;flex-direction:row;overflow:hidden}
.hx-steps{width:180px;flex:none;border-right:1px solid var(--g700);padding:46px 10px;display:flex;flex-direction:column}
.hx-step{display:flex;align-items:flex-start;gap:10px;padding:8px 10px;border-radius:8px;font-size:14px;color:var(--g400)}
.hx-step.hx-on{background:var(--g800);color:var(--g300);font-weight:500}
.hx-stepn{width:22px;height:22px;border-radius:50%;border:1.5px solid var(--g500);display:flex;align-items:center;justify-content:center;font-size:11.5px;flex:none}
.hx-step.hx-on .hx-stepn{background:var(--blue);border-color:var(--blue);color:#fff}
.hx-step.hx-done .hx-stepn{background:#3DAE5E;border-color:#3DAE5E;color:#fff}
.hx-stepline{width:1.5px;height:16px;background:var(--g500);margin-left:21px}
.hx-cmain{flex:1;padding:50px 40px 34px;display:flex;flex-direction:column;min-width:0}
.hx-cmain .hx-ct{font-size:24px;font-weight:700;margin:0}
.hx-cscroll{flex:1;overflow:auto;margin-top:18px;display:flex;flex-direction:column;gap:14px}
.hx-gamecard{border:1px solid var(--g600);border-radius:6px;padding:18px 20px;cursor:pointer;display:flex;flex-direction:column;gap:10px;min-height:130px}
.hx-gamecard.hx-on{border-color:var(--blue);box-shadow:0 0 0 1px var(--blue)}
.hx-gamecard .hx-gt{display:flex;gap:10px;align-items:center;font-size:18px;font-weight:500}
.hx-grid2{display:grid;grid-template-columns:1fr 1fr;gap:20px}
.hx-result{border:1px solid var(--g600);border-radius:6px;padding:14px 16px;display:flex;gap:14px;cursor:pointer}
.hx-result.hx-on{border-color:var(--blue)}
.hx-toasts{position:absolute;right:20px;bottom:20px;display:flex;flex-direction:column;gap:10px;width:420px;z-index:30}
.hx-toast{display:flex;align-items:flex-start;gap:12px;background:var(--g800);border:1px solid var(--g600);border-radius:7px;padding:12px 14px;font-size:15px;box-shadow:0 6px 20px rgba(0,0,0,.3)}
.hx-toast .hx-grow{padding-top:1px}
.hx-tic{width:20px;height:20px;border-radius:50%;flex:none;display:flex;align-items:center;justify-content:center;color:#fff;font-size:12px;font-weight:700}
.hx-tic.info{background:var(--g500)}.hx-tic.ok{background:#3DAE5E}.hx-tic.warn{background:transparent;color:var(--yellow)}.hx-tic.err{background:var(--red2)}
.hx-notif{position:absolute;right:16px;top:56px;width:620px;background:var(--g800);border:1px solid var(--g600);border-radius:14px;box-shadow:0 12px 40px rgba(0,0,0,.45);padding:22px;z-index:25;display:flex;flex-direction:column;gap:12px;max-height:620px;overflow:auto}
.hx-nitem{display:flex;gap:10px;align-items:flex-start;padding:8px 0;border-bottom:1px solid var(--g700)}
.hx-tipwrap{position:relative;display:inline-flex}
.hx-tip{position:absolute;top:calc(100% + 6px);left:0;background:var(--g900);border:1px solid var(--g600);border-radius:6px;padding:6px 9px;font-size:12.5px;color:var(--g300);white-space:pre-line;z-index:40;box-shadow:0 6px 18px rgba(0,0,0,.4);min-width:140px;max-width:340px;font-style:normal;font-weight:400;text-transform:none;font-family:Satoshi,sans-serif}
.hx-tip.hx-up{top:auto;bottom:calc(100% + 6px)}
.hx-tip.hx-right{left:auto;right:0}
.hx-bare{flex:1;display:flex;align-items:center;justify-content:center;background:var(--g900)}
.hx-logincard{width:500px;background:var(--g850);border:1px solid var(--g700);border-radius:14px;padding:32px 32px;display:flex;flex-direction:column;gap:16px}
.hx-logbox{background:var(--g900);border-radius:6px;padding:10px 12px;font-family:"JetBrains Mono",monospace;font-size:12px;color:var(--g400);max-height:150px;overflow:auto;white-space:pre-wrap}
.hx-home{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;text-align:center}
.hx-cube{width:66px;height:74px;background:#6c6e73;clip-path:polygon(50% 0,100% 25%,100% 75%,50% 100%,0 75%,0 25%);position:relative}
.hx-cube:after{content:"";position:absolute;right:16px;bottom:18px;width:20px;height:24px;background:var(--g800);clip-path:polygon(0 25%,100% 0,100% 75%,0 100%)}
.hx-hometitle{font-family:Chillax,sans-serif;font-weight:500;font-size:34px}
.hx-msgpage .hx-msgsec{margin-bottom:26px}
.hx-msgpage .hx-msglist{display:flex;flex-direction:column;gap:8px;align-items:flex-start}
.hx-msgpage .hx-toast{width:560px;box-shadow:none}
.hx-inlineerr{background:var(--g850);border:1px solid var(--g750);border-radius:8px;padding:14px 18px;color:var(--red2);width:100%}
.hx-note{border-radius:8px;padding:12px 16px;font-size:13.5px;background:var(--g850);border:1px solid var(--g750)}
.hx-note.hx-yellow{background:rgba(239,180,64,.10);border-color:rgba(239,180,64,.35)}
.hx-note.hx-red{background:rgba(207,69,69,.10);border-color:rgba(207,69,69,.35)}
.hx-confirm{background:rgba(174,139,50,.18)}
.hx-kv{display:grid;grid-template-columns:220px 1fr;gap:10px 20px;font-size:14.5px}
.hx-kv>:nth-child(odd){color:var(--g500)}
.hx-slot{display:inline}
.hx-tray{width:260px;background:#2f3136;border:1px solid #555;border-radius:8px;padding:5px 0;box-shadow:0 8px 24px rgba(0,0,0,.4)}
.hx-tray .hx-mi{padding:5px 14px;font-size:14px}
.hx-perm{display:flex;gap:16px;align-items:flex-start;padding:12px 20px}
`;

  // ---------------------------------------------------------------- icons
  const P = {
    chart: '<path d="M2 2v12h12" /><path d="M4.5 10.5l3-3 2 2 4-4.5" />',
    gear: '<circle cx="8" cy="8" r="2.2"/><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"/>',
    server: '<rect x="2" y="2.5" width="12" height="4.5" rx="1"/><rect x="2" y="9" width="12" height="4.5" rx="1"/><path d="M4.5 4.75h1M4.5 11.25h1"/>',
    folder: '<path d="M1.5 4.5a1 1 0 011-1h3.5l1.5 1.5h6a1 1 0 011 1v6.5a1 1 0 01-1 1h-11a1 1 0 01-1-1z" fill="currentColor"/>',
    puzzle: '<path d="M3 5h2.5a1.5 1.5 0 113 0H11v2.5a1.5 1.5 0 110 3V13H3z" fill="currentColor"/>',
    code: '<path d="M5.5 4L2 8l3.5 4M10.5 4L14 8l-3.5 4"/>',
    inbox: '<path d="M2 9l1.5-6h9L14 9v4H2z"/><path d="M2 9h3.5l1 1.5h3l1-1.5H14"/>',
    plusSq: '<rect x="2" y="2" width="12" height="12" rx="2" fill="currentColor" opacity=".5"/><path d="M8 5v6M5 8h6" stroke="#fff"/>',
    hdd: '<rect x="1.5" y="4" width="13" height="8" rx="1.5" fill="currentColor" opacity=".6"/><path d="M4 9.5h1M7 9.5h1"/>',
    bell: '<path d="M4 11V7a4 4 0 018 0v4l1 1.5H3z" fill="currentColor"/><path d="M6.5 14h3"/>',
    link: '<path d="M6.5 9.5l3-3"/><path d="M7 4.5l1-1a2.8 2.8 0 014 4l-1 1M9 11.5l-1 1a2.8 2.8 0 01-4-4l1-1"/>',
    copy: '<rect x="5" y="5" width="8.5" height="9" rx="1.5" fill="currentColor"/><path d="M3 11V3a1 1 0 011-1h6"/>',
    expand: '<path d="M2 5.5V2h3.5M10.5 2H14v3.5M14 10.5V14h-3.5M5.5 14H2v-3.5"/>',
    x: '<path d="M3.5 3.5l9 9M12.5 3.5l-9 9"/>',
    search: '<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14"/>',
    check: '<path d="M3 8.5l3 3 7-7"/>',
    upload: '<path d="M8 11V2.5M4.5 6L8 2.5 11.5 6M2.5 11v2.5h11V11"/>',
    download: '<path d="M8 2.5V11M4.5 7.5L8 11l3.5-3.5M2.5 11v2.5h11V11"/>',
    save: '<path d="M2.5 2.5h9l2 2v9h-11z" fill="currentColor" opacity=".7"/><rect x="5" y="9" width="6" height="4" fill="#2b2d32"/>',
    refresh: '<path d="M13 8a5 5 0 01-9 3M3 8a5 5 0 019-3"/><path d="M12 2v3h-3M4 14v-3h3"/>',
    users: '<circle cx="6" cy="5.5" r="2.3" fill="currentColor"/><path d="M1.5 13.5a4.5 4.5 0 019 0z" fill="currentColor"/><circle cx="11.3" cy="5.8" r="1.8" fill="currentColor"/><path d="M11 9a4 4 0 013.8 4.5H12" fill="currentColor"/>',
    user: '<circle cx="8" cy="8" r="6.5" fill="currentColor"/><circle cx="8" cy="6.3" r="2.2" fill="#26282c"/><path d="M4 12.3a4.5 4.5 0 018 0" fill="#26282c"/>',
    up: '<circle cx="8" cy="8" r="6.5" fill="currentColor"/><path d="M8 11.5V5M5.3 7.5L8 4.8l2.7 2.7" stroke="#26282c"/>',
    arrowDown: '<path d="M8 2.5v11M4 9.5l4 4 4-4"/>',
    arrowUp: '<path d="M8 13.5v-11M4 6.5l4-4 4 4"/>',
    chevR: '<path d="M6 3l5 5-5 5"/>',
    chevD: '<path d="M3 6l5 5 5-5"/>',
    caret: '<path d="M3.5 6h9L8 11z" fill="currentColor"/>',
    plus: '<path d="M8 2.5v11M2.5 8h11"/>',
    dots: '<circle cx="4" cy="8" r="1" fill="currentColor"/><circle cx="8" cy="8" r="1" fill="currentColor"/><circle cx="12" cy="8" r="1" fill="currentColor"/>',
    trash: '<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 9h5.6l.7-9"/>',
    ext: '<path d="M9 2.5h4.5V7M13.5 2.5L7.5 8.5M11.5 9.5v4h-9v-9h4"/>',
    file: '<path d="M4 1.5h5.5L12.5 4.5v10H4z" fill="currentColor" opacity=".75"/>',
    people: '<circle cx="6" cy="5.5" r="2.3" fill="currentColor"/><path d="M1.5 13.5a4.5 4.5 0 019 0z" fill="currentColor"/><circle cx="11.3" cy="5.8" r="1.8" fill="currentColor"/>',
    warn: '<path d="M8 1.8L15 14H1z" fill="currentColor"/><path d="M8 6v3.6M8 11.6v.4" stroke="#2b2d32"/>',
    info: '<circle cx="8" cy="8" r="6.5" fill="currentColor"/><path d="M8 7.2v4M8 4.8v.3" stroke="#2b2d32"/>',
    globe: '<circle cx="8" cy="8" r="6"/><path d="M2 8h12M8 2c2 2 2 10 0 12M8 2c-2 2-2 10 0 12"/>',
    key: '<circle cx="5" cy="8" r="3"/><path d="M8 8h6M12 8v2.5M10 8v2"/>',
    lock: '<rect x="3" y="7" width="10" height="7" rx="1"/><path d="M5 7V5a3 3 0 016 0v2"/>',
    back: '<path d="M10 3L5 8l5 5"/>',
  };
  function icon(name, size, color) {
    const s = size || 16;
    const el = h("span.hx-ico", { style: { display: "inline-flex", width: s + "px", height: s + "px", flex: "none", color: color || "" } });
    el.innerHTML = `<svg viewBox="0 0 16 16" width="${s}" height="${s}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${P[name] || ""}</svg>`;
    return el;
  }
  const gameIc = (g, big) => h("span.hx-gameic." + g + (big ? ".big" : ""));

  // ---------------------------------------------------------------- components
  const st = () => X.state;
  function btn(label, o) {
    o = o || {};
    const cls = ["hx-btn"]; if (o.v) cls.push("hx-" + o.v); if (o.sm) cls.push("hx-sm"); if (o.lg) cls.push("hx-lg"); if (o.dis) cls.push("hx-dis");
    return h("div", { class: cls.join(" "), role: "button", onclick: o.dis ? null : o.on, style: o.style }, o.icon ? icon(o.icon, o.iconSize || 15) : null, label);
  }
  const pill = (label, kind, extra) => h("span", { class: "hx-pill" + (kind ? " hx-" + kind : "") + (extra ? " " + extra : "") }, label);
  const tag = (label, kind) => h("span", { class: "hx-tag" + (kind ? " hx-" + kind : "") }, label);
  const toggle = (on, onclick, dis) => h("div", { class: "hx-toggle" + (on ? " hx-on" : "") + (dis ? " hx-dis" : ""), onclick: dis ? null : onclick });
  const box = (on) => h("span", { class: "hx-box" + (on ? " hx-on" : "") }, on ? icon("check", 13) : null);
  const check = (on, label, onclick) => h("span.hx-check", { onclick }, box(on), label);
  function input(content, o) {
    o = o || {};
    const kids = [];
    if (o.icon) kids.push(icon(o.icon, 16, "var(--g400)"));
    if (content == null && o.ph) kids.push(h("span.hx-ph", o.ph));
    else if (typeof content === "string") kids.push(tx(content));
    else if (content) kids.push(content);
    if (o.select) kids.push(icon("caret", 14, "var(--g400)"));
    return h("div", { class: "hx-input" + (o.mono ? " hx-mono" : "") + (o.focus ? " hx-focus" : "") + (o.err ? " hx-err" : "") + (o.select ? " hx-select" : "") + (o.w ? " hx-" + o.w : ""), style: o.style }, kids);
  }
  const field = (label, ctrl, hint, err) => h("div.hx-field", h("div.hx-label", label), ctrl, hint ? h("div.hx-muted.hx-small", hint) : null, err ? h("div.hx-err-t", err) : null);
  function srow(title, hint, ctrl, o) {
    o = o || {};
    return h("div", { class: "hx-srow" + (o.col ? " hx-col" : ""), style: o.style },
      h("div.hx-st", h("div.hx-stt", title, o.badge || null), hint ? h("div.hx-sth", hint) : null, o.below || null),
      ctrl ? h("div.hx-sc", ctrl) : null);
  }
  const rows = (...kids) => h("div.hx-card.hx-rows", kids);
  const ssec = (title, lead, ...kids) => h("div.hx-col", h("div.hx-shead", h("div.hx-h3", title), lead ? h("div.hx-sublead", lead) : null), kids);
  const loading = (label) => h("div.hx-flex.hx-muted", h("span.hx-dots", h("i"), h("i"), h("i")), label);
  function tip(el, text, o) {
    o = o || {};
    if (st().tooltips !== "on" || !text) return el;
    return h("span.hx-tipwrap", { style: o.block ? { display: "flex" } : null }, el, h("div", { class: "hx-tip" + (o.up ? " hx-up" : "") + (o.right ? " hx-right" : "") }, text));
  }
  function toast(kind, text, action) {
    const ic = { info: "i", ok: "✓", warn: "", err: "!" }[kind];
    return h("div.hx-toast", h("span.hx-tic." + kind, kind === "warn" ? icon("warn", 18) : tx(ic)), h("div.hx-grow", text),
      action ? h("span.hx-link", action) : null, icon("x", 16, "var(--g400)"));
  }
  function dlg(o, ...kids) {
    return h("div", { class: "hx-dlg", style: { width: (o.w || 640) + "px" } },
      o.noX ? null : h("div.hx-x", { onclick: () => X.close() }, icon("x", 18)),
      h("div.hx-dlgbody", o.title ? h("div.hx-dlgtitle", o.title) : null, kids));
  }
  const foot = (...b) => h("div.hx-dlgfoot", b);
  const close = () => X.close();
  const plain = (id) => { const s = window.CopyDesk.byId(id); return s ? window.CopyDesk.current(s)[0] : id };

  // ---------------------------------------------------------------- sample data
  const INST = [
    { name: "Survival SMP", desc: "A world for friends", port: 25565, uuid: "de71266c-4f1e-4c55-9a7e-3b1c0f8a2d11" },
    { name: "Creative Build", desc: "", port: 25566, uuid: "a03c9b21-77de-42a1-bb54-1f0e9d3c6a70", state: "running", game: "paper" },
    { name: "Modded Adventure", desc: "Create and friends", port: 25567, uuid: "5c2e8f90-1b3d-4e6a-8c7f-2d9a0b4e6f13", state: "stopped", game: "fabric" },
  ];
  const GAMES = { vanilla: "Minecraft", fabric: "Fabric (Minecraft)", paper: "Paper (Minecraft)" };
  const FRIENDS = [
    { name: "Alex", code: "K7Q2-M9XD-4TRW-H3BE", mc: "Alex_mc", shared: true, connected: true },
    { name: "Jordan", code: "P3VA-8NQE-2WZT-K5MC", mc: "Jordan_mc" },
    { name: "Sam", code: "D6HR-T1XB-9QJE-W4NA", mc: "Sam_mc", unverified: true },
  ];
  const coreName = () => st().coreKind === "remote" ? "Home server" : "peter's Lodestone Core";
  const userName = () => st().role === "user" ? "jordan" : "peter";
  const sel = () => { const i = +(X.params.i || 0); return Object.assign({ i }, INST[i] || INST[0]) };
  const selState = (inst) => inst.i === 0 || inst.i == null ? st().instance : inst.state;
  const selGame = (inst) => inst.i === 0 || inst.i == null ? st().game : inst.game;
  const isPlugins = () => selGame(sel()) === "paper";
  const supportsMods = () => selGame(sel()) !== "vanilla";
  const isOwner = () => st().role === "owner";
  const noInstances = () => st().instances === "none" || X.route === "first-run";

  function stateLabel(s) {
    return ({
      starting: () => S("StateStarting"), running: () => S("StateRunning"), stopping: () => S("StateStopping"),
      stopped: () => S("StateStopped"), crashed: () => S("StateError"), unknown: () => S("StateUnknown"),
    }[s] || (() => S("StateUnknown")))();
  }
  const stateDot = (s) => h("span", { class: "hx-dot " + ({ running: "hx-run", crashed: "hx-err", starting: "hx-warn", stopping: "hx-warn" }[s] || "hx-off") });
  const statePill = (s) => pill(stateLabel(s), { running: "green", crashed: "red", starting: "yellow", stopping: "yellow" }[s] || "grey");

  function power(s) {
    const go = (v) => () => X.set("instance", v);
    const start = () => (st().eula === "accepted" ? X.set("instance", "starting") : X.open("eula"));
    return ({
      stopped: [btn(M("Start"), { v: "primary", sm: true, on: start })],
      running: [btn(M("Stop"), { sm: true, on: go("stopping") }), btn(M("Restart"), { sm: true, on: go("starting") })],
      starting: [btn(M("Kill"), { sm: true, on: go("stopped") })],
      stopping: [btn(M("Kill"), { sm: true, on: go("stopped") })],
      crashed: [btn(M("Restart"), { sm: true, on: go("starting") })],
      unknown: [],
    })[s] || [];
  }

  // ---------------------------------------------------------------- frame
  function topBar() {
    const s = st();
    const notifCount = s.notifications === "some" ? 3 : 0;
    return h("div", { class: "hx-top" + (s.dev === "on" ? " hx-devline" : "") },
      h("div.hx-logo", h("span.hx-logomark"), tx("Lodestone")),
      s.dev === "on" ? h("span.hx-dev", S("DevBadge")) : null,
      tip(h("div.hx-bell", { onclick: () => X.set("notifPanel", s.notifPanel === "open" ? "closed" : "open") }, icon("bell", 16),
        notifCount ? h("span.hx-badge", tx(notifCount)) : null), C("Notifications"), { right: true }));
  }

  function sidebar() {
    const s = st();
    const route = X.route;
    const onInstance = TAB_ROUTES.includes(route);
    const inst = sel();
    const ist = selState(inst);
    const kids = [h("div.hx-cap", S("SelectedInstance"))];
    if (onInstance && !noInstances()) {
      const addr = "localhost:" + inst.port;
      const players = ist === "running" ? S("PlayersOnline", { online: 2, max: 20 }) : null;
      const atomLocked = s.role === "user" && s.atom === "on";
      kids.push(h("div.hx-instcard",
        h("div.hx-name", gameIc(selGame(inst)), tx(inst.name)),
        h("div.hx-meta", stateDot(ist), stateLabel(ist), tx("·"), tx(GAMES[selGame(inst)])),
        players ? h("div.hx-meta", players) : null,
        tip(h("span.hx-chipaddr", tx(addr), icon("copy", 11)), N("CopyAddress")),
        h("div.hx-power", atomLocked ? tip(h("span.hx-muted.hx-small", tx("—")), N("OwnerOnlyAtom")) : power(ist),
          tip(btn(N("ShareShort"), { sm: true, icon: "link", iconSize: 13, on: () => { X.set("sharing", s.sharing === "off" ? "starting" : s.sharing); X.go("overview", { i: inst.i }) } }), N("ShareShortHint")))));
      const tabs = [
        ["overview", "chart", S("TabOverview")], ["settings", "gear", S("TabSettings")], ["console", "server", S("TabConsole")],
        ["files", "folder", S("TabFiles")],
      ];
      if (supportsMods()) tabs.push(["mods", "puzzle", isPlugins() ? S("TabPlugins") : S("TabMods")]);
      if (route === "macros") tabs.push(["macros", "code", S("TabMacros")]);
      tabs.push(["eventlogs", "inbox", S("TabEventLogs")]);
      const active = route.startsWith("mods") ? "mods" : route;
      kids.push(h("div", { style: { marginTop: "6px", display: "flex", flexDirection: "column", gap: "1px" } },
        tabs.map(([r, ic, label]) => h("div", { class: "hx-row" + (active === r ? " hx-sel" : ""), onclick: () => X.go(r, { i: inst.i }) }, icon(ic, 16), h("span.hx-grow", label)))));
    } else {
      kids.push(h("div.hx-empty-sel", { style: { height: noInstances() ? "440px" : "400px" } }, icon("expand", 26, "var(--g500)"), S("NoInstanceSelected")));
    }
    kids.push(h("div.hx-cap", S("AllInstances")));
    if (noInstances()) kids.push(h("div.hx-muted.hx-italic", { style: { padding: "2px 8px 6px" } }, S("NoInstances")));
    else INST.forEach((it, i) => {
      const ss = i === 0 ? s.instance : it.state;
      kids.push(h("div", { class: "hx-row" + (onInstance && inst.i === i ? " hx-sel" : ""), onclick: () => X.go("overview", { i }) },
        gameIc(i === 0 ? s.game : it.game), h("span.hx-grow", tx(it.name)), stateDot(ss)));
    });
    if (s.creating === "on") kids.push(h("div.hx-row.hx-dis", h("span.hx-dots", h("i"), h("i"), h("i")), h("span.hx-grow", S("SettingUp", { name: "My Server" })), h("span.hx-small", N("SettingUpPercent", { percent: "42%" }))));
    const canCreate = s.role === "owner";
    kids.push(tip(h("div", { class: "hx-row" + (canCreate ? "" : " hx-dis"), onclick: canCreate ? () => X.open("create-games") : null }, icon("plusSq", 16), h("span.hx-grow", S("NewServer"))), canCreate ? null : S("NoPermission"), { block: true }));

    const conn = s.connection;
    const dotCls = { live: "hx-run", connecting: "hx-off", degraded: "hx-warn", reconnecting: "hx-err", signedout: "hx-err" }[conn];
    const label = { live: () => S("Connected"), connecting: () => S("Connecting"), degraded: () => S("Degraded"), reconnecting: () => S("Reconnecting"), signedout: () => S("SignedOut") }[conn]();
    const reason = conn === "degraded" || conn === "reconnecting" ? h("span", label, tx(": connection reset")) : label;
    const tipBody = h("div.hx-col.hx-gap4", reason,
      S("CoreAddress", { address: s.coreKind === "remote" ? "https://192.168.1.20:16662" : "https://127.0.0.1:16662" }),
      S("CoreVersion", { version: "0.5.1" }), S("CoreHost", { host: s.coreKind === "remote" ? "homeserver" : "peters-mbp" }),
      S("CoreSystem", { system: "macOS (Apple silicon)" }), S("CoreUptime", { uptime: FM("HoursMinutes", { hours: 2, minutes: 14 }) }));
    const footer = h("div.hx-foot",
      tip(h("div.hx-conn", h("span", { class: "hx-dot " + dotCls }), h("span.hx-grow", tx(coreName()))), tipBody, { up: true, block: true }),
      h("div.hx-muted.hx-small", { style: { padding: "0 8px" } }, S("SignedInAs", { name: userName() })),
      h("div", { class: "hx-row" + (X.dialog && X.dialog.startsWith("settings") ? " hx-sel" : ""), onclick: () => X.open("settings-general") }, icon("gear", 16), h("span.hx-grow", S("Settings"))),
      h("div.hx-row", { onclick: () => X.open("cores") }, icon("hdd", 16), h("span.hx-grow", C("ChangeCore"))));
    return h("div.hx-side", h("div.hx-sidescroll", { "data-mk-scroll": "side" }, kids), footer);
  }

  function notifPanel() {
    const s = st();
    const body = [h("div.hx-h3", { style: { fontSize: "19px" } }, C("Notifications"))];
    if (s.notifications === "some") {
      body.push(h("div.hx-small", { style: { fontWeight: 700, color: "var(--g400)" } }, C("InProgress")));
      body.push(h("div.hx-nitem", h("span.hx-dots", h("i"), h("i"), h("i")), h("div.hx-grow", S("SettingUp", { name: "My Server" }), h("div.hx-muted.hx-small", N("SettingUpPercent", { percent: "42%" })))));
    }
    body.push(h("div.hx-small", { style: { fontWeight: 700, color: "var(--g400)" } }, C("AllNotifications")));
    if (s.notifications === "loading") body.push(loading(C("LoadingNotifications")));
    else if (s.notifications === "none") body.push(h("div.hx-muted", { style: { padding: "10px 0" } }, C("NoNotifications")));
    else {
      const item = (txt, when, kind) => h("div.hx-nitem", h("span", { class: "hx-dot " + (kind || "hx-off"), style: { marginTop: "7px" } }), h("div.hx-grow", txt), h("span.hx-muted.hx-small", when));
      body.push(
        item(C("EventRunning", { name: "Survival SMP" }), FM("JustNow"), "hx-run"),
        item(C("PlayersJoined", { players: "Alex, Sam", name: "Survival SMP" }), FMP("MinutesAgo", { count: 2 })),
        item(C("PlayersLeft", { players: "Jordan", name: "Survival SMP" }), FMP("MinutesAgo", { count: 5 })),
        item(C("PlayersJoinedAndLeft", { joined: "Alex", left: "Sam", name: "Creative Build" }), FMP("MinutesAgo", { count: 9 })),
        item(C("PlayerMessage", { player: "Alex", message: "hi all", name: "Survival SMP" }), FMP("MinutesAgo", { count: 12 })),
        item(C("EventStarting", { name: "Survival SMP" }), FMP("HoursAgo", { count: 1 }), "hx-warn"),
        item(C("EventStopping", { name: "Creative Build" }), FMP("HoursAgo", { count: 1 }), "hx-warn"),
        item(C("EventStopped", { name: "Creative Build" }), FMP("HoursAgo", { count: 2 })),
        item(C("EventCrashed", { name: "Modded Adventure" }), FMP("HoursAgo", { count: 3 }), "hx-err"),
        item(C("EventWarning", { name: "Modded Adventure" }), FMP("HoursAgo", { count: 3 }), "hx-warn"),
        item(C("EventError", { name: "Modded Adventure" }), FMP("HoursAgo", { count: 3 }), "hx-err"),
        item(C("EventApiKeyCreated", { name: "backup script" }), FM("Yesterday")),
        item(C("EventApiKeyRevoked", { name: "old laptop" }), FMP("DaysAgo", { count: 2 })),
        item(C("MacroStarted", { pid: 12, server: C("CoreFallback") }), FMP("DaysAgo", { count: 3 })),
        item(C("MacroDetached", { pid: 12, server: "Survival SMP" }), FMP("DaysAgo", { count: 3 })),
        item(C("MacroStopped", { pid: 12, server: "Survival SMP", status: "0" }), FMP("DaysAgo", { count: 3 })),
      );
      body.push(h("div.hx-note", h("div", { style: { fontWeight: 600, marginBottom: "4px" } }, C("BetaTitle")), C("BetaBody")));
    }
    return h("div.hx-notif", body);
  }

  function toasts() {
    return h("div.hx-toasts",
      toast("info", FP("FilesUploading", { count: 3 })),
      toast("ok", F("FilesDeletedFile", { name: "server.properties" })),
      toast("ok", N("ShareCopied")),
      toast("err", F("ConsoleSendFailed", { error: "the server is stopped" }), M("TryAgain")));
  }

  // bare screens (before the shell): core starting, failed, signing in
  function bareScreen() {
    const s = st();
    const card = (...kids) => h("div.hx-bare", h("div.hx-logincard", h("div.hx-logo", { style: { fontSize: "24px" } }, h("span.hx-logomark"), tx("Lodestone")), kids));
    if (s.core === "starting") return card(loading(h("span", { style: { color: "var(--g300)", fontSize: "16px" } }, S("StartingCore"))));
    if (s.core === "restarting") return card(loading(h("span", { style: { color: "var(--g300)", fontSize: "16px" } }, S("RestartingCore"))));
    if (s.core === "stopping") return card(loading(h("span", { style: { color: "var(--g300)", fontSize: "16px" } }, S("StoppingCore"))));
    if (s.core === "failed" || s.core === "missing") {
      const missing = s.core === "missing";
      return card(
        h("div", { style: { fontFamily: "Chillax", fontWeight: 600, fontSize: "30px" } }, S("CoreFailed")),
        h("div.hx-muted", tx(missing ? "LODESTONE_HOST_CORE is set to /nonexistent/lodestone_core, which isn't a file." : "The core exited with status 101.")),
        missing ? null : h("div.hx-small.hx-muted", S("CoreLog")),
        h("div.hx-logbox", tx(missing ? "" : "thread 'main' panicked at core/src/main.rs:212:\nfailed to bind 127.0.0.1:16662: address in use\nnote: run with RUST_BACKTRACE=1")),
        h("div.hx-flex", btn(M("TryAgain"), { v: "primary", on: () => X.set("core", "starting") }), btn(S("OpenLogFolder"))));
    }
    if (s.core === "signin" || s.core === "signingin") {
      const busy = s.core === "signingin";
      return card(
        h("div", { style: { fontFamily: "Chillax", fontWeight: 600, fontSize: "28px" } }, S("SignInTitle")),
        h("div.hx-muted", S("SignInIntro")),
        field(S("Username"), input("peter")),
        field(S("Password"), input(null, { ph: tx("") }), null, s.signinErr === "on" ? S("NoPassword") : null),
        busy ? loading(S("SigningIn")) : h("div", btn(M("SignIn"), { v: "primary", on: () => X.set("core", "signingin") })));
    }
    return null;
  }

  function frame(ctx, page) {
    const s = st();
    const root = h("div.hx-root");
    if (!document.getElementById("hx-style")) document.head.append(h("style", { id: "hx-style" }, css));
    const routeDef = ROUTES[ctx.route];
    if (routeDef && routeDef.bare) { root.append(page); return root }
    if (s.core !== "running") { root.append(bareScreen()); return root }
    root.append(topBar());
    if (s.connection === "degraded") root.append(h("div.hx-banner", S("DegradedBanner"), btn(S("Reload"), { sm: true, on: () => X.set("connection", "live") })));
    if (s.lanHttp === "on") root.append(h("div.hx-banner.hx-red", s.role === "owner" ? C("LanHttpWarningOwner", { setting: C("LanHttp") }) : C("LanHttpWarningOther")));
    root.append(h("div.hx-body", sidebar(), h("div.hx-page", { "data-mk-scroll": "page" }, page)));
    if (s.notifPanel === "open") root.append(notifPanel());
    if (s.toasts === "on") root.append(toasts());
    return root;
  }

  // ---------------------------------------------------------------- pages
  const pageWrap = (...kids) => h("div.hx-pagein", kids);
  const heading = (label, extra) => h("div.hx-h1", label, extra || null);

  function homePage() {
    return h("div.hx-home",
      h("div.hx-cube"),
      h("div.hx-hometitle", S("HomeTitle")),
      h("div.hx-muted", { style: { fontSize: "16px" } }, S("HomeSubtitle")),
      btn(S("CreateServer"), { v: "primary", lg: true, icon: "plus", on: () => X.open("create-games") }));
  }

  // --- overview
  function shareCard() {
    const s = st();
    const sh = s.sharing;
    const friendsOn = !["off", "retired", "cert", "certManual"].includes(s.friends);
    const parts = [];
    if (friendsOn && sh !== "off") {
      parts.push(h("div", { style: { fontWeight: 600, fontSize: "16px", marginBottom: "12px" } }, N("ShareFriendsHeading")));
      if (s.friends === "none") parts.push(h("div.hx-muted.hx-small", N("ShareFriendsNone")));
      else {
        parts.push(friendList(false));
        parts.push(h("div.hx-muted.hx-small", { style: { marginTop: "10px" } }, N("ShareFriendsNote")));
      }
      parts.push(h("div.hx-hr"));
      parts.push(h("div", { style: { fontWeight: 600, fontSize: "16px", marginBottom: "12px" } }, N("ShareLinkHeading")));
    }
    if (sh === "off") {
      return h("div.hx-card.hx-pad.hx-between", h("div", { style: { maxWidth: "560px", fontSize: "15.5px" } }, N("ShareOff")),
        btn(N("ShareStart"), { v: "primary", icon: "link", on: () => X.set("sharing", "starting") }));
    }
    const stPill = { starting: () => pill(N("ShareStarting"), "grey"), sharing: () => pill(N("ShareSharing"), "green"), reconnecting: () => pill(N("ShareReconnecting"), "yellow") }[sh]();
    parts.push(h("div.hx-flex", { style: { marginBottom: "14px" } }, stPill, sh === "sharing" ? h("span.hx-muted.hx-small", N("ShareStableLink")) : null));
    const link = sh === "starting" ? input(null, { ph: N("SharePreparing"), mono: true, style: { flex: 1 } }) : input("https://connect.lodestone.cc/j/7KQ2M9XB4D#3f9a….c01d", { mono: true, style: { flex: 1 } });
    parts.push(h("div.hx-flex", tip(link, N("ShareLinkLabel"), { block: true }), btn(N("ShareCopy"), { v: "primary", icon: "copy", dis: sh === "starting" })));
    parts.push(h("div.hx-muted.hx-small", { style: { margin: "12px 0" } }, N("ShareHowFriendsJoin")));
    if (sh !== "starting") {
      parts.push(h("div.hx-warn-t", { style: { marginBottom: "12px" } }, N("ShareHardNetwork")));
      parts.push(h("div.hx-flex", { style: { marginBottom: "10px" } }, icon("people", 16, "var(--g400)"), h("span", { style: { fontSize: "15.5px" } }, NP("ShareConnected", { count: 2 })), h("span.hx-muted.hx-small", NP("ShareJoins", { count: 3 }))));
      const fl = (ip, conn, ms) => h("div.hx-input.hx-between", { style: { marginBottom: "6px" } }, h("span.hx-mono", tx(ip)), h("span.hx-muted.hx-small", N("ShareFriendLine", { connection: conn, ms })));
      parts.push(fl("203.0.113.42", N("ShareInternet"), 38), fl("192.168.1.31", N("ShareLocalNetwork"), 2), fl("127.0.0.1", N("ShareSameComputer"), 0), fl("2001:db8::7", N("ShareIpv6"), 24));
    }
    parts.push(h("div", { style: { display: "flex", justifyContent: "flex-end", marginTop: "12px" } }, btn(N("ShareStop"), { v: "danger", lg: true, on: () => X.open("stop-sharing") })));
    return h("div.hx-card.hx-pad", parts);
  }

  function chart(label, max) {
    if (st().graph === "loading") return h("div.hx-chart", h("div.hx-loadbox", h("span.hx-dots", h("i"), h("i"), h("i")), N("LoadingGraph")));
    const svg = h("div", { style: { position: "absolute", inset: 0 } });
    svg.innerHTML = '<svg viewBox="0 0 100 100" preserveAspectRatio="none" width="100%" height="100%"><path d="M0 70 L8 60 L16 64 L24 58 L32 72 L40 85 L48 80 L56 66 L64 58 L72 54 L80 60 L88 66 L100 70 L100 100 L0 100Z" fill="rgba(109,210,119,.25)" stroke="#6DD277" stroke-width="1" vector-effect="non-scaling-stroke"/></svg>';
    const c = h("div.hx-chart", h("div.hx-axis", tx(max), tx("0")), h("div.hx-plot", svg), h("div.hx-xaxis", N("GraphSeconds", { seconds: 59 }), tx("0")));
    return st().tooltips === "on" ? h("div", { style: { position: "relative" } }, c, h("div.hx-tip", { style: { left: "50%", top: "40%" } }, N("GraphSecondsAgo", { seconds: 24, value: label }))) : c;
  }

  function overviewPage() {
    const s = st(); const inst = sel(); const ist = selState(inst);
    const g = selGame(inst);
    const players = ist === "running" ? 2 : 0;
    const head = [
      h("div.hx-h1", { style: { marginBottom: "14px" } }, tip(h("span", tx(inst.name)), N("EditName"))),
      h("div.hx-pills", gameIc(g, true), statePill(ist), pill(N("VersionLabel", { version: g === "fabric" ? "1.21.4" : "26.3" })),
        pill(N("PlayerCount", { players, max: 20 })), tip(pill(h("span", tx("localhost:" + inst.port), " ", icon("copy", 12)), null, "hx-mono"), N("CopyAddress"))),
      tip(h("div.hx-lead", { style: { margin: "12px 0 0" } }, inst.desc ? tx(inst.desc) : N("NoDescription")), N("EditDescription")),
    ];
    if (s.role === "user" && s.atom === "on") head.push(h("div.hx-note.hx-yellow", { style: { marginTop: "12px" } }, N("OwnerOnlyAtom")));
    const share = h("div.hx-sec", h("div.hx-h2", N("ShareTitle")), h("div.hx-lead", N("ShareLead")), shareCard());
    const perf = h("div.hx-sec", h("div.hx-h2", N("PerformanceTitle")), h("div.hx-lead", N("PerformanceLead")),
      h("div.hx-grid2", { style: { gap: "30px" } },
        h("div.hx-col", h("div.hx-muted.hx-small", tip(N("CpuLoad", { value: ist === "running" ? 23 : 0 }), N("CpuUsage"))), chart(N("UnitPercent"), "100%")),
        h("div.hx-col", h("div.hx-muted.hx-small", tip(N("MemoryUsed", { value: ist === "running" ? "1.8" : "0.0" }), N("MemoryUsage"))), chart(N("UnitGiB"), "48.0GiB"))));
    const plist = s.players === "some" && ist === "running"
      ? h("div.hx-card.hx-rows", ["Alex_mc", "Sam_mc"].map((p, i) => h("div.hx-srow", { style: { minHeight: "48px" } }, h("span.hx-avatar.hx-sm", { style: { background: i ? "#4a7" : "#c84", borderRadius: "6px" } }, tx(p[0])), h("span.hx-grow", tx(p)), i === 0 ? tip(h("span.hx-usertag", tx("peter")), N("PlayersLinkedUser"), { right: true }) : null)))
      : h("div.hx-dashed", icon("user", 18), h("span.hx-italic", N("PlayersNone")));
    const pl = h("div.hx-sec", h("div.hx-h2", N("PlayersTitle")), h("div.hx-lead", N("PlayersLead")),
      h("div.hx-flex.hx-muted.hx-small", { style: { padding: "0 10px 8px" } }, N("PlayersSortByName"), icon("arrowDown", 14)), plist);
    const open = s.details === "open";
    const details = h("div.hx-sec",
      h("div.hx-flex", { style: { cursor: "pointer", fontSize: "19px", color: "var(--g400)" }, onclick: () => X.set("details", open ? "closed" : "open") }, icon(open ? "chevD" : "chevR", 18), N("DetailsTitle")),
      open ? h("div.hx-card.hx-pad", { style: { marginTop: "12px" } }, h("div.hx-kv",
        N("DetailsName"), tx(inst.name), N("DetailsDescription"), inst.desc ? tx(inst.desc) : N("NoDescription"), N("DetailsUuid"), h("span.hx-mono", tx(inst.uuid)),
        N("DetailsGame"), tx(GAMES[g]), N("DetailsMinecraftVersion"), tx("1.21.4"), N("DetailsVersion"), tx("26.3"), N("DetailsLoader"), tx("Fabric 0.16.9"),
        N("DetailsPort"), tx(inst.port), N("DetailsState"), stateLabel(ist), N("DetailsPlayers"), tx(players + "/20"),
        N("DetailsCreated"), FM("ShortDateWithYear", { month: FM("March"), day: 14, year: 2026 }), N("DetailsAutoStart"), M("No"), N("DetailsRestartOnCrash"), M("Yes"),
        N("DetailsRam"), N("DetailsRamValue", { min: 1024, max: 2048 }), N("DetailsFolder"), h("span.hx-mono", tx("~/.lodestone/instances/Survival SMP-de71266c")),
        N("DetailsModpack"), tx("Cobblemon Official Modpack 1.6"))) : null);
    return pageWrap(head, share, perf, pl, details);
  }

  // --- instance settings
  function friendList(inSettings) {
    const s = st();
    return h("div.hx-card.hx-rows", { style: inSettings ? { borderRadius: "10px 10px 0 0" } : null },
      s.friends === "error" && inSettings ? srow(h("span.hx-err-t", { style: { fontSize: "15px" } }, I("FriendsSyncFailed", { error: "Lodestone Core didn't answer" })), I("FriendsRetrying"), btn(I("FriendsRetry"), { sm: true })) : null,
      FRIENDS.map((f) => {
        const on = f.shared;
        const kids = [h("div.hx-srow.hx-col", { style: { gap: "4px" } },
          h("div.hx-between", check(on, I("FriendsShareWith", { name: f.name }), () => { if (f.unverified) X.set("friends", "confirm") }),
            h("div.hx-flex", h("span.hx-mono.hx-muted.hx-small", tx(f.code)), f.connected ? tag(I("FriendsConnected"), "green") : null, f.unverified ? tag(I("FriendsNotVerified"), "yellow") : null)),
          h("div.hx-muted.hx-small", { style: { paddingLeft: "28px" } }, I("FriendsMinecraft", { names: f.mc })))];
        if (f.unverified && s.friends === "confirm" && inSettings) kids.push(h("div.hx-srow.hx-confirm", h("div.hx-st", I("FriendsConfirmUnverified", { name: f.name })), h("div.hx-sc", btn(I("FriendsShareAnyway"), { sm: true, on: () => X.set("friends", "shared") }), btn(I("FriendsCancel"), { sm: true, on: () => X.set("friends", "shared") }))));
        return kids;
      }));
  }

  function friendsSection() {
    const s = st();
    const f = s.friends;
    const body = [];
    if (f === "loading") body.push(h("div.hx-card.hx-pad", loading(I("FriendsLoading"))));
    else if (f === "none") body.push(rows(srow(I("FriendsAddInLauncher"), I("FriendsAddInLauncherHint"))));
    else if (f === "retired") body.push(rows(srow(I("FriendsRetired"), I("FriendsRetiredHint"))));
    else if (f === "off") body.push(rows(srow(I("FriendsOff"), I("FriendsOffHint"))));
    else {
      const fl = friendList(true);
      let cert;
      if (f === "cert") cert = srow(I("FriendsCertificateRetired"), I("FriendsCertificateRetiredHint"), btn(I("FriendsCertificateNew"), { sm: true, on: () => X.set("friends", "shared") }));
      else if (f === "certManual") cert = srow(I("FriendsCertificateRetired"), I("FriendsCertificateManual", { path: "~/.lodestone/friends/certificate.json" }));
      else cert = srow(I("FriendsCertificate"), I("FriendsCertificateHint"), btn(I("FriendsCertificateRetire"), { sm: true, on: () => X.open("retire-cert") }));
      fl.append(cert);
      body.push(fl);
      body.push(h("div.hx-flex.hx-small", f === "error" || f === "cert" || f === "certManual" ? null : h("span.hx-ok-t.hx-flex", { style: { gap: "4px" } }, icon("check", 14), I("FriendsSynced")), h("span.hx-muted", IP("FriendsConnectedCount", { count: 1 }))));
      if (f === "error") body.push(h("div.hx-col.hx-gap4",
        h("div.hx-err-t", I("FriendsOpenFailed", { error: "the friend list is locked" })),
        h("div.hx-err-t", I("FriendsPublishFailed", { name: "Jordan", error: "relay unreachable" })),
        h("div.hx-err-t", I("FriendsPublishFailedAll", { error: "relay unreachable" })),
        h("div.hx-err-t", I("FriendsWhitelistFailed", { name: "Alex", error: "the server is stopped" })),
        h("div.hx-ok-t", I("FriendsWhitelisted", { names: "Alex_mc" }))));
    }
    return ssec(I("FriendsTitle"), I("FriendsLead"), body);
  }

  function settingsPage() {
    const s = st();
    const inst = sel();
    const sections = [
      ["cmd", tx("Command Line Settings")], ["props", tx("Server Properties Settings")], ["auto", I("Automation")], ["listing", C("ListingTitle")],
      ["friends", I("FriendsTitle")], ["pack", I("ResourcePack")], ["eula", I("Eula")], ["danger", I("DangerZone")],
    ];
    const cur = X.params.sec || "cmd";
    const nav = h("div.hx-snav", sections.map(([k, label]) => h("div", { class: "hx-si" + (cur === k ? " hx-on" : ""), onclick: () => X.go("settings", { i: inst.i, sec: k }) }, label)));
    let content;
    if (s.settingsLoad === "loading") content = [h("div.hx-card.hx-pad", loading(M("Loading")))];
    else if (s.settingsLoad === "failed") content = [h("div.hx-inlineerr", I("SettingsFailed", { error: "Server not found" }))];
    else if (s.settingsLoad === "noperm") content = [h("div.hx-card.hx-pad.hx-muted", I("NoPermission"))];
    else {
      const ce = s.codeExec;
      const cmdHint = s.role === "user" ? I("CommandLineOwnerOnly") : ce === "off" || ce === "locked" ? I("CommandLineReadOnly") : null;
      const ro = !!cmdHint;
      content = [
        ssec(tx("Command Line Settings"), tx("Settings are passed to the server and Java as command line arguments"), rows(
          srow(tx("Command line arguments"), h("span", tx("The command line arguments to pass to Java. Use quotes for arguments that contain spaces"), cmdHint ? h("div.hx-warn-t", cmdHint) : null), input("", { w: "w360", style: ro ? { opacity: ".5" } : null })),
          srow(tx("Minimum RAM"), tx("The minimum amount of RAM to allocate to the server, in MB"), input("1024", { w: "w360" })),
          srow(tx("Maximum RAM"), h("span", tx("The maximum amount of RAM to allocate to the server, in MB"), s.validation === "on" ? h("div.hx-err-t", I("MustBeLessThan", { max: 49152 })) : null), input("99999", { w: "w360", err: s.validation === "on" })),
          srow(tx("Java command"), h("span", tx("The command to use to run the java executable"), cmdHint ? h("div.hx-warn-t", cmdHint) : null), input("/usr/bin/java", { w: "w360" })))),
        ssec(tx("Server Properties Settings"), tx("All settings in the server.properties file can be configured here"), rows(
          srow(tx("Server Port"), h("span", tx("The port the server listens on."), s.validation === "on" ? h("div.hx-err-t", I("MustBeNumber")) : null), input(s.validation === "on" ? "25a65" : "25565", { w: "w360", err: s.validation === "on" })),
          srow(tx("Message of the day"), h("span", tx("Shown in the server list."), s.validation === "on" ? h("div.hx-err-t", I("CannotBeEmpty")) : null), input("", { w: "w360", err: s.validation === "on" })),
          srow(tx("View distance"), h("span", tx("In chunks."), s.validation === "on" ? h("div.hx-err-t", I("MustBeGreaterThan", { min: 2 })) : null), input("10", { w: "w360" })),
          srow(tx("RCON password"), null, h("div.hx-flex", input("••••••••", { w: "w300" }), btn(I("ShowPassword"), { sm: true }), btn(I("HidePassword"), { sm: true }), btn(I("Revert"), { sm: true }))))),
        ssec(I("Automation"), I("AutomationLead"), rows(
          s.neverStarted === "on" ? srow(I("NotAvailable"), I("NotAvailableHint")) : [
            srow(I("AutoStart"), I("AutoStartHint"), toggle(false, null, s.role === "user")),
            srow(I("RestartOnCrash"), I("RestartOnCrashHint"), toggle(true, null, s.role === "user"))],
          s.role === "user" ? srow(h("span.hx-muted", I("NoPermission")), s.atom === "on" ? I("OwnerOnlyAtom") : null) : null)),
        ssec(C("ListingTitle"), C("ListingLead"), rows(srow(C("ListedPublicly"), s.role === "user" ? C("ListingOwnerOnly") : null, toggle(false, null, s.role === "user")))),
        friendsSection(),
        ssec(I("ResourcePack"), I("ResourcePackLead"), rows(
          srow(I("PackUrl"), h("span", I("PackUrlHint"), s.validation === "on" ? h("div.hx-err-t", I("PackUrlScheme")) : null), input(s.validation === "on" ? "ftp://example.com/pack.zip" : null, { ph: tx("https://example.com/pack.zip"), w: "w300", err: s.validation === "on" })),
          srow(I("PackSha1"), h("span", I("PackSha1Hint"), s.validation === "on" ? h("div.hx-err-t", I("PackSha1Error")) : h("div.hx-ok-t", I("PackDownloaded", { size: FM("Megabytes", { size: "4.2" }) }))), h("div.hx-flex", input(null, { w: "w300" }), btn(I("PackCompute"), { sm: true }))),
          srow(I("PackRequired"), I("PackRequiredHint"), toggle(false)),
          srow(I("PackPrompt"), I("PackPromptHint"), input(null, { w: "w300" })),
          h("div.hx-srow", { style: { justifyContent: "flex-end" } }, h("span.hx-ok-t", M("Saved")), btn(I("PackRemove"), { sm: true }), btn(M("Save"), { v: "primary", sm: true })))),
        ssec(I("Eula"), I("EulaLead"), rows(
          s.eula === "accepted"
            ? srow(I("EulaAccepted"), I("EulaAcceptedHint"), btn(I("EulaRead"), { sm: true, icon: "ext" }), { badge: icon("check", 15, "var(--green)") })
            : srow(I("EulaNotAccepted"), s.role === "user" ? I("EulaNoPermission") : I("EulaNotAcceptedHint"), h("div.hx-flex", btn(I("EulaRead"), { sm: true, icon: "ext" }), btn(I("EulaAgree"), { v: "primary", sm: true, dis: s.role === "user", on: () => X.set("eula", "accepted") }))),
          s.eula === "accepted" ? null : h("div.hx-srow.hx-err-t", I("EulaAcceptFailed", { error: "permission denied" })))),
        ssec(I("DangerZone"), I("DangerZoneLead"), rows(
          srow(I("DeleteServer"), s.role === "user" ? (s.atom === "on" ? I("OwnerOnlyAtom") : I("NoPermission")) : I("DeleteServerHint"), btn(M("Delete"), { v: "danger", sm: true, dis: s.role === "user", on: () => X.open("delete-instance") })))),
      ];
    }
    return pageWrap(heading(S("TabSettings")), h("div", { style: { display: "flex", gap: "40px" } }, nav, h("div.hx-scontent", content)));
  }

  // --- console
  function consolePage() {
    const s = st(); const ist = selState(sel());
    const c = s.console;
    const lines = [
      ["", "[11:59:46] [Worker-Main-1/INFO]: Preparing spawn area: 90%"],
      ["", "[11:59:47] [Worker-Main-2/INFO]: Preparing spawn area: 95%"],
      ["", "[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.1"],
      ["", "[12:00:00] [Server thread/INFO]: Loading properties"],
      ["w", "[12:00:01] [Server thread/WARN]: Can't keep up! Is the server overloaded? Running 2034ms or 40 ticks behind"],
      ["e", "[12:00:01] [Server thread/ERROR]: Encountered an unexpected exception"],
      ["g", "[12:00:02 INFO]: Done (3.2s)! For help, type \"help\""],
    ];
    let log;
    if (c === "loading") log = h("div.hx-log", h("div.hx-home", loading(F("ConsoleLoading"))));
    else if (c === "noperm") log = h("div.hx-log", h("div.hx-home.hx-muted", F("ConsoleNoPermission")));
    else if (c === "empty") log = h("div.hx-log", h("div.hx-home.hx-muted", F("ConsoleEmpty")));
    else log = h("div.hx-log", { "data-mk-scroll": "log" },
      lines.map(([k, l]) => h("div", { class: k }, tx(l))),
      h("div", tx("[12:00:04] [Server thread/INFO]: Notch joined the game"), tip(h("span.hx-usertag", tx("peter")), F("ConsoleLinkedUser"))),
      h("div", tx("[12:00:06] [Server thread/INFO]: Alex[/127.0.0.1:50000] logged in with entity id 120 at (0.5, 64.0, 0.5)")));
    const status = { live: () => F("ConsoleStatusLive"), loading: () => F("ConsoleStatusLoading"), buffered: () => F("ConsoleStatusBuffered"), nobuffer: () => F("ConsoleStatusLiveNoBuffer"), closed: () => F("ConsoleStatusClosed"), error: () => F("ConsoleStatusError"), noperm: () => F("ConsoleStatusNoPermission"), empty: () => F("ConsoleStatusLive") }[c]();
    const dotCls = { live: "hx-run", empty: "hx-run", nobuffer: "hx-warn", error: "hx-err", closed: "hx-off" }[c] || "hx-off";
    let ph;
    if (c === "noperm") ph = F("ConsoleInputNoPermission");
    else if (s.role === "user" && s.atom === "on") ph = F("ConsoleOwnerOnlyAtom");
    else if (ist !== "running") ph = F("ConsoleServerStopped");
    else if (c === "closed") ph = F("ConsoleStatusClosed");
    else ph = F("ConsoleInputPlaceholder");
    return pageWrap(
      heading(S("TabConsole"), statePill(ist)),
      h("div.hx-flex", { style: { marginBottom: "16px" } }, input(null, { ph: F("ConsoleSearch"), icon: "search", mono: true, style: { flex: 1 } }), check(true, F("ConsoleRegex"))),
      tip(h("div.hx-console",
        h("div", { style: { position: "absolute", right: "14px", top: "12px", zIndex: 3 } }, tip(h("span", { class: "hx-dot " + dotCls, style: { width: "11px", height: "11px" } }), status, { right: true })),
        log,
        c === "live" ? h("div", { style: { position: "absolute", right: "24px", bottom: "74px" } }, btn(F("ConsoleJumpToLatest"), { sm: true, icon: "arrowDown" })) : null,
        h("div.hx-cmd", tip(input(null, { ph, mono: true }), F("ConsoleCommand"), { up: true, block: true }))), F("Console"), { block: true }),
      c === "error" ? h("div.hx-err-t", { style: { marginTop: "10px" } }, F("ConsoleSendFailed", { error: "connection reset" })) : null);
  }

  // --- files
  function filesPage() {
    const s = st(); const inst = sel();
    const f = s.files;
    const toolbar = h("div.hx-flex", { style: { marginBottom: "14px", position: "relative" } },
      btn(h("span.hx-flex", { style: { gap: "8px" } }, F("FilesAddRemove"), icon("caret", 14)), { lg: true, on: () => X.set("files", f === "addmenu" ? "editor" : "addmenu") }),
      h("div.hx-grow", { style: { fontSize: "16.5px", paddingLeft: "6px" } }, h("span.hx-link", tx(inst.name + "-" + inst.uuid.slice(0, 7))), tx(" / server.properties")),
      btn(M("Save"), { lg: true, icon: "save", dis: f !== "unsaved" }), btn(F("FilesDiscardChanges"), { lg: true, icon: "refresh", dis: f !== "unsaved", on: () => X.open("files-unsaved") }), btn(F("FilesUpload"), { lg: true, icon: "upload" }),
      f === "addmenu" ? h("div.hx-menu", { style: { left: 0, top: "44px" } },
        h("div.hx-mi", { onclick: () => X.open("files-new-file") }, F("FilesNewFile")), h("div.hx-mi", { onclick: () => X.open("files-new-folder") }, F("FilesNewFolder")), h("div.hx-mi", F("FilesUpload")), h("div.hx-sep"),
        h("div.hx-mi", F("FilesSelectAll")), h("div.hx-mi", F("FilesDeselectAll")), h("div.hx-sep"),
        h("div.hx-mi", F("FilesCutSelected")), h("div.hx-mi", F("FilesDownloadSelected")), h("div.hx-mi", F("FilesUnarchiveSelected")), h("div.hx-mi.hx-red", { onclick: () => X.open("files-delete") }, F("FilesDeleteSelected"))) : null);
    const files = [["macros", "folder", "#3B8FD9"], ["resources", "folder", "#3B8FD9"], [".lodestone_config", "file", "#2EF29A"], [".lodestone_minecraft_config.json", "file", "#EFB440"], ["eula.txt", "file", "#bbb"], ["server.jar", "file", "#FF5C5C"], ["server.properties", "file", "#bbb"], ["world.zip", "file", "#a77"]];
    let list;
    if (f === "loading") list = h("div.hx-flist", h("div.hx-home", loading(F("FilesLoading"))));
    else if (f === "noperm") list = h("div.hx-flist", h("div.hx-home.hx-muted", { style: { padding: "20px" } }, F("FilesNoReadPermission")));
    else if (f === "empty") list = h("div.hx-flist", h("div.hx-home.hx-muted", F("FilesEmpty")));
    else list = tip(h("div.hx-flist", files.map(([n, ic, col], i) => h("div", { class: "hx-frow" + (i === 4 ? " hx-sel" : "") + (i === 6 ? " hx-open" : ""), onclick: () => X.set("files", "menu") },
      tip(box(i === 4), i === 0 ? F("FilesSelect", { name: n }) : null), icon(ic, 18, col), h("span.hx-grow", { style: { whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" } }, tx(n)),
      i === 7 ? h("span.hx-muted.hx-small", F("FilesUnknownTime")) : null))), F("FilesList"));
    let editor;
    if (f === "toolarge") editor = h("div.hx-editor", h("div.hx-home.hx-muted", { style: { padding: "30px" } }, F("FilesTooLarge", { size: FM("Megabytes", { size: 5 }) }), btn(M("Open"), { icon: "download" })));
    else if (f === "loadingfile") editor = h("div.hx-editor", h("div.hx-home", loading(F("FilesLoadingFile"))));
    else if (f === "empty" || f === "noperm" || f === "loading") editor = h("div.hx-editor");
    else {
      const find = f === "find" ? h("div.hx-findbar",
        h("div.hx-flex", input("motd", { focus: true, w: "w300" }), tip(h("span.hx-muted.hx-small", F("FilesFindNone")), F("FilesFind")), tip(btn(icon("arrowUp", 14), { sm: true }), F("FilesFindPrevious")), tip(btn(icon("arrowDown", 14), { sm: true }), F("FilesFindNext")), h("span.hx-muted.hx-small", F("FilesFindCount", { current: 2, total: 5 }))),
        h("div.hx-flex", input(null, { ph: F("FilesReplaceWith"), w: "w300" }), btn(F("FilesReplace"), { sm: true }), btn(F("FilesReplaceAll"), { sm: true }), h("span.hx-grow"), tip(btn(icon("x", 14), { sm: true }), F("FilesFindClose"), { right: true }))) : null;
      editor = tip(h("div.hx-editor", find,
        h("div.hx-code", h("div.hx-gutter", tx("1"), h("br"), tx("2"), h("br"), tx("3")), h("div", { style: { padding: "4px 12px", color: "#9fd" } }, tx("server-port=25565"), h("br"), tx("motd=A world for friends"), h("br"), tx("view-distance=10"))),
        s.role === "user" ? h("div", { style: { position: "absolute", right: "16px", top: "10px" } }, tag(F("FilesReadOnly"))) : null), F("FilesEditorTitle"), { block: true });
    }
    const menu = f === "menu" ? h("div.hx-menu", { style: { left: "480px", top: "220px" } },
      h("div.hx-mi", M("Copy")), h("div.hx-mi", F("FilesCut")), h("div.hx-mi.hx-dis", F("FilesPaste")), h("div.hx-mi", F("FilesPasteInto", { name: "macros" })), h("div.hx-mi", FP("FilesPasteCount", { count: 2 })), h("div.hx-sep"),
      h("div.hx-mi", { onclick: () => X.open("files-rename") }, M("Rename")), h("div.hx-mi.hx-red", { onclick: () => X.open("files-delete") }, M("Delete")), h("div.hx-sep"),
      h("div.hx-mi", F("FilesZipFile", { name: "eula.txt", archive: "eula.txt.zip" })), h("div.hx-mi", F("FilesZip")), h("div.hx-mi", F("FilesZipTo", { archive: "Archive.zip" })), h("div.hx-mi", FP("FilesZipFiles", { count: 3, archive: "Archive.zip" })),
      h("div.hx-mi", F("FilesUnzipHere")), h("div.hx-mi", F("FilesUnzipHereSmart")), h("div.hx-mi", F("FilesUnzipTo", { name: "world" })), h("div.hx-sep"),
      h("div.hx-mi", M("ShowInFileManager")), h("div.hx-mi", F("FilesShowInFolder")), h("div.hx-sep"),
      h("div.hx-mi", { onclick: () => X.open("files-new-folder") }, F("FilesNewFolder")), h("div.hx-mi", { onclick: () => X.open("files-new-file") }, F("FilesNewFile"))) : null;
    return pageWrap(heading(S("TabFiles")), toolbar,
      h("div.hx-files", list, tip(h("div", { style: { width: "4px", cursor: "col-resize", background: "var(--g700)" } }), F("FilesResize")), editor, menu),
      h("div.hx-between", { style: { padding: "14px 20px" } }, h("div.hx-flex.hx-muted", FP("FilesSelected", { count: 1 }), tx("·"), FP("FilesInClipboard", { count: 2 })), check(true, F("FilesWrapLines"))));
  }

  // --- mods
  function modsTabs(cur) {
    return h("div.hx-tabs",
      h("div", { class: "hx-tab" + (cur === "installed" ? " hx-on" : ""), onclick: () => X.go("mods", { i: sel().i }) }, I("ModsInstalledView")),
      h("div", { class: "hx-tab" + (cur === "browse" ? " hx-on" : ""), onclick: () => X.go("mods-browse", { i: sel().i }) }, I("ModsBrowseView")));
  }
  const modsHeading = () => heading(isPlugins() ? S("TabPlugins") : S("TabMods"));
  function modsBlocked() {
    const s = st();
    if (!supportsMods() || s.mods === "unsupported") return pageWrap(heading(S("TabMods")), h("div.hx-muted", { style: { fontSize: "16px" } }, I("ModsUnsupported")));
    if (s.mods === "noread") return pageWrap(modsHeading(), h("div.hx-muted", { style: { fontSize: "16px" } }, I("ModsNoRead")));
    return null;
  }
  function modsPage() {
    const blocked = modsBlocked(); if (blocked) return blocked;
    const s = st(); const pl = isPlugins();
    const tools = h("div", { style: { display: "flex", justifyContent: "flex-end", gap: "10px", marginBottom: "16px" } },
      btn(I("ModsUpdateAll", { count: 2 }), { lg: true, v: "primary" }), btn(I("ModsCheckUpdates"), { lg: true }), tip(btn(I("ModsUpload"), { lg: true, icon: "upload", dis: s.role === "user" }), s.role === "user" ? I("ModsNoWrite") : I("ModsUploadPrompt"), { right: true }));
    const body = [modsTabs("installed"), tools];
    if (s.mods === "restart" || s.mods === "installed") body.push(h("div.hx-note.hx-yellow.hx-flex", { style: { marginBottom: "16px" } }, icon("refresh", 15, "var(--yellow)"), pl ? I("ModsRestartPlugin") : I("ModsRestartMod")));
    if (s.mods === "failed") { body.push(h("div.hx-inlineerr", pl ? I("ModsLoadFailedPlugin", { error: "Server not found" }) : I("ModsLoadFailedMod", { error: "Server not found" }))); return pageWrap(modsHeading(), body) }
    if (s.mods === "empty") { body.push(h("div.hx-card.hx-pad.hx-muted", pl ? I("ModsEmptyPlugin") : I("ModsEmptyMod"))); return pageWrap(modsHeading(), body) }
    if (!pl) body.push(h("div.hx-card.hx-pad.hx-col", { style: { marginBottom: "18px" } },
      h("div.hx-between", h("div.hx-col.hx-gap4", h("div", { style: { fontWeight: 600, fontSize: "17px" } }, IP("ModsPackTitle", { count: 4 })), h("div.hx-muted.hx-small", I("ModsLoader", { version: "0.16.9" }), tx(" · "), I("ModsLoaderNoVersion"))),
        btn(I("ModsDownloadPack"), { v: "primary", icon: "download" })),
      h("div.hx-muted.hx-small", IP("ModsImportHintJars", { count: 1 })),
      h("div.hx-muted.hx-small", I("ModsImportHint")),
      h("div.hx-muted.hx-small", I("ModsPlainFabric", { gameVersion: "1.21.4", loader: "Fabric Loader 0.16.9" }))));
    const mods = pl
      ? [["EssentialsX", "2.21.0", "server"], ["LuckPerms", "5.4.141", "server", "5.4.145"], ["WorldEdit", "7.3.9", "unknown"]]
      : [["Fabric API", "0.110.5", "server"], ["Sodium", "0.6.5", "players", "0.6.6"], ["Create", "6.0.4", "players"], ["Lithium", "0.14.3", "unknown"], ["Cloth Config", "15.0.140", "needed"]];
    body.push(h("div.hx-between", { style: { marginBottom: "8px" } }, h("span.hx-muted.hx-small", pl ? IP("ModsCountPlugin", { count: 3, directory: "plugins" }) : IP("ModsCountMod", { count: 5, directory: "mods" }))));
    body.push(h("div.hx-card.hx-rows", mods.map(([name, ver, side, upd], i) => {
      const sideTag = side === "server" ? tag(I("ModsServerOnly")) : side === "players" ? tag(I("ModsPlayersNeed"), "blue") : side === "needed" ? tag(I("ModsNeededBy", { mods: "Create, Sodium" }), "blue") : tip(tag(I("ModsUnknown"), "yellow"), i === 3 ? I("ModsUnknownNotOnModrinth") : I("ModsUnknownModrinth"));
      return h("div.hx-srow", h("div.hx-modicon", { style: { background: ["#3a6", "#c73", "#36a", "#a36", "#678"][i % 5] } }, tx(name[0])),
        h("div.hx-st", h("div.hx-stt", tx(name), sideTag), h("div.hx-sth.hx-mono", tx(name.toLowerCase().replace(/ /g, "-") + "-" + ver + ".jar"))),
        h("div.hx-sc", upd ? btn(I("ModsUpdateTo", { version: upd }), { sm: true, v: "primary" }) : null, tip(btn(icon("ext", 14), { sm: true }), I("ModsViewOnModrinth")), toggle(i !== 3), tip(btn(icon("trash", 14), { sm: true, on: () => X.open("mods-delete") }), I("ModsDeleteLabel", { name }), { right: true })));
    })));
    return pageWrap(modsHeading(), body);
  }
  function modsBrowsePage() {
    const blocked = modsBlocked(); if (blocked) return blocked;
    const s = st(); const pl = isPlugins();
    const results = pl
      ? [["EssentialsX", "EssentialsX", "The essential plugin suite for Minecraft servers.", 4.1, true], ["LuckPerms", "Luck", "A permissions plugin for Minecraft servers.", 3.2], ["Chunky", "pop4959", "Pre-generates chunks, quickly and efficiently.", 2.7]]
      : [["Sodium", "jellysquid3", "The fastest and most compatible rendering optimization mod for Minecraft.", 72.1, true], ["Lithium", "jellysquid3", "No-compromises game logic optimization mod.", 41.3], ["Create", "simibubi", "Aesthetic technology that empowers the player.", 18.6]];
    const body = [modsTabs("browse"),
      h("div.hx-flex", { style: { marginBottom: "12px" } }, tip(input(null, { ph: pl ? I("ModsSearchPlugin") : I("ModsSearchMod"), icon: "search", style: { flex: 1 } }), I("ModsSearchLabel"), { block: true })),
      h("div.hx-between", { style: { marginBottom: "14px" } }, check(true, I("ModsOnlyCompatible", { gameVersion: "1.21.4", loader: pl ? "Paper" : "Fabric" })),
        h("div.hx-flex", h("span.hx-muted", I("ModsSortBy")), input(I("ModsSortRelevance"), { select: true, w: "w240" })))];
    if (s.browse === "failed") body.push(h("div.hx-inlineerr", I("ModsSearchFailed", { error: "Modrinth didn't answer" })));
    else if (s.browse === "none") body.push(h("div.hx-card.hx-pad.hx-muted", I("ModsNoResults")));
    else if (s.browse === "link") body.push(h("div.hx-card.hx-pad.hx-col", loading(I("ModsOpening")), h("div.hx-err-t", I("ModsLinkFailed", { error: "not found" })), btn(I("ModsOpenLink", { slug: "sodium" }), { sm: true })));
    else {
      body.push(h("div.hx-col", results.map(([n, a, d, dl, inst]) => h("div.hx-result", { onclick: () => X.go("mods-project", { i: sel().i }) },
        h("div.hx-modicon", tx(n[0])),
        h("div.hx-grow.hx-col.hx-gap4", h("div.hx-flex", h("span", { style: { fontSize: "17px", fontWeight: 500 } }, tx(n)), h("span.hx-muted.hx-small", I("ModsByAuthor", { author: a })), inst ? tag(I("ModsInstalledTag"), "green") : null),
          h("div", tx(d)), h("div.hx-muted.hx-small.hx-flex", { style: { gap: "4px" } }, icon("download", 12), IP("ModsDownloads", { count: Math.round(dl * 1e6) }), tx(" · "), FM("CompactMillions", { value: dl })))))));
      body.push(h("div.hx-flex", { style: { justifyContent: "center", marginTop: "16px" } }, tip(btn(icon("back", 14), { sm: true }), I("ModsPreviousPage")), h("span.hx-muted", I("ModsPage", { page: 1, total: 12 })), tip(btn(icon("chevR", 14), { sm: true }), I("ModsNextPage"))));
    }
    return pageWrap(modsHeading(), body);
  }
  function modsProjectPage() {
    const blocked = modsBlocked(); if (blocked) return blocked;
    const s = st(); const pl = isPlugins();
    if (s.project === "failed") return pageWrap(modsHeading(), btn(M("Back"), { icon: "back", on: () => X.go("mods-browse", { i: sel().i }) }), h("div.hx-inlineerr", { style: { marginTop: "14px" } }, I("ModsProjectFailed", { error: "Not found" })));
    const kind = s.project;
    let notice = null;
    if (kind === "modpack") notice = h("div.hx-note.hx-col", tx(""), s.role === "owner" ? btn(I("ModsCreateFromModpack"), { v: "primary", on: () => X.open("create-version") }) : h("span", I("ModsModpackNoPermission")));
    else if (kind === "clientonly") notice = h("div.hx-note.hx-yellow", I("ModsNoServerSupport"));
    else if (kind === "wrongtype") notice = h("div.hx-note.hx-yellow", pl ? I("ModsWrongTypePlugin", { type: "mod" }) : I("ModsWrongTypeMod", { type: "resource pack" }));
    const verRow = (v, chan, gv, more, status) => h("div.hx-srow",
      h("div.hx-st", h("div.hx-stt", tx(v), chan === "release" ? tag(I("ModsRelease"), "green") : chan === "beta" ? tag(I("ModsBeta"), "yellow") : tag(I("ModsAlpha"), "red")),
        h("div.hx-sth", tx(gv), more ? h("span", tx(" "), IP("ModsMoreGameVersions", { count: more })) : null)),
      h("div.hx-sc", status === "installed" ? tag(I("ModsInstalledTag"), "green") : status === "notfor" ? h("span.hx-flex", h("span.hx-warn-t", I("ModsNotFor", { loader: "Fabric", gameVersion: "1.21.4" })), btn(M("Install"), { sm: true, on: () => X.open("mods-incompatible") })) : btn(M("Install"), { sm: true, v: "primary" })));
    return pageWrap(
      h("div", { style: { marginBottom: "14px" } }, btn(M("Back"), { icon: "back", on: () => X.go("mods-browse", { i: sel().i }) })),
      h("div.hx-flex.hx-gap16", h("div.hx-modicon", { style: { width: "72px", height: "72px", fontSize: "30px" } }, tx("S")),
        h("div.hx-col.hx-gap4", h("div.hx-flex", h("span", { style: { fontFamily: "Chillax", fontSize: "32px", fontWeight: 500 } }, tx("Sodium")), h("span.hx-muted", I("ModsByAuthor", { author: "jellysquid3" }))),
          h("div.hx-flex.hx-muted.hx-small", IP("ModsDownloads", { count: 72100000 }), tx("·"), IP("ModsFollowers", { count: 21345 }), tx("·"), I("ModsServerSide", { value: I("ModsSideOptional") }),
            tag(I("ModsPlayersNeedToo"), "blue")),
          h("div.hx-flex.hx-small", h("span.hx-link", I("ModsSource")), h("span.hx-link", I("ModsIssues")), h("span.hx-link", I("ModsWiki")), h("span.hx-link", I("ModsViewOnModrinth"))))),
      notice ? h("div", { style: { marginTop: "16px" } }, notice) : null,
      h("div.hx-grid2", { style: { marginTop: "26px", gridTemplateColumns: "1.2fr 1fr", gap: "30px", alignItems: "start" } },
        h("div.hx-col", h("div.hx-h3", I("ModsDescription")), h("div", tx("Sodium is a powerful rendering engine for Minecraft which improves frame rates and reduces lag spikes.")),
          h("div.hx-muted.hx-small", tx("Server side: "), I("ModsSideRequired"), tx(" / "), I("ModsSideUnsupported"), tx(" / "), I("ModsSideUnknown")),
          h("div.hx-muted.hx-italic", I("ModsRestOnModrinth"))),
        h("div.hx-col", h("div.hx-h3", I("ModsVersions")),
          check(true, I("ModsWithDependencies")), check(false, I("ModsShowAll")),
          s.versions === "none" ? h("div.hx-muted", I("ModsNoVersionsShowAll", { loader: "Fabric", gameVersion: "1.21.4" })) :
            s.versions === "noneall" ? h("div.hx-muted", I("ModsNoVersions", { loader: "Fabric", gameVersion: "1.21.4" })) :
              s.versions === "failed" ? h("div.hx-err-t", I("ModsVersionsFailed", { error: "timed out" })) :
                h("div.hx-card.hx-rows", verRow("mc1.21.4-0.6.6-fabric", "release", "1.21.4", 0, "install"), verRow("mc1.21.4-0.6.5-fabric", "release", "1.21.4", 2, "installed"), verRow("mc1.21.5-0.7.0-beta.1", "beta", "1.21.5", 0, "notfor"), verRow("mc1.21.5-0.7.0-alpha.3", "alpha", "1.21.5", 0, "notfor")),
          btn(IP("ModsShowMore", { count: 24 }), { sm: true }))));
  }

  function eventLogsPage() {
    const s = st();
    let body;
    if (s.eventlogs === "empty") body = h("div.hx-card.hx-pad.hx-muted", { style: { fontSize: "16px" } }, I("EventLogsEmpty"));
    else if (s.eventlogs === "failed") body = h("div.hx-inlineerr", I("EventLogsFailed", { error: "connection reset" }));
    else {
      const ev = (txt, when, by) => h("div.hx-srow", h("div.hx-st", h("div", txt, by ? h("span.hx-muted.hx-small", tx(" "), I("EventBy", { user: by })) : null)), h("div.hx-sc.hx-muted.hx-small", when));
      body = h("div.hx-card.hx-rows",
        ev(I("EventState", { state: S("StateRunning") }), FM("TodayAt", { time: "12:00" }), "peter"),
        ev(I("EventJoined", { names: "Alex, Sam" }), FM("TodayAt", { time: "12:03" })),
        ev(I("EventLeft", { names: "Sam" }), FM("TodayAt", { time: "12:40" })),
        ev(I("EventState", { state: S("StateStopping") }), FM("YesterdayAt", { time: "23:10" }), "peter"),
        ev(I("EventState", { state: S("StateStopped") }), FM("YesterdayAt", { time: "23:10" })),
        ev(I("EventState", { state: S("StateError") }), FM("ShortDate", { month: FM("November"), day: 2 })),
        ev(I("EventState", { state: S("StateStarting") }), FM("ShortDate", { month: FM("November"), day: 2 })));
    }
    return pageWrap(heading(S("TabEventLogs")), h("div.hx-muted", { style: { marginBottom: "14px" } }, I("EventLogsLead")), h("div", { style: { maxWidth: "866px" } }, body));
  }

  function macrosPage() {
    return pageWrap(heading(S("TabMacros")), h("div.hx-muted", { style: { fontSize: "16px", padding: "10px 20px" } }, S("NeedsScripting")),
      h("div.hx-muted", { style: { fontSize: "16px", padding: "10px 20px" } }, C("NoScripting")));
  }
  function notBuiltPage() {
    return pageWrap(h("div.hx-muted", { style: { fontSize: "16px" } }, S("NotBuilt")), h("div.hx-sec", heading(S("InstanceNotFound"))), h("div.hx-muted", N("NotFound")));
  }

  // ---------------------------------------------------------------- dialogs
  // Settings dialog
  const SET_PAGES = [["general", "server", () => S("SettingsGeneral")], ["users", "users", () => S("SettingsUsers")], ["profile", "user", () => S("SettingsProfile")], ["updates", "up", () => C("UpdatesTitle")]];
  function settingsDialog(page) {
    const s = st();
    const navRow = (k, ic, label) => h("div", { class: "hx-row" + (page === k || (page === "user" && k === "users") ? " hx-sel" : ""), onclick: () => X.open("settings-" + k) }, icon(ic, 17), h("span.hx-grow", label()));
    const nav = h("div.hx-snavd",
      h("div.hx-flex", { style: { padding: "0 12px 4px", gap: "14px" } }, h("span", { class: "hx-avatar" + (userName() === "peter" ? " hx-violet" : "") }, tx(userName()[0].toUpperCase())),
        h("div.hx-col.hx-gap4", { style: { gap: "2px" } }, h("div", { style: { fontSize: "18px", fontWeight: 500 } }, tx(userName())), h("div.hx-muted.hx-small", tx(coreName())))),
      h("div.hx-sgcap", S("SettingsCore")), navRow(...SET_PAGES[0]), navRow(...SET_PAGES[1]),
      h("div.hx-sgcap", S("SettingsAccount")), navRow(...SET_PAGES[2]),
      h("div.hx-sgcap", tx("Lodestone Host")), navRow(...SET_PAGES[3]),
      h("div.hx-grow"), h("div.hx-muted.hx-tiny", { style: { padding: "0 12px" } }, tx("Lodestone Host 0.6.0")));
    const title = { general: () => S("SettingsGeneral"), users: () => S("SettingsUsers"), user: () => C("Permissions"), profile: () => S("SettingsProfile"), updates: () => C("UpdatesTitle") }[page]();
    const body = { general: generalPage, users: usersPage, user: userPage, profile: profilePage, updates: updatesPage }[page]();
    return h("div", { class: "hx-dlg hx-settings" },
      tip(h("div.hx-x", { onclick: close }, icon("x", 18)), M("Close"), { right: true }),
      nav, h("div.hx-spage", { "data-mk-scroll": "spage" }, h("div.hx-h2t", title), body));
  }

  function generalPage() {
    const s = st(); const owner = isOwner();
    const ro = !owner;
    const remoteOn = s.remote === "on";
    const remoteToggle = () => { if (remoteOn) X.open("remote-off"); else if (s.owner === "nopassword") X.open("owner-account"); else X.set("remote", "on") };
    const pub = s.listPublic === "on";
    const flags = h("div.hx-flex.hx-gap16", { style: { marginTop: "8px" } }, check(true, C("FlagStatus")), check(true, C("FlagAddress")), check(false, C("FlagClientPack")));
    const shown = pub ? C("ShownText", { list: h("span", C("ShownNameVersion"), tx(", "), C("ShownStatus"), tx(", "), C("ShownAddress"), tx(", "), C("ShownClientPack")) }) : C("TurnOnFirst");
    const codeHint = s.codeExec === "javaonly" ? C("CodeExecutionHintJavaOnly") : C("CodeExecutionHint");
    return h("div.hx-col.hx-gap16", { style: { gap: "34px" } },
      ro ? h("div.hx-note.hx-yellow", C("ReadOnly")) : null,
      ssec(C("SectionCore"), null, rows(
        srow(C("CoreName"), s.validation === "on" ? h("span.hx-err-t", C("CoreNameTooLong", { max: 32 })) : null, input(coreName(), { w: "w300", err: s.validation === "on" })),
        srow(C("Domain"), h("span", C("DomainHint"), s.validation === "on" ? h("div.hx-err-t", C("DomainTooLong", { max: 253 })) : null), input(null, { ph: tx("127.0.0.1"), w: "w300" })),
        srow(C("RemoteAccess"), remoteOn ? C("RemoteAccessHintOn") : C("RemoteAccessHintOff"), toggle(remoteOn, remoteToggle, ro)))),
      ssec(C("SectionPublic"), C("SectionPublicLead"), rows(
        srow(C("ListPublicly"), C("ListPubliclyHint"), toggle(pub, () => X.set("listPublic", pub ? "off" : "on"), ro)),
        noInstances() ? srow(h("span.hx-muted", C("NoJavaServers"))) :
          srow(tx("Survival SMP"), pub ? shown : C("HiddenWhileOff", { shown: C("NotShown") }), h("div.hx-flex", h("span.hx-muted", C("ListingPublic")), toggle(false, null, !pub)), { below: flags }))),
      ssec(C("SectionInfo"), null, rows(
        srow(C("InfoCoreVersion"), null, tx("0.5.1")), srow(C("InfoHostVersion"), null, tx("0.6.0")), srow(C("InfoOs"), null, tx("macos")), srow(C("InfoArch"), null, tx("aarch64")),
        srow(C("InfoCpu"), null, tx("Apple M4 Pro")), srow(C("InfoCpuCores"), null, tx("14")), srow(C("InfoTotalRam"), null, s.ram === "none" ? C("InfoNoRam") : FM("Gigabytes", { size: 48 })),
        srow(C("InfoHttps"), null, s.tls === "none" ? C("TlsNone") : s.tls === "own" ? C("TlsUserProvided") : C("TlsSelfSigned")),
        s.tls === "none" ? null : srow(C("InfoFingerprint"), h("span.hx-mono.hx-tiny", tx("3F:9A:21:7C:0B:D4:55:E8:AA:19:C3:70:6E:42:9B:11:F0:8D:2C:67:B5:3E:94:0A:DD:71:58:C6:1F:83:2B:E9")), btn(M("Copy"), { sm: true })))),
      ssec(C("SectionDanger"), C("SectionDangerLead"), rows(
        srow(C("SafeMode"), C("SafeModeHint"), toggle(s.safeMode === "on", () => (s.safeMode === "on" ? X.open("safe-mode-off") : X.set("safeMode", "on")), ro)),
        srow(C("CodeExecution"), h("span", codeHint, s.codeExec === "locked" ? h("div.hx-warn-t", C("CodeExecutionLocked")) : null, s.codeExec === "javaonly" ? h("div.hx-muted", C("NoScripting")) : null),
          toggle(s.codeExec === "on" || s.codeExec === "javaonly", () => X.open("code-exec-off"), ro || s.codeExec === "locked")),
        srow(C("LanHttp"), C("LanHttpHint"), toggle(s.lanHttp === "on", () => (s.lanHttp === "on" ? X.set("lanHttp", "off") : X.open("lan-http")), ro)),
        srow(C("LocalClaim"), s.owner === "nopassword" ? h("span", C("LocalClaimHint"), h("div.hx-warn-t", C("LocalClaimNeedsPassword"))) : C("LocalClaimHint"), toggle(true, null, ro || s.owner === "nopassword")),
        srow(C("OpenPort"), h("span", C("OpenPortHint", { upnp: "UPnP" }), s.validation === "on" ? h("div.hx-err-t", C("PortRange")) : h("div.hx-ok-t", C("PortOpened", { port: 25565 }))),
          ro ? h("span.hx-muted", C("OpenPortNoPermission")) : h("div.hx-flex", tip(input("25565", { style: { width: "110px" }, err: s.validation === "on" }), C("OpenPortPort")), btn(C("OpenPortButton"), { sm: true }))))));
  }

  function usersPage() {
    const s = st();
    if (!isOwner()) return h("div.hx-col", h("div.hx-muted", C("UsersDescription")), h("div.hx-card.hx-pad.hx-muted", C("UsersOwnerOnly")));
    const users = s.users === "several" ? [["peter", true, "owner"], ["jordan", false, "admin"], ["sam", false, ""]] : [["peter", true, "owner"]];
    return h("div.hx-col.hx-gap16",
      h("div.hx-muted", { style: { fontSize: "16px" } }, C("UsersDescription")),
      h("div.hx-between", { style: { marginTop: "14px" } }, h("div", { style: { fontFamily: "Chillax", fontSize: "19px", fontWeight: 500 } }, C("AllMembersCount", { count: users.length })),
        btn(C("CreateUser"), { icon: "plus", lg: true, on: () => X.open("create-user") })),
      s.users === "loading" ? loading(C("LoadingUsers")) :
        h("div.hx-card.hx-rows", users.map(([n, me, role], i) => h("div.hx-srow", { style: { position: "relative" } },
          h("span.hx-avatar.hx-sm", { style: { background: ["#9b4d97", "#3b8fd9", "#3DAE5E"][i] } }, tx(n[0].toUpperCase())), h("span.hx-grow", { style: { fontSize: "16px" } }, tx(n)),
          me ? tag(C("You")) : null, role === "owner" ? tag(C("Owner")) : role === "admin" ? tag(C("Admin")) : null,
          h("span.hx-link", { onclick: () => X.open("settings-user") }, M("Open")),
          tip(btn(icon("dots", 16), { sm: true, on: () => X.set("userMenu", s.userMenu === "open" ? "closed" : "open") }), M("MoreActions"), { right: true }),
          s.userMenu === "open" && i === 0 ? h("div.hx-menu", { style: { right: "20px", top: "50px" } }, h("div.hx-mi", { onclick: () => X.open("change-password") }, C("ChangePasswordMenu")), tip(h("div.hx-mi.hx-dis", C("DeleteUser")), C("CantDeleteSelf"))) : null,
          s.userMenu === "open" && i === 1 ? h("div.hx-menu", { style: { right: "20px", top: "50px" } }, h("div.hx-mi", { onclick: () => X.open("change-password") }, C("ChangePasswordMenu")), h("div.hx-mi.hx-red", { onclick: () => X.open("delete-user") }, C("DeleteUser"))) : null))),
      h("div.hx-muted.hx-small", C("AllMembers"), tx(" · "), C("AllUsers")));
  }

  function userPage() {
    const s = st();
    const perm = (name, desc, on) => h("div.hx-perm", h("div.hx-st.hx-grow", h("div", { style: { fontWeight: 500, fontSize: "15px" } }, name), h("div.hx-muted.hx-small", desc)), toggle(on));
    const safe = s.safeMode === "on";
    return h("div.hx-col", { style: { gap: "26px" } },
      h("div", btn(M("Back"), { icon: "back", on: () => X.open("settings-users") })),
      h("div.hx-flex", h("span.hx-avatar", { style: { background: "#3b8fd9" } }, tx("J")), h("span", { style: { fontSize: "20px", fontWeight: 600 } }, tx("jordan")), tag(C("Admin"))),
      ssec(C("Permissions"), C("PermissionsLead", { username: "jordan" }), h("div.hx-card.hx-rows",
        perm(C("PermCreateInstance"), C("PermCreateInstanceUser"), true),
        perm(C("PermDeleteInstance"), C("PermDeleteInstanceUser"), false),
        perm(C("PermManagePermission"), C("PermManagePermissionUser"), false))),
      ssec(tx("Survival SMP"), null, h("div.hx-card.hx-rows",
        perm(C("PermViewInstance"), C("PermViewInstanceUser"), true), perm(C("PermStartInstance"), C("PermStartInstanceUser"), true), perm(C("PermStopInstance"), C("PermStopInstanceUser"), true),
        perm(C("PermConsole"), C("PermConsoleUser"), false), perm(C("PermInstanceSettings"), C("PermInstanceSettingsUser"), false),
        perm(C("PermReadResource"), C("PermReadResourceUser"), true), perm(C("PermWriteResource"), C("PermWriteResourceUser"), false),
        perm(C("PermReadFiles"), C("PermReadFilesUser"), false), perm(C("PermWriteFiles"), C("PermWriteFilesUser"), false)),
        noInstances() ? h("div.hx-muted", C("NoServersYet")) : null),
      ssec(C("UnsafePermissions"), safe ? C("UnsafePermissionsLead") : null, h("div.hx-card.hx-rows", { style: safe ? { opacity: ".6" } : null },
        perm(C("PermReadGlobalFile"), C("PermReadGlobalFileUser"), false), perm(C("PermWriteGlobalFile"), C("PermWriteGlobalFileUser"), false),
        perm(C("OwnerOnly"), h("span", C("OwnerOnlyHint"), s.codeExec === "javaonly" ? h("div", C("NoScripting")) : null), false))),
      apiKeysSection("jordan"),
      mcSection("jordan"));
  }

  function apiKeysSection(other) {
    const s = st();
    const lead = other ? C("ApiKeysUserLead", { username: other }) : C("ApiKeysOwnLead");
    let list;
    if (s.apiKeys === "loading") list = h("div.hx-card.hx-pad", loading(C("LoadingApiKeys")));
    else if (s.apiKeys === "password") list = h("div.hx-card.hx-pad.hx-muted", C("PasswordSessionOnly"));
    else if (s.apiKeys === "none") list = h("div.hx-card.hx-pad.hx-muted", other ? C("ApiKeysUserEmpty", { username: other }) : C("ApiKeysOwnEmpty"));
    else {
      const key = (name, scope, created, exp, used) => h("div.hx-srow", h("div.hx-st", h("div.hx-stt", tx(name), h("span.hx-mono.hx-muted.hx-tiny", tx("lsk_3f9a…c01d"))),
        h("div.hx-sth", scope), h("div.hx-sth.hx-flex", { style: { gap: "14px" } }, created, exp, used)), btn(C("Revoke"), { sm: true, v: "danger", on: () => X.open("api-revoke") }));
      list = h("div.hx-card.hx-rows",
        key("backup script", C("ScopeFull"), C("CreatedOn", { date: FM("ShortDate", { month: FM("October"), day: 3 }) }), C("ExpiresOn", { date: FM("ShortDateWithYear", { month: FM("January"), day: 1, year: 2027 }) }), C("LastUsed", { time: FMP("HoursAgo", { count: 3 }) })),
        key("launcher on laptop", C("ScopeLauncherConsole"), C("CreatedOn", { date: FM("Today") }), C("ExpiresNever"), C("LastUsedNever")),
        key("status bot", C("ScopeLauncherConsoleRead"), C("CreatedOn", { date: FM("Yesterday") }), C("ExpiredOn", { date: FM("ShortDate", { month: FM("September"), day: 30 }) }), C("LastUsed", { time: FM("JustNow") })),
        key("discord relay", C("ScopeLauncherConsoleWrite"), C("CreatedOn", { date: FM("ShortDate", { month: FM("August"), day: 12 }) }), C("ExpiresNever"), C("LastUsedNever")),
        key("dashboard", C("ScopeLauncher"), C("CreatedOn", { date: FM("ShortDate", { month: FM("July"), day: 1 }) }), C("ExpiresNever"), C("LastUsedNever")),
        key("custom", C("ScopeCustom", { servers: CP("ScopeServers", { count: 2 }), permissions: CP("ScopePermissions", { count: 5 }) }), C("CreatedOn", { date: FM("ShortDate", { month: FM("June"), day: 9 }) }), C("ExpiresNever"), C("LastUsedNever")));
    }
    return h("div.hx-col",
      h("div.hx-between", h("div", h("div.hx-h3", C("ApiKeys")), h("div.hx-sublead", lead)), other ? null : btn(C("CreateApiKey"), { icon: "plus", lg: true, on: () => X.open("api-create") })),
      list);
  }
  function mcSection(other) {
    const s = st();
    let list;
    if (s.mcAccounts === "tooold") list = h("div.hx-card.hx-pad.hx-muted", C("MinecraftTooOld"));
    else if (s.mcAccounts === "none") list = h("div.hx-card.hx-pad.hx-muted", other ? C("MinecraftUserEmpty", { username: other }) : C("MinecraftOwnEmpty"));
    else list = h("div.hx-card.hx-rows", ["Peter_mc", "PeterAlt"].map((n) => h("div.hx-srow", h("span.hx-avatar.hx-sm", { style: { borderRadius: "6px", background: "#7a5" } }, tx(n[0])), h("span.hx-grow", tx(n)), btn(M("Remove"), { sm: true, on: () => X.open("mc-remove") }))));
    return ssec(C("MinecraftAccounts"), other ? C("MinecraftUserLead", { username: other }) : C("MinecraftOwnLead"), list);
  }

  function profilePage() {
    const s = st();
    const nopw = s.owner === "nopassword" && isOwner();
    return h("div.hx-col", { style: { gap: "34px" } },
      ssec(C("Account"), null, rows(
        srow(C("ProfileUsername"), s.validation === "on" ? h("span.hx-err-t", C("UsernameRequired")) : null, input(userName(), { w: "w300" })),
        srow(C("ProfilePassword"), nopw ? C("NoPasswordYet") : null, btn(nopw ? C("SetPassword") : C("ChangePassword"), { lg: true, on: () => X.open(nopw ? "owner-account" : "change-password") })))),
      ssec(C("Appearance"), C("AppearanceLead"), rows(
        srow(C("Language"), null, input(C("LanguageAuto", { language: "English" }), { select: true, w: "w240" })),
        srow(C("Theme"), s.theme === "light" ? C("ThemeLightHint") : s.theme === "dark" ? C("ThemeDarkHint") : C("ThemeSystemHint"),
          h("div.hx-flex", ["system", "light", "dark"].map((th) => btn(th === "system" ? C("ThemeSystem") : th === "light" ? C("ThemeLight") : C("ThemeDark"), { sm: true, v: s.theme === th ? "primary" : null, on: () => X.set("theme", th) })))),
        srow(C("Zoom"), C("ZoomHint", { modifier: "⌘" }), h("div.hx-flex", btn(tx("−"), { sm: true }), h("span", { style: { fontSize: "17px", width: "50px", textAlign: "center" } }, FM("Percent", { value: 100 })), btn(tx("+"), { sm: true }), btn(M("Reset"), { sm: true }))),
        srow(C("ColoredConsole"), null, toggle(true)))),
      apiKeysSection(null),
      mcSection(null));
  }

  function updatesPage() {
    const s = st(); const u = s.update;
    let status; let action = null;
    if (u === "devbuild") status = C("UpdatesUnavailable");
    else if (u === "idle") { status = C("UpdateNotChecked"); action = btn(C("CheckForUpdates"), { sm: true }) }
    else if (u === "checking") status = C("UpdateChecking");
    else if (u === "uptodate") { status = C("UpToDate"); action = btn(C("CheckForUpdates"), { sm: true }) }
    else if (u === "available") { status = C("UpdateAvailable", { build: 742 }); action = btn(C("DownloadUpdate"), { sm: true, v: "primary" }) }
    else if (u === "downloading") status = C("UpdateDownloading", { build: 742, percent: FM("Percent", { value: 64 }) });
    else if (u === "ready") { status = C("UpdateReady", { build: 742 }); action = btn(C("RestartToUpdate"), { sm: true, v: "primary", on: () => X.open("restart-update") }) }
    else { status = h("span.hx-err-t", C("UpdateFailed", { error: "the download was interrupted" })); action = btn(C("CheckForUpdates"), { sm: true }) }
    const build = u === "devbuild" ? h("span", tx("Lodestone Host 0.6.0 · "), C("DevelopmentBuild")) : h("span", tx("Lodestone Host 0.6.0 · "), s.dev === "on" ? C("BuildNumberCommit", { build: 731, commit: "605928a6" }) : C("BuildNumber", { build: 731 }));
    const dis = u === "devbuild";
    const relayErr = s.validation === "on";
    return h("div.hx-col", { style: { gap: "34px" } },
      ssec(C("UpdatesTitle"), C("UpdatesLead"), rows(
        srow(C("ThisVersion"), h("span", status, h("div", { style: { marginTop: "10px" } }, build)), action),
        srow(C("UpdateAutomatically"), C("UpdateAutomaticallyHint"), toggle(!dis, null, dis)),
        srow(C("BetaBuilds"), C("BetaBuildsHint"), toggle(false, null, dis)),
        srow(C("DeveloperBuilds"), C("DeveloperBuildsHint"), toggle(s.dev === "on", null, dis)))),
      ssec(C("DeveloperSection"), null, rows(
        srow(C("ConnectService"), h("span", C("ConnectServiceHint"),
          s.relay === "custom" ? h("div.hx-warn-t", C("ConnectServiceInUse", { service: "https://relay.test:8443" })) : null,
          s.relay === "env" ? h("div.hx-warn-t", C("ConnectServiceEnv", { var: "LODESTONE_CONNECT_URL", service: "https://relay.test:8443" })) : null,
          relayErr ? h("div.hx-col.hx-gap4", h("div.hx-err-t", C("ConnectServiceScheme")), h("div.hx-err-t", C("ConnectServiceInsecure")), h("div.hx-err-t", C("ConnectServiceExtra")), h("div.hx-err-t", C("ConnectServiceAddress"))) : null),
          h("div.hx-flex", input(null, { ph: tx("https://connect.lodestone.cc"), w: "w300", err: relayErr }), btn(C("SaveAndRestartCore"), { v: "primary", on: () => X.open("restart-core") }), btn(M("Reset"), { sm: true }))))));
  }

  // Create server flow
  function createShell(step, steps, title, desc, content, footer) {
    const idx = steps.findIndex((s) => s[0] === step);
    const stepper = h("div.hx-steps", steps.map(([k, label], i) => [
      i ? h("div.hx-stepline") : null,
      h("div", { class: "hx-step" + (i === idx ? " hx-on" : "") + (i < idx ? " hx-done" : ""), onclick: () => X.open("create-" + k) }, h("span.hx-stepn", i < idx ? icon("check", 12) : tx(i + 1)), h("span", label())),
    ]));
    return h("div", { class: "hx-dlg hx-create" },
      h("div.hx-x", { onclick: close }, icon("x", 18)), stepper,
      h("div.hx-cmain", h("div", { style: { paddingRight: "50px" } }, h("div.hx-ct", title), desc ? h("div.hx-lead", { style: { margin: "6px 0 0" } }, desc) : null), h("div.hx-cscroll", content), h("div.hx-between", { style: { marginTop: "18px" } }, footer)));
  }
  const STEPS = [["games", () => N("StepSelectGame")], ["basic", () => N("StepBasicSettings")], ["server", () => N("StepServerSettings")], ["auto", () => N("StepAutoSettings")]];
  const MSTEPS = [["games", () => N("StepSelectGame")], ["modpack", () => N("StepSelectModpack")], ["version", () => N("StepSelectVersion")], ["mserver", () => N("StepServerSettings")]];
  const nav2 = (back, next, nextLabel, primary) => [back ? btn(M("Back"), { lg: true, on: () => X.open("create-" + back) }) : h("span"), btn(nextLabel || M("Next"), { lg: true, v: "primary", on: next ? () => X.open("create-" + next) : close, style: primary === false ? { opacity: ".6" } : null })];
  function createGames() {
    const s = st();
    const card = (g, name, desc, extra) => h("div", { class: "hx-gamecard" + (g === "vanilla" ? " hx-on" : ""), onclick: () => X.open(g === "modpack" ? "create-modpack" : "create-basic") }, h("div.hx-gt", gameIc(g, true), typeof name === "string" ? tx(name) : name, extra || null), h("div.hx-muted.hx-italic", { style: { fontSize: "15.5px" } }, desc));
    const content = s.createLoad === "loading" ? [loading(N("LoadingForm"))] : s.createLoad === "failed" ? [h("div.hx-err-t", N("GamesFailed", { error: "connection reset" })), h("div.hx-err-t", N("FormFailed", { error: "connection reset" }))] : [
      card("modpack", N("GameTypeModpack"), N("GameTypeModpackDescription"), h("span.hx-small", { style: { color: "var(--yellow)" } }, N("GameTypeNew"))),
      h("div.hx-grid2",
        card("vanilla", "Minecraft", N("GameVanillaDescription")), card("fabric", "Fabric (Minecraft)", N("GameFabricDescription")),
        card("forge", "Forge (Minecraft)", N("GameForgeDescription")), card("paper", "Paper (Minecraft)", N("GamePaperDescription")),
        card("paper", "Spigot (Minecraft)", N("GameSpigotDescription")), card("vanilla", "Bedrock (Minecraft)", N("GameBedrockDescription")),
        card("vanilla", "Minecraft: Quilt", N("GameOtherDescription", { name: "Quilt" })), card("forge", "Terraria", N("GameGenericDescription", { name: "Terraria" })))];
    return createShell("games", STEPS, N("GameTypeTitle"), N("GameTypeSubtitle"), content, nav2(null, "basic"));
  }
  function createBasic() {
    const s = st(); const err = s.validation === "on";
    return createShell("basic", STEPS, N("StepBasicSettings"), N("BasicSettingsDescription"), [rows(
      srow(N("FieldName"), h("span", N("FieldNameDescription"), err ? h("div.hx-err-t", N("ValidationRequired", { name: plain("host/instances/InstancesMsg/FieldName") })) : null), input(err ? "" : null, { ph: N("FieldNameDefault"), w: "w300", err })),
      srow(N("FieldDescription"), N("FieldDescriptionDescription"), input(null, { w: "w300" }), { badge: h("span.hx-muted.hx-small", N("Optional")) })),
      err ? h("div.hx-err-t", N("CheckFields")) : null], nav2("games", "server"));
  }
  function createServerStep() {
    const s = st(); const err = s.validation === "on";
    return createShell("server", STEPS, N("StepServerSettings"), N("ServerSettingsDescription"), [
      h("div.hx-muted", { style: { fontSize: "16px" } }, tx("Basic Settings")),
      rows(srow(tx("Version"), tx("The version of minecraft to use"), input("26.3", { select: true, w: "w300" })),
        srow(N("FieldPort"), h("span", N("FieldPortDescription"), err ? h("div.hx-err-t", N("ValidationInteger", { name: "Port" })) : null), input(err ? "255.65" : "25565", { w: "w300", err }))),
      h("div.hx-muted", { style: { fontSize: "16px" } }, tx("Advanced Settings")),
      rows(srow(N("FieldMinRam"), h("span", N("FieldMinRamDescription"), err ? h("div.hx-err-t", N("ValidationPositive", { name: "Minimum RAM" })) : null), input("1024", { w: "w300" })),
        srow(N("FieldMaxRam"), h("span", N("FieldMaxRamDescription"), err ? h("div.hx-err-t", N("MaxRamBelowMin")) : null), input("2048", { w: "w300", err })),
        srow(tx("Command Line Arguments"), s.role === "user" ? N("CommandLineOwnerOnly") : s.codeExec === "off" ? N("CommandLineReadOnly") : null, input("", { w: "w300" }), { badge: h("span.hx-muted.hx-small", N("Optional")) }),
        srow(tx("Difficulty"), err ? h("span.hx-err-t", N("ValidationOneOf", { name: "Difficulty" })) : null, input("Normal", { select: true, w: "w300" })),
        srow(tx("Max players"), err ? h("span.hx-err-t", N("ValidationMin", { name: "Max players", min: 1 })) : null, input("20", { w: "w300" })),
        srow(tx("View distance"), err ? h("span.hx-err-t", h("span", N("ValidationMax", { name: "View distance", max: 32 }), h("br"), N("ValidationNumber", { name: "View distance" }))) : null, input("10", { w: "w300" })),
        srow(tx("Hardcore Plus"), N("UnknownSetting"), null)),
    ], nav2("basic", "auto"));
  }
  function createAuto() {
    const s = st();
    return createShell("auto", STEPS, N("StepAutoSettings"), N("AutoSettingsDescription"), [rows(
      srow(N("FieldAutoStart"), N("FieldAutoStartDescription"), toggle(false)), srow(N("FieldRestartOnCrash"), N("FieldRestartOnCrashDescription"), toggle(false))),
    h("div.hx-grow"),
    h("div.hx-col.hx-gap4", check(s.eula === "accepted", N("EulaAgree", { link: h("span.hx-link.hx-u", N("EulaLink")) }), () => X.set("eula", s.eula === "accepted" ? "not" : "accepted")), h("div.hx-muted.hx-small", { style: { paddingLeft: "27px" } }, N("EulaRequired"))),
    s.validation === "on" ? h("div.hx-err-t", N("CreateFailed", { error: "port 25565 is in use" })) : null],
    nav2("server", null, N("CreateServer"), s.eula === "accepted"));
  }
  function createModpack() {
    const s = st();
    const res = (n, a, d, dl, on) => h("div", { class: "hx-result" + (on ? " hx-on" : ""), onclick: () => X.open("create-version") }, h("div.hx-modicon", { style: { width: "56px", height: "56px", background: "#c33" } }, tx(n[0])),
      h("div.hx-col.hx-gap4.hx-grow", h("div.hx-flex", h("span", { style: { fontSize: "17px", fontWeight: 500 } }, tx(n)), h("span.hx-muted.hx-small", N("ByAuthor", { author: a }))), h("div", tx(d)), h("div.hx-muted.hx-small", tx("⤓ " + dl))));
    let list;
    if (s.search === "loading") list = loading(N("SearchLoading"));
    else if (s.search === "failed") list = h("div.hx-col", h("div.hx-err-t", N("SearchFailed", { error: "Modrinth didn't answer" })), h("div.hx-err-t", N("SearchLinkFailed", { error: "not a Modrinth link" })));
    else if (s.search === "none") list = h("div.hx-muted", N("SearchNoResults"));
    else if (s.search === "link") list = h("div.hx-col", loading(N("SearchOpening")), btn(N("SearchOpenLink", { slug: "cobblemon-fabric" }), { sm: true }));
    else list = h("div.hx-col", res("Cobblemon Official Modpack [Fabric]", "CobbledStudios", "The official modpack of the Cobblemon mod, for Fabric!", "11.4M", true), res("COBBLEVERSE - Pokemon Adventure", "LUMYVERSE", "Start a true Pokémon adventure in Minecraft.", "7.2M"), res("Better MC [FABRIC] - BMC2", "SHXRKIE", "A Proper Vanilla+ Modpack.", "3.7M"),
      h("div.hx-flex", { style: { justifyContent: "center" } }, tip(btn(icon("back", 14), { sm: true }), N("PreviousPage")), h("span.hx-muted", N("SearchPage", { page: 1, total: 40 })), tip(btn(icon("chevR", 14), { sm: true }), N("NextPage"))),
      h("div.hx-muted.hx-small", tx("Sort: "), N("SortDownloads"), tx(" / "), N("SortFollows"), tx(" / "), N("SortNewest"), tx(" / "), N("SortUpdated")));
    const content = [
      h("div.hx-muted.hx-italic", { style: { marginTop: "-10px" } }, N("ModpackSourceDescription")),
      input(null, { ph: N("ModpackSearchPlaceholder"), icon: "search" }),
      h("div.hx-between", check(true, N("ModpackOnlyFabric")), h("div.hx-flex", h("span.hx-muted", N("SortLabel")), input(N("SortRelevance"), { select: true, w: "w240" }))),
      list,
      s.search === "failed" ? h("div.hx-col.hx-gap4", h("div.hx-err-t", N("ModpackUploadFailed", { error: "file too large" })), h("div.hx-err-t", N("ModpackNoUploadId")), h("div.hx-err-t", N("ModpackNotAModpack", { type: "mod" })), h("div.hx-muted", N("ModpackChooseMrpack")), loading(N("ModpackUploading", { name: "pack.mrpack" }))) : null,
    ];
    const sh = createShell("modpack", MSTEPS, N("ModpackSourceTitle"), null, content, [btn(M("Back"), { lg: true, on: () => X.open("create-games") }), h("span")]);
    sh.querySelector(".hx-cmain > div").append(h("div", { style: { position: "absolute", right: "90px", top: "48px" } }, btn(N("ModpackUpload"), { sm: true, icon: "upload" })));
    sh.querySelector(".hx-cmain").style.position = "relative";
    return sh;
  }
  function createVersion() {
    const s = st();
    const ver = (v, chan, on) => h("div", { class: "hx-result" + (on ? " hx-on" : "") }, h("div.hx-col.hx-gap4.hx-grow", h("div.hx-flex", h("span", { style: { fontWeight: 500 } }, tx(v)), chan === "release" ? tag(N("VersionRelease"), "green") : chan === "beta" ? tag(N("VersionBeta"), "yellow") : tag(N("VersionAlpha"), "red")), h("div.hx-muted.hx-small", tx("1.21.1 · Fabric"))),
      on ? tag(N("ModpackSelected"), "blue") : btn(N("ModpackSelect"), { sm: true }));
    let list;
    if (s.search === "loading") list = loading(N("ModpackLoadingVersions"));
    else if (s.search === "failed") list = h("div.hx-col", h("div.hx-err-t", N("ModpackVersionsFailed", { error: "timed out" })), h("div.hx-err-t", N("ModpackUnsupported", { reason: "it needs Forge" })), h("div.hx-err-t", N("ModpackInspectFailed", { error: "bad zip" })));
    else if (s.search === "none") list = h("div.hx-muted", N("ModpackNoVersions"));
    else list = h("div.hx-col", ver("1.6.1", "release", true), ver("1.6.0", "release"), ver("1.7.0-beta.2", "beta"), ver("1.7.0-alpha.1", "alpha"), btn(NP("ModpackShowMore", { count: 12 }), { sm: true }),
      h("div.hx-card.hx-pad", h("div.hx-kv", N("ModpackSummaryModpack"), tx("Cobblemon Official Modpack 1.6.1"), N("ModpackSummaryLoader"), tx("Fabric 0.16.9 · Minecraft 1.21.1"), N("ModpackSummaryServerFiles"), N("ModpackSummaryServerFilesValue", { count: 84, size: FM("Megabytes", { size: 212 }) }))),
      loading(N("ModpackDownloadingSize", { size: FM("Megabytes", { size: 18 }) })), loading(N("ModpackDownloading")), loading(N("ModpackLoading")));
    return createShell("version", MSTEPS, N("ModpackVersionTitle"), s.search === "uploaded" ? N("ModpackVersionUploaded") : N("ModpackVersionDescription"),
      [h("div.hx-muted.hx-small", N("ModpackVersionHint")), list], nav2("modpack", "mserver"));
  }
  function createModpackServer() {
    const s = st();
    return createShell("mserver", MSTEPS, N("ModpackSettingsTitle"), N("ServerSettingsDescription"), [rows(
      srow(N("FieldName"), N("FieldNameDescription"), input("Cobblemon", { w: "w300" })),
      srow(N("FieldPort"), N("FieldPortDescription"), input("25565", { w: "w300" })),
      srow(N("FieldMaxRam"), N("FieldMaxRamDescription"), input("6144", { w: "w300" })),
      srow(tx("Allowed mods"), h("span.hx-muted", N("NothingFound")), null)),
    h("div.hx-grow"),
    h("div.hx-col.hx-gap4", check(s.eula === "accepted", N("EulaAgree", { link: h("span.hx-link.hx-u", N("EulaLink")) }), () => X.set("eula", s.eula === "accepted" ? "not" : "accepted")), h("div.hx-muted.hx-small", { style: { paddingLeft: "27px" } }, N("EulaRequired")))],
    nav2("version", null, N("CreateServer")));
  }

  // simple confirmation dialogs
  const confirm = (title, body, okLabel, okKind, onOk) => dlg({ title, w: 620 }, h("div.hx-muted", { style: { fontSize: "15.5px" } }, body),
    foot(btn(okLabel, { v: okKind || "primary", lg: true, on: onOk || close }), btn(M("Cancel"), { lg: true, on: close })));

  function ownerAccountDlg() {
    const s = st(); const err = s.validation === "on";
    return dlg({ title: C("OwnerAccountTitle"), w: 636 },
      h("div.hx-muted", { style: { fontSize: "15.5px", color: "var(--g400)" } }, C("OwnerAccountLead")),
      field(C("Username"), input("peter"), null, err ? C("UsernameRequired") : null),
      field(C("Password"), input(null), null, err ? C("PasswordTooShort") : null),
      field(C("ConfirmPassword"), input(null), null, err ? C("PasswordsMustMatch") : null),
      err ? h("div.hx-col.hx-gap4", h("div.hx-err-t", C("PasswordRequired")), h("div.hx-err-t", C("ConfirmRequired"))) : null,
      foot(btn(C("OwnerAccountButton"), { v: "primary", lg: true, on: () => { X.set("owner", "password"); X.set("remote", "on"); X.close() } }), btn(M("Cancel"), { lg: true, on: close })),
      s.validation === "off" && s.owner === "password" ? h("div.hx-ok-t", C("OwnerAccountDone")) : null);
  }
  function createUserDlg() {
    return dlg({ title: C("CreateUserTitle"), w: 600 }, h("div.hx-muted", C("CreateUserLead")),
      field(C("Username"), input(null)), field(C("Password"), input(null)), field(C("ConfirmPassword"), input(null)),
      foot(btn(C("CreateUserButton"), { v: "primary", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function changePasswordDlg() {
    return dlg({ title: C("ChangePassword"), w: 600 },
      field(C("OldPassword"), input(null)), field(C("Password"), input(null)), field(C("ConfirmPassword"), input(null)),
      foot(btn(C("ChangePasswordButton"), { v: "primary", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function coresDlg(step) {
    const s = st();
    if (step === "trust") return dlg({ title: C("TrustTitle"), w: 760 },
      h("div.hx-muted", { style: { fontSize: "15px" } }, C("TrustBody", { host: "192.168.1.20" })),
      h("div.hx-label", C("FingerprintCaption")), h("div.hx-input.hx-mono", { style: { fontSize: "12.5px", whiteSpace: "normal" } }, tx("3F:9A:21:7C:0B:D4:55:E8:AA:19:C3:70:6E:42:9B:11:F0:8D:2C:67:B5:3E:94:0A:DD:71:58:C6:1F:83:2B:E9")),
      h("div.hx-muted.hx-small", C("FingerprintHelp")),
      foot(btn(C("TrustCertificate"), { v: "primary", lg: true, on: () => X.open("cores-signin") }), btn(M("Back"), { lg: true, on: () => X.open("cores") })));
    if (step === "signin") return dlg({ title: C("SignInTo", { name: "Home server" }), w: 640 },
      s.versionNote === "incompatible" ? h("div.hx-note.hx-yellow", C("VersionIncompatible", { theirs: "0.4.8", ours: "0.5.1" })) : s.versionNote === "patch" ? h("div.hx-note", C("VersionPatch", { theirs: "0.5.0", ours: "0.5.1" })) : null,
      field(C("Username"), input("peter"), null, s.validation === "on" ? C("UsernameRequired") : null),
      field(C("Password"), input(null), null, s.validation === "on" ? C("PasswordRequired") : null),
      foot(btn(C("SignIn"), { v: "primary", lg: true, on: () => { X.set("coreKind", "remote"); X.close() } }), btn(M("Back"), { lg: true, on: () => X.open("cores") })));
    const remote = s.coreKind === "remote";
    return dlg({ title: h("span", { style: { fontSize: "32px" } }, C("Cores")), w: 764 },
      h("div", { style: { fontSize: "16px", color: "var(--g400)" } }, C("CoresLead")),
      h("div.hx-card.hx-rows",
        srow(C("ThisComputerCore"), C("ThisComputerHint"), remote ? btn(C("UseCore"), { sm: true, on: () => X.set("coreKind", "local") }) : tag(C("CurrentCore"), "green")),
        s.remoteCores === "on" ? srow(tx("Home server"), h("span", tx("https://192.168.1.20:16662"),
          s.versionNote === "incompatible" ? h("div.hx-warn-t", C("VersionIncompatible", { theirs: "0.4.8", ours: "0.5.1" })) : s.versionNote === "patch" ? h("div.hx-muted", C("VersionPatch", { theirs: "0.5.0", ours: "0.5.1" })) : null),
          h("div.hx-flex", remote ? tag(C("CurrentCore"), "green") : btn(C("UseCore"), { sm: true, on: () => X.open("cores-signin") }), btn(C("RemoveCore"), { sm: true, v: "danger" }))) : null),
      h("div", { style: { fontSize: "19px", marginTop: "8px" } }, C("ConnectToCore")),
      field(C("CoreAddress"), input(null, { ph: tx("192.168.1.5") }), C("CoreAddressHint")),
      h("div", btn(C("Connect"), { v: "primary", lg: true, on: () => X.open("cores-trust") })));
  }
  function deleteInstanceDlg() {
    return dlg({ title: I("DeleteTitle", { name: sel().name }), w: 640 },
      h("div.hx-note.hx-red", h("div", { style: { fontWeight: 600 } }, I("DeleteCannotUndo")), h("div", I("DeleteWarning"))),
      check(false, I("DeleteConfirm")),
      foot(btn(M("Delete"), { v: "dangerfill", lg: true }), btn(M("Cancel"), { lg: true, on: close })),
      h("div.hx-muted.hx-small", I("Deleting", { name: sel().name })));
  }
  function apiCreateDlg() {
    const s = st(); const custom = s.keyAccess === "custom";
    const perm = (n, d, mine) => h("div.hx-perm", { style: { padding: "8px 0", opacity: mine ? 1 : 0.5 } }, box(mine && n !== null), h("div.hx-grow", h("div", n), h("div.hx-muted.hx-small", d)), mine ? null : tip(icon("lock", 14, "var(--g500)"), C("NotYours"), { right: true }));
    return dlg({ title: C("CreateApiKey"), w: 760 },
      h("div.hx-muted", C("CreateApiKeyLead")),
      field(C("ApiKeyName"), input(null, { err: s.validation === "on" }), null, s.validation === "on" ? h("span", C("ApiKeyNameRequired"), h("br"), C("ApiKeyNameTooLong", { max: 64 })) : null),
      field(C("ExpiresIn"), h("div.hx-flex", btn(C("Expiry30d"), { sm: true, v: "primary" }), btn(C("Expiry90d"), { sm: true }), btn(C("Expiry1y"), { sm: true }), btn(C("ExpiryNever"), { sm: true }))),
      h("div.hx-note.hx-yellow", C("NeverWarning")),
      field(C("Access"), h("div.hx-col", check(!custom, C("AccessFull"), () => X.set("keyAccess", "full")), check(custom, C("AccessCustom"), () => X.set("keyAccess", "custom")))),
      custom ? h("div.hx-col.hx-gap4",
        h("div.hx-muted.hx-small", C("OnlyYours")),
        perm(C("PermCreateInstance"), C("PermCreateInstanceKey"), true), perm(C("PermDeleteInstance"), C("PermDeleteInstanceKey"), false), perm(C("PermManagePermission"), C("PermManagePermissionKey"), true),
        perm(C("PermViewInstance"), C("PermViewInstanceKey"), true), perm(C("PermStartInstance"), C("PermStartInstanceKey"), true), perm(C("PermStopInstance"), C("PermStopInstanceKey"), true),
        perm(C("PermConsole"), C("PermConsoleKey"), true), perm(C("PermInstanceSettings"), C("PermInstanceSettingsKey"), true), perm(C("PermReadResource"), C("PermReadResourceKey"), true),
        perm(C("PermWriteResource"), C("PermWriteResourceKey"), true), perm(C("PermReadFiles"), C("PermReadFilesKey"), true), perm(C("PermWriteFiles"), C("PermWriteFilesKey"), true),
        h("div.hx-label", { style: { marginTop: "8px" } }, C("KeyUnsafe")), h("div.hx-muted.hx-small", C("KeyUnsafeSafeMode")),
        perm(C("PermReadGlobalFile"), C("PermReadGlobalFileKey"), false), perm(C("PermWriteGlobalFile"), C("PermWriteGlobalFileKey"), false),
        perm(C("KeyOwnerOnly"), C("KeyOwnerOnlyHint"), false),
        noInstances() ? h("div.hx-muted", C("NoServers")) : null,
        s.validation === "on" ? h("div.hx-err-t", C("ChooseOne")) : null) : null,
      foot(btn(C("CreateKey"), { v: "primary", lg: true, on: () => X.open("api-new") }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function apiNewDlg() {
    return dlg({ title: C("NewKeyTitle", { name: "backup script" }), w: 720 },
      h("div.hx-warn-t", { style: { fontSize: "15px" } }, C("NewKeyCopyNow")),
      h("div.hx-flex", input("lsk_3f9a21c70bd455e8aa19c3706e429b11f08d2c67b53e940add7158c6", { mono: true, style: { flex: 1 } }), btn(M("Copy"), { v: "primary" })),
      h("div", C("NewKeyUseWithCli")),
      h("div.hx-flex", h("div.hx-logbox", { style: { flex: 1 } }, tx("lodestone --core https://127.0.0.1:16662 --api-key lsk_3f9a… instances list")), btn(M("Copy"))),
      h("div.hx-muted.hx-small", C("NewKeyStore")),
      h("div.hx-flex", h("span.hx-ok-t", C("KeyCopied")), h("span.hx-ok-t", C("CliCopied"))),
      foot(btn(M("Done"), { v: "primary", lg: true, on: close })));
  }
  function eulaDlg() {
    return dlg({ title: I("EulaDialogTitle"), w: 620 }, h("div.hx-muted", { style: { fontSize: "15.5px" } }, I("EulaDialogBody", { name: sel().name })),
      h("div", btn(I("EulaRead"), { icon: "ext" })),
      foot(btn(I("EulaAcceptAndStart"), { v: "primary", lg: true, on: () => { X.set("eula", "accepted"); X.set("instance", "starting"); X.close() } }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function restartUpdateDlg() {
    return dlg({ title: C("RestartUpdateTitle"), w: 620 }, h("div", { style: { fontSize: "15.5px" } }, CP("ServersRunning", { count: 2 }), tx(" "), C("RestartUpdateBody")),
      foot(btn(C("RestartUpdateConfirm"), { v: "primary", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function restartCoreDlg() {
    return dlg({ title: C("RestartCoreTitle"), w: 620 }, h("div", { style: { fontSize: "15.5px" } }, CP("ServersRunning", { count: 1 }), tx(" "), C("RestartCoreBody")),
      foot(btn(C("SaveAndRestartCore"), { v: "primary", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function filesNameDlg(kind) {
    const s = st();
    const t1 = kind === "file" ? F("FilesNameFile") : kind === "folder" ? F("FilesNameFolder") : F("FilesRenameFile");
    const ph = kind === "folder" ? F("FilesNameFolderPlaceholder") : F("FilesNameFilePlaceholder");
    const ok = kind === "file" ? F("FilesCreateFile") : kind === "folder" ? F("FilesCreateFolder") : M("Rename");
    return dlg({ title: t1, w: 560 }, input(kind === "rename" ? "eula.txt" : null, { ph, focus: true, err: s.validation === "on" }),
      s.validation === "on" ? h("div.hx-err-t", F("FilesNameRequired")) : null,
      foot(btn(ok, { v: "primary", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function filesDeleteDlg() {
    return dlg({ title: FP("FilesDeleteTitle", { count: 3 }), w: 600 }, h("div", F("FilesDeleteMessage")),
      h("div.hx-col.hx-gap4.hx-mono.hx-small", h("div", tx("eula.txt")), h("div", tx("macros "), F("FilesDeleteDirectory")), h("div", tx("link.lnk "), F("FilesDeleteUnknownType"))),
      foot(btn(M("Delete"), { v: "dangerfill", lg: true, on: close }), btn(M("Cancel"), { lg: true, on: close })));
  }
  function filesUnsavedDlg() {
    return dlg({ title: M("UnsavedChanges"), w: 560 }, h("div", F("FilesSaveChanges", { name: "server.properties" })),
      foot(btn(M("Save"), { v: "primary", lg: true, on: close }), btn(M("Discard"), { lg: true, v: "danger", on: close }), btn(M("KeepEditing"), { lg: true, on: close })));
  }
  function modsDeleteDlg() {
    const pl = isPlugins();
    return confirm(pl ? I("ModsDeleteTitlePlugin") : I("ModsDeleteTitleMod"), I("ModsDeleteBody", { name: "Sodium", fileName: "sodium-0.6.5.jar" }), M("Delete"), "dangerfill");
  }
  function modsOverwriteDlg() {
    return confirm(I("ModsOverwriteTitle"), I("ModsOverwriteBody", { message: I("ModsOverwriteDefault") }), I("ModsOverwrite"));
  }
  function modsIncompatibleDlg() {
    return confirm(I("ModsIncompatibleTitle"), I("ModsIncompatibleBody", { title: "Sodium", version: "0.7.0-beta.1", loaders: "Fabric", gameVersions: "1.21.5", loader: "Fabric", gameVersion: "1.21.4" }), I("ModsInstallAnyway"), "danger");
  }

  // ---------------------------------------------------------------- messages
  const msgSec = (title, ...items) => h("div.hx-msgsec", h("div.hx-h3", { style: { marginBottom: "10px", fontSize: "17px" } }, tx(title)), h("div.hx-msglist", items));
  const msgPage = (...secs) => h("div.hx-pagein.hx-msgpage", secs);
  const routesMsgShell = () => msgPage(
    msgSec("Toasts", toast("err", C("CoreFailedNotice", { error: "failed to bind port 16662" })), toast("warn", C("RemoteSignedOut", { name: "Home server" })), toast("err", C("LoadFailed", { error: "connection reset" })), toast("warn", C("WarningToast")),
      toast("ok", N("CopiedText", { text: "localhost:25565" })), toast("ok", C("OwnerAccountDone")), toast("ok", C("ApiKeyCreated", { name: "backup script" })), toast("ok", C("ApiKeyRevoked", { name: "old laptop" })),
      toast("ok", C("MinecraftRemoved", { name: "PeterAlt" })), toast("ok", C("PortOpened", { port: 25565 })), toast("ok", C("KeyCopied")), toast("ok", C("CliCopied")), toast("ok", M("Copied")), toast("ok", M("Saved"))),
    msgSec("Sign-in form errors", h("div.hx-err-t", S("NoUsername")), h("div.hx-err-t", S("NoPassword"))),
    msgSec("Core settings validation", h("div.hx-err-t", C("CoreNameEmpty")), h("div.hx-err-t", C("CoreNameTooLong", { max: 32 })), h("div.hx-err-t", C("DomainTooLong", { max: 253 })), h("div.hx-err-t", C("PortRequired")), h("div.hx-err-t", C("PortNotNumber")), h("div.hx-err-t", C("PortRange"))),
    msgSec("App menu (macOS)", h("div.hx-tray", h("div.hx-mi", S("SettingsMenu")))),
    msgSec("Tray menu", h("div.hx-tray", h("div.hx-mi", S("ShowHost")), h("div.hx-sep", { style: { height: "1px", background: "#555", margin: "4px 0" } }), h("div.hx-mi", S("QuitHost")))),
    msgSec("Window title bar (Windows, Linux)", h("div.hx-flex", btn(M("MinimizeWindow"), { sm: true }), btn(M("MaximizeWindow"), { sm: true }), btn(M("RestoreWindow"), { sm: true }), btn(M("Close"), { sm: true }))),
    msgSec("Common buttons and words", h("div.hx-flex", { style: { flexWrap: "wrap" } },
      ["Add", "Apply", "Browse", "Change", "Continue", "Default", "Edit", "NotNow", "Ok", "Off", "On", "None", "OpenFolder", "Previous", "Refresh", "Search", "SignOut", "Update", "Menu", "MoveUp", "MoveDown", "Loading", "MoveToTrash", "OpenImage", "Keychain", "KeychainInFull", "TheTrash", "ThisComputer", "ThisComputerStart"].map((k) => btn(M(k), { sm: true }))),
      h("div.hx-muted", MP("Instances", { count: 1 }), tx(" · "), MP("Instances", { count: 3 }), tx(" · "), MP("Files", { count: 1 }), tx(" · "), MP("Files", { count: 4 })),
      h("div.hx-err-t", M("FileDialogUnavailable"))));
  const routesMsgFiles = () => msgPage(
    msgSec("Toasts", toast("info", FP("FilesUploading", { count: 1 })), toast("info", FP("FilesZipping", { count: 4 })), toast("info", F("FilesZippingForDownload")), toast("info", F("FilesUnzipping", { name: "world.zip" })),
      toast("ok", F("FilesDownloaded", { name: "world.zip" })), toast("ok", F("FilesDeletedFile", { name: "eula.txt" })), toast("ok", F("FilesDeletedFolder", { path: "logs/old" })),
      toast("ok", FP("FilesCopiedToClipboard", { count: 2 })), toast("ok", FP("FilesCutToClipboard", { count: 1 })),
      toast("err", F("FilesDownloadFailed", { name: "world", error: "disk full" })), toast("err", F("FilesMoveFailed", { name: "eula.txt", error: "already exists" })), toast("err", F("FilesZipFailed", { error: "permission denied" })),
      toast("err", F("FilesUnzipFailed", { name: "world.zip", error: "bad archive" })), toast("err", F("FilesSaveFailed", { name: "server.properties", error: "read only" })),
      toast("warn", F("FilesOnlyFilesUnzip")), toast("warn", F("FilesOnlyFilesUpload")), toast("warn", F("FilesUnsupportedZip")), toast("err", F("ConsoleSendFailed", { error: "the server is stopped" }))),
    msgSec("Sizes", h("div.hx-flex.hx-muted", FMP("Bytes", { count: 1 }), FMP("Bytes", { count: 512 }), FM("Kilobytes", { size: 4 }), FM("Megabytes", { size: 12 }), FM("Gigabytes", { size: 2 }), FM("Terabytes", { size: 1 }))));
  const routesMsgMods = () => msgPage(
    msgSec("Toasts", toast("ok", I("ModsResultInstalled", { mods: "Sodium, Lithium" })), toast("ok", I("ModsResultRemoved", { files: "old-mod.jar" })), toast("warn", I("ModsResultSkipped", { projects: "Iris" })), toast("info", I("ModsResultPlayersNeed", { mods: "Sodium" })),
      toast("ok", I("ModsUpdated", { name: "Sodium", version: "0.6.6" })), toast("err", I("ModsUpdateFailed", { name: "Sodium", error: "timed out" })), toast("ok", I("ModsUpdatesInstalled")), toast("err", I("ModsSomeUpdatesFailed", { failures: "Sodium, Create" })),
      toast("ok", I("ModsUploaded", { names: "mymod.jar" })), toast("err", I("ModsUploadFailed", { error: "file too large" })), toast("warn", I("ModsOnlyJars", { names: "notes.txt" })),
      toast("ok", I("ModsDeleted", { name: "Sodium" })), toast("err", I("ModsDeleteFailed", { error: "file in use" })), toast("err", I("ModsEnableFailed", { name: "Lithium", error: "file in use" })), toast("err", I("ModsDisableFailed", { name: "Lithium", error: "file in use" })),
      toast("ok", I("ModsPackSaved", { name: "Survival SMP.mrpack" })), toast("err", I("ModsDownloadPackFailed", { error: "disk full" })),
      toast("ok", I("ModsInstalledVersion", { title: "Sodium", version: "0.6.6" })), toast("err", I("ModsInstallFailed", { title: "Sodium", error: "hash mismatch" })),
      toast("ok", I("PackSaved")), toast("ok", I("PackSavedRestart")), toast("err", I("EulaAcceptFailed", { error: "permission denied" }))),
    msgSec("Mods page buttons and labels", h("div.hx-flex", btn(I("ModsSavePack"), { sm: true }), btn(I("ModsSortDownloads"), { sm: true }), btn(I("ModsSortFollows"), { sm: true }), btn(I("ModsSortNewest"), { sm: true }), btn(I("ModsSortUpdated"), { sm: true }))),
    msgSec("Resource pack URL errors", h("div.hx-err-t", I("PackUrlSpaces")), h("div.hx-err-t", I("PackNotAUrl")), h("div.hx-err-t", I("PackUrlScheme"))),
    msgSec("Friends", toast("ok", I("FriendsWhitelisted", { names: "Alex_mc, Sam_mc" })), toast("err", I("FriendsWhitelistFailed", { name: "Alex", error: "whitelist.json is read only" }))),
    msgSec("Instance (server) actions", toast("err", N("CreateFailed", { error: "port 25565 is in use" })), toast("info", I("Deleting", { name: "Survival SMP" })), h("div.hx-err-t", N("NameEmpty")), h("div.hx-err-t", N("CheckFields")), h("div.hx-muted", N("NoName"))),
    msgSec("Overview edit", h("div.hx-flex", input("Survival SMP", { focus: true, w: "w300" }), btn(M("Save"), { sm: true, v: "primary" }), btn(M("Cancel"), { sm: true })), h("div.hx-muted.hx-small", N("OwnerOnlyAtom"), tx(" · "), I("OwnerOnlyAtom"))));
  const routesMsgFormat = () => msgPage(
    msgSec("Dates and times (shared)", h("div.hx-col.hx-gap4",
      h("div.hx-flex", FM("JustNow"), FM("Today"), FM("Yesterday"), FM("TodayAt", { time: "14:05" }), FM("YesterdayAt", { time: "09:12" })),
      h("div.hx-flex", FMP("MinutesAgo", { count: 1 }), FMP("MinutesAgo", { count: 5 }), FMP("HoursAgo", { count: 1 }), FMP("HoursAgo", { count: 4 }), FMP("DaysAgo", { count: 1 }), FMP("DaysAgo", { count: 6 })),
      h("div.hx-flex", FM("LessThanAMinute"), FM("Minutes", { minutes: 12 }), FM("HoursMinutes", { hours: 3, minutes: 5 })),
      h("div.hx-flex", FM("PlayedJustNow"), FM("PlayedYesterday"), FM("PlayedAgo", { ago: FMP("DaysAgo", { count: 3 }) }), FM("PlayedOn", { date: FM("ShortDate", { month: FM("April"), day: 2 }) }), FM("NotPlayedYet")),
      h("div.hx-flex", FM("ShortDate", { month: FM("May"), day: 9 }), FM("ShortDateWithYear", { month: FM("December"), day: 31, year: 2025 }), FM("February"), FM("March")),
      h("div.hx-flex", FM("CompactThousands", { value: "12.5" }), FM("CompactMillions", { value: "3.7" }), FM("Percent", { value: 42 })),
      h("div.hx-flex.hx-muted", tx("Separators: group "), FM("GroupSeparator"), tx(" decimal "), FM("DecimalSeparator"), tx(" list "), FM("ListSeparator")))));

  // ---------------------------------------------------------------- routes
  const TAB_ROUTES = ["overview", "settings", "console", "files", "mods", "mods-browse", "mods-project", "macros", "eventlogs"];
  const ROUTES = {
    "home": { title: "Home (no instance selected)", group: "Pages", render: homePage },
    "first-run": { title: "First run (no servers yet)", group: "Pages", render: homePage },
    "overview": { title: "Overview", group: "Instance", render: overviewPage },
    "settings": { title: "Settings", group: "Instance", render: settingsPage },
    "console": { title: "Console", group: "Instance", render: consolePage },
    "files": { title: "Files", group: "Instance", render: filesPage },
    "mods": { title: "Mods: Installed", group: "Instance", render: modsPage },
    "mods-browse": { title: "Mods: Browse", group: "Instance", render: modsBrowsePage },
    "mods-project": { title: "Mods: project page", group: "Instance", render: modsProjectPage },
    "macros": { title: "Macros", group: "Instance", render: macrosPage },
    "eventlogs": { title: "Event logs", group: "Instance", render: eventLogsPage },
    "not-built": { title: "Not built / not found", group: "Pages", render: notBuiltPage },
    "msg-shell": { title: "Shell, core and common", group: "Messages", render: routesMsgShell },
    "msg-files": { title: "Files and console", group: "Messages", render: routesMsgFiles },
    "msg-mods": { title: "Mods, settings, instances", group: "Messages", render: routesMsgMods },
    "msg-format": { title: "Formats (dates, sizes)", group: "Messages", render: routesMsgFormat },

    "settings-general": { title: "Settings: General", group: "Settings dialog", dialog: true, render: () => settingsDialog("general") },
    "settings-users": { title: "Settings: Users", group: "Settings dialog", dialog: true, render: () => settingsDialog("users") },
    "settings-user": { title: "Settings: a user's permissions", group: "Settings dialog", dialog: true, render: () => settingsDialog("user") },
    "settings-profile": { title: "Settings: Profile", group: "Settings dialog", dialog: true, render: () => settingsDialog("profile") },
    "settings-updates": { title: "Settings: Updates", group: "Settings dialog", dialog: true, render: () => settingsDialog("updates") },

    "create-games": { title: "Create: select game", group: "Create server", dialog: true, render: createGames },
    "create-basic": { title: "Create: basic settings", group: "Create server", dialog: true, render: createBasic },
    "create-server": { title: "Create: server settings", group: "Create server", dialog: true, render: createServerStep },
    "create-auto": { title: "Create: auto settings", group: "Create server", dialog: true, render: createAuto },
    "create-modpack": { title: "Create: select modpack", group: "Create server", dialog: true, render: createModpack },
    "create-version": { title: "Create: modpack version", group: "Create server", dialog: true, render: createVersion },
    "create-mserver": { title: "Create: modpack server settings", group: "Create server", dialog: true, render: createModpackServer },

    "owner-account": { title: "Set up an owner account", group: "Dialogs", dialog: true, render: ownerAccountDlg },
    "stop-sharing": { title: "Stop sharing?", group: "Dialogs", dialog: true, render: () => confirm(N("ShareStopTitle"), N("ShareStopBody"), N("ShareStop"), "dangerfill", () => { X.set("sharing", "off"); X.close() }) },
    "cores": { title: "Cores (Change core)", group: "Dialogs", dialog: true, render: () => coresDlg("list") },
    "cores-trust": { title: "Cores: trust certificate", group: "Dialogs", dialog: true, render: () => coresDlg("trust") },
    "cores-signin": { title: "Cores: sign in", group: "Dialogs", dialog: true, render: () => coresDlg("signin") },
    "delete-instance": { title: "Delete server", group: "Dialogs", dialog: true, render: deleteInstanceDlg },
    "eula": { title: "Accept the Minecraft EULA", group: "Dialogs", dialog: true, render: eulaDlg },
    "restart-update": { title: "Restart to update?", group: "Dialogs", dialog: true, render: restartUpdateDlg },
    "restart-core": { title: "Restart the core? (relay)", group: "Dialogs", dialog: true, render: restartCoreDlg },
    "api-create": { title: "Create API key", group: "Dialogs", dialog: true, render: apiCreateDlg },
    "api-new": { title: "API key created", group: "Dialogs", dialog: true, render: apiNewDlg },
    "api-revoke": { title: "Revoke API key", group: "Dialogs", dialog: true, render: () => confirm(C("RevokeTitle"), C("RevokeBody", { name: "backup script", display: "lsk_3f9a…c01d" }), C("Revoke"), "dangerfill") },
    "create-user": { title: "Create user", group: "Dialogs", dialog: true, render: createUserDlg },
    "change-password": { title: "Change password", group: "Dialogs", dialog: true, render: changePasswordDlg },
    "delete-user": { title: "Delete user", group: "Dialogs", dialog: true, render: () => confirm(C("DeleteUserTitle"), C("DeleteUserConfirm", { name: "sam" }), M("Delete"), "dangerfill") },
    "mc-remove": { title: "Remove Minecraft account", group: "Dialogs", dialog: true, render: () => confirm(C("MinecraftRemoveTitle"), C("MinecraftRemoveBody", { name: "PeterAlt" }), M("Remove"), "dangerfill") },
    "safe-mode-off": { title: "Turn off safe mode?", group: "Dialogs", dialog: true, render: () => confirm(C("SafeModeOffTitle"), C("SafeModeOffBody"), C("TurnOff"), "dangerfill", () => { X.set("safeMode", "off"); X.close() }) },
    "code-exec-off": { title: "Turn off code execution?", group: "Dialogs", dialog: true, render: () => confirm(C("CodeExecutionOffTitle"), st().codeExec === "javaonly" ? C("CodeExecutionOffBodyJavaOnly") : C("CodeExecutionOffBody"), C("TurnOff"), "dangerfill", () => { X.set("codeExec", "off"); X.close() }) },
    "lan-http": { title: "Allow plain HTTP?", group: "Dialogs", dialog: true, render: () => confirm(C("LanHttpConfirmTitle"), C("LanHttpConfirmBody"), C("LanHttpAllow"), "dangerfill", () => { X.set("lanHttp", "on"); X.close() }) },
    "remote-off": { title: "Turn off access from other devices?", group: "Dialogs", dialog: true, render: () => confirm(C("RemoteAccessOffTitle"), C("RemoteAccessOffBody"), C("TurnOff"), "dangerfill", () => { X.set("remote", "off"); X.close() }) },
    "retire-cert": { title: "Retire friend certificate", group: "Dialogs", dialog: true, render: () => confirm(I("FriendsCertificateRetire"), I("FriendsCertificateRetireBody"), I("FriendsCertificateRetireConfirm"), "dangerfill", () => { X.set("friends", "cert"); X.close() }) },
    "files-new-file": { title: "Files: new file", group: "Dialogs", dialog: true, render: () => filesNameDlg("file") },
    "files-new-folder": { title: "Files: new folder", group: "Dialogs", dialog: true, render: () => filesNameDlg("folder") },
    "files-rename": { title: "Files: rename", group: "Dialogs", dialog: true, render: () => filesNameDlg("rename") },
    "files-delete": { title: "Files: delete", group: "Dialogs", dialog: true, render: filesDeleteDlg },
    "files-unsaved": { title: "Files: unsaved changes", group: "Dialogs", dialog: true, render: filesUnsavedDlg },
    "mods-delete": { title: "Mods: delete", group: "Dialogs", dialog: true, render: modsDeleteDlg },
    "mods-overwrite": { title: "Mods: replace files", group: "Dialogs", dialog: true, render: modsOverwriteDlg },
    "mods-incompatible": { title: "Mods: incompatible version", group: "Dialogs", dialog: true, render: modsIncompatibleDlg },
  };
  // instance routes default to the first server
  for (const r of TAB_ROUTES) ROUTES[r].defaultParams = { i: 0 };
  // keep the style element in sync when the module is re-registered
  const oldStyle = document.getElementById("hx-style"); if (oldStyle) oldStyle.textContent = css;

  const opt = (...pairs) => pairs.map((p) => (Array.isArray(p) ? p : [p, p]));
  Mock.register("host", {
    name: "Lodestone Host",
    size: [1280, 800],
    initial: "overview",
    states: {
      core: { label: "Core process", options: opt(["running", "Running (the shell)"], ["starting", "Starting"], ["restarting", "Restarting after it stopped"], ["failed", "Couldn't start (log)"], ["missing", "Couldn't start (no core file)"], ["stopping", "Stopping (quitting)"], ["signin", "Sign in (no local claim)"], ["signingin", "Signing in…"]), default: "running" },
      connection: { label: "Connection to the core", options: opt(["live", "Live"], ["connecting", "Connecting"], ["degraded", "Degraded (banner)"], ["reconnecting", "Reconnecting"], ["signedout", "Signed out"]), default: "live" },
      coreKind: { label: "Core in use", options: opt(["local", "This computer's"], ["remote", "Another core (remote)"]), default: "local" },
      remoteCores: { label: "Saved remote cores", options: opt(["off", "None"], ["on", "One saved"]), default: "on" },
      versionNote: { label: "Remote core version", options: opt(["none", "Same version"], ["incompatible", "Incompatible"], ["patch", "Patch difference"]), default: "none" },
      instances: { label: "Servers", options: opt(["some", "Three servers"], ["none", "None yet (first run)"]), default: "some" },
      creating: { label: "Server being set up", options: opt("off", "on"), default: "off" },
      instance: { label: "Selected server state", options: opt(["stopped", "Stopped"], ["starting", "Starting"], ["running", "Running"], ["stopping", "Stopping"], ["crashed", "Crashed"], ["unknown", "Unknown"]), default: "running" },
      game: { label: "Selected server game", options: opt(["fabric", "Fabric (mods)"], ["paper", "Paper (plugins)"], ["vanilla", "Vanilla (no mods)"]), default: "fabric" },
      eula: { label: "Minecraft EULA", options: opt(["accepted", "Accepted"], ["not", "Not accepted"]), default: "accepted" },
      sharing: { label: "Share with friends (link)", options: opt(["off", "Off"], ["starting", "Starting"], ["sharing", "Sharing"], ["reconnecting", "Reconnecting"]), default: "sharing" },
      friends: { label: "Friends", options: opt(["shared", "Shared with a friend"], ["none", "No friends yet"], ["loading", "Loading"], ["confirm", "Confirm unverified friend"], ["error", "Sync failed (errors)"], ["cert", "Certificate retired"], ["certManual", "Certificate retired (manual)"], ["retired", "Friend identity retired"], ["off", "Off in test copy"]), default: "shared" },
      players: { label: "Players online", options: opt(["some", "Two"], ["none", "None"]), default: "some" },
      graph: { label: "Performance graphs", options: opt(["data", "Data"], ["loading", "Loading"]), default: "data" },
      details: { label: "Overview details", options: opt(["closed", "Collapsed"], ["open", "Open"]), default: "closed" },
      settingsLoad: { label: "Server settings", options: opt(["ok", "Loaded"], ["loading", "Loading"], ["failed", "Failed"], ["noperm", "No permission"]), default: "ok" },
      neverStarted: { label: "Server never started", options: opt("off", "on"), default: "off" },
      validation: { label: "Form errors shown", options: opt("off", "on"), default: "off" },
      console: { label: "Console", options: opt(["live", "Live"], ["loading", "Loading"], ["buffered", "History only"], ["nobuffer", "Live, no history"], ["closed", "Closed"], ["error", "Connection lost"], ["empty", "No messages"], ["noperm", "No permission"]), default: "live" },
      files: { label: "Files", options: opt(["editor", "List and editor"], ["menu", "Right-click menu"], ["addmenu", "Add/Remove menu"], ["find", "Find bar"], ["unsaved", "Unsaved changes"], ["loadingfile", "Loading a file"], ["toolarge", "File too large"], ["empty", "Empty folder"], ["loading", "Loading"], ["noperm", "No permission"]), default: "editor" },
      mods: { label: "Mods", options: opt(["list", "Installed list"], ["installed", "Restart needed"], ["empty", "None installed"], ["failed", "Couldn't load"], ["unsupported", "Unsupported server"], ["noread", "No permission"]), default: "list" },
      browse: { label: "Mods browse", options: opt(["results", "Results"], ["none", "No results"], ["failed", "Search failed"], ["link", "Opening a link"]), default: "results" },
      project: { label: "Mod project page", options: opt(["mod", "A mod"], ["modpack", "A modpack"], ["clientonly", "Client only"], ["wrongtype", "Wrong type"], ["failed", "Couldn't load"]), default: "mod" },
      versions: { label: "Project versions", options: opt(["list", "List"], ["none", "None (show all off)"], ["noneall", "None at all"], ["failed", "Failed"]), default: "list" },
      eventlogs: { label: "Event logs", options: opt(["some", "Some events"], ["empty", "Empty"], ["failed", "Failed"]), default: "some" },
      createLoad: { label: "Create form", options: opt(["ok", "Loaded"], ["loading", "Loading"], ["failed", "Failed"]), default: "ok" },
      search: { label: "Modpack search", options: opt(["results", "Results"], ["loading", "Loading"], ["none", "No results"], ["failed", "Errors"], ["link", "Opening a link"], ["uploaded", "Uploaded .mrpack"]), default: "results" },
      role: { label: "Signed-in user", options: opt(["owner", "Owner"], ["user", "Limited user"]), default: "owner" },
      atom: { label: "Atom server (owner only)", options: opt("off", "on"), default: "off" },
      owner: { label: "Owner password", options: opt(["password", "Set"], ["nopassword", "None yet"]), default: "password" },
      remote: { label: "Access from other devices", options: opt("off", "on"), default: "on" },
      listPublic: { label: "List servers publicly", options: opt("off", "on"), default: "off" },
      lanHttp: { label: "LAN over plain HTTP", options: opt("off", "on"), default: "off" },
      safeMode: { label: "Safe mode", options: opt("on", "off"), default: "on" },
      codeExec: { label: "Code execution", options: opt(["on", "On"], ["off", "Off"], ["locked", "Locked by admin"], ["javaonly", "No scripting (Java only)"]), default: "javaonly" },
      tls: { label: "HTTPS certificate", options: opt(["self", "Self-signed"], ["own", "Your own"], ["none", "None"]), default: "self" },
      ram: { label: "Total RAM", options: opt(["ok", "Known"], ["none", "Zero"]), default: "ok" },
      users: { label: "Users", options: opt(["one", "Just you"], ["several", "Three"], ["loading", "Loading"]), default: "several" },
      userMenu: { label: "User row menu", options: opt("closed", "open"), default: "closed" },
      apiKeys: { label: "API keys", options: opt(["some", "Some"], ["none", "None"], ["loading", "Loading"], ["password", "Signed in with a key"]), default: "some" },
      keyAccess: { label: "New key access", options: opt(["full", "Full"], ["custom", "Custom"]), default: "custom" },
      mcAccounts: { label: "Minecraft accounts", options: opt(["none", "None"], ["some", "Two linked"], ["tooold", "Core too old"]), default: "none" },
      theme: { label: "Theme setting", options: opt("system", "light", "dark"), default: "system" },
      update: { label: "Update", options: opt(["devbuild", "Dev build (no updates)"], ["idle", "Not checked"], ["checking", "Checking"], ["uptodate", "Up to date"], ["available", "Available"], ["downloading", "Downloading"], ["ready", "Ready to restart"], ["failed", "Failed"]), default: "ready" },
      relay: { label: "Connect service", options: opt(["default", "Default"], ["custom", "Custom (dev)"], ["env", "Set by env var"]), default: "default" },
      notifications: { label: "Notifications", options: opt(["some", "Some"], ["none", "None"], ["loading", "Loading"]), default: "some" },
      notifPanel: { label: "Notifications panel", options: opt("closed", "open"), default: "closed" },
      toasts: { label: "Toasts", options: opt("off", "on"), default: "off" },
      tooltips: { label: "Tooltips (always shown)", options: opt("off", "on"), default: "off" },
      dev: { label: "DEV badge (developer build)", options: opt("off", "on"), default: "off" },
      signinErr: { label: "Sign-in error", options: opt("off", "on"), default: "off" },
    },
    routes: ROUTES,
    frame,
  });
})();
