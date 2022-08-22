import { DOMAttributes } from "react";

// A styled button component
export default function Button({
  label,
  disabled = false,
  className,
  onClick,
}: {
  label: string;
  disabled?: boolean;
  className?: string;
  onClick?: DOMAttributes<HTMLButtonElement>['onClick'];
}) {
  return (
    <button
      className={`select-none py-1 px-2 rounded-lg bg-gray-600 enabled:outline-gray-400 enabled:outline enabled:outline-1 enabled:text-gray-300 tracking-tight leading-snug font-bold enabled:focus-visible:ring-4 enabled:focus-visible:ring-gray-500 enabled:hover:bg-gray-700 enabled:hover:outline-0 disabled:text-gray-500 ${className}`}
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </button>
  );
}
