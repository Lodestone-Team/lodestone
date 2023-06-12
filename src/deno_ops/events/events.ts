import { ClientEvent } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone_core/main/bindings/ClientEvent.ts"
import { MacroPID } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone_core/main/bindings/MacroPID.ts";

declare const Deno: any;
const core = Deno[Deno.internal].core;
const { ops } = core;

export async function nextEvent(): Promise<ClientEvent> {
    return core.opAsync("next_event");
}

export function emitDetach(pid: MacroPID, instanceUuid: string | null) {
    ops.emit_detach(pid, instanceUuid);
}

export function emitConsoleOut(line: string, instanceName: string, instanceUuid: string) {
    ops.emit_console_out(line, instanceName, instanceUuid);
}

