// Interaction and transitions (motion.rs in the game does the same):
// - go(): another screen. The current one tunes out, the next tunes in. Between menu pages
//   only the page and the assistant swap; the frame stays and the selection slides.
// - a sliding selection (.ind) in every list and chip group;
// - choices burst once, steppers roll their value, buy categories and items swap in place.
const T_OUT = 180;

// Put the sliding selection under the chosen row or chip. `animate` false snaps it.
function placeInd(group, animate = true) {
  const ind = group.querySelector(':scope > .ind');
  if (!ind) return;
  const on = group.querySelector(':scope > .on');
  if (!animate) ind.style.transition = 'none';
  if (on) Object.assign(ind.style, { opacity: 1, transform: `translate(${on.offsetLeft}px, ${on.offsetTop}px)`, width: `${on.offsetWidth}px`, height: `${on.offsetHeight}px` });
  else ind.style.opacity = 0;
  if (!animate) { void ind.offsetWidth; ind.style.transition = ''; }
}

// After new markup lands: sliding selections in place, setting rows staggered.
function decorate(root) {
  for (const g of root.querySelectorAll('.chips')) {
    if (g.querySelector(':scope > .chip') && !g.querySelector(':scope > .ind')) g.prepend(Object.assign(document.createElement('i'), { className: 'ind' }));
  }
  root.querySelectorAll('.srow').forEach((r, i) => { r.classList.add('tune'); r.style.setProperty('--i', i + 1); });
  const groups = root.querySelectorAll('.nav, .chips');
  for (const g of groups) placeInd(g, false);
  // chip widths change once the web fonts arrive
  document.fonts?.ready.then(() => { for (const g of groups) placeInd(g, false); });
}

// Swap a container's content: the old content tunes out, the new tunes in.
function swap(el, html, then) {
  el.classList.add('out');
  setTimeout(() => {
    el.classList.remove('out');
    el.innerHTML = html;
    if (el.classList.contains('tune')) { el.classList.remove('tune'); void el.offsetWidth; el.classList.add('tune'); }
    decorate(el);
    speakAll(el);
    then?.();
  }, T_OUT);
}

function restart(el, cls) { el.classList.remove(cls); void el.offsetWidth; el.classList.add(cls); }

function go(k) {
  if (k === current || busy) return;
  const a = SCREENS[current], b = SCREENS[k];
  history.replaceState(null, '', '#' + k);
  busy = true;
  if (a.menu && b.menu) {
    // Same frame: slide the selection, swap the page and the assistant.
    const nav = stage.querySelector('.nav');
    nav.querySelectorAll(':scope > .item').forEach(i => i.classList.toggle('on', i.dataset.go === k));
    placeInd(nav);
    const on = nav.querySelector('.item.on .h2');
    if (on) restart(on, 'pop');   // back on the main menu nothing is selected
    current = k;
    for (const t of tabs.children) t.setAttribute('aria-current', t.dataset.k === k);
    note.textContent = b.note;
    swap(stage.querySelector('#extra'), b.extra());
    const pg = stage.querySelector('#page');
    if (b.menu === 'Main') {
      // back to the straight menu: everything swings flat, the page leaves
      flatten3d(stage);
      pg.classList.add('out');
      setTimeout(() => { pg.classList.remove('out'); pg.classList.add('empty'); pg.innerHTML = ''; busy = false; }, T_OUT);
      return;
    }
    if (a.menu === 'Main') {
      // from the straight menu: the page comes in and everything swings into 3D
      pg.innerHTML = b.page(); pg.classList.remove('empty'); decorate(pg); restart(pg, 'tune');
      setTimeout(() => { open3d(stage); busy = false; }, 30);
      return;
    }
    swing(pg, true);
    swap(pg, b.page(), () => { busy = false; swing(pg, false); });
    return;
  }
  stage.classList.add('out');
  setTimeout(() => { stage.classList.remove('out'); show(k); busy = false; }, T_OUT);
}
let busy = false;

// Step a value like "2.40", "80%", "300" or "4".
function step(v, dir) {
  const n = parseFloat(v), pct = v.endsWith('%'), dec = v.includes('.');
  const d = dec ? 0.1 : pct ? 5 : n >= 100 ? 10 : 1;
  const x = Math.max(0, n + dir * d);
  return (dec ? x.toFixed(2) : String(Math.round(x))) + (pct ? '%' : '');
}

