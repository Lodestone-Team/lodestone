//! The fixtures in `testdata/options/`: `1.21.11.txt` and `1.20.1.txt` were written by
//! the game (Minecraft 1.21.11, and a vanilla 1.20.1 instance) and are unchanged;
//! `1.8.9.txt` and `1.12.2.txt` are hand-written from those versions' defaults, in the
//! order their `GameSettings.saveOptions` writes them.

use std::{fs, io::Write as _, path::PathBuf};

use super::*;

fn fixture(version: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/options")
        .join(format!("{version}.txt"));
    fs::read_to_string(path).unwrap()
}

/// A data directory with instances.
struct World {
    _dir: tempfile::TempDir,
    data: DataDir,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::at(dir.path());
        Self { _dir: dir, data }
    }

    /// An instance with `options` as its `options.txt` (none if `None`).
    fn instance(&self, id: &str, options: Option<&str>) -> PathBuf {
        let dir = self.data.instances_dir().join(id);
        fs::create_dir_all(dir.join("minecraft")).unwrap();
        fs::write(dir.join("instance.json"), "{}").unwrap();
        if let Some(options) = options {
            fs::write(dir.join("minecraft/options.txt"), options).unwrap();
        }
        dir
    }

    fn set(&self, values: &[(&str, &str)]) {
        update_profile(&self.data, |profile| {
            for (key, value) in values {
                profile.values.insert(key.to_string(), value.to_string());
            }
        })
        .unwrap();
    }

    fn profile(&self) -> Profile {
        load_profile(&self.data).unwrap()
    }

    fn launch(&self, id: &str, version: &str) -> LaunchOutcome {
        let dir = self.data.instances_dir().join(id);
        before_launch(&self.data, id, &dir, version, None).unwrap()
    }

    fn exit(&self, id: &str, version: &str) -> ExitOutcome {
        let dir = self.data.instances_dir().join(id);
        after_exit(&self.data, id, &dir, version).unwrap()
    }
}

fn options_of(dir: &Path) -> String {
    fs::read_to_string(dir.join("minecraft/options.txt")).unwrap()
}

fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.split_once(':').filter(|(k, _)| *k == key))
        .map(|(_, v)| v)
}

/// What the game does while it runs: changes lines of `options.txt`.
fn play(dir: &Path, changes: &[(&str, &str)]) {
    let path = dir.join("minecraft/options.txt");
    let mut file = OptionsFile::read(&path).unwrap();
    for (key, value) in changes {
        file.set(key, value);
    }
    file.write(&path).unwrap();
}

/// The lines whose keys are not shared settings.
fn unshared_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            let key = line.split_once(':').map_or(*line, |(k, _)| k);
            find(key).is_none() && !is_other_binding(key)
        })
        .collect()
}

/// A global set as a 1.21.11 player might have it.
const SHARED: &[(&str, &str)] = &[
    ("mouseSensitivity", "0.8169014084507042"),
    ("invertYMouse", "true"),
    ("key_key.forward", "key.keyboard.up"),
    ("key_key.sneak", "key.keyboard.left.alt"),
    ("key_key.swapOffhand", "key.keyboard.g"),
    ("key_key.attack", "key.mouse.4"),
    ("fov", "0.25"),
    ("simulationDistance", "8"),
    ("graphicsPreset", "fabulous"),
    ("ao", "false"),
    ("soundCategory_music", "0.25"),
    ("soundCategory_ui", "0.5"),
    ("lang", "de_de"),
    ("invertXMouse", "true"),
    ("menuBackgroundBlurriness", "2"),
];

