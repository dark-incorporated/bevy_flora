//! Impostors: a far tree drawn as one camera-facing quad.
//!
//! A tree 200 m away is a few dozen pixels tall, and its leaves are sub-pixel;
//! drawing its meshes there is a waste. An impostor is a picture of the tree,
//! baked once per variant from eight yaws around it, put on a quad that turns
//! about the trunk to face the camera and shows the picture taken from the
//! nearest yaw ([`ImpostorMaterial`]).
//!
//! There are two pictures, not a lit one: the tree's colour, and its surface (the
//! normal in the tree's own frame, how much of the sky each leaf sees, how much
//! light it lets through). The scene's own lights, shadows, ambient light and fog
//! shade them each frame, so impostors follow the time of day and glow when the
//! sun is behind them, and the quad sways with [`FloraWind`](crate::FloraWind).
//!
//! The pictures are baked in the running app: [`TreePool`](crate::TreePool)
//! renders each variant's level 1 meshes into a 2048x512 image (eight 256x256
//! tiles for the colour, eight for the surface, the camera 10° above the
//! horizon), reads it back, adds mipmaps and stores it as an [`ImpostorAtlas`].
//! This needs a renderer; without one (a headless test,
//! [`LodDistances::impostor`](crate::LodDistances::impostor) at infinity) a pool
//! simply has none and its coarsest level is drawn at every distance.
//!
//! # Limits
//!
//! - **Eight yaws at one elevation.** Turning around a far tree switches
//!   between eight pictures, and the quad stays upright however high the camera
//!   is. Impostors receive scene shadows only where shadow cascades reach, and
//!   there is no self-shadowing beyond the baked crown occlusion.
//! - **No motion vectors.** An impostor is drawn outside depth, motion-vector
//!   and shadow prepasses, and casts no shadow.

mod bake;
mod material;

use bevy::app::App;
use bevy::asset::{AssetApp, AssetServer, Assets, Handle, RenderAssetUsages};
use bevy::ecs::prelude::*;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Vec2, Vec3};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::pbr::{MaterialPlugin, PbrPlugin};
use bevy::render::RenderApp;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use material::ImpostorBakeMaterial;
pub use material::{ImpostorMaterial, ImpostorTint};

pub(crate) use bake::{ImpostorBakes, ImpostorsBaked, drive_impostor_bakes, start_impostor_bakes};

/// Views of a tree baked into an atlas: yaws `k / VIEWS` of a turn apart.
pub const IMPOSTOR_VIEWS: u32 = 8;
/// Width and height of one baked view, in pixels.
pub(crate) const TILE_PIXELS: u32 = 256;
/// Levels of the atlas's mip chain; the smallest has tiles of 8 pixels.
pub(crate) const MIP_LEVELS: u32 = 6;
/// How far above the horizon the baking camera looks, in degrees.
const ELEVATION_DEGREES: f32 = 10.0;
/// Margin around the tree in a view, as a share of its size on each side.
const FRAME_MARGIN: f32 = 0.04;

/// The pictures of one tree variant seen from [`IMPOSTOR_VIEWS`] yaws, and
/// what is needed to draw them.
#[derive(Clone, Debug)]
pub struct ImpostorAtlas {
    /// The colour atlas: [`IMPOSTOR_VIEWS`] square views side by side, view `k`
    /// taken from the direction that is the tree's local `+Z` turned by `k / views`
    /// of a turn about `Y`. Alpha is coverage.
    pub albedo: Handle<Image>,
    /// The surface atlas, laid out like `albedo`: the normal in the tree's own
    /// frame, octahedral-encoded, in `rg`; the share of the sky a leaf sees (the
    /// crown's occlusion) in `b`; how much light the surface lets through in `a`.
    /// Linear.
    pub surface: Handle<Image>,
    /// Number of views in the atlas.
    pub views: u32,
    /// Size of the quad, in metres: width, height.
    pub size: Vec2,
    /// Height of the quad's centre above the tree's base, in metres.
    pub center_y: f32,
    /// The materials that draw the atlas, one per look of the pool ([`TreePool::looks`](crate::TreePool::looks);
    /// the same pictures in each, tinted to the look's colours: see [`ImpostorTint`]).
    pub materials: Vec<Handle<ImpostorMaterial>>,
    /// The quad, in the tree's local space.
    pub quad: Handle<Mesh>,
}

