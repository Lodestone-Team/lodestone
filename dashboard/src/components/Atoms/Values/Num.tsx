import { NumberPart, useI18n } from 'i18n';
import { cn } from 'utils/util';
import { DASH } from 'utils/values';

/**
 * A number: tabular figures in the number colour, its unit one step lighter.
 * Give the pieces from formatBytesParts()/formatFractionParts(), or a value
 * and a unit. A `minWidth` keeps a live value from shifting the layout.
 */
export function Num({
  parts,
  value,
  unit,
  minWidth,
  className,
}: {
  parts?: NumberPart[];
  value?: string | number | null;
  unit?: string;
  minWidth?: string;
  className?: string;
}) {
  const { formatNumber } = useI18n();
  const pieces: NumberPart[] = parts ?? [
    {
      text:
        value === null || value === undefined
          ? DASH
          : typeof value === 'number'
            ? formatNumber(value)
            : value,
      unit: false,
    },
    ...(unit && value !== null && value !== undefined
      ? [
          { text: ' ', unit: false },
          { text: unit, unit: true },
        ]
      : []),
  ];
  return (
    <span
      className={cn(
        'whitespace-nowrap text-gray-400 tabular-nums',
        minWidth && 'inline-block text-right',
        className
      )}
      style={minWidth ? { minWidth } : undefined}
    >
      {pieces.map((part, i) =>
        part.unit ? (
          <span key={i} className="text-ink-muted">
            {part.text}
          </span>
        ) : (
          part.text
        )
      )}
    </span>
  );
}

/** A size in bytes, e.g. 1.5 MB, with the unit lighter */
export function Bytes({
  bytes,
  decimals,
  unitOf,
  className,
}: {
  bytes: number;
  decimals?: number;
  unitOf?: number;
  className?: string;
}) {
  const { formatBytesParts } = useI18n();
  return (
    <Num
      parts={formatBytesParts(bytes, decimals, unitOf)}
      className={className}
    />
  );
}

/** "3 / 20" or "1.2 / 3.4 MB" (the unit once), a dash when unknown */
export function Fraction({
  done,
  total,
  bytes,
  className,
}: {
  done: number | null;
  total: number | null;
  bytes?: boolean;
  className?: string;
}) {
  const { formatFractionParts } = useI18n();
  return (
    <Num
      parts={formatFractionParts(done, total, { bytes })}
      className={className}
    />
  );
}
