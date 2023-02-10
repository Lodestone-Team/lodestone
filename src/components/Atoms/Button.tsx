import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { DOMAttributes, forwardRef } from 'react';
import clsx from 'clsx';
import { BeatLoader } from 'react-spinners';
import { cva, VariantProps } from 'class-variance-authority';
import { myTwMerge } from 'utils/util';

const buttonClasses = cva(
  [
    'group flex',
    'select-none flex-row flex-nowrap items-center',
    'leading-normal tracking-medium',
    'enabled:focus-visible:ring-4',
    'enabled:focus-visible:ring-blue-faded/50',
  ],
  {
    variants: {
      align: {
        start: 'justify-start',
        center: 'justify-center',
        end: 'justify-end',
        between: 'justify-between',
      },
      size: {
        slim: ['gap-1 rounded-sm p-1 text-small'],
        medium: ['gap-1.5 rounded py-1 px-2 text-medium'],
        large: ['gap-1.5 rounded py-1.5 px-3 text-h3'],
      },
      intention: {
        info: [
          'text-gray-300 disabled:text-white/50',
          'font-medium',
          'bg-gray-800 enabled:hover:bg-gray-700 enabled:active:bg-gray-800 enabled:ui-active:bg-gray-700',
          'outline-gray-faded/30 enabled:hover:outline-white/50',
        ],
        danger: [
          'text-red-200 active:text-white enabled:hover:text-white disabled:text-white/50 enabled:ui-active:text-white',
          'font-bold',
          'bg-gray-800 enabled:hover:bg-red-300 enabled:active:bg-red-400 enabled:ui-active:bg-red-300',
          'outline-gray-faded/30 enabled:hover:outline-white/50',
        ],
        primary: [
          'text-white disabled:text-white/50',
          'font-medium',
          'bg-blue enabled:hover:bg-blue-400 enabled:active:bg-blue-500 disabled:bg-blue-faded/50 enabled:ui-active:bg-blue-400',
          'outline-blue-faded/50 enabled:hover:outline-blue-200/75',
        ],
      },
      variant: {
        contained: 'outline outline-1',
        text: '',
      },
    },
    defaultVariants: {
      align: 'center',
      size: 'medium',
      intention: 'info',
      variant: 'contained',
    },
  }
);

export interface ButtonProps extends VariantProps<typeof buttonClasses> {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
  type?: 'button' | 'submit' | 'reset';
  iconComponent?: React.ReactNode;
  icon?: IconDefinition;
  iconRight?: IconDefinition;
  form?: string;
  value?: string;
}

// A styled button component
const Button = forwardRef(
  (
    {
      label,
      disabled = false,
      loading = false,
      align,
      intention,
      size,
      variant,
      className,
      iconComponent,
      icon,
      iconRight,
      type = 'button',
      ...props
    }: ButtonProps,
    ref: React.Ref<HTMLButtonElement>
  ) => {
    return (
      <button
        className={myTwMerge(
          buttonClasses({ align, intention, size, variant }),
          className
        )}
        disabled={disabled || loading}
        ref={ref}
        type={type}
        {...props}
      >
        {iconComponent}
        {icon && <FontAwesomeIcon icon={icon} className="w-4 opacity-50" />}
        <div className={`flex items-center truncate`}>
          {loading && (
            <BeatLoader size={5} color="#6b7280" className="absolute" />
          )}
          <span className={clsx(loading && 'opacity-0')}>{label}</span>
        </div>

        {iconRight && (
          <FontAwesomeIcon icon={iconRight} className="w-4 opacity-50" />
        )}
      </button>
    );
  }
);

Button.displayName = 'Button';

export default Button;
