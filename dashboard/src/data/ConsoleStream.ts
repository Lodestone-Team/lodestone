import { coreWebSocketUrl, getSnowflakeTimestamp } from './../utils/util';
import { InstanceEvent } from './../bindings/InstanceEvent';
import { match, otherwise } from 'variant';
import { useUserAuthorized } from 'data/UserInfo';
import axios from 'axios';
import { useCallback, useContext, useEffect, useRef, useState } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { ClientEvent } from 'bindings/ClientEvent';
import { openReconnectingWebSocket } from 'utils/websocket';

/** Only the newest lines are kept, older ones are dropped. */
export const MAX_CONSOLE_LINES = 5000;

export type ConsoleStreamStatus =
  | 'no-permission'
  | 'loading'
  | 'buffered'
  | 'live'
  | 'live-no-buffer'
  | 'closed'
  | 'error';

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
const toConsoleEvent = (event: ClientEvent): ConsoleEvent => {
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

  return {
    timestamp: getSnowflakeTimestamp(event.snowflake),
    snowflake: event.snowflake,
    detail: event.details,
    uuid: event_inner.instance_uuid,
    name: event_inner.instance_name,
    message: message,
  };
};

const isConsoleOutput = (event: ClientEvent) =>
  event.event_inner.type === 'InstanceEvent' &&
  event.event_inner.instance_event_inner.type === 'InstanceOutput';

// snowflakes are 64-bit, so compare them as BigInt
const compareSnowflakes = (a: string, b: string) => {
  const x = BigInt(a);
  const y = BigInt(b);
  return x < y ? -1 : x > y ? 1 : 0;
};

/**
 * Merges new lines into the log, ordered by snowflake (time), without
 * duplicates, keeping at most MAX_CONSOLE_LINES.
 * Live lines are almost always newer than the last line, which is checked in
 * O(1). Only an overlapping batch (e.g. the buffer arriving after live lines)
 * needs the full merge.
 */
export const mergeConsoleEvents = (
  oldLog: ConsoleEvent[],
  newEvents: ConsoleEvent[]
): ConsoleEvent[] => {
  if (newEvents.length === 0) return oldLog;
  let merged: ConsoleEvent[];
  let last = oldLog.length > 0 ? oldLog[oldLog.length - 1].snowflake : null;
  const appendable = newEvents.every((event) => {
    if (last !== null && compareSnowflakes(event.snowflake, last) <= 0)
      return false;
    last = event.snowflake;
    return true;
  });
  if (appendable) {
    merged = [...oldLog, ...newEvents];
  } else {
    const seen = new Set<string>();
    merged = [...oldLog, ...newEvents]
      .filter((event) => {
        if (seen.has(event.snowflake)) return false;
        seen.add(event.snowflake);
        return true;
      })
      .sort((a, b) => compareSnowflakes(a.snowflake, b.snowflake));
  }
  return merged.length > MAX_CONSOLE_LINES
    ? merged.slice(merged.length - MAX_CONSOLE_LINES)
    : merged;
};

/**
 * Does two things:
 * 1. fetches the console buffer (history)
 * 2. opens a websocket connection to the server and subscribes to the console stream,
 *    reconnecting with backoff (and refetching the buffer to fill the gap) if it drops
 *
 * @param uuid the uuid of the instance to subscribe to
 */
export const useConsoleStream = (uuid: string) => {
  const { core, token } = useContext(LodestoneContext);
  const [consoleLog, setConsoleLog] = useState<ConsoleEvent[]>([]);
  const [status, setStatusInner] = useState<ConsoleStreamStatus>('loading'); //callbacks should use statusRef.current instead of status
  const statusRef = useRef<ConsoleStreamStatus>('loading');
  const setStatus = (newStatus: ConsoleStreamStatus) => {
    statusRef.current = newStatus;
    setStatusInner(newStatus);
  };
  const canAccessConsole = useUserAuthorized(
    'can_access_instance_console',
    uuid
  );

  const clearConsoleLog = () => {
    setConsoleLog([]);
  };

  const mergeConsoleLog = (newLog: ClientEvent[]) => {
    const consoleEvents = newLog.filter(isConsoleOutput).map(toConsoleEvent);
    setConsoleLog((oldLog) => mergeConsoleEvents(oldLog, consoleEvents));
  };

  // resolves to true if the buffer was fetched
  const fetchBuffer = useCallback(
    () =>
      axios
        .get<ClientEvent[]>(`/instance/${uuid}/console/buffer`)
        .then((response) => {
          mergeConsoleLog(response.data);
          return true;
        })
        .catch((e) => {
          console.error(e);
          return false;
        }),
    [uuid]
  );

  useEffect(() => {
    if (!canAccessConsole) {
      setStatus('no-permission');
      return;
    }
    if (!token) return;
    setStatus('loading');
    let cancelled = false;
    // The history and the live stream arrive in either order. The status only
    // moves on once the history has been fetched (or failed): the socket
    // usually opens first, and showing "live but failed to fetch history"
    // until the history arrived made the status dot flash yellow every time.
    let socketOpen = false;
    let buffer: 'pending' | 'fetched' | 'failed' = 'pending';
    const updateStatus = () => {
      if (cancelled) return;
      if (buffer === 'pending') {
        setStatus('loading');
      } else if (socketOpen) {
        setStatus(buffer === 'fetched' ? 'live' : 'live-no-buffer');
      } else if (buffer === 'fetched') {
        setStatus('buffered');
      }
      // failed and not connected yet: stays loading until the socket opens
    };

    fetchBuffer().then((ok) => {
      if (cancelled) return;
      buffer = ok ? 'fetched' : 'failed';
      // the socket may have dropped meanwhile; that status stays
      if (socketOpen || statusRef.current === 'loading') updateStatus();
    });

    const websocket = openReconnectingWebSocket({
      url: () =>
        coreWebSocketUrl(
          core,
          `/instance/${uuid}/console/stream?token=${encodeURIComponent(
            `Bearer ${token}`
          )}`
        ),
      onOpen: (_, isReconnect) => {
        socketOpen = true;
        if (isReconnect) {
          // fill in the lines sent while we were disconnected
          buffer = 'pending';
          updateStatus();
          fetchBuffer().then((ok) => {
            if (cancelled || !socketOpen) return;
            buffer = ok ? 'fetched' : 'failed';
            updateStatus();
          });
          return;
        }
        updateStatus();
      },
      onMessage: (messageEvent) => {
        let event: ClientEvent;
        try {
          event = JSON.parse(messageEvent.data);
        } catch (e) {
          console.error('Invalid event from the console stream', e);
          return;
        }
        mergeConsoleLog([event]);
      },
      onClose: (event) => {
        socketOpen = false;
        if (event.code === 1000) {
          // closed by the server on purpose
          setStatus('closed');
          return false;
        }
        setStatus('error');
        return true;
      },
    });

    return () => {
      cancelled = true;
      websocket.close();
    };
  }, [core, token, uuid, canAccessConsole, fetchBuffer]);

  return {
    consoleLog,
    consoleStatus: status,
    clearConsoleLog,
  };
};
