//! The machine plate: the family API standard, checked against `FloraPlugin`.
//!
//! Names come from `System::name()`, which is the full type path only with Bevy's `debug` feature;
//! the dev-dependency on `bevy` turns it on, and the test asserts it saw at least one
//! `bevy_flora::` system so a build without it fails instead of passing vacuously.

use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::reflect::{AppTypeRegistry, ReflectComponent, ReflectResource};
use bevy::ecs::schedule::{Schedules, SystemSet};
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::reflect::{GetTypeRegistration, TypeRegistry};
use bevy_flora::plugin::{TreeFlowerMesh, TreeFoliageMesh, TreeLodMesh, TreeTrunkMesh};
use bevy_flora::{
    BuckTree, BuildInBackground, Burnt, FallenFruit, FellTree, Felled, FloraPlugin, FloraSystems,
    FoliagePusher, FromPool, GpuGrowth, Growing, IntoDeadfall, LeafBurst, PooledDeadfall,
    PooledLook, PooledTree, ProceduralTree, ProceduralTreeAsset, SeeThroughTrees, TreeAge,
    TreeCondition, TreeFruit, TreeLod, TreeLog, TreeNotch, TreeRender, TreeShape,
    TreeStump,
};

const CRATE: &str = "bevy_flora::";

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(FloraPlugin);
    app.update();
    app
}

/// Every system of the crate must be in one of the crate's root-set variants.
#[test]
fn every_system_is_in_the_root_set() {
    let app = app();
    let roots = [FloraSystems.intern()];
    let schedules = app.world().resource::<Schedules>();
    let mut seen = 0;
    let mut outside = Vec::new();
    for (label, schedule) in schedules.iter() {
        let Ok(systems) = schedule.systems() else {
            continue;
        };
        for (key, system) in systems {
            let name = system.name().to_string();
            if !name.starts_with(CRATE) {
                continue;
            }
            seen += 1;
            let in_root = roots.iter().any(|root| {
                schedule
                    .graph()
                    .systems_in_set(*root)
                    .is_ok_and(|set| set.contains(&key))
            });
            if !in_root {
                outside.push(format!("{name} in {label:?}"));
            }
        }
    }
    assert!(seen > 0, "no `{CRATE}` systems seen: is Bevy's `debug` feature on?");
    assert!(outside.is_empty(), "systems outside FloraSystems: {outside:#?}");
}

fn assert_component<T: GetTypeRegistration>(registry: &TypeRegistry) {
    let name = std::any::type_name::<T>();
    let registration = registry
        .get(std::any::TypeId::of::<T>())
        .unwrap_or_else(|| panic!("{name} is not registered by FloraPlugin"));
    assert!(registration.data::<ReflectComponent>().is_some(), "{name} has no ReflectComponent");
}

/// Every public component is reflected and registered.
#[test]
fn components_are_reflected() {
    let app = app();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    assert_component::<BuckTree>(&registry);
    assert_component::<BuildInBackground>(&registry);
    assert_component::<Burnt>(&registry);
    assert_component::<FallenFruit>(&registry);
    assert_component::<FellTree>(&registry);
    assert_component::<Felled>(&registry);
    assert_component::<FoliagePusher>(&registry);
    assert_component::<FromPool>(&registry);
    assert_component::<GpuGrowth>(&registry);
    assert_component::<Growing>(&registry);
    assert_component::<IntoDeadfall>(&registry);
    assert_component::<LeafBurst>(&registry);
    assert_component::<PooledDeadfall>(&registry);
    assert_component::<PooledLook>(&registry);
    assert_component::<PooledTree>(&registry);
    assert_component::<ProceduralTree>(&registry);
    assert_component::<ProceduralTreeAsset>(&registry);
    assert_component::<SeeThroughTrees>(&registry);
    assert_component::<TreeAge>(&registry);
    assert_component::<TreeCondition>(&registry);
    assert_component::<TreeFlowerMesh>(&registry);
    assert_component::<TreeFoliageMesh>(&registry);
    assert_component::<TreeFruit>(&registry);
    assert_component::<TreeLod>(&registry);
    assert_component::<TreeLodMesh>(&registry);
    assert_component::<TreeLog>(&registry);
    assert_component::<TreeNotch>(&registry);
    assert_component::<TreeRender>(&registry);
    assert_component::<TreeShape>(&registry);
    assert_component::<TreeStump>(&registry);
    assert_component::<TreeTrunkMesh>(&registry);
}

/// The crate has no settings resources: none of its public resources is named `*Settings`
/// (`LodSettings` and `DeadfallSettings` are values, not resources).
#[test]
fn no_settings_resources() {
    let app = app();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    let settings = registry
        .iter()
        .filter(|r| r.data::<ReflectResource>().is_some())
        .filter(|r| r.type_info().type_path().starts_with(CRATE))
        .map(|r| r.type_info().type_path().to_owned())
        .filter(|path| path.ends_with("Settings"))
        .collect::<Vec<_>>();
    assert!(settings.is_empty(), "settings resources need the settings checks: {settings:?}");
}
