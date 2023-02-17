import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { forwardRef } from 'react';
import { cva, VariantProps } from 'class-variance-authority';
import { myTwMerge } from 'utils/util';

const buttonClasses = cva(
  [
    'group flex',
    'select-none flex-row flex-nowrap items-center',
    'leading-normal tracking-medium aspect-square',
  ],
  {
    variants: {
      align: {
        start: 'justify-start',
        center: 'justify-center',
        end: 'justify-end',
      },
      size: {
        slim: [''],
        medium: ['gap-2.5 rounded p-1.5 text-medium'],
        large: [''],
      },
      intention: {
        default: [
          'text-gray-faded/30 enabled:hover:text-white/50 disabled:text-gray-900',
          'outline outline-1 rounded',
          'bg-gray-800 border-gray-faded/30 enabled:hover:bg-gray-700 enabled:hover:border-gray-faded/50 enabled:active:bg-gray-800 enabled:active:border-gray-faded/50 disabled:border-fade-700/10',
          'outline-gray-faded/30 enabled:hover:outline-white/50',
          'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        ],
      },
    },
    defaultVariants: {
      align: 'center',
      size: 'medium',
      intention: 'default',
    },
  }
);

export interface IconButtonProps extends VariantProps<typeof buttonClasses> {
  disabled?: boolean;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
  type?: 'button' | 'submit' | 'reset';
  icon?: IconDefinition;
}

// A styled button component
const IconButton = forwardRef(
  (
    {
      disabled = false,
      align,
      intention,
      size,
      className,
      icon,
      type = 'button',
      ...props
    }: IconButtonProps,
    ref: React.Ref<HTMLButtonElement>
  ) => {
    return (
      <button
        className={myTwMerge(
          buttonClasses({ align, intention, size }),
          className
        )}
        disabled={disabled}
        ref={ref}
        type={type}
        {...props}
      >
        {icon && <FontAwesomeIcon icon={icon} className="w-4" />}
      </button>
    );
  }
);

IconButton.displayName = 'IconButton';

export default IconButton;
