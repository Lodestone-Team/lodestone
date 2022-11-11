export default function ProgressBar({
  progress,
  className = '',
  colorClass = 'bg-blue',
}: {
  progress: number;
  className?: string;
  colorClass?: string;
}) {
  return (
    <div className={`h-1 bg-transparent ${className}`}>
      <div
        className={`h-full ${colorClass}`}
        style={{ width: `${progress*100}%` }}
      />
    </div>
  );
}
