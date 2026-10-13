//! The textures trees wear: the bundled CC0 scans (see `src/textures/CREDITS.md`) and the vein
//! images grown on the CPU, as images.
//!
//! [`FloraTextures`] decodes each scan the first time a tree asks for it and keeps it, so a
//! forest of oaks shares one pair of images. Without the `textures` feature nothing is bundled
//! and every scan resolves to `None`: wood and leaves are plain colour (leaves keep their
//! procedural shape, with veins grown by space colonisation, which needs no bundled file).

use std::collections::HashMap;

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::color::{Color, LinearRgba};
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::Surface;
use crate::foliage::uses_scan;
use crate::params::{BarkScan, LeafScan, LeafStyle, TreeParams};
use crate::venation;

/// Most a material's base colour may be brightened by dividing out a scan's average colour
/// (per linear channel): a pale preset on a dark scan, such as white birch, must not overflow.
const MAX_TINT: f32 = 8.0;

/// Seed of the leaf veins every tree shares. One vein image serves every leaf-shaped card, so
/// trees keep sharing their leaf material whatever their seeds.
const VENATION_SEED: i32 = 11;

/// `preset` divided by `mean`, per linear channel, at most [`MAX_TINT`] times brighter:
/// multiplied by a texture that averages `mean` it gives `preset` on average.
fn over_mean(preset: Color, mean: LinearRgba) -> Color {
    let preset = preset.to_linear();
    let divide = |preset: f32, mean: f32| (preset / mean.max(1.0e-3)).min(MAX_TINT);
    Color::linear_rgba(
        divide(preset.red, mean.red),
        divide(preset.green, mean.green),
        divide(preset.blue, mean.blue),
        preset.alpha,
    )
}

/// A decoded bark scan: its images and average colour.
#[derive(Clone, Debug)]
pub struct BarkTexture {
    /// Which scan this is.
    pub scan: BarkScan,
    /// The colour map (sRGB, repeating, with mipmaps).
    pub color: Handle<Image>,
    /// The normal map (tangent space, OpenGL convention, repeating, with mipmaps).
    pub normal: Handle<Image>,
    /// The colour map's average, in linear light.
    pub mean: LinearRgba,
}

impl BarkTexture {
    /// The base colour a material needs for the scan to average out to `preset`: `preset`
    /// divided by the scan's mean, per linear channel and at most 8 times brighter. Multiplying
    /// it by the colour map gives wood whose average colour is `preset`, so far trees, impostors
    /// and untextured trees keep their colour.
    pub fn base_color(&self, preset: Color) -> Color {
        over_mean(preset, self.mean)
    }
}

/// A decoded leaf scan: the atlas of four leaves and its average colour.
#[derive(Clone, Debug)]
pub struct LeafTexture {
    /// Which scan this is.
    pub scan: LeafScan,
    /// The colour atlas (sRGB, with mipmaps).
    pub color: Handle<Image>,
    /// The normal atlas (tangent space, OpenGL convention, with mipmaps).
    pub normal: Handle<Image>,
    /// The average linear colour of the leaves' own pixels in the colour atlas.
    pub mean: LinearRgba,
}

impl LeafTexture {
    /// The base colour a leaf material needs for the atlas to average out to `preset`.
    pub fn base_color(&self, preset: Color) -> Color {
        over_mean(preset, self.mean)
    }
}

/// The bundled textures, decoded on first use, and the vein image. Resolving one needs the image
/// assets; where there are none (a headless app) or a scan cannot be decoded, trees are plain
/// colour.
#[derive(Resource, Default)]
pub struct FloraTextures {
    bark: HashMap<BarkScan, Option<BarkTexture>>,
    leaves: HashMap<LeafScan, Option<LeafTexture>>,
    venation: Option<Handle<Image>>,
    end_grain: Option<Handle<Image>>,
}

