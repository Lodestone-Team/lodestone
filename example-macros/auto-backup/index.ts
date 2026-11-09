import { format } from "https://deno.land/std@0.177.1/datetime/format.ts";
import { copy } from "https://deno.land/std@0.191.0/fs/copy.ts";
import {
  isAbsolute,
  join,
  relative,
  resolve,
} from "https://deno.land/std@0.191.0/path/mod.ts";
import { sleep } from "https://deno.land/x/sleep@v1.2.1/mod.ts";
import {
  EventStream,
  ProgressionHandler,
} from "https://raw.githubusercontent.com/Lodestone-Team/lodestone-macro-lib/main/events.ts";
import { MinecraftJavaInstance } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone-macro-lib/main/instance.ts";

const currentInstance = await MinecraftJavaInstance.current();

const eventStream = new EventStream(
  currentInstance.getUUID(),
  await currentInstance.name()
);

// Lodestone will parse the configuration class and inject the configuration into the macro
class LodestoneConfig {
  // Where to store the backups relative to the instance path
  backupFolderRelative: string = "backups";
  // How long to wait between backups in seconds
  delaySec: number = 3600;
  // Comma-separated list of folders to back up, relative to the instance path
  foldersToBackup: string = "world, world_nether, world_the_end";
  // Compress each backup into a .tar.gz archive. Needs the `tar` command, which comes with Windows 10+, macOS and most Linux distributions
  compressBackup: boolean = true;
}

// not technically necessary, but it's a good practice to appease the linter
declare const config: LodestoneConfig;

// make sure the config is injected properly
console.log(config);

