import { faCheck, faXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { CountBadge } from 'components/Atoms/Values';
import { useT } from 'i18n';
import { ReactNode, useState } from 'react';
import { cn } from 'utils/util';
import { splitShown } from 'utils/values';

export type NameListItem = {
  id: string;
  icon?: ReactNode;
  name: ReactNode;
  reason?: ReactNode;
};

export type NameListGroup = {
  /** left out when the dialog's title already says what the list is */
  heading?: string;
  tone?: 'neutral' | 'success' | 'danger';
  items: NameListItem[];
};

function Group({
  group,
  collapsedAfter,
}: {
  group: NameListGroup;
  collapsedAfter: number;
}) {
  const t = useT();
  const [expanded, setExpanded] = useState(false);
  const { shown, rest } = expanded
    ? { shown: group.items, rest: 0 }
    : splitShown(group.items, collapsedAfter);
  const toneIcon =
    group.tone === 'success' ? (
      <FontAwesomeIcon
        icon={faCheck}
        className="w-3 text-green"
        aria-hidden="true"
      />
    ) : group.tone === 'danger' ? (
      <FontAwesomeIcon
        icon={faXmark}
        className="w-3 text-red-200"
        aria-hidden="true"
      />
    ) : null;
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      {group.heading && (
        <div className="flex items-center gap-1.5 text-small font-mediumbold text-gray-400">
          {group.heading}
          <CountBadge count={group.items.length} />
        </div>
      )}
      <ul className="flex min-w-0 flex-col">
        {shown.map((item) => (
          <li
            key={item.id}
            className="flex min-h-6 min-w-0 flex-wrap items-center gap-x-2 text-medium text-gray-300 @sm:flex-nowrap"
          >
            {(item.icon ?? toneIcon) && (
              <span className="flex w-3 shrink-0 justify-center">
                {item.icon ?? toneIcon}
              </span>
            )}
            <span className="min-w-0 truncate">{item.name}</span>
            {item.reason && (
              <span className="min-w-0 text-small wrap-break-word text-ink-muted @sm:ml-auto @sm:text-right">
                {item.reason}
              </span>
            )}
          </li>
        ))}
      </ul>
      {rest > 0 && (
        <button
          type="button"
          className="w-fit text-small font-medium text-ink-muted hover:text-gray-300 hover:underline"
          onClick={() => setExpanded(true)}
        >
          {t('values.showMore', { count: rest })}
        </button>
      )}
    </div>
  );
}

/**
 * Names that would otherwise be a comma list in a sentence: one group per
 * outcome with a heading and a count, a row per name with its reason, and
 * the first `collapsedAfter` rows with a counted "Show N more". Callers
 * bound how many items they pass; nothing here is rendered as HTML.
 */
export function NameList({
  groups,
  collapsedAfter = 5,
  className,
}: {
  groups: NameListGroup[];
  collapsedAfter?: number;
  className?: string;
}) {
  const shown = groups.filter((group) => group.items.length > 0);
  if (shown.length === 0) return null;
  return (
    <div className={cn('@container flex min-w-0 flex-col gap-3', className)}>
      {shown.map((group, i) => (
        <Group
          key={group.heading ?? i}
          group={group}
          collapsedAfter={collapsedAfter}
        />
      ))}
    </div>
  );
}
