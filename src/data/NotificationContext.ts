import { getSnowflakeTimestamp } from './../utils/util';
import { ClientEvent } from 'bindings/ClientEvent';
import { createContext, useReducer } from 'react';

export type NotificationStatus = 'error' | 'info' | 'success';

export type NotificationItem = {
  status: NotificationStatus;
  message: string;
  timestamp: number;
  key: string;
};

export type OngoingState = 'ongoing' | 'done' | 'error';

export type OngoingNotificationItem = {
  state: OngoingState;
  progress?: number;
  title: string;
  message?: string;
  timestamp: number;
  key: string;
};

// used for dispatching to the notification reducer
export type NotificationAction = {
  message: string;
  status: NotificationStatus;
  event: ClientEvent;
};

export type OngoingNotificationAction = {
  type: 'add' | 'update' | 'done' | 'error';
  title?: string;
  message?: string;
  progress?: number;
  event: ClientEvent;
  ongoing_key: string;
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
    (state: NotificationItem[], action: NotificationAction) => {
      const { message, status, event } = action;
      const key = event.idempotency;
      const timestamp = getSnowflakeTimestamp(event.snowflake_str);
      if (state.some((item) => item.key === key)) {
        console.warn('Notification with duplicate key received');
        return state;
      }
      return [
        ...state,
        { message, status, timestamp, key } as NotificationItem,
      ];
    },
    []
  );

  return { notifications, dispatch };
};

export const useOngoingNotificationReducer = () => {
  const [ongoingNotifications, ongoingDispatch] = useReducer(
    (state: OngoingNotificationItem[], action: OngoingNotificationAction) => {
      const { message, title, progress, event, ongoing_key } = action;
      const key = ongoing_key;
      const timestamp = getSnowflakeTimestamp(event.snowflake_str);
      switch (action.type) {
        case 'add':
          if (state.some((item) => item.key === key)) {
            console.warn('Notification with duplicate key received');
            return state;
          }
          return [
            ...state,
            {
              state: 'ongoing' as OngoingState,
              progress,
              message: message,
              title: title ?? 'Working on it...',
              timestamp,
              key,
            },
          ];
        case 'update':
          return state.map((item) => {
            if (item.key === key) {
              return {
                ...item,
                progress: progress ?? item.progress,
                message: message ?? item.message,
                title: title ?? item.title,
                timestamp,
              };
            }
            return item;
          });
        case 'done':
          return state.map((item) => {
            if (item.key === key) {
              return {
                ...item,
                state: 'done' as OngoingState,
                progress: 1,
                message: message ?? item.message,
                title: title ?? item.title,
                timestamp,
              };
            }
            return item;
          });
        case 'error':
          return state.map((item) => {
            if (item.key === key) {
              return {
                ...item,
                state: 'error' as OngoingState,
                progress: 1,
                message: message ?? item.message,
                title: title ?? item.title,
                timestamp,
              };
            }
            return item;
          });
        default:
          throw new Error('Invalid action type');
          return state;
      }
    },
    []
  );
  return { ongoingNotifications, ongoingDispatch };
};
