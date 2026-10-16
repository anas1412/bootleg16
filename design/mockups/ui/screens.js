// Every screen at 1080p, built only from `ui`. Positions match the game (client/src/*.rs).
const shot = n => `${A}shots/${n}`;

// ---- menu frame: numbered list left, wordmark bottom left, you top right, page right ----
// The frame stays while pages swap inside #page and #extra (see go() in index.html).
const MENU_ITEMS = [['01', 'Play', 'プレイ', 'play'], ['02', 'Practice', '練習', 'practice'], ['03', 'Settings', '設定', 'settings'], ['04', 'Voice', '音声', 'voice'], ['05', 'Quit', '終了', '']];
function menuFrame(page, content, extra = '') {
  // depth: the space and the far 16 sit back, the list and the page float in front (parallax)
  return `<div class="space" data-tilt="none" data-depth="-0.6"></div><div class="far16" data-tilt="none" data-depth="-3">16</div>
  <div class="abs scan" style="inset:0"></div>
  <div class="abs tune" style="left:80px;top:200px;width:560px" data-tilt="y 16" data-hinge="left center" data-depth="0.3">${ui.nav(MENU_ITEMS.map(([n, l, j, k]) => [n, l, j, l === page, k]))}</div>
  <div class="abs jp tune" style="left:92px;bottom:262px;font-size:16px;color:var(--tx2);--i:3" data-tilt="y 12" data-hinge="left center">ブートレグ・シックスティーン</div>
  <div class="abs tune" style="left:88px;bottom:96px;display:flex;align-items:flex-end;gap:10px;--i:4" data-tilt="y 12" data-hinge="left center"><span class="display">Bootleg</span><span class="wm16"><span class="display">16</span></span></div>
  <div class="tune" style="--i:1" data-tilt="none">${ui.identity(80)}</div>
  <div id="page" class="abs tune plate${content ? '' : ' empty'}" style="left:680px;width:fit-content;min-width:640px;max-width:1180px;top:160px;--i:2" data-tilt="y -10" data-hinge="right center" data-depth="0.2">${content}</div><div id="extra" data-tilt="none">${extra}</div>`;
}

