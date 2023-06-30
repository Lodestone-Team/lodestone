import { CausedBy } from "../../../../../../deno_bindings/CausedBy.ts";
import { ConfigurableManifest } from "../../../../../../deno_bindings/ConfigurableManifest.ts";
import { ConfigurableValue } from "../../../../../../deno_bindings/ConfigurableValue.ts";
import { DotLodestoneConfig } from "../../../../../../deno_bindings/DotLodestoneConfig.ts";
import { Game } from "../../../../../../deno_bindings/Game.ts";
import { GenericPlayer } from "../../../../../../deno_bindings/GenericPlayer.ts";
import { InstanceState } from "../../../../../../deno_bindings/InstanceState.ts";
import { SetupValue } from "../../../../../../deno_bindings/SetupValue.ts";
import { PerformanceReport } from "../../../../../../deno_bindings/PerformanceReport.ts";
import { SetupManifest } from "../../../../../../deno_bindings/SetupManifest.ts";

export abstract class AtomInstance {
    public abstract setupManifest(): Promise<SetupManifest>;
    public abstract setup(
        setupValue: SetupValue,
        dotLodestoneConfig: DotLodestoneConfig,
        path: string,
    ): Promise<void>;
    public abstract restore(dotLodestoneConfig: DotLodestoneConfig,
        path: string): Promise<void>;
    public abstract start(caused_by: CausedBy, block: boolean): Promise<void>;
    public abstract stop(caused_by: CausedBy, block: boolean): Promise<void>;
    public abstract restart(caused_by: CausedBy, block: boolean): Promise<void>;
    public abstract kill(caused_by: CausedBy): Promise<void>;
    public abstract state(): Promise<InstanceState>;
    public abstract sendCommand(command: string, caused_by: CausedBy): Promise<void>;
    public abstract monitor(): Promise<PerformanceReport>;
    public abstract configurableManifest(): Promise<ConfigurableManifest>;
    public abstract name(): Promise<string>;
    public abstract version(): Promise<string>;
    public abstract game(): Promise<Game>;
    public abstract description(): Promise<string>;
    public abstract port(): Promise<number>;
    public abstract getAutoStart(): Promise<boolean>;
    public abstract getRestartOnCrash(): Promise<boolean>;
    public abstract setName(name: string): Promise<void>;
    public abstract setDescription(description: string): Promise<void>;
    public abstract setPort(port: number): Promise<void>;
    public abstract setAutoStart(auto_start: boolean): Promise<void>;
    public abstract setRestartOnCrash(restart_on_crash: boolean): Promise<void>;
    public abstract playerCount(): Promise<number>;
    public abstract maxPlayerCount(): Promise<number>;
    public abstract playerList(): Promise<GenericPlayer[]>;
    public abstract updateConfigurable(section_id: string, setting_id: string, value: ConfigurableValue): Promise<void>;

}