impl FloraTextures {
    /// The images of `scan`, decoded and added to `images` on the first call. `None` without the
    /// `textures` feature, or when the scan fails to decode (logged once).
    pub fn bark(&mut self, images: &mut Assets<Image>, scan: BarkScan) -> Option<BarkTexture> {
        self.bark.entry(scan).or_insert_with(|| load_bark(images, scan)).clone()
    }

    /// The atlas of `scan`, decoded and added to `images` on the first call. `None` without the
    /// `textures` feature, or when the scan fails to decode (logged once).
    pub fn leaves(&mut self, images: &mut Assets<Image>, scan: LeafScan) -> Option<LeafTexture> {
        self.leaves.entry(scan).or_insert_with(|| load_leaves(images, scan)).clone()
    }

    /// The image of leaf veins grown by space colonisation ([`venation`]), made on the first
    /// call and shared by every leaf without a scan: `R8`, 128 x 256, with mipmaps, the card's
    /// UVs address it (the petiole at `v = 0`).
    pub fn venation(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.venation.get_or_insert_with(|| images.add(venation_image())).clone()
    }
}

/// [`FloraTextures`] and the image assets together, for systems that resolve a tree's textures:
/// the plugin's systems take it. Not for building by hand.
#[derive(SystemParam)]
pub struct TextureLibrary<'w> {
    textures: ResMut<'w, FloraTextures>,
    images: Option<ResMut<'w, Assets<Image>>>,
}

impl TextureLibrary<'_> {
    /// The surface of the wood of a tree with `params`, with the bark scan they ask for.
    pub(crate) fn wood(&mut self, params: &TreeParams) -> Surface {
        let bark = params
            .bark
            .scan
            .zip(self.images.as_deref_mut())
            .and_then(|(scan, images)| self.textures.bark(images, scan));
        Surface::wood(params, bark)
    }

    /// The surface of the leaves of a tree with `params`: with the scan's atlas when they ask for
    /// one that decodes, else with the vein image (when the leaves have veins to draw). A hull
    /// crown ([`CrownStyle::Hull`](crate::CrownStyle)) wears neither: its facets are flat colour.
    pub(crate) fn leaves(&mut self, params: &TreeParams) -> Surface {
        let foliage = &params.foliage;
        let hull = matches!(params.crown, crate::params::CrownStyle::Hull { .. });
        let Some(images) = self.images.as_deref_mut().filter(|_| !hull) else {
            return Surface::leaves(params, None, None);
        };
        if uses_scan(foliage) {
            let scan = foliage.scan.and_then(|scan| self.textures.leaves(images, scan));
            return Surface::leaves(params, scan, None);
        }
        let leaf_shaped = !matches!(foliage.style, LeafStyle::NeedleCluster | LeafStyle::ClusterSphere);
        let venation = (leaf_shaped && foliage.shading.veins > 0.0).then(|| self.textures.venation(images));
        Surface::leaves(params, None, venation)
    }

    /// The end-grain picture a cut face wears (a felled tree and its stump): made on the first
    /// call and shared. `None` without image assets (a headless app).
    pub(crate) fn end_grain(&mut self) -> Option<Handle<Image>> {
        let images = self.images.as_deref_mut()?;
        Some(self.textures.end_grain.get_or_insert_with(|| images.add(crate::felling::end_grain_image())).clone())
    }
}