const SCREENS = {
  main: {
    name: 'Main menu', note: 'Straight and calm, no page. Pick an item: its page comes in and the menu swings into 3D. Esc swings it back flat.',
    menu: 'Main', page: () => '', extra: () => '',
  },
  practice: {
    name: 'Practice', note: 'Practice page open. An invite arrives under your name.',
    menu: 'Practice', page: () => `<div style="max-width:560px">
      ${ui.pageTitle('No rounds · infinite ammo · instant respawn', 'Practice', '練習')}
      ${ui.label('Map')}${ui.chips(['de_dust2', 'de_dust', 'de_inferno', 'de_train', 'de_nuke', 'de_mirage', 'cs_office', 'de_aztec'], 'de_dust2')}
      ${ui.label('Dummies')}${ui.stepper('4')}
      <div style="margin-top:40px">${ui.primary('Start · de_dust2')}</div></div>`,
    extra: () => ui.toast(164),
  },
  found: {
    name: 'Match found', note: 'Over any menu page. ACCEPT before the bar runs out; slots fill as players accept. Missing it costs a short queue cooldown.',
    menu: 'Practice', page: () => SCREENS.practice.page(), extra: () => matchFound(4, false),
  },
  accepted: {
    name: 'Accepted', note: 'After you accept: your slot is filled and the button waits for the others.',
    menu: 'Practice', page: () => SCREENS.practice.page(), extra: () => matchFound(7, true),
  },
  rejoin: {
    name: 'Reconnect', note: 'Your match is still on (a crash, or you left): the way back sits under your name on every page until the match ends.',
    menu: 'Practice', page: () => SCREENS.practice.page(), extra: () => ui.rejoin(164),
  },
  settings: {
    name: 'Settings', note: 'The AI assistant only speaks in matches; here you pick it by its face.',
    menu: 'Settings', page: () => `${ui.pageTitle('Saved as you change them', 'Settings', '設定')}
      <div class="rows" style="margin-top:24px">
        ${ui.settingRow('Name', `<span class="num-s" style="font-size:20px">anas_</span><span class="label" style="color:var(--tx3);margin-left:12px">Type to change</span>`)}
        ${ui.settingRow('Sensitivity', ui.stepper('2.40'))}
        ${ui.settingRow('Volume', ui.stepper('80%'))}
        ${ui.settingRow('Frame rate cap', ui.stepper('300'))}
        ${ui.settingRow('VSync', ui.chip('On') + ui.chip('Off', true))}
        ${ui.settingRow('Team IDs through walls', ['Off', 'Pips', 'Pips + names'].map(l => ui.chip(l, l === 'Pips + names')).join(''))}
        ${ui.settingRow('AI assistant', Object.entries(AI).map(([k, a]) => ui.chip(a.name, k === 'grumpk', `<span style="margin-left:-8px">${ui.portrait(k, 28)}</span>`)).join('') + ui.chip('Off'))}
        ${ui.settingRow('AI assistant messages', ['Quiet', 'Normal', 'Chatty'].map(l => ui.chip(l, l === 'Normal')).join(''))}
        ${ui.settingRow('Relay team info', ui.chip('On', true) + ui.chip('Off'))}
        ${ui.settingRow('During rounds', ui.chip('On', true) + ui.chip('Off'))}
      </div>`,
    extra: () => '',
  },
  voice: {
    name: 'Voice', note: 'Settings rows reused; the mic meter is live while this page is open.',
    menu: 'Voice', page: () => `${ui.pageTitle('Team hears you anywhere, proximity reaches anyone near', 'Voice', '音声')}
      <div class="rows" style="margin-top:24px">
        ${ui.settingRow('Voice chat', ui.chip('On', true) + ui.chip('Off'))}
        ${ui.settingRow('Team voice key', ui.chip('V'))}
        ${ui.settingRow('Proximity voice key', ui.chip('Mouse 4'))}
        ${ui.settingRow('Microphone', ui.chip('System default'))}
        ${ui.settingRow('Mic volume', ui.stepper('100%'))}
        ${ui.settingRow('Voice volume', ui.stepper('80%'))}
        ${ui.settingRow('Mic test', `<div style="width:320px;height:8px;background:var(--bg1)"><i class="breathe" style="display:block;width:46%;height:100%;background:var(--ok)"></i></div><span class="label" style="margin-left:12px">Talk now</span>`)}
      </div>`,
    extra: () => '',
  },
  play: {
    name: 'Play', note: 'Party and Steam friends. After a queue penalty the main button waits out the cooldown.',
    html: (state = 'cooldown') => {
      const party = [['anas_1412', 'Leader · you', '#c9733f', '#3b2418', 'you'], ['kova', 'Ready', '#5b8fd6', '#1b2a44', 'ok'], ['lux', 'Not ready', '#7fbf6a', '#1f3320', '']];
      const slots = party.map((p, i) => `<div class="card tune${p[4] === 'you' ? ' sel' : ''}" style="--i:${i};height:280px;padding:var(--pad);display:flex;flex-direction:column;align-items:center;gap:var(--pad)">
          ${ui.avatar(p[2], p[3], 96)}<div style="text-align:center"><div class="h3" style="text-transform:none">${p[0]}</div><div class="label" style="margin-top:8px;color:${p[4] === 'you' ? 'var(--amber)' : p[4] === 'ok' ? 'var(--ok)' : 'var(--tx2)'}">${p[1]}</div></div></div>`).join('') +
        [3, 4].map(i => `<div class="card invite tune" style="--i:${i};height:280px;padding:23px;border:1px dashed var(--line-2);display:flex;flex-direction:column;align-items:center;gap:var(--pad)">
          <div style="width:96px;height:96px;border:1px dashed var(--line-2);display:grid;place-items:center;font:300 40px var(--f-text);color:var(--tx3)">+</div><span class="label">Invite a friend</span></div>`).join('');
      const fr = [['In game', 'var(--amber)', [['mako', 'In a match · 7:5', '#d4a03f', '#3a2a10', 'Invite']]],
        ['Online', 'var(--ok)', [['kova', 'In your party', '#5b8fd6', '#1b2a44', 'In party'], ['vera', 'Online', '#d66f8f', '#3a1a26', 'Invite'], ['rook', 'Away', '#9b6fd6', '#271a3a', 'Invited', 1]]],
        ['Offline', 'var(--tx3)', [['tetsu', 'Last online 2 h ago', '#6f7c8a', '#20262c'], ['jun', 'Last online yesterday', '#6f7c8a', '#20262c']]]];
      const friends = fr.map(([g, dot, list]) => `<div class="label" style="padding:16px 24px 8px">${g} · ${list.length}</div>` + list.map(f => `
        <div style="display:flex;align-items:center;gap:var(--gap);height:64px;padding:0 var(--pad);${g === 'Offline' ? 'opacity:.55' : ''}">
          ${ui.avatar(f[2], f[3], 44, f[5] ? 'var(--tx2)' : dot)}<div style="flex:1;min-width:0"><div class="body" style="font-weight:700">${f[0]}</div><div class="label" style="text-transform:none;letter-spacing:.04em">${f[1]}</div></div>
          <div style="width:112px;display:flex;justify-content:center">${f[4] ? (f[4] === 'Invite' ? ui.chip('Invite', false, '', 'width:112px;justify-content:center;padding:0') : `<span class="label" style="color:var(--tx3)">${f[4]}</span>`) : ''}</div></div>`).join('')).join('');
      return `<div class="abs scan" style="inset:0"></div>
      <div class="phead">${ui.pageTitle('Play · Casual 5v5', 'Party', 'パーティー')}</div>${ui.identity(72, 68)}
      <div class="abs" style="left:var(--margin);bottom:48px"><span class="chip" data-go="main" style="gap:12px">Back <b class="key" style="margin:0">Esc</b></span></div>
      <div class="abs" style="left:var(--margin);top:200px;width:1200px">
        <div class="label" style="margin-bottom:16px">Your party · 3 of 5 · code KX7-42Q</div>
        <div style="display:grid;grid-template-columns:repeat(5,1fr);gap:16px">${slots}</div>
        ${ui.label('Mode', 40)}<div class="chips">${ui.chip('Casual queue', true)}${ui.chip('Custom game · 10 players')}</div>
        <div style="display:flex;align-items:center;gap:var(--pad);margin-top:40px">${state === 'cooldown'
          ? ui.primary('Find match', 'Queue again in 0:45', true) + '<span class="label" style="color:var(--tx3)">A match was not accepted in time</span>'
          : ui.primary('Find match') + '<span class="label" style="color:var(--tx3)">Only the party leader can start the queue</span>'}</div>
      </div>
      <div class="abs flat tune" style="left:1344px;right:var(--margin);top:200px;bottom:120px;--i:2">
        <div style="display:flex;align-items:baseline;padding:20px var(--pad) 16px;border-bottom:1px solid var(--line)"><span class="h3">Friends</span><span class="label" style="margin-left:auto">1 in game · 3 online</span></div>
        <div style="padding:8px 0">${friends}</div></div>
      `;
    },
  },
  vote: {
    name: 'Vote', game: 'de_dust_ct.jpg', note: 'A kick or surrender vote: top centre under the score. Your team only; F1 yes, F2 no.',
    html: () => hud() + `<div class="abs" style="left:50%;top:150px;transform:translateX(-50%)">${ui.vote('Kick Rook', 2, 1, 3, 4, '0:18')}</div>
      <div class="abs" style="left:0;right:0;top:300px;display:grid;place-items:center">${ui.announce('Rook was voted out', 'Bureau plays on with 4', 'var(--ct-color)')}</div>`,
  },
  spectate: {
    name: 'Spectating', game: 'spec_de_dust2_front.jpg', note: 'Dead: you watch a living teammate. The corners show their health, armor and weapon; the plate at the bottom says who it is, their game, and how to switch.',
    html: () => spectate(),
  },
  hud: {
    name: 'In game', game: 'de_dust_ct.jpg', note: 'Live: hits drain health with a trail, shots roll the clip, reloads fill from the centre, money counts with its change, voice rows show who talks, the timer pulses in its last 10 s.',
    html: () => hud(),
  },
  buy: {
    name: 'Buy', game: 'de_dust_ct.jpg', covered: true, note: 'Opens over the game: the game blurs and dims behind it. Toggle it to see both ways.',
    html: () => hud() + `<div class="over">${buy()}</div>`,
  },
  scores: {
    name: 'Scoreboard', game: 'de_dust_ct.jpg', covered: true, note: 'Right-click frees the mouse; click a player to report them, or call a kick vote on a teammate.',
    html: () => hud() + `<div class="over">${scores()}</div>`,
  },
  pause: {
    name: 'Pause', game: 'de_dust2_t.jpg', covered: true, note: 'Esc. The same list rows as the main menu.',
    html: () => hud() + `<div class="over" style="display:grid;place-items:center"><div class="tune" style="width:560px">
      ${ui.pageTitle('Match', 'Paused', '一時停止')}<div style="margin-top:32px">${ui.nav([['01', 'Resume', '再開', true], ['02', 'Choose team', 'チーム'], ['03', 'Settings', '設定'], ['04', 'Leave match', '退出']])}</div></div></div>`,
  },
  teams: {
    name: 'Team pick', game: 'de_dust2_t.jpg', covered: true, note: 'Each side card carries its colour bar on top. Keys 1 and 2.',
    html: () => hud() + `<div class="over" style="display:grid;place-items:center"><div class="tune">
      ${ui.pageTitle('Pick a side', 'Choose your team', 'チーム選択')}
      <div style="display:flex;gap:var(--gap);margin-top:32px">
        ${[['1', 'Strays', 'ストレイズ', 'Attack · plant the bomb', 'var(--t-color)', 1], ['2', 'Bureau', 'ビューロー', 'Defend · defuse', 'var(--ct-color)']].map((s, i) => `
        <div class="card tune${s[5] ? ' sel' : ''}" style="--i:${i + 1};width:400px;padding:var(--pad)"><div class="abs" style="left:0;right:0;top:0;height:var(--bar);background:${s[4]}"></div>
          <div class="label">${s[0]}</div><div class="h2" style="margin-top:24px;color:${s[4]}">${s[1]}</div><div class="jp" style="margin-top:8px">${s[2]}</div>
          <div class="body" style="margin-top:24px;color:var(--tx2)">${s[3]}</div></div>`).join('')}
      </div><div style="display:flex;gap:6px;margin-top:var(--gap)">${ui.chip('Auto-assign')}${ui.chip('Spectate')}</div></div></div>`,
  },
  assistant: {
    name: 'AI assistant', note: 'In matches, bottom centre: the edge light draws out, the icon tunes in on it, the line resolves from the centre outward. Click one to replay.',
    html: () => {
      const demo = [['claudette', 'thinking', 'Two down at B. Viper has the AWP. Play it slow.'],
        ['childgbt', 'neutral', 'Your chance of winning this round is eleven percent. I recommend saving. Thank you for listening.'],
        ['twins', 'annoyed|neutral', 'Ugh, fine, save. It\'s not like I care if you win.|She means: save this round, you\'ll have rifles next.'],
        ['grumpk', 'laughing', 'Broke again. Inspiring. Buy a decoy, it\'s about your level.']];
      return `<div class="phead">${ui.pageTitle('In matches only · bottom centre · click one to replay', 'AI assistant', 'アシスタント')}</div>
      <div class="abs" style="left:var(--margin);right:var(--margin);top:200px;display:grid;grid-template-columns:1fr 1fr;gap:40px 64px;justify-items:center">
        ${demo.map(([k, m, l], i) => `<div class="tune" style="--i:${i}">${ui.uplink(k, m, l)}</div>`).join('')}
      </div>
      <div class="abs tune" style="left:var(--margin);right:var(--margin);top:800px;--i:5">
        <div class="label">Live faces · head turn with depth, blinks, mood poses</div>
        <div style="display:flex;gap:32px;margin-top:12px">${['claudette', 'childgbt', 'twins', 'grumpk'].map(k => `<div style="display:flex;align-items:center;gap:16px">
          ${ui.portrait(k, 132).replace('class="tile"', 'class="tile" data-cycle')}<div style="width:200px"><div class="label" style="color:${AI[k].c}">${AI[k].name}</div><div class="h3 mood-name" style="margin-top:6px">neutral</div></div></div>`).join('')}</div>
      </div>`;
    },
  },
  moods: {
    name: 'AI assistants', note: 'The game\'s own face sheets: 9 moods, set by the same event as the line.',
    html: () => {
      const cols = 'grid-template-columns:240px repeat(9,1fr)';
      const who = [['claudette', 'Assertive, careful'], ['childgbt', 'Cold and polite'], ['twins', 'Her · tsundere', 'sister'], ['twins', 'Him · the calm one', 'brother'], ['grumpk', 'Roaster, troll']];
      return `<div class="phead">${ui.pageTitle('Four assistants · nine moods each', 'AI assistants', '表情')}</div>
      <div class="abs" style="left:var(--margin);right:var(--margin);top:192px">
        <div style="display:grid;${cols};gap:16px;margin-bottom:12px"><span></span>${MOODS.map(m => `<span class="label" style="text-align:center">${m}</span>`).join('')}</div>
        ${who.map(([k, d, sheet], r) => `<div class="tune" style="--i:${r};display:grid;${cols};gap:16px;align-items:center;padding:10px 0;border-top:1px solid var(--line)">
          <div>${sheet === 'brother' ? '' : `<div class="h2" style="font-size:40px">${AI[k].name}</div>`}<div class="label">${d}</div></div>
          ${MOODS.map(m => `<div style="display:grid;place-items:center">${ui.portrait(k, 132, m, sheet || AI[k].face)}</div>`).join('')}</div>`).join('')}
      </div>`;
    },
  },
  system: {
    name: 'Design system', note: 'tokens.rs, kit.rs, panels.rs, widgets.rs and motion.rs, drawn. Hover the parts: every state eases.',
    html: () => system(),
  },
};

