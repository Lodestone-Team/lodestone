use std::path::PathBuf;

use super::*;
use crate::{instances::Loader, paths::DataDir};

const GIB: u64 = 1 << 30;

fn pc(ram_gib: u64, threads: u32, gpu: Option<(GpuKind, u64)>) -> Hardware {
    Hardware {
        total_memory: Some(ram_gib * GIB),
        threads,
        gpu: gpu.map(|(kind, gib)| Gpu {
            name: String::new(),
            kind,
            memory: Some(gib * GIB),
        }),
    }
}

fn gaming_pc() -> Hardware {
    pc(32, 16, Some((GpuKind::Discrete, 12)))
}

fn laptop() -> Hardware {
    pc(8, 8, Some((GpuKind::Integrated, 1)))
}

fn mods(count: u32, mb: u64) -> ModLoad {
    ModLoad {
        count,
        bytes: mb * 1024 * 1024,
        ..ModLoad::default()
    }
}

#[test]
fn memory_grows_with_the_mods() {
    let hw = gaming_pc();
    let plan = |load: &ModLoad| recommend_memory(&hw, load, true, true, None).memory.max;
    // A few performance mods.
    assert_eq!(plan(&mods(30, 60)), 3072);
    // A mid-size pack.
    assert_eq!(plan(&mods(150, 400)), 5120);
    // A kitchen-sink pack.
    assert_eq!(plan(&mods(400, 1200)), 9728);
    // Never past the useful maximum.
    assert_eq!(plan(&mods(900, 3000)), MAX_RECOMMENDED_MB);
    // Distant Horizons and shaders want more.
    let mut dh = mods(30, 60);
    dh.distant_horizons = true;
    dh.shaders = true;
    assert_eq!(plan(&dh), 4096);
    // Vanilla.
    assert_eq!(
        recommend_memory(&hw, &ModLoad::default(), false, true, None)
            .memory
            .max,
        3072
    );
    assert_eq!(
        recommend_memory(&laptop(), &ModLoad::default(), false, true, None)
            .memory
            .max,
        2048
    );
    assert_eq!(
        recommend_memory(&hw, &ModLoad::default(), false, false, None)
            .memory
            .max,
        2048
    );
}

#[test]
fn memory_leaves_room_for_the_system() {
    let big = mods(400, 1200);
    // 8 GB: 2 GB stays for the system.
    let plan = recommend_memory(&laptop(), &big, true, true, None);
    assert_eq!(plan.memory.max, 6144);
    assert!(plan.limited());
    assert_eq!(plan.wanted, 9728);
    // 16 GB: a quarter stays.
    let plan = recommend_memory(&pc(16, 8, None), &big, true, true, None);
    assert_eq!(plan.memory.max, 9728);
    assert!(!plan.limited());
    // 4 GB.
    let plan = recommend_memory(&pc(4, 4, None), &big, true, true, None);
    assert_eq!(plan.memory.max, 2048);
    // Unknown memory: the launcher's default.
    let unknown = Hardware {
        total_memory: None,
        threads: 8,
        gpu: None,
    };
    assert_eq!(
        recommend_memory(&unknown, &big, true, true, None)
            .memory
            .max,
        4096
    );
    // The heap starts at half.
    assert_eq!(
        recommend_memory(&laptop(), &big, true, true, None)
            .memory
            .min,
        3072
    );
}

#[test]
fn jvm_arguments_depend_on_the_heap() {
    let small = recommend_jvm_args(3072);
    assert!(small.contains("-XX:+UseG1GC"));
    assert!(small.contains("-XX:+UnlockExperimentalVMOptions"));
    assert!(!small.contains("G1HeapRegionSize"));
    // The experimental flags come after they are unlocked.
    let unlock = small.find("UnlockExperimentalVMOptions").unwrap();
    assert!(small.find("G1NewSizePercent").unwrap() > unlock);
    assert!(small.find("G1MixedGCLiveThresholdPercent").unwrap() > unlock);
    assert!(recommend_jvm_args(6144).contains("-XX:G1HeapRegionSize=8M"));
    let large = recommend_jvm_args(12288);
    assert!(large.contains("-XX:G1HeapRegionSize=16M"));
    assert!(large.contains("-XX:G1NewSizePercent=40"));
    // They are words `split_args` keeps apart.
    assert!(
        crate::game::launch::split_args(&large)
            .iter()
            .all(|a| a.starts_with("-XX:"))
    );
}

