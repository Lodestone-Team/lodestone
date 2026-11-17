import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ReactNode } from 'react';
import { cn } from 'utils/util';

/**
 * One keyed value. With an icon the key is screen-reader text and the icon
 * stands for it (use a title so mouse users get it too); without one the
 * key is shown, lighter than the value. Either way screen readers read
 * "key: value" from real text.
 */
export function Fact({
  label,
  icon,
  children,
  title,
  className,
}: {
  label: string;
  icon?: IconDefinition;
  children: ReactNode;
  title?: string;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'inline-flex min-w-0 items-center gap-1 whitespace-nowrap',
        className
      )}
      title={title}
    >
      {icon ? (
        <>
          <FontAwesomeIcon
            icon={icon}
            className="w-3 shrink-0 text-gray-500"
            aria-hidden="true"
          />
          <span className="sr-only">{label}: </span>
        </>
      ) : (
        <span className="shrink-0 text-ink-muted">
          {label}
          <span className="sr-only">:</span>
        </span>
      )}
      <span className="min-w-0 truncate">{children}</span>
    </span>
  );
}

/** The title of a row whose facts may not all fit: one "key: value" a line */
export const factsTitle = (facts: { label: string; value: string }[]) =>
  facts.map(({ label, value }) => `${label}: ${value}`).join('\n');

/**
 * Line 2 of a list row: facts 12px apart, in priority order, one line high.
 * The facts that don't fit wrap onto a hidden second line, so the lowest
 * ones drop off without measuring; give the row a `title` (factsTitle) that
 * lists them all.
 */
export function FactLine({
  children,
  title,
  className,
}: {
  children: ReactNode;
  title?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        // nowrap: a long value never breaks at its hyphens onto the hidden line
        'flex h-lh min-w-0 flex-wrap items-center gap-x-3 overflow-hidden text-small whitespace-nowrap text-gray-400',
        className
      )}
      title={title}
    >
      {children}
    </div>
  );
}
