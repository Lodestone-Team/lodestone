import {
  faChevronLeft,
  faChevronRight,
  faMagnifyingGlass,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ModrinthProjectType } from 'bindings/ModrinthProjectType';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { ModrinthUrlTarget } from 'bindings/ModrinthUrlTarget';
import IconButton from 'components/Atoms/IconButton';
import {
  ModrinthSearchParams,
  useModrinthResolve,
  useModrinthSearch,
} from 'data/Mods';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { useDebounceValue } from 'usehooks-ts';
import { looksLikeModrinthLink } from 'utils/mods';
import { cn, DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { ModrinthHitCard } from './common';

const PAGE_SIZE = 20;
const DEBOUNCE_MS = 300;

/** A project picked from the results, or from a pasted link. */
export type ModrinthPick = {
  projectId: string;
  versionId: string | null;
  projectType: string;
  /** known when picked from a search result */
  author?: string;
};

const SORT_OPTIONS: Array<[ModrinthSearchIndex, string]> = [
  ['relevance', 'Relevance'],
  ['downloads', 'Downloads'],
  ['follows', 'Follows'],
  ['newest', 'Newest'],
  ['updated', 'Recently updated'],
];

// the dashboard's input style, from globals.css
const INPUT_CLASSES =
  'input-shape input-background input-outlines input-text-style input-border-normal';

/** The sort order of a Modrinth search. */
export const SortSelect = ({
  value,
  onChange,
}: {
  value: ModrinthSearchIndex;
  onChange: (index: ModrinthSearchIndex) => void;
}) => (
  <label className="flex flex-row items-center gap-2 text-medium text-white/50">
    Sort by
    <select
      className={INPUT_CLASSES}
      value={value}
      onChange={(e) => onChange(e.target.value as ModrinthSearchIndex)}
    >
      {SORT_OPTIONS.map(([index, label]) => (
        <option key={index} value={index}>
          {label}
        </option>
      ))}
    </select>
  </label>
);

/**
 * A Modrinth search box with paginated results. Text that looks like a
 * Modrinth link (contains `modrinth.com/`) is resolved through the core
 * instead, and the linked project is picked as soon as it resolves.
 */
export default function ModrinthSearch({
  projectType,
  gameVersion,
  loader,
  index,
  initialText = '',
  placeholder,
  toolbar,
  installedProjectIds,
  onPick,
}: {
  projectType: ModrinthProjectType;
  gameVersion?: string;
  loader?: string;
  index: ModrinthSearchIndex;
  initialText?: string;
  placeholder: string;
  /** filters and sort controls, shown under the search box */
  toolbar?: ReactNode;
  installedProjectIds?: Set<string>;
  onPick: (pick: ModrinthPick) => void;
}) {
  const [text, setText] = useState(initialText);
  const [debounced] = useDebounceValue(text.trim(), DEBOUNCE_MS);
  const isLink = looksLikeModrinthLink(debounced);

  const resolve = useModrinthResolve(isLink ? debounced : null);
  // pick a resolved link once, so going back to the results doesn't reopen it
  const pickedLink = useRef<string | null>(null);
  const onPickRef = useRef(onPick);
  useEffect(() => {
    onPickRef.current = onPick;
  });
  const resolved: ModrinthUrlTarget | undefined = isLink
    ? resolve.data
    : undefined;
  useEffect(() => {
    if (!resolved || pickedLink.current === debounced) return;
    pickedLink.current = debounced;
    onPickRef.current(pickFromTarget(resolved));
  }, [resolved, debounced]);

  const params: Omit<ModrinthSearchParams, 'offset'> = {
    query: debounced || undefined,
    project_type: projectType,
    game_version: gameVersion,
    loader,
    index,
    limit: PAGE_SIZE,
  };
  // the page belongs to one search; a new search starts on the first page
  const paramsKey = JSON.stringify(params);
  const [pageState, setPageState] = useState({ key: paramsKey, page: 0 });
  const page = pageState.key === paramsKey ? pageState.page : 0;
  const setPage = (newPage: number) =>
    setPageState({ key: paramsKey, page: newPage });

  const search = useModrinthSearch(
    { ...params, offset: page * PAGE_SIZE },
    !isLink
  );
  const totalPages = search.data
    ? Math.max(1, Math.ceil(search.data.total_hits / PAGE_SIZE))
    : 1;

  return (
    <div className="flex flex-col gap-3">
      <div className="relative">
        <FontAwesomeIcon
          icon={faMagnifyingGlass}
          className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-white/30"
        />
        <input
          className={cn(INPUT_CLASSES, 'w-full pl-9')}
          type="text"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={placeholder}
          autoComplete={DISABLE_AUTOFILL}
          aria-label="Search Modrinth"
        />
      </div>
      {toolbar}

      {isLink ? (
        resolve.error ? (
          <div className="text-medium text-red-200">
            Couldn&apos;t open the link: {errorToString(resolve.error)}
          </div>
        ) : resolve.isPending ? (
          <div className="text-medium text-white/50">Opening the link...</div>
        ) : (
          resolved && (
            <button
              type="button"
              className="self-start text-medium text-blue-200 hover:underline"
              onClick={() => onPick(pickFromTarget(resolved))}
            >
              Open {resolved.slug}
            </button>
          )
        )
      ) : search.error ? (
        <div className="text-medium text-red-200">
          Search failed: {errorToString(search.error)}
        </div>
      ) : !search.data ? (
        <div className="text-medium text-white/50">Searching...</div>
      ) : search.data.hits.length === 0 ? (
        <div className="text-medium text-white/50">No results.</div>
      ) : (
        <>
          <div className="flex flex-col gap-2">
            {search.data.hits.map((hit) => (
              <ModrinthHitCard
                key={hit.project_id}
                hit={hit}
                installed={installedProjectIds?.has(hit.project_id)}
                onClick={() =>
                  onPick({
                    projectId: hit.project_id,
                    versionId: null,
                    projectType: hit.project_type,
                    author: hit.author,
                  })
                }
              />
            ))}
          </div>
          <div className="flex flex-row items-center justify-center gap-3 text-medium text-white/50">
            <IconButton
              icon={faChevronLeft}
              disabled={page === 0 || search.isFetching}
              onClick={() => setPage(page - 1)}
            />
            <span>
              Page {page + 1} of {totalPages}
            </span>
            <IconButton
              icon={faChevronRight}
              disabled={page + 1 >= totalPages || search.isFetching}
              onClick={() => setPage(page + 1)}
            />
          </div>
        </>
      )}
    </div>
  );
}

const pickFromTarget = (target: ModrinthUrlTarget): ModrinthPick => ({
  projectId: target.project_id,
  versionId: target.version_id,
  projectType: target.project_type,
});
