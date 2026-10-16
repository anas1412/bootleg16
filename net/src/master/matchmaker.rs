//! Casual matchmaking: fair, fast, and kind to parties. No ranks are shown, but every player
//! has a hidden skill (`rating`), and each match is built to be as close to 50/50 as the queue
//! allows:
//!
//! 1. The party waiting longest goes first. Parties near its skill (a window that widens the
//!    longer it waits) and with a good ping to the region are the candidates.
//! 2. Of those, the 10 players and the 5 v 5 split (parties never split) with the predicted
//!    win chance closest to 50% win, a little extra for keeping the skill spread small and for
//!    big premades meeting big premades. What counts as "fair enough" also widens with time.
//! 3. Everyone gets the match to accept (`ACCEPT_SECONDS`). All accept: a server is booked.
//!    Anyone doesn't: they get a short queue cooldown (longer each time that day), and the
//!    others go back to the queue keeping their place.
//! 4. At the end, the game server reports who won; ratings update. Abandoning counts as a loss
//!    and brings a cooldown too.
use super::rating::{self, Rating};
use super::*;

/// Seconds to accept a found match.
pub const ACCEPT_SECONDS: f32 = 20.0;
/// The skill window around the oldest party (in rating mu), and how it widens per second.
const SKILL_WINDOW: f64 = 4.0;
const SKILL_WIDEN_PER_SECOND: f64 = 0.05;
/// How far from 50/50 a match may be at first, how that widens per second, and the most.
const FAIR: f64 = 0.04;
const FAIR_WIDEN_PER_SECOND: f64 = 0.0015;
const FAIR_MAX: f64 = 0.20;
/// Candidates looked at per match (the search is over their combinations).
const CANDIDATES: usize = 14;
/// Cooldowns: not accepting a match (doubling each time in a day, at most an hour), and
/// abandoning one (30 minutes, doubling).
const DECLINE_COOLDOWN: u64 = 60;
const ABANDON_COOLDOWN: u64 = 30 * 60;

pub struct ReadyCheck {
    players: Vec<(u32, Group)>,
    accepted: Vec<u32>,
    until: Instant,
    region: String,
}

/// A queued party, as the matchmaker sees it.
#[derive(Clone)]
struct Entry {
    members: Vec<u32>,
    ratings: Vec<Rating>,
    mu: f64,
    waited: f64,
}

impl Master {
    fn rating_of(&self, player: u32) -> Rating {
        let steam = self.players.values().find(|p| p.id == player).map(|p| p.steam).unwrap_or(0);
        if steam == 0 {
            return Rating::default();
        }
        self.ratings.get(&steam).copied().unwrap_or_default()
    }

    /// Who a cooldown (or a running match) sticks to: the Steam account, or (no Steam) this
    /// session.
    pub(super) fn penalty_key(&self, player: u32) -> u64 {
        match self.players.values().find(|p| p.id == player).map(|p| p.steam) {
            Some(steam) if steam != 0 => steam,
            _ => 1 << 63 | player as u64,
        }
    }

    /// Seconds before this player may queue again.
    pub(super) fn cooldown_of(&self, player: u32) -> Option<f32> {
        let (_, until) = self.penalties.get(&self.penalty_key(player))?;
        let now = crate::crypto::now();
        (*until > now).then(|| (*until - now) as f32)
    }

    fn penalize(&mut self, key: u64, base: u64) {
        let now = crate::crypto::now();
        let (count, until) = self.penalties.entry(key).or_insert((0, 0));
        if now > *until + 24 * 3600 {
            *count = 0; // a clean day forgives
        }
        let secs = (base << (*count).min(5)).min(if base == DECLINE_COOLDOWN { 3600 } else { 24 * 3600 });
        *count += 1;
        *until = now + secs;
    }

    pub(super) fn ready_of(&self, player: u32) -> Option<ReadyNet> {
        let c = self.checks.iter().find(|c| c.players.iter().any(|(p, _)| *p == player))?;
        Some(ReadyNet { accepted: c.accepted.len() as u8, total: c.players.len() as u8, left: c.until.saturating_duration_since(Instant::now()).as_secs_f32(), you: c.accepted.contains(&player) })
    }

    pub(super) fn accept(&mut self, player: u32) {
        if let Some(c) = self.checks.iter_mut().find(|c| c.players.iter().any(|(p, _)| *p == player)) {
            if !c.accepted.contains(&player) {
                c.accepted.push(player);
            }
        }
    }

    /// Said no (or cancelled during a check): the check fails for them right away.
    pub(super) fn decline(&mut self, player: u32) {
        if let Some(c) = self.checks.iter_mut().find(|c| c.players.iter().any(|(p, _)| *p == player) && !c.accepted.contains(&player)) {
            c.until = Instant::now();
        }
    }

