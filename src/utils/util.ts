import { LabelColor } from "components/Label";
import { InstanceStatus } from "data/InstanceList";

export const capitalizeFirstLetter = (string: string) => {
  return string.charAt(0).toUpperCase() + string.slice(1);
};

// a map from InstanceStatus to string names
// instancestatus is a union type
export const statusToLabelColor: { [key in InstanceStatus]: LabelColor } = {
  stopped: 'gray',
  running: 'green',
  starting: 'ochre',
  stopping: 'ochre',
  crashed: 'red',
  error: 'red',
  loading: 'gray',
};