#[test]
fn merge_into_a_real_1_21_11_file() {
    let world = World::new();
    let original = fixture("1.21.11").replace("lastServer:", "lastServer:play.example.net");
    let dir = world.instance("a", Some(&original));
    world.set(SHARED);
    // Excluded keys in the global set (a hand-edited file) are never written.
    world.set(&[
        ("resourcePacks", "[\"file/Evil.zip\"]"),
        ("lastServer", "evil.example.net"),
        ("version", "1"),
        ("unknownKey", "1"),
    ]);
    let outcome = world.launch("a", "1.21.11");
    assert!(outcome.written > 0 && !outcome.created, "{outcome:?}");
    let text = options_of(&dir);
    assert_eq!(value(&text, "mouseSensitivity"), Some("0.8169014084507042"));
    assert_eq!(value(&text, "key_key.forward"), Some("key.keyboard.up"));
    assert_eq!(value(&text, "key_key.attack"), Some("key.mouse.4"));
    assert_eq!(value(&text, "graphicsPreset"), Some("\"fabulous\""));
    assert_eq!(value(&text, "ao"), Some("false"));
    assert_eq!(value(&text, "lang"), Some("de_de"));
    assert_eq!(value(&text, "menuBackgroundBlurriness"), Some("2"));
    assert_eq!(value(&text, "soundCategory_ui"), Some("0.5"));
    // Excluded and unknown keys are untouched.
    assert_eq!(value(&text, "resourcePacks"), Some("[]"));
    assert_eq!(value(&text, "lastServer"), Some("play.example.net"));
    assert_eq!(value(&text, "version"), Some("4671"));
    assert_eq!(value(&text, "unknownKey"), None);
    // The keys of older versions aren't added.
    assert_eq!(value(&text, "graphicsMode"), None);
    assert_eq!(value(&text, "key_key.swapHands"), None);
    // Every other line is kept, in order, and so are the line count and the order of
    // the shared lines.
    assert_eq!(unshared_lines(&text), unshared_lines(&original));
    let keys = |t: &str| {
        t.lines()
            .map(|l| l.split(':').next().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&text), keys(&original));
    assert!(text.ends_with('\n'));
}

#[test]
fn merge_into_1_20_1_uses_its_keys() {
    let world = World::new();
    let original = fixture("1.20.1");
    let dir = world.instance("a", Some(&original));
    world.set(SHARED);
    world.launch("a", "1.20.1");
    let text = options_of(&dir);
    assert_eq!(value(&text, "graphicsMode"), Some("2"));
    assert_eq!(value(&text, "graphicsPreset"), None);
    assert_eq!(value(&text, "key_key.swapOffhand"), Some("key.keyboard.g"));
    assert_eq!(value(&text, "simulationDistance"), Some("8"));
    // 1.20.1 has no horizontal invert, interface volume or menu blur.
    assert_eq!(value(&text, "invertXMouse"), None);
    assert_eq!(value(&text, "soundCategory_ui"), None);
    assert_eq!(value(&text, "menuBackgroundBlurriness"), None);
    assert_eq!(
        value(&text, "resourcePacks"),
        Some("[\"vanilla\",\"fabric\"]")
    );
    assert_eq!(unshared_lines(&text), unshared_lines(&original));
    assert_eq!(text.lines().count(), original.lines().count());
}

#[test]
fn merge_into_1_12_2_converts_to_key_codes() {
    let world = World::new();
    let original = fixture("1.12.2");
    let dir = world.instance("a", Some(&original));
    world.set(SHARED);
    world.launch("a", "1.12.2");
    let text = options_of(&dir);
    assert_eq!(value(&text, "key_key.forward"), Some("200"));
    assert_eq!(value(&text, "key_key.sneak"), Some("56"));
    assert_eq!(value(&text, "key_key.attack"), Some("-97"));
    // swapOffhand was swapHands before 1.16.
    assert_eq!(value(&text, "key_key.swapHands"), Some("34"));
    assert_eq!(value(&text, "key_key.swapOffhand"), None);
    assert_eq!(value(&text, "fancyGraphics"), Some("true"));
    assert_eq!(value(&text, "ao"), Some("0"));
    assert_eq!(value(&text, "lang"), Some("de_de"));
    assert_eq!(value(&text, "invertYMouse"), Some("true"));
    for missing in [
        "simulationDistance",
        "graphicsMode",
        "graphicsPreset",
        "invertXMouse",
        "rawMouseInput",
        "toggleCrouch",
        "soundCategory_ui",
    ] {
        assert_eq!(value(&text, missing), None, "{missing}");
    }
    assert_eq!(unshared_lines(&text), unshared_lines(&original));
    assert_eq!(text.lines().count(), original.lines().count());
}

#[test]
fn merge_into_1_8_9_uses_the_legacy_format() {
    let world = World::new();
    let original = fixture("1.8.9");
    let dir = world.instance("a", Some(&original));
    world.set(SHARED);
    world.launch("a", "1.8.9");
    let text = options_of(&dir);
    assert_eq!(value(&text, "lang"), Some("de_DE"));
    assert_eq!(value(&text, "key_key.forward"), Some("200"));
    assert_eq!(value(&text, "fancyGraphics"), Some("true"));
    assert_eq!(value(&text, "fov"), Some("0.25"));
    // 1.8.9 has no off-hand, auto-jump or voice volume, and no version line.
    for missing in [
        "key_key.swapHands",
        "key_key.swapOffhand",
        "version",
        "soundCategory_voice",
        "autoJump",
    ] {
        assert_eq!(value(&text, missing), None, "{missing}");
    }
    assert_eq!(unshared_lines(&text), unshared_lines(&original));
    assert_eq!(text.lines().count(), original.lines().count());
}

