import { useQuery, useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { useContext, useEffect, useRef, useState } from 'react';
import { useToken } from 'utils/hooks';
import { LodestoneContext } from './LodestoneContext';

export type EventInner = { InstanceOutput: string };

export interface Event {
  event_inner: EventInner;
  instance_uuid: string;
  instance_name: string;
  details: string;
  timestamp: number;
  idempotency: string;
}

export type ConsoleStreamStatus =
  | 'no-permission'
  | 'loading'
  | 'buffered'
  | 'live'
  | 'closed'
  | 'error';

/**
 * Does two things:
 * 1. calls useEffect to fetch the console stream
 * 2. calls useEffect to open a websocket connection to the server and subscribe to the console stream
 *   the websocket will update the query cache with new console output
 *
 * Note that we don't use the useQuery hook here and we are managing the query cache manually
 *
 * @param uuid the uuid of the instance to subscribe to
 * @return whatever useQuery returns
 */
export const useConsoleStream = (uuid: string) => {
  const { address, port, apiVersion, isReady } = useContext(LodestoneContext);
  const { token } = useToken();
  const [consoleLog, setConsoleLog] = useState<Event[]>([]);
  const [status, setStatus] = useState<ConsoleStreamStatus>('loading'); //callbacks should use statusRef.current instead of status
  const statusRef = useRef<ConsoleStreamStatus>('loading');
  statusRef.current = status;

  const mergeConsoleLog = (newLog: Event[]) => {
    console.log('merged console log', newLog);
    setConsoleLog((oldLog) => {
      const mergedLog = [...oldLog, ...newLog];
      // TODO: implement snowflake ids and use those instead of idempotency
      // this is slow ik
      return mergedLog.filter((event, index) => {
        return (
          mergedLog.findIndex((e) => e.idempotency === event.idempotency) ===
          index
        );
      });
    });
  };

  useEffect(() => {
    if (!isReady) {
      setStatus('loading');
      return;
    }
    if (!token) {
      // TODO: proper permission handling
      setStatus('no-permission');
      return;
    }
    setStatus('loading');

    const websocket = new WebSocket(
      `ws://${address}:${
        port ?? 3000
      }/api/${apiVersion}/instance/${uuid}/console/stream?token=Bearer ${token}`
    );

    websocket.onopen = () => {
      setStatus('live');
    };

    websocket.onmessage = (messageEvent) => {
      const event: Event = JSON.parse(messageEvent.data);
      mergeConsoleLog([event]);
    };

    websocket.onclose = (event) => {
      if (event.code === 1000) {
        setStatus('closed');
      } else {
        setStatus('error');
      }
    };

    try {
      axios
        .get(`/instance/${uuid}/console/buffer`)
        .then((response) => {
          mergeConsoleLog(response.data);
        })
        .finally(() => {
          if (statusRef.current === 'loading') setStatus('buffered');
        });
    } catch (e) {
      console.error(e);
      setStatus('error');
    }

    return () => {
      websocket.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isReady, address, port, apiVersion, uuid, token]);

  return {
    consoleLog,
    consoleStatus: status,
  };
};