    /// Ready checks: everyone accepted, book the server; time's up, sort out who didn't.
    fn checks_step(&mut self) {
        let mut i = 0;
        while i < self.checks.len() {
            let c = &self.checks[i];
            if c.accepted.len() == c.players.len() {
                let c = self.checks.remove(i);
                if !self.start_match(c.players.clone(), false, Some(&c.region)) {
                    // The server went away meanwhile: everyone back in line, first.
                    self.requeue(&c.players.iter().map(|(p, _)| *p).collect::<Vec<_>>());
                }
                continue;
            }
            if Instant::now() >= c.until {
                let c = self.checks.remove(i);
                let (yes, no): (Vec<u32>, Vec<u32>) = c.players.iter().map(|(p, _)| *p).partition(|p| c.accepted.contains(p));
                for p in &no {
                    let key = self.penalty_key(*p);
                    self.penalize(key, DECLINE_COOLDOWN);
                    if let Some(party) = self.parties.values_mut().find(|q| q.members.iter().any(|(m, _)| m == p)) {
                        party.queued = None;
                    }
                    if let Some(pl) = self.players.values_mut().find(|q| q.id == *p) {
                        pl.notice = "You didn't accept the match: you can queue again in a little while.".into();
                    }
                }
                // Parties whose members all accepted go back to the queue with their wait.
                self.requeue(&yes);
                for p in &yes {
                    if let Some(pl) = self.players.values_mut().find(|q| q.id == *p) {
                        pl.notice = "Someone didn't accept: back in the queue, keeping your place.".into();
                    }
                }
                continue;
            }
            i += 1;
        }
    }

    /// Parties of these players go back in the queue (they kept `queued` all along, so their
    /// place too), except parties with someone who didn't accept.
    fn requeue(&mut self, players: &[u32]) {
        for party in self.parties.values_mut() {
            if party.members.iter().any(|(m, _)| players.contains(m)) && party.queued.is_none() {
                party.queued = Some(Instant::now());
            }
        }
    }

    /// Find matches (as many as the queue makes, a few a step) and run the ready checks.
    pub(super) fn matchmake(&mut self) {
        self.checks_step();
        let side = self.match_size / 2;
        let checking: Vec<u32> = self.checks.iter().flat_map(|c| c.players.iter().map(|(p, _)| *p)).collect();
        let mut queue: Vec<Entry> = self
            .parties
            .values()
            .filter(|p| p.queued.is_some() && p.mode == Mode::Casual && p.members.len() <= side)
            .filter(|p| !p.members.iter().any(|(m, _)| checking.contains(m)))
            .map(|p| {
                let members: Vec<u32> = p.members.iter().map(|(m, _)| *m).collect();
                let ratings: Vec<Rating> = members.iter().map(|m| self.rating_of(*m)).collect();
                let mu = ratings.iter().map(|r| r.mu).sum::<f64>() / ratings.len() as f64;
                Entry { members, ratings, mu, waited: p.queued.unwrap().elapsed().as_secs_f64() }
            })
            .collect();
        queue.sort_by(|a, b| b.waited.total_cmp(&a.waited));
        let mut regions: Vec<String> = self.servers.values().filter(|s| s.official && !s.busy).map(|s| s.region.clone()).collect();
        regions.sort();
        regions.dedup();
        let mut free: HashMap<String, usize> = HashMap::new();
        for s in self.servers.values().filter(|s| s.official && !s.busy) {
            *free.entry(s.region.clone()).or_default() += 1;
        }
        for _ in 0..8 {
            let Some((players, region)) = self.find_match(&queue, &regions, &free, side) else { break };
            *free.get_mut(&region).unwrap() -= 1;
            queue.retain(|e| !e.members.iter().any(|m| players.iter().any(|(p, _)| p == m)));
            for (p, _) in &players {
                if let Some(pl) = self.players.values_mut().find(|q| q.id == *p) {
                    pl.notice = "Match found: accept it!".into();
                }
            }
            println!("match found in {region}: {} players, waiting for them to accept", players.len());
            self.checks.push(ReadyCheck { players, accepted: Vec::new(), until: Instant::now() + Duration::from_secs_f32(ACCEPT_SECONDS), region });
        }
    }