#[test]
fn video_follows_the_computer_and_mods() {
    let get = |map: &BTreeMap<String, String>, key: &str| map[key].clone();
    let low = recommend_video(Tier::Low, &ModLoad::default(), false);
    assert_eq!(get(&low, "renderDistance"), "8");
    assert_eq!(get(&low, "graphicsPreset"), "fast");
    let ultra = recommend_video(Tier::Ultra, &ModLoad::default(), false);
    assert_eq!(get(&ultra, "renderDistance"), "20");
    assert_eq!(get(&ultra, "graphicsPreset"), "fancy");
    // A heavy pack without Sodium draws closer.
    let heavy = recommend_video(Tier::High, &mods(200, 500), true);
    assert_eq!(get(&heavy, "renderDistance"), "14");
    assert_eq!(get(&heavy, "simulationDistance"), "6");
    // With Sodium, further.
    let mut fast = mods(40, 50);
    fast.renderer = true;
    assert_eq!(
        get(&recommend_video(Tier::High, &fast, true), "renderDistance"),
        "20"
    );
    // Distant Horizons draws the far terrain.
    fast.distant_horizons = true;
    assert_eq!(
        get(&recommend_video(Tier::Ultra, &fast, true), "renderDistance"),
        "12"
    );
    // The simulation distance never passes the render distance.
    for tier in [Tier::Low, Tier::Medium, Tier::High, Tier::Ultra] {
        let v = recommend_video(tier, &mods(300, 900), true);
        let r: u32 = v["renderDistance"].parse().unwrap();
        let s: u32 = v["simulationDistance"].parse().unwrap();
        assert!(s <= r && r >= 6 && s >= 5, "{tier:?}: {r} {s}");
    }
}

#[test]
fn modern_versions() {
    assert!(is_modern("1.18"));
    assert!(is_modern("1.21.11"));
    assert!(is_modern("26.1"));
    assert!(is_modern("24w14a"));
    assert!(!is_modern("1.12.2"));
    assert!(!is_modern("1.7.10"));
    assert!(!is_modern("b1.7.3"));
    assert!(!is_modern("rd-132211"));
}

#[test]
fn scans_the_mods_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mods_dir = dir.path().join("mods");
    std::fs::create_dir_all(mods_dir.join("folder.jar")).unwrap();
    for (name, size) in [
        ("sodium-fabric-0.5.11+mc1.21.jar", 1000),
        ("iris-fabric-1.7.0+mc1.21.jar", 2000),
        ("DistantHorizons-2.1.2-a-1.20.4.jar", 3000),
        ("create-1.20.1-0.5.1.f.jar", 4000),
        ("disabled-mod.jar.disabled", 5000),
        ("notes.txt", 6000),
    ] {
        std::fs::write(mods_dir.join(name), vec![0u8; size]).unwrap();
    }
    let load = scan_mods(dir.path());
    assert_eq!(load.count, 4);
    assert_eq!(load.bytes, 10_000);
    assert!(load.renderer && load.shaders && load.distant_horizons);
    assert_eq!(scan_mods(&dir.path().join("missing")), ModLoad::default());
}

fn new_instance(loader: Loader, version: &str) -> Instance {
    Instance {
        id: "pack".into(),
        name: "Pack".into(),
        game_version: version.into(),
        loader,
        ..Instance::default()
    }
}

fn game_with_mods(count: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mods_dir = dir.path().join("mods");
    std::fs::create_dir_all(&mods_dir).unwrap();
    for i in 0..count {
        std::fs::write(mods_dir.join(format!("mod{i}.jar")), b"x").unwrap();
    }
    dir
}

