import { isUserAuthorized } from './UserInfo';
import { useUserInfo } from 'data/UserInfo';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { useContext, useEffect, useRef, useState } from 'react';
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
  | 'live-no-buffer'
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
  const { address, port, apiVersion, isReady, token } = useContext(LodestoneContext);
  const [consoleLog, setConsoleLog] = useState<Event[]>([]);
  const [status, setStatus] = useState<ConsoleStreamStatus>('loading'); //callbacks should use statusRef.current instead of status
  const statusRef = useRef<ConsoleStreamStatus>('loading');
  statusRef.current = status;

  const { data: userInfo } = useUserInfo();
  const canAccessConsole = isUserAuthorized(userInfo, 'CanAccessConsole', uuid);

  const mergeConsoleLog = (newLog: Event[]) => {
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
    if (!canAccessConsole) {
      setStatus('no-permission');
      return;
    }
    setStatus('loading');
    console.log(status, statusRef.current);
    console.log(token);
    console.log(userInfo);

    const websocket = new WebSocket(
      `ws://${address}:${
        port ?? 3000
      }/api/${apiVersion}/instance/${uuid}/console/stream?token=Bearer ${token}`
    );

    websocket.onopen = () => {
      if (statusRef.current === 'loading') setStatus('live-no-buffer');
      if (statusRef.current === 'buffered') setStatus('live');
    };

    websocket.onmessage = (messageEvent) => {
      const event: Event = JSON.parse(messageEvent.data);
      mergeConsoleLog([event]);
    };

    websocket.onclose = (event) => {
      setStatus(event.code === 1000 ? 'closed' : 'error');
    };

    return () => {
      websocket.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isReady, address, port, apiVersion, uuid, canAccessConsole]);

  useEffect(() => {
    if (!isReady) return;
    if (!canAccessConsole) return;
    axios
      .get(`/instance/${uuid}/console/buffer`)
      .then((response) => {
        mergeConsoleLog(response.data);
        if (statusRef.current === 'loading') setStatus('buffered');
        if (statusRef.current === 'live-no-buffer') setStatus('live');
      })
      .catch((e) => {
        console.error(e);
      });
  }, [canAccessConsole, isReady, uuid]);
  return {
    consoleLog,
    consoleStatus: status,
  };
};
