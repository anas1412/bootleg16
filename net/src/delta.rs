//! Snapshot deltas: a snapshot sent as the bytes that changed since one the client confirmed
//! (its baseline), not in full. Most of a snapshot stays the same byte for byte from one to the
//! next (the same players, weapons, round); what moves is a few bytes per player.
//!
//! The patch is a list of ops, each `[skip][copy][n][n literal bytes]` (varints, skip
//! zigzagged): move the baseline cursor by `skip`, copy `copy` bytes from it, then take `n`
//! new bytes (which replace as many baseline bytes). Shifts (a list growing by an item, a
//! number needing another byte) are found by looking a few bytes either way.

/// How far either way to look for where the baseline lines up again.
const RESYNC: isize = 8;
/// A shifted match must be at least this long to be worth an op.
const MIN_SHIFTED: usize = 4;
/// Patched snapshots are never bigger than this (a bad patch can't blow up memory).
const MAX_OUT: usize = 64 * 1024;

fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get(p: &[u8], at: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *p.get(*at)?;
        *at += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b < 0x80 {
            return Some(v);
        }
    }
    None
}

/// How many bytes of `t[i..]` match `b[j..]`.
fn run(b: &[u8], t: &[u8], i: usize, j: isize) -> usize {
    if j < 0 {
        return 0;
    }
    let j = j as usize;
    t[i..].iter().zip(b.get(j..).unwrap_or(&[])).take_while(|(x, y)| x == y).count()
}

/// The patch that turns `base` into `target`.
pub fn diff(base: &[u8], target: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0isize);
    let (mut skip, mut copy, mut lit) = (0isize, 0usize, Vec::new());
    let flush = |out: &mut Vec<u8>, skip: isize, copy: usize, lit: &[u8]| {
        put(out, ((skip << 1) ^ (skip >> 63)) as u64);
        put(out, copy as u64);
        put(out, lit.len() as u64);
        out.extend_from_slice(lit);
    };
    while i < target.len() {
        // Where does the baseline match best: here, or a little either way?
        let (mut d, mut len) = (0, run(base, target, i, j));
        if len == 0 {
            for k in (1..=RESYNC).flat_map(|k| [-k, k]) {
                let l = run(base, target, i, j + k);
                if l > len {
                    (d, len) = (k, l);
                }
            }
            if len < MIN_SHIFTED {
                len = 0;
            }
        }
        if len == 0 {
            lit.push(target[i]);
            i += 1;
            j += 1;
            continue;
        }
        // A copy comes after a skip and before literals: start a new op if needed.
        if !lit.is_empty() || (d != 0 && copy > 0) {
            flush(&mut out, skip, copy, &lit);
            (skip, copy) = (0, 0);
            lit.clear();
        }
        skip += d;
        j += d + len as isize;
        copy += len;
        i += len;
    }
    if copy > 0 || !lit.is_empty() {
        flush(&mut out, skip, copy, &lit);
    }
    out
}

/// `base` patched by `patch` (None if the patch doesn't fit the baseline).
pub fn apply(base: &[u8], patch: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut at, mut j) = (0usize, 0isize);
    while at < patch.len() {
        let z = get(patch, &mut at)?;
        let skip = (z >> 1) as isize ^ -((z & 1) as isize);
        let copy = get(patch, &mut at)? as usize;
        let n = get(patch, &mut at)? as usize;
        j = j.checked_add(skip)?;
        let from = usize::try_from(j).ok()?;
        out.extend_from_slice(base.get(from..from.checked_add(copy)?)?);
        out.extend_from_slice(patch.get(at..at.checked_add(n)?)?);
        at += n;
        j += (copy + n) as isize;
        if out.len() > MAX_OUT {
            return None;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_edits_inserts_and_removals() {
        let mut seed = 7u64;
        let mut rnd = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for _ in 0..2000 {
            let base: Vec<u8> = (0..rnd(300)).map(|_| rnd(6) as u8).collect();
            let mut t = base.clone();
            for _ in 0..rnd(8) {
                let at = rnd(t.len() as u64 + 1) as usize;
                match rnd(3) {
                    0 if at < t.len() => t[at] = rnd(256) as u8,
                    1 => t.insert(at, rnd(256) as u8),
                    _ if at < t.len() => _ = t.remove(at),
                    _ => {}
                }
            }
            assert_eq!(apply(&base, &diff(&base, &t)).as_deref(), Some(&t[..]));
        }
        // Small changes make small patches; unrelated bytes still round-trip.
        let base: Vec<u8> = (0..250u32).map(|i| (i * 7) as u8).collect();
        let mut t = base.clone();
        t[100] ^= 1;
        t.insert(200, 9);
        assert!(diff(&base, &t).len() < 16, "{}", diff(&base, &t).len());
        assert_eq!(apply(&[], &diff(&[], &t)).unwrap(), t);
        // Garbage never panics.
        assert!(apply(&base, &[0xff; 12]).is_none());
        assert!(apply(&base, &[2, 255, 255, 3, 0]).is_none());
    }
}
