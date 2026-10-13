//! `GpuGrowth` in a headless app: what the systems upload and when.

use bevy::app::App;
use bevy::asset::{AssetApp, AssetEvent, AssetPlugin, Assets};
use bevy::ecs::prelude::*;
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::render::storage::ShaderBuffer;

use super::state::GrowthState;
use super::{GpuGrowth, GpuGrowthSupport, GrowthMaterial};
use crate::params::TreeParams;
use crate::background::BuildInBackground;
use crate::plugin::{FloraPlugin, Paint, ProceduralTree, TreeAge, TreeGenerated, TreeRender};
use crate::presets::TreePreset;
use crate::skeleton::generate_skeleton;
use crate::{build_tree, pipeline};

fn app(support: bool) -> App {
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(FloraPlugin);
    app.insert_resource(GpuGrowthSupport(support));
    app
}

/// The node buffer a tree should hold at `age`.
fn expected_buffer(params: &TreeParams, age: f32) -> Vec<[f32; 4]> {
    let full = generate_skeleton(params);
    let mut state = GrowthState::default();
    state.update(&full, params, age);
    let mut out = vec![[0.0; 4]; state.vec4_len()];
    state.encode(&mut out);
    out
}

/// The node buffer a tree holds now, if it is painted for the GPU.
fn uploaded_buffer(app: &App, tree: Entity) -> Option<Vec<[f32; 4]>> {
    let render = app.world().get::<TreeRender>(tree)?;
    let Paint::Growth(paint) = &render.paint else { return None };
    let buffers = app.world().resource::<Assets<ShaderBuffer>>();
    Some(buffers.get(&paint.nodes)?.cast_slice::<[f32; 4]>()?.to_vec())
}

fn modified<A: bevy::asset::Asset>(app: &App) -> usize {
    app.world()
        .resource::<Messages<AssetEvent<A>>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, AssetEvent::Modified { .. }))
        .count()
}

fn is_gpu(app: &App, tree: Entity) -> bool {
    matches!(app.world().get::<TreeRender>(tree).map(|r| &r.paint), Some(Paint::Growth(_)))
}

#[test]
fn growing_on_the_gpu_writes_node_state_and_nothing_else() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(2.0), GpuGrowth))
        .id();
    app.update();
    assert!(is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 2.0));

    let mature = build_tree(&params, f32::INFINITY);
    let mut cursor = app.world().resource::<Messages<TreeGenerated>>().get_cursor_current();
    for age in [3.0, 4.0, 5.0, 6.0, 7.5] {
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
        app.update();
        assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, age), "age {age}");
        // Neither the mesh nor either material is touched: a modified asset is
        // re-prepared by the renderer, and the point is to upload only the buffer.
        assert_eq!(modified::<Mesh>(&app), 0, "age {age}: mesh");
        assert_eq!(modified::<GrowthMaterial>(&app), 0, "age {age}: material");
        let sent: Vec<_> = cursor.read(app.world().resource::<Messages<TreeGenerated>>()).copied().collect();
        assert_eq!(sent.len(), 1, "age {age}: {sent:?}");
        let progress = pipeline::build_tree(&params, age).skeleton.nodes[0].progress;
        assert!((sent[0].height - params.trunk.height * progress).abs() < 1e-4, "age {age}");
    }

    // The mesh in the asset store is the mature tree at every age.
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let wood = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
    assert!(wood.count_vertices() >= mature.trunk.positions.len());
    assert!(wood.attribute(super::ATTRIBUTE_GROWTH_ID).is_some());
}

#[test]
fn a_gpu_tree_built_in_the_background_takes_the_age_it_finds_and_then_ages_in_the_frame() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(2.0), GpuGrowth, BuildInBackground))
        .id();
    app.update();
    assert!(!is_gpu(&app, tree), "the first frame only starts the build");
    // It ages while it is built: the state it gets is of the age it has when the build is done.
    app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 4.0;
    for _ in 0..3000 {
        app.update();
        if is_gpu(&app, tree) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(is_gpu(&app, tree), "the build never finished");
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 4.0));
    // From then on its age is written in the frame, as without the component.
    app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 5.0;
    app.update();
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 5.0));
}

