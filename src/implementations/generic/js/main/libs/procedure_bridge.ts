// Some glue code that implements the procedure bridge on the Deno side.
// !!do NOT edit this file manually (other than maybe the imports for implementation functions) unless you know what you're doing!!
// The procedure bridge is the main driver of the script.
// It's responsible for receiving procedure calls from the host (Rust side), and calling the corresponding implementation functions.

// deno-lint-ignore no-explicit-any
declare const Deno: any;
const { core } = Deno;
const { ops } = core;
import { ProcedureCallResultIR } from "./bindings/ProcedureCallResultIR.ts";
import { ProcedureCall } from "./bindings/ProcedureCall.ts";
// This is the only code you should edit.
import { isErrorIR } from "./typeguards/ErrorIRTypeGuard.ts";
import { ProcedureCallResultInner } from "./bindings/ProcedureCallResultInner.ts";

import { isTConfig, isTMacro, isTPlayer, isTServer } from "./utils.ts";
import { AtomInstance } from "./atom_instance.ts";

let instance!: AtomInstance;

export function init_instance(i: AtomInstance) {
    instance = i;
}

const emit_result = (result: ProcedureCallResultIR) =>
    ops.emit_result(result);

async function tPlayerHandle(procedure: ProcedureCall) {
    const inner = procedure.inner;
    let ret: ProcedureCallResultInner = "Void";
    try {
        if (inner.type === "GetPlayerCount") {
            ret = {
                Num: await instance.playerCount(),
            };
        } else if (inner.type === "GetPlayerList") {
            ret = {
                Player: await instance.playerList(),
            };
        } else if (inner.type === "GetMaxPlayerCount") {
            ret = {
                Num: await instance.maxPlayerCount(),
            };
        }
    } catch (e) {
        if (isErrorIR(e)) {
            emit_result({
                id: procedure.id,
                success: false,
                procedure_call_kind: inner.type,
                inner: null,
                error: e,
            });
        } else {
            emit_result({
                id: procedure.id,
                success: false,
                procedure_call_kind: inner.type,
                inner: null,
                error: {
                    kind: "Internal",
                    source: e.toString(),
                }
            });

        }
        return;
    }
    emit_result({
        id: procedure.id,
        success: true,
        procedure_call_kind: inner.type,
        inner: ret,
        error: null,
    });

}


async function tConfigHandle(procedure: ProcedureCall) {
    const inner = procedure.inner;
    let ret: ProcedureCallResultInner = "Void";
    try {
        if (inner.type === "GetName") {
            ret = {
                String: await instance.name(),
            }
        } else if (inner.type === "GetDescription") {
            ret = {
                String: await instance.description(),
            };
        } else if (inner.type === "GetVersion") {
            ret = {
                String: await instance.version(),
            }
        } else if (inner.type === "GetGame") {
            ret = {
                Game: await instance.game(),
            }
        } else if (inner.type === "GetPort") {
            ret = {
                Num: await instance.port(),
            }
        } else if (inner.type === "GetAutoStart") {
            ret = {
                Bool: await instance.getAutoStart(),
            }
        } else if (inner.type === "GetRestartOnCrash") {
            // ret = {
            //     Bool: await getRestartOnCrash(),
            // }
        } else if (inner.type === "GetConfigurableManifest") {
            ret = {
                ConfigurableManifest: await instance.configurableManifest(),
            }
        } else if (inner.type === "SetName") {
            await instance.setName(inner.new_name);
        } else if (inner.type === "SetDescription") {
            await instance.setDescription(inner.new_description);
        } else if (inner.type === "SetPort") {
            await instance.setPort(inner.new_port);
        } else if (inner.type === "SetAutoStart") {
            await instance.setAutoStart(inner.new_auto_start);
        } else if (inner.type === "SetRestartOnCrash") {
            // await setRestartOnCrash(inner.new_restart_on_crash);
        } else if (inner.type === "UpdateConfigurable") {
            await instance.updateConfigurable(inner.section_id, inner.setting_id, inner.new_value);
        }
    } catch (e) {
        if (isErrorIR(e)) {
            emit_result(
                {
                    id: procedure.id,
                    success: false,
                    procedure_call_kind: inner.type,
                    error: e,
                    inner: null
                },
            );
        } else {
            emit_result(
                {
                    id: procedure.id,
                    success: false,
                    procedure_call_kind: inner.type,
                    error: {
                        kind: "Internal",
                        source: e.message,
                    },
                    inner: null
                },
            );
        }
        return;
    }
    emit_result(
        {
            id: procedure.id,
            success: true,
            procedure_call_kind: inner.type,
            error: null,
            inner: ret,
        },
    );
}

