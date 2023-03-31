import { ConfigurableManifest } from "../libs/bindings/ConfigurableManifest.ts";
import { DotLodestoneConfig } from "../libs/bindings/DotLodestoneConfig.ts";
import { ManifestValue } from "../libs/bindings/ManifestValue.ts";
import { SetupManifest } from "../libs/bindings/SetupManifest.ts";


export async function setupManifest(): Promise<SetupManifest> {
    throw new Error("Not implemented");
}

// deno-lint-ignore require-await
export async function setupInstance(
    setupValue: ManifestValue,
    dotLodestoneConfig: DotLodestoneConfig,
    path: string,
) {
    throw new Error("Not implemented");
}

export async function restoreInstance(
    dotLodestoneConfig: DotLodestoneConfig,
    path: string,
) {
    throw new Error("Not implemented");
}