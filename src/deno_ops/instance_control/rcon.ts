import { getInstanceGame, isRconAvailable, sendRconCommand, waitTillRconAvailable } from "./instance_control.ts";

export type RconError = "Unsupported" | "Unavailable" | "Unknown";


export class Rcon {
    private constructor() { }
    /**
     * Attempts to acquire an Rcon instance.
     * 
     * @throws RconError if the Rcon instance could not be acquired.
     * 
     * Some reasons for this error include:
     * 
     * - Rcon server is not available.
     * 
     * - The instance is not a Minecraft Java instance.
     * 
     * - The instance did not have rcon enabled in its server.properties file.
     * 
     * @returns An Rcon instance if successful.
     * 
     */
    static async acquire(): Promise<Rcon> {
        const game = await getInstanceGame();
        if (game.type !== "MinecraftJava") {
            throw "Unsupported";
        }
        if (await isRconAvailable()) {
            return new Rcon();
        } else {
            throw "Unavailable";
        }
    }

    /**
     * Acquires a Rcon instance, the promise will resolve when rcon is available.
     * 
     * @throws RconError if the underlying instance does not support rcon.
     */
    static async waitForAcquisition(): Promise<Rcon> {
        const game = await getInstanceGame();
        if (game.type !== "MinecraftJava") {
            throw "Unsupported";
        }
        await waitTillRconAvailable();
        return new Rcon();
    }

    /**
     * Checks if the Rcon connection is open.
     * 
     * Note: Using this method before sending a command is not recommended as the connection may close between the check and the command.
     * 
     * @returns true if the Rcon connection is open.
     * 
     * @throws RconError if the underlying instance does not support rcon.
     * 
     */
    async isOpen(): Promise<boolean> {
        return await isRconAvailable();
    }
    async send(command: string): Promise<string> {
        return await sendRconCommand(command);
    }
}

