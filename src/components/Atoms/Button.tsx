import { IconDefinition } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { DOMAttributes } from "react";

// A styled button component
export default function Button({
  label,
  disabled = false,
  loading = false,
  className,
  onClick,
  type = "button",
  icon,
  form,
  value,
  ... props
}: {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  className?: string;
  icon?: IconDefinition;
  form?: string;
  value?: string;
  onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
  type?: "button" | "submit" | "reset";
}) {
  disabled = disabled || loading;
  return (
    <button
      className={`${className} group select-none py-1 px-2 rounded-lg bg-gray-600 enabled:outline-gray-400 enabled:outline enabled:outline-0 enabled:text-gray-300 tracking-tight leading-snug font-semibold enabled:focus-visible:ring-4 enabled:focus-visible:ring-gray-500 enabled:hover:bg-gray-700 enabled:hover:outline-2 disabled:text-gray-500`}
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
          className="w-4 mr-2 text-gray-500 hover:cursor-pointer group-hover:text-gray-400"
        />
      )}
      {loading ? '...' : label}
    </button>
  );
}