stage.addEventListener('click', e => {
  const t = e.target;
  const goEl = t.closest('[data-go]');
  if (goEl) return go(goEl.dataset.go);

  // buy: category, then item
  const cat = t.closest('.buycats > .item');
  if (cat) {
    const i = [...cat.parentElement.querySelectorAll(':scope > .item')].indexOf(cat);
    if (i === buyState.cat) return;
    cat.parentElement.querySelectorAll(':scope > .item').forEach((x, j) => x.classList.toggle('on', j === i));
    placeInd(cat.parentElement); restart(cat.querySelector('.h2'), 'pop');
    buyState.cat = i; buyState.sel = 0;
    swap(stage.querySelector('#cards'), buyCards(i, 0));
    swap(stage.querySelector('#details'), buyDetails(i, 0));
    return;
  }
  const card = t.closest('#cards > .card');
  if (card) {
    const i = +card.dataset.item;
    if (i === buyState.sel) return;
    stage.querySelectorAll('#cards > .card').forEach(c => c.classList.toggle('sel', c === card));
    restart(card, 'pop');
    buyState.sel = i;
    swap(stage.querySelector('#details'), buyDetails(buyState.cat, i));
    return;
  }

  // a stepper: roll the value toward the change
  const stepBtn = t.closest('.stepper > .chip');
  if (stepBtn) {
    const v = stepBtn.parentElement.querySelector('.v'), up = stepBtn === stepBtn.parentElement.lastElementChild;
    v.innerHTML = `<span class="${up ? 'roll-up' : 'roll-down'}">${step(v.textContent.trim(), up ? 1 : -1)}</span>`;
    return;
  }
  // a choice in a group: the selection slides to it, it bursts once
  const chip = t.closest('.chips > .chip');
  if (chip && !chip.classList.contains('on')) {
    const g = chip.parentElement;
    g.querySelectorAll(':scope > .chip').forEach(c => c.classList.toggle('on', c === chip));
    placeInd(g); restart(chip, 'pop');
    return;
  }
  // an assistant card: say it again
  const up = t.closest('.uplink[data-ai]');
  if (up) return speak(up, { stay: true });
  // pause menu: Resume closes it; side cards select
  const item = t.closest('.over .nav > .item');
  if (item && item.querySelector('.h2').textContent === 'Resume') return toggle.click();
  const side = t.closest('.over .card');
  if (side) { stage.querySelectorAll('.over .card').forEach(c => c.classList.toggle('sel', c === side)); restart(side, 'pop'); }
});

