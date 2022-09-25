export default function AutoGrowInput({
  value,
  onChange,
  onBlur,
  className = '',
  autoFocus = false,
}: {
  value: string;
  onChange: (arg: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur: (arg: React.FocusEvent<HTMLInputElement>) => void;
  className?: string;
  autoFocus?: boolean;
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
        className="w-full p-0 border-none inherit-font-styling pr-[0.5ch]"
        autoFocus={autoFocus}
        size={1}
      />
      <span
        style={{
          gridArea: '1 / 1 / 2 / 2',
        }}
        className="invisible not-italic whitespace-pre pr-[0.5ch]"
      >
        {value}
      </span>
    </div>
  );
}
