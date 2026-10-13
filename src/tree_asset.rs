//! Trees from assets: [`TreeParams`] is an [`Asset`](bevy::asset::Asset), and a
//! [`ProceduralTreeAsset`] draws a tree from a handle to one.
//!
//! The plugin keeps the entity's [`ProceduralTree`] equal to the asset: it is inserted once the
//! asset has loaded and replaced whenever the asset changes, so every tree that shares a handle
//! follows an edit, whether it comes from code (`Assets<TreeParams>::get_mut`), an inspector, or,
//! with the `ron` and `file_watcher` features, a `.tree.ron` file saved on disk.
//!
//! With the `ron` feature, [`TreeParamsLoader`] loads `.tree.ron` files, and
//! [`TreeParams::to_ron`] / [`TreeParams::from_ron`] write and read the same format. A file may
//! leave fields out (they take their [`Default`]), and a field the crate does not know is an
//! error that names it, so a typo is reported instead of ignored.

use bevy::asset::{AssetApp, AssetEvent, AssetId, AssetServer, Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::reflect::Reflect;
use bevy::transform::components::Transform;

use crate::params::TreeParams;
use crate::plugin::ProceduralTree;

/// A tree drawn from a [`TreeParams`] asset instead of a value: the plugin inserts the entity's
/// [`ProceduralTree`] once the asset has loaded and replaces it whenever the asset changes, so
/// every tree that shares the handle rebuilds after an edit. Until the asset has loaded the
/// entity draws nothing. Set [`TreeAge`](crate::TreeAge), [`Growing`](crate::Growing),
/// [`GpuGrowth`](crate::GpuGrowth) or [`TreeShape`](crate::TreeShape) on the entity as on any
/// tree.
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use bevy_flora::prelude::*;
///
/// // Every tree spawned from the same handle follows edits to it.
/// fn spawn(mut commands: Commands, mut assets: ResMut<Assets<TreeParams>>) {
///     let maple = assets.add(TreePreset::Maple.params());
///     for x in [-6.0, 0.0, 6.0] {
///         commands.spawn((ProceduralTreeAsset(maple.clone()), Transform::from_xyz(x, 0.0, 0.0)));
///     }
/// }
/// ```
///
/// With the `ron` feature, `asset_server.load("trees/maple.tree.ron")` gives such a handle.
/// Pools are built from a value (`build_pool(assets.get(&handle)...)`) and do not follow the
/// asset.
#[derive(Component, Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
#[require(Transform, Visibility)]
pub struct ProceduralTreeAsset(pub Handle<TreeParams>);

/// Keep every [`ProceduralTreeAsset`]'s [`ProceduralTree`] equal to its asset: when the
/// component is new or changed, or the asset was added, loaded or modified.
pub(crate) fn sync_tree_assets(
    mut events: MessageReader<AssetEvent<TreeParams>>,
    assets: Res<Assets<TreeParams>>,
    changed_sources: Query<(), Changed<ProceduralTreeAsset>>,
    mut trees: Query<(Entity, Ref<ProceduralTreeAsset>, Option<&mut ProceduralTree>)>,
    mut commands: Commands,
) {
    let changed: Vec<AssetId<TreeParams>> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } => {
                Some(*id)
            }
            AssetEvent::Removed { .. } | AssetEvent::Unused { .. } => None,
        })
        .collect();
    if changed.is_empty() && changed_sources.is_empty() {
        return;
    }
    for (entity, source, tree) in &mut trees {
        if !source.is_changed() && !changed.contains(&source.0.id()) {
            continue;
        }
        let Some(params) = assets.get(&source.0) else { continue };
        match tree {
            // Equal parameters change nothing, so an unrelated reload rebuilds nothing.
            Some(mut tree) => {
                if tree.0 != *params {
                    tree.0 = params.clone();
                }
            }
            None => {
                commands.entity(entity).insert(ProceduralTree(params.clone()));
            }
        }
    }
}

/// Register the asset type (and its loader), when the asset server exists. The plugin may be
/// added before the asset plugin, so [`finish`] tries again.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.register_type::<ProceduralTreeAsset>();
    add_asset_type(app);
}

/// See [`build`].
pub(crate) fn finish(app: &mut bevy::app::App) {
    add_asset_type(app);
}

fn add_asset_type(app: &mut bevy::app::App) {
    if !app.world().contains_resource::<AssetServer>() || app.world().contains_resource::<Assets<TreeParams>>() {
        return;
    }
    app.init_asset::<TreeParams>().register_asset_reflect::<TreeParams>();
    #[cfg(feature = "ron")]
    app.register_asset_loader(TreeParamsLoader);
}

#[cfg(feature = "ron")]
pub use file::{TreeFileError, TreeParamsLoader};

#[cfg(feature = "ron")]
mod file {
    use bevy::asset::io::Reader;
    use bevy::asset::{AssetLoader, LoadContext};
    use bevy::reflect::TypePath;

    use crate::params::TreeParams;

    /// Loads [`TreeParams`] from `.tree.ron` files (the `ron` feature), the format
    /// [`TreeParams::to_ron`] writes. Registered by the plugin.
    #[derive(Clone, Copy, Debug, Default, TypePath)]
    #[type_path = "bevy_flora::tree_asset"]
    pub struct TreeParamsLoader;

    impl AssetLoader for TreeParamsLoader {
        type Asset = TreeParams;
        type Settings = ();
        type Error = TreeFileError;

        async fn load(
            &self,
            reader: &mut dyn Reader,
            _settings: &(),
            _load_context: &mut LoadContext<'_>,
        ) -> Result<TreeParams, TreeFileError> {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await.map_err(|error| TreeFileError(Problem::Io(error)))?;
            let text = std::str::from_utf8(&bytes).map_err(|error| TreeFileError(Problem::Utf8(error)))?;
            TreeParams::from_ron(text)
        }

