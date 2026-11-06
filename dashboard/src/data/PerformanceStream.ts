import { useContext, useEffect, useState } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { PerformanceReport } from 'bindings/PerformanceReport';
import { useInterval } from 'usehooks-ts';
import { coreWebSocketUrl } from 'utils/util';
import { openReconnectingWebSocket } from 'utils/websocket';

// assuming the backend pings every 1 second, we can buffer 60 seconds of events
export const PERFORMANCE_BUFFER_SIZE = 60;

const emptyReport: PerformanceReport = {
  memory_usage: null,
  cpu_usage: null,
  disk_usage: null,
  start_time: null,
};

const emptyBuffer = (): PerformanceReport[] =>
  Array(PERFORMANCE_BUFFER_SIZE).fill(emptyReport);

/**
 * A hook to get the performance monitor information of a specific instance.
 * The backend will replay all events from the last 60 seconds, so no manual fetching is required.
 *
 * @param uuid the uuid of the instance to subscribe to
 * @return buffer: the last PERFORMANCE_BUFFER_SIZE reports, oldest first and
 *   newest last (padded with empty reports at the start);
 *   counter: the number of reports received; latency_s: seconds since the last report
 */
export const usePerformanceStream = (uuid: string) => {
  const [buffer, setBuffer] = useState<PerformanceReport[]>(emptyBuffer);
  const [lastPing, setLastPing] = useState(Date.now());
  const [latency_s, setLatency_s] = useState(0);
  const [counter, setCounter] = useState(-1);
  const { core, token } = useContext(LodestoneContext);

  useInterval(() => {
    setLatency_s((Date.now() - lastPing) / 1000);
  }, 1000);

  useEffect(() => {
    if (!token) return;
    const websocket = openReconnectingWebSocket({
      url: () =>
        coreWebSocketUrl(
          core,
          `/monitor/${uuid}?token=${encodeURIComponent(`Bearer ${token}`)}`
        ),
      onOpen: (_, isReconnect) => {
        // the core replays the last 60 seconds on every connection
        if (isReconnect) setBuffer(emptyBuffer());
      },
      onMessage: (messageEvent) => {
        let event: PerformanceReport;
        try {
          event = JSON.parse(messageEvent.data);
        } catch (e) {
          console.error('Invalid performance report', e);
          return;
        }
        setBuffer((oldBuffer) => [
          ...oldBuffer.slice(-(PERFORMANCE_BUFFER_SIZE - 1)),
          event,
        ]);
        setCounter((oldCounter) => oldCounter + 1);
        setLastPing(Date.now());
      },
    });

    return () => {
      websocket.close();
    };
  }, [core, token, uuid]);

  return {
    buffer,
    counter,
    latency_s,
  };
};
