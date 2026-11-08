import { ReactNode, useEffect, useRef, useState } from 'react';
import { cn } from 'utils/util';

/** The classes of an entry in a list of settings sections or pages */
export const settingsNavItemClass = ({
  active,
  danger = false,
  activeClass = 'bg-gray-850 text-gray-300',
}: {
  active: boolean;
  danger?: boolean;
  /** the highlight, one step lighter than the surface the list is on */
  activeClass?: string;
}) =>
  cn(
    'flex w-full flex-row items-center gap-2 rounded-lg px-2.5 py-1.5 text-left',
    'text-medium font-mediumbold',
    'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
    active
      ? activeClass
      : danger
        ? 'text-red/60 hover:text-red-200'
        : 'text-gray-500 hover:text-gray-300'
  );

type TocItem = { id: string; title: string; danger: boolean };

// how far under the top of the scroll area a section counts as the current one
const CURRENT_OFFSET = 60;

/** The nearest ancestor that scrolls vertically, null for the page itself */
function scrollParent(element: HTMLElement): HTMLElement | null {
  for (let p = element.parentElement; p; p = p.parentElement) {
    if (/(auto|scroll|overlay)/.test(getComputedStyle(p).overflowY)) return p;
  }
  return null;
}

const sections = (content: HTMLElement) => [
  ...content.querySelectorAll<HTMLElement>('section[data-settings-section]'),
];

/**
 * The content column with a list of its sections on the left (the sections
 * are the SettingsSections with an `id`). Clicking an entry scrolls to its
 * section, and the current section is highlighted as the page scrolls. The
 * list is hidden when there is no room for it.
 */
export function SettingsWithToc({ children }: { children: ReactNode }) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [items, setItems] = useState<TocItem[]>([]);
  const [active, setActive] = useState<string | null>(null);
  // while a click scrolls, the clicked section stays highlighted
  const lockTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (lockTimer.current !== null) clearTimeout(lockTimer.current);
    },
    []
  );

  // Read the sections from the page: some only render once their data has
  // loaded, so keep reading as the content changes.
  useEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const read = () => {
      const next = sections(content).map((section) => ({
        id: section.id,
        title: section.dataset.settingsSection ?? '',
        danger: section.dataset.danger === 'true',
      }));
      setItems((old) =>
        old.length === next.length &&
        old.every(
          (item, i) =>
            item.id === next[i].id &&
            item.title === next[i].title &&
            item.danger === next[i].danger
        )
          ? old
          : next
      );
    };
    read();
    const observer = new MutationObserver(read);
    observer.observe(content, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['id', 'data-settings-section', 'data-danger'],
    });
    return () => observer.disconnect();
  }, []);

  // highlight the section at the top of the scroll area
  useEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const scroller = scrollParent(content);
    const update = () => {
      if (lockTimer.current !== null) return;
      const all = sections(content);
      if (all.length === 0) return;
      const box = scroller ?? document.documentElement;
      const top = scroller ? scroller.getBoundingClientRect().top : 0;
      let current = all[0];
      for (const section of all) {
        if (section.getBoundingClientRect().top - top <= CURRENT_OFFSET)
          current = section;
      }
      // at the end, the last section, even when it can't reach the top
      const scrollable = box.scrollHeight > box.clientHeight + 4;
      if (
        scrollable &&
        box.scrollTop + box.clientHeight >= box.scrollHeight - 4
      )
        current = all[all.length - 1];
      setActive(current.id);
    };
    update();
    const target: HTMLElement | Window = scroller ?? window;
    target.addEventListener('scroll', update, { passive: true });
    window.addEventListener('resize', update);
    return () => {
      target.removeEventListener('scroll', update);
      window.removeEventListener('resize', update);
    };
  }, [items]);

  const scrollTo = (id: string) => {
    const section = document.getElementById(id);
    if (!section) return;
    if (lockTimer.current !== null) clearTimeout(lockTimer.current);
    lockTimer.current = setTimeout(() => {
      lockTimer.current = null;
    }, 1000);
    setActive(id);
    const reduceMotion = window.matchMedia(
      '(prefers-reduced-motion: reduce)'
    ).matches;
    section.scrollIntoView({
      behavior: reduceMotion ? 'auto' : 'smooth',
      block: 'start',
    });
  };

  return (
    <div className="flex w-full flex-row items-start gap-7">
      <nav
        aria-label="Sections"
        className={cn(
          'sticky top-4 hidden w-44 shrink-0 flex-col gap-0.5',
          items.length > 1 && '@3xl:flex'
        )}
      >
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            aria-current={active === item.id ? 'location' : undefined}
            className={settingsNavItemClass({
              active: active === item.id,
              danger: item.danger,
            })}
            onClick={() => scrollTo(item.id)}
          >
            <span className="truncate">{item.title}</span>
          </button>
        ))}
      </nav>
      <div
        ref={contentRef}
        className="flex max-w-170 min-w-0 grow flex-col gap-6.5"
      >
        {children}
      </div>
    </div>
  );
}
