//! CPU-side mesh buffers and the meshers that fill them.

pub mod branch;
pub mod cut;
pub mod rings;
pub mod smoothing;
pub mod trunk;

use bevy::asset::RenderAssetUsages;
use bevy::math::{Vec2, Vec3, Vec4};
use bevy::mesh::{Indices, Mesh, MeshVertexAttribute, PrimitiveTopology, VertexFormat};

/// Per leaf vertex: the leaf's flutter direction scaled by the distance from where it is
/// attached (xyz, metres) and its phase (w).
pub const ATTRIBUTE_LEAF_SWAY: MeshVertexAttribute =
    MeshVertexAttribute::new("LeafSway", 0x666C_6F72_6110, VertexFormat::Float32x4);

/// Triangle-list geometry: positions, normals, UVs and optionally vertex colours, a second UV
/// set (leaf shading) and a per-vertex leaf sway.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// Vertex positions, in metres.
    pub positions: Vec<Vec3>,
    /// Unit vertex normals, one per position.
    pub normals: Vec<Vec3>,
    /// Texture coordinates, one per position.
    pub uvs: Vec<Vec2>,
    /// Linear RGBA vertex colours, one per position, or empty for no colour attribute.
    pub colors: Vec<Vec4>,
    /// Second texture coordinates, one per position, or empty for none. Leaves use it for
    /// shading: `x` is 1 on the upper side of a card and 0 on the lower, `y` is how much of the
    /// sky the leaf sees (1 = all of it).
    pub uv1: Vec<Vec2>,
    /// Per leaf vertex, one per position or empty for none: see [`ATTRIBUTE_LEAF_SWAY`].
    pub sway: Vec<Vec4>,
    /// Triangle-list indices into the vertex arrays; length is a multiple of 3.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Append `other`, offsetting its indices. If either side carries vertex
    /// colours, the other side's vertices are padded with opaque white; `uv1` is padded with
    /// `(1, 1)` (upper side, fully visible) and `sway` with zero.
    pub fn extend(&mut self, other: &MeshData) {
        let offset = self.positions.len() as u32;
        pad(&mut self.colors, &other.colors, offset as usize, other.positions.len(), Vec4::ONE);
        pad(&mut self.uv1, &other.uv1, offset as usize, other.positions.len(), Vec2::ONE);
        pad(&mut self.sway, &other.sway, offset as usize, other.positions.len(), Vec4::ZERO);
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.indices.extend(other.indices.iter().map(|i| i + offset));
    }

    /// True when there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The same surface shaded flat: every triangle gets three vertices of its own carrying the
    /// face's normal, and copies of the original vertices' texture coordinates, colours, second
    /// coordinates and sway, so a prism reads as planes. A triangle with no area keeps its first
    /// vertex's normal. The vertex count is three times the triangle count.
    pub fn flat_shaded(&self) -> MeshData {
        let count = self.indices.len();
        let mut out = MeshData {
            positions: Vec::with_capacity(count),
            normals: Vec::with_capacity(count),
            uvs: Vec::with_capacity(count),
            colors: Vec::with_capacity(if self.colors.is_empty() { 0 } else { count }),
            uv1: Vec::with_capacity(if self.uv1.is_empty() { 0 } else { count }),
            sway: Vec::with_capacity(if self.sway.is_empty() { 0 } else { count }),
            indices: (0..count as u32).collect(),
        };
        for tri in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = tri.map(|i| self.positions[i as usize]);
            let face = (b - a).cross(c - a);
            let normal = if face.length_squared() > 1.0e-20 { face.normalize() } else { self.normals[tri[0] as usize] };
            for &i in tri {
                let i = i as usize;
                out.positions.push(self.positions[i]);
                out.normals.push(normal);
                out.uvs.push(self.uvs[i]);
                if !self.colors.is_empty() {
                    out.colors.push(self.colors[i]);
                }
                if !self.uv1.is_empty() {
                    out.uv1.push(self.uv1[i]);
                }
                if !self.sway.is_empty() {
                    out.sway.push(self.sway[i]);
                }
            }
        }
        out
    }

    /// Convert into a Bevy [`Mesh`] (triangle list, `u32` indices).
    pub fn into_bevy_mesh(self) -> Mesh {
        let positions: Vec<[f32; 3]> = self.positions.iter().map(Vec3::to_array).collect();
        let normals: Vec<[f32; 3]> = self.normals.iter().map(Vec3::to_array).collect();
        let uvs: Vec<[f32; 2]> = self.uvs.iter().map(Vec2::to_array).collect();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(self.indices));
        if !self.colors.is_empty() {
            let colors: Vec<[f32; 4]> = self.colors.iter().map(|v| v.to_array()).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
        if !self.uv1.is_empty() {
            let uv1: Vec<[f32; 2]> = self.uv1.iter().map(Vec2::to_array).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1);
        }
        if !self.sway.is_empty() {
            let sway: Vec<[f32; 4]> = self.sway.iter().map(|v| v.to_array()).collect();
            mesh.insert_attribute(ATTRIBUTE_LEAF_SWAY, sway);
        }
        mesh
    }
}