#[test]
fn a_new_modded_instance_gets_memory_and_arguments() {
    let game = game_with_mods(200);
    let mut instance = new_instance(Loader::Fabric, "1.20.1");
    let outcome = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert!(outcome.changed);
    let memory = instance.memory.unwrap();
    assert_eq!(memory.max, 4096);
    assert_eq!(outcome.memory, Some(memory));
    assert!(outcome.jvm_args);
    assert_eq!(
        instance.jvm_args.as_deref(),
        Some(recommend_jvm_args(4096).as_str())
    );
    let state = instance.auto_config.clone().unwrap();
    assert_eq!(state.memory, Some(memory));
    assert!(!state.options);

    // The same launch again changes nothing.
    let again = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(again, LaunchOutcome::default());

    // More mods: the memory follows, and the arguments with it.
    for i in 200..500 {
        std::fs::write(game.path().join("mods").join(format!("mod{i}.jar")), b"x").unwrap();
    }
    let more = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(instance.memory.unwrap().max, 7168);
    assert_eq!(more.memory, instance.memory);
    // 4 and 7 GB get the same flags.
    assert!(!more.jvm_args);
    for i in 500..1200 {
        std::fs::write(game.path().join("mods").join(format!("mod{i}.jar")), b"x").unwrap();
    }
    let huge = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(instance.memory.unwrap().max, MAX_RECOMMENDED_MB);
    assert!(huge.jvm_args, "a 12 GB heap gets bigger regions");
    assert!(
        instance
            .jvm_args
            .as_deref()
            .unwrap()
            .contains("G1HeapRegionSize=16M")
    );

    // Once the user changes the memory, it is theirs.
    instance.memory = Some(Memory {
        min: 1024,
        max: 2048,
    });
    let theirs = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(
        instance.memory,
        Some(Memory {
            min: 1024,
            max: 2048
        })
    );
    assert_eq!(theirs.memory, None);
    assert_eq!(theirs.low_memory, Some((2048, MAX_RECOMMENDED_MB)));
}

#[test]
fn a_vanilla_instance_gets_memory_but_keeps_mojangs_arguments() {
    let game = tempfile::tempdir().unwrap();
    let mut instance = new_instance(Loader::None, "1.21.4");
    let outcome = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(
        instance.memory,
        Some(Memory {
            min: 1536,
            max: 3072
        })
    );
    assert!(!outcome.jvm_args);
    assert_eq!(instance.jvm_args, None);
}

#[test]
fn global_settings_and_played_instances_are_kept() {
    let game = game_with_mods(300);
    // A global maximum the user chose.
    let defaults = InstanceDefaults {
        max_ram: Some(3072),
        jvm_args: "-XX:+UseZGC".into(),
        ..InstanceDefaults::default()
    };
    let mut instance = new_instance(Loader::Forge, "1.20.1");
    let outcome = configure_launch(&mut instance, &defaults, &gaming_pc(), game.path());
    assert_eq!(instance.memory, None);
    assert_eq!(instance.jvm_args, None);
    assert!(outcome.low_memory.is_some());
    // Still set up, for its options.
    assert!(instance.auto_config.is_some());

    // An instance played before automatic setup existed.
    let mut old = new_instance(Loader::Forge, "1.20.1");
    old.last_played = Some(1);
    let outcome = configure_launch(
        &mut old,
        &InstanceDefaults::default(),
        &gaming_pc(),
        game.path(),
    );
    assert_eq!(outcome, LaunchOutcome::default());
    assert_eq!(old.memory, None);
    assert_eq!(old.auto_config, None);
}

#[test]
fn a_small_computer_is_noted() {
    let game = game_with_mods(500);
    let mut instance = new_instance(Loader::NeoForge, "1.21.1");
    let outcome = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &laptop(),
        game.path(),
    );
    assert_eq!(instance.memory.unwrap().max, 6144);
    assert_eq!(outcome.limited, Some((6144, 7168)));
}

struct Data {
    _dir: tempfile::TempDir,
    data: DataDir,
}

impl Data {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::at(dir.path());
        Self { _dir: dir, data }
    }

    fn instance_dir(&self, id: &str, options: Option<&str>) -> PathBuf {
        let dir = self.data.instances_dir().join(id);
        std::fs::create_dir_all(dir.join("minecraft")).unwrap();
        std::fs::write(dir.join("instance.json"), "{}").unwrap();
        if let Some(text) = options {
            std::fs::write(dir.join("minecraft").join("options.txt"), text).unwrap();
        }
        dir
    }

    fn options(&self, id: &str) -> String {
        std::fs::read_to_string(
            self.data
                .instances_dir()
                .join(id)
                .join("minecraft")
                .join("options.txt"),
        )
        .unwrap_or_default()
    }
}

