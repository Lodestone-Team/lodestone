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
      color = 'gray',
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
      variant?: 'contained' | 'outlined' | 'text';
      align?: 'start' | 'center' | 'end';
      color?: 'gray' | 'red';
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
          {
            slim: 'gap-1 rounded-sm py-1 px-1.5 text-base font-normal',
            medium: 'gap-1.5 rounded py-1 px-2 text-base font-medium',
            large: 'gap-1.5 rounded py-1.5 px-3 text-base font-medium',
          }[size],
          {
            gray: 'text-gray-300 disabled:text-white/50',
            red: 'text-red-accent disabled:text-red/50',
          }[color],
          {
            gray: 'enabled:focus-visible:ring-blue/30',
            red: 'enabled:focus-visible:ring-red-faded/30',
          }[color],
          {
            gray: active
              ? 'bg-gray-700'
              : `bg-gray-800 enabled:hover:bg-gray-700 enabled:active:bg-gray-800`,
            red: active
              ? 'bg-red-faded/25'
              : `bg-gray-800 enabled:hover:bg-red-faded/25 enabled:active:bg-red-faded/10`,
          }[color],
          variant !== 'text' &&
            `outline outline-1 outline-gray-faded/30 enabled:hover:outline-white/50`,
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
