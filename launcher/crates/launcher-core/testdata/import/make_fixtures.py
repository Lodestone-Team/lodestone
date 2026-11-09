"""Writes the import fixtures: one made-up data folder per launcher, in each launcher's
format (see the module docs in src/import/). Run from anywhere with `python3 -I
make_fixtures.py`; it writes next to itself, replacing what is there. The Modrinth App
databases need Python's sqlite3 module with SQLite 3.45 or later (for jsonb()).

Everything is made up: no file comes from a real launcher's folder."""

import json, os, shutil, sqlite3, struct, zlib

HERE = os.path.dirname(os.path.abspath(__file__))


def write(path, data):
    path = os.path.join(HERE, path)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    mode = "wb" if isinstance(data, bytes) else "w"
    with open(path, mode) as f:
        f.write(data)


def jwrite(path, value):
    write(path, json.dumps(value, indent=2) + "\n")


def png(width, height, rgb):
    """A solid-colour PNG."""
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def game(base, mods=(), extra=True):
    """A game folder with player files, and caches and logs that are left out."""
    for name in mods:
        write(f"{base}/mods/{name}", f"jar bytes of {name}\n")
    write(f"{base}/options.txt", "version:3465\nfov:0.5\n")
    write(f"{base}/servers.dat", b"\x0a\x00\x00\x00")
    write(f"{base}/saves/My World/level.dat", b"level data")
    write(f"{base}/config/example.toml", "enabled = true\n")
    write(f"{base}/resourcepacks/Faithful.zip", b"PK pack")
    if extra:
        write(f"{base}/screenshots/2026-01-01_12.00.00.png", png(2, 2, (10, 20, 30)))
        write(f"{base}/logs/latest.log", "[main/INFO]: log line\n")
        write(f"{base}/crash-reports/crash-2026.txt", "crash\n")
        write(f"{base}/.cache/fabric.cache", b"cache")
        write(f"{base}/Thumbs.db", b"\x00")


for name in ["vanilla", "prism", "multimc", "modrinth-app", "modrinth-app-0.15", "modrinth-app-legacy",
             "atlauncher", "curseforge", "gdlauncher", "gdlauncher-carbon"]:
    shutil.rmtree(os.path.join(HERE, name), ignore_errors=True)

# The official launcher (launcher_profiles.json, minecraft.wiki).
V = "vanilla/minecraft"
jwrite(f"{V}/launcher_profiles.json", {
    "profiles": {
        "a1": {"type": "latest-release", "lastVersionId": "latest-release", "icon": "Grass",
               "lastUsed": "2026-01-03T00:00:00.000Z", "name": ""},
        "b2": {"type": "custom", "name": "Fabric 1.20.1", "lastVersionId": "fabric-loader-0.16.14-1.20.1",
               "gameDir": "profiles/fabric", "javaArgs": "-Xmx4G -XX:+UseG1GC -Xms1G",
               "icon": "data:image/png;base64," + __import__("base64").b64encode(png(4, 4, (200, 30, 30))).decode(),
               "lastUsed": "2026-01-02T00:00:00.000Z"},
        "c3": {"type": "custom", "name": "Old Forge", "lastVersionId": "1.7.10-Forge10.13.4.1614-1.7.10",
               "javaDir": "/Library/Java/bin/java", "lastUsed": "2026-01-01T00:00:00.000Z"},
    },
    "settings": {}, "version": 3,
})
jwrite(f"{V}/versions/fabric-loader-0.16.14-1.20.1/fabric-loader-0.16.14-1.20.1.json",
       {"id": "fabric-loader-0.16.14-1.20.1", "inheritsFrom": "1.20.1", "libraries": []})
write(f"{V}/launcher_accounts.json", '{"accounts": {"x": {"accessToken": "made-up"}}}\n')
write(f"{V}/launcher_msa_credentials.bin", b"made-up secret")
write(f"{V}/launcher_log.txt", "log\n")
write(f"{V}/assets/indexes/17.json", "{}")
write(f"{V}/libraries/org/x.jar", b"lib")
write(f"{V}/versions/1.20.1/1.20.1.jar", b"jar")
write(f"{V}/webcache2/Cache", b"cache")
game(V)
game(f"{V}/profiles/fabric", mods=["sodium-fabric-0.5.13.jar", "unknown-mod.jar"])