fn line<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
}

fn set_up(instance: &mut Instance) {
    instance.auto_config = Some(Default::default());
}

#[test]
fn a_new_instance_gets_video_settings_and_the_last_instances_controls() {
    let data = Data::new();
    // An instance played on 1.21.11, with its own sensitivity, keys and language.
    let old = data.instance_dir(
        "survival",
        Some(
            "version:4671\nmouseSensitivity:0.25\nkey_key.jump:key.keyboard.f\nlang:fr_fr\nrenderDistance:32\n",
        ),
    );
    let mut old_instance = new_instance(Loader::None, "1.21.11");
    old_instance.id = "survival".into();
    old_instance.name = "Survival".into();
    old_instance.last_played = Some(10);

    let dir = data.instance_dir("pack", None);
    let mut instance = new_instance(Loader::None, "1.12.2");
    set_up(&mut instance);
    let candidates = vec![Candidate {
        source: ControlsSource::Instance("Survival".into()),
        game_dir: old.join("minecraft"),
        game_version: "1.21.11".into(),
    }];
    let outcome = configure_options(
        &data.data,
        &mut instance,
        &dir,
        None,
        &laptop(),
        &candidates,
    )
    .unwrap();
    assert!(outcome.changed);
    assert!(instance.auto_config.as_ref().unwrap().options);
    assert_eq!(outcome.video, Some(Tier::Low));
    assert_eq!(outcome.render_distance, Some(8));
    assert_eq!(
        outcome.controls_from,
        Some(ControlsSource::Instance("Survival".into()))
    );
    let text = data.options("pack");
    // In 1.12.2's format: its data version, fancyGraphics and key codes.
    assert_eq!(line(&text, "version"), Some("1343"));
    assert_eq!(line(&text, "renderDistance"), Some("8"));
    assert_eq!(line(&text, "fancyGraphics"), Some("false"));
    assert_eq!(line(&text, "simulationDistance"), None, "1.12.2 has none");
    assert_eq!(line(&text, "mouseSensitivity"), Some("0.25"));
    assert_eq!(line(&text, "key_key.jump"), Some("33"), "F's key code");
    assert_eq!(line(&text, "lang"), Some("fr_fr"));
    // The other instance's video settings aren't copied.
    assert_ne!(line(&text, "renderDistance"), Some("32"));

    // Only once.
    std::fs::remove_file(dir.join("minecraft").join("options.txt")).unwrap();
    let again = configure_options(
        &data.data,
        &mut instance,
        &dir,
        None,
        &laptop(),
        &candidates,
    )
    .unwrap();
    assert_eq!(again, OptionsOutcome::default());
    assert_eq!(data.options("pack"), "");
}

#[test]
fn a_modpacks_own_options_are_kept() {
    let data = Data::new();
    let dir = data.instance_dir("pack", Some("renderDistance:6\n"));
    let mut instance = new_instance(Loader::Forge, "1.12.2");
    set_up(&mut instance);
    let outcome =
        configure_options(&data.data, &mut instance, &dir, None, &gaming_pc(), &[]).unwrap();
    assert!(outcome.changed);
    assert_eq!(outcome.written, 0);
    assert_eq!(data.options("pack"), "renderDistance:6\n");
    assert!(instance.auto_config.unwrap().options);
}