// Match found: over the menu, the blurred menu behind (like menus over the game).
function matchFound(accepted, you) {
  const slots = Array.from({ length: 10 }, (_, i) => `<i class="mf-slot${i < accepted ? ' on' : ''}${you && i === accepted - 1 ? ' you' : ''}" style="--i:${i}"></i>`).join('');
  return `<div class="mf tune" style="--i:0"><div class="mf-box">
    <div class="label" style="color:var(--amber)">Casual 5v5 · ready check</div>
    <div class="display mf-title">Match found</div>
    <div class="mf-slots">${slots}</div>
    <div class="label" style="margin-top:12px">${accepted} of 10 accepted</div>
    <div class="mf-timer"><i style="width:${you ? 46 : 72}%"></i></div>
    <div class="label mf-left">0:${you ? '09' : '14'}</div>
    <div style="margin-top:32px;display:flex;flex-direction:column;align-items:center;gap:12px">
      ${you ? ui.primary('Accepted', 'Waiting for ' + (10 - accepted), true).replace('class="primary off"', 'class="primary off mf-wait"') : ui.primary('Accept', 'Enter').replace('class="primary"', 'class="primary mf-accept"')}
      ${you ? '' : ui.chip('Decline')}
    </div></div></div>`;
}

// Under a player's scoreboard row (mouse freed with right-click): report, and for teammates a
// kick vote.
function playerMenu(name, mate) {
  return `<div class="tune" style="display:flex;align-items:center;gap:8px;height:56px;padding-left:120px">
    <span class="label" style="margin-right:8px">${name}</span>${['Cheating', 'Griefing', 'Abuse'].map(r => ui.chip('Report · ' + r)).join('')}
    ${mate ? '<span style="width:1px;height:28px;background:var(--line-2);margin:0 8px"></span>' + ui.chip('Vote to kick') : ''}</div>`;
}

