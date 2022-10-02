
// Note: children should only be used for absolute positioned elements
export default function AutoGrowInput({
  value,
  onChange,
  onBlur,
  className = '',
  textClassName = '',
  autoFocus = false,
  children,
}: {
  value: string;
  onChange: (arg: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur: (arg: React.FocusEvent<HTMLInputElement>) => void;
  className?: string;
  textClassName?: string;
  autoFocus?: boolean;
  children?: React.ReactNode;
}) {
  return (
    <div className={`inline-grid items-center justify-start ${className}`}>
      <input
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`w-full pl-[0.25ch] border-none ${textClassName}`}
        autoFocus={autoFocus}
        size={1}
      />
      <span
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className={`invisible px-[0.25ch] whitespace-pre ${textClassName}`}
      >
        {value}
      </span>
      {children}
    </div>
  );
}
