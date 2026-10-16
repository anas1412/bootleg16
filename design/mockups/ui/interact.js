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
    restart(nav.querySelector('.item.on .h2'), 'pop');
    current = k;
    for (const t of tabs.children) t.setAttribute('aria-current', t.dataset.k === k);
    note.textContent = b.note;
    swap(stage.querySelector('#extra'), b.extra());
    swap(stage.querySelector('#page'), b.page(), () => { busy = false; });
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