// Menu pages draw inside the shared frame.
for (const s of Object.values(SCREENS)) if (s.menu) s.html = () => menuFrame(s.menu, s.page(), s.extra());

// ---- in game ---------------------------------------------------------------------------
// Every HUD panel: rounded, tinted to your team, an edge light on top in your team's colour
// (flares on events), and the same motion language as the menus (hud.js runs the live demo).
const MIC = `url('data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="8" y="2" width="8" height="13" rx="4"/><path d="M5 11a7 7 0 0 0 14 0" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"/><path d="M12 18v4M8 22h8" stroke="#000" stroke-width="2" stroke-linecap="round"/></svg>')}')`;
const hudIco = (n, w, h, c, extra = '') => `<i class="spr" style="--s:url(${A}hud/${n}.png);width:${w}px;height:${h}px;color:${c}"${extra}></i>`;
// One voice row: mic, live level, the name in its team's colour, the channel. Yours carries
// the amber bar on the outer (left) edge.
const voiceRow = (name, color, ch, you) => `<div class="vc panel hud tune${you ? ' you bar-l' : ''}" data-voice="${name}">
  <i class="spr vc-mic" style="--s:${MIC};width:18px;height:18px;color:${you ? 'var(--amber)' : color}"></i>
  <span class="vc-lv">${[6, 12, 9, 4].map(h => `<i style="--h:${h}px"></i>`).join('')}</span>
  <span class="vc-name" style="color:${you ? 'var(--tx1)' : color}">${name}</span><span class="label vc-ch">${ch}</span></div>`;
// A teammate's tag over their head: name (mic and live level when they talk), health under it,
// the pip pointing down at them. Far teammates fade (game: teammates.rs).
const mateTag = (x, y, name, hp, talking, fade = 1) => `<div class="abs mtag" style="left:${x}px;top:${y}px;opacity:${fade}">
  ${talking ? `<i class="spr" style="--s:${MIC};width:18px;height:18px;color:var(--ct-color);margin-bottom:2px"></i>` : ''}<div class="mt-name"><span>${name}</span></div>
  <div class="mt-hp"><i style="width:${hp}%;${hp <= 25 ? 'background:var(--bad)' : ''}"></i></div><i class="mt-pip"></i></div>`;

// Spectating (game: spectate.rs + hud): the same corners as playing, showing who you watch, and
// one plate at the bottom: who it is, their game this match, how to switch.
function spectate() {
  const corners = hud().replace(/<div class="abs keep"[\s\S]*?<\/div><\/div>\s*<\/div>/, '');
  // not sent while watching: the crosshair, their ammo
  return `<style>.stage .xhair, .stage #ammo > :not(.wpn):not(.edge) { display: none !important; }</style>${corners}
  ${mateTag(1180, 358, 'kova', 64, true)}${mateTag(1490, 410, 'vera', 18, false, .55)}
  <div class="abs spec tune" style="left:50%;bottom:var(--safe);transform:translateX(-50%)">
    <div class="label" style="color:var(--tx2)">Watching</div>
    <div class="sp-row"><span class="h3" style="color:var(--ct-color);text-transform:none">Ghost</span>
      <span class="sp-stat"><b>14</b> K</span><span class="sp-stat"><b>9</b> D</span><span class="sp-stat"><b>92</b> ADR</span></div>
    <div class="sp-keys"><span class="label"><b class="key">Mouse 1</b> Next</span><span class="label"><b class="key">Mouse 2</b> Previous</span></div>
  </div>`;
}

