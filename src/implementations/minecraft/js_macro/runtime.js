const core = Deno[Deno.internal].core;
const { ops } = core;

const Lodestone = {
    instance: {
        sendStdin(cmd) {
            return ops.send_stdin(cmd.toString());
        },
        sendRcon(cmd) {
            return ops.send_rcon(cmd.toString());
        },
        getConfig() {
            return core.JSON.parse(ops.config());
        },
        async onEvent(event) {
            return core.JSON.parse(await core.opAsync("on_event", event));
        },
        async asyncHello() {
            return await core.opAsync("async_hello");
        }
    }


};

globalThis.Lodestone = Lodestone;