#[test]
fn changing_the_parameters_rebuilds_the_mature_tree_and_the_buffer() {
    let mut params = TreePreset::Oak.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(4.0), GpuGrowth))
        .id();
    app.update();

    params.seed = 11;
    params.trunk.height *= 1.3;
    app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0 = params.clone();
    app.update();
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 4.0));
    assert_eq!(modified::<Mesh>(&app), 2, "the wood and leaf meshes are replaced in place");

    // A colour change touches the materials, once, and not the mesh. Counted from a cursor: the
    // frame before rewrote both materials for the new trunk height, and its messages can still
    // be around.
    let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
    app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0.trunk_color = red;
    let mut cursor = app.world().resource::<Messages<AssetEvent<GrowthMaterial>>>().get_cursor_current();
    app.update();
    let materials_modified = cursor
        .read(app.world().resource::<Messages<AssetEvent<GrowthMaterial>>>())
        .filter(|e| matches!(e, AssetEvent::Modified { .. }))
        .count();
    assert_eq!(materials_modified, 1);
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let Paint::Growth(paint) = &render.paint else { panic!("still on the GPU") };
    assert_eq!(world.resource::<Assets<GrowthMaterial>>().get(&paint.trunk).unwrap().base.base_color, red);
}

#[test]
fn a_tree_follows_its_gpu_growth_component_and_the_devices_support() {
    let params = TreePreset::Willow.params();
    let mut app = app(true);
    let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(5.0))).id();
    app.update();
    assert!(!is_gpu(&app, tree), "no GpuGrowth: the CPU path");

    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.update();
    assert!(is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2, "the old children are gone");

    app.world_mut().entity_mut(tree).remove::<GpuGrowth>();
    app.update();
    assert!(!is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let uploaded = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
    assert_eq!(uploaded.count_vertices(), build_tree(&params, 5.0).trunk.positions.len());

    // A device without storage buffers keeps the tree on the CPU, and a device
    // that turns out to have them moves it over.
    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.world_mut().insert_resource(GpuGrowthSupport(false));
    app.update();
    assert!(!is_gpu(&app, tree));
    app.world_mut().insert_resource(GpuGrowthSupport(true));
    app.update();
    assert!(is_gpu(&app, tree));
}

#[test]
fn a_tree_has_its_levels_of_detail_only_while_it_is_built_on_the_cpu() {
    use crate::lod::TreeLod;
    use bevy::camera::visibility::VisibilityRange;
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY), TreeLod::default()))
        .id();
    let children = |app: &App| app.world().get::<Children>(tree).unwrap().to_vec();
    let has_levels = |app: &App| app.world().get::<TreeRender>(tree).unwrap().lod.is_some();
    app.update();
    // Trunk and leaves, then a wood and a leaf child per coarse level.
    assert!(has_levels(&app));
    assert_eq!(children(&app).len(), 6);

    // On the GPU it has one level: the coarse children go with the CPU's, and nothing is banded.
    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.update();
    assert!(is_gpu(&app, tree));
    assert!(!has_levels(&app));
    let gpu_children = children(&app);
    assert_eq!(gpu_children.len(), 2);
    for child in gpu_children {
        assert!(app.world().get::<VisibilityRange>(child).is_none(), "{child} is banded");
    }

    app.world_mut().entity_mut(tree).remove::<GpuGrowth>();
    app.update();
    assert!(!is_gpu(&app, tree));
    assert!(has_levels(&app));
    assert_eq!(children(&app).len(), 6);
}

