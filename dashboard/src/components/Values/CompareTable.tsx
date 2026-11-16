import { faTriangleExclamation } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useT } from 'i18n';
import { ReactNode } from 'react';
import { cn } from 'utils/util';

export type CompareRow = {
  label: string;
  a: ReactNode;
  b: ReactNode;
  mismatch?: boolean;
};

/**
 * Two values side by side that should match (the core's version and the
 * dashboard's). Only the rows that differ are marked, in yellow with an icon.
 */
export function CompareTable({
  columns,
  rows,
  className,
}: {
  columns: [string, string];
  rows: CompareRow[];
  className?: string;
}) {
  const t = useT();
  const cell = (value: ReactNode, mismatch?: boolean) => (
    <td className="py-1 pr-3 align-middle">
      <span
        className={cn(
          'inline-flex items-center gap-1.5',
          mismatch && 'rounded-sm bg-pill-yellow px-1.5 py-0.5 text-yellow'
        )}
      >
        {mismatch && (
          <>
            <FontAwesomeIcon
              icon={faTriangleExclamation}
              className="w-3"
              aria-hidden="true"
            />
            <span className="sr-only">{t('values.differs')}: </span>
          </>
        )}
        {value}
      </span>
    </td>
  );
  return (
    <table className={cn('w-full border-collapse text-medium', className)}>
      <thead>
        <tr className="border-b border-line">
          <td />
          {columns.map((column) => (
            <th
              key={column}
              scope="col"
              className="py-1 pr-3 text-left text-small font-medium text-ink-muted"
            >
              {column}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.label} className="border-b border-line last:border-b-0">
            <th
              scope="row"
              className="py-1 pr-3 text-left text-small font-medium text-ink-muted"
            >
              {row.label}
            </th>
            {cell(row.a, row.mismatch)}
            {cell(row.b, row.mismatch)}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
