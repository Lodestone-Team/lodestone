import { toast } from 'react-toastify';

const SAVED_TOAST_ID = 'settings-saved';

/**
 * A small "Saved" toast for settings that save as they change. One at a time:
 * saving again while it shows only changes its text.
 */
export function notifySaved(message = 'Saved') {
  if (toast.isActive(SAVED_TOAST_ID)) {
    toast.update(SAVED_TOAST_ID, { render: message });
    return;
  }
  toast(message, {
    toastId: SAVED_TOAST_ID,
    autoClose: 1500,
    hideProgressBar: true,
    icon: false,
    style: {
      minHeight: 0,
      width: 'fit-content',
      marginLeft: 'auto',
      padding: '8px 14px',
    },
  });
}
