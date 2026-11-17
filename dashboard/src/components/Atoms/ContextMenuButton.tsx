import { forwardRef } from 'react';
import clsx from 'clsx';
import { myTwMerge } from 'utils/util';

const ContextMenuButton = forwardRef(
  (
    {
      label,
      subLabel,
      caption,
      disabled = false,
      className,
      onClick,
      iconComponent,
    }: {
      label: string;
      subLabel?: string;
      /** what the action goes to (an archive or folder name), in mono under the label */
      caption?: string;
      disabled?: boolean;
      className?: string;
      onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
      iconComponent?: React.ReactNode;
    },
    ref: React.Ref<HTMLButtonElement>
  ) => {
    return (
      <button
        className={myTwMerge(
          'gap-1.5 rounded px-2 py-1 text-justify text-medium',
          'text-white enabled:hover:text-on-accent disabled:text-white/30 enabled:ui-active:text-on-accent',
          'font-medium',
          'bg-blue enabled:hover:bg-blue-400 enabled:active:bg-blue-500 enabled:ui-active:bg-blue-400',
          'group flex',
          'flex-row flex-nowrap items-center select-none',
          'leading-normal tracking-medium',
          'enabled:focus-visible:ring-blue-faded/50',
          className
        )}

        disabled={disabled}
        ref={ref}
        type="button"
        onClick={onClick}
      >
        <div className={`flex grow items-center truncate`}>
          <span className="flex min-w-0 flex-col items-start">
            <span
              className={
                'truncate text-ellipsis' + clsx(!disabled && 'text-gray-300')
              }
            >
              {label}
            </span>
            {caption && (
              <span
                className="max-w-full truncate font-mono text-caption opacity-60"
                title={caption}
              >
                {caption}
              </span>
            )}
          </span>
          {subLabel && (
            <span
              className={
                'mr-0 ml-auto text-small font-medium opacity-50' +
                clsx(!disabled && 'text-gray-300 group-hover:opacity-100')
              }
            >
              {subLabel}
            </span>
          )}
        </div>
        {iconComponent}
      </button>
    );
  }
);

ContextMenuButton.displayName = 'ContextMenuButton';

export default ContextMenuButton;
