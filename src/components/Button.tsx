// A styled button component
export default function Button({
  label,
  disabled = false,
  onClick,
}: {
  label: string;
  disabled?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      className={`select-none py-1 px-2 rounded-lg bg-gray-600 enabled:border-gray-400 enabled:border enabled:text-gray-300 tracking-tight leading-snug font-bold enabled:focus:ring-2 enabled:focus:ring-gray-500 enabled:hover:bg-gray-700 disabled:text-gray-500 disabled:m-px`}
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </button>
  );
}
