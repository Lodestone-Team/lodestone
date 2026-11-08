import {
  faChevronLeft,
  faChevronRight,
  faMagnifyingGlass,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ModrinthProjectType } from 'bindings/ModrinthProjectType';
import { ModrinthSearchHit } from 'bindings/ModrinthSearchHit';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { ModrinthUrlTarget } from 'bindings/ModrinthUrlTarget';
import IconButton from 'components/Atoms/IconButton';
import {
  ModrinthSearchParams,
  useModrinthResolve,
  useModrinthSearch,
} from 'data/Mods';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { useDebounceCallback, useDebounceValue } from 'usehooks-ts';
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
  /** the search result it was picked from, to show before the project loads */
  hit?: ModrinthSearchHit;
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

/** A result card's shape while the results load, so nothing jumps. */
export const HitCardSkeleton = () => (
  <div className="flex h-21 w-full animate-pulse flex-row items-start gap-3 rounded-lg border border-gray-faded/30 bg-gray-800 p-3">
    <div className="size-12 shrink-0 rounded-md bg-gray-700" />
    <div className="flex grow flex-col gap-2 pt-1">
      <div className="h-4 w-1/3 rounded-sm bg-gray-700" />
      <div className="h-3 w-3/4 rounded-sm bg-gray-700" />
    </div>
  </div>
);

/**
 * A Modrinth search box with paginated results. Text that looks like a
 * Modrinth link (contains `modrinth.com/`) is resolved through the core
 * instead, and the linked project is picked as soon as it resolves.
 *
 * While a link resolves, the results of the last search stay where they are
 * with a status line above them, so the list doesn't collapse and come back.
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
  const initialIsLink = looksLikeModrinthLink(initialText.trim());
  const [text, setText] = useState(initialText);
  // The last text that was not a link: what the results are for.
  const [searchText, setSearchText] = useState(
    initialIsLink ? '' : initialText
  );
  const [debouncedSearch] = useDebounceValue(searchText.trim(), DEBOUNCE_MS);
  // The link to resolve. A pasted link is resolved at once, a typed one after
  // the usual pause (so half a link isn't sent).
  const [link, setLink] = useState(initialIsLink ? initialText.trim() : '');
  const setLinkLater = useDebounceCallback(setLink, DEBOUNCE_MS);
  const pasted = useRef(false);

  const onChange = (value: string) => {
    setText(value);
    const trimmed = value.trim();
    if (looksLikeModrinthLink(trimmed)) {
      if (pasted.current) {
        setLinkLater.cancel();
        setLink(trimmed);
      } else {
        setLinkLater(trimmed);
      }
    } else {
      setLinkLater.cancel();
      setLink('');
      setSearchText(value);
    }
    pasted.current = false;
  };
  const isLink = looksLikeModrinthLink(text.trim());
  // false while a typed link waits for the pause
  const linkReady = isLink && link === text.trim();

  const resolve = useModrinthResolve(linkReady ? link : null);
  // pick a resolved link once, so going back to the results doesn't reopen it
  const pickedLink = useRef<string | null>(null);
  const onPickRef = useRef(onPick);
  useEffect(() => {
    onPickRef.current = onPick;
  });
  const resolved: ModrinthUrlTarget | undefined = linkReady
    ? resolve.data
    : undefined;
  useEffect(() => {
    if (!resolved || pickedLink.current === link) return;
    pickedLink.current = link;
    onPickRef.current(pickFromTarget(resolved));
  }, [resolved, link]);

  const params: Omit<ModrinthSearchParams, 'offset'> = {
    query: debouncedSearch || undefined,
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

  const search = useModrinthSearch({ ...params, offset: page * PAGE_SIZE });
  const totalPages = search.data
    ? Math.max(1, Math.ceil(search.data.total_hits / PAGE_SIZE))
    : 1;

  const linkStatus = !isLink ? null : linkReady && resolve.error ? (
    <div className="text-medium text-red-200">
      Couldn&apos;t open the link: {errorToString(resolve.error)}
    </div>
  ) : resolved ? (
    <button
      type="button"
      className="self-start text-medium text-blue-200 hover:underline"
      onClick={() => onPick(pickFromTarget(resolved))}
    >
      Open {resolved.slug}
    </button>
  ) : null;
  // shown inside the search box, so the results below don't move
  const opening = isLink && !resolved && !(linkReady && resolve.error);

  return (
    <div className="flex flex-col gap-3">
      <div className="relative">
        <FontAwesomeIcon
          icon={faMagnifyingGlass}
          className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-white/30"
        />
        <input
          className={cn(INPUT_CLASSES, 'w-full pl-9', isLink && 'pr-36')}
          type="text"
          value={text}
          onPaste={() => {
            pasted.current = true;
          }}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          autoComplete={DISABLE_AUTOFILL}
          aria-label="Search Modrinth"
        />
        {opening && (
          <span
            className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-small text-white/50"
            role="status"
          >
            Opening the link...
          </span>
        )}
      </div>
      {toolbar}
      {linkStatus}

      {search.error ? (
        <div className="text-medium text-red-200">
          Search failed: {errorToString(search.error)}
        </div>
      ) : !search.data ? (
        <div className="flex flex-col gap-2" aria-busy="true">
          {Array.from({ length: 5 }, (_, i) => (
            <HitCardSkeleton key={i} />
          ))}
        </div>
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
                    hit,
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
