export type ReconnectingWebSocketOptions = {
  /** Builds the URL. Called again before every reconnect attempt. */
  url: () => string;
  onOpen?: (websocket: WebSocket, isReconnect: boolean) => void;
  onMessage: (event: MessageEvent) => void;
  /**
   * Called every time the socket closes, unless close() was called.
   * opened is false when the connection was never established (the handshake
   * failed or the server is unreachable).
   * Return (or resolve to) false to stop reconnecting.
   */
  onClose?: (
    event: CloseEvent,
    opened: boolean
  ) => boolean | void | Promise<boolean | void>;
  /** Delay before the first reconnect, doubled after each failure. */
  initialDelayMs?: number;
  maxDelayMs?: number;
};

export type ReconnectingWebSocket = {
  /** Closes the socket and cancels any pending reconnect. */
  close: () => void;
};

/**
 * Opens a WebSocket and reopens it with exponential backoff when it closes.
 * The backoff resets once a connection opens. After close() no callback runs
 * and no new socket is created, so it is safe to call from an effect cleanup.
 */
export function openReconnectingWebSocket({
  url,
  onOpen,
  onMessage,
  onClose,
  initialDelayMs = 1000,
  maxDelayMs = 30000,
}: ReconnectingWebSocketOptions): ReconnectingWebSocket {
  let cancelled = false;
  let current: WebSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let failures = 0;
  let hasOpened = false;

  const scheduleReconnect = () => {
    if (cancelled) return;
    const delay = Math.min(maxDelayMs, initialDelayMs * 2 ** failures);
    failures += 1;
    timer = setTimeout(connect, delay);
  };

  const connect = () => {
    timer = undefined;
    if (cancelled) return;
    let websocket: WebSocket;
    try {
      websocket = new WebSocket(url());
    } catch (e) {
      // invalid URL, or blocked as mixed content
      console.error(e);
      scheduleReconnect();
      return;
    }
    current = websocket;
    let opened = false;

    websocket.onopen = () => {
      if (cancelled || current !== websocket) return;
      opened = true;
      failures = 0;
      onOpen?.(websocket, hasOpened);
      hasOpened = true;
    };

    websocket.onmessage = (event) => {
      if (cancelled || current !== websocket) return;
      onMessage(event);
    };

    websocket.onclose = async (event) => {
      if (current !== websocket) return;
      current = null;
      if (cancelled) return;
      let reconnect: boolean | void = true;
      try {
        reconnect = await onClose?.(event, opened);
      } catch (e) {
        console.error(e);
      }
      if (cancelled || reconnect === false) return;
      scheduleReconnect();
    };
  };

  connect();

  return {
    close: () => {
      cancelled = true;
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      const websocket = current;
      current = null;
      websocket?.close(1000);
    },
  };
}