#[test]
fn shared_settings_win() {
    let data = Data::new();
    // The shared set has controls and a render distance.
    crate::shared_options::update_profile(&data.data, |profile| {
        profile
            .values
            .insert("mouseSensitivity".into(), "0.75".into());
        profile.values.insert("lang".into(), "de_de".into());
        profile.values.insert("renderDistance".into(), "10".into());
    })
    .unwrap();
    let old = data.instance_dir("old", Some("version:1343\nmouseSensitivity:0.1\n"));
    let dir = data.instance_dir("pack", None);
    let mut instance = new_instance(Loader::None, "1.12.2");
    set_up(&mut instance);
    let candidates = vec![Candidate {
        source: ControlsSource::Instance("Old".into()),
        game_dir: old.join("minecraft"),
        game_version: "1.12.2".into(),
    }];
    let outcome = configure_options(
        &data.data,
        &mut instance,
        &dir,
        None,
        &gaming_pc(),
        &candidates,
    )
    .unwrap();
    assert_eq!(outcome.controls_from, None, "sharing covers the controls");
    assert_eq!(
        outcome.render_distance, None,
        "sharing sets the render distance"
    );
    assert!(outcome.video.is_some());
    let text = data.options("pack");
    assert_eq!(line(&text, "mouseSensitivity"), None);
    assert_eq!(line(&text, "renderDistance"), None);
    assert_eq!(line(&text, "fancyGraphics"), Some("true"));

    // An instance that doesn't use sharing gets the copied controls.
    let other = data.instance_dir("solo", None);
    crate::shared_options::update_instance(&other, |s| s.enabled = false).unwrap();
    let mut solo = new_instance(Loader::None, "1.12.2");
    solo.id = "solo".into();
    set_up(&mut solo);
    let outcome = configure_options(
        &data.data,
        &mut solo,
        &other,
        None,
        &gaming_pc(),
        &candidates,
    )
    .unwrap();
    assert_eq!(
        outcome.controls_from,
        Some(ControlsSource::Instance("Old".into()))
    );
    assert_eq!(line(&data.options("solo"), "mouseSensitivity"), Some("0.1"));
}

#[test]
fn not_set_up_means_untouched() {
    let data = Data::new();
    let dir = data.instance_dir("pack", None);
    let mut instance = new_instance(Loader::None, "1.12.2");
    let outcome =
        configure_options(&data.data, &mut instance, &dir, None, &gaming_pc(), &[]).unwrap();
    assert_eq!(outcome, OptionsOutcome::default());
    assert_eq!(data.options("pack"), "");
}

#[test]
fn candidates_put_the_main_instance_first() {
    let make = |id: &str, played: Option<u64>| {
        let mut i = new_instance(Loader::None, "1.21.1");
        i.id = id.into();
        i.name = id.to_uppercase();
        i.last_played = played;
        (i, PathBuf::from(id))
    };
    let list = vec![
        make("a", Some(5)),
        make("b", Some(9)),
        make("main", Some(1)),
        make("never", None),
        make("this", Some(20)),
    ];
    let prism = ControlsSource::OtherLauncher {
        launcher: "Prism Launcher".into(),
        instance: "Pack".into(),
    };
    let outside = |source: ControlsSource, played: u64| Outside {
        candidate: Candidate {
            source,
            game_dir: PathBuf::from("outside"),
            game_version: String::new(),
        },
        played,
    };
    let names: Vec<ControlsSource> = candidates(
        Some("main"),
        &list,
        "this",
        vec![
            outside(ControlsSource::MinecraftLauncher, 2),
            outside(prism.clone(), 7),
        ],
    )
    .into_iter()
    .map(|c| c.source)
    .collect();
    // The main instance, then by when each was played, here or elsewhere.
    let instance = |n: &str| ControlsSource::Instance(n.into());
    assert_eq!(
        names,
        [
            instance("MAIN"),
            instance("B"),
            prism,
            instance("A"),
            ControlsSource::MinecraftLauncher,
        ]
    );
}

