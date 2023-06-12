import { MacroPID } from "../../../deno_bindings/MacroPID.ts";
import { emitDetach } from "../events/events.ts";

declare const Deno: any;

declare const __macro_pid: MacroPID;
declare const __instance_uuid: string | null;
export function taskPid(): MacroPID {
    return __macro_pid;
}

export function instanceUUID(): string | null {
    return __instance_uuid;
}

// Notifies the caller that the macro wishes to be run in the background.
// This is a no-op if the macro is already running in the background, or called multiple times.
// This function DOES NOT exit the macro.
export function detach() {
    emitDetach(taskPid(), instanceUUID());
}