#[test]
fn read_back_from_old_versions_gives_current_names() {
    for (version, fixture_name) in [("1.12.2", "1.12.2"), ("1.8.9", "1.8.9")] {
        let world = World::new();
        let dir = world.instance("old", Some(&fixture(fixture_name)));
        set_preferred(&world.data, Some("old"), None).unwrap();
        world.launch("old", version);
        play(
            &dir,
            &[
                ("key_key.jump", "45"),
                ("key_key.use", "-96"),
                ("fancyGraphics", "false"),
                ("lang", if version == "1.8.9" { "fr_FR" } else { "fr_fr" }),
                ("mouseSensitivity", "0.3"),
            ],
        );
        let outcome = world.exit("old", version);
        assert!(matches!(outcome, ExitOutcome::Emitted(_)), "{outcome:?}");
        let values = world.profile().values;
        assert_eq!(values["key_key.jump"], "key.keyboard.x", "{version}");
        assert_eq!(values["key_key.use"], "key.mouse.5");
        assert_eq!(values["graphicsPreset"], "fast");
        assert_eq!(values["lang"], "fr_fr");
        assert_eq!(values["mouseSensitivity"], "0.3");
        // Unchanged keys the global set didn't have come too, in the current format.
        assert_eq!(values["key_key.forward"], "key.keyboard.w");
        assert_eq!(values["ao"], "true");
        assert!(!values.contains_key("resourcePacks"));
        assert!(!values.contains_key("lastServer"));
        assert!(!values.contains_key("version"));
    }
}

#[test]
fn a_change_in_one_instance_reaches_the_others() {
    let world = World::new();
    let modern = world.instance("modern", Some(&fixture("1.21.11")));
    let legacy = world.instance("legacy", Some(&fixture("1.12.2")));
    let middle = world.instance("middle", Some(&fixture("1.20.1")));
    set_preferred(&world.data, Some("modern"), None).unwrap();
    world.launch("modern", "1.21.11");
    play(
        &modern,
        &[
            ("mouseSensitivity", "0.9"),
            ("key_key.drop", "key.keyboard.z"),
            ("graphicsPreset", "\"fast\""),
        ],
    );
    world.exit("modern", "1.21.11");
    world.launch("legacy", "1.12.2");
    world.launch("middle", "1.20.1");
    let legacy = options_of(&legacy);
    assert_eq!(value(&legacy, "mouseSensitivity"), Some("0.9"));
    assert_eq!(value(&legacy, "key_key.drop"), Some("44"));
    assert_eq!(value(&legacy, "fancyGraphics"), Some("false"));
    let middle = options_of(&middle);
    assert_eq!(value(&middle, "mouseSensitivity"), Some("0.9"));
    assert_eq!(value(&middle, "key_key.drop"), Some("key.keyboard.z"));
    assert_eq!(value(&middle, "graphicsMode"), Some("0"));
}

#[test]
fn only_the_preferred_instance_emits() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.20.1")));
    let b = world.instance("b", Some(&fixture("1.20.1")));
    world.set(&[("mouseSensitivity", "0.5")]);
    set_preferred(&world.data, Some("a"), Some(&a)).unwrap();
    world.launch("a", "1.20.1");
    world.launch("b", "1.20.1");
    play(&a, &[("mouseSensitivity", "0.7")]);
    play(&b, &[("mouseSensitivity", "0.2"), ("fov", "0.5")]);
    // b exits first: nothing reaches the global set.
    let outcome = world.exit("b", "1.20.1");
    assert_eq!(
        outcome,
        ExitOutcome::Changed(vec!["fov".into(), "mouseSensitivity".into()])
    );
    assert_eq!(world.profile().values["mouseSensitivity"], "0.5");
    assert!(!world.profile().values.contains_key("fov"));
    let outcome = world.exit("a", "1.20.1");
    assert!(
        matches!(&outcome, ExitOutcome::Emitted(keys) if keys.contains(&"mouseSensitivity".to_string()))
    );
    assert_eq!(world.profile().values["mouseSensitivity"], "0.7");
    // b keeps its record of what it changed.
    let sharing = load_instance(&b).unwrap();
    assert_eq!(sharing.changed["mouseSensitivity"], "0.2");
    assert_eq!(sharing.session, None);
}