function hud() {
  const ticks = (c, n) => `<div style="display:flex;gap:4px">${[0, 1, 2, 3, 4].map(i => `<i style="width:28px;height:4px;border-radius:2px;background:${i < n ? c : 'var(--line-2)'}"></i>`).join('')}</div>`;
  return `<div class="abs panel hud scan" style="left:var(--safe);top:var(--safe);width:256px;height:256px;overflow:hidden"><i class="edge"></i>
    <img src="${A}ov/de_dust.png" alt="" class="abs" style="width:768px;max-width:none;left:-502px;top:-119px;filter:grayscale(1) contrast(1.2) brightness(.8);opacity:.8">
    <div class="abs" style="left:120px;top:106px;border-left:8px solid transparent;border-right:8px solid transparent;border-bottom:22px solid var(--amber)"></div>
    <div class="abs" style="left:84px;top:172px;width:12px;height:12px;border-radius:50%;background:var(--ct-color);box-shadow:0 0 0 2px var(--bg0)"></div>
    <div class="abs" style="left:178px;top:74px;width:12px;height:12px;border-radius:50%;background:var(--ct-color);box-shadow:0 0 0 2px var(--bg0)"></div>
    <div class="abs" style="left:150px;top:150px;width:12px;height:12px;background:var(--t-color);box-shadow:0 0 0 2px var(--bg0)"></div></div>
  <div class="abs panel hud" id="money" style="left:var(--safe);top:296px;width:256px;height:48px;display:flex;align-items:center;gap:8px;padding:0 16px"><i class="edge"></i>
    <span style="font:700 26px var(--f-num);color:var(--ok)">$</span><span id="cash" style="font:700 26px var(--f-num);font-variant-numeric:tabular-nums">3250</span>
    <span class="cash-delta" id="delta"></span>
    <span class="st-row"><span class="st" id="st-kit" title="Defuse kit">${hudIco('defuser', 24, 24, 'var(--ct-color)')}</span><span class="st on" id="st-buy" title="Buy zone">${hudIco('buyzone', 24, 24, 'var(--ok)')}</span></span></div>
  <div class="abs" id="voices" style="left:var(--safe);top:360px;display:flex;flex-direction:column;align-items:flex-start;gap:4px">
    ${voiceRow('anas', 'var(--ct-color)', 'Team', true)}${voiceRow('Ghost', 'var(--ct-color)', 'Team')}</div>
  <div class="abs" style="left:0;right:0;top:var(--safe);display:flex;flex-direction:column;align-items:center;gap:12px">
    <div style="display:flex;gap:4px">
      <div class="panel hud score" style="width:88px;display:grid;place-items:center;--edge:var(--ct-color)"><i class="edge on"></i><span class="num-m" style="color:var(--ct-color)">7</span></div>
      <div class="panel hud" id="clock" style="width:148px;padding:10px 0;display:flex;flex-direction:column;align-items:center;gap:6px"><span class="num-m" id="clock-t">1:42</span><span class="label">Round 13</span></div>
      <div class="panel hud score" style="width:88px;display:grid;place-items:center;--edge:var(--t-color)"><i class="edge on"></i><span class="num-m" style="color:var(--t-color)">5</span></div></div>
    <div style="width:332px;display:flex;justify-content:space-between">${ticks('var(--ct-color)', 3)}${ticks('var(--t-color)', 4)}</div></div>
  <div class="abs" style="right:var(--safe);top:var(--safe);display:flex;flex-direction:column;align-items:flex-end;gap:4px">
    <div class="kf"><span style="color:var(--t-color)">Viper</span>${ui.ico('d_ak47', 84, 28)}<span style="color:var(--ct-color)">Hawk</span></div>
    <div class="kf you bar-r"><span style="color:var(--ct-color)">anas</span>${ui.ico('d_awp', 84, 28)}${ui.ico('d_headshot', 63, 28)}<span style="color:var(--t-color)">Rook</span></div>
    <div class="kf"><span style="color:var(--ct-color)">Ghost</span>${ui.ico('d_m4a1', 84, 28)}<span style="color:var(--t-color)">Blitz</span></div></div>
  <div class="abs xhair" style="left:960px;top:540px">${[[-1, -20, 2, 12], [-1, 8, 2, 12], [-20, -1, 12, 2], [8, -1, 12, 2]].map(([x, y, w, h]) => `<i class="abs" style="left:${x}px;top:${y}px;width:${w}px;height:${h}px;background:#4CFF6A;box-shadow:0 0 0 1px #0008"></i>`).join('')}</div>
  <div class="abs keep" style="left:50%;bottom:var(--safe);transform:translateX(-50%)">${ui.uplink('claudette', 'worried', 'Bomb\'s down on A. Forty seconds. Hold your angle, don\'t peek.')}</div>
  <div class="abs panel hud" id="vitals" style="left:var(--safe);bottom:var(--safe);display:flex"><i class="edge"></i><i class="wash"></i>
    <div style="padding:16px var(--pad);display:flex;align-items:center;gap:var(--gap)">${ui.ico('cross', 36, 36)}<div><div class="num-l" id="hp">100</div><div class="meter" style="width:132px;margin-top:10px"><b id="hp-ghost" style="width:100%"></b><i id="hp-bar" style="width:100%"></i></div></div></div>
    <div style="width:1px;background:var(--line)"></div>
    <div style="padding:16px var(--pad);display:flex;align-items:center;gap:14px">${ui.ico('suithelmet_full', 32, 32, 'var(--tx2)')}<div><div class="num-m" id="ap" style="color:var(--tx2)">67</div><div class="meter" style="width:72px;margin-top:10px"><b id="ap-ghost" style="width:67%"></b><i id="ap-bar" style="width:67%;background:var(--tx2)"></i></div></div></div></div>
  <div class="abs panel hud" id="ammo" style="right:var(--safe);bottom:var(--safe);display:flex"><i class="edge"></i><i class="reload" id="reload"></i>
    <div class="wpn tune" id="wpn" style="width:184px;white-space:nowrap;padding:16px var(--pad);display:flex;flex-direction:column;justify-content:center;gap:10px"><span class="label" style="color:var(--tx1)">AK-47</span>${ui.ico('d_ak47', 108, 36, 'var(--tx2)')}</div>
    <div style="width:1px;background:var(--line)"></div>
    <div style="padding:16px var(--pad);display:flex;align-items:baseline;gap:10px"><span class="num-l clip" id="clip">24</span><span class="num-s" id="reserve" style="color:var(--tx2)">/ 90</span></div></div>`;
}