/// The first instance on a computer with only other launchers' games: the one played
/// last in another launcher gives its sensitivity.
#[test]
fn another_launchers_last_game_gives_the_controls() {
    use crate::{game::platform::Os, import::Locations};
    use std::time::{Duration, SystemTime};

    let home = tempfile::tempdir().unwrap();
    let roaming = home.path().join("AppData").join("Roaming");
    let prism = roaming.join("PrismLauncher");
    std::fs::create_dir_all(&prism).unwrap();
    std::fs::write(prism.join("prismlauncher.cfg"), "[General]\n").unwrap();
    let at = |secs: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
    let prism_instance = |folder: &str, name: &str, options: Option<(&str, u64)>| {
        let dir = prism.join("instances").join(folder);
        std::fs::create_dir_all(dir.join("minecraft")).unwrap();
        std::fs::write(
            dir.join("instance.cfg"),
            format!("[General]\nname={name}\nMinecraftVersion=1.21.1\n"),
        )
        .unwrap();
        if let Some((text, played)) = options {
            let path = dir.join("minecraft").join("options.txt");
            std::fs::write(&path, text).unwrap();
            let file = std::fs::File::options().write(true).open(&path).unwrap();
            file.set_modified(at(played)).unwrap();
        }
    };
    prism_instance(
        "ftb",
        "FTB Evolution 2",
        Some(("version:3955\nmouseSensitivity:0.2558\n", 2_000_000)),
    );
    prism_instance(
        "old",
        "Old Pack",
        Some(("version:3955\nmouseSensitivity:0.5\n", 1_000_000)),
    );
    prism_instance("fresh", "Never Played", None);
    // The official launcher's folder, played longer ago.
    let vanilla = roaming.join(".minecraft");
    std::fs::create_dir_all(&vanilla).unwrap();
    std::fs::write(
        vanilla.join("options.txt"),
        "version:3955\nmouseSensitivity:0.9\n",
    )
    .unwrap();
    std::fs::File::options()
        .write(true)
        .open(vanilla.join("options.txt"))
        .unwrap()
        .set_modified(at(500_000))
        .unwrap();

    let outside = outside_candidates_in(&Locations {
        os: Os::Windows,
        home: home.path().to_path_buf(),
        app_data: roaming.clone(),
        config: roaming,
        documents: home.path().join("Documents"),
        applications: Vec::new(),
    });
    let mut found: Vec<(ControlsSource, u64)> = outside
        .iter()
        .map(|o| (o.candidate.source.clone(), o.played))
        .collect();
    found.sort_by_key(|(_, played)| *played);
    let ftb = ControlsSource::OtherLauncher {
        launcher: "Prism Launcher".into(),
        instance: "FTB Evolution 2".into(),
    };
    assert_eq!(
        found,
        [
            (ControlsSource::MinecraftLauncher, 500_000),
            (
                ControlsSource::OtherLauncher {
                    launcher: "Prism Launcher".into(),
                    instance: "Old Pack".into(),
                },
                1_000_000
            ),
            (ftb.clone(), 2_000_000),
        ],
        "a game never played has no options to copy"
    );

    let data = Data::new();
    let dir = data.instance_dir("first", None);
    // 1.12.2, whose format is known without its client jar.
    let mut instance = new_instance(Loader::None, "1.12.2");
    set_up(&mut instance);
    let list = candidates(None, &[], "first", outside);
    let outcome =
        configure_options(&data.data, &mut instance, &dir, None, &gaming_pc(), &list).unwrap();
    assert_eq!(outcome.controls_from, Some(ftb));
    assert_eq!(
        line(&data.options("first"), "mouseSensitivity"),
        Some("0.2558")
    );
}

#[test]
fn a_packs_recommendation_is_a_floor() {
    let few = mods(30, 60);
    let max = |hw: &Hardware, pack| recommend_memory(hw, &few, true, true, pack).memory.max;
    // More than the estimate: the pack's.
    assert_eq!(max(&gaming_pc(), Some(8192)), 8192);
    // Less than the estimate: the estimate.
    assert_eq!(max(&gaming_pc(), Some(2048)), 3072);
    // Rounded up to the next 512 MiB.
    assert_eq!(max(&gaming_pc(), Some(6000)), 6144);
    // Past the usual maximum, when the computer has room.
    assert_eq!(max(&gaming_pc(), Some(16384)), 16384);
    // Never past what the computer can spare.
    let plan = recommend_memory(&pc(16, 8, None), &few, true, true, Some(16384));
    assert_eq!(plan.memory.max, 12288);
    assert!(plan.limited());
    let plan = recommend_memory(&laptop(), &few, true, true, Some(8192));
    assert_eq!((plan.memory.max, plan.wanted), (6144, 8192));
}

#[test]
fn a_new_modpack_instance_gets_the_packs_memory() {
    let game = game_with_mods(40);
    let mut instance = new_instance(Loader::Forge, "1.20.1");
    instance.recommended_memory = Some(8192);
    let outcome = configure_launch(
        &mut instance,
        &InstanceDefaults::default(),
        &pc(16, 8, Some((GpuKind::Discrete, 8))),
        game.path(),
    );
    assert_eq!(
        instance.memory,
        Some(Memory {
            min: 4096,
            max: 8192
        })
    );
    assert_eq!(outcome.limited, None);
    assert_eq!(
        instance.jvm_args.as_deref(),
        Some(recommend_jvm_args(8192).as_str())
    );
}
