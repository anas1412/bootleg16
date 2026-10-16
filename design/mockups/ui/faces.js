// The AI assistants' faces: anime busts drawn as holographic projections in each assistant's
// colour. One source for the mockup and the game sheets (9 moods × still/talking).
// Construction: a wide cranium tapering to a small chin, large eyes at mid face, layered hair
// with pointed locks. Lines: a heavier silhouette, fine inner lines. The projection: lit face,
// mid-tone hair, clothes fading out, glowing irises and edges, scanlines.
const MOODS9 = ['neutral', 'happy', 'laughing', 'smug', 'shocked', 'thinking', 'worried', 'sad', 'annoyed'];
const INK = '#10141B', WHITE = '#FFFFFF';

function hex(c) { return [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16)); }
function mix(a, b, t) { const A = hex(a), B = hex(b); return '#' + A.map((x, i) => Math.round(x + (B[i] - x) * t).toString(16).padStart(2, '0')).join(''); }
function palette(c) {
  return { c, skin: mix(WHITE, c, .16), skinShade: mix(WHITE, c, .38), hair: c, hairDark: mix(c, '#000000', .42), hairLight: mix(c, WHITE, .5),
    cloth: mix(c, '#05070A', .7), clothLight: mix(c, '#05070A', .45), iris: mix(c, '#000000', .25), irisLight: mix(c, WHITE, .35), blush: mix(c, '#FF4F8B', .6) };
}
// A path. o: f fill, s stroke, w width, op opacity, c class (skin | hair | hairD | cloth | sil).
const P = (d, o = {}) => `<path d="${d}" fill="${o.f || 'none'}"${o.c ? ` class="${o.c}"` : ''}${o.s ? ` stroke="${o.s}" stroke-width="${o.w ?? 2}" stroke-linecap="round" stroke-linejoin="round"` : ''}${o.op != null ? ` opacity="${o.op}"` : ''}/>`;
const E = (cx, cy, rx, ry, f, o = '') => `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" fill="${f}" ${o}/>`;

// ---- anatomy (eyes on y 101, x 82 and 118; chin at 137) ----
const EY = 101, EXL = 82, EXR = 118;
const FACE_F = 'M64 78 C63 98 68 114 80 126 Q90 135 100 137 Q110 135 120 126 C132 114 137 98 136 78 C136 50 122 36 100 36 C78 36 64 50 64 78 Z';
const FACE_C = 'M63 80 C62 100 68 116 81 127 Q91 134 100 135 Q109 134 119 127 C132 116 138 100 137 80 C137 50 122 35 100 35 C78 35 63 50 63 80 Z';
// adults: longer faces; his jaw square
const FACE_AF = 'M65 78 C64 100 70 119 82 131 Q92 140 100 142 Q108 140 118 131 C130 119 136 100 135 78 C135 50 122 36 100 36 C78 36 65 50 65 78 Z';
const FACE_AM = 'M62 78 C61 102 63 121 72 133 Q85 143 100 144 Q115 143 128 133 C137 121 139 102 138 78 C138 50 122 35 100 35 C78 35 62 50 62 78 Z';
const FACE_M = 'M63 78 C62 100 66 116 77 127 Q88 137 100 138 Q112 137 123 127 C134 116 138 100 137 78 C137 50 122 35 100 35 C78 35 63 50 63 78 Z';
function bust(p, kind, collar) {
  const male = kind === 'm', child = kind === 'c';
  const neck = P(male ? 'M89 124 L88 154 L112 154 L111 124 Z' : child ? 'M93 126 L92 150 L108 150 L107 126 Z' : 'M92 126 L91 152 L109 152 L108 126 Z', { f: p.skin, c: 'skin' }) +
    P(male ? 'M89 131 Q100 144 111 131 L111 139 Q100 150 89 139 Z' : 'M92 132 Q100 142 108 132 L108 138 Q100 147 92 138 Z', { f: p.skinShade, op: .7 });
  const shoulders = P(male ? 'M30 172 C38 158 64 151 89 149 L111 149 C136 151 162 158 170 172 Z' : child ? 'M52 172 C58 160 74 152 92 149 L108 149 C126 152 142 160 148 172 Z'
    : 'M40 172 C48 159 70 152 91 150 L109 150 C130 152 152 159 160 172 Z', { f: p.cloth, c: 'cloth sil' });
  return neck + shoulders + collar;
}

