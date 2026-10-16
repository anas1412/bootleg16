//! The lobby's state as a snapshot: everyone's session (with its encryption state), parties,
//! the queue and the seats of running matches. Written to disk every couple of seconds (a
//! restarted lobby carries on where it was: nobody even notices) and streamed to a standby
//! lobby (see `standby`). The snapshot holds session keys: keep it as private as they are.
use super::*;
use crate::crypto::{Channel, ChannelState};

#[derive(Serialize, Deserialize)]
struct PlayerSnap {
    session: u64,
    channel: ChannelState,
    id: u32,
    name: String,
    steam: u64,
    addr: SocketAddr,
    party: u32,
    found: Option<(SocketAddr, u64, [u8; 32])>,
    /// (inviter, seconds since)
    invites: Vec<(u32, f32)>,
    pings: Vec<(String, u16)>,
}

#[derive(Serialize, Deserialize)]
struct PartySnap {
    id: u32,
    code: String,
    leader: u32,
    mode: Mode,
    members: Vec<(u32, Group)>,
    /// Seconds in the queue.
    queued: Option<f32>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Snapshot {
    next_id: u32,
    players: Vec<PlayerSnap>,
    parties: Vec<PartySnap>,
    /// (ticket, Steam id, name, seconds since booked)
    seats: Vec<(u64, u64, String, f32)>,
}

/// `Instant` `secs` ago (saturating at now).
fn ago(secs: f32) -> Instant {
    Instant::now().checked_sub(Duration::from_secs_f32(secs.max(0.0))).unwrap_or_else(Instant::now)
}

impl Master {
    pub fn snapshot(&self) -> Vec<u8> {
        let snap = Snapshot {
            next_id: self.next_id,
            players: self
                .players
                .iter()
                .map(|(session, p)| PlayerSnap {
                    session: *session,
                    channel: p.channel.export(),
                    id: p.id,
                    name: p.name.clone(),
                    steam: p.steam,
                    addr: p.addr,
                    party: p.party,
                    found: p.found,
                    invites: p.invites.iter().map(|(from, at)| (*from, at.elapsed().as_secs_f32())).collect(),
                    pings: p.pings.iter().map(|(r, ms)| (r.clone(), *ms)).collect(),
                })
                .collect(),
            parties: self.parties.iter().map(|(id, p)| PartySnap { id: *id, code: p.code.clone(), leader: p.leader, mode: p.mode, members: p.members.clone(), queued: p.queued.map(|t| t.elapsed().as_secs_f32()) }).collect(),
            seats: self.seats.iter().map(|(t, (steam, name, at))| (*t, *steam, name.clone(), at.elapsed().as_secs_f32())).collect(),
        };
        encode(&snap)
    }

    /// Take over a snapshot (everything we had is replaced). False if it isn't one.
    pub fn restore(&mut self, bytes: &[u8]) -> bool {
        let Some(snap) = decode::<Snapshot>(bytes) else { return false };
        self.next_id = snap.next_id;
        self.players = snap
            .players
            .into_iter()
            .map(|p| {
                let player = Player {
                    channel: Channel::import(&p.channel),
                    id: p.id,
                    name: p.name,
                    steam: p.steam,
                    addr: p.addr,
                    party: p.party,
                    seen: Instant::now(),
                    found: p.found,
                    notice: String::new(),
                    invites: p.invites.into_iter().map(|(from, secs)| (from, ago(secs))).collect(),
                    pings: p.pings.into_iter().collect(),
                };
                (p.session, player)
            })
            .collect();
        self.parties = snap.parties.into_iter().map(|p| (p.id, Party { code: p.code, leader: p.leader, mode: p.mode, members: p.members, queued: p.queued.map(ago) })).collect();
        self.seats = snap.seats.into_iter().map(|(t, steam, name, secs)| (t, (steam, name, ago(secs)))).collect();
        true
    }

    /// Save to `path` now (atomically: a crash mid-write never leaves half a file).
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let tmp = path.with_extension("tmp");
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(&self.snapshot())?;
        f.sync_all()?;
        std::fs::rename(tmp, path)
    }

    /// Load what was saved at `path` (nothing there: a fresh start).
    pub fn load(&mut self, path: &std::path::Path) -> bool {
        std::fs::read(path).is_ok_and(|b| self.restore(&b))
    }
}
