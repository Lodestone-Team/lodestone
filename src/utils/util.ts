import { InstanceState } from 'data/InstanceList';
import { LabelColor } from "components/Label";
import { NextRouter } from 'next/router';

export const capitalizeFirstLetter = (string: string) => {
  return string.charAt(0).toUpperCase() + string.slice(1);
};

// a map from InstanceStatus to string names
// instancestatus is a union type
export const stateToLabelColor: { [key in InstanceState]: LabelColor } = {
  Running: 'green',
  Starting: 'ochre',
  Stopping: 'ochre',
  Stopped: 'gray',
  Error: 'red',
  // Loading: 'gray',
};

export const pushKeepQuery = (router: NextRouter, pathname: string) => {
  router.push({
    pathname,
    query: router.query,
  },
  undefined,
  { shallow: true });
};
