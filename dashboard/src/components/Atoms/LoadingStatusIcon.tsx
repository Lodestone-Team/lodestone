import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

import {
  faCircleCheck,
  faCircleXmark,
} from '@fortawesome/free-regular-svg-icons';
import { EventLevel } from '@bindings/EventLevel';
import { OngoingState } from 'data/NotificationContext';

export default function LoadingStatusIcon({
  level,
  state,
  className = 'h-5',
  bright = false,
}: {
  level: EventLevel;
  state?: OngoingState;
  className?: string;
  bright?: boolean;
}) {
  const NotificationLevelToFgColorClass = (
    level: EventLevel,
    state?: OngoingState
  ) => {
    switch (level) {
      case 'Warning':
        return 'text-yellow';
      case 'Error':
        return 'text-red';
      default:
        switch (state) {
          case 'done':
            return 'text-green';
          case 'error':
            return 'text-red';
          case 'ongoing':
          default:
            return bright ? 'text-gray-300' : 'text-gray-500';
        }
    }
  };

  const fgColorClass = NotificationLevelToFgColorClass(level, state);
  if (state) {
    if (state === 'ongoing')
      return (
        <svg
          viewBox="0 0 512 512"
          xmlns="http://www.w3.org/2000/svg"
          role="img"
          focusable="false"
          aria-hidden="true"
          className={`svg-inline--fa animate-spin ${fgColorClass} ${className}`}
        >
          <g clipPath="url(#clip0_1614_2092)">
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
        icon={state === 'done' ? faCircleCheck : faCircleXmark}
        className={`${fgColorClass} ${className}`}
      />
    );
  } else {
    switch (level) {
      case 'Warning':
        return (
          <svg
            viewBox="0 0 512 512"
            xmlns="http://www.w3.org/2000/svg"
            role="img"
            focusable="false"
            aria-hidden="true"
            className={`svg-inline--fa ${fgColorClass} ${className}`}
          >
            <g clipPath="url(#clip0_1499_2030)">
              <path
                d="M232.5 152.5C232.5 139.2 243.2 128.5 256.5 128.5C269.8 128.5 280.5 139.2 280.5 152.5V264.5C280.5 277.8 269.8 288.5 256.5 288.5C243.2 288.5 232.5 277.8 232.5 264.5V152.5ZM256.5 384.5C238.8 384.5 224.5 370.2 224.5 352.5C224.5 334.8 238.8 320.5 256.5 320.5C274.2 320.5 288.5 334.8 288.5 352.5C288.5 370.2 274.2 384.5 256.5 384.5Z"
                fill="currentColor"
              />
              <path
                d="M256.5 384.5C274.173 384.5 288.5 370.173 288.5 352.5C288.5 334.827 274.173 320.5 256.5 320.5C238.827 320.5 224.5 334.827 224.5 352.5C224.5 370.173 238.827 384.5 256.5 384.5Z"
                fill="currentColor"
              />
              <path
                d="M256.5 288.5C269.8 288.5 280.5 277.8 280.5 264.5V152.5C280.5 139.2 269.8 128.5 256.5 128.5C243.2 128.5 232.5 139.2 232.5 152.5V264.5C232.5 277.8 243.2 288.5 256.5 288.5Z"
                fill="currentColor"
              />
              <path
                d="M256.5 0.5C115.1 0.5 0.5 115.1 0.5 256.5C0.5 397.9 115.1 512.5 256.5 512.5C397.9 512.5 512.5 397.9 512.5 256.5C512.5 115.1 397.9 0.5 256.5 0.5ZM256.5 464.5C141.6 464.5 48.5 371.4 48.5 256.5C48.5 141.6 141.6 48.5 256.5 48.5C371.4 48.5 464.5 141.6 464.5 256.5C464.5 371.4 371.4 464.5 256.5 464.5Z"
                fill="currentColor"
              />
            </g>
            <defs>
              <clipPath id="clip0_1499_2030">
                <rect width="512" height="512" fill="white" />
              </clipPath>
            </defs>
          </svg>
        );
      case 'Error':
        return (
          <FontAwesomeIcon
            icon={faCircleXmark}
            className={`${fgColorClass} ${className}`}
          />
        );
      default:
        return (
          <svg
            viewBox="0 0 512 512"
            xmlns="http://www.w3.org/2000/svg"
            role="img"
            focusable="false"
            aria-hidden="true"
            className={`svg-inline--fa ${fgColorClass} ${className}`}
          >
            <g clipPath="url(#clip0_1499_2024)">
              <path
                d="M256 192C273.673 192 288 177.673 288 160C288 142.327 273.673 128 256 128C238.327 128 224 142.327 224 160C224 177.673 238.327 192 256 192Z"
                fill="currentColor"
              />
              <path
                d="M296 336H288V248C288 234.7 277.3 224 264 224H216C202.7 224 192 234.7 192 248C192 261.3 202.7 272 216 272H240V336H216C202.7 336 192 346.7 192 360C192 373.3 202.7 384 216 384H296C309.3 384 320 373.3 320 360C320 346.7 309.3 336 296 336Z"
                fill="currentColor"
              />
              <path
                d="M256 128C273.7 128 288 142.3 288 160C288 177.7 273.7 192 256 192C238.3 192 224 177.7 224 160C224 142.3 238.3 128 256 128ZM296 384H216C202.7 384 192 373.3 192 360C192 346.7 202.7 336 216 336H240V272H216C202.7 272 192 261.3 192 248C192 234.7 202.7 224 216 224H264C277.3 224 288 234.7 288 248V336H296C309.3 336 320 346.7 320 360C320 373.3 309.3 384 296 384Z"
                fill="currentColor"
              />
              <path
                d="M256 0C114.6 0 0 114.6 0 256C0 397.4 114.6 512 256 512C397.4 512 512 397.4 512 256C512 114.6 397.4 0 256 0ZM256 464C141.1 464 48 370.9 48 256C48 141.1 141.1 48 256 48C370.9 48 464 141.1 464 256C464 370.9 370.9 464 256 464Z"
                fill="currentColor"
              />
            </g>
            <defs>
              <clipPath id="clip0_1499_2024">
                <rect width="512" height="512" fill="white" />
              </clipPath>
            </defs>
          </svg>
        );
    }
  }
}
