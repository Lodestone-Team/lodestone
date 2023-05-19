import { ConfigurableManifest } from "../libs/bindings/ConfigurableManifest.ts";
import { DotLodestoneConfig } from "../libs/bindings/DotLodestoneConfig.ts";
import { ManifestValue } from "../libs/bindings/ManifestValue.ts";
import { SetupManifest } from "../libs/bindings/SetupManifest.ts";


export async function setupManifest(): Promise<SetupManifest> {
    return {
        setting_sections: {
            "test": {
                section_id: "section_id1",
                name: "section_name1",
                description: "section_description1",
                settings: {
                    "setting_id1": {
                        setting_id: "setting_id1",
                        name: "setting_name1",
                        description: "setting_description1",
                        value: null,
                        value_type: { type: "String", regex: null },
                        default_value: null,
                        is_secret: false,
                        is_required: true,
                        is_mutable: true,
                    }
                },
            }
        }
    };
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