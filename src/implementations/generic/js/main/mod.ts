import { AtomInstance } from "./libs/atom_instance.ts";
import { procedure_bridge, init_instance } from "./libs/procedure_bridge.ts";

export function init(instance: AtomInstance) {
    init_instance(instance);
}

export function run() {
    procedure_bridge();
}