    /// The best match around the oldest party that can have one, if fair enough.
    fn find_match(&self, queue: &[Entry], regions: &[String], free: &HashMap<String, usize>, side: usize) -> Option<(Vec<(u32, Group)>, String)> {
        for seed in queue {
            for region in regions.iter().filter(|r| free.get(*r).copied().unwrap_or(0) > 0) {
                let ok = |e: &Entry| self.worst_ping(&e.members, region).is_some_and(|p| p as f64 <= Self::ping_limit(Duration::from_secs_f64(e.waited)) as f64);
                if !ok(seed) {
                    continue;
                }
                let window = SKILL_WINDOW + seed.waited * SKILL_WIDEN_PER_SECOND;
                let mut near: Vec<&Entry> = queue.iter().filter(|e| !std::ptr::eq(*e, seed) && (e.mu - seed.mu).abs() <= window && ok(e)).collect();
                near.sort_by(|a, b| (a.mu - seed.mu).abs().total_cmp(&(b.mu - seed.mu).abs()));
                near.truncate(CANDIDATES);
                let fair = (FAIR + seed.waited * FAIR_WIDEN_PER_SECOND).min(FAIR_MAX);
                if let Some((_, off, a, b)) = best_split(seed, &near, side) {
                    if off <= fair {
                        let players = a.iter().flat_map(|e| e.members.iter().map(|m| (*m, Group::A))).chain(b.iter().flat_map(|e| e.members.iter().map(|m| (*m, Group::B)))).collect();
                        return Some((players, region.clone()));
                    }
                }
            }
        }
        None
    }

    /// The game server's result: ratings move, abandoners get a cooldown.
    pub(super) fn match_result(&mut self, winners: &[u64], losers: &[u64], abandoned: &[u64]) {
        // The match is over: nobody can rejoin it.
        let seats: Vec<u64> = winners.iter().chain(losers).chain(abandoned).copied().collect();
        self.active.retain(|_, (_, t, _)| !seats.contains(t));
        let steam_of = |m: &Master, t: &u64| m.seats.get(t).map(|s| s.0).filter(|s| *s != 0);
        for t in abandoned {
            let key = steam_of(self, t).unwrap_or(1 << 62 | *t);
            self.penalize(key, ABANDON_COOLDOWN);
        }
        if winners.is_empty() || losers.is_empty() {
            return; // a draw (or a one-sided report): nobody's rating moves
        }
        let ids = |m: &Master, ts: &[u64]| ts.iter().filter_map(|t| steam_of(m, t)).collect::<Vec<u64>>();
        let (w, l) = (ids(self, winners), ids(self, losers));
        let mut wr: Vec<Rating> = w.iter().map(|s| self.ratings.get(s).copied().unwrap_or_default()).collect();
        let mut lr: Vec<Rating> = l.iter().map(|s| self.ratings.get(s).copied().unwrap_or_default()).collect();
        if wr.is_empty() || lr.is_empty() {
            return;
        }
        rating::update(&mut wr, &mut lr);
        for (s, r) in w.iter().zip(wr).chain(l.iter().zip(lr)) {
            self.ratings.insert(*s, r);
        }
        if let Some(path) = &self.ratings_file {
            if let Err(e) = save_ratings(&self.ratings, path) {
                println!("can't save ratings to {}: {e}", path.display());
            }
        }
    }
}

/// The best 5 v 5 out of the seed and its candidates (parties stay whole): (its score, how
/// far its win chance is from 50/50, team A, team B). The score is that distance plus small
/// extras for a narrow skill spread and premades meeting premades.
fn best_split<'a>(seed: &'a Entry, near: &[&'a Entry], side: usize) -> Option<(f64, f64, Vec<&'a Entry>, Vec<&'a Entry>)> {
    let mut best: Option<(f64, f64, Vec<&Entry>, Vec<&Entry>)> = None;
    // Every set of candidates that makes 2 x side players with the seed...
    let n = near.len();
    for mask in 0u32..(1 << n) {
        let set: Vec<&Entry> = std::iter::once(seed).chain((0..n).filter(|i| mask & (1 << i) != 0).map(|i| near[i])).collect();
        if set.iter().map(|e| e.members.len()).sum::<usize>() != 2 * side {
            continue;
        }
        // ...and every way to split it into two teams of `side` (the seed on team A).
        let k = set.len();
        for split in 0u32..(1 << (k - 1)) {
            let in_a = |i: usize| i == 0 || split & (1 << (i - 1)) != 0;
            let size_a: usize = (0..k).filter(|i| in_a(*i)).map(|i| set[i].members.len()).sum();
            if size_a != side {
                continue;
            }
            let a: Vec<&Entry> = (0..k).filter(|i| in_a(*i)).map(|i| set[i]).collect();
            let b: Vec<&Entry> = (0..k).filter(|i| !in_a(*i)).map(|i| set[i]).collect();
            let ratings = |t: &[&Entry]| t.iter().flat_map(|e| e.ratings.iter().copied()).collect::<Vec<Rating>>();
            let fair = (rating::win_chance(&ratings(&a), &ratings(&b)) - 0.5).abs();
            // Small extras: a narrow skill spread, and premades meeting premades.
            let mus: Vec<f64> = set.iter().flat_map(|e| e.ratings.iter().map(|r| r.mu)).collect();
            let spread = mus.iter().cloned().fold(f64::MIN, f64::max) - mus.iter().cloned().fold(f64::MAX, f64::min);
            let biggest = |t: &[&Entry]| t.iter().map(|e| e.members.len()).max().unwrap_or(0) as f64;
            let premade = (biggest(&a) - biggest(&b)).abs();
            let score = fair + spread * 0.002 + premade * 0.01;
            if best.as_ref().is_none_or(|(s, _, _, _)| score < *s) {
                best = Some((score, fair, a, b));
            }
        }
    }
    best
}

