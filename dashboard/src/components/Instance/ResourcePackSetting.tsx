import { useQuery, useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ResourcePackHash } from 'bindings/ResourcePackHash';
import { ServerResourcePack } from 'bindings/ServerResourcePack';
import Button from 'components/Atoms/Button';
import { Toggle } from 'components/Atoms/Toggle';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { formatPackSize, isSha1, packUrlError } from 'utils/resourcePack';
import { axiosWrapper, DISABLE_AUTOFILL, errorToString } from 'utils/util';

const EMPTY: ServerResourcePack = {
  url: '',
  sha1: '',
  required: false,
  prompt: '',
};

const inputClass =
  'input-shape input-background input-outlines input-text-style input-border-normal w-full';

/**
 * The server resource pack: the `resource-pack`, `resource-pack-sha1`,
 * `require-resource-pack` and `resource-pack-prompt` lines of
 * server.properties. The core downloads the URL to compute the SHA-1.
 */
export function ResourcePackSetting({
  instance,
  canEdit,
  disabledReason,
}: {
  instance: InstanceInfo;
  canEdit: boolean;
  disabledReason: string;
}) {
  const queryClient = useQueryClient();
  const queryKey = ['instances', instance.uuid, 'resource_pack'];
  const { data: saved } = useQuery({
    queryKey,
    queryFn: () =>
      axiosWrapper<ServerResourcePack>({
        method: 'GET',
        url: `/instance/${instance.uuid}/resource_pack`,
      }),
    enabled: instance.game_type.type === 'MinecraftJava',
    retry: false,
  });
  // the edits; `null` shows the saved pack
  const [edited, setDraft] = useState<ServerResourcePack | null>(null);
  const [busy, setBusy] = useState<'saving' | 'hashing' | null>(null);
  const [hashNote, setHashNote] = useState<string | null>(null);
  if (!saved) return null;
  const draft = edited ?? saved;

  const urlError = packUrlError(draft.url);
  const sha1Error = isSha1(draft.sha1)
    ? null
    : 'The SHA-1 is 40 hexadecimal characters.';
  const changed =
    draft.url.trim() !== saved.url ||
    draft.sha1.trim().toLowerCase() !== saved.sha1 ||
    draft.required !== saved.required ||
    draft.prompt.trim() !== saved.prompt;
  const disabled = !canEdit || busy !== null;

  const save = async (pack: ServerResourcePack) => {
    setBusy('saving');
    try {
      const result = await axiosWrapper<ServerResourcePack>({
        method: 'PUT',
        url: `/instance/${instance.uuid}/resource_pack`,
        data: pack,
      });
      queryClient.setQueryData(queryKey, result);
      setDraft(null);
      toast.success(
        instance.state === 'Stopped'
          ? 'Resource pack saved'
          : 'Resource pack saved. Restart the server to use it.'
      );
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setBusy(null);
    }
  };

  const computeSha1 = async () => {
    setBusy('hashing');
    setHashNote(null);
    try {
      const hash = await axiosWrapper<ResourcePackHash>({
        method: 'POST',
        url: `/instance/${instance.uuid}/resource_pack/hash`,
        data: { url: draft.url.trim() },
      });
      setDraft({ ...draft, sha1: hash.sha1 });
      setHashNote(`Downloaded ${formatPackSize(hash.size)}.`);
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setBusy(null);
    }
  };

  const row =
    'flex flex-col gap-2 bg-gray-800 px-4 py-3 @4xl:flex-row @4xl:items-center @4xl:justify-between';
  const label = 'text-medium font-medium text-gray-300';
  const hint = 'text-medium font-medium tracking-medium text-white/50';

  return (
    <div className="mb-16 flex flex-col gap-4 @4xl:flex-row">
      <div className="w-full shrink-0">
        <h2 className="text-h2 font-extrabold tracking-medium">
          Resource Pack
        </h2>
        <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
          A pack players are asked to download when they join
        </h3>
      </div>
      <div className="w-full min-w-0 overflow-hidden rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
        <div className={row}>
          <div className="flex min-w-0 grow flex-col">
            <label className={label} htmlFor="resource-pack-url">
              URL
            </label>
            <div className={urlError ? 'text-small text-red' : hint}>
              {!canEdit
                ? disabledReason
                : (urlError ??
                  'A direct link to the pack zip. Leave it empty for none.')}
            </div>
          </div>
          <input
            id="resource-pack-url"
            className={`${inputClass} @4xl:w-5/12`}
            value={draft.url}
            placeholder="https://example.com/pack.zip"
            maxLength={2048}
            disabled={disabled}
            autoComplete={DISABLE_AUTOFILL}
            onChange={(e) => {
              setDraft({ ...draft, url: e.target.value });
              setHashNote(null);
            }}
          />
        </div>
        <div className={row}>
          <div className="flex min-w-0 grow flex-col">
            <label className={label} htmlFor="resource-pack-sha1">
              SHA-1
            </label>
            <div className={sha1Error ? 'text-small text-red' : hint}>
              {sha1Error ??
                hashNote ??
                'Lets players check the download and cache the pack.'}
            </div>
          </div>
          <div className="flex flex-row gap-2 @4xl:w-5/12">
            <input
              id="resource-pack-sha1"
              className={`${inputClass} font-mono`}
              value={draft.sha1}
              maxLength={40}
              disabled={disabled}
              autoComplete={DISABLE_AUTOFILL}
              onChange={(e) => setDraft({ ...draft, sha1: e.target.value })}
            />
            <Button
              label="Compute"
              loading={busy === 'hashing'}
              disabled={disabled || !draft.url.trim() || urlError !== null}
              onClick={() => void computeSha1()}
            />
          </div>
        </div>
        <div className={row}>
          <div className="flex min-w-0 grow flex-col">
            <label className={label}>Require the pack</label>
            <div className={hint}>Players who decline it are disconnected.</div>
          </div>
          <Toggle
            value={draft.required}
            onChange={(required) => setDraft({ ...draft, required })}
            disabled={disabled}
          />
        </div>
        <div className={row}>
          <div className="flex min-w-0 grow flex-col">
            <label className={label} htmlFor="resource-pack-prompt">
              Prompt
            </label>
            <div className={hint}>
              The message shown when players are asked. Plain text, or a JSON
              text component.
            </div>
          </div>
          <input
            id="resource-pack-prompt"
            className={`${inputClass} @4xl:w-5/12`}
            value={draft.prompt}
            maxLength={2048}
            disabled={disabled}
            autoComplete={DISABLE_AUTOFILL}
            onChange={(e) => setDraft({ ...draft, prompt: e.target.value })}
          />
        </div>
        <div className="flex flex-row justify-end gap-2 bg-gray-800 px-4 py-3">
          <Button
            label="Remove pack"
            disabled={disabled || !saved.url}
            onClick={() => void save(EMPTY)}
          />
          <Button
            label="Save"
            intention="primary"
            loading={busy === 'saving'}
            disabled={
              disabled || !changed || urlError !== null || sha1Error !== null
            }
            onClick={() => void save(draft)}
          />
        </div>
      </div>
    </div>
  );
}