// Strays side. [name, price, art, ammo or note, damage, rpm, move speed, penetration]; equipment has no gun stats.
const BUY = [
  ['Pistols', [['Glock-18', 400, 'gun/glock18', '20 / 120', 25, 400, 250, 1], ['USP', 500, 'gun/usp', '12 / 100', 34, 352, 250, 1], ['Desert Eagle', 650, 'gun/deagle', '7 / 35', 54, 267, 250, 2]]],
  ['SMGs', [['MAC-10', 1400, 'gun/mac10', '30 / 100', 29, 857, 250, 1], ['MP5', 1500, 'gun/mp5navy', '30 / 120', 26, 750, 250, 1], ['UMP45', 1700, 'gun/ump45', '25 / 100', 30, 571, 250, 1], ['P90', 2350, 'gun/p90', '50 / 100', 26, 857, 245, 1]]],
  ['Rifles', [['Galil', 2000, 'gun/galil', '35 / 90', 30, 666, 240, 2], ['AK-47', 2500, 'gun/ak47', '30 / 90', 36, 600, 221, 2], ['Scout', 2750, 'gun/scout', '10 / 90', 75, 48, 260, 3], ['AWP', 4750, 'gun/awp', '10 / 30', 115, 41, 210, 3]]],
  ['Equipment', [['Kevlar', 650, 'hud/suit_full', 'Armor 100'], ['Kevlar + Helmet', 1000, 'hud/suithelmet_full', 'Armor 100 · helmet'], ['Flashbang', 200, 'gun/flashbang', 'Max 2'], ['HE Grenade', 300, 'gun/hegrenade', 'Max 1'], ['Smoke Grenade', 300, 'gun/smokegrenade', 'Max 1']]],
];
const FUNDS = 3250;
const buyState = { cat: 2, sel: 1 };

function buyCards(cat, sel) {
  return BUY[cat][1].map((g, i) => {
    const cant = g[1] > FUNDS, on = i === sel, icon = g[2].startsWith('hud/');
    const art = icon ? `<i class="spr abs" style="--s:url(${A}${g[2]}.png);left:50%;top:52px;width:88px;height:88px;margin-left:-44px;color:${cant ? 'var(--tx3)' : 'var(--tx1)'}"></i>`
      : `<i class="gun abs" style="--s:url(${A}${g[2]}.png);left:40px;right:40px;top:52px;height:88px;${cant ? 'filter:grayscale(1) brightness(.45)' : ''}"></i>`;
    return `<div class="card tune${on ? ' sel' : ''}" data-item="${i}" style="--i:${i};height:200px;padding:20px var(--pad);cursor:pointer">
      <div style="display:flex"><span class="label n">${String(i + 1).padStart(2, '0')}</span><span class="label" style="margin-left:auto">${g[3]}</span></div>${art}
      <div class="abs" style="left:var(--pad);right:var(--pad);bottom:18px;display:flex;align-items:baseline"><span class="h3" style="color:${cant ? 'var(--tx3)' : 'var(--tx1)'}">${g[0]}</span><span class="num-s" style="margin-left:auto;font-size:20px;color:${cant ? 'var(--bad)' : 'var(--tx1)'}">$${g[1]}</span></div></div>`;
  }).join('');
}

function buyDetails(cat, sel) {
  const g = BUY[cat][1][sel], cant = g[1] > FUNDS, icon = g[2].startsWith('hud/');
  const size = Math.min(96, Math.floor(404 / (g[0].length * 0.43)));
  const spec = g.length > 4
    ? [['Damage', g[4], g[4] / 120], ['Fire rate', `${g[5]} rpm`, g[5] / 900], ['Magazine', g[3], 0], ['Move speed', g[6], g[6] / 260], ['Penetration', g[7], g[7] / 3]]
    : [['Use', g[3], 0]];
  return `<div class="tune"><div class="label" style="color:var(--amber)">${BUY[cat][0]} · selected</div>
    <div class="display" style="font-size:${size}px;line-height:96px;height:96px;margin-top:12px;white-space:nowrap">${g[0]}</div></div>
    ${icon ? `<i class="spr tune" style="--s:url(${A}${g[2]}.png);display:block;width:120px;height:120px;margin:24px auto;color:var(--tx1);--i:1"></i>` : `<i class="gun tune" style="--s:url(${A}${g[2]}.png);height:120px;margin:24px 0;--i:1"></i>`}
    ${spec.map((s, i) => `<div class="tune" style="--i:${i + 2};display:grid;grid-template-columns:130px 1fr 112px;gap:16px;align-items:center;height:44px;border-top:1px solid var(--line)"><span class="label">${s[0]}</span><div class="meter"${s[2] ? '' : ' style="visibility:hidden"'}><i style="width:${Math.min(1, s[2]) * 100}%"></i></div><span class="num-s" style="text-align:right;font-size:16px">${s[1]}</span></div>`).join('')}
    <div class="abs" style="left:32px;right:32px;bottom:32px">${(cant ? ui.primary(`Buy · $${g[1]}`, `Need $${g[1] - FUNDS}`, true) : ui.primary(`Buy · $${g[1]}`)).replace('min-width', 'width')}</div>`;
}

