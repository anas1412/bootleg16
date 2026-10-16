// The main menu's world (screens.js backdrop): the map stills cross-fade every 12 s while each
// drifts toward you.
function menuWorld(root) {
  const bd = root.querySelector('.backdrop');
  if (!bd || document.body.classList.contains('still')) return;
  const imgs = [...bd.querySelectorAll('img')];
  let i = 0;
  const alive = () => root.isConnected && bd.isConnected;
  const next = () => {
    if (!alive()) return;
    imgs[i].classList.remove('on');
    i = (i + 1) % imgs.length;
    restart(imgs[i], 'on');
    setTimeout(next, 12000);
  };
  setTimeout(next, 12000);
}
