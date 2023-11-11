import { LODESTONE_PORT } from './../utils/util';
import { useUserAuthorized } from 'data/UserInfo';
import axios from 'axios';
import { useContext, useEffect, useRef, useState } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { ClientEvent } from 'bindings/ClientEvent';
import { ConsoleEvent, toConsoleEvent} from 'data/ConsoleEvent';

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
  const { core, token } = useContext(LodestoneContext);
  const { address, port, apiVersion, protocol } = core;
  const [consoleLog, setConsoleLog] = useState<ConsoleEvent[]>([]);
  const [status, setStatusInner] = useState<ConsoleStreamStatus>('loading'); //callbacks should use statusRef.current instead of status
  const statusRef = useRef<ConsoleStreamStatus>('loading');
  statusRef.current = status;
  const setStatus = (newStatus: ConsoleStreamStatus) => {
    statusRef.current = newStatus;
    setStatusInner(newStatus);
  };
  const canAccessConsole = useUserAuthorized(
    'can_access_instance_console',
    uuid
  );

  const mergeConsoleLog = (newLog: ClientEvent[]) => {
    setConsoleLog((oldLog) => {
      const consoleEvents = newLog
        .filter((event) => {
          return (
            event.event_inner.type === 'InstanceEvent' &&
            event.event_inner.instance_event_inner.type === 'InstanceOutput'
          );
        })
        .map(toConsoleEvent);

      const mergedLog = [...oldLog, ...consoleEvents];
      // this is slow ik
      return mergedLog.filter(
        (event, index) =>
          mergedLog.findIndex((e) => e.snowflake === event.snowflake) === index
      );
    });
  };

  useEffect(() => {
    if (!canAccessConsole) {
      setStatus('no-permission');
      return;
    }
    setStatus('loading');

    try {
      const websocket = new WebSocket(
        `${protocol === 'https' ? 'wss' : 'ws'}://${address}:${
          port ?? LODESTONE_PORT
        }/api/${apiVersion}/instance/${uuid}/console/stream?token=Bearer ${token}`
      );

      websocket.onopen = () => {
        if (statusRef.current === 'loading') setStatus('live-no-buffer');
        if (statusRef.current === 'buffered') setStatus('live');
      };

      websocket.onmessage = (messageEvent) => {
        const event: ClientEvent = JSON.parse(messageEvent.data);
        mergeConsoleLog([event]);
      };

      websocket.onclose = (event) => {
        setStatus(event.code === 1000 ? 'closed' : 'error');
      };

      return () => {
        websocket.close();
      };
    } catch (e) {
      console.error(e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [address, port, apiVersion, uuid, canAccessConsole]);

  useEffect(() => {
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
  }, [canAccessConsole, uuid]);
  return {
    consoleLog,
    consoleStatus: status,
  };
};
