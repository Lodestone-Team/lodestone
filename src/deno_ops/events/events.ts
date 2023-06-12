import { ClientEvent } from "../../../deno_bindings/ClientEvent.ts"
import { TaskPID } from "../../../deno_bindings/TaskPID.ts";
import * as prelude from "../prelude/prelude.ts"
import * as InstanceControl from "../instance_control/instance_control.ts"
import { InstanceEvent } from "../../../deno_bindings/InstanceEvent.ts";
import { InstanceState } from "../../../deno_bindings/InstanceState.ts";

declare const Deno: any;
const core = Deno[Deno.internal].core;
const { ops } = core;

export function nextEvent(): Promise<ClientEvent> {
    return core.opAsync("next_event");
}

export function nextInstanceEvent(instanceUuid: string = prelude.instanceUUID()!): Promise<InstanceEvent> {
    return core.opAsync("next_instance_event", instanceUuid);
}

export function nextInstanceStateChange(instanceUuid: string = prelude.instanceUUID()!): Promise<InstanceState> {
    return core.opAsync("next_instance_state_change", instanceUuid);
}

/**  Notifies the caller that the macro wishes to be run in the background.
 * 
* This is a no-op if the macro is already running in the background, or called multiple times.

* This function DOES NOT exit the macro.
*/
export function detach() {
    emitDetach();
}

export function emitDetach(instanceUuid: string = prelude.instanceUUID()!, pid: TaskPID = prelude.taskPid()) {
    ops.emit_detach(pid, instanceUuid);
}

export function emitConsoleOut(instanceUuid: string = prelude.instanceUUID()!, line: string) {
    InstanceControl.getInstanceName().then((name) => {
        ops.emit_console_out(instanceUuid, name, line);
    }
    )
}

