// The uplink's timeline (assistant/view.rs does the same):
//   0 ms     a chirp; the edge light draws out from the centre, split into its two channels,
//            and settles clean;
//   100 ms   the panel unfolds down from it, one scanline sweeps it;
//   160 ms   the icon tunes in on the edge (Grumpk slams in) and four brackets lock onto it;
//   420 ms   the line resolves from the centre outward. While it talks its voice drives the
//            visual: the waveforms carry it out from the icon to both ends, the edge light and
//            the icon's glow breathe with it;
//   Twins    a short chirp; the icon glitches over to him (his own mood), her line dims, a
//            second row opens and his line resolves;
//   hold     a timer line shrinks to the centre for 4 s; then the text fades, the panel folds
//            into the edge, the icon tunes out, the light retracts to the centre, a lower chirp.
const OPEN = 420, HAND = 260, HOLD = 4000;

// How long the mouth moves: about as long as reading the line aloud.
function talkTime(s) { return Math.min(3200, 600 + s.length * 38); }

// The voice as loudness over time (one value per 20 ms): vowels peak, consonants are lower,
// spaces and punctuation dip, eased so it moves like speech rather than flickers.
function envelope(s, ms) {
  const n = Math.max(1, Math.round(ms / 20)), out = new Float32Array(n);
  let v = 0;
  for (let i = 0; i < n; i++) {
    const ch = s[Math.floor(i / n * s.length)] || ' ';
    const target = /[aeiouy]/i.test(ch) ? .85 + Math.random() * .15 : /[a-z0-9]/i.test(ch) ? .45 + Math.random() * .25 : /[.,!?:]/.test(ch) ? 0 : .12;
    v += (target - v) * .35;
    out[i] = v;
  }
  return out;
}

// UI sounds (the game plays the same, quiet, so footsteps stay clear). Comm chirps: open
// rises, the Twins hand-over is one blip, close falls; each assistant has its own pitch.
const PITCH = { claudette: 660, childgbt: 990, twins: 784, grumpk: 440 };
let audio;
function tone(freqs, { gap = .06, len = .09, vol = .06, type = 'triangle' } = {}) {
  try {
    if (!audio || audio.state !== 'running') return;
    freqs.forEach((f, i) => {
      const o = audio.createOscillator(), g = audio.createGain(), t = audio.currentTime + i * gap;
      o.type = type; o.frequency.setValueAtTime(f, t);
      g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(vol, t + .008); g.gain.exponentialRampToValueAtTime(.0001, t + len);
      o.connect(g).connect(audio.destination); o.start(t); o.stop(t + len + .02);
    });
  } catch { /* no audio: the visuals carry it */ }
}
function chirp(ai, kind) {
  const f = PITCH[ai] || 660;
  tone({ open: [f, f * 1.5], hand: [f * 1.5], close: [f * 1.25, f * .84] }[kind]);
}
// sound only after a click (browsers need one)
document.addEventListener('pointerdown', () => { try { audio ??= new AudioContext(); audio.resume(); } catch {} }, { once: true });

// While it talks: the waveforms show the voice moving outward (each bar is the voice a little
// earlier, quieter toward the ends), and --env drives the edge light and the icon's glow.
function visualise(card, env) {
  cancelAnimationFrame(card.raf);
  const bars = [...card.querySelectorAll('.ul-wave')].map(w => [...w.children]);
  const t0 = performance.now(), n = bars[0].length;
  let level = 0;
  const frame = (now) => {
    const at = (now - t0) / 20, live = at < env.length;
    const sample = (k) => (live ? env[Math.max(0, Math.floor(at - k * 1.8))] : 0);
    level += ((live ? env[Math.floor(at)] : 0) - level) * .25;
    card.style.setProperty('--env', level.toFixed(3));
    for (const side of bars) side.forEach((b, k) => { b.style.height = (3 + sample(k) * 15 * (1 - k / (n + 2))).toFixed(1) + 'px'; });
    if (live || level > .01) card.raf = requestAnimationFrame(frame);
    else { card.style.setProperty('--env', 0); for (const side of bars) side.forEach(b => (b.style.height = '3px')); }
  };
  card.raf = requestAnimationFrame(frame);
}

function speak(card, { stay = false, again = false } = {}) {
  if (document.body.classList.contains('still')) return;
  clearTimeout(card.timer);
  const ai = card.dataset.ai, rig = card.querySelector('.tile').rig;
  const lines = card.dataset.text.split('|'), moods = card.dataset.moods.split('|');
  const rows = [...card.querySelectorAll('.ul-row')], name = card.querySelector('.ul-name');
  if (rig) rig.setWho(AI[ai].face, moods[0]);
  if (rows[1]) name.textContent = AI[ai].name + ' · her';
  card.classList.remove('closing', 'talking', 'holding');
  rows.forEach((r, i) => { r.classList.remove('reveal', 'past'); r.classList.add('waiting'); r.classList.toggle('closed', i > 0); });
  restart(card, 'opening');
  chirp(ai, 'open');
  if (ai === 'grumpk') restart(card.querySelector('.ul-face'), 'slam');
  const at = (ms, f) => { card.timer = setTimeout(f, ms); };
  const say = (r, s) => {
    r.classList.remove('waiting'); restart(r, 'reveal');
    card.classList.add('talking'); if (rig) rig.talking = true;
    visualise(card, envelope(s, talkTime(s)));
  };
  const quiet = () => { card.classList.remove('talking'); if (rig) rig.talking = false; };
  const end = () => {
    quiet();
    if (stay) return;
    restart(card, 'holding');
    at(HOLD, () => {
      card.classList.remove('holding'); card.classList.add('closing'); chirp(ai, 'close');
      if (again) at(1400, () => speak(card, { again }));   // the mockup loops it
    });
  };
  at(OPEN, () => {
    say(rows[0], lines[0]);
    if (!rows[1]) return at(talkTime(lines[0]), end);
    at(talkTime(lines[0]), () => {
      quiet(); chirp(ai, 'hand');
      if (rig) { rig.setWho('brother', moods[1] || moods[0]); restart(card.querySelector('.ul-face'), 'pop'); }
      rows[0].classList.add('past'); name.textContent = AI[ai].name + ' · him'; rows[1].classList.remove('closed');
      at(HAND, () => { say(rows[1], lines[1]); at(talkTime(lines[1]), end); });
    });
  });
}

// Every uplink in new markup opens and speaks once, then stays (so the page can be studied);
// the one in the HUD runs the whole cycle, closing included, again and again.
function speakAll(root) {
  if (document.body.classList.contains('still')) return;
  rigAll(root);
  for (const t of root.querySelectorAll('[data-cycle]')) cycle(t);
  for (const card of root.querySelectorAll('.uplink[data-ai]')) speak(card, card.closest('.keep') ? { again: true } : { stay: true });
}

// The live demo: a portrait walking through all nine moods.
function cycle(tile) {
  let i = 0;
  const tick = () => { if (!tile.isConnected) return; tile.rig?.setMood(MOODS9[i = (i + 1) % 9]); tile.parentElement.querySelector('.mood-name').textContent = MOODS9[i]; setTimeout(tick, 2200); };
  setTimeout(tick, 1600);
}