#[test]
fn nothing_is_emitted_without_a_preferred_instance() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.21.11")));
    world.set(&[("mouseSensitivity", "0.5")]);
    world.launch("a", "1.21.11");
    play(&a, &[("mouseSensitivity", "0.1")]);
    assert_eq!(
        world.exit("a", "1.21.11"),
        ExitOutcome::Changed(vec!["mouseSensitivity".into()])
    );
    assert_eq!(world.profile().values.len(), 1);
    assert_eq!(world.profile().values["mouseSensitivity"], "0.5");
}

#[test]
fn overrides_are_never_written_or_emitted() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.20.1")));
    world.set(&[("mouseSensitivity", "0.9"), ("fov", "0.5")]);
    set_preferred(&world.data, Some("a"), Some(&a)).unwrap();
    set_overrides(&a, ["mouseSensitivity"], true).unwrap();
    world.launch("a", "1.20.1");
    let text = options_of(&a);
    assert_eq!(value(&text, "mouseSensitivity"), Some("0.5"), "kept");
    assert_eq!(value(&text, "fov"), Some("0.5"), "merged");
    play(&a, &[("mouseSensitivity", "0.1"), ("fov", "0.75")]);
    let outcome = world.exit("a", "1.20.1");
    let ExitOutcome::Emitted(keys) = outcome else {
        panic!("{outcome:?}");
    };
    assert!(!keys.contains(&"mouseSensitivity".to_string()));
    let values = world.profile().values;
    assert_eq!(values["mouseSensitivity"], "0.9");
    assert_eq!(values["fov"], "0.75");
    // The next launch still leaves the instance's own value.
    world.launch("a", "1.20.1");
    assert_eq!(value(&options_of(&a), "mouseSensitivity"), Some("0.1"));
}

#[test]
fn in_game_changes_become_overrides_or_are_replaced() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.21.11")));
    world.set(&[
        ("mouseSensitivity", "0.5"),
        ("fov", "0.0"),
        ("guiScale", "2"),
    ]);
    world.launch("a", "1.21.11");
    play(&a, &[("mouseSensitivity", "0.3"), ("fov", "0.2")]);
    assert_eq!(
        world.exit("a", "1.21.11"),
        ExitOutcome::Changed(vec!["fov".into(), "mouseSensitivity".into()])
    );
    let sharing = keep_changes_as_overrides(&a).unwrap();
    assert!(sharing.changed.is_empty());
    assert_eq!(
        sharing.overrides.iter().collect::<Vec<_>>(),
        ["fov", "mouseSensitivity"]
    );
    world.launch("a", "1.21.11");
    let text = options_of(&a);
    assert_eq!(value(&text, "mouseSensitivity"), Some("0.3"));
    assert_eq!(value(&text, "fov"), Some("0.2"));
    assert_eq!(value(&text, "guiScale"), Some("2"));

    // Dismissed changes are replaced by the shared values at the next launch.
    play(&a, &[("guiScale", "4")]);
    assert_eq!(
        world.exit("a", "1.21.11"),
        ExitOutcome::Changed(vec!["guiScale".into()])
    );
    dismiss_changes(&a).unwrap();
    world.launch("a", "1.21.11");
    assert_eq!(value(&options_of(&a), "guiScale"), Some("2"));
    // A launch also clears a list nobody answered.
    play(&a, &[("guiScale", "4")]);
    world.exit("a", "1.21.11");
    world.launch("a", "1.21.11");
    assert!(load_instance(&a).unwrap().changed.is_empty());
}

#[test]
fn deleting_the_preferred_instance_clears_the_preference() {
    let world = World::new();
    let store = crate::instances::InstanceStore::new(world.data.instances_dir());
    let instance = store.create("Main", "1.21.11", None).unwrap();
    set_preferred(
        &world.data,
        Some(&instance.id),
        Some(&store.dir(&instance.id)),
    )
    .unwrap();
    assert_eq!(
        world.profile().preferred.as_deref(),
        Some(instance.id.as_str())
    );
    // What the Trash does: the folder goes away.
    fs::remove_dir_all(store.dir(&instance.id)).unwrap();
    assert_eq!(world.profile().preferred, None);
    let raw = fs::read_to_string(profile_path(&world.data)).unwrap();
    assert!(!raw.contains("preferred"), "{raw}");
}

