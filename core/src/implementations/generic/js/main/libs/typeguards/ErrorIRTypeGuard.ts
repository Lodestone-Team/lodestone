import { ErrorIR } from "../bindings/ErrorIR.ts";
import { ErrorKindIR } from "../bindings/ErrorKindIR.ts";

export function isErrorKindIR(value: unknown): value is ErrorKindIR {
  // export type ErrorKindIR = "NotFound" | "UnsupportedOperation" | "BadRequest" | "Internal";
  return typeof value === "string" &&
    ["NotFound", "UnsupportedOperation", "BadRequest", "Internal"].includes(
      value,
    );
}

export function isErrorIR(value: unknown): value is ErrorIR {
  return typeof value === "object" && value !== null && "kind" in value &&
    "source" in value && isErrorKindIR(value.kind) &&
    typeof value.source === "string";
}
