use deno_core::op;

use crate::prelude::VERSION;

#[op]
fn get_lodestone_version() -> String {
    VERSION.with(|v| v.to_string())
}

pub fn register_prelude_ops(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    worker_options.extensions.push(
        deno_core::Extension::builder("prelude_ops")
            .ops(vec![get_lodestone_version::decl()])
            .build(),
    );
}
