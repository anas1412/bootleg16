//! Trace lines in 8 directions from a point. Usage: tline <map.bsp> x y z
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bsp = goldsrc::bsp::Bsp::load(&a[1]).unwrap();
    let w = sim::collision::World::from_bsp(&bsp);
    let p: Vec<f32> = a[2..5].iter().map(|x| x.parse().unwrap()).collect();
    let p = [p[0], p[1], p[2]];
    println!("contents at point: {:?}, standing hull solid: {}", w.point_contents(p), w.is_solid(sim::collision::Hull::Standing, p));
    for yaw in (0..360).step_by(45) {
        let (s, c) = (yaw as f32).to_radians().sin_cos();
        let t = w.trace_line(p, [p[0] + c * 100.0, p[1] + s * 100.0, p[2]]);
        let h = w.trace(sim::collision::Hull::Standing, [p[0], p[1], p[2] - 20.0], [p[0] + c * 100.0, p[1] + s * 100.0, p[2] - 20.0]);
        println!("yaw {yaw:3}: line fraction {:.2} start_solid {}  | hull fraction {:.2}", t.fraction, t.start_solid, h.fraction);
    }
}
