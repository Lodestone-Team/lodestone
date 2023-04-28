import { ClientEvent } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone_core/releases/0.5.0/deno_bindings/ClientEvent.ts"

declare const Deno: any;
const core = Deno.core;
const { ops } = core;

export async function nextEvent() : Promise<ClientEvent> {
    return core.opAsync("next_event");
}

export function broadcastEvent(event: ClientEvent) {
    ops.broadcast_event(event);
}

export function emitConsoleOut(line : string, instanceName : string, instanceUuid : string) {
    ops.emit_console_out(line, instanceName, instanceUuid);
}

