
// Note: children should only be used for absolute positioned elements
export default function AutoGrowInput({
  value,
  onChange,
  onBlur,
  className = '',
  textClassName = '',
  autoFocus = false,
  placeholder = '',
  children,
}: {
  value: string;
  onChange: (arg: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur: (arg: React.FocusEvent<HTMLInputElement>) => void;
  className?: string;
  textClassName?: string;
  autoFocus?: boolean;
  placeholder?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className={`inline-grid items-center justify-start ml-[-1ch] mr-[-0.5ch] ${className}`}>
      <input
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`w-full pl-[1ch] pr-[0.5ch] border-none placeholder:text-gray-500 truncate ${textClassName}`}
        autoFocus={autoFocus}
        size={1}
        placeholder={placeholder}
      />
      <span
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`invisible pl-[1ch] pr-[0.5ch] whitespace-pre truncate ${textClassName}`}
      >
        {value ? value : placeholder}
      </span>
      {children}
    </div>
  );
}
