// Shows one 1920x1080 screen (?s=menu|hud|buy|score), scaled to fit the window.
const screens = ['menu', 'play', 'invite', 'maps', 'char', 'settings', 'moods', 'hud', 'buy', 'score', 'system', 'system2'].filter(x => document.getElementById(x));
const dir = location.pathname.split('/').pop().replace('.html', '');
const s = new URLSearchParams(location.search).get('s') || 'menu';
document.getElementById(s).classList.add('on');
function fit() {
  const k = Math.min(innerWidth / 1920, innerHeight / 1080);
  for (const el of document.querySelectorAll('.stage')) {
    el.style.transform = `translate(${(innerWidth - 1920 * k) / 2}px, ${(innerHeight - 1080 * k) / 2}px) scale(${k})`;
  }
}
addEventListener('resize', fit); fit();
if (!new URLSearchParams(location.search).has('clean')) {
  const nav = document.createElement('div'); nav.id = 'nav';
  for (const d of ['a', 'b', 'c', 'd', 'e']) nav.innerHTML += `<a href="${d}.html?s=${s}" class="${d === dir ? 'on' : ''}">${d.toUpperCase()}</a>`;
  nav.innerHTML += '<span style="width:8px"></span>';
  for (const x of screens) nav.innerHTML += `<a href="?s=${x}" class="${x === s ? 'on' : ''}">${x}</a>`;
  document.body.appendChild(nav);
}
