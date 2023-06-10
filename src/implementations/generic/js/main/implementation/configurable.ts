import { ConfigurableManifest } from "../libs/bindings/ConfigurableManifest.ts";
import { ConfigurableValue } from "../libs/bindings/ConfigurableValue.ts";
import { Game } from "../libs/bindings/Game.ts";

export async function getConfigurableManifest(): Promise<ConfigurableManifest> {
    throw new Error("Not implemented");
}

export async function getName(): Promise<string> {
    throw new Error("Not implemented");
}

export async function getGame(): Promise<Game> {
    throw new Error("Not implemented");
}

export async function getVersion(): Promise<string> {
    throw new Error("Not implemented");
}

export async function getDescription(): Promise<string> {
    throw new Error("Not implemented");
}

export async function getPort(): Promise<number> {
    throw new Error("Not implemented");
}

export async function getAutoStart(): Promise<boolean> {
    throw new Error("Not implemented");
}

export async function getRestartOnCrash(): Promise<boolean> {
    throw new Error("Not implemented");
}

export async function setName(name: string) {
    throw new Error("Not implemented");
}

export async function setDescription(description: string) {
    throw new Error("Not implemented");
}

export async function setPort(port: number) {
    throw new Error("Not implemented");
}

export async function setAutoStart(auto_start: boolean) {
    throw new Error("Not implemented");
}

export async function setRestartOnCrash(restart_on_crash: boolean) {
    throw new Error("Not implemented");
}

export async function updateConfigurable(section_id: string, setting_id: string, value: ConfigurableValue) {
    throw new Error("Not implemented");
}