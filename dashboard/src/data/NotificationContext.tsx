import { ProgressionStartValue } from './../bindings/ProgressionStartValue';
import { getSnowflakeTimestamp } from './../utils/util';
import { ClientEvent } from 'bindings/ClientEvent';
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useState,
} from 'react';
import { ProgressionEvent } from 'bindings/ProgressionEvent';
import { fields, match, variant, VariantOf } from 'variant';
import { EventLevel } from 'bindings/EventLevel';
import { LodestoneContext } from './LodestoneContext';

/** Only the newest notifications are kept, older ones are dropped. */
export const MAX_NOTIFICATIONS = 200;

export type NotificationItem = {
  title: string;
  message: string;
  description: string | null;
  timestamp: number;
  key: string;
  level: EventLevel;
  fresh: boolean;
};

export type OngoingState = 'ongoing' | 'done' | 'error';

// Invariant progress = parent
export type OngoingNotificationItem = {
  state: OngoingState;
  progress: number;
  total: number | null;
  title: string;
  message: string | null;
  timestamp: number;
  event_id: string;
  key: string;
  level: EventLevel;
  start_value: ProgressionStartValue | null;
  fresh: boolean;
};

export const NotificationAction = variant({
  add: fields<{
    title: string;
    message?: string;
    event: ClientEvent;
    fresh: boolean;
  }>(),
  clear: {},
});

export type NotificationAction = VariantOf<typeof NotificationAction>;

export type OngoingNotificationAction = {
  event: ClientEvent;
  progressionEvent: ProgressionEvent;
  fresh: boolean;
};

interface NotificationContext {
  notifications: NotificationItem[];
  ongoingNotifications: OngoingNotificationItem[];
  dispatch: React.Dispatch<NotificationAction>;
  ongoingDispatch: React.Dispatch<OngoingNotificationAction>;
  /** false until the past events (the core's event buffer) have been fetched */
  historyLoaded: boolean;
  setHistoryLoaded: (loaded: boolean) => void;
}

export const NotificationContext = createContext<NotificationContext>({
  notifications: [],
  ongoingNotifications: [],
  dispatch: () => {
    console.error('dispatch not implemented');
  },
  ongoingDispatch: () => {
    console.error('ongoingDispatch not implemented');
  },
  historyLoaded: false,
  setHistoryLoaded: () => {},
});

type NotificationState = {
  notifications: NotificationItem[];
  ongoingNotifications: OngoingNotificationItem[];
};

type Action =
  | { kind: 'notification'; action: NotificationAction }
  | { kind: 'ongoing'; action: OngoingNotificationAction };

const initialState: NotificationState = {
  notifications: [],
  ongoingNotifications: [],
};

// Appends a notification, skipping duplicates (the same event can come from
// both the buffer and the websocket) and dropping the oldest past the limit.
const addNotification = (
  notifications: NotificationItem[],
  item: NotificationItem
): NotificationItem[] => {
  // the duplicate is almost always one of the last few items
  for (let i = notifications.length - 1; i >= 0; i--) {
    if (notifications[i].key === item.key) return notifications;
  }
  const next = [...notifications, item];
  return next.length > MAX_NOTIFICATIONS
    ? next.slice(next.length - MAX_NOTIFICATIONS)
    : next;
};

const notificationFromEvent = (
  event: ClientEvent,
  title: string,
  message: string,
  fresh: boolean
): NotificationItem => ({
  title,
  message,
  description: null,
  timestamp: getSnowflakeTimestamp(event.snowflake),
  key: event.snowflake,
  level: event.level,
  fresh,
});

const ongoingReducer = (
  state: NotificationState,
  { event, progressionEvent, fresh }: OngoingNotificationAction
): NotificationState => {
  const { event_id } = progressionEvent;
  const level = event.level;
  const { ongoingNotifications } = state;

  return match(progressionEvent.progression_event_inner, {
    ProgressionStart: ({ progression_name, total, inner }) => {
      if (ongoingNotifications.some((item) => item.event_id === event_id))
        return state;
      const item: OngoingNotificationItem = {
        state: 'ongoing',
        progress: 0,
        total,
        title: progression_name,
        message: null,
        timestamp: getSnowflakeTimestamp(event.snowflake),
        event_id,
        key: event_id,
        level,
        start_value: inner,
        fresh,
      };
      return {
        ...state,
        ongoingNotifications: [...ongoingNotifications, item],
      };
    },
    ProgressionUpdate: ({ progress, progress_message }) => {
      if (!ongoingNotifications.some((item) => item.event_id === event_id))
        return state;
      return {
        ...state,
        ongoingNotifications: ongoingNotifications.map((item) => {
          if (item.event_id !== event_id) return item;
          let newProgress = item.progress + progress;
          if (item.total && newProgress > item.total) newProgress = item.total;
          return {
            ...item,
            progress: newProgress,
            message: progress_message || item.message,
            level,
          };
        }),
      };
    },
    ProgressionEnd: ({ message }) => {
      const item = ongoingNotifications.find(
        (item) => item.event_id === event_id
      );
      if (!item) return state;
      // move it from ongoing to the notification list
      return {
        notifications: addNotification(
          state.notifications,
          notificationFromEvent(
            event,
            item.title,
            message || item.message || '',
            fresh
          )
        ),
        ongoingNotifications: ongoingNotifications.filter(
          (item) => item.event_id !== event_id
        ),
      };
    },
  });
};

const reducer = (
  state: NotificationState,
  action: Action
): NotificationState => {
  if (action.kind === 'ongoing') return ongoingReducer(state, action.action);
  return match(action.action, {
    add: ({ title, message, event, fresh }) => {
      const notifications = addNotification(
        state.notifications,
        notificationFromEvent(event, title, message ?? '', fresh)
      );
      if (notifications === state.notifications) return state;
      return { ...state, notifications };
    },
    clear: () => initialState,
  });
};

/**
 * Holds the notification lists, so that a new notification only re-renders
 * the components that read them. Must be inside LodestoneContext; the lists
 * are cleared when the user logs out or switches core.
 */
export const NotificationContextProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const [state, rawDispatch] = useReducer(reducer, initialState);
  const [historyLoaded, setHistoryLoaded] = useState(false);
  const { token } = useContext(LodestoneContext);

  const dispatch = useMemo<React.Dispatch<NotificationAction>>(
    () => (action) => rawDispatch({ kind: 'notification', action }),
    []
  );
  const ongoingDispatch = useMemo<React.Dispatch<OngoingNotificationAction>>(
    () => (action) => rawDispatch({ kind: 'ongoing', action }),
    []
  );

  useEffect(() => {
    if (token) return;
    dispatch({ type: 'clear' });
    // logged out: there are no past events to wait for
    setHistoryLoaded(true);
  }, [token, dispatch]);

  const value = useMemo(
    () => ({
      notifications: state.notifications,
      ongoingNotifications: state.ongoingNotifications,
      dispatch,
      ongoingDispatch,
      historyLoaded,
      setHistoryLoaded,
    }),
    [state, dispatch, ongoingDispatch, historyLoaded]
  );

  return (
    <NotificationContext.Provider value={value}>
      {children}
    </NotificationContext.Provider>
  );
};
