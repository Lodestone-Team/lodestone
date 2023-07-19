export default function WarningAlert({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-md border border-red-200 bg-red-faded/25 p-2 text-medium font-medium tracking-medium text-white">
      <span className="block sm:inline">{children}</span>
    </div>
  );
}