# Prism Launcher (QSettings INI, mmc-pack.json).
P = "prism/PrismLauncher"
write(f"{P}/prismlauncher.cfg", "[General]\nConfigVersion=1.2\nInstanceDir=instances\nIconsDir=icons\n")
write(f"{P}/icons/mypack.png", png(8, 8, (30, 200, 30)))
I = f"{P}/instances/Fabric Pack"
write(f"{I}/instance.cfg", "[General]\nConfigVersion=1.3\nInstanceType=OneSix\niconKey=mypack\n"
      "name=\"Fabric Pack, Deluxe\"\nOverrideMemory=true\nMinMemAlloc=1024\nMaxMemAlloc=6144\n"
      "OverrideJavaArgs=true\nJvmArgs=\"-XX:+UseZGC -Dfoo=bar\"\nOverrideJavaLocation=false\n")
jwrite(f"{I}/mmc-pack.json", {"formatVersion": 1, "components": [
    {"uid": "org.lwjgl3", "version": "3.3.1", "dependencyOnly": True},
    {"uid": "net.minecraft", "version": "1.20.1", "important": True},
    {"uid": "net.fabricmc.intermediary", "version": "1.20.1", "dependencyOnly": True},
    {"uid": "net.fabricmc.fabric-loader", "version": "0.16.14"},
]})
game(f"{I}/minecraft", mods=["sodium-fabric-0.5.13.jar", "unknown-mod.jar"])
write(f"{I}/minecraft/shaderpacks/BSL.zip", b"PK shader")
I = f"{P}/instances/Forge Old"
write(f"{I}/instance.cfg", "[General]\nConfigVersion=1.3\nname=Forge Old\niconKey=flame\n"
      "OverrideMemory=false\nMaxMemAlloc=9999\n")
jwrite(f"{I}/mmc-pack.json", {"formatVersion": 1, "components": [
    {"uid": "net.minecraft", "version": "1.7.10"},
    {"uid": "net.minecraftforge", "version": "10.13.4.1614"},
]})
game(f"{I}/.minecraft", extra=False)
I = f"{P}/instances/NeoForge 1.20.1"
write(f"{I}/instance.cfg", "[General]\nname=NeoForge 1.20.1\n")
jwrite(f"{I}/mmc-pack.json", {"formatVersion": 1, "components": [
    {"uid": "net.minecraft", "version": "1.20.1"}, {"uid": "net.neoforged", "version": "47.1.106"}]})
write(f"{P}/instances/not an instance/readme.txt", "hi\n")

# MultiMC (section-less instance.cfg, legacy IntendedVersion).
M = "multimc/MultiMC.app/Data"
write(f"{M}/multimc.cfg", "InstanceDir=instances\n")
I = f"{M}/instances/Legacy"
write(f"{I}/instance.cfg", "InstanceType=OneSix\nname=Legacy\\#1\nIntendedVersion=1.8.9\n"
      "ForgeVersion=11.15.1.2318\nOverrideMemory=true\nMaxMemAlloc=2048\n")
game(f"{I}/.minecraft", extra=False)

# ATLauncher (instance.json, Gson).
A = "atlauncher/instances/VanillaPlusFabric"
jwrite(f"{A}/instance.json", {
    "id": "1.21.1", "type": "release", "inheritsFrom": None,
    "launcher": {"name": "Vanilla Plus (Fabric)", "pack": "Vanilla Plus", "version": "1.0",
                 "loaderVersion": {"version": "0.16.5", "rawVersion": "0.16.5", "recommended": False, "type": "Fabric"},
                 "maximumMemory": 8192, "javaArguments": "-XX:+UseG1GC", "vanillaInstance": False},
})
write(f"{A}/instance.png", png(4, 4, (0, 0, 200)))
game(A, mods=["fabric-api.jar"])
A = "atlauncher/instances/ForgeOne"
jwrite(f"{A}/instance.json", {"id": "1.20.1", "launcher": {"name": "Forge One",
       "loaderVersion": {"version": "47.4.0", "rawVersion": "1.20.1-47.4.0", "type": "Forge"}}})
game(A, extra=False)

# CurseForge (minecraftinstance.json).
C = "curseforge/minecraft/Instances/All the Mods"
jwrite(f"{C}/minecraftinstance.json", {
    "name": "All the Mods", "gameVersion": "1.20.1", "installPath": "/Users/x/curseforge/minecraft/Instances/All the Mods",
    "baseModLoader": {"name": "forge-47.4.0", "forgeVersion": "47.4.0", "minecraftVersion": "1.20.1", "type": 1},
    "isMemoryOverride": True, "allocatedMemory": 10240, "javaArgsOverride": "-XX:+UseG1GC", "profileImagePath": None,
    "installedAddons": [], "playedCount": 3})