function buy() {
  const { cat, sel } = buyState;
  return `<div class="abs tune" style="left:var(--margin);right:var(--margin);top:120px;height:96px;display:flex;align-items:flex-end;padding-bottom:var(--pad);border-bottom:1px solid var(--line)">
      <div><div class="label" style="color:var(--t-color)">Buy · Strays · 0:14 left</div><div class="ptitle"><div class="row"><span class="h2">Armory</span><span class="jp">武器庫</span></div></div></div>
      <div class="bteam"><div class="label">Team</div><div class="row">${[['kova', 4350], ['vera', 1200], ['rook', 800, 1]].map(([n, m, dead]) => `<div class="bt${m < 2700 ? ' low' : ''}${dead ? ' dead' : ''}"><span>${n}</span><b>$${m}</b></div>`).join('')}</div></div>
      <div style="margin-left:48px;text-align:right"><div class="label">Funds</div><div style="font:700 44px var(--f-num);margin-top:4px"><span style="color:var(--ok)">$</span>${FUNDS}</div></div></div>
    <div class="abs tune" style="left:var(--margin);top:240px;width:360px;--i:1">${ui.nav(BUY.map((c, i) => [String(i + 1).padStart(2, '0'), c[0], '', i === cat]), 'compact buycats')}</div>
    <div id="cards" class="abs" style="left:496px;top:240px;width:820px;display:grid;grid-template-columns:1fr 1fr;gap:16px">${buyCards(cat, sel)}</div>
    <div class="abs panel tune" style="left:1356px;right:var(--margin);top:240px;bottom:96px;--i:3"><div id="details" class="abs" style="inset:0;padding:32px">${buyDetails(cat, sel)}</div></div>`;
}

function scores() {
  const hist = ['ct:d_skull', 't:c4', 'ct:defuser', 'ct:d_skull', 't:d_skull', 'ct:stopwatch', 't:c4', 'ct:d_skull', 't:d_skull', 'ct:defuser', 'ct:d_skull', 't:d_skull', 'now'];
  const strip = Array.from({ length: 24 }, (_, i) => hist[i] || '').map(r => {
    if (!r || r === 'now') return `<div style="flex:1;height:40px;background:var(--bg1);box-shadow:inset 0 0 0 ${r ? '2px var(--amber)' : '1px var(--line)'}"></div>`;
    const [t, n] = r.split(':'), c = t === 'ct' ? 'var(--ct-color)' : 'var(--t-color)';
    return `<div style="flex:1;height:40px;background:var(--bg1);box-shadow:inset 0 calc(-1 * var(--bar)) 0 ${c};display:grid;place-items:center">${ui.ico(n, 26, 26, c)}</div>`;
  }).join('');
  const row = (r) => `<div style="display:grid;grid-template-columns:96px 1fr 96px 96px;align-items:center;height:48px;padding:0 var(--pad);border-top:1px solid var(--line);${r[4] ? 'background:var(--amber-wash);box-shadow:inset var(--bar) 0 0 var(--amber)' : ''};color:${r[3] === 'Dead' ? 'var(--tx3)' : 'var(--tx1)'}">
    <span>${r[3] ? ui.status(r[3], { C4: 'c4', Kit: 'kit', Dead: 'dead', Bot: '' }[r[3]]) : ''}</span><span class="body" style="font-size:19px">${r[0]}</span><span class="num-s" style="text-align:right">${r[1]}</span><span class="num-s" style="text-align:right">${r[2]}</span></div>${r[5] ? playerMenu(r[0], r[5] === 'mate') : ''}`;
  const team = (name, jp, score, c, rows, i) => `<div class="tune" style="--i:${i};margin-top:32px">
    <div style="display:flex;align-items:center;background:var(--bg1);box-shadow:inset var(--bar) 0 0 ${c}">
      <div style="padding:12px var(--pad);flex:1;display:flex;align-items:baseline;gap:var(--gap)"><span class="h2" style="font-size:40px">${name}</span><span class="jp">${jp}</span><span class="label">${rows.length} players</span></div>
      <div class="num-m" style="padding:0 var(--pad);color:${c}">${String(score).padStart(2, '0')}</div></div>
    <div style="display:grid;grid-template-columns:96px 1fr 96px 96px;height:36px;align-items:center;padding:0 var(--pad)"><span></span><span class="label">Player</span><span class="label" style="text-align:right">K</span><span class="label" style="text-align:right">D</span></div>
    ${rows.map(row).join('')}</div>`;
  return `<div class="abs" style="left:360px;right:360px;top:160px">
    <div class="tune" style="display:flex;align-items:center"><span class="label">de_dust · Round 13</span><span class="num-s" style="margin-left:auto">1:42</span></div>
    <div class="tune" style="display:flex;gap:4px;margin-top:20px;--i:1">${strip}</div>
    ${team('Bureau', 'ビューロー', 7, 'var(--ct-color)', [['anas', 18, 9, 'Kit', 1], ['Ghost', 15, 10, '', 0, 'mate'], ['Sarge', 9, 11], ['Hawk', 11, 12, 'Dead'], ['Nova', 6, 13, 'Bot']], 2)}
    ${team('Strays', 'ストレイズ', 5, 'var(--t-color)', [['Viper', 17, 11], ['Kane', 8, 12, 'C4'], ['Mako', 7, 13], ['Rook', 12, 13, 'Dead'], ['Blitz', 10, 12, 'Dead']], 3)}</div>`;
}

