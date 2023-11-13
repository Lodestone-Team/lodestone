import { getSnowflakeTimestamp } from './../utils/util';
import { InstanceEvent } from './../bindings/InstanceEvent';
import { ConsoleQueryParams } from './../bindings/ConsoleQueryParams';
import { match, otherwise } from 'variant';
import { ClientEvent } from 'bindings/ClientEvent';
import { getConsoleEvents } from 'utils/apis';

// simplified version of a ClientEvent with just InstanceOutput
export type ConsoleEvent = {
  timestamp: number;
  snowflake: string;
  detail: string;
  uuid: string;
  name: string;
  message: string;
};

// function to convert a ClientEvent to a ConsoleEvent
export const toConsoleEvent = (event: ClientEvent): ConsoleEvent => {
  const event_inner: InstanceEvent = match(
    event.event_inner,
    otherwise(
      {
        InstanceEvent: (instanceEvent) => instanceEvent,
      },
      () => {
        throw new Error('Expected InstanceEvent');
      }
    )
  );

  const message = match(
    event_inner.instance_event_inner,
    otherwise(
      {
        InstanceOutput: (instanceOutput) => instanceOutput.message,
      },
      () => {
        throw new Error('Expected InstanceOutput');
      }
    )
  );

  // console.log(event.snowflake);
  // getConsoleEvents(event_inner.instance_uuid, { start_snowflake_id: event.snowflake as unknown as bigint, count: 5 })
  //   .then((e) => console.log(e));



  return {
    timestamp: getSnowflakeTimestamp(event.snowflake),
    snowflake: event.snowflake,
    detail: event.details,
    uuid: event_inner.instance_uuid,
    name: event_inner.instance_name,
    message: message,
  };
};
