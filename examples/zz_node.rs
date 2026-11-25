use bevy_flora::{generate_skeleton, TreePreset};
fn main() {
    let p = TreePreset::Willow.params();
    let s = generate_skeleton(&p);
    for n in s.nodes.iter().filter(|n| n.kind == bevy_flora::NodeKind::Primary) {
        let ax = n.axis(n.axis_points());
        println!("id {} start {:?} dir {:?} len {:.2} base_r {:.3} bend {:.2} t {:.2} tipdir {:?} end {:?}", n.id, n.start, n.direction, n.length, n.base_radius, n.bend, n.attach_t, ax.last().unwrap().1, s.end(n.id as usize));
    }
}
