import { CausedBy } from "../libs/bindings/CausedBy.ts";
import { InstanceState } from "../libs/bindings/InstanceState.ts";
import { PerformanceReport } from "../libs/bindings/PerformanceReport.ts";
import { readLines } from "https://deno.land/std@0.104.0/io/mod.ts";
import { emit_console_out } from "../libs/events.ts";

let process: Deno.Process<
  { cmd: [string, string]; stdout: "piped"; stdin: "piped"; stderr: "piped" }
>;

/**
 * @param {CausedBy} caused_by - The source that requested this instance to start
 *
 * @param {boolean} block - Whether to block the function from returning until the instance is started
 * 
 * **THIS FUNCTION MUST BE IMPLEMENTED**
 * 
 * This function is called when the instance is requested to start.
 *
 * It can be either from the user or from Lodestone Core for auto start.
 */
// deno-lint-ignore require-await
export async function startInstance(caused_by: CausedBy, block: boolean) {
  // open /home/peter/dev/backend/generic_instance_test/launch.sh with sh and get output
  process = Deno.run({
    cmd: [
      "sh",
      "/home/peter/dev/backend/generic_instance_test/launch.sh",
    ],
    stdout: "piped",
    stdin: "piped",
    stderr: "piped",
  });
  // asynchronously read output line by line and print it

  (async () => {
    for await (const line of readLines(process.stdout)) {
      emit_console_out(line);
    }
  })();
}

/**
 * @param {CausedBy} caused_by - The source that requested this instance to stop
 *
 * @param {boolean} block - Whether to block the function from returning until the instance is stopped
 * 
 * **THIS FUNCTION MUST BE IMPLEMENTED**
 *
 * This function is called when the instance is requested to stop.
 *
 * It can be either from the user or from Lodestone Core when running clean ups.
 *
 * This function should not kill the instance, but instead gracefully stop it.
 *
 * If the instance is already stopped, this function should do nothing.
 *
 * If the instance is in an intermediate state, it is up to the implementation to decide what to do.
 */
// deno-lint-ignore require-await
export async function stopInstance(caused_by: CausedBy, block: boolean) {
  // implementation below
}

/**
 * @param {CausedBy} caused_by - The source that requested this instance to be killed
 * 
 * **THIS FUNCTION MUST BE IMPLEMENTED**
 *
 * Unlike stop_instance, this function should immediately abort the server instance's process
 *
 * If the instance is already stopped, this function should do nothing
 */

// deno-lint-ignore require-await
export async function killInstance(caused_by: CausedBy) {
  throw new Error("Not implemented");
  // implementation below
}

/**
 * 
 * @param caused_by - The source that requested this instance to be killed
 * @param block - Whether or block the function from returning until the instance is restarted
 * 
 * **THIS FUNCTION MUST BE IMPLEMENTED**
 * 
 */

export async function restartInstance(caused_by: CausedBy, block: boolean) {
  throw new Error("Not implemented")
}

/**
 * Return the current state of the instance.
 *
 * Note that the Error state should be reserved for the most serious of errors
 *
 * Such as the server instance executable deleted
 */

// deno-lint-ignore require-await
export async function getState(): Promise<InstanceState> {
  throw new Error("Not implemented");
}

/**
 * @param {string} command - The command to be sent to the instance
 * @param {CausedBy} caused_by - Source of the request
 *
 * It is up to the implementation to decide how the command is delivered
 */
// deno-lint-ignore require-await
export async function sendCommand(
  command: string,
  caused_by: CausedBy,
): Promise<void> {
  throw new Error("Not implemented");
}

/**
 * @returns {PerformanceReport} - The performance report of the instance
 *
 * This function is called when the performance report of the instance is requested
 *
 * It can be either from the frontend dashboard or from Lodestone Core on a schedule
 *
 * **THIS FUNCTION SHOULD NEVER THROW**
 *
 * Return null values instead
 */
// deno-lint-ignore require-await
export async function monitor(): Promise<PerformanceReport> {
  throw new Error("Not implemented");
}
