export default function SystemStat({
  name,
  value = '...',
}: {
  name: string;
  value?: string;
}) {
  return (
    <div className="flex w-full flex-row justify-between gap-1 text-smaller font-bold tracking-tight text-gray-500">
      <p>{name.toUpperCase()}:</p>
      <p className="truncate">{value.toUpperCase()}</p>
    </div>
  );
}
