import { instanceUUID, taskPid } from "../prelude/prelude.ts";
import { InstanceState } from "../../../deno_bindings/InstanceState.ts";
import { PerformanceReport } from "../../../deno_bindings/PerformanceReport.ts";
import { Player } from "../../../deno_bindings/Player.ts";
import { Game } from "../../../deno_bindings/Game.ts";
// deno-lint-ignore no-explicit-any
declare const Deno: any;
const core = Deno[Deno.internal].core;

export function startInstance(block: boolean, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("start_instance", instanceUuid, taskPid(), block);
}

export function stopInstance(block: boolean, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("stop_instance", instanceUuid, taskPid(), block);
}

export function restartInstance(block: boolean, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("restart_instance", instanceUuid, taskPid(), block);
}

export function killInstance(instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("kill_instance", instanceUuid, taskPid());
}

export function getInstanceState(instanceUuid: string = instanceUUID()!): Promise<InstanceState> {
    return core.opAsync("get_instance_state", instanceUuid);
}

export function sendCommand(command: string, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("send_command", instanceUuid, command);
}

export function monitorInstance(instanceUuid: string = instanceUUID()!): Promise<PerformanceReport> {
    return core.opAsync("monitor_instance", instanceUuid);
}

export function getInstancePlayerCount(instanceUuid: string = instanceUUID()!): Promise<number> {
    return core.opAsync("get_instance_player_count", instanceUuid);
}

export function getInstanceMaxPlayers(instanceUuid: string = instanceUUID()!): Promise<number> {
    return core.opAsync("get_instance_max_players", instanceUuid);
}

export function getInstancePlayerList(instanceUuid: string = instanceUUID()!): Promise<Player[]> {
    return core.opAsync("get_instance_player_list", instanceUuid);
}

export function getInstanceName(instanceUuid: string = instanceUUID()!): Promise<string> {
    return core.opAsync("get_instance_name", instanceUuid);
}

export function getInstanceGame(instanceUuid: string = instanceUUID()!): Promise<Game> {
    return core.opAsync("get_instance_game", instanceUuid);
}

export function getInstanceGameVersion(instanceUuid: string = instanceUUID()!): Promise<string> {
    return core.opAsync("get_instance_game_version", instanceUuid);
}

export function getInstanceDescription(instanceUuid: string = instanceUUID()!): Promise<string> {
    return core.opAsync("get_instance_description", instanceUuid);
}

export function getInstancePort(instanceUuid: string = instanceUUID()!): Promise<number> {
    return core.opAsync("get_instance_port", instanceUuid);
}

export function getInstancePath(instanceUuid: string = instanceUUID()!): Promise<string> {
    return core.opAsync("get_instance_path", instanceUuid);
}

export function setInstanceName(name: string, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("set_instance_name", instanceUuid, name);
}

export function setInstanceDescription(description: string, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("set_instance_description", instanceUuid, description);
}

export function setInstancePort(port: number, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("set_instance_port", instanceUuid, port);
}

export function setInstanceAutoStart(autoStart: boolean, instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("set_instance_auto_start", instanceUuid, autoStart);
}

export function isRconAvailable(instanceUuid: string = instanceUUID()!): Promise<boolean> {
    return core.opAsync("is_rcon_available", instanceUuid);
}

export function sendRconCommand(command: string, instanceUuid: string = instanceUUID()!): Promise<string> {
    return core.opAsync("send_rcon_command", instanceUuid, command);
}

export function waitTillRconAvailable(instanceUuid: string = instanceUUID()!): Promise<void> {
    return core.opAsync("wait_till_rcon_available", instanceUuid);
}