pub fn save_ratings(ratings: &HashMap<u64, Rating>, path: &std::path::Path) -> std::io::Result<()> {
    let list: Vec<(u64, Rating)> = ratings.iter().map(|(k, v)| (*k, *v)).collect();
    crate::secure::write_private(path, &encode(&list))
}

impl Master {
    /// Load the ratings kept at `path` (nothing there: everyone starts fresh).
    pub fn load_ratings(&mut self, path: &std::path::Path) {
        if let Some(list) = std::fs::read(path).ok().and_then(|b| decode::<Vec<(u64, Rating)>>(&b)) {
            self.ratings = list.into_iter().collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(mus: &[f64], waited: f64) -> Entry {
        let ratings: Vec<Rating> = mus.iter().map(|m| Rating { mu: *m, sigma: 3.0, matches: 20 }).collect();
        Entry { members: (0..mus.len() as u32).collect(), mu: mus.iter().sum::<f64>() / mus.len() as f64, ratings, waited }
    }

    #[test]
    fn results_move_ratings_and_abandoning_brings_a_cooldown() {
        let mut m = Master::bind("127.0.0.1:0".parse().unwrap(), vec![]).unwrap();
        for (ticket, steam) in [(1, 11), (2, 22), (3, 33)] {
            m.seats.insert(ticket, (steam, String::new(), Instant::now()));
        }
        m.active.insert(11, ("127.0.0.1:1".parse().unwrap(), 1, [0; 32]));
        m.match_result(&[1], &[2, 3], &[3]);
        let r = |s: u64| m.ratings[&s];
        assert!(r(11).mu > rating::MU && r(22).mu < rating::MU, "winner up, loser down");
        assert!(m.penalties.get(&33).is_some_and(|(_, until)| *until > crate::crypto::now() + 60), "abandoned: a cooldown");
        assert!(!m.penalties.contains_key(&22));
        assert!(m.active.is_empty(), "the match is over: no more rejoining");
    }

    #[test]
    fn splits_are_fair_and_keep_parties_whole() {
        // A strong pair, a weak pair and six middling solos: the 5 v 5 comes out even.
        let seed = entry(&[35.0, 35.0], 30.0);
        let others: Vec<Entry> = [vec![15.0, 15.0], vec![25.0], vec![25.0], vec![25.0], vec![25.0], vec![25.0], vec![25.0]].iter().map(|m| entry(m, 10.0)).collect();
        let near: Vec<&Entry> = others.iter().collect();
        let (_, off, a, b) = best_split(&seed, &near, 5).unwrap();
        assert!(off < 0.02, "{off}: should be close to 50/50");
        assert_eq!(a.iter().chain(&b).map(|e| e.members.len()).sum::<usize>(), 10);
        // (Several splits are exactly even here: what matters is the total skill per team.)
        let total = |t: &[&Entry]| t.iter().flat_map(|e| e.ratings.iter().map(|r| r.mu)).sum::<f64>();
        assert!((total(&a) - total(&b)).abs() < 1e-9, "{} v {}", total(&a), total(&b));
        // A 5-stack prefers another 5-stack over five strangers (equal skill).
        let stack = entry(&[25.0; 5], 30.0);
        let other_stack = entry(&[25.0; 5], 5.0);
        let solos: Vec<Entry> = (0..5).map(|_| entry(&[25.0], 5.0)).collect();
        let mut near: Vec<&Entry> = solos.iter().collect();
        near.push(&other_stack);
        let (_, _, _, b) = best_split(&stack, &near, 5).unwrap();
        assert_eq!(b.len(), 1, "five strangers were picked over the other 5-stack");
    }
}