// 3D: the UI sits on a curved visor. Each part turns toward you by where it sits (left parts
// in from the left, right parts from the right, top and bottom tilt in), hinged on its outer
// edge; `data-depth` pushes a layer back (negative) or forward.
const CURVE_Y = 26, CURVE_X = 18;   // degrees at the screen edges (the HUD)
// Persona-style: the main menu sits straight and flat (no page). Opening something brings its
// page in and swings every part into its 3D place in turn, overshooting a little so it lands
// with weight; Esc swings it all back flat and the page leaves. The HUD takes its curve at once.
// `?p=0..1` freezes the swing part way (stills).
const SETTLE = 820, SETTLE_STAGGER = 70, FLATTEN = 380;
const SETTLE_EASE = 'cubic-bezier(.2, 1.3, .32, 1)', FLATTEN_EASE = 'cubic-bezier(.4, 0, .2, 1)';
function rot(ry, rx, k = 1) {
  return ry || rx ? (Math.abs(ry) > Math.abs(rx) ? `y ${(ry * k).toFixed(1)}deg` : `x ${(rx * k).toFixed(1)}deg`) : 'none';
}
function menuParts(root) { return [...root.querySelectorAll(':scope > *:not(.world):not(script)')]; }
// Put every part k of the way to its 3D place (0 flat, 1 placed), animated or not.
function placeAll(root, k, { dur = 0, ease = SETTLE_EASE, stagger = 0 } = {}) {
  menuParts(root).forEach((el, i) => {
    if (!el.dataset.place) return;
    const [ry, rx, z] = JSON.parse(el.dataset.place);
    const inner = el.matches('[data-tilt="none"]:not(.space):not(.far16)') ? [...el.querySelectorAll('.abs')] : [];
    const t = dur ? `rotate ${dur}ms ${ease} ${i * stagger}ms, translate ${dur}ms ${ease} ${i * stagger}ms` : 'none';
    el.style.transition = t;
    el.style.translate = `0 0 ${(z * k).toFixed(1)}px`;
    el.style.rotate = rot(ry, rx, k);
    for (const n of inner) { n.style.transition = t; n.style.transformOrigin = 'right center'; n.style.rotate = `y ${(-14 * k).toFixed(1)}deg`; }
  });
}
function curve(root) {
  const hud = !!root.querySelector('.hudlayer');
  const parts = hud ? root.querySelectorAll('.hudlayer > .abs:not(.over):not(.scrim):not(.sweep)') : menuParts(root);
  const still = document.body.classList.contains('still');
  const p = Math.min(1, Math.max(0, parseFloat(new URLSearchParams(location.search).get('p') ?? '1')));
  for (const el of parts) {
    if (el.classList.contains('world') || el.classList.contains('over')) continue;
    const r = el.getBoundingClientRect(), b = root.getBoundingClientRect(), k = 1920 / b.width;
    const cx = ((r.left + r.right) / 2 - b.left) * k / 1920 - .5, cy = ((r.top + r.bottom) / 2 - b.top) * k / 1080 - .5;
    const wide = r.width * k > 1500, tall = r.height * k > 900;
    // only the HUD follows the visor curve; menu screens turn only the parts that ask to
    // (`data-tilt="y 12"`)
    const [axis, deg] = (el.dataset.tilt ?? '').split(' ');
    let ry = !hud || wide ? 0 : -cx * 2 * CURVE_Y, rx = !hud || tall ? 0 : cy * 2 * CURVE_X;
    if (axis) { ry = axis === 'y' ? +deg : 0; rx = axis === 'x' ? +deg : 0; }
    const z = parseFloat(el.dataset.depth ?? '0') * 160;
    // HUD: hinged on the side toward the middle, so the outer edge comes toward you (curved
    // visor glass); menus set their own hinge
    el.style.transformOrigin = el.dataset.hinge ?? (Math.abs(ry) >= Math.abs(rx) ? `${cx < 0 ? 'right' : 'left'} center` : `center ${cy < 0 ? 'bottom' : 'top'}`);
    el.dataset.place = JSON.stringify([ry, rx, z]);
  }
  // the HUD doesn't turn part by part: it bends as one surface (visor, below)
  if (hud) return visor(root);
  // a menu page open: shown placed (a still at p), or arrive flat then swing; none: flat
  const open = !!root.querySelector('#page:not(.empty)');
  placeAll(root, open && still ? p : 0);
  if (open && !still) setTimeout(() => placeAll(root, 1, { dur: SETTLE, stagger: SETTLE_STAGGER }), 30);
}
// The HUD seen through a visor: a very slight bulge, the middle swelling toward you and the
// edges leaning back. The game bends the HUD's image with one shader (smooth sampling); here
// each panel takes the same bulge in real 3D, which the browser draws smoothly.
const BULGE_Z = 40, BULGE_Y = 9, BULGE_X = 7;   // px toward you at the middle; degrees at the edges
function visor(root) {
  const b = root.getBoundingClientRect(), k = 1920 / b.width;
  // ?bulge=0..2 scales it (comparison shots)
  const m = parseFloat(new URLSearchParams(location.search).get('bulge') ?? '1');
  for (const el of root.querySelectorAll('.hudlayer > .abs:not(.over):not(.scrim):not(.sweep)')) {
    const r = el.getBoundingClientRect();
    const u = (((r.left + r.right) / 2 - b.left) * k / 1920 - .5) * 2, v = (((r.top + r.bottom) / 2 - b.top) * k / 1080 - .5) * 2;
    if (r.width * k > 1500) continue;
    const r2 = u * u + v * v;
    el.style.transformOrigin = 'center';
    el.style.translate = `0 0 ${(m * BULGE_Z * (1 - r2 / 2)).toFixed(1)}px`;
    // the outer edges lean away: left panels turn their left edge back, top panels their top
    const ax = -v * BULGE_X * m, ay = u * BULGE_Y * m, ang = Math.hypot(ax, ay);
    el.style.rotate = ang ? `${(ax / ang).toFixed(3)} ${(ay / ang).toFixed(3)} 0 ${ang.toFixed(2)}deg` : 'none';
  }
}

// Opening a page from the straight menu, and Esc back to it.
function open3d(root) { placeAll(root, 1, { dur: SETTLE, stagger: SETTLE_STAGGER }); }
function flatten3d(root) { placeAll(root, 0, { dur: FLATTEN, ease: FLATTEN_EASE }); }
// A page swap while open: the page straightens while it swaps, then swings back into place.
function swing(el, flat) {
  if (!el?.dataset.place) return;
  const [ry, rx, z] = JSON.parse(el.dataset.place);
  el.style.transition = flat ? `rotate ${T_OUT}ms ease-in, translate ${T_OUT}ms ease-in` : `rotate ${SETTLE}ms ${SETTLE_EASE}, translate ${SETTLE}ms ${SETTLE_EASE}`;
  el.style.rotate = flat ? 'none' : rot(ry, rx);
  el.style.translate = flat ? '0 0 0' : `0 0 ${z}px`;
}
// Esc: a menu page closes back to the straight main menu.
document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && SCREENS[current]?.menu && SCREENS[current].menu !== 'Main') go('main');
});