// One eye, drawn as the right eye around (0,0); the left is mirrored by the caller.
// kind (by age): 'big' child, 'f' / 'm' teen girl / boy, 'af' / 'am' adult woman / man.
// Adults get smaller, narrower (almond) eyes; children the biggest, roundest ones.
const EYE_SIZE = { big: 1.5, f: 1.3, m: 1.12, af: 1.12, am: 1.0 };
const EYE_SQUASH = { big: 1, f: 1, m: 1, af: .78, am: .68 };
const LASHES = { big: true, f: true, af: true };
function eye(p, state, kind, dx = 0, dy = 0, id = '') {
  const s = EYE_SIZE[kind];
  const sclera = `M${-9 * s} ${2 * s} C${-6 * s} ${-6 * s} ${4 * s} ${-8.5 * s} ${11 * s} ${-4 * s} C${9 * s} ${4 * s} ${3 * s} ${7.5 * s} ${-6 * s} ${6 * s} Z`;
  const lid = `M${-10 * s} ${1.5 * s} C${-6 * s} ${-7 * s} ${5 * s} ${-9.5 * s} ${12 * s} ${-4.5 * s}`;
  const flick = !LASHES[kind] ? '' : P(`M${11 * s} ${-4.5 * s} l${3 * s} ${-2.4 * s}`, { s: INK, w: 2.2 });
  const lower = P(`M${-5 * s} ${7 * s} Q${2 * s} ${9 * s} ${8 * s} ${4.5 * s}`, { s: INK, w: 1, op: .55 });
  const clip = `<clipPath id="e${id}"><path d="${sclera}"/></clipPath>`;
  const irisR = [6.4, 7.6];
  const iris = (pin = false) => `<g clip-path="url(#e${id})">${P(sclera, { f: WHITE })}
      <ellipse class="iris" cx="${1 + dx}" cy="${1 + dy}" rx="${irisR[0] * s}" ry="${irisR[1] * s}" fill="url(#ig${id})"/>
      ${pin ? E(1 + dx, 1 + dy, 1.1 * s, 1.4 * s, INK) : E(1 + dx, 1.5 + dy, 2.5 * s, 3.4 * s, INK)}
      <path class="cover" d="M${-12 * s} ${-9 * s} L${14 * s} ${-9 * s} L${14 * s} ${-1 * s} Q0 ${-5 * s} ${-12 * s} ${-1 * s} Z" fill="${INK}" opacity=".14"/></g>`;
  const defs = `<defs>${clip}<linearGradient id="ig${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${p.iris}"/><stop offset="1" stop-color="${p.irisLight}"/></linearGradient></defs>`;
  switch (state) {
    case 'closed': return P(`M${-10 * s} ${1 * s} Q${1 * s} ${-7 * s} ${12 * s} ${1 * s}`, { s: INK, w: 2.6 }) + (!LASHES[kind] ? '' : P(`M${12 * s} ${1 * s} l${2.6 * s} ${-1 * s}`, { s: INK, w: 2 }));
    case 'laugh': return P(`M${9 * s} ${-5 * s} L${-6 * s} ${0} L${9 * s} ${5 * s}`, { s: INK, w: 2.6 });
    case 'half': return defs + iris() + `<path class="cover" d="M${-12 * s} ${-9 * s} L${15 * s} ${-9 * s} L${15 * s} ${-0.5 * s} L${-12 * s} ${0.5 * s} Z" fill="${p.skin}"/>` +
      P(`M${-10 * s} ${0.8 * s} Q${1 * s} ${-1.4 * s} ${12 * s} ${-0.8 * s}`, { s: INK, w: 2.8 }) + lower;
    case 'wide': return `<defs>${clip}<linearGradient id="ig${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${p.iris}"/><stop offset="1" stop-color="${p.irisLight}"/></linearGradient></defs><g clip-path="url(#e${id})">${P(sclera, { f: WHITE })}${E(1, 1, 3.8 * s, 4.6 * s, `url(#ig${id})`, 'class="iris"')}${E(1, 1.2, 1.3 * s, 1.7 * s, INK)}</g>` + P(lid, { s: INK, w: 2.2 }) + flick + lower;
    case 'pin': return defs + iris(true) + P(lid, { s: INK, w: 2.8 }) + flick + lower;
    default: return defs + iris() + P(lid, { s: INK, w: 2.8 }) + flick + lower;
  }
}
function eyes(p, l, r = l, kind = 'f', dx = 0, dy = 0, uid = '') {
  const q = EYE_SQUASH[kind];
  return `<g transform="translate(${EXR} ${EY}) scale(1 ${q})">${eye(p, r, kind, dx, dy, uid + 'r')}</g><g transform="translate(${EXL} ${EY}) scale(-1 ${q})">${eye(p, l, kind, -dx, dy, uid + 'l')}</g>`;
}
// Brows (drawn from the eye centre): rest, up, worry (inner up), angry (inner down), flat.
const BROW = { rest: 'M-8 -17 Q2 -20 13 -16', up: 'M-8 -21 Q2 -24.5 13 -20', worry: 'M-8 -21 Q2 -19.5 13 -15', angry: 'M-8 -14 Q2 -16.5 13 -20', flat: 'M-8 -17 L13 -17' };
function brows(kind, kindL = kind, w = 1.8, color = INK) {
  return `<g transform="translate(${EXR} ${EY})">${P(BROW[kind], { s: color, w })}</g><g transform="translate(${EXL} ${EY}) scale(-1 1)">${P(BROW[kindL], { s: color, w })}</g>`;
}
const MOUTH = {
  neutral: ['M96 124 Q100 125.2 104 124'], smile: ['M94 122.5 Q100 128 106 122.5'], grin: ['M93 121.5 Q100 123 107 121.5 Q105.5 130 100 130.5 Q94.5 130 93 121.5 Z', true],
  smirk: ['M95.5 125 Q102 125 106.5 121.5'], o: ['', 'o'], flat: ['M96.5 124.5 L103.5 124'], wavy: ['M95 125 Q97.5 123.4 100 125 Q102.5 126.6 105 125'],
  frown: ['M95 126.5 Q100 122.5 105 126.5'], line: ['M95.5 124.8 L104.5 124.8'], talk: ['M96 123 Q100 129 104 123 Q100 124.4 96 123 Z', true],
};
function mouth(p, k) {
  const [d, fill] = MOUTH[k];
  if (fill === 'o') return E(100, 126, 3, 3.9, INK) + E(100, 127.8, 1.8, 1.2, p.blush, 'opacity=".8"');
  if (fill) return P(d, { f: INK, s: INK, w: 1.2 }) + (k === 'grin' ? P('M95.5 127.5 Q100 130.5 104.5 127.5 Q100 129 95.5 127.5 Z', { f: p.blush, op: .9 }) : '');
  return P(d, { s: INK, w: 1.6 });
}
const nose = P('M100.6 112 l-1.4 4.4 l2.2 .2', { s: INK, w: 1.1, op: .6 });
// Comic effects keep their real colours (drawn on top of the projection, not tinted).
const FX_RED = '#FF4040', FX_BLUE = '#8FD8FF', FX_PINK = '#FF6F91';
const blush = () => E(76, 116, 7, 3, FX_PINK, 'opacity=".45"') + E(124, 116, 7, 3, FX_PINK, 'opacity=".45"');
const sweat = `<g transform="translate(140 70) scale(1.9) translate(-142 -72)">${P('M142 72 q-4 7 0 11 q4 -4 0 -11 Z', { f: FX_BLUE })}</g>`;
// the manga anger mark: four bulging curves around a gap, about the size of an eye
const vein = `<g transform="translate(132 66)">${P('M-3 -11 Q-4 -4 -11 -3 M3 -11 Q4 -4 11 -3 M-3 11 Q-4 4 -11 3 M3 11 Q4 4 11 3', { s: FX_RED, w: 3 })}</g>`;
const tear = (x, y) => P(`M${x} ${y} q-3 6 0 9 q3 -3 0 -9 Z`, { f: FX_BLUE });

// Mood → [eyes, brows, mouth, extras] for an eye kind.
function moodParts(p, m, kind, uid) {
  const k = kind;
  return {
    neutral: [eyes(p, 'open', 'open', k, 0, 0, uid), brows('rest'), 'neutral', ''],
    happy: [eyes(p, 'closed', 'closed', k, 0, 0, uid), brows('up'), 'smile', blush()],
    laughing: [eyes(p, 'laugh', 'laugh', k, 0, 0, uid), brows('up'), 'grin', blush()],
    smug: [eyes(p, 'half', 'open', k, 0, 0, uid), brows('rest', 'up'), 'smirk', ''],
    shocked: [eyes(p, 'wide', 'wide', k, 0, 0, uid), brows('up'), 'o', sweat],
    thinking: [eyes(p, 'open', 'open', k, 3, -3, uid), brows('rest', 'up'), 'flat', ''],
    worried: [eyes(p, 'open', 'open', k, 0, 1, uid), brows('worry'), 'wavy', sweat],
    sad: [eyes(p, 'half', 'half', k, 0, 2, uid), brows('worry'), 'frown', tear(128, 110)],
    annoyed: [eyes(p, 'half', 'half', k, 0, 0, uid), brows('angry'), 'line', vein],
  }[m];
}
const talkOr = (talk, m, k) => talk && !['laughing', 'shocked'].includes(m) ? 'talk' : k;
const strands = (p, d, op = .8) => P(d, { s: p.hairDark, w: 1, op });
const shine = (p, d) => P(d, { s: p.hairLight, w: 2.4, op: .75 });

// Age marks: faint inner lines that read as an adult face.
const fine = (d, op = .55) => P(d, { s: INK, w: .9, op });
const AGE_F = fine('M101.6 103 L100.8 112') + fine('M98.6 117.2 Q100 118.2 101.6 117.2', .45) + fine('M70 112 Q73 119 79 122', .4) +
  fine('M84 155 Q92 158 98 156 M116 155 Q108 158 102 156', .5);
const LIPS = fine('M96.2 124.8 Q98 123.2 100 124 Q102 123.2 103.8 124.8', .6) + fine('M97.2 129.4 Q100 130.8 102.8 129.4', .45);
const AGE_M = fine('M74 111 Q81 114.5 89 111 M126 111 Q119 114.5 111 111', .5) + fine('M92 117 Q87.5 123 89.5 130 M108 117 Q112.5 123 110.5 130', .5) +
  fine('M86 73 Q100 69.5 114 73 M90 78 Q100 75.5 110 78', .35) + fine('M98 142 Q100 145 102 142', .6) + fine('M101.8 102 L100.8 112') + fine('M71 108 l-4 -2 M129 108 l4 -2', .45);
// the lower face sits lower on a longer, adult face
const lower = (svg, dy) => dy ? `<g transform="translate(0 ${dy})">${svg}</g>` : svg;

// ---- the characters ----
const FACES = {
  // Assertive and careful, early thirties. A sleek bob, fringe swept across from a part on the
  // right, pearl earring, beauty mark. Suit and open collar.
  claudette(p, m, talk, uid) {
    const back = P('M58 76 C55 46 76 24 100 24 C124 24 145 46 142 76 C144 98 146 116 143 132 L136 138 L132 130 L126 139 L121 129 L79 129 L74 139 L68 130 L64 138 L57 132 C54 116 56 98 58 76 Z', { f: p.hairDark, c: 'hairD sil' });
    const collar = P('M86 150 L100 167 L114 150 L108 148 L100 159 L92 148 Z', { f: WHITE, op: .85 }) + P('M66 156 L90 149 L100 172 L76 172 Z M134 156 L110 149 L100 172 L124 172 Z', { f: p.clothLight, c: 'cloth' });
    const hair = P('M60 86 C56 52 76 27 106 26 C130 27 145 46 142 74 L139 88 L135 75 L131 89 L127 71 C121 79 114 83 106 85 L103 77 L97 89 L92 79 L86 93 L81 81 L74 97 L70 85 L64 99 Z', { f: p.hair, c: 'hair sil' }) +
      P('M59 84 C55 106 57 124 63 138 L68 129 L72 139 C68 119 67 101 69 87 Z', { f: p.hair, c: 'hair sil' }) +
      P('M142 76 C146 100 144 120 138 137 L133 127 L129 138 C133 118 134 100 133 85 Z', { f: p.hair, c: 'hair sil' }) +
      strands(p, 'M120 30 C112 46 100 62 84 82 M127 34 C121 50 112 64 101 78 M111 30 C102 44 90 58 76 74 M65 92 C64 108 66 122 70 132 M136 90 C137 106 135 120 132 130') +
      P('M120 28 C122 40 125 54 128 68', { s: p.hairDark, w: 1.2 }) + shine(p, 'M74 50 C84 40 98 35 112 34');
    const [ey, br, mo, ex] = moodParts(p, m, 'af', uid);
    return { back, body: bust(p, 'f', collar), head: P(FACE_AF, { f: p.skin, c: 'skin sil' }) + E(64, 114, 2.2, 2.2, WHITE) + AGE_F,
      eyes: ey, shut: eyes(p, 'closed', 'closed', 'af', 0, 0, uid + 's'), brows: br,
      mouth: lower(nose + mouth(p, mo) + (['neutral', 'smug', 'thinking', 'annoyed', 'worried', 'sad'].includes(m) ? LIPS : ''), 3), talk: lower(nose + mouth(p, talkOr(true, m, mo)), 3),
      extra: E(114, 124, 1, 1, INK), fx: ex, front: hair };
  },
  // A child AI, cold and polite (about ten). Long straight hair, a blunt hime cut, white band,
  // large still eyes. High collar.
  childgbt(p, m, talk, uid) {
    const back = P('M56 74 C54 44 76 23 100 23 C124 23 146 44 144 74 C146 110 148 140 150 168 L50 168 C52 140 54 110 56 74 Z', { f: p.hairDark, c: 'hairD sil' });
    const collar = P('M89 148 L111 148 L109 160 Q100 164 91 160 Z', { f: WHITE, op: .9 }) + P('M98 160 L102 160 L101 172 L99 172 Z', { f: p.c });
    const hair = P('M60 88 C57 52 76 28 100 28 C124 28 143 52 140 88 L132 86 L128 90 L119 86 L110 90 L100 86 L90 90 L81 86 L72 90 L68 86 Z', { f: p.hair, c: 'hair sil' }) +
      P('M60 88 C58 104 58 118 59 130 L75 130 C72 116 70 102 70 90 Z M140 88 C142 104 142 118 141 130 L125 130 C128 116 130 102 130 90 Z', { f: p.hair, c: 'hair sil' }) +
      strands(p, 'M86 34 L84 86 M100 31 L100 84 M114 34 L116 86 M64 96 L66 128 M136 96 L134 128') +
      P('M64 58 Q100 34 136 58', { s: WHITE, w: 3.6, op: .95 }) + shine(p, 'M78 46 C88 40 112 40 122 46');
    const [ey, , mo] = moodParts(p, m, 'big', uid);
    const e = m === 'laughing' ? eyes(p, 'closed', 'closed', 'big', 0, 0, uid) : ey;
    const cold = { neutral: 'neutral', smug: 'smirk', happy: 'smile', laughing: 'smile' }[m] || mo;   // she never really laughs
    const nose2 = P('M100.5 113 l-.8 3', { s: INK, w: 1, op: .55 });
    return { back, body: bust(p, 'c', collar), head: P(FACE_C, { f: p.skin, c: 'skin sil' }),
      eyes: e, shut: eyes(p, 'closed', 'closed', 'big', 0, 0, uid + 's'), brows: brows(m === 'annoyed' ? 'flat' : 'rest', m === 'annoyed' ? 'flat' : 'rest', 1.4),
      mouth: nose2 + mouth(p, cold), talk: nose2 + mouth(p, talkOr(true, m, cold)), extra: '', fx: m === 'sad' ? tear(128, 112) : '', front: hair };
  },
  // Twins, her (late teens, tsundere). High twin tails with white ties, choppy bangs, a fang.
  sister(p, m, talk, uid) {
    const tails = P('M64 50 C40 54 30 84 34 118 C37 140 44 156 52 168 L56 150 L61 163 C58 136 60 108 70 84 Z M136 50 C160 54 170 84 166 118 C163 140 156 156 148 168 L144 150 L139 163 C142 136 140 108 130 84 Z', { f: p.hair, c: 'hair sil' }) +
      strands(p, 'M44 80 C40 106 42 132 50 156 M156 80 C160 106 158 132 150 156') + E(64, 52, 4.5, 4.5, WHITE) + E(136, 52, 4.5, 4.5, WHITE);
    const back = P('M60 78 C58 46 78 26 100 26 C122 26 142 46 140 78 C140 96 138 108 134 116 L66 116 C62 108 60 96 60 78 Z', { f: p.hairDark, c: 'hairD sil' });
    const collar = P('M84 150 L100 165 L116 150 L110 148 L100 158 L90 148 Z', { f: WHITE, op: .9 }) + P('M94 163 L106 163 L108 174 L100 170 L92 174 Z', { f: p.c });
    const hair = P('M61 86 C58 50 78 30 100 30 C122 30 142 50 139 86 L134 74 L130 90 L124 72 L118 88 L112 70 L106 86 L100 68 L94 86 L88 70 L82 88 L76 72 L70 90 L66 76 Z', { f: p.hair, c: 'hair sil' }) +
      P('M61 86 C60 100 62 112 66 120 L71 106 Z M139 86 C140 100 138 112 134 120 L129 106 Z', { f: p.hair, c: 'hair sil' }) +
      strands(p, 'M100 32 L100 66 M88 36 L86 68 M112 36 L114 68') + shine(p, 'M78 46 C88 38 112 38 122 46');
    const [ey, br, mo, ex] = moodParts(p, m, 'f', uid);
    const fang = P('M103 125.4 l1.3 2.6 l1.3 -2.6 Z', { f: WHITE, s: INK, w: .5 });
    const pout = { neutral: 'flat', smug: 'smirk' }[m] || mo;
    return { back: tails + back, body: bust(p, 'f', collar), head: P(FACE_F, { f: p.skin, c: 'skin sil' }),
      eyes: ey, shut: eyes(p, 'closed', 'closed', 'f', 0, 0, uid + 's'), brows: m === 'neutral' ? brows('angry') : br,
      mouth: nose + mouth(p, pout) + (['happy', 'smug', 'laughing'].includes(m) ? fang : ''), talk: nose + mouth(p, talkOr(true, m, pout)), extra: '', fx: ex + (['neutral', 'happy', 'smug', 'shocked', 'worried'].includes(m) && !ex.includes(FX_PINK) ? blush() : ''), front: hair };
  },
  // Twins, him (late teens, the calm one). Tidy hair swept from a side part, glasses.
  brother(p, m, talk, uid) {
    const back = P('M62 78 C60 48 78 28 100 28 C122 28 140 48 138 78 C138 88 136 94 134 98 L66 98 C64 94 62 88 62 78 Z', { f: p.hairDark, c: 'hairD sil' });
    const collar = P('M84 150 L100 166 L116 150 L110 148 L100 158 L90 148 Z', { f: WHITE, op: .9 }) + P('M98 160 L102 160 L104 174 L96 174 Z', { f: p.clothLight });
    const hair = P('M62 86 C59 50 78 30 102 30 C124 30 141 48 138 84 L134 72 C128 63 118 59 108 57 C102 67 90 75 76 77 L70 90 Z', { f: p.hair, c: 'hair sil' }) +
      P('M62 86 L64 102 L68 100 L67 88 Z M138 84 L136 102 L132 100 L133 88 Z', { f: p.hair, c: 'hair' }) +
      strands(p, 'M108 57 C102 67 90 75 76 77 M116 36 C110 48 98 58 84 66 M124 40 C130 50 134 60 136 70') + shine(p, 'M80 44 C92 36 112 36 124 42');
    const [ey, br, mo, ex] = moodParts(p, m, 'm', uid);
    const glint = ['smug', 'thinking', 'annoyed'].includes(m);
    const glasses = `<g fill="none" stroke="${INK}" stroke-width="1.6"><rect x="68" y="91" width="28" height="20" rx="5"/><rect x="104" y="91" width="28" height="20" rx="5"/><path d="M96 99 Q100 96.5 104 99 M68 98 L62 96 M132 98 L138 96"/></g>` +
      (glint ? P('M72 108 L84 92 M108 108 L120 92', { s: WHITE, w: 2, op: .85 }) : '');
    return { back, body: bust(p, 'm', collar), head: P(FACE_M, { f: p.skin, c: 'skin sil' }),
      eyes: ey, shut: eyes(p, 'closed', 'closed', 'm', 0, 0, uid + 's'), brows: br,
      mouth: nose + mouth(p, mo), talk: nose + mouth(p, talkOr(true, m, mo)), extra: glasses, fx: ex, front: hair };
  },
  // Roaster and troll, forties. Spiky hair swept up, heavy brows, a scar through the right
  // brow, stubble, a hoodie.
  grumpk(p, m, talk, uid) {
    const back = P('M62 80 C60 52 78 34 100 34 C122 34 140 52 138 80 C138 90 136 96 134 100 L66 100 C64 96 62 90 62 80 Z', { f: p.hairDark, c: 'hairD sil' });
    const collar = P('M66 156 Q100 176 134 156 L138 166 Q100 186 62 166 Z', { f: p.clothLight, c: 'cloth' }) + P('M92 166 L90 178 M108 166 L110 178', { s: p.hairLight, w: 1.2 });
    // swept up and to his left, a widow's peak in front
    const hair = P('M61 90 C58 68 63 52 71 45 L57 36 L77 35 L70 19 L91 30 L95 12 L107 29 L120 13 L123 33 L141 25 L135 43 L152 44 L139 57 C143 67 143 79 140 90 L135 78 C127 71 117 67 106 70 L100 62 L94 72 C83 71 73 75 66 83 Z', { f: p.hair, c: 'hair sil' }) +
      strands(p, 'M74 44 L88 60 M95 16 L98 46 M120 17 L112 48 M140 29 L124 52 M70 56 L80 66') + P('M62 88 L64 102 M139 88 L137 102', { s: p.hair, w: 4 });
    const [ey, , mo, ex] = moodParts(p, m, 'am', uid);
    const bk = { neutral: ['flat', 'up'], happy: ['up', 'up'], laughing: ['up', 'up'], smug: ['flat', 'up'], shocked: ['up', 'up'], thinking: ['angry', 'flat'], worried: ['worry', 'worry'], sad: ['worry', 'worry'], annoyed: ['angry', 'angry'] }[m];
    const stubble = [84, 88, 92, 96, 100, 104, 108, 112, 116, 86, 90, 94, 98, 102, 106, 110, 114, 92, 96, 100, 104, 108].map((x, i) => `<circle cx="${x}" cy="${i < 9 ? 133 + Math.abs(x - 100) * -.12 : i < 17 ? 137 : 140.5}" r=".7" fill="${INK}" opacity=".55"/>`).join('');
    const grin = { neutral: 'smirk', happy: 'grin', smug: 'smirk' }[m] || mo;
    return { back, body: bust(p, 'm', collar), head: P(FACE_AM, { f: p.skin, c: 'skin sil' }) + AGE_M + stubble,
      eyes: ey, shut: eyes(p, 'closed', 'closed', 'am', 0, 0, uid + 's'), brows: brows(bk[1], bk[0], 3.4) + P('M116 80 L121 92', { s: WHITE, w: 1.4, op: .9 }),
      mouth: lower(nose + mouth(p, grin), 4), talk: lower(nose + mouth(p, talkOr(true, m, grin)), 4), extra: '', fx: ex, front: hair };
  },
};

// The face in layers, so the rig (ai.js) can turn the head with depth: back hair moves
// against a turn, the face with it, the features and fringe the most. Eyes and mouth come in
// two states each (open / shut, mood / talking) for blinking and speech.
function layers(L, talk) {
  return `<g class="L-back">${L.back}</g>${L.body}<g class="L-head"><g class="L-skull">${L.head}</g>` +
    `<g class="L-feat"><g class="L-eyes">${L.eyes}</g><g class="L-shut" visibility="hidden">${L.shut}</g><g class="L-brows">${L.brows}</g>` +
    `<g class="L-mouth"${talk ? ' visibility="hidden"' : ''}>${L.mouth}</g><g class="L-talk"${talk ? '' : ' visibility="hidden"'}>${L.talk}</g>${L.extra}</g>` +
    `<g class="L-front">${L.front}</g></g>`;
}

let FACE_N = 0;
// A face as SVG. who: claudette | childgbt | sister | brother | grumpk.
// Drawn as a projection: the art with tinted fills (face lit, hair mid, clothes dark) and
// glowing edges, a brightness layer on top, then scanlines and a fade at the shoulders.
function faceSvg(who, color, mood = 'neutral', talk = false, view = '28 22 144 144', only = null) {
  const id = 'f' + (FACE_N++), p = palette(color), light = mix(color, WHITE, .75), bg = '#07080B';
  // `only`: one layer by itself (back, body, head, extra, eyes, shut, brows, mouth, talk, fx,
  // front), for the game's sprite sheets; the rest of the drawing is left out.
  const fx = only && only !== 'fx' ? '' : FACES[who](p, mood, talk, id + 'x').fx || '';
  const art = (u) => {
    const L = FACES[who](p, mood, talk, id + u);
    return !only ? layers(L, talk) : only === 'fx' ? '' : L[only] || '';
  };
  return `<svg viewBox="${view}" xmlns="http://www.w3.org/2000/svg" style="width:100%;height:100%;display:block">
  <defs>
    <filter id="${id}b" color-interpolation-filters="sRGB"><feColorMatrix type="luminanceToAlpha"/><feComponentTransfer><feFuncA type="gamma" exponent="2.6" amplitude="1.1"/></feComponentTransfer>
      <feFlood flood-color="${color}"/><feComposite operator="in" in2="SourceGraphic"/></filter>
    <filter id="${id}g" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="1.4" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
    <pattern id="${id}s" width="4" height="3" patternUnits="userSpaceOnUse"><rect width="4" height="1" fill="${bg}" opacity=".26"/></pattern>
    <linearGradient id="${id}fg" x1="0" y1="0" x2="0" y2="1"><stop offset=".68" stop-color="#fff"/><stop offset=".95" stop-color="#fff" stop-opacity="0"/></linearGradient>
    <mask id="${id}m"><rect x="0" y="0" width="200" height="200" fill="url(#${id}fg)"/></mask>
    <style>
      #${id}e [fill]:not([fill="none"]) { fill: ${mix(color, bg, .8)}; stroke: ${light}; stroke-width: .7; }
      #${id}e .skin { fill: ${mix(light, bg, .52)} !important; }
      #${id}e .hair { fill: ${mix(color, bg, .42)} !important; }
      #${id}e .hairD { fill: ${mix(color, bg, .68)} !important; }
      #${id}e .cloth { fill: ${mix(color, bg, .82)} !important; }
      #${id}e .sil { stroke-width: 1.5 !important; }
      #${id}e [stroke] { stroke: ${light}; }
      #${id}e circle[fill="${WHITE}"], #${id}e ellipse[fill="${WHITE}"] { fill: ${light} !important; stroke: none !important; }
      #${id}e path[fill="#CFF3FF"] { fill: ${light} !important; }
      #${id}e path[fill="${WHITE}"] { fill: ${mix(light, bg, .18)} !important; }
      #${id}e .iris { fill: ${color} !important; stroke: ${light} !important; stroke-width: .5 !important; }
      #${id}e .cover { stroke: none !important; fill: ${mix(light, bg, .52)} !important; }
    </style>
  </defs>
  <g mask="url(#${id}m)">
    <g id="${id}e" filter="url(#${id}g)">${art('e')}</g>
    <g filter="url(#${id}b)" opacity=".3">${art('a')}</g>
    <g class="L-head"><g class="L-feat" filter="url(#${id}g)">${fx}</g></g>
    ${only ? '' : `<rect x="0" y="0" width="200" height="200" fill="url(#${id}s)"/>`}
  </g></svg>`;
}
