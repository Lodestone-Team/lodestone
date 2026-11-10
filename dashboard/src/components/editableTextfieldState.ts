export type EditableTextfieldState = {
  /** the committed text shown when not editing */
  displayText: string;
  /** the text in the input while editing */
  editText: string;
  isEditing: boolean;
  /** message from the last failed save, empty when there is none */
  error: string;
};

/**
 * The state after a save of `submitted` settled. `error` is the message the
 * submit rejected with, or empty when it succeeded. A successful save, changed
 * or not, leaves edit mode; a failed one stays in it with the attempted text
 * and the error visible.
 */
export function settleSave(
  state: EditableTextfieldState,
  submitted: string,
  error: string
): EditableTextfieldState {
  if (error) {
    return { ...state, editText: submitted, isEditing: true, error };
  }
  return {
    displayText: submitted,
    editText: submitted,
    isEditing: false,
    error: '',
  };
}
