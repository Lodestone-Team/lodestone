use std::{fs, io::Write, path::Path};

use super::*;
use crate::game::log4j::Stream;

/// Writes a jar with these files (empty) and a `fabric.mod.json` for `id` and `name`.
fn jar(path: &Path, id: &str, name: &str, files: &[&str]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::FileOptions::default();
    zip.start_file("fabric.mod.json", options).unwrap();
    write!(
        zip,
        r#"{{"schemaVersion": 1, "id": "{id}", "name": "{name}", "version": "1.0"}}"#
    )
    .unwrap();
    for file in files {
        zip.start_file(*file, options).unwrap();
    }
    zip.finish().unwrap();
}

/// A jar that bundles `inner` (with `files`) in `META-INF/jars/`.
fn jar_with_nested(path: &Path, id: &str, files: &[&str]) {
    let mut inner = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::FileOptions::default();
    for file in files {
        inner.start_file(*file, options).unwrap();
    }
    let inner = inner.finish().unwrap().into_inner();
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    zip.start_file("fabric.mod.json", options).unwrap();
    write!(
        zip,
        r#"{{"schemaVersion": 1, "id": "{id}", "version": "1.0"}}"#
    )
    .unwrap();
    zip.start_file("META-INF/jars/library.jar", options)
        .unwrap();
    zip.write_all(&inner).unwrap();
    zip.finish().unwrap();
}

fn instance() -> (tempfile::TempDir, InstanceDirs) {
    let dir = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(dir.path());
    fs::create_dir_all(dirs.content_dir(ContentKind::Mod)).unwrap();
    (dir, dirs)
}

fn mods(dirs: &InstanceDirs) -> std::path::PathBuf {
    dirs.content_dir(ContentKind::Mod)
}

/// Shaped like a real Fabric 1.20.1 report: the crash is in Enchantips' code, Zenith
/// injected into a method further down, and the Head block found no mixins, while a
/// later block (for another stack) lists Flywheel.
const ENCHANTIPS: &str = "---- Minecraft Crash Report ----
// Daisy, daisy...

Time: 2025-05-08 18:13:02
Description: Rendering screen

java.lang.NullPointerException: Cannot invoke \"com.fedpol1.enchantips.config.tree.GroupNode.getChild(String)\" because \"gn\" is null
\tat knot//com.fedpol1.enchantips.util.EnchantmentLevel.getColor(EnchantmentLevel.java:71)
\tat knot//com.fedpol1.enchantips.util.EnchantmentAppearanceHelper.getName(EnchantmentAppearanceHelper.java:15)
\tat knot//MC//net.minecraft.enchantment.Enchantment.getName(Enchantment.java:2520)
\tat knot//MC//net.minecraft.item.ItemStack.redirect$bakh000$zenith$enchTooltipRewrite(ItemStack.java:2641)
\tat knot//MC//net.minecraft.item.ItemStack.getTooltip(ItemStack.java:656)
\tat knot//dev.shadowsoffire.apotheosis.ench.table.ApothEnchScreen.render(ApothEnchScreen.java:188)
\tat knot//MC//net.minecraft.client.render.GameRenderer.mixinextras$bridge$method_47413$326(GameRenderer.java)
\tat knot//MC//net.minecraft.client.render.GameRenderer.render(GameRenderer.java:945)
\tat knot//net.minecraft.client.main.Main.main(Main.java:250)
\tat app//net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)
\tat app//org.prismlauncher.EntryPoint.main(EntryPoint.java:70)


A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Stacktrace:
\tat knot//com.fedpol1.enchantips.util.EnchantmentLevel.getColor(EnchantmentLevel.java:71)
Mixins in Stacktrace: None found

-- Screen render details --
Stacktrace:
\tat knot//net.minecraft.client.main.Main.main(Main.java:250)
Mixins in Stacktrace:
\tnet.minecraft.client.main.Main:
\t\tcom.jozufozu.flywheel.mixin.ClientMainMixin (flywheel.mixins.json)

-- System Details --
";

fn enchantips_instance() -> (tempfile::TempDir, InstanceDirs) {
    let (dir, dirs) = instance();
    let mods = mods(&dirs);
    jar(
        &mods.join("enchantips-1.5.jar"),
        "enchantips",
        "Enchantips",
        &["com/fedpol1/enchantips/util/EnchantmentLevel.class"],
    );
    jar(&mods.join("zenith-1.2.jar"), "zenith", "Zenith", &[]);
    jar(
        &mods.join("apotheosis.jar"),
        "apotheosis",
        "Apotheosis",
        &["dev/shadowsoffire/apotheosis/ench/table/ApothEnchScreen.class"],
    );
    jar(
        &mods.join("flywheel.jar"),
        "flywheel",
        "Flywheel",
        &[
            "flywheel.mixins.json",
            "com/jozufozu/flywheel/mixin/ClientMainMixin.class",
        ],
    );
    jar(&mods.join("sodium.jar"), "sodium", "Sodium", &[]);
    (dir, dirs)
}