#[test]
fn sharing_off_leaves_the_file_alone() {
    let world = World::new();
    let original = fixture("1.20.1");
    let a = world.instance("a", Some(&original));
    world.set(SHARED);
    set_preferred(&world.data, Some("a"), None).unwrap();
    update_instance(&a, |sharing| sharing.enabled = false).unwrap();
    let outcome = world.launch("a", "1.20.1");
    assert_eq!(outcome.written, 0);
    assert_eq!(options_of(&a), original);
    assert_eq!(load_instance(&a).unwrap().session, None);
    play(&a, &[("mouseSensitivity", "0.1")]);
    assert_eq!(world.exit("a", "1.20.1"), ExitOutcome::Nothing);
    assert_eq!(
        world.profile().values["mouseSensitivity"],
        "0.8169014084507042"
    );
}

#[test]
fn a_category_that_isnt_shared_is_skipped() {
    let world = World::new();
    let original = fixture("1.20.1");
    let a = world.instance("a", Some(&original));
    world.set(SHARED);
    update_profile(&world.data, |p| {
        p.disabled.insert(Category::Controls);
    })
    .unwrap();
    world.launch("a", "1.20.1");
    let text = options_of(&a);
    assert_eq!(value(&text, "mouseSensitivity"), Some("0.5"));
    assert_eq!(value(&text, "key_key.forward"), Some("key.keyboard.w"));
    assert_eq!(value(&text, "fov"), Some("0.25"));
}

#[test]
fn a_missing_file_is_created() {
    // 1.8.9: no data version, so no version line and the legacy format.
    let world = World::new();
    let a = world.instance("a", None);
    world.set(SHARED);
    let outcome = world.launch("a", "1.8.9");
    assert!(outcome.created);
    let text = options_of(&a);
    assert_eq!(value(&text, "version"), None);
    assert_eq!(value(&text, "key_key.forward"), Some("200"));
    assert_eq!(value(&text, "lang"), Some("de_DE"));
    assert_eq!(value(&text, "simulationDistance"), None);
    assert_eq!(value(&text, "key_key.swapHands"), None);

    // 1.12.2: the version line from the table, then key codes.
    let b = world.instance("b", None);
    world.launch("b", "1.12.2");
    let text = options_of(&b);
    assert!(text.starts_with("version:1343\n"), "{text}");
    assert_eq!(value(&text, "key_key.forward"), Some("200"));
    assert_eq!(value(&text, "key_key.swapHands"), Some("34"));

    // 1.20.1: the version line from the client jar, then current names.
    let c = world.instance("c", None);
    let jar = world.data.root().join("1.20.1.jar");
    write_jar(&jar, 3465);
    let outcome = before_launch(&world.data, "c", &c, "1.20.1", Some(&jar)).unwrap();
    assert!(outcome.created);
    let text = options_of(&c);
    assert!(text.starts_with("version:3465\n"), "{text}");
    assert_eq!(value(&text, "key_key.forward"), Some("key.keyboard.up"));
    assert_eq!(value(&text, "graphicsMode"), Some("2"));
    assert_eq!(value(&text, "simulationDistance"), Some("8"));
    assert_eq!(value(&text, "invertXMouse"), None);
    assert_eq!(value(&text, "resourcePacks"), None);

    // Without a jar a 1.20.1 file can't be made safely: the game would read current key
    // names as LWJGL 2 codes. Nothing is written.
    let d = world.instance("d", None);
    let outcome = world.launch("d", "1.20.1");
    assert!(outcome.skipped.is_some());
    assert!(!d.join("minecraft/options.txt").exists());

    // An empty global set doesn't create a file.
    let world = World::new();
    let e = world.instance("e", None);
    assert_eq!(world.launch("e", "1.8.9").written, 0);
    assert!(!e.join("minecraft/options.txt").exists());
}

#[test]
fn a_file_with_only_resource_packs_gets_a_version_line() {
    // Content management creates options.txt with only the pack list.
    let world = World::new();
    let a = world.instance("a", Some("resourcePacks:[\"vanilla\",\"file/Pack.zip\"]\n"));
    world.set(&[("mouseSensitivity", "0.7")]);
    let jar = world.data.root().join("26.1.jar");
    write_jar(&jar, 4786);
    before_launch(&world.data, "a", &a, "26.1", Some(&jar)).unwrap();
    assert_eq!(
        options_of(&a),
        "version:4786\nresourcePacks:[\"vanilla\",\"file/Pack.zip\"]\nmouseSensitivity:0.7\n"
    );
}

