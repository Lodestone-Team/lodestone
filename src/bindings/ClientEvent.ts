// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { EventInner } from './EventInner';

export interface ClientEvent {
  event_inner: EventInner;
  details: string;
  snowflake: bigint;
  snowflake_str: string;
}
