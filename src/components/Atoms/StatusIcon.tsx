import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

import {
  faCircleCheck,
  faCircleXmark,
} from '@fortawesome/free-regular-svg-icons';
import { NotificationStatus } from 'data/NotificationContext';

export default function StatusIcon({
  status,
  className = '',
}: {
  status: NotificationStatus;
  className?: string;
}) {
  if (status === 'info')
    return (
      <svg
        viewBox="0 0 512 512"
        xmlns="http://www.w3.org/2000/svg"
        role="img"
        focusable="false"
        aria-hidden="true"
        className={`svg-inline--fa animate-spin text-gray-400 ${className}`}
      >
        <path
          d="M512 256.3C512 256.2 512 256.1 512 256C511.7 243 501.1 232.5 488 232.5C474.9 232.5 464.3 243 464 256C464 370.9 370.9 464 256 464C242.7 464 232 474.7 232 488C232 501.3 242.7 512 256 512C397.1 512 511.5 397.9 512 257C512 256.8 512 256.7 512 256.5C512 256.4 512 256.4 512 256.3Z"
          fill="currentColor"
        />
      </svg>
    );
  return (
    <FontAwesomeIcon
      icon={status === 'success' ? faCircleCheck : faCircleXmark}
      className={`text-2xl ${
        status === 'success' ? 'text-green' : 'text-red'
      } ${className}`}
    />
  );
}