#[test]
fn mod_key_bindings_are_shared_only_where_they_exist() {
    let world = World::new();
    let with = format!("{}key_key.sodium.menu:key.keyboard.o\n", fixture("1.20.1"));
    let a = world.instance("a", Some(&with));
    let b = world.instance("b", Some(&fixture("1.20.1")));
    set_preferred(&world.data, Some("a"), None).unwrap();
    world.launch("a", "1.20.1");
    play(&a, &[("key_key.sodium.menu", "key.keyboard.p")]);
    world.exit("a", "1.20.1");
    assert_eq!(
        world.profile().values["key_key.sodium.menu"],
        "key.keyboard.p"
    );
    world.launch("b", "1.20.1");
    assert_eq!(value(&options_of(&b), "key_key.sodium.menu"), None);
    let c = world.instance("c", Some(&with));
    world.launch("c", "1.20.1");
    assert_eq!(
        value(&options_of(&c), "key_key.sodium.menu"),
        Some("key.keyboard.p")
    );
}

#[test]
fn a_new_preferred_instance_fills_the_global_set_on_its_first_exit() {
    let world = World::new();
    let a = world.instance("a", None);
    let jar = world.data.root().join("1.20.1.jar");
    write_jar(&jar, 3465);
    set_preferred(&world.data, Some("a"), Some(&a)).unwrap();
    // Sound isn't shared and the instance overrides its FOV.
    update_profile(&world.data, |profile| {
        profile.disabled.insert(Category::Sound);
    })
    .unwrap();
    set_overrides(&a, ["fov"], true).unwrap();
    let outcome = before_launch(&world.data, "a", &a, "1.20.1", Some(&jar)).unwrap();
    assert_eq!(outcome.written, 0);
    assert!(!a.join("minecraft/options.txt").exists());
    // The game writes its whole file when it first exits.
    fs::write(a.join("minecraft/options.txt"), fixture("1.20.1")).unwrap();
    let outcome = world.exit("a", "1.20.1");
    let ExitOutcome::Emitted(keys) = outcome else {
        panic!("{outcome:?}");
    };
    assert!(keys.contains(&"mouseSensitivity".to_string()), "{keys:?}");
    let values = world.profile().values;
    assert_eq!(values["mouseSensitivity"], "0.5");
    assert_eq!(values["renderDistance"], "12");
    assert_eq!(values["key_key.forward"], "key.keyboard.w");
    assert_eq!(values["key_key.swapOffhand"], "key.keyboard.f");
    assert_eq!(values["graphicsPreset"], "fancy");
    assert!(!values.contains_key("fov"));
    assert!(!values.contains_key("soundCategory_music"));
    assert!(!values.contains_key("resourcePacks"));

    // Another instance's first game adds nothing to the global set.
    let b = world.instance("b", None);
    before_launch(&world.data, "b", &b, "1.20.1", Some(&jar)).unwrap();
    fs::write(
        b.join("minecraft/options.txt"),
        fixture("1.20.1").replace("chatScale:1.0", "chatScale:0.5"),
    )
    .unwrap();
    world.exit("b", "1.20.1");
    assert_eq!(world.profile().values, values);
}

#[test]
fn a_game_the_launcher_didnt_see_exit_is_read_back_at_the_next_launch() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.21.11")));
    set_preferred(&world.data, Some("a"), None).unwrap();
    world.launch("a", "1.21.11");
    play(&a, &[("mouseSensitivity", "0.42")]);
    // The launcher quit; the next launch reads the change back before merging.
    let outcome = world.launch("a", "1.21.11");
    assert!(matches!(outcome.recovered, Some(ExitOutcome::Emitted(_))));
    assert_eq!(world.profile().values["mouseSensitivity"], "0.42");
    assert_eq!(value(&options_of(&a), "mouseSensitivity"), Some("0.42"));
}

