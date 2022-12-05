((globalThis) => {
    const { core } = Deno;
    const { ops } = core;

    // core.initializeAsyncOps();
    globalThis.Lodestone = {
        instance: {
            sendStdin(cmd) {
                ops.send_stdin(cmd.toString());
            },
            sendRcon(cmd) {
                return ops.send_rcon(cmd.toString());
            },
            getConfig() {
                return JSON.parse(ops.config());
            },
            async onEvent(event) {
                return JSON.parse(await ops.on_event(event));
            }
        }


    };
})(globalThis);
