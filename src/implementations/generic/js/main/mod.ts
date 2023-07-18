import { AtomInstance } from "./libs/atom_instance.ts";
import { procedure_bridge } from "./libs/procedure_bridge.ts";


export function run(instance: AtomInstance) {
    procedure_bridge(instance);
}