game(C, mods=["jei.jar"])
C = "curseforge/minecraft/Instances/Fabulously Fabric"
jwrite(f"{C}/minecraftinstance.json", {"name": "Fabulously Fabric", "gameVersion": "1.21.1",
       "baseModLoader": {"name": "fabric-0.16.9-1.21.1", "minecraftVersion": "1.21.1", "type": 4},
       "isMemoryOverride": False, "allocatedMemory": 4096})
game(C, extra=False)

# GDLauncher legacy (config.json).
G = "gdlauncher/gdlauncher_next/instances/Forge 1.16"
jwrite(f"{G}/config.json", {"loader": {"loaderType": "forge", "loaderVersion": "1.16.5-36.2.39", "mcVersion": "1.16.5",
       "source": "curseforge"}, "javaArgs": "-XX:+UseG1GC", "javaMemory": 6144, "background": "background.png",
       "timePlayed": 100, "mods": []})
write(f"{G}/background.png", png(4, 4, (250, 250, 0)))
game(G, mods=["create.jar"])

# GDLauncher Carbon (instance.json + instance/).
G = "gdlauncher-carbon/gdlauncher_carbon/data/instances/Quilt Test"
jwrite(f"{G}/instance.json", {"_version": "1", "name": "Quilt Test", "icon": "icon.png",
       "game_configuration": {"version": {"release": "1.20.4", "modloaders": [{"type": "Quilt", "version": "0.26.0"}]},
                              "global_java_args": True, "extra_java_args": "-Dx=y",
                              "memory": {"min_mb": 1024, "max_mb": 3072}, "java_override": None},
       "notes": ""})
write(f"{G}/icon.png", png(4, 4, (100, 0, 100)))
game(f"{G}/instance", extra=False)


