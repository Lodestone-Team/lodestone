import { ClientEvent } from "../../../deno_bindings/ClientEvent.ts"
import { TaskPID } from "../../../deno_bindings/TaskPID.ts";
import * as InstanceControl from "../instance_control/instance_control.ts"
import { InstanceEvent } from "../../../deno_bindings/InstanceEvent.ts";
import { InstanceState } from "../../../deno_bindings/InstanceState.ts";
import { ProgressionStartValue } from "../../../deno_bindings/ProgressionStartValue.ts";
import { ProgressionEndValue } from "../../../deno_bindings/ProgressionEndValue.ts";

// re-exports 
export type { ClientEvent, TaskPID, InstanceControl, InstanceEvent, InstanceState };

// deno-lint-ignore no-explicit-any
declare const Deno: any;
const core = Deno[Deno.internal].core;
const { ops } = core;

export interface PlayerMessage {
    player: string;
    message: string;
}

export function nextEvent(): Promise<ClientEvent> {
    return core.opAsync("next_event");
}

export function nextInstanceEvent(instanceUuid: string): Promise<InstanceEvent> {
    return core.opAsync("next_instance_event", instanceUuid);
}

export function nextInstanceStateChange(instanceUuid: string): Promise<InstanceState> {
    return core.opAsync("next_instance_state_change", instanceUuid);
}

export function nextInstanceConsoleOut(instanceUuid: string): Promise<string> {
    return core.opAsync("next_instance_output", instanceUuid);
}

export function nextPlayerMessage(instanceUuid: string): Promise<PlayerMessage> {
    return core.opAsync("next_instance_player_message", instanceUuid);
}

export function nextInstanceSystemMessage(instanceUuid: string): Promise<string> {
    return core.opAsync("next_instance_system_message", instanceUuid);
}


/**  Notifies the caller that the macro wishes to be run in the background.
 * 
* This is a no-op if the macro is already running in the background, or called multiple times.

* This function DOES NOT exit the macro.
*/
export function emitDetach(pid: TaskPID) {
    ops.emit_detach(pid);
}

export function emitConsoleOut(line: string, instanceUuid: string) {
    InstanceControl.getInstanceName(instanceUuid).then((name) => {
        ops.emit_console_out(instanceUuid, name, line);
    }
    )
}

export function emitStateChange(state: InstanceState, instanceName: string, instanceUuid: string) {
    ops.emit_state_change(instanceUuid, instanceName, state);
}

export function emitProgressionEventStart(progressionName : string, total : number | null, inner : ProgressionStartValue | null) : string {
    return ops.emit_progression_event_start(progressionName, total, inner);
}

export function emitProgressiontEventUpdate(eventId : string, progressMessage : string, progress : number) {
    ops.emit_progression_event_update(eventId, progressMessage, progress);
}

export function emitProgressionEventEnd(eventId : string, success : boolean, message : string | null, inner : ProgressionEndValue | null) {
    ops.emit_progression_event_end(eventId, success, message, inner);
}