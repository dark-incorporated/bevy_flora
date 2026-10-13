// Where a falling leaf is (`falling.rs`, `FloraExtension::falling`): the vertex stages of the main pass
// (`falling.wesl`) and of the prepass and shadow maps (`falling_prepass.wesl`) share it, so the
// shadows fall with the leaves.
//
// Each card of a burst carries, besides the tree leaf's own attributes:
//   anchor: where the card's centre hangs in the burst's space (xyz) and the render time it lets go (w)
//   path:   the ground's height under where it lands (x, burst space), a seed in 0..1 (y), how long
//           it lies on the ground (z, seconds) and how fast it falls once it has picked up speed (w)
//   face:   the card's face normal in the burst's space (xyz), one per card, so the tumble and the
//           settling turn the whole card alike
// `falling.rs` mirrors the timing (`landing_time`, the drift) to size the burst's bounds and its life.
import package::shading::common::{flora_wind, world_per_metre};

// Seconds a leaf takes to pick up most of its speed: it falls from rest, not from its full speed.
const PICK_UP: f32 = 0.35;
// How fast the wind carries a leaf across, metres per second at strength 1 (times the world's
// units to a metre, `world_per_metre`: so are `LIFT` and the swing below).
const DRIFT: f32 = 1.5;
// Seconds a leaf takes to shrink away once its time on the ground is up.
const SHRINK: f32 = 1.0;
// How far above the ground a fallen leaf lies, so it does not flicker into it.
const LIFT: f32 = 0.03;
// The share of its height a fallen leaf keeps (its curl): folded flat, the planes of a crossed
// card would lie in one plane and flicker through each other.
const CURL: f32 = 0.12;
// Seconds over which a leaf turns to lie flat before it touches the ground.
const SETTLE: f32 = 0.3;
// The render clock (`globals.time`) wraps every hour.
const WRAP: f32 = 3600.0;

struct FallingLeaf {
    // The vertex in world space.
    position: vec3<f32>,
    // Its normal in world space.
    normal: vec3<f32>,
    // How far the leaf is out of its tree's shade: 0 while it hangs, 1 a second after it let go.
    open: f32,
    // False once it has shrunk away: the vertex then draws nothing.
    shown: bool,
}

// A number in 0..1 for the leaf with `seed`, a different one for each `k`.
fn leaf_hash(seed: f32, k: f32) -> f32 {
    return fract(sin(seed * 127.1 + k * 311.7) * 43758.5453);
}

// `v` turned by `angle` radians about the unit `axis` (Rodrigues).
fn turn(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return v * c + cross(axis, v) * s + axis * (dot(axis, v) * (1.0 - c));
}

// How far a leaf has fallen `t` seconds after letting go, at full speed `speed`.
fn fallen(t: f32, speed: f32) -> f32 {
    return speed * (t - PICK_UP * (1.0 - exp(-t / PICK_UP)));
}

// Seconds a leaf that lets go `height` metres above the ground takes to reach it (`fallen` = `height`),
// by Newton's method from the time it would take at full speed.
fn landing_time(height: f32, speed: f32) -> f32 {
    if height <= 0.0 {
        return 0.0;
    }
    var t = height / speed + PICK_UP;
    for (var i = 0; i < 4; i = i + 1) {
        let e = exp(-t / PICK_UP);
        let f = speed * (t - PICK_UP * (1.0 - e)) - height;
        t = t - f / max(speed * (1.0 - e), 1.0e-3);
    }
    return max(t, 0.0);
}

// The vertex at `position` (normal `normal`) of a falling leaf's card, `time` being the render time.
fn falling_leaf(
    world_from_local: mat4x4<f32>,
    position: vec3<f32>,
    normal: vec3<f32>,
    anchor: vec4<f32>,
    path: vec4<f32>,
    face: vec4<f32>,
    time: f32,
) -> FallingLeaf {
    var out: FallingLeaf;
    out.shown = true;
    var t = time - anchor.w;
    if t < -0.5 * WRAP {
        t = t + WRAP;
    }
    // Not let go yet: it hangs where it grew.
    if t <= 0.0 {
        out.position = (world_from_local * vec4<f32>(position, 1.0)).xyz;
        out.normal = normalize((world_from_local * vec4<f32>(normal, 0.0)).xyz);
        out.open = 0.0;
        return out;
    }
    let seed = path.y;
    let speed = path.w;
    let rest = path.z;
    let height = anchor.y - path.x;
    let land = landing_time(height, speed);
    if t > land + rest + SHRINK {
        out.shown = false;
        return out;
    }
    let flight = min(t, land);

    // Down, picking up speed; across, carried by the wind and swinging from side to side as a
    // falling leaf does, the swing growing as it falls. (How fast it falls is the burst's own,
    // in world units: `path.w`.)
    let wind = flora_wind[0];
    let metre = world_per_metre();
    let swing_heading = 6.2831853 * leaf_hash(seed, 1.0);
    let swing = mix(0.12, 0.4, leaf_hash(seed, 2.0))
        * sin(mix(1.8, 3.2, leaf_hash(seed, 3.0)) * flight + 6.2831853 * leaf_hash(seed, 4.0))
        * (1.0 - exp(-flight / 0.6)) * metre;
    let across = wind.xy * (wind.z * DRIFT * metre * flight) + vec2<f32>(cos(swing_heading), sin(swing_heading)) * swing;
    var centre = anchor.xyz + vec3<f32>(across.x, -min(fallen(flight, speed), height), across.y);

    // It tumbles about an axis of its own, and turns to lie flat, face up or face down as it came,
    // over the last moments before it touches the ground.
    let axis = normalize(vec3<f32>(leaf_hash(seed, 5.0), leaf_hash(seed, 6.0), leaf_hash(seed, 7.0)) - 0.5 + vec3<f32>(1.0e-3));
    let spin = mix(2.0, 6.0, leaf_hash(seed, 8.0)) * select(-1.0, 1.0, leaf_hash(seed, 9.0) > 0.5) * flight;
    var offset = turn(position - anchor.xyz, axis, spin);
    var n = turn(normal, axis, spin);
    let f = turn(face.xyz, axis, spin);
    let level = select(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), f.y >= 0.0);
    let settle = smoothstep(land - SETTLE, land, t);
    let hinge = cross(f, level);
    let sine = length(hinge);
    if sine > 1.0e-4 {
        let angle = atan2(sine, dot(f, level)) * settle;
        offset = turn(offset, hinge / sine, angle);
        n = turn(n, hinge / sine, angle);
    }
    // What stands across the face (the second card of crossed planes, a star's other arms) folds
    // down into it, all but a little curl, so the leaf lies flat whatever its shape; each side's
    // normal turns to face up or down with it.
    offset = offset - level * (dot(offset, level) * settle * (1.0 - CURL));
    n = normalize(mix(n, level * select(-1.0, 1.0, dot(n, level) >= 0.0), settle));
    if t >= land {
        centre.y = path.x + LIFT * metre;
    }
    // Its time on the ground up, it shrinks away.
    let size = 1.0 - smoothstep(land + rest, land + rest + SHRINK, t);
    out.position = (world_from_local * vec4<f32>(centre + offset * size, 1.0)).xyz;
    out.normal = normalize((world_from_local * vec4<f32>(n, 0.0)).xyz);
    out.open = smoothstep(0.0, 1.0, t);
    return out;
}
