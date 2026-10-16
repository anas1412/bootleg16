// Components as functions, one per function in client/src/ui/. Screens only call these.
const A = 'assets/';
// The four assistants: colour, channel, face sheet, and how their words arrive.
const AI = {
  claudette: { name: 'Claudette', c: '#E8875F', ch: '01', face: 'claudette', arrive: 'Streams word by word' },
  childgbt: { name: 'ChildGBT', c: '#6FE3C1', ch: '02', face: 'childgbt', arrive: 'Types each character, cursor blinking' },
  twins: { name: 'Twins', c: '#A98BFF', ch: '03', face: 'sister', arrive: 'Her line, then he cuts in' },
  grumpk: { name: 'Grumpk', c: '#D4FF3A', ch: '04', face: 'grumpk', arrive: 'Slams in all at once' },
};
// small tiles crop to the face so it reads at UI size
const TILE_VIEW = '44 42 112 112';
const MOODS = ['neutral', 'happy', 'laughing', 'smug', 'shocked', 'thinking', 'worried', 'sad', 'annoyed'];

const ui = {
  // A square portrait tile in the assistant's colour.
  portrait: (ai, size, mood = 'neutral', face = AI[ai].face) => `<div class="tile" style="--c:${AI[ai].c};width:${size}px;height:${size}px" data-face="${face}" data-mood="${mood}">${faceSvg(face, AI[ai].c, mood, false, size < 100 ? TILE_VIEW : undefined)}</div>`,
  // The uplink (in matches only, bottom centre): the icon centred in the top edge of a HUD
  // panel, waveforms out to both sides, the name and the line centred.
  // Twins: "her mood|his mood" and "her line|his line"; his line opens a second row.
  uplink: (ai, mood, text, { style = '' } = {}) => {
    const a = AI[ai], lines = text.split('|'), moods = mood.split('|');
    const wave = side => `<span class="ul-wave ${side}">${[14, 9, 16, 11, 8, 12, 6, 9, 4, 5].map((h, k) => `<i style="--k:${k};--h:${h}px"></i>`).join('')}</span>`;
    return `<div class="uplink talking" style="--c:${a.c};${style}" data-ai="${ai}" data-text="${text.replace(/"/g, '&quot;')}" data-moods="${mood}">
      <div class="ul-face">${ui.portrait(ai, 88, moods[0])}<i class="ul-lock"></i></div>
      <div class="ul-band"><i class="ul-edge"></i><i class="ul-sweep"></i><i class="ul-timer"></i>${wave('l')}${wave('r')}
        <div class="label ul-name">${a.name}${lines.length > 1 ? ' · him' : ''}</div>
        ${lines.map((l, i) => `<div class="ul-row${i ? ' two' : lines.length > 1 ? ' past' : ''}"><div class="ul-in"><div class="ul-line">${l}</div><i class="ul-scan l"></i><i class="ul-scan r"></i></div></div>`).join('')}
      </div></div>`;
  },
  t: (cls, s, color) => `<span class="${cls}"${color ? ` style="color:${color}"` : ''}>${s}</span>`,
  navItem: (num, label, jp, on, go) => `<div class="item${on ? ' on' : ''}"${go ? ` data-go="${go}"` : ''}><span class="label n">${num}</span><span class="h2">${label}</span>${jp ? `<span class="jp">${jp}</span>` : ''}</div>`,
  nav: (items, cls = '') => `<div class="nav ${cls}"><i class="ind"></i>${items.map(i => ui.navItem(...i)).join('')}</div>`,
  chip: (label, on, extra = '', style = '') => `<span class="chip${on ? ' on' : ''}"${style ? ` style="${style}"` : ''}>${extra}${label}</span>`,
  chips: (labels, on) => `<div class="chips">${labels.map(l => ui.chip(l, l === on)).join('')}</div>`,
  stepper: v => `<div class="stepper">${ui.chip('−')}<span class="v">${v}</span>${ui.chip('+')}</div>`,
  settingRow: (name, controls) => `<div class="srow"><span class="name">${name}</span><div class="chips" style="align-items:center">${controls}</div></div>`,
  primary: (label, hint = 'Enter', off = false) => `<div class="primary${off ? ' off' : ''}"><b>${label}</b><span class="label">${hint}</span></div>`,
  pageTitle: (eyebrow, title, jp) => `<div class="ptitle"><div class="label">${eyebrow}</div><div class="row"><span class="h2">${title}</span><span class="jp">${jp}</span></div></div>`,
  status: (s, kind = '') => `<span class="label status ${kind}">${s}</span>`,
  label: (s, mt = 32) => `<div class="label" style="margin:${mt}px 0 12px">${s}</div>`,
  avatar: (a, b, size = 44, presence) => `<div class="av" style="--a:${a};--b:${b};width:${size}px;height:${size}px">${presence ? `<i style="--p:${presence}"></i>` : ''}</div>`,
  // Who you are, top right.
  identity: (top, height) => `<div class="abs panel" style="right:var(--margin);top:${top}px;${height ? `height:${height}px;` : ''}display:flex;align-items:center;gap:var(--gap);padding:12px var(--pad) 12px 12px">
    ${ui.avatar('#c9733f', '#3b2418', 44, 'var(--ok)')}<div><div class="body" style="font-weight:700">anas_1412</div><div class="label">Online · via Steam</div></div></div>`,
  // Invite toast: right edge, so the amber bar is on the right.
  toast: (top) => `<div class="abs panel raised bar-r tune" style="right:var(--margin);top:${top}px;width:480px;padding:var(--gap) 20px;z-index:4">
    <div style="display:flex;gap:var(--gap);align-items:center">${ui.avatar('#5b8fd6', '#1b2a44')}<div><div class="label" style="color:var(--amber)">Party invite</div><div class="body"><b>kova</b> invited you to their party</div></div></div>
    <div style="display:flex;gap:var(--gap-s);margin-top:var(--gap);padding-left:60px;align-items:center">${ui.chip('Accept', true)}${ui.chip('Ignore')}<span class="label" style="margin-left:auto;color:var(--tx3)">0:24</span></div></div>`,
  // Your match is still on: under your name, every menu page, until it ends.
  rejoin: (top) => `<div class="abs panel raised bar-r tune" style="right:var(--margin);top:${top}px;width:480px;padding:var(--gap) 20px;z-index:4;border-radius:6px">
    <div class="label" style="color:var(--amber)">Your match is still on</div>
    <div class="body" style="margin-top:4px">Your team is waiting: get back in before it ends</div>
    <div style="margin-top:var(--gap)">${ui.primary('Reconnect', 'Enter').replace('min-width:420px', '')}</div></div>`,
  // A team vote: what it's about, yes and no against what's needed, time left, the keys.
  vote: (what, yes, no, needed, total, left) => `<div class="vote tune">
    <div class="vote-head"><span class="label" style="color:var(--amber)">Vote</span><span class="h3" style="font-size:20px">${what}</span><span class="label" style="margin-left:auto">${left}</span></div>
    <div class="vote-bar">${Array.from({ length: total }, (_, i) => `<i class="${i < yes ? 'yes' : i >= total - no ? 'no' : ''}"></i>`).join('')}<b style="left:${needed / total * 100}%"></b></div>
    <div class="vote-keys"><span class="label"><b class="key">F1</b> Yes · ${yes}</span><span class="label">${needed} needed</span><span class="label">No · ${no} <b class="key">F2</b></span></div></div>`,
  // A round-level announcement in the centre (voted out, surrendered).
  announce: (title, sub, c) => `<div class="announce tune"${c ? ` style="--c:${c}"` : ''}><div class="h2">${title}</div><i></i><div class="an-sub">${sub}</div></div>`,
  ico: (n, w, h, c = 'var(--tx1)') => `<i class="spr" style="--s:url(${A}hud/${n}.png);width:${w}px;height:${h}px;color:${c}"></i>`,
};
