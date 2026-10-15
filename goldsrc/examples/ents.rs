//! Print entities of a map whose classname contains a filter string.
//! Usage: ents <map.bsp> <filter>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bsp = goldsrc::bsp::Bsp::load(&a[1]).unwrap();
    for e in bsp.entity_list() {
        if e.get("classname").is_some_and(|c| c.contains(&a[2])) {
            let mut kv: Vec<_> = e.iter().collect();
            kv.sort();
            println!("{}", kv.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("  "));
        }
    }
}
