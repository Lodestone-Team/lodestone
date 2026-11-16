import { ClientFile } from 'bindings/ClientFile';
import { ObjectChip, ObjectKind } from 'components/Atoms/Values';
import { ObjectToast } from 'components/Values';
import { toast, TypeOptions } from 'react-toastify';
import { supportedZip } from 'utils/util';

/** The kind icon of a file: a folder, an archive Lodestone can unzip, or a file */
export const fileKind = (
  file: Pick<ClientFile, 'file_type' | 'extension'>
): ObjectKind =>
  file.file_type === 'Directory'
    ? 'folder'
    : supportedZip.includes(file.extension ?? '')
      ? 'archive'
      : 'file';

/** The last part of a path, whichever separator it uses */
export const baseName = (path: string) =>
  path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/**
 * A toast about a file: its name (never the whole path) as a mono chip with
 * its kind icon, the outcome, and the reason (the core's error) under it.
 */
export function fileToast(
  type: TypeOptions,
  file: { name: string; kind: ObjectKind },
  outcome: string,
  reason?: string | null
) {
  toast(
    <ObjectToast
      object={<ObjectChip kind={file.kind} name={file.name} mono />}
      outcome={outcome}
      reason={reason}
    />,
    { type }
  );
}