async function log(message: string) {
  await eventStream.emitConsoleOut(`[Backup Macro] ${message}`);
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function isSameOrInside(parent: string, child: string): boolean {
  const rel = relative(parent, child);
  return rel === "" || (!rel.startsWith("..") && !isAbsolute(rel));
}

// Resolves a name from the config to a path strictly inside `root`.
// Rejects empty names, absolute paths and ".." segments.
function resolveInside(root: string, name: string, what: string): string {
  const trimmed = name.trim();
  if (trimmed === "") {
    throw new Error(`${what} is empty`);
  }
  if (trimmed.includes("\0")) {
    throw new Error(`${what} "${trimmed}" contains an invalid character`);
  }
  // checked by hand so that Windows-style paths are rejected on every platform
  if (
    isAbsolute(trimmed) ||
    /^[a-zA-Z]:/.test(trimmed) ||
    trimmed.startsWith("/") ||
    trimmed.startsWith("\\")
  ) {
    throw new Error(`${what} "${trimmed}" must be relative to the instance`);
  }
  if (trimmed.split(/[\\/]+/).includes("..")) {
    throw new Error(`${what} "${trimmed}" must not contain ".."`);
  }
  const resolved = resolve(root, trimmed);
  if (resolved === resolve(root) || !isSameOrInside(root, resolved)) {
    throw new Error(`${what} "${trimmed}" is not inside the instance`);
  }
  return resolved;
}

// Returns the real path of a folder to back up and its path relative to the instance,
// or throws if the folder cannot be backed up safely. Symlinks are resolved first, so a
// link inside the instance cannot lead somewhere else.
async function resolveSource(
  instancePath: string,
  instanceReal: string,
  backupRoot: string,
  name: string
): Promise<{ source: string; relativePath: string }> {
  const trimmed = name.trim();
  const lexical = resolveInside(instancePath, name, "Folder");
  let source: string;
  try {
    source = await Deno.realPath(lexical);
  } catch {
    throw new Error(`folder "${trimmed}" does not exist`);
  }
  if (source === instanceReal || !isSameOrInside(instanceReal, source)) {
    throw new Error(`folder "${trimmed}" is not inside the instance`);
  }
  if (!(await Deno.stat(source)).isDirectory) {
    throw new Error(`"${trimmed}" is not a folder`);
  }
  // the backup folder, or a folder containing it, would be copied into itself
  if (isSameOrInside(source, backupRoot) || isSameOrInside(backupRoot, source)) {
    throw new Error(`folder "${trimmed}" overlaps the backup folder`);
  }
  return { source, relativePath: relative(instanceReal, source) };
}

// True if a backup with this name exists, as a folder, an archive or in any other state.
async function backupNameTaken(root: string, backupName: string): Promise<boolean> {
  for (const path of [
    join(root, `${backupName}.partial`),
    join(root, backupName),
    join(root, `${backupName}_INCOMPLETE`),
    join(root, `${backupName}.tar.gz`),
  ]) {
    try {
      await Deno.lstat(path);
      return true;
    } catch {
      // does not exist
    }
  }
  return false;
}

// `tar` runs without a shell and gets paths relative to `cwd`, so spaces and other special
// characters in the instance path cannot break it.
async function runTar(cwd: string, args: string[]): Promise<void> {
  const output = await new Deno.Command("tar", {
    args,
    cwd,
    stdout: "null",
    stderr: "piped",
  }).output();
  if (!output.success) {
    const stderr = new TextDecoder().decode(output.stderr).trim();
    throw new Error(`tar exited with code ${output.code}${stderr ? `: ${stderr}` : ""}`);
  }
}

interface BackupResult {
  // folders that are part of the backup
  backedUp: string[];
  // one message per problem
  failures: string[];
  // false if a folder is missing from the backup
  complete: boolean;
  // where the backup ended up, if anything was written
  location: string | null;
}

async function runBackup(
  instancePath: string,
  instanceReal: string,
  backupRoot: string,
  progression: ProgressionHandler
): Promise<BackupResult> {
  const result: BackupResult = {
    backedUp: [],
    failures: [],
    complete: true,
    location: null,
  };
  const fail = async (message: string) => {
    result.failures.push(message);
    await log(`ERROR: ${message}`);
  };

  const stamp = format(new Date(), "yy-MM-dd_HH-mm-ss");
  let backupName = `backup_${stamp}`;
  for (let n = 2; await backupNameTaken(backupRoot, backupName); n++) {
    backupName = `backup_${stamp}_${n}`;
  }
  const stagingPath = join(backupRoot, `${backupName}.partial`);
  await Deno.mkdir(stagingPath, { recursive: true });

  const seen = new Set<string>();
  for (const name of config.foldersToBackup.split(",")) {
    try {
      const { source, relativePath } = await resolveSource(
        instancePath,
        instanceReal,
        backupRoot,
        name
      );
      if (seen.has(relativePath)) {
        continue;
      }
      seen.add(relativePath);
      await log(`Backing up ${relativePath}...`);
      await copy(source, join(stagingPath, relativePath), { overwrite: true });
      result.backedUp.push(relativePath);
      progression.addProgress(1, `Backed up ${relativePath}`);
    } catch (e) {
      result.complete = false;
      await fail(`could not back up "${name.trim()}": ${errorMessage(e)}`);
    }
  }

  if (result.backedUp.length === 0) {
    await Deno.remove(stagingPath, { recursive: true }).catch(() => {});
    return result;
  }

  if (!result.complete) {
    // keep what was copied, but make it obvious that it is not a full backup
    result.location = join(backupRoot, `${backupName}_INCOMPLETE`);
    await Deno.rename(stagingPath, result.location);
    return result;
  }

  result.location = join(backupRoot, backupName);
  await Deno.rename(stagingPath, result.location);

  if (config.compressBackup) {
    const archiveName = `${backupName}.tar.gz`;
    try {
      await log(`Compressing ${backupName}...`);
      await runTar(backupRoot, ["-czf", archiveName, backupName]);
      // make sure the archive can be read before the folder it was made from is removed
      await runTar(backupRoot, ["-tzf", archiveName]);
      await Deno.remove(result.location, { recursive: true });
      result.location = join(backupRoot, archiveName);
    } catch (e) {
      await Deno.remove(join(backupRoot, archiveName)).catch(() => {});
      await fail(`compression failed, the uncompressed backup was kept: ${errorMessage(e)}`);
    }
  }
  return result;
}

// Fails with a clear error if the macro cannot work at all.
async function prepare(): Promise<{
  instancePath: string;
  instanceReal: string;
  backupRoot: string;
}> {
  const instancePath = await currentInstance.path();
  const instanceReal = await Deno.realPath(instancePath);
  const backupPath = resolveInside(
    instancePath,
    config.backupFolderRelative,
    "backupFolderRelative"
  );
  await Deno.mkdir(backupPath, { recursive: true });
  const backupRoot = await Deno.realPath(backupPath);
  if (backupRoot === instanceReal || !isSameOrInside(instanceReal, backupRoot)) {
    throw new Error(
      `backupFolderRelative "${config.backupFolderRelative}" is not inside the instance`
    );
  }
  return { instancePath, instanceReal, backupRoot };
}

let setup;
try {
  setup = await prepare();
} catch (e) {
  await log(`ERROR: cannot start: ${errorMessage(e)}`);
  throw e;
}
const { instancePath, instanceReal, backupRoot } = setup;

EventStream.emitDetach();
while (true) {
  if ((await currentInstance.state()) == "Stopped") {
    await log("Instance stopped, exiting...");
    break;
  }
  await log("Backing up world...");

  const progression = ProgressionHandler.create(
    "Backing up instance",
    config.foldersToBackup.split(",").length
  );
  let outcome: BackupResult;
  try {
    outcome = await runBackup(instancePath, instanceReal, backupRoot, progression);
  } catch (e) {
    outcome = {
      backedUp: [],
      failures: [`unexpected error: ${errorMessage(e)}`],
      complete: false,
      location: null,
    };
    await log(`ERROR: ${outcome.failures[0]}`);
  }

  if (outcome.failures.length === 0) {
    const message = `Backup of ${outcome.backedUp.join(", ")} completed: ${outcome.location}`;
    await log(message);
    progression.complete(true, message);
  } else {
    const kept = outcome.location ? ` Written to ${outcome.location}.` : "";
    const summary = outcome.complete
      ? "Backup copied but NOT compressed"
      : "Backup INCOMPLETE";
    const message = `${summary}: ${outcome.failures.join("; ")}.${kept}`;
    await log(message);
    progression.complete(false, message);
  }

  await sleep(config.delaySec);
}
