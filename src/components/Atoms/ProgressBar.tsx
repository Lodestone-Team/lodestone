export default function ProgressBar({
  progress_percent,
  className = '',
  colorClass = 'bg-blue',
}: {
  progress_percent: number;
  className?: string;
  colorClass?: string;
}) {
  return (
    <div className={`h-1 bg-transparent ${className}`}>
      <div
        className={`h-full ${colorClass} transition-[width] duration-100 ease-in rounded-full`}
        style={{ width: `${progress_percent * 100}%` }}
      />
    </div>
  );
}
