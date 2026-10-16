//! Reviewing reports: `server --reports <reports.jsonl>` lists them (newest last, numbered);
//! `server --review <reports.jsonl> <n> [port] [--demos dir]` replays the recording it points at
//! from a little before the moment (the recording is on the game server named in the report:
//! copy it from that machine's demos folder first).
use serde_json::Value;
use std::path::Path;

/// Start this long before the reported moment (the lead-up matters).
const LEAD_SECONDS: u32 = 10;

fn load(path: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn who(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        v => format!("{} ({})", v["name"].as_str().unwrap_or("?"), v["steam"]),
    }
}

/// One line per report: number, age, why, who reported whom, where.
pub fn list(path: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    for (i, r) in load(path).iter().enumerate() {
        let hours = now.saturating_sub(r["time"].as_u64().unwrap_or(0)) / 3600;
        let detail = r["detail"].as_str().map(|d| format!(": {d}")).unwrap_or_default();
        let demo = r["demo"].as_str().filter(|d| !d.is_empty()).map(|d| format!("{d} at tick {}", r["tick"])).unwrap_or("no recording".into());
        println!(
            "#{}  {hours}h ago  {}{detail}  by {} against {}  {} round {}  on {}  {demo}",
            i + 1,
            r["reason"].as_str().unwrap_or("?"),
            who(&r["reporter"]),
            who(&r["reported"]),
            r["map"].as_str().unwrap_or("?"),
            r["round"].as_u64().map(|n| n.to_string()).unwrap_or("-".into()),
            r["server"].as_str().unwrap_or("?"),
        );
    }
}

/// Report number `n`'s recording (in `demos`) and where to start watching.
pub fn find(path: &str, n: usize, demos: &Path) -> (std::path::PathBuf, u32) {
    let all = load(path);
    let r = all.get(n.wrapping_sub(1)).unwrap_or_else(|| panic!("no report #{n} (there are {})", all.len()));
    let demo = r["demo"].as_str().filter(|d| !d.is_empty()).unwrap_or_else(|| panic!("report #{n} has no recording"));
    // Only a file name: never let a report point outside the demos folder.
    let name = Path::new(demo).file_name().unwrap_or_else(|| panic!("bad recording name {demo}"));
    let file = demos.join(name);
    if !file.exists() {
        panic!("{} isn't here: copy it from {}'s demos folder", file.display(), r["server"].as_str().unwrap_or("the game server"));
    }
    let tick = r["tick"].as_u64().unwrap_or(0) as u32;
    (file, tick.saturating_sub(LEAD_SECONDS * net::protocol::TICK_RATE))
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_the_recording_and_starts_before_the_moment() {
        let dir = std::env::temp_dir().join(format!("bootleg16-review-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.bdem"), b"").unwrap();
        let reports = dir.join("reports.jsonl");
        let line = r#"{"time":1,"reason":"Cheating","reporter":{"steam":1,"name":"a"},"reported":{"steam":2,"name":"b"},"map":"de_dust2","round":3,"demo":"../../m.bdem","tick":5000,"server":"1.2.3.4:27015"}"#;
        std::fs::write(&reports, format!("{line}\nnot json\n")).unwrap();
        let (file, from) = super::find(reports.to_str().unwrap(), 1, &dir);
        assert_eq!(file, dir.join("m.bdem"), "stays inside the demos folder");
        assert_eq!(from, 5000 - 10 * net::protocol::TICK_RATE);
        super::list(reports.to_str().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
