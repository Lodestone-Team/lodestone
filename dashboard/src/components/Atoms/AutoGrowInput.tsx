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
    <div
      className={`ml-[-1ch] mr-[-0.5ch] inline-grid items-center justify-start ${className}`}
    >
      <input
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`font-style w-full appearance-none truncate border-none pl-[1ch] pr-[0.5ch] placeholder:text-gray-500 ${textClassName}`}
        autoFocus={autoFocus}
        size={1}
        placeholder={placeholder}
      />
      <span
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`invisible truncate whitespace-pre pl-[1ch] pr-[0.5ch] ${textClassName}`}
      >
        {value ? value : placeholder}
      </span>
      {children}
    </div>
  );
}
