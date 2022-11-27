import { ProgressionStartValue } from './../bindings/ProgressionStartValue';
import { getSnowflakeTimestamp } from './../utils/util';
import { ClientEvent } from 'bindings/ClientEvent';
import { createContext, useReducer } from 'react';
import { ProgressionEvent } from 'bindings/ProgressionEvent';
import { fields, match, variant, VariantOf } from 'variant';
import { EventLevel } from 'bindings/EventLevel';

export type NotificationItem = {
  title: string;
  message: string;
  description: string | null;
  timestamp: number;
  key: string;
  level: EventLevel;
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
};

export const NotificationAction = variant({
  add: fields<{
    title: string;
    message?: string;
    event: ClientEvent;
  }>(),
  clear: {},
});

export type NotificationAction = VariantOf<typeof NotificationAction>;

export type OngoingNotificationAction = {
  event: ClientEvent;
  progressionEvent: ProgressionEvent;
  dispatch: React.Dispatch<NotificationAction>;
};

interface NotificationContext {
  notifications: NotificationItem[];
  ongoingNotifications: OngoingNotificationItem[];
  dispatch: React.Dispatch<NotificationAction>;
  ongoingDispatch: React.Dispatch<OngoingNotificationAction>;
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
});

export const useNotificationReducer = () => {
  const [notifications, dispatch] = useReducer(
    (state: NotificationItem[], action: NotificationAction) =>
      match(action, {
        add: ({ title, message, event }) => {
          const { snowflake_str: key, level } = event;
          const timestamp = getSnowflakeTimestamp(event.snowflake_str);
          if (state.some((item) => item.key === key)) {
            console.warn('Notification with duplicate key received');
            return state;
          }
          return [
            ...state,
            { title, timestamp, message, key, level } as NotificationItem,
          ];
        },
        clear: () => [],
      }),
    []
  );

  return { notifications, dispatch };
};

export const useOngoingNotificationReducer = () => {
  const [ongoingNotifications, ongoingDispatch] = useReducer(
    (state: OngoingNotificationItem[], action: OngoingNotificationAction) => {
      const { snowflake_str: snowflake } = action.event;
      const timestamp = getSnowflakeTimestamp(snowflake);
      const event_inner = action.progressionEvent.progression_event_inner;
      const event_id = action.progressionEvent.event_id;
      const level = action.event.level;
      const dispatch = action.dispatch;

      return match(event_inner, {
        ProgressionStart: ({ progression_name, total, inner }) => {
          return [
            ...state,
            {
              state: 'ongoing',
              progress: 0,
              total,
              title: progression_name,
              message: null,
              timestamp,
              event_id,
              key: event_id,
              level,
              start_value: inner,
            } as OngoingNotificationItem,
          ]
        },
        ProgressionUpdate: ({ progress, progress_message }) => {
          const newState = [...state];
          newState.map((item) => {
            if (item.event_id === event_id) {
              item.progress += progress;
              if (progress_message) item.message = progress_message;
              item.level = level;
            }
          });
          return newState;
        },
        ProgressionEnd: ({ success, message }) => {
          const item = state.find((item) => item.event_id === event_id);
          if (!item) return state;
          dispatch({
            type: 'add',
            title: item.title,
            message: message || item.message || '',
            event: action.event,
          });
          // remove from ongoing
          return [...state.filter((item) => item.event_id !== event_id)];
          // state.map((item) => {
          //   if (item.event_id === event_id) {
          //     item.state = success ? 'done' : 'error';
          //     item.progress = item.total ?? 0;
          //     if (message) item.message = message;
          //     item.level = level;
          //   }
          // });
        },
      });
    },
    []
  );
  return { ongoingNotifications, ongoingDispatch };
};
