//! Trees from assets: [`TreeParams`] is an [`Asset`](bevy::asset::Asset), and a
//! [`ProceduralTreeAsset`] draws a tree from a handle to one.
//!
//! The plugin keeps the entity's [`ProceduralTree`] equal to the asset: it is inserted once the
//! asset has loaded and replaced whenever the asset changes, so every tree that shares a handle
//! follows an edit, whether it comes from code (`Assets<TreeParams>::get_mut`), an inspector, or,
//! with the `ron` or `yaml` and `file_watcher` features, a `.tree.ron` or `.tree.yaml` file saved
//! on disk.
//!
//! With the `ron` feature, [`TreeParamsLoader`] loads `.tree.ron` files, and
//! [`TreeParams::to_ron`] / [`TreeParams::from_ron`] write and read the same format. With the
//! `yaml` feature, [`TreeParamsYamlLoader`] loads `.tree.yaml` files, and
//! [`TreeParams::to_yaml`] / [`TreeParams::from_yaml`] write and read that one: the text
//! `serde_yaml_ng` writes for the type, which is what the Grimoire editor saves. The two features
//! are independent, and a tree is the same tree in either. A file may leave fields out (they take
//! their [`Default`]), and a field the crate does not know is an error that names it, so a typo is
//! reported instead of ignored.

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
/// With the `ron` feature, `asset_server.load("trees/maple.tree.ron")` gives such a handle, and
/// with the `yaml` feature so does `asset_server.load("trees/maple.tree.yaml")`.
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
        // The tree is the asset's as the generator can draw it: a hostile `.tree.ron` is clamped here, so
        // an unrelated reload still finds the component equal to it.
        let Some(params) = assets.get(&source.0).map(TreeParams::sanitized) else { continue };
        match tree {
            // Equal parameters change nothing, so an unrelated reload rebuilds nothing.
            Some(mut tree) => {
                if tree.0 != *params {
                    tree.0 = params.into_owned();
                }
            }
            None => {
                commands.entity(entity).insert(ProceduralTree(params.into_owned()));
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
    #[cfg(feature = "yaml")]
    app.register_asset_loader(TreeParamsYamlLoader);
}

#[cfg(any(feature = "ron", feature = "yaml"))]
pub use file::TreeFileError;
#[cfg(feature = "ron")]
pub use file::TreeParamsLoader;
#[cfg(feature = "yaml")]
pub use file::TreeParamsYamlLoader;

#[cfg(any(feature = "ron", feature = "yaml"))]
mod file {
    use bevy::asset::io::Reader;
    use bevy::asset::{AssetLoader, LoadContext};
    use bevy::reflect::TypePath;

    use crate::params::TreeParams;

    /// Loads [`TreeParams`] from `.tree.ron` files (the `ron` feature), the format
    /// [`TreeParams::to_ron`] writes. Registered by the plugin.
    #[cfg(feature = "ron")]
    #[derive(Clone, Copy, Debug, Default, TypePath)]
    #[type_path = "bevy_flora::tree_asset"]
    pub struct TreeParamsLoader;

    #[cfg(feature = "ron")]
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
            TreeParams::from_ron(&read_text(reader).await?)
        }

        fn extensions(&self) -> &[&str] {
            &["tree.ron"]
        }
    }

    /// Loads [`TreeParams`] from `.tree.yaml` files (the `yaml` feature), the format
    /// [`TreeParams::to_yaml`] writes and the Grimoire editor saves. Registered by the plugin.
    #[cfg(feature = "yaml")]
    #[derive(Clone, Copy, Debug, Default, TypePath)]
    #[type_path = "bevy_flora::tree_asset"]
    pub struct TreeParamsYamlLoader;

    #[cfg(feature = "yaml")]
    impl AssetLoader for TreeParamsYamlLoader {
        type Asset = TreeParams;
        type Settings = ();
        type Error = TreeFileError;

        async fn load(
            &self,
            reader: &mut dyn Reader,
            _settings: &(),
            _load_context: &mut LoadContext<'_>,
        ) -> Result<TreeParams, TreeFileError> {
            TreeParams::from_yaml(&read_text(reader).await?)
        }

        fn extensions(&self) -> &[&str] {
            &["tree.yaml"]
        }
    }

    /// A tree file's text, whole: both loaders read it the same way.
    async fn read_text(reader: &mut dyn Reader) -> Result<String, TreeFileError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.map_err(|error| TreeFileError(Problem::Io(error)))?;
        String::from_utf8(bytes).map_err(|error| TreeFileError(Problem::Utf8(error.utf8_error())))
    }

    /// Why a `.tree.ron` or `.tree.yaml` file could not be read: its message names the line, the
    /// column and, for a misspelt field, the fields that exist.
    #[derive(Debug)]
    pub struct TreeFileError(Problem);

    #[derive(Debug)]
    enum Problem {
        Io(std::io::Error),
        Utf8(std::str::Utf8Error),
        #[cfg(feature = "ron")]
        Ron(ron::error::SpannedError),
        #[cfg(feature = "yaml")]
        Yaml(serde_yaml_ng::Error),
        /// A YAML text with no document in it (nothing, whitespace, comments or `null`).
        #[cfg(feature = "yaml")]
        EmptyYaml,
    }

    impl std::fmt::Display for TreeFileError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.0 {
                Problem::Io(error) => write!(f, "could not read the tree file: {error}"),
                Problem::Utf8(error) => write!(f, "the tree file is not UTF-8: {error}"),
                #[cfg(feature = "ron")]
                Problem::Ron(error) => write!(f, "the tree file is not valid: {error}"),
                #[cfg(feature = "yaml")]
                Problem::Yaml(error) => write!(f, "the tree file is not valid: {error}"),
                #[cfg(feature = "yaml")]
                Problem::EmptyYaml => {
                    write!(f, "the tree file is empty: write `{{}}` for a tree of all defaults")
                }
            }
        }
    }

    impl std::error::Error for TreeFileError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.0 {
                Problem::Io(error) => Some(error),
                Problem::Utf8(error) => Some(error),
                #[cfg(feature = "ron")]
                Problem::Ron(error) => Some(error),
                #[cfg(feature = "yaml")]
                Problem::Yaml(error) => Some(error),
                #[cfg(feature = "yaml")]
                Problem::EmptyYaml => None,
            }
        }
    }

    #[cfg(feature = "ron")]
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

    #[cfg(feature = "yaml")]
    impl TreeParams {
        /// This tree as a `.tree.yaml` file's text, every field written out: export a preset
        /// (`TreePreset::Maple.params().to_yaml()`) and edit it from there. It is
        /// `serde_yaml_ng::to_string` of the type, as the Grimoire editor writes it, so a crown or
        /// trunk style that carries fields is a YAML tag (`crown: !Hull {tiers: 5, ...}`) and
        /// floats are written as the shortest text that reads back as the same `f32`.
        pub fn to_yaml(&self) -> String {
            serde_yaml_ng::to_string(self).expect("every field of TreeParams has a YAML form")
        }

        /// A tree from a `.tree.yaml` file's text. Fields left out take their [`Default`]; a
        /// field this crate does not know is an error that names it and its line. A number the
        /// generator cannot draw (`.nan`, `1e9`) is read as written and held in range where the
        /// tree is built ([`TreeParams::sanitized`]).
        ///
        /// A text with no document in it (nothing, whitespace, comments, `null`) is an error, as
        /// an empty `.tree.ron` is, not a tree of all defaults (`{}` is that): a file watcher can
        /// read a file between an editor's truncate and its write, and the tree must stay as it
        /// was rather than flash to the defaults. More than one document is an error too.
        pub fn from_yaml(text: &str) -> Result<Self, TreeFileError> {
            match serde_yaml_ng::from_str::<Option<Self>>(text) {
                Ok(Some(params)) => Ok(params),
                Ok(None) => Err(TreeFileError(Problem::EmptyYaml)),
                Err(error) => Err(TreeFileError(Problem::Yaml(error))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::prelude::*;

    use crate::{FloraPlugin, ProceduralTree, ProceduralTreeAsset, TreeParams, TreePreset};

    fn app() -> App {
        app_reading("assets".to_string())
    }

    /// An app whose asset server reads the files under `file_path` (relative to the crate, or absolute).
    fn app_reading(file_path: String) -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin { file_path, ..default() }));
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

    #[test]
    fn a_tree_follows_its_asset_as_the_generator_can_draw_it() {
        let mut app = app();
        let mut hostile = TreePreset::Oak.params();
        hostile.trunk.height = f32::NAN;
        hostile.branches.start = -1.0;
        let handle = app.world_mut().resource_mut::<Assets<TreeParams>>().add(hostile.clone());
        let tree = app.world_mut().spawn(ProceduralTreeAsset(handle.clone())).id();
        app.update();
        app.update();
        // The tree holds the parameters in range; the asset (the file's own words) is as it was.
        let held = app.world().get::<ProceduralTree>(tree).unwrap().0.clone();
        assert_eq!(held.trunk.height, TreeParams::default().trunk.height);
        assert_eq!(held.branches.start, 0.0);
        assert_eq!(app.world().resource::<Assets<TreeParams>>().get(&handle).unwrap().branches.start, -1.0);
        // A reload of the same asset leaves the component as it was.
        app.world_mut().resource_mut::<Assets<TreeParams>>().get_mut(&handle).unwrap().branches.start = -1.0;
        app.update();
        app.update();
        assert_eq!(app.world().get::<ProceduralTree>(tree).unwrap().0, held);
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
        let by_hand = TreeParams::from_ron("(crown: Hull(tiers: 4, sides: 6, lift: 0.1, jitter: 0.5, trunk_only: false), trunk_style: Prism(sides: 4))").unwrap();
        assert_eq!(by_hand.crown, crate::CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.5, trunk_only: false });
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

    #[cfg(feature = "yaml")]
    #[test]
    fn every_preset_survives_a_yaml_tree_file() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            assert_eq!(TreeParams::from_yaml(&params.to_yaml()).unwrap(), params, "{preset:?}");
        }
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_hull_crown_and_a_prism_trunk_survive_a_yaml_tree_file_as_tags_and_a_file_may_leave_them_out() {
        let params = crate::crown_hull::hull_spruce();
        let text = params.to_yaml();
        assert!(text.contains("!Hull") && text.contains("!Prism"), "{text}");
        assert_eq!(TreeParams::from_yaml(&text).unwrap(), params);
        let old = TreeParams::from_yaml("seed: 3").unwrap();
        assert_eq!((old.crown, old.trunk_style), (crate::CrownStyle::Leaves, crate::TrunkStyle::Round));
        let by_hand = TreeParams::from_yaml(
            "crown: !Hull {tiers: 4, sides: 6, lift: 0.1, jitter: 0.5, trunk_only: false}\ntrunk_style: !Prism {sides: 4}\n",
        )
        .unwrap();
        assert_eq!(by_hand.crown, crate::CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.5, trunk_only: false });
        assert_eq!(by_hand.trunk_style, crate::TrunkStyle::Prism { sides: 4 });
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_tree_file_may_leave_fields_out_but_not_misspell_them() {
        let partial = TreeParams::from_yaml("seed: 7\ntrunk:\n  height: 9.0\n").unwrap();
        let defaults = TreeParams::default();
        assert_eq!(partial.seed, 7);
        assert_eq!(partial.trunk.height, 9.0);
        assert_eq!(partial.trunk.radius, defaults.trunk.radius);
        assert_eq!(partial.foliage, defaults.foliage);

        let typo = TreeParams::from_yaml("seed: 7\ntrunk:\n  hieght: 9.0\n").unwrap_err().to_string();
        assert!(typo.contains("hieght") && typo.contains("height"), "{typo}");
        assert!(typo.contains("line 3"), "{typo}");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn a_yaml_tree_file_is_read_as_the_generator_can_draw_it() {
        let defaults = TreeParams::default();
        let hostile = TreeParams::from_yaml("trunk:\n  height: .nan\n  radius: 1e9\nbranches:\n  start: -1\n").unwrap();
        assert!(hostile.trunk.height.is_nan(), "the file's own words are kept");
        let held = hostile.sanitized();
        assert_eq!(held.trunk.height, defaults.trunk.height);
        assert_eq!(held.branches.start, 0.0);
        assert!(held.trunk.radius.is_finite() && held.trunk.radius < 1e6, "{}", held.trunk.radius);
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn text_that_is_not_a_tree_is_an_error_at_once() {
        let started = std::time::Instant::now();
        let mut bomb = String::from("trunk: &a0 [0, 0]\n");
        for level in 1..40 {
            bomb.push_str(&format!("l{level}: &a{level} [*a{}, *a{}]\n", level - 1, level - 1));
        }
        let deep = format!("trunk: {}", "[".repeat(10_000));
        for text in [bomb.as_str(), deep.as_str(), "", "---\nseed: 1\n---\nseed: 2\n", "seed: [1, 2]", "\u{0}", "- a\n- b\n"] {
            assert!(TreeParams::from_yaml(text).is_err(), "{:?}", text.chars().take(40).collect::<String>());
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    }

    #[cfg(all(feature = "yaml", feature = "ron"))]
    #[test]
    fn a_tree_is_the_same_tree_in_yaml_and_in_ron() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            assert_eq!(
                TreeParams::from_yaml(&params.to_yaml()).unwrap(),
                TreeParams::from_ron(&params.to_ron()).unwrap(),
                "{preset:?}"
            );
        }
        let ron = include_str!("../examples/assets/trees/maple.tree.ron");
        let yaml = include_str!("../examples/assets/trees/maple.tree.yaml");
        assert_eq!(TreeParams::from_yaml(yaml).unwrap(), TreeParams::from_ron(ron).unwrap());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn the_example_yaml_tree_file_loads() {
        let text = include_str!("../examples/assets/trees/maple.tree.yaml");
        TreeParams::from_yaml(text).unwrap();
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn the_asset_server_loads_a_tree_yaml_file_and_says_where_a_bad_one_is_wrong() {
        use bevy::asset::{AssetServer, LoadState};

        let dir = std::env::temp_dir().join(format!("bevy_flora_tree_yaml_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let oak = TreePreset::Oak.params();
        std::fs::write(dir.join("oak.tree.yaml"), oak.to_yaml()).unwrap();
        std::fs::write(dir.join("typo.tree.yaml"), "seed: 7\ntrunk:\n  hieght: 9.0\n").unwrap();
        #[cfg(feature = "ron")]
        std::fs::write(dir.join("birch.tree.ron"), TreePreset::Birch.params().to_ron()).unwrap();

        let mut app = app_reading(dir.to_string_lossy().into_owned());
        let server = app.world().resource::<AssetServer>().clone();
        let good: Handle<TreeParams> = server.load("oak.tree.yaml");
        let bad: Handle<TreeParams> = server.load("typo.tree.yaml");
        #[cfg(feature = "ron")]
        let ron: Handle<TreeParams> = server.load("birch.tree.ron");
        let tree = app.world_mut().spawn(ProceduralTreeAsset(good.clone())).id();

        let settled = |server: &AssetServer, handle: &Handle<TreeParams>| {
            matches!(server.get_load_state(handle), Some(LoadState::Loaded | LoadState::Failed(_)))
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            app.update();
            #[cfg(feature = "ron")]
            let ron_settled = settled(&server, &ron);
            #[cfg(not(feature = "ron"))]
            let ron_settled = true;
            if settled(&server, &good) && settled(&server, &bad) && ron_settled {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        app.update();
        app.update();

        assert_eq!(app.world().resource::<Assets<TreeParams>>().get(&good), Some(&oak));
        assert_eq!(app.world().get::<ProceduralTree>(tree), Some(&ProceduralTree(oak)));
        let Some(LoadState::Failed(error)) = server.get_load_state(&bad) else {
            panic!("a tree file with a misspelt field loaded: {:?}", server.get_load_state(&bad));
        };
        let message = error.to_string();
        assert!(message.contains("hieght") && message.contains("line 3"), "{message}");
        #[cfg(feature = "ron")]
        assert_eq!(app.world().resource::<Assets<TreeParams>>().get(&ron), Some(&TreePreset::Birch.params()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn saving_a_tree_yaml_file_rebuilds_its_trees_and_a_save_that_does_not_read_leaves_them() {
        use bevy::asset::{AssetServer, LoadState};

        let dir = std::env::temp_dir().join(format!("bevy_flora_tree_yaml_resave_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old_oak.tree.yaml");
        let (oak, birch) = (TreePreset::Oak.params(), TreePreset::Birch.params());
        std::fs::write(&path, oak.to_yaml()).unwrap();

        let mut app = app_reading(dir.to_string_lossy().into_owned());
        let server = app.world().resource::<AssetServer>().clone();
        let handle: Handle<TreeParams> = server.load("old_oak.tree.yaml");
        let trees: Vec<Entity> =
            (0..2).map(|_| app.world_mut().spawn(ProceduralTreeAsset(handle.clone())).id()).collect();
        let drawn = |app: &App, want: &TreeParams| {
            trees.iter().all(|&tree| app.world().get::<ProceduralTree>(tree) == Some(&ProceduralTree(want.clone())))
        };
        let pump_until = |app: &mut App, mut done: Box<dyn FnMut(&App) -> bool + '_>| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while std::time::Instant::now() < deadline {
                app.update();
                if done(app) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("the asset server did not settle: {:?}", server.get_load_state(&handle));
        };

        pump_until(&mut app, Box::new(|app| drawn(app, &oak)));

        // The editor saves another tree over it: every tree of the handle follows.
        std::fs::write(&path, birch.to_yaml()).unwrap();
        server.reload("old_oak.tree.yaml");
        pump_until(&mut app, Box::new(|app| drawn(app, &birch)));

        // A save with a typo and one with nothing in it fail to read and leave the trees as they were.
        for text in ["seed: 7\ntrunk:\n  hieght: 9.0\n", ""] {
            std::fs::write(&path, text).unwrap();
            server.reload("old_oak.tree.yaml");
            pump_until(
                &mut app,
                Box::new(|_| matches!(server.get_load_state(&handle), Some(LoadState::Failed(_)))),
            );
            app.update();
            assert!(drawn(&app, &birch), "a file that does not read changed the trees ({text:?})");

            // The next good save is followed again.
            std::fs::write(&path, oak.to_yaml()).unwrap();
            server.reload("old_oak.tree.yaml");
            pump_until(&mut app, Box::new(|app| drawn(app, &oak)));
            std::fs::write(&path, birch.to_yaml()).unwrap();
            server.reload("old_oak.tree.yaml");
            pump_until(&mut app, Box::new(|app| drawn(app, &birch)));
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn an_empty_yaml_file_is_not_a_default_tree() {
        for text in ["", "  \n\n", "# nothing here yet\n", "null", "~", "---\n"] {
            let error = TreeParams::from_yaml(text).unwrap_err().to_string();
            assert!(error.contains("empty"), "{text:?}: {error}");
        }
        assert_eq!(TreeParams::from_yaml("{}").unwrap(), TreeParams::default());

        // What `to_yaml` writes for a value the generator cannot draw reads back as it was.
        let mut odd = TreeParams::default();
        odd.trunk.height = f32::NAN;
        odd.trunk.radius = f32::INFINITY;
        let back = TreeParams::from_yaml(&odd.to_yaml()).unwrap();
        assert!(back.trunk.height.is_nan() && back.trunk.radius == f32::INFINITY);
    }
}
