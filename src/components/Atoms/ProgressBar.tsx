export default function ProgressBar({
  progress_percent,
  heightClass = 'h-1',
  colorClass = 'bg-blue',
}: {
  progress_percent: number;
  heightClass?: string;
  colorClass?: string;
}) {
  return (
    <div className={`${heightClass} bg-transparent`}>
      <div
        className={`h-full ${colorClass} rounded-full transition-[width] duration-100 ease-in`}
        style={{ width: `${progress_percent * 100}%` }}
      />
    </div>
  );
}
