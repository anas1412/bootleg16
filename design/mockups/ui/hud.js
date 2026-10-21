// The HUD, live (client/src/hud_motion.rs does the same in the game). A demo loop on the HUD
// screens: hits, shots, a reload, money, a weapon switch, voices, buy zone and kit, and the
// timer's last 30 s (as CS 1.6: the clock flips to red, faster as time runs out, no sound). Every change uses the same motion as the menus: values count or roll,
// bars drain with a trail, edges flare, things open and close in place.
function hudLive(root) {
  const $ = id => root.querySelector('#' + id);
  if (!$('vitals') || document.body.classList.contains('still')) return;
  const alive = () => root.isConnected && $('vitals').isConnected;
  const at = (ms, f) => setTimeout(() => alive() && f(), ms);
  const flare = el => restart(el, 'flare');

  // a number that counts to its new value (fast, eased)
  const count = (el, to, ms = 260, fmt = v => String(v)) => {
    const from = parseInt(el.textContent.replace(/\D/g, ''), 10) || 0, t0 = performance.now();
    const f = now => { const k = Math.min(1, (now - t0) / ms), e = 1 - (1 - k) ** 3; el.textContent = fmt(Math.round(from + (to - from) * e)); if (k < 1 && alive()) requestAnimationFrame(f); };
    requestAnimationFrame(f);
  };
  // a meter: the fill moves at once; going down, the trail stays and drains after it
  const meter = (bar, ghost, pct) => {
    const down = pct < parseFloat(bar.style.width);
    bar.style.width = pct + '%';
    if (!down) { ghost.style.transition = 'none'; void ghost.offsetWidth; }
    ghost.style.width = pct + '%';
    if (!down) requestAnimationFrame(() => (ghost.style.transition = ''));
  };
  const state = { hp: 100, ap: 67, clip: 24, max: 30, cash: 3250, t: 20 };
  const vitals = (hp, ap) => {
    const v = $('vitals'), hurt = hp < state.hp;
    state.hp = hp; state.ap = ap;
    count($('hp'), hp); count($('ap'), ap);
    meter($('hp-bar'), $('hp-ghost'), hp); meter($('ap-bar'), $('ap-ghost'), ap);
    if (hurt) { restart(v, 'hit'); at(520, () => v.classList.remove('hit')); }
    v.classList.toggle('low', hp <= 25);
  };
  const shoot = () => {
    if (!state.clip) return;
    state.clip--;
    const c = $('clip');
    c.textContent = state.clip; restart(c, 'roll-down');
    c.classList.toggle('low', state.clip <= state.max * .2);
    flare($('ammo'));
  };
  const reload = (ms) => {
    const a = $('ammo');
    a.style.setProperty('--rt', ms + 'ms');
    restart(a, 'reloading');
    at(ms, () => { a.classList.remove('reloading'); state.clip = state.max; const c = $('clip'); c.textContent = state.clip; c.classList.remove('low'); restart(c, 'roll-up'); flare(a); });
  };
  const cash = (d) => {
    state.cash += d;
    count($('cash'), state.cash, 420);
    const el = $('delta');
    el.textContent = (d > 0 ? '+' : '−') + Math.abs(d);
    el.style.color = d > 0 ? 'var(--ok)' : 'var(--tx2)';
    restart(el, 'go'); flare($('money'));
  };
  const weapon = (name, ico, clip, max, reserve) => {
    swap($('wpn'), `<span class="label" style="color:var(--tx1)">${name}</span>${ui.ico(ico, 108, 36, 'var(--tx2)')}`);
    state.clip = clip; state.max = max;
    const c = $('clip'); c.textContent = clip; c.classList.remove('low'); restart(c, 'roll-up');
    $('reserve').textContent = '/ ' + reserve; flare($('ammo'));
  };
  // the carried strip: the one in your hands bright
  const own = id => root.querySelectorAll('.own').forEach(o => o.classList.toggle('on', o.id === 'own-' + id));
  const status = (id, on) => { const s = $(id); if (s.classList.contains('on') === on) return; s.classList.toggle('on', on); if (on) restart(s, 'pop'); flare($('money')); tone([on ? 880 : 660], { vol: .03 }); };
  const voice = (name, color, ch, on) => {
    const list = $('voices'), row = list.querySelector(`[data-voice="${name}"]`);
    if (on && !row) list.insertAdjacentHTML('beforeend', voiceRow(name, color, ch));
    if (!on && row) { row.classList.add('out-self'); at(T_OUT, () => row.remove()); }
  };
  // voice levels: each talking row's bars follow a made-up voice
  const levels = () => {
    for (const row of root.querySelectorAll('.vc')) {
      const bars = row.querySelectorAll('.vc-lv i'), v = Math.random();
      bars.forEach((b, i) => { b.style.height = (3 + Math.max(0, v - i * .12) * 12 * (0.6 + Math.random() * .4)).toFixed(1) + 'px'; });
    }
    if (alive()) setTimeout(levels, 70);
  };
  levels();
  // the round timer (game: round_hud top_bar): from 30 s it flips to red and back, every
  // 0.8 s, then 0.4 from 20, 0.2 from 10, 0.1 from 5, 0.05 from 2, red for good at 0; the
  // last ten seconds also swell. Silent, as in CS 1.6.
  const period = l => l <= 2 ? .05 : l <= 5 ? .1 : l <= 10 ? .2 : l <= 20 ? .4 : .8;
  let end = performance.now() + state.t * 1000, shownSec = -1;
  state.restart = secs => { end = performance.now() + secs * 1000; };
  const clock = () => {
    if (!alive()) return;
    const left = Math.max(0, (end - performance.now()) / 1000), sec = Math.ceil(left), t = $('clock-t');
    if (sec !== shownSec) {
      t.textContent = `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, '0')}`;
      if (left <= 10 && sec > 0) restart(t, 'tick');
      shownSec = sec;
    }
    const warn = left <= 30;
    $('clock').classList.toggle('warn', warn);
    $('clock').classList.toggle('red', warn && (left <= 0 || Math.floor(left / period(left)) % 2 === 0));
    setTimeout(clock, 25);
  };
  // the live demo is past buy time, and starts before the kit is bought
  $('round-l').textContent = 'Round 13';
  $('own-kit').classList.add('gone');
  clock();

  // the loop
  const loop = () => {
    if (!alive()) return;
    at(600, () => vitals(73, 58));
    [1600, 1720, 1840, 1960].forEach(ms => at(ms, shoot));
    at(2800, () => voice('Ghost', 'var(--ct-color)', 'Team', false));
    at(3400, () => cash(300));
    at(4400, () => vitals(22, 41));
    at(5200, () => reload(2500));
    at(6000, () => voice('Hawk', 'var(--ct-color)', 'Near', true));
    at(7000, () => { cash(-200); $('own-kit').classList.remove('gone'); restart($('own-kit'), 'pop'); });
    at(8400, () => { weapon('USP', 'd_usp', 12, 12, 100); own('usp'); });
    at(9200, () => status('st-buy', false));
    [9800, 10100, 10400].forEach(ms => at(ms, shoot));
    at(12500, () => voice('Hawk', '', '', false));
    at(15000, () => {
      // next round: back to full
      vitals(100, 67); weapon('AK-47', 'd_ak47', 24, 30, 90); own('ak'); status('st-buy', true); $('own-kit').classList.add('gone');
      state.cash = 3250; $('cash').textContent = '3250'; voice('Ghost', 'var(--ct-color)', 'Team', true);
      state.restart(21);
    });
    at(16000, loop);
  };
  loop();
}