#[test]
fn growing_trees_reach_their_mature_state_on_the_gpu() {
    use crate::plugin::Growing;
    let params = TreePreset::Spruce.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(0.0), Growing { rate: 40.0 }, GpuGrowth))
        .id();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
    let mature_at = generate_skeleton(&params).mature_at(&params.growth);
    for _ in 0..4 {
        app.update();
    }
    let age = app.world().get::<TreeAge>(tree).unwrap().0;
    assert!(age > 0.0 && age <= mature_at);
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, age));
}

#[test]
fn the_plugin_can_be_added_before_the_asset_plugin() {
    // Plugins may be added in any order (0.3.5 did not care). GPU growth needs
    // the renderer, which is not there yet: it stays off and trees grow on the CPU.
    let mut app = App::new();
    app.add_plugins(FloraPlugin);
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.finish();
    app.cleanup();
    assert!(!app.world().resource::<GpuGrowthSupport>().0);

    let params = TreePreset::Oak.params();
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(4.0), GpuGrowth))
        .id();
    app.update();
    assert!(app.world().get::<TreeRender>(tree).is_some());
    assert!(!is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
}

#[test]
fn a_hull_or_prism_tree_stays_on_the_cpu_under_gpu_growth_and_a_change_of_parameters_moves_it() {
    use crate::params::TrunkStyle;
    let hull = crate::crown_hull::hull_spruce();
    let mut prism = TreePreset::Oak.params();
    prism.trunk_style = TrunkStyle::Prism { sides: 5 };
    let oak = TreePreset::Oak.params();
    let mut app = app(true);
    let spawn = |app: &mut App, params: &TreeParams| {
        app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(f32::INFINITY), GpuGrowth)).id()
    };
    let (hulled, prismatic, round) = (spawn(&mut app, &hull), spawn(&mut app, &prism), spawn(&mut app, &oak));
    app.update();
    assert!(!is_gpu(&app, hulled), "a hull crown grew on the GPU");
    assert!(!is_gpu(&app, prismatic), "a prism trunk grew on the GPU");
    assert!(is_gpu(&app, round), "a plain tree with `GpuGrowth` is on the GPU");
    for tree in [hulled, prismatic] {
        assert!(app.world().get::<TreeRender>(tree).is_some(), "built on the CPU, not left unbuilt");
        assert!(app.world().get::<Children>(tree).is_some_and(|children| !children.is_empty()));
    }

    // Changing a tree's parameters moves it between the paths, both ways.
    app.world_mut().get_mut::<ProceduralTree>(hulled).unwrap().0 = oak.clone();
    app.world_mut().get_mut::<ProceduralTree>(round).unwrap().0 = hull.clone();
    for _ in 0..3 {
        app.update();
    }
    assert!(is_gpu(&app, hulled), "a tree that lost its hull did not move to the GPU");
    assert!(!is_gpu(&app, round), "a tree that gained a hull stayed on the GPU");
    assert!(app.world().get::<TreeRender>(round).is_some());
}

#[test]
fn only_a_leaf_card_tree_with_a_round_trunk_grows_on_the_gpu() {
    use super::grows_on_gpu;
    use crate::params::{CrownStyle, TrunkStyle};
    let oak = TreePreset::Oak.params();
    let hull = TreeParams { crown: CrownStyle::Hull { tiers: 3, sides: 5, lift: 0.2, jitter: 1.0, trunk_only: true }, ..oak.clone() };
    let prism = TreeParams { trunk_style: TrunkStyle::Prism { sides: 5 }, ..oak.clone() };
    for (params, expected) in [(&oak, true), (&hull, false), (&prism, false)] {
        assert_eq!(grows_on_gpu(true, &GpuGrowthSupport(true), params), expected);
        assert!(!grows_on_gpu(false, &GpuGrowthSupport(true), params), "no component, no GPU");
        assert!(!grows_on_gpu(true, &GpuGrowthSupport(false), params), "no support, no GPU");
    }
}
