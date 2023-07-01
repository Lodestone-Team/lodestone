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

// re-export
export type { CausedBy } from "../../../../../../deno_bindings/CausedBy.ts";
export type { ConfigurableManifest } from "../../../../../../deno_bindings/ConfigurableManifest.ts";
export type { ConfigurableValue } from "../../../../../../deno_bindings/ConfigurableValue.ts";
export type { DotLodestoneConfig } from "../../../../../../deno_bindings/DotLodestoneConfig.ts";
export type { Game } from "../../../../../../deno_bindings/Game.ts";
export type { GenericPlayer } from "../../../../../../deno_bindings/GenericPlayer.ts";
export type { InstanceState } from "../../../../../../deno_bindings/InstanceState.ts";
export type { SetupValue } from "../../../../../../deno_bindings/SetupValue.ts";
export type { PerformanceReport } from "../../../../../../deno_bindings/PerformanceReport.ts";
export type { SetupManifest } from "../../../../../../deno_bindings/SetupManifest.ts";


export abstract class AtomInstance {
    /**
     * Get the setup manifest for this instance.
     * 
     * Setup manifest is a list of configurable values that is then passed to the setup function.
     * 
     * Setup manifest consists of an order list of sections, each section has a name and a list of configurable values.
     * 
     * Note: implementation must ensure each setting_id and section_id is unique among each other.
     * 
     * @returns {Promise<SetupManifest>} The setup manifest for this instance.
     */
    public abstract setupManifest(): Promise<SetupManifest>;

    /**
     * 
     * Setup the instance.
     * 
     * This function is called when the instance is first created.
     * 
     * Implementation must ensure that the instance object is in a valid state after this function has successfully returned.
     * 
     * @param setupValue - The setup value for this instance.
     * @param dotLodestoneConfig - The dot lodestone config for this instance, the most important field is the uuid field.
     * @param path - The file system path the instance is located at.
     */
    public abstract setup(
        setupValue: SetupValue,
        dotLodestoneConfig: DotLodestoneConfig,
        path: string,
    ): Promise<void>;
    /**
     * 
     * Restore the instance from the file system.
     * 
     * This function is called when the instance has been created and the instance is being restored from the file system by Lodestone Core
     * 
     * Implementation must ensure that the instance object is in a valid state after this function has successfully returned.
     * 
     * Note the lack of any state passed into this function, the implementation must restore the state from the file system.
     * 
     * @param dotLodestoneConfig - The dot lodestone config for this instance, the most important field is the uuid field.
     * @param path - The file system path the instance is located at.
     */
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