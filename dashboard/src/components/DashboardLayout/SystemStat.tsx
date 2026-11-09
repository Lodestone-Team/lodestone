export default function SystemStat({
  name,
  value = '...',
}: {
  name: string;
  value?: string;
}) {
  return (
    <div className="flex w-full flex-row justify-between gap-1 text-caption font-bold tracking-tight text-gray-500">
      <p className="uppercase">{name}:</p>
      <p className="truncate uppercase">{value}</p>
    </div>
  );
}
