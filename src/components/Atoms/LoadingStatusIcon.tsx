import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

import {
  faCircleCheck,
  faCircleXmark,
} from '@fortawesome/free-regular-svg-icons';
import { NotificationStatus } from 'data/NotificationContext';

export default function LoadingStatusIcon({
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
        className={`svg-inline--fa animate-spin text-gray-400 h-5 ${className}`}
      >
        <g clip-path="url(#clip0_1614_2092)">
          <path
            d="M512 256C512 397.4 397.4 512 256 512C114.6 512 0 397.4 0 256C0 114.6 114.6 0 256 0C397.4 0 512 114.6 512 256ZM256 48C141.1 48 48 141.1 48 256C48 370.9 141.1 464 256 464C370.9 464 464 370.9 464 256C464 141.1 370.9 48 256 48Z"
            fill="#44464B"
          />
          <path
            d="M384 34.3C372.5 27.7 357.8 31.6 351.2 43.1C344.6 54.6 348.5 69.3 360 75.9C360.3 76.1 360.7 76.3 361 76.4C422.6 112.5 464 179.4 464 256C464 332.6 422.6 399.5 361 435.6C360.7 435.8 360.3 436 360 436.1C348.5 442.7 344.6 457.4 351.2 468.9C357.8 480.4 372.5 484.3 384 477.7C460.5 433.4 512 350.7 512 256C512 161.3 460.5 78.5 384 34.3Z"
            fill="currentColor"
          />
        </g>
        <defs>
          <clipPath id="clip0_1614_2092">
            <rect width="512" height="512" fill="white" />
          </clipPath>
        </defs>
      </svg>
    );
  return (
    <FontAwesomeIcon
      icon={status === 'success' ? faCircleCheck : faCircleXmark}
      className={`h-5 ${
        status === 'success' ? 'text-green' : 'text-red'
      } ${className}`}
    />
  );
}
