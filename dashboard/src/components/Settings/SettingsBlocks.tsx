import { ReactNode } from 'react';
import { cn } from 'utils/util';

/*
 * The building blocks of the settings pages: a page with one content column,
 * sections (a heading, an optional lead and cards), cards of rows, and the
 * small pieces used in rows (tags, text links, values).
 */

/** The muted text under a label, or a section lead */
export const settingsHintClass = 'text-small font-medium text-gray-500';

/**
 * A page of the settings modal: the title and a one-line lead at the top,
 * and under them the sections in one column, which scrolls.
 */
export function SettingsPage({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex size-full min-h-0 flex-col">
      {/* room on the right for the modal's close button */}
      <header className="shrink-0 px-8 pt-6 pb-4 max-[760px]:px-4 max-[760px]:pt-4">
        <div className="mx-auto w-full max-w-170 pr-10">
          <h1 className="font-heading text-h2 leading-tight font-bold text-gray-300">
            {title}
          </h1>
          {description && (
            <p className="mt-1 text-medium font-medium text-gray-500">
              {description}
            </p>
          )}
        </div>
      </header>
      <div className="@container min-h-0 grow gutter-stable overflow-y-auto px-8 pb-10 max-[760px]:px-4">
        <div className="mx-auto flex w-full max-w-170 flex-col gap-6.5 pt-2">
          {children}
        </div>
      </div>
    </div>
  );
}

/**
 * A section: a heading, an optional one-line lead, and its cards. A section
 * with an `id` is listed by SettingsWithToc.
 */
export function SettingsSection({
  id,
  title,
  lead,
  action,
  danger = false,
  children,
  className,
}: {
  id?: string;
  title: string;
  lead?: ReactNode;
  /** a button or link on the right of the heading */
  action?: ReactNode;
  /** listed in red in the list of sections */
  danger?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      id={id}
      data-settings-section={id ? title : undefined}
      data-danger={danger || undefined}
      className={cn('flex scroll-mt-5 flex-col', className)}
    >
      <div className="mb-2.5 flex flex-row items-end justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-1">
          <h2 className="font-heading text-h3 leading-tight font-bold text-gray-300">
            {title}
          </h2>
          {lead && (
            <p className="text-[0.8125rem] font-medium text-gray-500">{lead}</p>
          )}
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      <div className="flex flex-col gap-2.5">{children}</div>
    </section>
  );
}

/** A card of rows, with a line between rows */
export function SettingsCard({
  children,
  danger = false,
  className,
}: {
  children: ReactNode;
  /** a red edge, for destructive actions */
  danger?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'w-full min-w-0 divide-y divide-fade-700 rounded-xl border bg-gray-850',
        danger ? 'border-red/40' : 'border-fade-700',
        className
      )}
    >
      {children}
    </div>
  );
}

/**
 * A row of a card: the label and an optional hint on the left, the control on
 * the right. An error or a warning shows as a line under the row.
 */
export function SettingsRow({
  label,
  labelId,
  htmlFor,
  hint,
  error,
  warning,
  danger = false,
  disabled = false,
  children,
  controlClassName,
  below,
  className,
}: {
  label: ReactNode;
  /** the label's id, for controls labelled with aria-labelledby */
  labelId?: string;
  /** the id of the control the label belongs to */
  htmlFor?: string;
  hint?: ReactNode;
  error?: ReactNode;
  warning?: ReactNode;
  /** a destructive action: the label is red */
  danger?: boolean;
  /** the label is muted, e.g. a setting that can't be changed */
  disabled?: boolean;
  /** the control(s), right-aligned */
  children?: ReactNode;
  controlClassName?: string;
  /** content under the label and control, e.g. options of a toggle */
  below?: ReactNode;
  className?: string;
}) {
  const labelClass = cn(
    'text-medium font-mediumbold',
    danger ? 'text-red-200' : disabled ? 'text-gray-500' : 'text-gray-300'
  );
  return (
    <div
      className={cn(
        'flex min-h-14 flex-col justify-center gap-2 px-4 py-3',
        className
      )}
    >
      <div className="flex flex-row items-center gap-4">
        <div className="flex min-w-0 grow flex-col gap-0.5">
          {htmlFor ? (
            <label id={labelId} htmlFor={htmlFor} className={labelClass}>
              {label}
            </label>
          ) : (
            <div id={labelId} className={labelClass}>
              {label}
            </div>
          )}
          {hint && <div className={settingsHintClass}>{hint}</div>}
        </div>
        {children !== undefined && children !== null && children !== false && (
          <div
            className={cn(
              'flex shrink-0 flex-row items-center justify-end gap-2',
              controlClassName
            )}
          >
            {children}
          </div>
        )}
      </div>
      {below}
      {error && (
        <p className="text-small font-medium text-red-200" role="alert">
          {error}
        </p>
      )}
      {warning && (
        <p className="text-small font-medium text-yellow-300">{warning}</p>
      )}
    </div>
  );
}

/** A row with only a note in it, e.g. when a list is empty */
export function SettingsEmptyRow({ children }: { children: ReactNode }) {
  return (
    <div className="px-4 py-3.5 text-[0.8125rem] font-medium text-gray-500">
      {children}
    </div>
  );
}

/** A read-only value on the right of a row */
export function SettingsValue({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'text-right text-medium font-medium text-gray-300',
        className
      )}
    >
      {children}
    </span>
  );
}

/** A small outlined tag, e.g. "You" or "Recommended" */
export function SettingsTag({
  children,
  tone = 'neutral',
}: {
  children: ReactNode;
  tone?: 'neutral' | 'recommended';
}) {
  return (
    <span
      className={cn(
        'inline-flex shrink-0 items-center rounded-[5px] border px-1.5 text-[0.6875rem] leading-4 font-mediumbold',
        tone === 'recommended'
          ? 'border-green/30 text-green'
          : 'border-gray-700 text-gray-500 light:border-gray-550'
      )}
    >
      {children}
    </span>
  );
}

/** A text button for light actions */
export function SettingsLink({
  children,
  onClick,
  muted = false,
  disabled = false,
  className,
}: {
  children: ReactNode;
  onClick: () => void;
  muted?: boolean;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'rounded-sm text-[0.8125rem] font-mediumbold',
        'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
        'disabled:cursor-default disabled:text-gray-550',
        muted
          ? 'text-gray-500 enabled:hover:text-gray-300'
          : 'text-blue-200 enabled:hover:underline',
        className
      )}
    >
      {children}
    </button>
  );
}
