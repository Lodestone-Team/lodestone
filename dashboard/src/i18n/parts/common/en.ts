// Words used all over the dashboard
export default {
  'common.cancel': 'Cancel',
  'common.confirm': 'Confirm',
  'common.save': 'Save',
  'common.saved': 'Saved',
  'common.saving': 'Saving…',
  'common.delete': 'Delete',
  'common.remove': 'Remove',
  'common.rename': 'Rename',
  'common.edit': 'Edit',
  'common.create': 'Create',
  'common.add': 'Add',
  'common.close': 'Close',
  'common.back': 'Back',
  'common.next': 'Next',
  'common.done': 'Done',
  'common.copy': 'Copy',
  'common.copied': 'Copied',
  'common.copiedToClipboard': 'Copied to clipboard',
  'common.retry': 'Retry',
  'common.refresh': 'Refresh',
  'common.search': 'Search',
  'common.loading': 'Loading…',
  'common.yes': 'Yes',
  'common.no': 'No',
  'common.ok': 'OK',
  'common.on': 'On',
  'common.off': 'Off',
  'common.enabled': 'Enabled',
  'common.disabled': 'Disabled',
  'common.none': 'None',
  'common.unknown': 'Unknown',
  'common.default': 'Default',
  'common.optional': 'Optional',
  'common.required': 'Required',
  'common.learnMore': 'Learn more',
  'common.showMore': 'Show more',
  'common.showLess': 'Show less',
  'common.you': 'You',
  'common.unknownError': 'Unknown error',
  'common.networkError': 'Network error',
  'common.networkErrorWithMessage': 'Network error: {message}',
  'common.invalidToken': 'Invalid token: try to log out and log in again',
  'common.notAnInteger': 'Not an integer',
  'common.notANumber': 'Not a valid number',
  'common.justNow': 'just now',
  /** a page title in the browser tab, e.g. "Settings - Lodestone" */
  'common.pageTitle': '{page} - Lodestone',
  /** a server's state (InstanceState), e.g. common.state.Running */
  'common.state.Starting': 'Starting',
  'common.state.Running': 'Running',
  'common.state.Stopping': 'Stopping',
  'common.state.Stopped': 'Stopped',
  'common.state.Error': 'Error',
  // Form fields and settings rows (Atoms/Form, Atoms/Config)
  /** a form field’s label above the field, e.g. "Name:" */
  'common.fieldLabel': '{label}:',
  /** a dropdown with no value chosen yet */
  'common.select': 'Select…',
  /** shown under a setting the user isn’t allowed to see */
  'common.noPermission': 'No permission',
  'common.input.cannotBeEmpty': 'Cannot be empty',
  'common.input.mustBeNumber': 'Must be a number',
  /** {min} is the smallest allowed value */
  'common.input.mustBeGreaterThan': 'Must be greater than {min}',
  /** {max} is the largest allowed value */
  'common.input.mustBeLessThan': 'Must be less than {max}',
  /** a combo box option that adds what the user typed, {query} */
  'common.combo.addCustom': 'Add “{query}”',
  /** a combo box search that matches no option */
  'common.combo.nothingFound': 'Nothing found.',
  // Table
  /** an empty table */
  'common.table.noData': 'No data currently available',
  // ClipboardTextfield: text that copies itself when clicked
  /** tooltip */
  'common.clipboard.copyToClipboard': 'Copy to clipboard',
  /** a toast; {text} is what was copied, e.g. an address */
  'common.clipboard.copiedText': 'Copied “{text}”',
  // The server resource pack URL check (utils/resourcePack.ts)
  'common.resourcePack.urlSpaces': 'The URL cannot contain spaces.',
  'common.resourcePack.notAUrl': 'That is not a URL.',
  'common.resourcePack.urlScheme':
    'The URL must start with http:// or https://.',
} as const;
