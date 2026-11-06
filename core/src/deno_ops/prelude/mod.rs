use std::borrow::Cow;

use deno_core::{op2, Extension};

use crate::prelude::VERSION;

#[op2]
#[string]
fn get_lodestone_version() -> String {
    VERSION.with(|v| v.to_string())
}

pub fn register_prelude_ops(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    worker_options.extensions.push(Extension {
        name: "prelude_ops",
        ops: Cow::Owned(vec![get_lodestone_version()]),
        ..Default::default()
    });
}
