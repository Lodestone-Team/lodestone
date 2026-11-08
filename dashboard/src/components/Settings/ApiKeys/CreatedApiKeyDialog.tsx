import { faClone } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import Button from 'components/Atoms/Button';
import { cliCoreUrl } from 'data/ApiKeys';
import { LodestoneContext } from 'data/LodestoneContext';
import { Fragment, useContext } from 'react';
import { toast } from 'react-toastify';

// The toasts never contain what was copied: it holds the key.
const copyToClipboard = async (text: string, what: string) => {
  try {
    // navigator.clipboard only exists on HTTPS and localhost
    await navigator.clipboard.writeText(text);
    toast.success(`${what} copied to the clipboard`);
  } catch {
    toast.error(
      "Couldn't copy to the clipboard. Select the text and copy it by hand."
    );
  }
};

const CodeBlock = ({ text, label }: { text: string; label: string }) => (
  <div className="flex flex-row items-start gap-2 rounded-lg border border-gray-faded/30 bg-gray-900 p-3">
    <code className="min-w-0 grow font-mono text-small break-all text-gray-300 select-all">
      {text}
    </code>
    <Button
      label="Copy"
      icon={faClone}
      className="shrink-0"
      onClick={() => copyToClipboard(text, label)}
    />
  </div>
);

/**
 * Shows a new API key, once. The key is only in the created prop: the parent
 * clears it in afterLeave, when this dialog has closed.
 */
export default function CreatedApiKeyDialog({
  created,
  isOpen,
  onClose,
  afterLeave,
}: {
  created: CreatedApiKey | null;
  isOpen: boolean;
  onClose: () => void;
  afterLeave: () => void;
}) {
  const { core } = useContext(LodestoneContext);
  const cliExample = created
    ? `LODESTONE_URL=${cliCoreUrl(core)} LODESTONE_TOKEN=${
        created.key
      } lodestone_cli instance list`
    : '';

  return (
    <Transition
      appear
      show={isOpen && created !== null}
      as={Fragment}
      afterLeave={afterLeave}
    >
      {/* Clicking outside or pressing Escape doesn't close it: the key would
          be lost. Only the Done button does. */}
      <Dialog onClose={() => undefined} className="relative z-60">
        <ModalBackdrop />
        <div className="fixed inset-0 overflow-y-auto">
          <div className="flex min-h-full items-center justify-center p-4">
            <ModalPanel className="flex w-150 max-w-full flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
              <Dialog.Title className="text-h2 font-extrabold tracking-tight text-gray-300">
                API key &quot;{created?.info.name}&quot; created
              </Dialog.Title>
              <p className="text-h3 font-bold tracking-medium text-yellow-300">
                Copy this key now. You won&apos;t be able to see it again.
              </p>
              {created && <CodeBlock text={created.key} label="API key" />}
              <p className="mt-2 text-medium font-medium tracking-medium text-white/50">
                Use it with the Lodestone CLI:
              </p>
              {created && <CodeBlock text={cliExample} label="CLI example" />}
              <p className="text-medium font-medium tracking-medium text-white/50">
                Store the key like a password. Anyone who has it can use
                Lodestone with its access until it expires or you revoke it.
              </p>
              <div className="flex flex-row justify-end">
                <Button label="Done" intention="primary" onClick={onClose} />
              </div>
            </ModalPanel>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
