import { cn } from 'utils/util';
import { middleTruncate } from 'utils/values';

/**
 * A mod, plugin or modpack version as a small mono tag, apart from the name
 * it follows. Never for the game or a loader: those are one plain phrase
 * ("Minecraft 1.21.1", see utils/values). `from` and `to` are the two sides
 * of a change. Noisy versions are kept verbatim, cut in the middle.
 */
export function VersionTag({
  version,
  tone = 'normal',
  max = 24,
  className,
}: {
  version: string;
  tone?: 'normal' | 'from' | 'to';
  max?: number;
  className?: string;
}) {
  const shown = middleTruncate(version, max);
  return (
    <span
      className={cn(
        'inline-flex h-4.5 max-w-40 shrink-0 items-center rounded-sm border border-line bg-gray-900 px-1.5 font-mono text-[11px] leading-none font-medium whitespace-nowrap',
        {
          normal: 'text-gray-400',
          from: 'text-ink-muted',
          to: 'text-gray-300',
        }[tone],
        className
      )}
      title={shown === version ? undefined : version}
    >
      {shown === version ? (
        version
      ) : (
        <>
          <span aria-hidden="true">{shown}</span>
          <span className="sr-only">{version}</span>
        </>
      )}
    </span>
  );
}
