import {
  faArrowUpRightFromSquare,
  faClone,
  faEye,
  faEyeSlash,
} from '@fortawesome/free-solid-svg-icons';
import {
  FontAwesomeIcon,
  FontAwesomeIconProps,
} from '@fortawesome/react-fontawesome';
import { useT } from 'i18n';
import { useState } from 'react';
import { safeLinkUrl } from 'utils/markdown';
import { cn } from 'utils/util';
import { middleTruncate, shortenPath } from 'utils/values';
import { copyWithToast } from './copy';

const ACTION =
  'flex h-5 w-5 shrink-0 items-center justify-center rounded-sm text-ink-muted hover:bg-gray-800 hover:text-gray-300 focus-visible:ring-2 focus-visible:ring-focus focus-visible:outline-hidden';

function Action({
  icon,
  label,
  onClick,
}: {
  icon: FontAwesomeIconProps['icon'];
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={ACTION}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      title={label}
      aria-label={label}
    >
      <FontAwesomeIcon icon={icon} className="w-2.5" aria-hidden="true" />
    </button>
  );
}

/** A small Copy button for a value shown elsewhere (an address in a strip) */
export function CopyButton({ value }: { value: string }) {
  const t = useT();
  return (
    <Action
      icon={faClone}
      label={t('values.copy')}
      onClick={() => copyWithToast(value)}
    />
  );
}

/**
 * A long exact value on its own line, out of the sentence: a path (home as
 * "~", the last two parts kept), a URL, a code or a secret (masked until
 * shown). It can be copied; a URL can be opened. There is no "Show in
 * Finder": the folder is on the core's machine, not the browser's.
 */
export function ValueField({
  label,
  value,
  kind,
  max = 48,
  className,
}: {
  label?: string;
  value: string;
  kind: 'path' | 'url' | 'code' | 'secret';
  max?: number;
  className?: string;
}) {
  const t = useT();
  const [revealed, setRevealed] = useState(false);
  const masked = kind === 'secret' && !revealed;
  const shown = masked
    ? '•'.repeat(Math.min(24, Math.max(8, Array.from(value).length)))
    : kind === 'path'
      ? shortenPath(value, { max })
      : middleTruncate(value, max);
  const url = kind === 'url' ? safeLinkUrl(value) : null;
  return (
    <div className={cn('flex min-w-0 flex-col gap-1', className)}>
      {label && (
        <span className="text-small font-medium text-ink-muted">{label}</span>
      )}
      <div className="flex h-7 min-w-0 items-center gap-1 rounded-sm border border-line bg-gray-900 pr-1 pl-2">
        <span
          className="min-w-0 grow truncate font-mono text-small text-gray-300"
          title={masked ? undefined : value}
        >
          {masked || shown === value ? (
            <span aria-hidden={masked ? 'true' : undefined}>{shown}</span>
          ) : (
            <>
              <span aria-hidden="true">{shown}</span>
              <span className="sr-only">{value}</span>
            </>
          )}
        </span>
        {kind === 'secret' && (
          <Action
            icon={revealed ? faEyeSlash : faEye}
            label={revealed ? t('values.hide') : t('values.show')}
            onClick={() => setRevealed((r) => !r)}
          />
        )}
        {url && (
          <Action
            icon={faArrowUpRightFromSquare}
            label={t('values.open')}
            onClick={() => window.open(url, '_blank', 'noopener,noreferrer')}
          />
        )}
        <Action
          icon={faClone}
          label={t('values.copy')}
          onClick={() => copyWithToast(value)}
        />
      </div>
    </div>
  );
}
