import { DOMAttributes } from "react";

// A styled button component
export default function Button({
  label,
  disabled = false,
  loading = false,
  className,
  onClick,
  type = "button",
  ... props
}: {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  className?: string;
  onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
  type?: "button" | "submit" | "reset";
}) {
  disabled = disabled || loading;
  return (
    <button
      className={`${className} select-none py-1 px-2 rounded-lg bg-gray-600 enabled:outline-gray-400 enabled:outline enabled:outline-0 enabled:text-gray-300 tracking-tight leading-snug font-bold enabled:focus-visible:ring-4 enabled:focus-visible:ring-gray-500 enabled:hover:bg-gray-700 enabled:hover:outline-1 disabled:text-gray-500`}
      disabled={disabled}
      onClick={onClick}
      type={type}
      {...props}
    >
      {loading ? '...' : label}
    </button>
  );
}
