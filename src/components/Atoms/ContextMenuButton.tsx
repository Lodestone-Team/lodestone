import { forwardRef } from 'react';
import clsx from 'clsx';
import { myTwMerge } from 'utils/util';

const ContextMenuButton = forwardRef(
  (
    {
      label,
      subLabel,
      disabled = false,
      className,
      onClick,
      iconComponent,
    }: {
      label: string;
      subLabel?: string,
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
          'gap-1.5 rounded py-1 px-2 text-medium text-justify',
          'text-white disabled:text-white/30',
          'font-medium ',
          'bg-blue enabled:hover:bg-blue-400 enabled:active:bg-blue-500 enabled:ui-active:bg-blue-400',
          'group flex',
          'select-none flex-row flex-nowrap items-center',
          'leading-normal tracking-medium',
          'enabled:focus-visible:ring-blue-faded/50',
          className,
        )}

        disabled={disabled}
        ref={ref}
        type='button'
        onClick={onClick}
      >
        <div className={`flex grow items-center truncate`}>
          <span className={'truncate' + clsx(!disabled && 'text-gray-300')}
          >
            {label}
          </span>
          {subLabel &&
            <span className={"ml-auto mr-0 text-small font-medium opacity-50" + 
              clsx(!disabled && 'text-gray-300 group-hover:opacity-100')
            }>
              {subLabel}
            </span> 
          }
        </div>
        {iconComponent}
      </button>
    );
  }
);

ContextMenuButton.displayName = 'ContextMenuButton';

export default ContextMenuButton;
