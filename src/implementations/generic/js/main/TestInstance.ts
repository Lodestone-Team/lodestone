import * as atom from "https://raw.githubusercontent.com/Lodestone-Team/lodestone_core/dev/src/implementations/generic/js/main/libs/atom_instance.ts";


export default class TestInstance extends atom.AtomInstance {
    public async setupManifest(): Promise<atom.SetupManifest> {
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
    public async setup(setupValue: atom.SetupValue, dotLodestoneConfig: atom.DotLodestoneConfig, path: string): Promise<void> {
        return;
    }
    public restore(dotLodestoneConfig: atom.DotLodestoneConfig, path: string): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public async start(caused_by: atom.CausedBy, block: boolean): Promise<void> {
        console.log("start");
        return;
    }
    public stop(caused_by: atom.CausedBy, block: boolean): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public restart(caused_by: atom.CausedBy, block: boolean): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public kill(caused_by: atom.CausedBy): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public state(): Promise<atom.InstanceState> {
        throw new Error("Method not implemented.");
    }
    public sendCommand(command: string, caused_by: atom.CausedBy): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public monitor(): Promise<atom.PerformanceReport> {
        throw new Error("Method not implemented.");
    }
    public configurableManifest(): Promise<atom.ConfigurableManifest> {
        throw new Error("Method not implemented.");
    }
    public name(): Promise<string> {
        throw new Error("Method not implemented.");
    }
    public version(): Promise<string> {
        throw new Error("Method not implemented.");
    }
    public game(): Promise<atom.Game> {
        throw new Error("Method not implemented.");
    }
    public description(): Promise<string> {
        throw new Error("Method not implemented.");
    }
    public port(): Promise<number> {
        throw new Error("Method not implemented.");
    }
    public getAutoStart(): Promise<boolean> {
        throw new Error("Method not implemented.");
    }
    public getRestartOnCrash(): Promise<boolean> {
        throw new Error("Method not implemented.");
    }
    public setName(name: string): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public setDescription(description: string): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public setPort(port: number): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public setAutoStart(auto_start: boolean): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public setRestartOnCrash(restart_on_crash: boolean): Promise<void> {
        throw new Error("Method not implemented.");
    }
    public playerCount(): Promise<number> {
        throw new Error("Method not implemented.");
    }
    public maxPlayerCount(): Promise<number> {
        throw new Error("Method not implemented.");
    }
    public playerList(): Promise<atom.GenericPlayer[]> {
        throw new Error("Method not implemented.");
    }
    public updateConfigurable(section_id: string, setting_id: string, value: atom.ConfigurableValue): Promise<void> {
        throw new Error("Method not implemented.");
    }
}
