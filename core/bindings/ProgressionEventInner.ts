// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { ProgressionEndValue } from "./ProgressionEndValue";
import type { ProgressionStartValue } from "./ProgressionStartValue";

export type ProgressionEventInner = { "type": "ProgressionStart", progression_name: string, total: number | null, inner: ProgressionStartValue | null, } | { "type": "ProgressionUpdate", progress_message: string, progress: number, } | { "type": "ProgressionEnd", success: boolean, message: string | null, inner: ProgressionEndValue | null, };