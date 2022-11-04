export default function ProgressBar({
  progress,
  className = '',
}: {
  progress: number;
  className?: string;
}) {
  return (
    <div className={`h-1 bg-transparent ${className}`}>
      <div
        className="h-full bg-blue"
        style={{ width: `${progress*100}%` }}
      />
    </div>
  );
}
