import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { DOMAttributes, forwardRef } from 'react';
import classNames from 'classnames';

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
      color = 'gray',
      className,
      onClick,
      type = 'button',
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
      variant?: 'contained' | 'outlined' | 'text';
      align?: 'start' | 'center' | 'end';
      color?: 'gray' | 'red';
      className?: string;
      icon?: IconDefinition;
      iconRight?: IconDefinition;
      form?: string;
      value?: string;
      onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
      type?: 'button' | 'submit' | 'reset';
    },
    ref: React.Ref<HTMLButtonElement>
  ) => {
    const hover = active ? '' : 'hover:';
    return (
      <button
        className={classNames(
          `button-base group flex select-none flex-row flex-nowrap items-center gap-1 justify-${align}`,
          {
            gray: 'text-gray-300 disabled:text-white/50',
            red: 'text-red disabled:text-red/50',
          }[color],
          {
            gray: 'enabled:focus-visible:ring-blue/30',
            red: 'enabled:focus-visible:ring-red-faded/30',
          }[color],
          variant === 'contained' &&
            {
              gray: `bg-gray-700 ${hover}enabled:bg-gray-600`,
              red: `bg-red-faded/30 ${hover}enabled:bg-red-faded/40 enabled:active:bg-red-faded/30`,
            }[color],
          variant === 'text' &&
            `bg-transparent ${hover}enabled:bg-gray-faded/20 enabled:active:bg-gray-faded/30`,
          variant !== 'text' &&
            `outline outline-1 outline-gray-faded/30 ${hover}enabled:outline-white/50`,
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
        {icon && <FontAwesomeIcon icon={icon} className="w-4" />}
        {loading ? '...' : label}
        {iconRight && <FontAwesomeIcon icon={iconRight} className="w-4" />}
      </button>
    );
  }
);

Button.displayName = 'Button';

export default Button;
