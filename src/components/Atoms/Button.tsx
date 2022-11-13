import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { DOMAttributes } from 'react';

// A styled button component
export default function Button({
  label,
  disabled = false,
  loading = false,
  className,
  onClick,
  type = 'button',
  icon,
  form,
  value,
  ...props
}: {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  className?: string;
  icon?: IconDefinition;
  form?: string;
  value?: string;
  onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
  type?: 'button' | 'submit' | 'reset';
}) {
  disabled = disabled || loading;
  return (
    <button
      className={`${className} group select-none rounded-lg bg-gray-700 py-1 px-2 text-base font-semibold leading-snug tracking-tight outline outline-1 outline-gray-faded/30 enabled:text-gray-300 enabled:hover:bg-gray-600 enabled:hover:outline-white/50 enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue/30 enabled:active:bg-gray-700 disabled:bg-gray-800 disabled:text-white/50`}
      disabled={disabled}
      onClick={onClick}
      type={type}
      form={form}
      value={value}
      {...props}
    >
      {icon && (
        <FontAwesomeIcon
          icon={icon}
          className="mr-2 w-4 text-gray-500 hover:cursor-pointer group-hover:text-gray-400"
        />
      )}
      {loading ? '...' : label}
    </button>
  );
}
