// deno-lint-ignore no-explicit-any
declare const Deno: any;

export const emit_console_out = (event: string) => {
  Deno.core.ops.emit_console_out(event);
};

