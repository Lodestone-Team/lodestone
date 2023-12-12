import clsx from 'clsx'

export default function WarningAlert({
  children,
  className,
}: {
  children: React.ReactNode,
  className?: string,
}) {
  return (
    <div className={clsx("rounded-md border border-red-200 bg-red-faded/25 p-2 text-medium font-medium tracking-medium text-white", className)}>
      <span className="block sm:inline">{children}</span>
    </div>
  );
}