# The Modrinth App (app.db, SQLite in WAL mode).
def modrinth(base, newer):
    os.makedirs(os.path.join(HERE, base), exist_ok=True)
    path = os.path.join(HERE, base, "app.db")
    db = sqlite3.connect(path)
    db.execute("PRAGMA page_size = 4096")
    db.execute("CREATE TABLE settings (id INTEGER NOT NULL CHECK (id = 0), custom_dir TEXT NULL, prev_custom_dir TEXT NULL, PRIMARY KEY (id))")
    db.execute("INSERT INTO settings (id) VALUES (0)")
    long_args = json.dumps(["-Dlong=" + "x" * 9000, "-XX:+UseG1GC"])
    if not newer:
        db.execute("""CREATE TABLE profiles (
            path TEXT NOT NULL PRIMARY KEY,
            install_stage TEXT NOT NULL,
            name TEXT NOT NULL,
            icon_path TEXT NULL,
            game_version TEXT NOT NULL,
            mod_loader TEXT NOT NULL,
            mod_loader_version TEXT NULL,
            groups JSONB NOT NULL
        )""")
        db.execute("INSERT INTO profiles VALUES ('Sodium Pack', 'installed', 'Sodium Pack', NULL, '1.20.1', 'fabric', '0.16.14', jsonb('[]'))")
        db.commit()
        # Columns added later, as the app's migrations do.
        db.execute("ALTER TABLE profiles ADD COLUMN override_java_path TEXT NULL")
        db.execute("ALTER TABLE profiles ADD COLUMN override_extra_launch_args JSONB NULL")
        db.execute("ALTER TABLE profiles ADD COLUMN override_mc_memory_max INTEGER NULL")
        db.commit()
        db.execute("PRAGMA journal_mode = WAL")
        db.execute("PRAGMA wal_autocheckpoint = 0")
        # Only in the WAL: a new profile with a large (overflowing) JSONB column, and a change.
        db.execute("INSERT INTO profiles (path, install_stage, name, icon_path, game_version, mod_loader, mod_loader_version, groups, override_extra_launch_args, override_mc_memory_max, override_java_path) "
                   "VALUES ('forge-pack', 'installed', 'Forge Pack', ?, '1.20.1', 'forge', '47.4.28', jsonb('[\"Modded\"]'), jsonb(?), 6144, '/opt/java')",
                   (os.path.join("caches", "icons", "forge.png"), long_args))
        db.execute("UPDATE profiles SET name = 'Sodium Pack (renamed)' WHERE path = 'Sodium Pack'")
        db.execute("INSERT INTO profiles (path, install_stage, name, game_version, mod_loader, groups) VALUES ('Vanilla', 'not_installed', 'Plain', '1.21.1', 'vanilla', jsonb('[]'))")
        db.commit()
    else:
        db.execute("""CREATE TABLE instances (
            id TEXT NOT NULL, path TEXT NOT NULL, applied_content_set_id TEXT NULL,
            install_stage TEXT NOT NULL, launcher_feature_version TEXT NOT NULL,
            update_channel TEXT NOT NULL DEFAULT 'release', name TEXT NOT NULL, icon_path TEXT NULL,
            created INTEGER NOT NULL, modified INTEGER NOT NULL, last_played INTEGER NULL,
            submitted_time_played INTEGER NOT NULL DEFAULT 0, recent_time_played INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (id), UNIQUE (path))""")
        db.execute("""CREATE TABLE instance_launch_overrides (
            instance_id TEXT NOT NULL, overrides JSONB NOT NULL,
            PRIMARY KEY (instance_id), FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE)""")
        db.execute("""CREATE TABLE instance_content_sets (
            id TEXT NOT NULL, instance_id TEXT NOT NULL, name TEXT NOT NULL, source_kind TEXT NOT NULL,
            status TEXT NOT NULL, game_version TEXT NOT NULL, protocol_version INTEGER NULL,
            loader TEXT NOT NULL, loader_version TEXT NULL, created INTEGER NOT NULL, modified INTEGER NOT NULL,
            PRIMARY KEY (id), FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE)""")
        for i in range(60):
            # Enough rows for interior b-tree pages.
            db.execute("INSERT INTO instances VALUES (?, ?, ?, 'installed', '1', 'release', ?, NULL, 1, 1, NULL, 0, 0)",
                       (f"id{i:02}", f"Instance {i:02}", f"set{i:02}", f"Instance {i:02} " + "n" * 60))
            db.execute("INSERT INTO instance_content_sets VALUES (?, ?, 'Default', 'local', 'applied', '1.21.1', NULL, ?, ?, 1, 1)",
                       (f"set{i:02}", f"id{i:02}", "neoforge" if i == 0 else "vanilla", "21.1.209" if i == 0 else None))
        db.execute("INSERT INTO instance_launch_overrides VALUES ('id00', jsonb(?))",
                   (json.dumps({"java_path": None, "extra_launch_args": ["-XX:+UseZGC"], "memory": {"maximum": 5120}, "hooks": {}}),))
        db.commit()
        db.execute("PRAGMA journal_mode = WAL")
        db.execute("PRAGMA wal_autocheckpoint = 0")
        db.execute("UPDATE instances SET name = 'NeoForge 21' WHERE id = 'id00'")
        db.commit()
    # Copy the files while the connection is open, so the WAL isn't checkpointed away.
    snapshot = os.path.join(HERE, base, "snapshot")
    os.makedirs(snapshot)
    for suffix in ["", "-wal"]:
        shutil.copy(path + suffix, os.path.join(snapshot, "app.db" + suffix))
    db.close()
    for suffix in ["", "-wal", "-shm"]:
        if os.path.exists(path + suffix):
            os.remove(path + suffix)
    for suffix in ["", "-wal"]:
        os.rename(os.path.join(snapshot, "app.db" + suffix), path + suffix)
    os.rmdir(snapshot)


modrinth("modrinth-app/ModrinthApp", newer=False)
write("modrinth-app/ModrinthApp/caches/icons/forge.png", png(4, 4, (255, 128, 0)))
game("modrinth-app/ModrinthApp/profiles/Sodium Pack", mods=["sodium-fabric-0.5.13.jar"])
game("modrinth-app/ModrinthApp/profiles/forge-pack", extra=False)
game("modrinth-app/ModrinthApp/profiles/Vanilla", extra=False)
modrinth("modrinth-app-0.15/ModrinthApp", newer=True)
game("modrinth-app-0.15/ModrinthApp/profiles/Instance 00", extra=False)

# The Modrinth App before SQLite (profile.json).
R = "modrinth-app-legacy/com.modrinth.theseus"
jwrite(f"{R}/settings.json", {"loaded_config_dir": None})
jwrite(f"{R}/profiles/Quilt Fun/profile.json", {
    "path": "Quilt Fun", "install_stage": "installed",
    "metadata": {"name": "Quilt Fun", "icon": None, "groups": [], "game_version": "1.20.1",
                 "loader": "quilt", "loader_version": {"id": "0.26.0", "url": "", "stable": True}},
    "java": {"extra_arguments": ["-Dq=1"]}, "memory": {"maximum": 3000}})
game(f"{R}/profiles/Quilt Fun", extra=False)
print("fixtures written")