/// Set when [`ImpostorMaterial`] was registered with the renderer, which only
/// happens if the plugin was added after the renderer's plugins.
#[derive(Resource)]
pub(crate) struct ImpostorSupport;

/// Where the baking camera looks and how far it sees to frame a tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImpostorFrame {
    /// Size of the quad in metres.
    pub(crate) size: Vec2,
    /// Height of the quad's centre above the tree's base.
    pub(crate) center_y: f32,
    /// Extent the orthographic camera sees, in metres, across and up the picture.
    pub(crate) view: Vec2,
}

impl ImpostorFrame {
    /// The frame of a tree `height` tall whose crown reaches `radius` from the
    /// trunk axis, seen from [`ELEVATION_DEGREES`] above the horizon: a box
    /// around the tree's bounding cylinder as it looks from that height.
    pub(crate) fn new(height: f32, radius: f32) -> Self {
        let (sin, cos) = ELEVATION_DEGREES.to_radians().sin_cos();
        // Up the picture: the base is at 0; a point at height `y` and depth `s`
        // toward the camera is at `y cos - s sin`.
        let top = height * cos + radius * sin;
        let bottom = -radius * sin;
        let margin = 1.0 + 2.0 * FRAME_MARGIN;
        let view = Vec2::new(2.0 * radius * margin, (top - bottom) * margin);
        // The quad stands upright: a metre up the quad is `cos` metres up the picture.
        Self {
            size: Vec2::new(view.x, view.y / cos),
            center_y: (top + bottom) / 2.0 / cos,
            view,
        }
    }

    /// Where the baking camera sits relative to what it looks at: unit length,
    /// toward the viewer.
    pub(crate) fn toward_camera() -> Vec3 {
        let (sin, cos) = ELEVATION_DEGREES.to_radians().sin_cos();
        Vec3::new(0.0, sin, cos)
    }

    /// The quad's mesh: upright in the XY plane, facing `+Z`, `x` across, the
    /// atlas's view uv.
    pub(crate) fn quad(&self) -> Mesh {
        let half = self.size / 2.0;
        let (bottom, top) = (self.center_y - half.y, self.center_y + half.y);
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-half.x, bottom, 0.0],
                [half.x, bottom, 0.0],
                [half.x, top, 0.0],
                [-half.x, top, 0.0],
            ],
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        );
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        mesh
    }
}

/// Register [`ImpostorMaterial`] where the renderer is, and the settings.
///
/// Plugins may be added in any order; see [`crate::gpu::build`]. Added ahead of
/// the renderer's, this registers nothing that needs it and the pools get no
/// impostors.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<ImpostorBakes>()
        .init_resource::<ImpostorsBaked>();
    if app.get_sub_app(RenderApp).is_some() && app.is_plugin_added::<PbrPlugin>() {
        material::add_shaders(app);
        app.add_plugins((
            MaterialPlugin::<ImpostorMaterial>::default(),
            MaterialPlugin::<ImpostorBakeMaterial>::default(),
        ))
        .insert_resource(ImpostorSupport);
    } else {
        add_asset_types(app);
    }
}

/// Once every plugin is built, make sure the asset types the systems take exist
/// even if the plugin was added too early to register them.
pub(crate) fn finish(app: &mut App) {
    add_asset_types(app);
}

fn add_asset_types(app: &mut App) {
    if !app.world().contains_resource::<AssetServer>() {
        return;
    }
    if !app.world().contains_resource::<Assets<ImpostorMaterial>>() {
        app.init_asset::<ImpostorMaterial>();
    }
    if !app.world().contains_resource::<Assets<ImpostorBakeMaterial>>() {
        app.init_asset::<ImpostorBakeMaterial>();
    }
}

/// `base` (RGBA8, sRGB-encoded, `width` x `height`) followed by its smaller
/// levels, [`MIP_LEVELS`] in all, each half the size of the one before. Levels are
/// averaged in linear light, weighting each pixel's colour by its alpha so the
/// transparent black around a tree does not darken its edge.
pub(crate) fn with_mip_chain(base: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut chain = base.to_vec();
    let (mut w, mut h) = (width as usize, height as usize);
    let mut from = 0;
    for _ in 1..MIP_LEVELS {
        let (nw, nh) = (w / 2, h / 2);
        let mut level = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                let (mut rgb, mut alpha) = ([0.0f32; 3], 0.0f32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let at = from + ((2 * y + dy) * w + 2 * x + dx) * 4;
                    let a = f32::from(chain[at + 3]) / 255.0;
                    for (c, sum) in rgb.iter_mut().enumerate() {
                        *sum += srgb_to_linear(chain[at + c]) * a;
                    }
                    alpha += a;
                }
                for sum in rgb {
                    level.push(if alpha > 0.0 {
                        linear_to_srgb(sum / alpha)
                    } else {
                        0
                    });
                }
                level.push((alpha / 4.0 * 255.0).round() as u8);
            }
        }
        from = chain.len();
        chain.extend_from_slice(&level);
        (w, h) = (nw, nh);
    }
    chain
}