// ---- the system, drawn -----------------------------------------------------------------
function system() {
  const sw = [['bg0', '#07080B'], ['bg1', '#0E1116'], ['bg2', '#151A21'], ['bg3', '#1D242D'], ['tx1', '#EAF2F8', 1], ['tx2', '#8E9BA8', 1], ['tx3', '#56616D', 1], ['amber', '#FFA000', 1], ['ct-color · Bureau', '#49C6FF', 1], ['t-color · Strays', '#FF4F8B', 1], ['ok', '#5BE3A0', 1], ['bad', '#FF4040', 1]];
  const type = [['Display · 150', '<span class="display" style="font-size:80px">Bootleg</span>'], ['H2 · 52', '<span class="h2">Armory</span>'], ['H3 · 28', '<span class="h3">Bureau</span>'], ['Body · 18', '<span class="body">Bomb\'s down on A. Hold your angle.</span>'], ['Label · 13', '<span class="label">Round 13 · Buy zone</span>'], ['Jp · 13', '<span class="jp">ビューロー</span>'], ['Num L / M / S', '<span class="num-l">100</span> <span class="num-m">1:42</span> <span class="num-s">$3250</span>']];
  const line = (a, b) => `<div style="display:flex;align-items:center;gap:var(--gap);min-height:36px;border-top:1px solid var(--line)"><span class="num-s" style="width:96px;font-size:15px;color:var(--amber)">${a}</span><span class="body" style="font-size:15px">${b}</span></div>`;
  return `<div class="phead">${ui.pageTitle('One source: client/src/ui/tokens.rs', 'Design system', 'デザイン')}</div>
  <div class="abs" style="left:var(--margin);top:192px;width:760px">
    <div class="label">Colour · amber is you · cyan and pink are the two channels</div>
    <div style="display:grid;grid-template-columns:repeat(6,1fr);gap:8px;margin-top:12px">${sw.map(s => `<div style="height:64px;padding:8px;display:flex;align-items:flex-end;background:${s[1]};color:${s[2] ? '#07080B' : 'var(--tx2)'};${s[2] ? '' : 'box-shadow:inset 0 0 0 1px var(--line)'};font:700 11px/1.3 var(--f-num)">${s[0]}<br>${s[1]}</div>`).join('')}</div>
    <div class="label" style="margin-top:32px">Type scale · Big Shoulders, Chakra Petch, JetBrains Mono, Noto Sans JP</div>
    <div style="margin-top:8px">${type.map(t => `<div style="display:flex;align-items:baseline;gap:var(--pad);padding:10px 0;border-top:1px solid var(--line)"><span class="label" style="width:150px;flex:none">${t[0]}</span>${t[1]}</div>`).join('')}</div>
    <div class="label" style="margin-top:32px">Space · 4 px grid</div>
    <div style="display:flex;align-items:flex-end;gap:var(--gap);margin-top:12px">${[['gap-xs', 4], ['gap-s', 8], ['gap', 16], ['pad', 24], ['safe', 32], ['margin', 96]].map(([n, v]) => `<div><div style="width:${v}px;height:${v}px;background:var(--bg3);box-shadow:inset 0 0 0 1px var(--line-2)"></div><div class="label" style="margin-top:6px">${n} ${v}</div></div>`).join('')}</div>
  </div>
  <div class="abs" style="left:920px;top:192px;width:420px">
    <div class="label">Anchor · mirror to the edge you sit on</div>
    <div style="position:relative;height:168px;margin-top:12px;background:var(--bg3)">
      <div class="abs kf you bar-r" style="right:16px;top:16px"><span style="color:var(--ct-color)">anas</span>${ui.ico('d_awp', 63, 21)}<span style="color:var(--t-color)">Rook</span></div>
      <div class="abs panel hud" style="left:16px;bottom:16px;padding:12px 20px;display:flex;align-items:center;gap:12px">${ui.ico('cross', 24, 24)}<span class="num-m">100</span></div>
      <div class="abs panel hud" style="right:16px;bottom:16px;padding:12px 20px;display:flex;align-items:baseline;gap:8px"><span class="num-m">24</span><span class="num-s" style="color:var(--tx2)">/ 90</span></div></div>
    <div class="body" style="margin-top:12px;font-size:15px;color:var(--tx2)">Parts on the right edge mirror: content and the amber bar sit on the outer side.</div>
    <div class="label" style="margin-top:32px">List row · hover it</div>
    <div style="margin-top:12px">${ui.nav([['01', 'Practice', '練習'], ['02', 'Play', 'プレイ', true]], 'compact')}</div>
    <div class="label" style="margin-top:32px">Chips, stepper</div>
    <div style="display:flex;flex-direction:column;gap:8px;margin-top:12px">${ui.chips(['Quiet', 'Normal', 'Chatty'], 'Normal')}${ui.stepper('2.40')}</div>
    <div class="label" style="margin-top:32px">Status labels</div>
    <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:12px">${ui.status('Ready', 'ok')}${ui.status('Leader · you', 'you')}${ui.status('C4', 'c4')}${ui.status('Kit', 'kit')}${ui.status('Dead', 'dead')}${ui.status('Need $2000', 'bad')}</div>
  </div>
  <div class="abs" style="left:1404px;right:var(--margin);top:192px">
    <div class="label">Main action · one per page</div>
    <div style="margin-top:12px">${ui.primary('Buy · $2500').replace('min-width:420px', '')}</div>
    <div style="margin-top:8px">${ui.primary('Buy · $4750', 'Need $1500', true)}</div>
    <div class="label" style="margin-top:32px">Signal · glitch only while something changes</div>
    <div style="display:flex;gap:12px;margin-top:12px">${[['Changing', 6], ['At rest', 0]].map(([l, v]) => `<div style="flex:1;padding:16px 20px;box-shadow:inset 0 0 0 1px var(--line)"><div class="h2 chroma" style="font-size:36px;--split:${v}px;color:var(--ct-color)">Bureau</div><div class="h2 chroma" style="font-size:36px;--split:${v}px;color:var(--t-color);margin-top:4px">Strays</div><div class="label" style="margin-top:10px">${l}</div></div>`).join('')}</div>
    <div class="label" style="margin-top:32px">Motion</div>
    <div style="margin-top:12px">${line('320 ms', 'tune in: split channels settle clean')}${line('240 ms', 'hover burst, then clean')}${line('40 ms', 'stagger between siblings')}${line('220 ms', 'blur, dim and one sweep behind menus')}${line('180 ms', 'tune out when closing')}</div>
    <div class="label" style="margin-top:32px">Layers</div>
    <div style="margin-top:12px">${line('20', 'Esc menu, team pick')}${line('7', 'Buy, scoreboard')}${line('6', 'AI assistant')}${line('5', 'Scrim: dim and blur')}${line('0', 'HUD')}</div>
  </div>`;
}