#[test]
fn blames_the_mod_whose_code_crashed() {
    let (_dir, dirs) = enchantips_instance();
    let suspects = find(&dirs, Some(ENCHANTIPS), &[]);
    let found: Vec<_> = suspects
        .iter()
        .map(|s| (s.name.as_str(), s.evidence))
        .collect();
    assert_eq!(
        found,
        [
            ("Enchantips", Evidence::CrashedInItsCode),
            ("Zenith", Evidence::PatchedCrashingCode),
            ("Apotheosis", Evidence::InStack),
        ]
    );
    assert_eq!(suspects[0].file_name, "enchantips-1.5.jar");
    assert!(likely(&suspects));
}

#[test]
fn ignores_disabled_mods() {
    let (_dir, dirs) = enchantips_instance();
    let mods = mods(&dirs);
    fs::rename(
        mods.join("enchantips-1.5.jar"),
        mods.join("enchantips-1.5.jar.disabled"),
    )
    .unwrap();
    let suspects = find(&dirs, Some(ENCHANTIPS), &[]);
    assert_eq!(suspects[0].name, "Zenith");
    assert!(!likely(&suspects));
}

#[test]
fn only_the_head_mixin_block_counts() {
    // Flywheel patched the code that wrote the report, not the code that crashed.
    let (_dir, dirs) = enchantips_instance();
    let suspects = find(&dirs, Some(ENCHANTIPS), &[]);
    assert!(suspects.iter().all(|s| s.name != "Flywheel"));
}

const MIXIN_HEAD: &str = "---- Minecraft Crash Report ----
Description: Unexpected error

java.lang.NullPointerException: Cannot read field \"x\" because \"this.y\" is null
\tat knot//MC//net.minecraft.class_761.method_22710(class_761.java:1234)
\tat knot//MC//net.minecraft.class_757.method_3188(class_757.java:567)
\tat knot//MC//net.minecraft.class_310.method_1523(class_310.java:890)

A detailed walkthrough of the error, its code path and all known details is as follows:

-- Head --
Thread: Render thread
Stacktrace:
\tat knot//MC//net.minecraft.class_761.method_22710(class_761.java:1234)
Mixins in Stacktrace:
\tnet.minecraft.class_761:
\t\tnet.caffeinemc.mods.sodium.mixin.core.WorldRendererMixin (sodium.mixins.json)
\t\tnet.fabricmc.fabric.mixin.client.rendering.WorldRendererMixin (fabric-rendering-v1.mixins.json)
\t\tcom.example.skybox.mixin.SkyMixin (skybox.mixins.json)
\tnet.minecraft.class_757:
\t\tnet.caffeinemc.mods.sodium.mixin.core.GameRendererMixin (sodium.mixins.json)
\tnet.minecraft.class_310:
\t\tnet.caffeinemc.mods.sodium.mixin.core.MinecraftMixin (sodium.mixins.json)

-- Affected level --
";

#[test]
fn a_mod_that_patched_the_whole_stack_counts_for_little() {
    let (_dir, dirs) = instance();
    let mods = mods(&dirs);
    jar(
        &mods.join("sodium.jar"),
        "sodium",
        "Sodium",
        &[
            "sodium.mixins.json",
            "net/caffeinemc/mods/sodium/mixin/core/WorldRendererMixin.class",
        ],
    );
    jar(
        &mods.join("skybox.jar"),
        "skybox",
        "Skybox",
        &["skybox.mixins.json"],
    );
    let suspects = find(&dirs, Some(MIXIN_HEAD), &[]);
    let found: Vec<_> = suspects
        .iter()
        .map(|s| (s.name.as_str(), s.evidence))
        .collect();
    assert_eq!(
        found,
        [
            ("Skybox", Evidence::PatchedCrashingCode),
            ("Sodium", Evidence::InStack),
        ]
    );
    assert!(!likely(&suspects));
}

#[test]
fn names_the_mod_that_failed_to_patch_from_the_log() {
    let (_dir, dirs) = instance();
    let mods = mods(&dirs);
    jar(&mods.join("old-mod.jar"), "oldmod", "Old Mod", &[]);
    jar(&mods.join("other.jar"), "other", "Other", &[]);
    let log = [
        LogLine::plain(Stream::Stdout, "[main/INFO]: Loading 2 mods"),
        LogLine::plain(
            Stream::Stdout,
            "[main/ERROR]: Mixin apply for mod oldmod failed oldmod.mixins.json:TitleMixin from mod oldmod -> net.minecraft.class_442: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException: Critical injection failure",
        ),
    ];
    let suspects = find(&dirs, None, &log);
    assert_eq!(suspects.len(), 1);
    assert_eq!(suspects[0].name, "Old Mod");
    assert_eq!(suspects[0].evidence, Evidence::FailedToPatch);
    assert!(likely(&suspects));
}