pub(crate) fn srgb_to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// The sampler of both atlases: filtered, clamped at the edges.
fn atlas_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    })
}

/// The colour atlas as an image Bevy can sample: `base` (as [`with_mip_chain`] takes
/// it) with its mipmaps, filtered, clamped at the edges.
pub(crate) fn atlas_image(base: &[u8], width: u32, height: u32) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        base.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(with_mip_chain(base, width, height));
    image.texture_descriptor.mip_level_count = MIP_LEVELS;
    image.sampler = atlas_sampler();
    image
}

/// Sign of each component, 1 for zero.
fn sign_not_zero(v: Vec2) -> Vec2 {
    Vec2::new(
        if v.x >= 0.0 { 1.0 } else { -1.0 },
        if v.y >= 0.0 { 1.0 } else { -1.0 },
    )
}

/// A unit vector as two numbers in -1..1: the octahedral mapping. `impostor.wesl` and
/// `shading/common.wesl` have the same functions.
pub(crate) fn oct_encode(n: Vec3) -> Vec2 {
    let p = Vec2::new(n.x, n.y) / (n.x.abs() + n.y.abs() + n.z.abs());
    if n.z < 0.0 {
        (Vec2::ONE - Vec2::new(p.y.abs(), p.x.abs())) * sign_not_zero(p)
    } else {
        p
    }
}

/// The unit vector [`oct_encode`] made `e` of.
pub(crate) fn oct_decode(e: Vec2) -> Vec3 {
    let mut v = Vec3::new(e.x, e.y, 1.0 - e.x.abs() - e.y.abs());
    if v.z < 0.0 {
        let folded = (Vec2::ONE - Vec2::new(v.y.abs(), v.x.abs())) * sign_not_zero(Vec2::new(v.x, v.y));
        v.x = folded.x;
        v.y = folded.y;
    }
    v.normalize()
}

/// The surface row of a capture (`rgba`, captured through an sRGB view, so every colour
/// channel is sRGB-encoded) as the linear data it was written as. Alpha was never encoded.
pub(crate) fn linearize_surface(bytes: &[u8]) -> Vec<u8> {
    let decode: Vec<u8> = (0..=255u8)
        .map(|b| (srgb_to_linear(b) * 255.0).round() as u8)
        .collect();
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|px| [decode[px[0] as usize], decode[px[1] as usize], decode[px[2] as usize], px[3]])
        .collect()
}

/// Unit-range `rg` of a surface texel's bytes.
fn texel_normal(texel: &[u8]) -> Vec3 {
    oct_decode(Vec2::new(f32::from(texel[0]), f32::from(texel[1])) / 255.0 * 2.0 - Vec2::ONE)
}

/// `surface` (RGBA8, linear, `width` x `height`) followed by its smaller levels, [`MIP_LEVELS`] in
/// all. Each texel is the average of the four below it weighted by the alpha of the colour
/// atlas's texel at the same place (`albedo_chain`, [`with_mip_chain`]'s output), so what is not
/// the tree does not count: the normals are averaged as vectors and renormalised, visibility
/// and translucency as numbers, and a texel with nothing under it is all zero.
pub(crate) fn surface_mip_chain(surface: &[u8], albedo_chain: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut chain = surface.to_vec();
    let (mut w, mut h) = (width as usize, height as usize);
    let (mut from, mut albedo_from) = (0, 0);
    for _ in 1..MIP_LEVELS {
        let (nw, nh) = (w / 2, h / 2);
        let mut level = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                let (mut normal, mut visibility, mut translucency, mut weight) = (Vec3::ZERO, 0.0f32, 0.0f32, 0.0f32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let at = ((2 * y + dy) * w + 2 * x + dx) * 4;
                    let a = f32::from(albedo_chain[albedo_from + at + 3]) / 255.0;
                    let texel = &chain[from + at..][..4];
                    normal += texel_normal(texel) * a;
                    visibility += f32::from(texel[2]) * a;
                    translucency += f32::from(texel[3]) * a;
                    weight += a;
                }
                if weight > 0.0 {
                    let e = oct_encode(normal.normalize_or(Vec3::Z)) * 0.5 + Vec2::splat(0.5);
                    level.extend_from_slice(&[
                        (e.x * 255.0).round() as u8,
                        (e.y * 255.0).round() as u8,
                        (visibility / weight).round() as u8,
                        (translucency / weight).round() as u8,
                    ]);
                } else {
                    level.extend_from_slice(&[0; 4]);
                }
            }
        }
        from = chain.len();
        albedo_from += w * h * 4;
        chain.extend_from_slice(&level);
        (w, h) = (nw, nh);
    }
    chain
}

