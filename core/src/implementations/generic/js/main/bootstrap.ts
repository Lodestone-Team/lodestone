// This file "links" a concrete implementation of the Atom instance to the procedure bridge.
// Lodestone generates it from this template: the two module placeholders below are replaced
// with string literals (see `bootstrap_code` in ../../mod.rs).

import * as a from __ATOM_MODULE__;
import { run } from __BRIDGE_MODULE__;
const instance = new a.default();
run(instance);