#[test]
fn finds_a_failing_mixin_config_in_a_forge_style_message() {
    let (_dir, dirs) = instance();
    let mods = mods(&dirs);
    jar(
        &mods.join("thing.jar"),
        "thing",
        "Thing",
        &["thing.mixins.json"],
    );
    let log = [LogLine::plain(
        Stream::Stdout,
        "Mixin [thing.mixins.json:client.ScreenMixin] from phase [DEFAULT] in config [thing.mixins.json] FAILED during APPLY",
    )];
    let suspects = find(&dirs, None, &log);
    assert_eq!(suspects[0].name, "Thing");
    assert_eq!(suspects[0].evidence, Evidence::FailedToPatch);
}

#[test]
fn finds_classes_in_bundled_jars_and_reads_the_log_trace() {
    let (_dir, dirs) = instance();
    let mods = mods(&dirs);
    jar_with_nested(
        &mods.join("bundle.jar"),
        "bundle",
        &["org/example/lib/Thing.class"],
    );
    let mut line = LogLine::plain(Stream::Stdout, "Unreported exception thrown!");
    line.throwable = Some(
        "java.lang.IllegalStateException: boom\n\tat java.base/java.util.Objects.requireNonNull(Objects.java:233)\n\tat org.example.lib.Thing.run(Thing.java:5)\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:1)".into(),
    );
    let suspects = find(&dirs, None, &[line]);
    assert_eq!(suspects.len(), 1);
    assert_eq!(suspects[0].file_name, "bundle.jar");
    assert_eq!(suspects[0].evidence, Evidence::CrashedInItsCode);
}

#[test]
fn nothing_to_go_on() {
    let (_dir, dirs) = instance();
    jar(&mods(&dirs).join("a.jar"), "a", "A", &[]);
    let report = "Description: Initializing game\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.<init>(Minecraft.java:1)\n";
    assert!(find(&dirs, Some(report), &[]).is_empty());
    assert!(!likely(&[]));
}

#[test]
fn parses_frames() {
    assert_eq!(
        frame("\tat knot//MC//net.minecraft.item.ItemStack.getTooltip(ItemStack.java:656)"),
        Some(Frame {
            class: "net.minecraft.item.ItemStack".into(),
            method: "getTooltip".into()
        })
    );
    assert_eq!(
        frame("at TRANSFORMER/examplemod@1.0/com.example.Foo.bar(Foo.java:10) ~[examplemod-1.0.jar%23215!/:1.0] {re:classloading}")
            .map(|f| f.class),
        Some("com.example.Foo".into())
    );
    assert_eq!(frame("Caused by: java.lang.RuntimeException"), None);
    assert_eq!(
        injected_by("redirect$bakh000$zenith$enchTooltipRewrite").as_deref(),
        Some("zenith")
    );
    assert_eq!(injected_by("handler$zza000$onRender"), None);
    assert_eq!(injected_by("mixinextras$bridge$method_47413$326"), None);
    assert_eq!(
        injected_by("handler$abc000$fabric-rendering-v1$onRender"),
        None
    );
}

#[test]
fn reads_the_root_cause() {
    let text = "Description: x\n\njava.lang.RuntimeException: outer\n\tat net.minecraft.Outer.run(Outer.java:1)\n\tat com.outer.Mod.call(Mod.java:2)\nCaused by: java.lang.NullPointerException\n\tat com.inner.Mod.go(Mod.java:3)\n\t... 2 more\n\nA detailed walkthrough";
    let trace = first_trace(text);
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[1][0].class, "com.inner.Mod");
}

#[test]
fn names_the_mod_whose_entrypoint_failed() {
    let (_dir, dirs) = instance();
    let mods = mods(&dirs);
    jar(&mods.join("india-1.0.jar"), "india", "India Effects", &[]);
    jar(&mods.join("juliet-1.0.jar"), "juliet", "Juliet Menus", &[]);
    let report = "---- Minecraft Crash Report ----\nDescription: Initializing game\n\njava.lang.RuntimeException: Could not execute entrypoint stage 'main' due to errors, provided by 'india' at 'com.example.india.IndiaMod'!\n\tat net.fabricmc.loader.impl.FabricLoaderImpl.lambda$invokeEntrypoints$2(FabricLoaderImpl.java:403)\n\tat knot//net.minecraft.class_310.<init>(class_310.java:458)\n";
    let suspects = find(&dirs, Some(report), &[]);
    assert_eq!(suspects.len(), 1);
    assert_eq!(suspects[0].name, "India Effects");
    assert_eq!(suspects[0].evidence, Evidence::CrashedInItsCode);
    assert!(likely(&suspects));
}
