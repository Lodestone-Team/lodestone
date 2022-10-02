export default function SystemStat({
  name,
  value = '...',
}: {
  name: string
  value?: string
}) {
  return (
    <div className="flex flex-row justify-between w-full gap-1 font-bold tracking-tight text-gray-500 text-smaller">
      <p>{name.toUpperCase()}:</p>
      <p className="truncate">{value.toUpperCase()}</p>
    </div>
  )
}