async function tServerHandle(procedure: ProcedureCall) {
    const inner = procedure.inner;
    let ret: ProcedureCallResultInner = "Void";
    try {
        if (inner.type === "StartInstance") {
            instance.start(inner.caused_by, inner.block);
        } else if (inner.type === "StopInstance") {
            instance.stop(inner.caused_by, inner.block);
        } else if (inner.type === "RestartInstance") {
            instance.restart(inner.caused_by, inner.block);
        } else if (inner.type === "KillInstance") {
            instance.kill(inner.caused_by);
        } else if (inner.type === "GetState") {
            await instance.state();
        } else if (procedure.inner.type === "SendCommand") {
            await instance.sendCommand(procedure.inner.command, procedure.inner.caused_by)
        } else if (procedure.inner.type === "Monitor") {
            ret = {
                Monitor: await instance.monitor()
            };
        }
    } catch (e) {
        if (isErrorIR(e)) {
            emit_result(
                {
                    id: procedure.id,
                    success: false,
                    procedure_call_kind: inner.type,
                    error: e,
                    inner: null
                },
            );
        } else {
            emit_result(
                {
                    id: procedure.id,
                    success: false,
                    procedure_call_kind: inner.type,
                    error: {
                        kind: "Internal",
                        source: e.message,
                    },
                    inner: null
                },
            );
        }
        return;
    }
    emit_result(
        {
            id: procedure.id,
            success: true,
            procedure_call_kind: inner.type,
            error: null,
            inner: ret
        },
    );
}


// Your script should call this function once, and only once when it's ready to receive procedure calls.
// Do NOT await this function as it will block forever.
export async function procedure_bridge() {
    // This function will throw if it's called more than once.
    ops.proc_bridge_ready();
    while (true) {
        const procedure: ProcedureCall = await core.opAsync("on_procedure");
        const inner = procedure.inner;
        let ret: ProcedureCallResultInner = "Void";
        if (isTConfig(inner)) {
            await tConfigHandle(procedure);
        } else if (isTServer(inner)) {
            await tServerHandle(procedure);
        } else if (isTPlayer(inner)) {
            await tPlayerHandle(procedure);
        } else
            try {
                if (inner.type === "GetSetupManifest") {
                    ret = {
                        SetupManifest: await instance.setupManifest()
                    }
                }
                if (inner.type === "SetupInstance") {
                    await instance.setup(inner.setup_value, inner.dot_lodestone_config, inner.path)

                } else if (inner.type == "RestoreInstance") {
                    await instance.restore(inner.dot_lodestone_config, inner.path);
                }
            } catch (e) {
                if (isErrorIR(e)) {
                    emit_result(
                        {
                            id: procedure.id,
                            success: false,
                            procedure_call_kind: inner.type,
                            inner: null,
                            error: e,
                        },
                    );
                } else {
                    emit_result(
                        {
                            id: procedure.id,
                            success: false,
                            procedure_call_kind: inner.type,
                            inner: null,
                            error: {
                                kind: "Internal",
                                source: e.toString(),
                            },
                        },
                    );
                }
                continue;
            }

        emit_result(
            {
                id: procedure.id,
                success: true,
                procedure_call_kind: inner.type,
                inner: ret,
                error: null,
            },
        );
    }
}
