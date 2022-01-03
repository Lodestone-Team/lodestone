import { FontAwesomeIcon } from '@fortawesome/react-fontawesome'
import React from "react";

// export default function Icon(props) {
//   return (<span><FontAwesomeIcon {...props} /></span>);
// }

//a class component that renders a FontAwesomeIcon
export default class Icon extends React.Component {
  render() {
    return (<FontAwesomeIcon {...this.props} />);
  }
}