/// The surface atlas as an image Bevy can sample: `surface` (linear, see [`surface_mip_chain`])
/// with its mipmaps, weighted by the coverage of `albedo` (the colour atlas's base level).
pub(crate) fn surface_image(surface: &[u8], albedo: &[u8], width: u32, height: u32) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        surface.to_vec(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(surface_mip_chain(surface, &with_mip_chain(albedo, width, height), width, height));
    image.texture_descriptor.mip_level_count = MIP_LEVELS;
    image.sampler = atlas_sampler();
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where `(y, s)` (height, depth toward the camera) lands up the picture,
    /// measured from the picture's centre, as a share of the view's height.
    fn up_the_picture(frame: &ImpostorFrame, y: f32, s: f32) -> f32 {
        let (sin, cos) = ELEVATION_DEGREES.to_radians().sin_cos();
        let center = frame.center_y * cos;
        (y * cos - s * sin - center) / frame.view.y
    }

    #[test]
    fn frame_holds_the_whole_crown_seen_from_above_the_horizon() {
        let (height, radius) = (14.0, 6.0);
        let frame = ImpostorFrame::new(height, radius);
        // The tree's extremes: the base at the near and far edge of the crown,
        // the top of the crown at the near and far edge.
        for (y, s) in [
            (0.0, radius),
            (0.0, -radius),
            (height, radius),
            (height, -radius),
        ] {
            let up = up_the_picture(&frame, y, s);
            assert!(
                up.abs() < 0.5,
                "({y}, {s}) is {up} views from the centre, outside the picture"
            );
        }
        // The crown's width fits across, with a margin.
        assert!(frame.view.x > 2.0 * radius);
        // The quad stands upright, so it is a little taller than the picture.
        assert!(frame.size.y >= frame.view.y);
        assert_eq!(frame.size.x, frame.view.x);
    }

    #[test]
    fn quad_covers_the_frame_and_faces_the_camera() {
        let frame = ImpostorFrame::new(10.0, 4.0);
        let quad = frame.quad();
        assert_eq!(quad.count_vertices(), 4);
        let positions = quad
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let (bottom, top) = (
            frame.center_y - frame.size.y / 2.0,
            frame.center_y + frame.size.y / 2.0,
        );
        assert!(positions.iter().all(|p| p[2] == 0.0));
        assert_eq!(positions[0], [-frame.size.x / 2.0, bottom, 0.0]);
        assert_eq!(positions[2], [frame.size.x / 2.0, top, 0.0]);
        // Counter-clockwise seen from +Z, where the billboard's front is.
        let Some(Indices::U32(indices)) = quad.indices() else {
            panic!("quad has u32 indices")
        };
        let (a, b, c) = (
            positions[indices[0] as usize],
            positions[indices[1] as usize],
            positions[indices[2] as usize],
        );
        let cross_z = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(cross_z > 0.0);
    }

    #[test]
    fn mip_chain_has_every_level_and_keeps_color_at_the_edge_of_a_shape() {
        // 128x32: the left half an opaque orange, the right half empty.
        let (w, h) = (128usize, 32usize);
        let orange = [230u8, 120, 20, 255];
        let mut base = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w / 2 {
                let at = (y * w + x) * 4;
                base[at..at + 4].copy_from_slice(&orange);
            }
        }
        let chain = with_mip_chain(&base, w as u32, h as u32);
        let texels: usize = (0..MIP_LEVELS).map(|l| (w >> l) * (h >> l)).sum();
        assert_eq!(chain.len(), texels * 4);

        // The last level is 4x1: two orange texels, two empty ones.
        let last = &chain[chain.len() - 16..];
        for (texel, expected) in last.chunks(4).zip([true, true, false, false]) {
            if expected {
                assert!(
                    texel.iter().zip(orange).all(|(&a, b)| a.abs_diff(b) <= 1),
                    "{texel:?}"
                );
            } else {
                assert_eq!(texel[3], 0);
            }
        }

        // A texel half covered keeps the full colour, not an orange darkened by
        // the empty black next to it. Empty the column at x = 63 in the first two
        // rows: level 1's texel (31, 0) averages two orange and two empty texels.
        for y in 0..2 {
            let at = (y * w + 63) * 4;
            base[at..at + 4].fill(0);
        }
        let chain = with_mip_chain(&base, w as u32, h as u32);
        let edge = &chain[w * h * 4 + 31 * 4..][..4];
        assert_eq!(edge[3], 128, "half covered: {edge:?}");
        assert!(
            edge.iter()
                .take(3)
                .zip(orange)
                .all(|(&a, b)| a.abs_diff(b) <= 1),
            "{edge:?} lost its color"
        );
    }

    #[test]
    fn oct_round_trips() {
        let mut directions = Vec::new();
        for x in [-1.0f32, 0.0, 1.0] {
            for y in [-1.0f32, 0.0, 1.0] {
                for z in [-1.0f32, 0.0, 1.0] {
                    let v = Vec3::new(x, y, z);
                    if v != Vec3::ZERO {
                        directions.push(v.normalize());
                    }
                }
            }
        }
        assert_eq!(directions.len(), 26);
        let mut rng = crate::SeededRng::new(5);
        for _ in 0..100 {
            let v = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            directions.push(v.normalize());
        }
        for n in directions {
            let back = oct_decode(oct_encode(n));
            assert!(back.distance(n) < 1e-5, "{n:?} came back as {back:?}");

            // Through the 8 bits a texel has.
            let bytes = (oct_encode(n) * 0.5 + Vec2::splat(0.5)) * 255.0;
            let stored = Vec2::new(bytes.x.round(), bytes.y.round()) / 255.0 * 2.0 - Vec2::ONE;
            let degrees = oct_decode(stored).dot(n).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(degrees < 1.5, "{n:?} is {degrees} degrees off after quantising");
        }
    }

    #[test]
    fn surface_mips_average_covered_normals_only() {
        // 8x4: the left half holds +X in every other column and is covered there, the
        // uncovered columns and the whole right half hold +Y.
        let (w, h) = (8usize, 4usize);
        let byte = |e: Vec2| [((e.x * 0.5 + 0.5) * 255.0).round() as u8, ((e.y * 0.5 + 0.5) * 255.0).round() as u8];
        let (x_axis, y_axis) = (byte(oct_encode(Vec3::X)), byte(oct_encode(Vec3::Y)));
        let mut surface = vec![0u8; w * h * 4];
        let mut albedo = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let at = (y * w + x) * 4;
                let covered = x < w / 2 && x % 2 == 0;
                let normal = if covered { x_axis } else { y_axis };
                surface[at..at + 4].copy_from_slice(&[normal[0], normal[1], 200, 255]);
                albedo[at..at + 4].copy_from_slice(&[200, 100, 50, if covered { 255 } else { 0 }]);
            }
        }
        let chain = surface_mip_chain(&surface, &with_mip_chain(&albedo, w as u32, h as u32), w as u32, h as u32);
        let level_one = &chain[w * h * 4..];
        for x in 0..w / 2 {
            let texel = &level_one[x * 4..][..4];
            if x < w / 4 {
                assert!(texel_normal(texel).dot(Vec3::X) > 0.99, "texel {x}: {texel:?} is not +X");
                assert_eq!((texel[2], texel[3]), (200, 255));
            } else {
                assert_eq!(texel, [0, 0, 0, 0], "texel {x} has nothing under it");
            }
        }
    }

    #[test]
    fn linearize_surface_decodes_srgb_and_keeps_alpha() {
        let out = linearize_surface(&[188, 0, 255, 77, 255, 255, 255, 0]);
        assert!(out[0].abs_diff(128) <= 1, "188 became {}", out[0]);
        assert_eq!((out[1], out[2], out[3]), (0, 255, 77));
        assert_eq!(out[4..], [255, 255, 255, 0]);
    }
}
