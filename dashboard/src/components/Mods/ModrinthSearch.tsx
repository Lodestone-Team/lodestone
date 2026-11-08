import {
  faChevronLeft,
  faChevronRight,
  faMagnifyingGlass,
  faSort,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  Label,
  Listbox,
  ListboxButton,
  ListboxOption,
  ListboxOptions,
} from '@headlessui/react';
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
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';

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

/**
 * The sort order of a Modrinth search: a compact dropdown in the same style as
 * the dashboard's other dropdowns (SelectBox), not the browser's native select,
 * which macOS draws as a system popup button.
 */
export const SortSelect = ({
  value,
  onChange,
}: {
  value: ModrinthSearchIndex;
  onChange: (index: ModrinthSearchIndex) => void;
}) => {
  const current = SORT_OPTIONS.find(([index]) => index === value)?.[1] ?? value;
  return (
    <Listbox value={value} onChange={onChange}>
      <div className="flex flex-row items-center gap-2">
        <Label className="text-medium font-medium tracking-medium text-white/50">
          Sort by
        </Label>
        <ListboxButton
          className={cn(
            'input-outlines input-text-style input-border-normal group relative',
            'min-w-44 rounded py-1.5 pr-9 pl-3 text-left font-medium',
            'cursor-pointer hover:outline-white/30',
            'ui-open:bg-gray-700 ui-not-open:bg-gray-850 ui-not-open:hover:bg-gray-700'
          )}
        >
          {current}
          <FontAwesomeIcon
            icon={faSort}
            className="pointer-events-none absolute top-1/2 right-3 w-4 -translate-y-1/2 text-gray-faded/30 group-hover:text-gray-500"
          />
        </ListboxButton>
        <ListboxOptions
          anchor={{ to: 'bottom end', gap: 8 }}
          transition
          className={cn(
            'input-text-style z-40 w-(--button-width) overflow-auto rounded py-1',
            'bg-gray-850 outline-1 outline-gray-550 outline-solid drop-shadow-md',
            'transition duration-150 ease-in data-leave:data-closed:opacity-0'
          )}
        >
          {SORT_OPTIONS.map(([index, label]) => (
            <ListboxOption
              key={index}
              value={index}
              className={cn(
                'cursor-pointer py-2 pr-4 pl-3 font-medium text-gray-300 select-none',
                'data-focus:bg-gray-800 data-selected:bg-gray-700 data-selected:data-focus:bg-gray-600'
              )}
            >
              {label}
            </ListboxOption>
          ))}
        </ListboxOptions>
      </div>
    </Listbox>
  );
};

/** A result card's shape while the results load, so nothing jumps. */
export const HitCardSkeleton = () => (
  <div className="flex h-21 w-full flex-row items-start gap-3 rounded-lg border border-gray-faded/30 bg-gray-800 p-3">
    <Skeleton className="size-12 shrink-0" />
    <div className="flex grow flex-col gap-2 pt-1">
      <Skeleton className="h-4 w-1/3 rounded-sm" />
      <Skeleton className="h-3 w-3/4 rounded-sm" />
      <Skeleton className="h-3 w-1/2 rounded-sm" />
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
  showClientSide = false,
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
  /** for mods: show on each result whether players need it too */
  showClientSide?: boolean;
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
        <SkeletonGroup label="Loading results" className="flex flex-col gap-2">
          {Array.from({ length: 5 }, (_, i) => (
            <HitCardSkeleton key={i} />
          ))}
        </SkeletonGroup>
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
                showClientSide={showClientSide}
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
