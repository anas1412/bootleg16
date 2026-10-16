// The face rig: makes every portrait alive (assistant/view.rs mirrors it with sprite layers).
// - Head turn with depth: layers move by different amounts (back hair against the turn, the
//   features and fringe the most) and the head tilts on the neck.
// - Idle: a slow drift and blinks. Talking: the mouth moves and the head nods with it.
// - Moods: each has a pose the head eases into; changing mood glitches in.
// - Personality scales all of it: ChildGBT barely moves, Grumpk and the sister overact.

// yaw (turn, + to her left), pitch (+ down), roll (+ tilt clockwise), in rig units (~degrees).
const POSE = {
  neutral: [0, 0, 0], happy: [2, -2, 5], laughing: [0, -6, 7], smug: [5, -4, -7], shocked: [0, -7, 0],
  thinking: [-7, -4, 8], worried: [-2, 3, -4], sad: [-3, 7, -5], annoyed: [9, 1, -4],
};
const LIVELY = { claudette: .75, childgbt: .25, sister: 1.25, brother: .6, grumpk: 1.35 };
// how far each layer moves per unit of turn / nod
const DEPTH = { 'L-back': -0.35, 'L-skull': 0.4, 'L-feat': 0.95, 'L-front': 0.75 };

const RIGS = new Set();

class Rig {
  constructor(tile) {
    this.tile = tile;
    this.who = tile.dataset.face;
    this.c = getComputedStyle(tile).getPropertyValue('--c').trim();
    this.mood = tile.dataset.mood || 'neutral';
    this.phase = Math.random() * 100;
    this.pose = [0, 0, 0];          // current, eased
    this.kick = [0, 0, 0];          // a short impulse (jolts, bounces)
    this.talking = false;
    this.nextBlink = performance.now() + 800 + Math.random() * 3000;
    this.blinkUntil = 0;
    this.flap = 0;
    this.bind();
    RIGS.add(this);
    tile.rig = this;
  }
  bind() {
    const q = c => [...this.tile.querySelectorAll('.' + c)];
    this.g = Object.fromEntries(Object.keys(DEPTH).map(k => [k, q(k)]));
    this.head = q('L-head');
    this.eyes = q('L-eyes'); this.shut = q('L-shut'); this.mouth = q('L-mouth'); this.talk = q('L-talk');
  }
  render() {
    this.tile.innerHTML = faceSvg(this.who, this.c, this.mood, false, this.tile.offsetWidth < 100 ? TILE_VIEW : undefined);
    this.bind();
  }
  setMood(m) {
    if (m === this.mood) return;
    this.mood = m;
    this.tile.dataset.mood = m;
    this.render();
    restart(this.tile, 'pop');                       // the change glitches in
    if (m === 'shocked') this.kick = [0, -8, 0];     // a jolt back
    if (m === 'laughing') this.kick = [0, -4, 6];
  }
  setWho(w, mood = this.mood) { this.who = w; this.mood = mood; this.tile.dataset.mood = mood; this.render(); restart(this.tile, 'pop'); }
  step(now, dt) {
    const L = LIVELY[this.who] ?? .8, t = now / 1000 + this.phase;
    const base = POSE[this.mood].map(v => v * L);
    // idle drift: slow, uneven, never repeating exactly
    const idle = [Math.sin(t * .7) * 2.2 + Math.sin(t * 1.3) * 1, Math.sin(t * .9) * 1.2, Math.sin(t * .5) * 1.6].map(v => v * L);
    const target = base.map((v, i) => v + idle[i] + this.kick[i]);
    if (this.talking) target[1] += Math.sin(t * 13) * 1.4 * L;               // nods with the words
    if (this.mood === 'laughing') target[1] -= Math.abs(Math.sin(t * 9)) * 2.5 * L;
    const k = 1 - Math.exp(-dt / 0.12);
    this.pose = this.pose.map((v, i) => v + (target[i] - v) * k);
    this.kick = this.kick.map(v => v * Math.exp(-dt / 0.18));
    const [yaw, pitch, roll] = this.pose;
    for (const [cls, d] of Object.entries(DEPTH)) {
      for (const el of this.g[cls]) {
        const tx = yaw * d, ty = pitch * d;
        // the features compress a little on the side turning away
        const sx = cls === 'L-feat' ? 1 - Math.abs(yaw) * 0.006 : 1;
        el.setAttribute('transform', cls === 'L-back'
          ? `rotate(${roll} 100 140) translate(${tx} ${ty})`
          : `translate(${tx} ${ty}) translate(100 101) scale(${sx} 1) translate(-100 -101)`);
      }
    }
    for (const el of this.head) el.setAttribute('transform', `rotate(${roll} 100 140)`);
    // blinks (ChildGBT almost never blinks)
    if (now > this.nextBlink && !['happy', 'laughing'].includes(this.mood)) {
      this.blinkUntil = now + 110;
      this.nextBlink = now + (this.who === 'childgbt' ? 6000 + Math.random() * 4000 : 2200 + Math.random() * 3200);
    }
    const shut = now < this.blinkUntil;
    for (const el of this.eyes) el.setAttribute('visibility', shut ? 'hidden' : 'visible');
    for (const el of this.shut) el.setAttribute('visibility', shut ? 'visible' : 'hidden');
    // mouth: alternate open and mood shape while talking
    if (this.talking && now > this.flap) this.flap = now + 90 + Math.random() * 70, this.open = !this.open;
    const open = this.talking && this.open;
    for (const el of this.mouth) el.setAttribute('visibility', open ? 'hidden' : 'visible');
    for (const el of this.talk) el.setAttribute('visibility', open ? 'visible' : 'hidden');
  }
}

// Rig every portrait in new markup; one loop drives them all.
function rigAll(root) {
  if (document.body.classList.contains('still') || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  for (const tile of root.querySelectorAll('.tile')) if (!tile.rig) new Rig(tile);
}
let rigLast = performance.now();
(function loop(now) {
  const dt = Math.min(0.05, (now - rigLast) / 1000); rigLast = now;
  for (const r of RIGS) { if (!r.tile.isConnected) RIGS.delete(r); else r.step(now, dt); }
  requestAnimationFrame(loop);
})(rigLast);