        fn extensions(&self) -> &[&str] {
            &["tree.ron"]
        }
    }

    /// Why a `.tree.ron` file could not be read: its message names the line, the column and,
    /// for a misspelt field, the fields that exist.
    #[derive(Debug)]
    pub struct TreeFileError(Problem);

    #[derive(Debug)]
    enum Problem {
        Io(std::io::Error),
        Utf8(std::str::Utf8Error),
        Ron(ron::error::SpannedError),
    }

    impl std::fmt::Display for TreeFileError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.0 {
                Problem::Io(error) => write!(f, "could not read the tree file: {error}"),
                Problem::Utf8(error) => write!(f, "the tree file is not UTF-8: {error}"),
                Problem::Ron(error) => write!(f, "the tree file is not valid: {error}"),
            }
        }
    }

    impl std::error::Error for TreeFileError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.0 {
                Problem::Io(error) => Some(error),
                Problem::Utf8(error) => Some(error),
                Problem::Ron(error) => Some(error),
            }
        }
    }

    impl TreeParams {
        /// This tree as a `.tree.ron` file's text, every field written out: export a preset
        /// (`TreePreset::Maple.params().to_ron()`) and edit it from there.
        pub fn to_ron(&self) -> String {
            ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
                .expect("every field of TreeParams has a RON form")
        }

        /// A tree from a `.tree.ron` file's text. Fields left out take their [`Default`]; a
        /// field this crate does not know is an error that names it.
        pub fn from_ron(text: &str) -> Result<Self, TreeFileError> {
            ron::from_str(text).map_err(|error| TreeFileError(Problem::Ron(error)))
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::prelude::*;

    use crate::{FloraPlugin, ProceduralTree, ProceduralTreeAsset, TreeParams, TreePreset};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        app
    }

    #[test]
    fn trees_sharing_an_asset_follow_it_and_wait_for_it() {
        let mut app = app();
        let handle = app.world_mut().resource_mut::<Assets<TreeParams>>().reserve_handle();
        let trees: Vec<Entity> =
            (0..2).map(|_| app.world_mut().spawn(ProceduralTreeAsset(handle.clone())).id()).collect();
        app.update();
        assert!(app.world().get::<ProceduralTree>(trees[0]).is_none(), "a tree was drawn before its asset");

        let maple = TreePreset::Maple.params();
        app.world_mut().resource_mut::<Assets<TreeParams>>().insert(handle.id(), maple.clone()).unwrap();
        app.update();
        app.update();
        for &tree in &trees {
            assert_eq!(app.world().get::<ProceduralTree>(tree), Some(&ProceduralTree(maple.clone())));
        }

        let reseeded = TreeParams { seed: 7, ..maple };
        *app.world_mut().resource_mut::<Assets<TreeParams>>().get_mut(&handle).unwrap() = reseeded.clone();
        app.update();
        app.update();
        for &tree in &trees {
            assert_eq!(app.world().get::<ProceduralTree>(tree), Some(&ProceduralTree(reseeded.clone())));
        }

        // A tree spawned after the asset loaded takes it at once.
        let late = app.world_mut().spawn(ProceduralTreeAsset(handle.clone())).id();
        app.update();
        assert_eq!(app.world().get::<ProceduralTree>(late), Some(&ProceduralTree(reseeded)));
    }

    #[cfg(feature = "ron")]
    #[test]
    fn every_preset_survives_a_tree_file() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            assert_eq!(TreeParams::from_ron(&params.to_ron()).unwrap(), params, "{preset:?}");
        }
    }

    #[cfg(feature = "ron")]
    #[test]
    fn a_hull_crown_and_a_prism_trunk_survive_a_tree_file_and_a_file_may_leave_them_out() {
        let params = crate::crown_hull::hull_spruce();
        let text = params.to_ron();
        assert!(text.contains("Hull") && text.contains("Prism"), "{text}");
        assert_eq!(TreeParams::from_ron(&text).unwrap(), params);
        // A file written before they existed has neither: leaf cards and a round trunk.
        let old = TreeParams::from_ron("(seed: 3)").unwrap();
        assert_eq!((old.crown, old.trunk_style), (crate::CrownStyle::Leaves, crate::TrunkStyle::Round));
        let by_hand = TreeParams::from_ron("(crown: Hull(tiers: 4, sides: 6, lift: 0.1, jitter: 0.5), trunk_style: Prism(sides: 4))").unwrap();
        assert_eq!(by_hand.crown, crate::CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.5 });
        assert_eq!(by_hand.trunk_style, crate::TrunkStyle::Prism { sides: 4 });
    }

    #[cfg(feature = "ron")]
    #[test]
    fn a_tree_file_may_leave_fields_out_but_not_misspell_them() {
        let partial = TreeParams::from_ron("(seed: 7, trunk: (height: 9.0))").unwrap();
        let defaults = TreeParams::default();
        assert_eq!(partial.seed, 7);
        assert_eq!(partial.trunk.height, 9.0);
        assert_eq!(partial.trunk.radius, defaults.trunk.radius);
        assert_eq!(partial.foliage, defaults.foliage);

        let typo = TreeParams::from_ron("(trunk: (hieght: 9.0))").unwrap_err().to_string();
        assert!(typo.contains("hieght"), "{typo}");
    }

    #[cfg(feature = "ron")]
    #[test]
    fn the_example_tree_file_loads() {
        let text = include_str!("../examples/assets/trees/maple.tree.ron");
        TreeParams::from_ron(text).unwrap();
    }
}
