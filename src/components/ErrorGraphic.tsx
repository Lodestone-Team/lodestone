import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';

interface ErrorGraphicProps {
  icon: IconDefinition;
  message: string;
  message2?: string;
  className?: string;
  iconClassName: string;
  messageClassName: string;
}

export default function ErrorGraphic({
  className,
  icon,
  iconClassName,
  message,
  message2,
  messageClassName,
}: ErrorGraphicProps) {
  return (
    <div
      className={clsx(
        'flex h-full w-full grow flex-col items-center justify-center gap-4 break-all bg-gray-800',
        className
      )}
    >
      <FontAwesomeIcon
        icon={icon}
        className={clsx('text-title', iconClassName)}
      />
      <p className={clsx('text-center text-h3 font-medium', messageClassName)}>
        {message}
      </p>
      {message2 && (
        <p
          className={clsx('mx-6 text-center text-h3 font-medium', messageClassName)}
        >
          {message2}
        </p>
      )}
    </div>
  );
}