#[test]
fn concurrent_changes_are_not_lost() {
    // Threads take the same file locks another launcher process would.
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.21.11")));
    set_preferred(&world.data, Some("a"), Some(&a)).unwrap();
    let root = world.data.root().to_path_buf();
    let threads: Vec<_> = (0..6)
        .map(|thread| {
            let (root, a) = (root.clone(), a.clone());
            std::thread::spawn(move || {
                let data = DataDir::at(&root);
                for i in 0..10 {
                    match thread {
                        0 => {
                            before_launch(&data, "a", &a, "1.21.11", None).unwrap();
                            after_exit(&data, "a", &a, "1.21.11").unwrap();
                        }
                        1 | 2 => {
                            update_instance(&a, |sharing| {
                                sharing.overrides.insert(format!("key_{thread}.{i}"));
                            })
                            .unwrap();
                        }
                        _ => {
                            update_profile(&data, |profile| {
                                let key = format!("key_{thread}.{i}");
                                profile.values.insert(key, "key.keyboard.w".into());
                            })
                            .unwrap();
                        }
                    }
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let profile = world.profile();
    let sharing = load_instance(&a).unwrap();
    for i in 0..10 {
        for thread in [1, 2] {
            assert!(sharing.overrides.contains(&format!("key_{thread}.{i}")));
        }
        for thread in 3..6 {
            assert!(profile.values.contains_key(&format!("key_{thread}.{i}")));
        }
    }
    assert_eq!(profile.preferred.as_deref(), Some("a"));
}

#[test]
fn a_change_waits_for_the_lock() {
    let world = World::new();
    let held = lock(world.data.root()).unwrap();
    let root = world.data.root().to_path_buf();
    let writer = std::thread::spawn(move || {
        set_value(&DataDir::at(&root), "renderDistance", "8").unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    let on_disk = storage::load::<Profile>(&profile_path(&world.data)).unwrap();
    assert!(on_disk.document.values.is_empty());
    drop(held);
    writer.join().unwrap();
    assert_eq!(world.profile().values["renderDistance"], "8");
}

#[test]
fn equal_values_are_not_rewritten() {
    let world = World::new();
    let original = fixture("1.20.1");
    let a = world.instance("a", Some(&original));
    // The same values in another spelling: 1 vs 1.0, and ao already on.
    world.set(&[
        ("soundCategory_master", "1"),
        ("ao", "true"),
        ("key_key.forward", "key.keyboard.w"),
    ]);
    assert_eq!(world.launch("a", "1.20.1").written, 0);
    assert_eq!(options_of(&a), original);
}

#[test]
fn crlf_files_keep_their_line_endings() {
    let world = World::new();
    let original = fixture("1.20.1").replace('\n', "\r\n");
    let a = world.instance("a", Some(&original));
    world.set(&[("mouseSensitivity", "0.7")]);
    world.launch("a", "1.20.1");
    assert_eq!(
        options_of(&a),
        original.replace("mouseSensitivity:0.5\r\n", "mouseSensitivity:0.7\r\n")
    );
}

#[test]
fn lines_that_arent_utf8_are_kept() {
    // A Windows-1252 server name, as 1.12.2 writes it on Windows.
    let world = World::new();
    let a = world.instance("a", None);
    let path = a.join("minecraft/options.txt");
    let original = b"version:1343\nlastServer:caf\xe9.example.net\nmouseSensitivity:0.5\nfov:0.0\n";
    fs::write(&path, original).unwrap();
    world.set(&[("mouseSensitivity", "0.7")]);
    world.launch("a", "1.12.2");
    assert_eq!(
        fs::read(&path).unwrap(),
        b"version:1343\nlastServer:caf\xe9.example.net\nmouseSensitivity:0.7\nfov:0.0\n"
    );
    set_overrides(&a, ["fov"], true).unwrap();
    set_instance_value(&a, "1.12.2", None, "fov", "0.25").unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        b"version:1343\nlastServer:caf\xe9.example.net\nmouseSensitivity:0.7\nfov:0.25\n"
    );
}

#[test]
fn copy_from_instance_and_reset() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.12.2")));
    let count = copy_from_instance(&world.data, &a, "1.12.2", None).unwrap();
    let values = world.profile().values;
    assert_eq!(values.len(), count);
    assert_eq!(values["key_key.forward"], "key.keyboard.w");
    assert_eq!(values["key_key.swapOffhand"], "key.keyboard.f");
    assert_eq!(values["graphicsPreset"], "fancy");
    assert_eq!(values["ao"], "true");
    assert_eq!(values["lang"], "en_us");
    for excluded in keys::EXCLUDED {
        assert!(!values.contains_key(*excluded), "{excluded}");
    }
    assert!(!values.contains_key("mainHand"));
    let empty = world.instance("empty", None);
    assert!(copy_from_instance(&world.data, &empty, "1.20.1", None).is_err());
    reset(&world.data).unwrap();
    assert!(world.profile().values.is_empty());
}

#[test]
fn editing_values() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.12.2")));
    set_value(&world.data, "key_key.jump", "J").unwrap();
    assert_eq!(world.profile().values["key_key.jump"], "key.keyboard.j");
    assert!(set_value(&world.data, "fov", "wide").is_err());
    assert!(set_value(&world.data, "resourcePacks", "[]").is_err());
    set_instance_value(&a, "1.12.2", None, "key_key.jump", "key.keyboard.k").unwrap();
    assert_eq!(value(&options_of(&a), "key_key.jump"), Some("37"));
    assert!(set_instance_value(&a, "1.12.2", None, "simulationDistance", "6").is_err());
}

#[test]
fn rows_for_the_settings_pages() {
    let world = World::new();
    let a = world.instance("a", Some(&fixture("1.20.1")));
    world.set(&[
        ("mouseSensitivity", "0.7"),
        ("key_key.mod.thing", "key.keyboard.k"),
    ]);
    set_overrides(&a, ["fov"], true).unwrap();
    let profile = world.profile();
    let global = rows(&profile, None);
    assert_eq!(global.len(), 2);
    let (file, target) = read_instance_options(&a, "1.20.1", None).unwrap();
    let local = instance_values(&file, target.unwrap().dv);
    let sharing = load_instance(&a).unwrap();
    let rows = rows(&profile, Some((&sharing, &local)));
    let fov = rows.iter().find(|r| r.key == "fov").unwrap();
    assert!(fov.overridden);
    assert_eq!(fov.local.as_deref(), Some("0.0"));
    assert_eq!(fov.show("0.0"), "70°");
    let sensitivity = rows.iter().find(|r| r.key == "mouseSensitivity").unwrap();
    assert_eq!(sensitivity.shared.as_deref(), Some("0.7"));
    assert!(!sensitivity.overridden);
    assert!(rows.iter().any(|r| r.key == "key_key.mod.thing"));
    assert!(
        rows.iter()
            .all(|r| !keys::EXCLUDED.contains(&r.key.as_str()))
    );
}

#[test]
fn the_documents_round_trip_unknown_keys() {
    let world = World::new();
    fs::create_dir_all(world.data.root()).unwrap();
    fs::write(
        profile_path(&world.data),
        r#"{"version": 1, "values": {"fov": "0.5"}, "later": [1]}"#,
    )
    .unwrap();
    update_profile(&world.data, |p| {
        p.disabled.insert(Category::Sound);
    })
    .unwrap();
    let raw: Value = serde_json::from_slice(&fs::read(profile_path(&world.data)).unwrap()).unwrap();
    assert_eq!(
        raw,
        serde_json::json!({"version": 1, "values": {"fov": "0.5"}, "disabled": ["sound"], "later": [1]})
    );
    // An instance without the file shares.
    let a = world.instance("a", None);
    assert!(load_instance(&a).unwrap().enabled);
}

#[test]
fn the_data_version_comes_from_the_jar() {
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("client.jar");
    write_jar(&jar, 4671);
    assert_eq!(jar_data_version(&jar), Some(4671));
    assert_eq!(jar_data_version(&dir.path().join("missing.jar")), None);
}

fn write_jar(path: &Path, world_version: u32) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("version.json", zip::write::FileOptions::default())
        .unwrap();
    write!(zip, "{{\"id\": \"x\", \"world_version\": {world_version}}}").unwrap();
    zip.finish().unwrap();
}

#[test]
fn keeping_the_narrator_off_reaches_a_new_instance() {
    let world = World::new();
    assert!(!narrator_off(&world.profile()));
    let profile = set_narrator_off(&world.data, true).unwrap();
    assert!(narrator_off(&profile));

    // A brand-new instance: no options.txt yet, so its first start would open the
    // accessibility screen and read it aloud.
    let a = world.instance("a", None);
    let jar = world.data.root().join("1.21.11.jar");
    write_jar(&jar, 4671);
    let outcome = before_launch(&world.data, "a", &a, "1.21.11", Some(&jar)).unwrap();
    assert!(outcome.created);
    let text = options_of(&a);
    assert_eq!(value(&text, "narrator"), Some("0"));
    assert_eq!(value(&text, "onboardAccessibility"), Some("false"));

    // An existing instance that had the narrator on gets it turned off.
    let b = world.instance(
        "b",
        Some(&fixture("1.21.11").replace("narrator:0", "narrator:1")),
    );
    world.launch("b", "1.21.11");
    assert_eq!(value(&options_of(&b), "narrator"), Some("0"));

    // Turning it off again stops sharing both values.
    let profile = set_narrator_off(&world.data, false).unwrap();
    assert!(!narrator_off(&profile));
    assert!(!profile.values.contains_key("narrator"));
    assert!(!profile.values.contains_key("onboardAccessibility"));
}

#[test]
fn keeping_the_narrator_off_does_nothing_where_accessibility_isnt_shared() {
    let world = World::new();
    set_narrator_off(&world.data, true).unwrap();
    update_profile(&world.data, |profile| {
        profile.disabled.insert(Category::Accessibility);
    })
    .unwrap();
    let a = world.instance(
        "a",
        Some(&fixture("1.21.11").replace("narrator:0", "narrator:1")),
    );
    world.launch("a", "1.21.11");
    assert_eq!(value(&options_of(&a), "narrator"), Some("1"));
}
