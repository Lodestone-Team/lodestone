import { DotLodestoneConfig } from "../libs/bindings/DotLodestoneConfig.ts";
import { ManifestValue } from "../libs/bindings/ManifestValue.ts";

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