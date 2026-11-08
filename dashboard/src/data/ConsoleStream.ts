import { coreWebSocketUrl, getSnowflakeTimestamp } from './../utils/util';
import { InstanceEvent } from './../bindings/InstanceEvent';
import { match, otherwise } from 'variant';
import { useUserAuthorized } from 'data/UserInfo';
import axios from 'axios';
import { useCallback, useContext, useEffect, useRef, useState } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { ClientEvent } from 'bindings/ClientEvent';
import { openReconnectingWebSocket } from 'utils/websocket';
import { ConsoleHistoryPage } from 'bindings/ConsoleHistoryPage';
import {
  ConsoleEvent,
  MAX_CONSOLE_LINES,
  MAX_LOADED_LINES,
  mergeConsoleEvents,
  olderSnowflake,
} from 'utils/consoleLog';

export type { ConsoleEvent };

/** Lines asked for in one request for older history (the core caps it as well). */
const HISTORY_PAGE_SIZE = 200;

export type ConsoleStreamStatus =
  | 'no-permission'
  | 'loading'
  | 'buffered'
  | 'live'
  | 'live-no-buffer'
  | 'closed'
  | 'error';

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

  // Older history, fetched page by page from the core's event database. The
  // cursor is the oldest snowflake received so far, from the buffer, the live
  // stream or an earlier page, so a page never repeats a line already shown.
  const [hasOlder, setHasOlder] = useState(true);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [historyFailed, setHistoryFailed] = useState(false);
  // counts the times older lines were put in front of the log
  const [olderVersion, setOlderVersion] = useState(0);
  const oldestRef = useRef<string | null>(null);
  const maxLinesRef = useRef(MAX_CONSOLE_LINES);
  const logLengthRef = useRef(0);
  const hasOlderRef = useRef(true);
  const loadingOlderRef = useRef(false);
  // changes with the instance, so that a page of the previous one is dropped
  const generationRef = useRef(0);

  useEffect(() => {
    logLengthRef.current = consoleLog.length;
  }, [consoleLog]);

  useEffect(() => {
    generationRef.current += 1;
    oldestRef.current = null;
    maxLinesRef.current = MAX_CONSOLE_LINES;
    hasOlderRef.current = true;
    loadingOlderRef.current = false;
    setHasOlder(true);
    setLoadingOlder(false);
    setHistoryFailed(false);
    setConsoleLog([]);
  }, [uuid]);

  const clearConsoleLog = () => {
    setConsoleLog([]);
  };

  const mergeConsoleLog = (newLog: ClientEvent[]) => {
    for (const event of newLog) {
      oldestRef.current = olderSnowflake(oldestRef.current, event.snowflake);
    }
    const consoleEvents = newLog.filter(isConsoleOutput).map(toConsoleEvent);
    setConsoleLog((oldLog) =>
      mergeConsoleEvents(oldLog, consoleEvents, maxLinesRef.current)
    );
  };

  /** Fetches the page of console lines before the oldest one received. */
  const loadOlder = useCallback(async () => {
    if (loadingOlderRef.current || !hasOlderRef.current) return;
    loadingOlderRef.current = true;
    setLoadingOlder(true);
    setHistoryFailed(false);
    const generation = generationRef.current;
    try {
      const response = await axios.get<ConsoleHistoryPage>(
        `/instance/${uuid}/console/history`,
        {
          params: {
            limit: HISTORY_PAGE_SIZE,
            ...(oldestRef.current !== null && { before: oldestRef.current }),
          },
        }
      );
      if (generation !== generationRef.current) return;
      const { events, has_more } = response.data;
      // lines that are not console output still move the cursor
      for (const event of events) {
        oldestRef.current = olderSnowflake(oldestRef.current, event.snowflake);
      }
      const consoleEvents = events.filter(isConsoleOutput).map(toConsoleEvent);
      // the loaded history is kept, up to a limit
      const total = logLengthRef.current + consoleEvents.length;
      maxLinesRef.current = Math.min(
        MAX_LOADED_LINES,
        Math.max(maxLinesRef.current, total)
      );
      setConsoleLog((oldLog) =>
        mergeConsoleEvents(oldLog, consoleEvents, maxLinesRef.current)
      );
      if (consoleEvents.length > 0) setOlderVersion((version) => version + 1);
      hasOlderRef.current = has_more && total < MAX_LOADED_LINES;
      setHasOlder(hasOlderRef.current);
    } catch (e) {
      if (generation !== generationRef.current) return;
      console.error(e);
      setHistoryFailed(true);
    } finally {
      if (generation === generationRef.current) {
        loadingOlderRef.current = false;
        setLoadingOlder(false);
      }
    }
  }, [uuid]);

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
      // nothing in memory: the core may have restarted since it stored lines
      if (oldestRef.current === null) loadOlder();
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
  }, [core, token, uuid, canAccessConsole, fetchBuffer, loadOlder]);

  return {
    consoleLog,
    consoleStatus: status,
    clearConsoleLog,
    loadOlder,
    loadingOlder,
    hasOlder,
    historyFailed,
    olderVersion,
  };
};
