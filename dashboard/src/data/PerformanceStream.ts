import { useContext, useEffect, useState } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { PerformanceReport } from '@bindings/PerformanceReport';
import { useInterval } from 'usehooks-ts';
import { LODESTONE_PORT } from 'utils/util';

const emptyReport: PerformanceReport = {
  memory_usage: null,
  cpu_usage: null,
  disk_usage: null,
  start_time: null,
};

/**
 * A hook to get the performance monitor information of a specific instance.
 * The backend will replay all events from the last 60 seconds, so no manual fetching is required.
 *
 * @param uuid the uuid of the instance to subscribe to
 * @return whatever useQuery returns
 */
export const usePerformanceStream = (uuid: string) => {
  // assuming the backend pings every 1 second, we can buffer 60 seconds of events
  const [buffer, setBuffer] = useState<PerformanceReport[]>(
    Array(60).fill(emptyReport)
  );
  const [lastPing, setLastPing] = useState(Date.now());
  const [latency_s, setLatency_s] = useState(0);
  const [counter, setCounter] = useState(-1);
  const { core } = useContext(LodestoneContext);
  const { address, port, apiVersion, protocol } = core;

  useInterval(() => {
    setLatency_s((Date.now() - lastPing) / 1000);
  }, 1000);

  useEffect(() => {
    try {
      const websocket = new WebSocket(
        `${protocol === 'https' ? 'wss' : 'ws'}://${address}:${
          port ?? LODESTONE_PORT
        }/api/${apiVersion}/monitor/${uuid}`
      );

      websocket.onmessage = (messageEvent) => {
        const event: PerformanceReport = JSON.parse(messageEvent.data);
        setBuffer((oldBuffer) => {
          if (oldBuffer.length > 60) {
            oldBuffer.shift();
          }
          oldBuffer.push(event);
          return oldBuffer;
        });
        setCounter((oldCounter) => oldCounter + 1);
        setLastPing(Date.now());
      };

      return () => {
        websocket.close();
      };
    } catch (e) {
      console.error(e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [address, port, apiVersion, uuid]);

  return {
    buffer,
    counter,
    latency_s,
  };
};
