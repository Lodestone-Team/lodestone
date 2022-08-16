export default function InstanceCard({ name }: { name: string }) {
  return (
    <div className="w-full h-32 bg-[#292b30] mt-4 rounded-xl p-4">
      <h1 className="text-base font-bold tracking-tight">{name}</h1>
    </div>
  )
}
