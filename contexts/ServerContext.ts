import {createContext} from "react";

interface ServerContextInterface {
    address: string;
    port: number;
}

export const ServerContext = createContext<ServerContextInterface|null>(null);