/// Append the per-vertex attribute of a mesh of `added` vertices to `into`, which holds one entry
/// for each of the `existing` vertices before it, or none. Either side lacking the attribute is
/// filled with `fill`; neither having it leaves `into` empty.
fn pad<T: Copy>(into: &mut Vec<T>, from: &[T], existing: usize, added: usize, fill: T) {
    if into.is_empty() && from.is_empty() {
        return;
    }
    into.resize(existing, fill);
    if from.is_empty() {
        into.resize(existing + added, fill);
    } else {
        into.extend_from_slice(from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle(color: Option<Vec4>) -> MeshData {
        MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![Vec2::ZERO; 3],
            colors: color.map_or_else(Vec::new, |c| vec![c; 3]),
            indices: vec![0, 1, 2],
            ..MeshData::default()
        }
    }

    #[test]
    fn extend_keeps_colors_aligned_with_positions() {
        let red = Vec4::new(1.0, 0.0, 0.0, 1.0);
        for (first, second) in [(None, Some(red)), (Some(red), None), (Some(red), Some(red)), (None, None)] {
            let mut mesh = triangle(first);
            mesh.extend(&triangle(second));
            let expected = if first.is_some() || second.is_some() { 6 } else { 0 };
            assert_eq!(mesh.colors.len(), expected, "{first:?} + {second:?}");
            if expected > 0 {
                assert_eq!(mesh.colors[0], first.unwrap_or(Vec4::ONE));
                assert_eq!(mesh.colors[3], second.unwrap_or(Vec4::ONE));
            }
        }
    }

    #[test]
    fn extend_pads_leaf_attributes() {
        let leaf = |upper: f32, sway: f32| MeshData {
            uv1: vec![Vec2::new(upper, 0.5); 3],
            sway: vec![Vec4::new(sway, 0.0, 0.0, 0.25); 3],
            ..triangle(None)
        };
        for (first, second) in [(None, Some((0.0, 2.0))), (Some((0.0, 2.0)), None), (Some((0.0, 2.0)), Some((0.0, 3.0))), (None, None)] {
            let mut mesh = first.map_or_else(|| triangle(None), |(u, s)| leaf(u, s));
            mesh.extend(&second.map_or_else(|| triangle(None), |(u, s)| leaf(u, s)));
            if first.is_none() && second.is_none() {
                assert!(mesh.uv1.is_empty() && mesh.sway.is_empty());
                continue;
            }
            assert_eq!(mesh.uv1.len(), 6, "{first:?} + {second:?}");
            assert_eq!(mesh.sway.len(), 6, "{first:?} + {second:?}");
            let (uv1, sway) = (&mesh.uv1, &mesh.sway);
            match first {
                Some((u, s)) => assert_eq!((uv1[0], sway[0].x), (Vec2::new(u, 0.5), s)),
                None => assert_eq!((uv1[0], sway[0]), (Vec2::ONE, Vec4::ZERO)),
            }
            match second {
                Some((u, s)) => assert_eq!((uv1[3], sway[3].x), (Vec2::new(u, 0.5), s)),
                None => assert_eq!((uv1[3], sway[3]), (Vec2::ONE, Vec4::ZERO)),
            }
        }
    }

    #[test]
    fn flat_shading_gives_each_triangle_its_own_face_normal_and_keeps_the_attributes() {
        // A quad folded along its diagonal: two triangles sharing two vertices, smooth normals.
        let mesh = MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.5), Vec3::Y],
            normals: vec![Vec3::Z; 4],
            uvs: (0..4).map(|i| Vec2::new(i as f32, 0.0)).collect(),
            colors: (0..4).map(|i| Vec4::splat(i as f32)).collect(),
            uv1: (0..4).map(|i| Vec2::new(0.0, i as f32)).collect(),
            sway: (0..4).map(|i| Vec4::new(0.0, 0.0, i as f32, 1.0)).collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let flat = mesh.flat_shaded();
        assert_eq!(flat.positions.len(), 6);
        assert_eq!(flat.indices, (0..6).collect::<Vec<u32>>());
        for (k, tri) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            let [a, b, c] = tri.map(|i| mesh.positions[i as usize]);
            let face = (b - a).cross(c - a).normalize();
            for v in 3 * k..3 * k + 3 {
                assert_eq!(flat.normals[v], face, "vertex {v}");
                let from = tri[v - 3 * k] as usize;
                assert_eq!(
                    (flat.positions[v], flat.uvs[v], flat.colors[v], flat.uv1[v], flat.sway[v]),
                    (mesh.positions[from], mesh.uvs[from], mesh.colors[from], mesh.uv1[from], mesh.sway[from])
                );
            }
        }
        assert_ne!(flat.normals[0], flat.normals[3], "the fold is a crease, not a smooth surface");
        // A mesh without the optional attributes gains none.
        let plain = triangle(None).flat_shaded();
        assert!(plain.colors.is_empty() && plain.uv1.is_empty() && plain.sway.is_empty());
    }
}
