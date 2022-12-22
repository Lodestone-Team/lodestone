import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { DOMAttributes, forwardRef } from 'react';
import clsx from 'clsx';

// A styled button component
const Button = forwardRef(
  (
    {
      label,
      disabled = false,
      loading = false,
      active,
      variant = 'contained',
      align = 'center',
      color = 'plain',
      size = 'medium',
      className,
      onClick,
      type = 'button',
      iconComponent,
      icon,
      iconRight,
      form,
      value,
      ...props
    }: {
      label: string;
      disabled?: boolean;
      loading?: boolean;
      active?: boolean;
      variant?: 'contained' | 'text';
      align?: 'start' | 'center' | 'end';
      color?: 'plain' | 'danger' | 'primary';
      size?: 'slim' | 'medium' | 'large';
      className?: string;
      iconComponent?: React.ReactNode;
      icon?: IconDefinition;
      iconRight?: IconDefinition;
      form?: string;
      value?: string;
      onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
      type?: 'button' | 'submit' | 'reset';
    },
    ref: React.Ref<HTMLButtonElement>
  ) => {
    return (
      <button
        className={clsx(
          `group flex select-none flex-row flex-nowrap items-center justify-${align}`,
          'leading-normal tracking-medium enabled:focus-visible:ring-4',
          'enabled:focus-visible:ring-blue-faded/50',
          {
            slim: 'gap-1 rounded-sm py-1 px-1.5 text-base',
            medium: 'gap-1.5 rounded py-1 px-2 text-base',
            large: 'gap-1.5 rounded py-1.5 px-3 text-base',
          }[size],
          color==='danger' ? 'font-bold' : {
            slim: 'font-normal',
            medium: 'font-medium',
            large: 'font-medium',
          }[size],
          {
            plain: 'text-gray-300 disabled:text-white/50 child:text-white/50',
            danger:
              variant === 'text'
                ? 'text-red-200 disabled:text-red/50 child:text-red-200/50'
                : 'text-red-200 hover:text-white active:text-white disabled:text-white/50 child:text-red-200/50 hover:child:text-white/50 active:hover:child:text-white/50 disabled:child:text-white/50',
            primary: 'text-gray-300 disabled:text-white/50 child:text-white/50',
          }[color],
          active &&
            {
              plain: 'bg-gray-700',
              danger: variant === 'text' ? 'bg-red-faded/25' : 'bg-red-300',
              primary: 'bg-blue-400',
            }[color],
          !active &&
            {
              plain: `bg-gray-800 enabled:hover:bg-gray-700 enabled:active:bg-gray-800`,
              danger:
                variant === 'text'
                  ? 'bg-gray-800 enabled:hover:bg-red-faded/25 enabled:active:bg-red-faded/10'
                  : 'bg-gray-800 enabled:hover:bg-red-300 enabled:active:bg-red-400',
              primary: `bg-blue enabled:hover:bg-blue-400 enabled:active:bg-blue-500 disabled:bg-blue-faded/50`,
            }[color],
          variant !== 'text' &&
            {
              plain:
                'outline outline-1 outline-gray-faded/30 enabled:hover:outline-white/50',
              danger:
                'outline outline-1 outline-gray-faded/30 enabled:hover:outline-white/50',
              primary:
                'outline outline-1 outline-blue-faded/50 enabled:hover:outline-blue-200/75', //TODO: remove hardcoded colors
            }[color],
          className
        )}
        disabled={disabled || loading}
        onClick={onClick}
        type={type}
        form={form}
        value={value}
        ref={ref}
        {...props}
      >
        {iconComponent}
        {icon && <FontAwesomeIcon icon={icon} className="w-4" />}
        {loading ? '...' : label}
        {iconRight && <FontAwesomeIcon icon={iconRight} className="w-4" />}
      </button>
    );
  }
);

Button.displayName = 'Button';

export default Button;
