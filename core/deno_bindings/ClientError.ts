// This file was created MANUALLY
// We have a custom Serialize implementation 
import type { ErrorKind } from "./ErrorKind.ts";

export interface ClientError { kind: ErrorKind, causes: Array<String>, }
