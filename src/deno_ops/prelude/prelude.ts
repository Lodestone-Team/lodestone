import { MacroPID } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone_core/main/bindings/MacroPID";
import { emiDetach } from "../events/events.ts";

declare const __macro_pid: MacroPID;
declare const __instance_uuid: string | null;
export function macroPid(): MacroPID {
    return __macro_pid;
}

export function instanceUUID(): string | null {
    return __instance_uuid;
}
// Notifies the caller that the macro wishes to be run in the background.
// This is a no-op if the macro is already running in the background, or called multiple times.
// This function DOES NOT exit the macro.
export function detach() {
    emiDetach(macroPid(), instanceUUID());
}