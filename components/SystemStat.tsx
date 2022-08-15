export default function SystemStat({
  name,
  value = '...',
}: {
  name: string
  value?: string
}) {
  return (
    <div className="flex flex-row justify-between w-full font-bold tracking-tight text-small text-fade">
      <p>{name.toUpperCase()}:</p>
      <p className="truncate">{value.toUpperCase()}</p>
    </div>
  )
}
