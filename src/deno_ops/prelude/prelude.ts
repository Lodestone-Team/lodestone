import { TaskPID } from "../../../deno_bindings/TaskPID.ts";


declare const __macro_pid: TaskPID;
declare const __instance_uuid: string | null;
export function taskPid(): TaskPID {
    return __macro_pid;
}

export function instanceUUID(): string | null {
    return __instance_uuid;
}
