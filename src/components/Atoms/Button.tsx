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
      className={`${className} button-base group flex select-none flex-row flex-nowrap items-center justify-center`}
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
          className="mr-2 w-4 text-gray-500 enabled:hover:cursor-pointer enabled:group-hover:text-gray-400"
        />
      )}
      {loading ? '...' : label}
    </button>
  );
}
