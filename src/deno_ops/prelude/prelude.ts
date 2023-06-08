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

export function detach() {
    emiDetach(macroPid(), instanceUUID());
}