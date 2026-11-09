import { coreWebSocketUrl, getSnowflakeTimestamp } from './../utils/util';
import { InstanceEvent } from './../bindings/InstanceEvent';
import { match, otherwise } from 'variant';
import { useUserAuthorized } from 'data/UserInfo';
import axios from 'axios';
import {
  useContext,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react';
import { LodestoneContext } from './LodestoneContext';
import { ClientEvent } from 'bindings/ClientEvent';
import { openReconnectingWebSocket } from 'utils/websocket';
import { ConsoleHistoryPage } from 'bindings/ConsoleHistoryPage';
import { ConsoleEvent } from 'utils/consoleLog';
import { ConsoleLogSource, ConsoleLogStore } from 'utils/consoleLogStore';

export type { ConsoleEvent };

const noLines: ConsoleEvent[] = [];

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

const consoleLogSource: ConsoleLogSource = {
  buffer: (uuid) =>
    axios
      .get<ClientEvent[]>(`/instance/${uuid}/console/buffer`)
      .then((response) => response.data),
  history: (uuid, before, limit) =>
    axios
      .get<ConsoleHistoryPage>(`/instance/${uuid}/console/history`, {
        params: { limit, ...(before !== null && { before }) },
      })
      .then((response) => response.data),
  toConsoleEvents: (events) =>
    events.filter(isConsoleOutput).map(toConsoleEvent),
};

/**
 * Does two things:
 * 1. fetches the console buffer (history), and older history page by page
 * 2. opens a websocket connection to the server and subscribes to the console stream,
 *    reconnecting with backoff (and refetching the buffer to fill the gap) if it drops
 *
 * Answers that arrive after the instance changed are dropped, see
 * `ConsoleLogStore`.
 *
 * @param uuid the uuid of the instance to subscribe to
 */
export const useConsoleStream = (uuid: string) => {
  const { core, token } = useContext(LodestoneContext);
  const [store] = useState(() => new ConsoleLogStore(consoleLogSource));
  const state = useSyncExternalStore(
    store.subscribe,
    store.getState,
    store.getState
  );
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

  useEffect(() => {
    if (!canAccessConsole) {
      store.close();
      setStatus('no-permission');
      return;
    }
    if (!token) return;
    setStatus('loading');
    const session = store.open(uuid);
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

    store.fetchBuffer().then((result) => {
      if (cancelled || result === 'stale') return;
      // nothing in memory: the core may have restarted since it stored lines
      if (store.isEmpty()) store.loadOlder();
      buffer = result;
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
          store.fetchBuffer().then((result) => {
            if (cancelled || result === 'stale' || !socketOpen) return;
            buffer = result;
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
        store.addLive(session, [event]);
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
      store.close();
      websocket.close();
    };
  }, [core, token, uuid, canAccessConsole, store]);

  // until the effect has opened a newly selected instance, the store still
  // holds the previous one's lines, which are not shown
  const current = state.uuid === uuid;
  return {
    consoleLog: current ? state.log : noLines,
    consoleStatus: status,
    loadOlder: store.loadOlder,
    loadingOlder: current && state.loadingOlder,
    hasOlder: current && state.hasOlder,
    historyFailed: current && state.historyFailed,
    historyLimitReached: current && state.limitReached,
  };
};