/// The vein image: space colonisation, rasterised, with a box-filtered mip chain.
fn venation_image() -> Image {
    let (width, height) = (venation::IMAGE_WIDTH as usize, venation::IMAGE_HEIGHT as usize);
    let base = venation::rasterize(&venation::grow(VENATION_SEED), venation::IMAGE_WIDTH, venation::IMAGE_HEIGHT);
    let levels = 6u32;
    let mut chain = base;
    let (mut w, mut h) = (width, height);
    let mut from = 0;
    for _ in 1..levels {
        let (nw, nh) = (w / 2, h / 2);
        let mut level = Vec::with_capacity(nw * nh);
        for y in 0..nh {
            for x in 0..nw {
                let at = from + 2 * y * w + 2 * x;
                let sum = u32::from(chain[at]) + u32::from(chain[at + 1]) + u32::from(chain[at + w]) + u32::from(chain[at + w + 1]);
                level.push(((sum + 2) / 4) as u8);
            }
        }
        from = chain.len();
        chain.extend_from_slice(&level);
        (w, h) = (nw, nh);
    }
    // `Image::new` takes the base level alone (debug builds check its length); the chain replaces
    // the data afterwards, as for the scans' images.
    let mut image = Image::new(
        Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        chain[..width * height].to_vec(),
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(chain);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    image
}

#[cfg(feature = "textures")]
fn load_bark(images: &mut Assets<Image>, scan: BarkScan) -> Option<BarkTexture> {
    let (color_bytes, normal_bytes) = bundled::bark_bytes(scan);
    let (color, normal, mean) = bundled::load_pair(images, bundled::bark_name(scan), color_bytes, normal_bytes, None)?;
    Some(BarkTexture { scan, color, normal, mean })
}

#[cfg(not(feature = "textures"))]
fn load_bark(_images: &mut Assets<Image>, _scan: BarkScan) -> Option<BarkTexture> {
    None
}

#[cfg(feature = "textures")]
fn load_leaves(images: &mut Assets<Image>, scan: LeafScan) -> Option<LeafTexture> {
    let (color_bytes, normal_bytes) = bundled::leaf_bytes(scan);
    let [r, g, b] = crate::textures::leaves::LEAF_SETS[scan as usize].mean;
    let mean = LinearRgba::rgb(r, g, b);
    let (color, normal, mean) =
        bundled::load_pair(images, bundled::leaf_name(scan), color_bytes, normal_bytes, Some(mean))?;
    Some(LeafTexture { scan, color, normal, mean })
}

#[cfg(not(feature = "textures"))]
fn load_leaves(_images: &mut Assets<Image>, _scan: LeafScan) -> Option<LeafTexture> {
    None
}

/// Decoding the compiled-in files.
#[cfg(feature = "textures")]
mod bundled {
    use bevy::asset::{Assets, Handle, RenderAssetUsages};
    use bevy::color::LinearRgba;
    use bevy::image::{
        CompressedImageFormats, Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor,
        ImageType,
    };
    use bevy::log::warn;
    use bevy::render::render_resource::TextureFormat;

    use crate::impostor::{MIP_LEVELS, srgb_to_linear, with_mip_chain};
    use crate::params::{BarkScan, LeafScan};

    /// Decode, mip and add the colour and normal map named `name`; the colour map's average is
    /// `mean` when it is given (a leaf atlas: only the leaves' pixels count), else measured.
    pub(super) fn load_pair(
        images: &mut Assets<Image>,
        name: &str,
        color_bytes: &[u8],
        normal_bytes: &[u8],
        mean: Option<LinearRgba>,
    ) -> Option<(Handle<Image>, Handle<Image>, LinearRgba)> {
        let fail = |what: &str, error: &str| {
            warn!("bevy_flora: texture {name} ({what}) cannot be used ({error}); the trees that wear it are plain colour");
        };
        let mut color = decode_jpeg(color_bytes, true).inspect_err(|e| fail("colour", e)).ok()?;
        let mut normal = decode_jpeg(normal_bytes, false).inspect_err(|e| fail("normal", e)).ok()?;
        let mean = match mean.or_else(|| mean_linear(&color)) {
            Some(mean) => mean,
            None => {
                fail("colour", "no pixel data");
                return None;
            }
        };
        let chain = |image: &mut Image, mips: fn(&[u8], u32, u32) -> Vec<u8>| -> Result<(), String> {
            let (w, h) = (image.width(), image.height());
            let step = 1u32 << (MIP_LEVELS - 1);
            if w % step != 0 || h % step != 0 {
                return Err(format!("{w}x{h} is not a multiple of {step} pixels"));
            }
            let base = image.data.as_deref().ok_or("no pixel data")?;
            image.data = Some(mips(base, w, h));
            image.texture_descriptor.mip_level_count = MIP_LEVELS;
            Ok(())
        };
        chain(&mut color, with_mip_chain).inspect_err(|e| fail("colour", e)).ok()?;
        chain(&mut normal, normal_mip_chain).inspect_err(|e| fail("normal", e)).ok()?;
        Some((images.add(color), images.add(normal), mean))
    }

    /// The sampler of the wood and leaf textures: they tile, so repeat; filtered, with the
    /// anisotropy a trunk seen along its length needs.
    fn sampler() -> ImageSampler {
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        })
    }

    /// A JPEG as an RGBA8 image, sRGB or linear, without mipmaps.
    fn decode_jpeg(bytes: &[u8], srgb: bool) -> Result<Image, String> {
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("jpg"),
            CompressedImageFormats::NONE,
            srgb,
            sampler(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .map_err(|error| error.to_string())?;
        let format = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
        if image.texture_descriptor.format == format {
            return Ok(image);
        }
        let from = image.texture_descriptor.format;
        image.convert(format).ok_or_else(|| format!("{from:?} cannot be converted to {format:?}"))
    }

    /// The average linear colour of an RGBA8 sRGB image, or `None` without pixel data.
    fn mean_linear(image: &Image) -> Option<LinearRgba> {
        let data = image.data.as_deref()?;
        let mut sum = [0.0f64; 3];
        for texel in data.as_chunks::<4>().0 {
            for (total, &byte) in sum.iter_mut().zip(texel) {
                *total += f64::from(srgb_to_linear(byte));
            }
        }
        let count = (data.len() / 4).max(1) as f64;
        Some(LinearRgba::rgb((sum[0] / count) as f32, (sum[1] / count) as f32, (sum[2] / count) as f32))
    }

    /// `base` (RGBA8 tangent-space normals, `width` x `height`) followed by its smaller levels,
    /// [`MIP_LEVELS`] in all. Each texel averages the four below it as vectors and renormalises,
    /// so distant bark smooths to a flat surface instead of a darker one; alpha is 255.
    pub(super) fn normal_mip_chain(base: &[u8], width: u32, height: u32) -> Vec<u8> {
        let decode = |byte: u8| f32::from(byte) / 127.5 - 1.0;
        let encode = |value: f32| ((value * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        let mut chain = base.to_vec();
        let (mut w, mut h) = (width as usize, height as usize);
        let mut from = 0;
        for _ in 1..MIP_LEVELS {
            let (nw, nh) = (w / 2, h / 2);
            let mut level = Vec::with_capacity(nw * nh * 4);
            for y in 0..nh {
                for x in 0..nw {
                    let mut sum = [0.0f32; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let at = from + ((2 * y + dy) * w + 2 * x + dx) * 4;
                        let n = [decode(chain[at]), decode(chain[at + 1]), decode(chain[at + 2])];
                        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1.0e-6);
                        for (total, value) in sum.iter_mut().zip(n) {
                            *total += value / length;
                        }
                    }
                    let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt().max(1.0e-6);
                    level.extend(sum.map(|value| encode(value / length)));
                    level.push(255);
                }
            }
            from = chain.len();
            chain.extend_from_slice(&level);
            (w, h) = (nw, nh);
        }
        chain
    }

    /// The file stem of a scan in `src/textures/bark/`.
    pub(super) fn bark_name(scan: BarkScan) -> &'static str {
        match scan {
            BarkScan::Oak => "oak",
            BarkScan::Pine => "pine",
            BarkScan::Willow => "willow",
            BarkScan::Birch => "birch",
            BarkScan::Palm => "palm",
            BarkScan::Cedar => "cedar",
            BarkScan::Zelkova => "zelkova",
            BarkScan::Maple => "maple",
            BarkScan::KnottedPine => "knotted_pine",
            BarkScan::Sakura => "sakura",
            BarkScan::PalmTree => "palm_tree",
            BarkScan::Hackberry => "hackberry",
            BarkScan::Brown => "brown",
            BarkScan::Metasequoia => "metasequoia",
        }
    }

    /// A bark scan's colour and normal map files, compiled in.
    pub(super) fn bark_bytes(scan: BarkScan) -> (&'static [u8], &'static [u8]) {
        macro_rules! files {
            ($name:literal) => {
                (
                    include_bytes!(concat!("../textures/bark/", $name, "_color.jpg")).as_slice(),
                    include_bytes!(concat!("../textures/bark/", $name, "_normal.jpg")).as_slice(),
                )
            };
        }
        match scan {
            BarkScan::Oak => files!("oak"),
            BarkScan::Pine => files!("pine"),
            BarkScan::Willow => files!("willow"),
            BarkScan::Birch => files!("birch"),
            BarkScan::Palm => files!("palm"),
            BarkScan::Cedar => files!("cedar"),
            BarkScan::Zelkova => files!("zelkova"),
            BarkScan::Maple => files!("maple"),
            BarkScan::KnottedPine => files!("knotted_pine"),
            BarkScan::Sakura => files!("sakura"),
            BarkScan::PalmTree => files!("palm_tree"),
            BarkScan::Hackberry => files!("hackberry"),
            BarkScan::Brown => files!("brown"),
            BarkScan::Metasequoia => files!("metasequoia"),
        }
    }

    /// The file stem of a scan in `src/textures/leaves/`.
    pub(super) fn leaf_name(scan: LeafScan) -> &'static str {
        match scan {
            LeafScan::Oak => "leaf_oak",
            LeafScan::Willow => "leaf_willow",
            LeafScan::Birch => "leaf_birch",
            LeafScan::Bonsai => "leaf_bonsai",
            LeafScan::Maple => "leaf_maple",
            LeafScan::Poplar => "leaf_poplar",
            LeafScan::JapaneseMaple => "leaf_japanese_maple",
        }
    }

    /// A leaf scan's colour and normal atlas files, compiled in.
    pub(super) fn leaf_bytes(scan: LeafScan) -> (&'static [u8], &'static [u8]) {
        macro_rules! files {
            ($name:literal) => {
                (
                    include_bytes!(concat!("../textures/leaves/", $name, "_color.jpg")).as_slice(),
                    include_bytes!(concat!("../textures/leaves/", $name, "_normal.jpg")).as_slice(),
                )
            };
        }
        match scan {
            LeafScan::Oak => files!("oak"),
            LeafScan::Willow => files!("willow"),
            LeafScan::Birch => files!("birch"),
            LeafScan::Bonsai => files!("bonsai"),
            LeafScan::Maple => files!("maple"),
            LeafScan::Poplar => files!("poplar"),
            LeafScan::JapaneseMaple => files!("japanese_maple"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "textures")]
    use crate::impostor::MIP_LEVELS;

    #[cfg(feature = "textures")]
    #[test]
    fn a_normal_mip_chain_averages_as_vectors() {
        // 32x32 normals tilted left in the left half and right in the right half, equally.
        let (w, h) = (32usize, 32usize);
        let tilt = |x: usize| if x < w / 2 { [64u8, 128, 220, 255] } else { [192u8, 128, 220, 255] };
        let base: Vec<u8> = (0..w * h).flat_map(|i| tilt(i % w)).collect();
        let chain = bundled::normal_mip_chain(&base, w as u32, h as u32);
        let texels: usize = (0..MIP_LEVELS).map(|l| (w >> l) * (h >> l)).sum();
        assert_eq!(chain.len(), texels * 4);
        // The 1x1 level (the last texel) is the flat normal: x and y centred, z the strongest.
        let last = &chain[chain.len() - 4..];
        assert!((i32::from(last[0]) - 128).abs() <= 1 && (i32::from(last[1]) - 128).abs() <= 1, "{last:?}");
        assert!(last[2] > 220 && last[3] == 255, "{last:?}");
    }

    #[cfg(feature = "textures")]
    #[test]
    fn every_bundled_scan_decodes_to_filterable_mipmapped_images() {
        let mut images = Assets::<Image>::default();
        let mut textures = FloraTextures::default();
        let mut pairs = Vec::new();
        for scan in BarkScan::ALL {
            let bark = textures.bark(&mut images, scan).unwrap_or_else(|| panic!("{scan:?} did not load"));
            pairs.push((format!("{scan:?}"), bark.color, bark.normal, bark.mean));
        }
        for scan in LeafScan::ALL {
            let leaves = textures.leaves(&mut images, scan).unwrap_or_else(|| panic!("{scan:?} leaves did not load"));
            pairs.push((format!("{scan:?} leaves"), leaves.color, leaves.normal, leaves.mean));
        }
        for (what, color, normal, mean) in pairs {
            for (handle, srgb) in [(&color, true), (&normal, false)] {
                let image = images.get(handle).expect("the image was added");
                let format = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
                assert_eq!(image.texture_descriptor.format, format, "{what}");
                assert_eq!(image.texture_descriptor.mip_level_count, MIP_LEVELS, "{what}");
                let texels: usize =
                    (0..MIP_LEVELS).map(|l| (image.width() as usize >> l) * (image.height() as usize >> l)).sum();
                assert_eq!(image.data.as_ref().map(Vec::len), Some(texels * 4), "{what}: mip chain length");
            }
            let normal = images.get(&normal).expect("normal");
            let data = normal.data.as_ref().expect("data");
            let base = &data[..(normal.width() * normal.height()) as usize * 4];
            let mean_z: f32 = base.as_chunks::<4>().0.iter().map(|t| f32::from(t[2]) / 255.0 * 2.0 - 1.0).sum::<f32>()
                / (base.len() / 4) as f32;
            assert!(mean_z > 0.5, "{what}: normals do not point out of the surface (mean z {mean_z})");
            for channel in [mean.red, mean.green, mean.blue] {
                assert!((0.005..0.9).contains(&channel), "{what}: implausible mean {mean:?}");
            }
        }
    }

    #[test]
    fn dividing_out_the_mean_keeps_the_preset_colour_on_average() {
        let bark = BarkTexture {
            scan: BarkScan::Oak,
            color: Handle::default(),
            normal: Handle::default(),
            mean: LinearRgba::rgb(0.2, 0.1, 0.005),
        };
        let preset = Color::linear_rgb(0.1, 0.1, 0.1);
        let base = bark.base_color(preset).to_linear();
        // A texture averaging `mean` times this base averages the preset.
        assert!((base.red * 0.2 - 0.1).abs() < 1e-5);
        assert!((base.green * 0.1 - 0.1).abs() < 1e-5);
        // A dark channel is clamped rather than blown up.
        assert_eq!(base.blue, MAX_TINT);
    }

    #[test]
    fn the_vein_image_is_a_mipmapped_r8_with_veins_in_every_level() {
        let image = venation_image();
        assert_eq!(image.texture_descriptor.format, TextureFormat::R8Unorm);
        let levels = image.texture_descriptor.mip_level_count;
        let (w, h) = (venation::IMAGE_WIDTH as usize, venation::IMAGE_HEIGHT as usize);
        let data = image.data.as_ref().expect("data");
        let texels: usize = (0..levels).map(|l| (w >> l) * (h >> l)).sum();
        assert_eq!(data.len(), texels);
        // Averaging keeps a trace of the veins down to the smallest level.
        assert!(data[data.len() - (w >> (levels - 1)) * (h >> (levels - 1))..].iter().any(|&v| v > 0));
    }
}
