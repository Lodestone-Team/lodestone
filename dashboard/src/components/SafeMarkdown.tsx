import { ReactNode, useMemo } from 'react';
import { MdBlock, MdInline, parseMarkdown } from 'utils/markdown';
import { cn } from 'utils/util';

/*
 * Renders untrusted Markdown (Modrinth project bodies) as React elements.
 * utils/markdown.ts parses it and filters every URL; this file only maps
 * the syntax tree to elements, so there is no HTML string anywhere.
 */

const headingClasses: Record<number, string> = {
  1: 'text-h1 font-bold',
  2: 'text-h2 font-bold',
  3: 'text-h3 font-bold',
  4: 'text-medium font-bold',
  5: 'text-medium font-bold',
  6: 'text-medium font-bold text-white/50',
};

const ExternalLink = ({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) => (
  <a
    href={href}
    target="_blank"
    rel="noreferrer noopener"
    className="text-blue-200 hover:underline"
  >
    {children}
  </a>
);

const renderInline = (nodes: MdInline[], inLink = false): ReactNode[] =>
  nodes.map((node, i) => {
    switch (node.type) {
      case 'text':
        return node.text;
      case 'br':
        return <br key={i} />;
      case 'code':
        return (
          <code
            key={i}
            className="rounded-sm bg-gray-900 px-1 font-mono text-small"
          >
            {node.text}
          </code>
        );
      case 'strong':
        return <strong key={i}>{renderInline(node.children, inLink)}</strong>;
      case 'em':
        return <em key={i}>{renderInline(node.children, inLink)}</em>;
      case 'del':
        return <del key={i}>{renderInline(node.children, inLink)}</del>;
      case 'link':
        // a rejected URL, or a link inside a link, keeps only its text
        if (node.href === null || inLink)
          return <span key={i}>{renderInline(node.children, inLink)}</span>;
        return (
          <ExternalLink key={i} href={node.href}>
            {renderInline(node.children, true)}
          </ExternalLink>
        );
      case 'image':
        if (node.src === null) return node.alt;
        return (
          <img
            key={i}
            src={node.src}
            alt={node.alt}
            loading="lazy"
            referrerPolicy="no-referrer"
            className="inline-block h-auto max-w-full"
          />
        );
    }
  });

const alignClass = (align: 'left' | 'center' | 'right' | null) =>
  align === 'center'
    ? 'text-center'
    : align === 'right'
      ? 'text-right'
      : 'text-left';

const renderBlocks = (blocks: MdBlock[]): ReactNode[] =>
  blocks.map((block, i) => {
    switch (block.type) {
      case 'heading': {
        const Tag = `h${block.level}` as const;
        return (
          <Tag
            key={i}
            className={cn('mt-2 text-gray-300', headingClasses[block.level])}
          >
            {renderInline(block.children)}
          </Tag>
        );
      }
      case 'paragraph':
        return <p key={i}>{renderInline(block.children)}</p>;
      case 'code':
        return (
          <pre
            key={i}
            className="overflow-x-auto rounded-md bg-gray-900 p-3 font-mono text-small"
          >
            <code>{block.text}</code>
          </pre>
        );
      case 'blockquote':
        return (
          <blockquote
            key={i}
            className="flex flex-col gap-3 border-l-4 border-gray-faded/30 pl-3 text-white/50"
          >
            {renderBlocks(block.children)}
          </blockquote>
        );
      case 'list': {
        const items = block.items.map((item, j) => (
          <li key={j} className="[&>*+*]:mt-1">
            {renderBlocks(item)}
          </li>
        ));
        return block.ordered ? (
          <ol
            key={i}
            start={block.start}
            className="flex list-decimal flex-col gap-1 pl-6"
          >
            {items}
          </ol>
        ) : (
          <ul key={i} className="flex list-disc flex-col gap-1 pl-6">
            {items}
          </ul>
        );
      }
      case 'hr':
        return <hr key={i} className="border-gray-faded/30" />;
      case 'table':
        return (
          <div key={i} className="overflow-x-auto">
            <table className="border-collapse text-small">
              <thead>
                <tr>
                  {block.header.map((cell, j) => (
                    <th
                      key={j}
                      className={cn(
                        'border border-gray-faded/30 px-2 py-1 font-bold',
                        alignClass(block.align[j] ?? null)
                      )}
                    >
                      {renderInline(cell)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {block.rows.map((row, j) => (
                  <tr key={j}>
                    {row.map((cell, k) => (
                      <td
                        key={k}
                        className={cn(
                          'border border-gray-faded/30 px-2 py-1',
                          alignClass(block.align[k] ?? null)
                        )}
                      >
                        {renderInline(cell)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
    }
  });

/**
 * Untrusted Markdown, rendered safely: raw HTML tags are removed and their
 * text kept (except filtered links and images), links are http(s) only and
 * open in a new tab, and images are https only.
 */
export default function SafeMarkdown({
  source,
  className,
}: {
  source: string;
  className?: string;
}) {
  const blocks = useMemo(() => parseMarkdown(source), [source]);
  return (
    <div
      className={cn(
        'flex min-w-0 flex-col gap-3 text-medium leading-relaxed wrap-break-word text-gray-300',
        className
      )}
    >
      {renderBlocks(blocks)}
    </div>
  );
}
