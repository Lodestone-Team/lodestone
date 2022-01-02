import "./Instance.css";
import "./Card.css";

import React, { useEffect, useState } from "react";
import { faCircle, faExclamationCircle, faPauseCircle, faPlay, faStop, faStopCircle, faTrash } from '@fortawesome/free-solid-svg-icons'

import Card from "./Card";
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome'
import { faCircle as faRing } from '@fortawesome/free-regular-svg-icons'

var utils = require("../utils")


async function getStatus(uuid) {
  //TODO: replace placeholder value with actual status
  const status = ["starting", "running", "stopping", "stopped", "error"];
  //randomly choose a status
  return status[Math.floor(Math.random() * status.length)];
}

async function getPlayercount(uuid) {
  //TODO: replace placeholder value with actual playercount
  
  //randomly return a number between 0 and 10
  return Math.floor(Math.random() * 10)+"/20";
}

const domain = window.location.host.split(":")[0];

const tiltOptions = {
  scale: 1.05,
  speed: 1000,
  max: 10
};

export default function Instance({ name, version, flavour, port, uuid }) {
  const [playerCount, setPlayerCount] = useState("");
  const [status, setStatus] = useState("");

  function renderStatusDot(status) {
    switch (status) {
      case "starting":
        return <FontAwesomeIcon icon={faRing} className="status-dot green" />
      case "running":
        return <FontAwesomeIcon icon={faCircle} className="status-dot green" />
      case "stopping":
        return <FontAwesomeIcon icon={faPauseCircle} className="status-dot gray" />
      case "stopped":
        return <FontAwesomeIcon icon={faStopCircle} className="status-dot gray" />
      default:
        return <FontAwesomeIcon icon={faExclamationCircle} className="status-dot red" />
    }
  }

  React.useEffect(() => {
    getStatus(uuid).then(setStatus);
    getPlayercount(uuid).then(setPlayerCount);
  }, [uuid]);

  return (
    <Card className={"instance " + status} options={tiltOptions} >
      <div className="title-bar">
        <h2 className="title">{name}</h2>
        <h3 className="subtitle">{domain}:{port}</h3>
      </div>
      <h4 className="small">{utils.capitalize(flavour)} {version}</h4>
      <span className="player-count">{playerCount ? playerCount : "..."}</span>

      <div className="status-bar">
        {renderStatusDot(status)}
        <span className="status">{status ? utils.capitalize(status) : "..."}</span>
        <span className="instance-actions">
          <FontAwesomeIcon icon={faPlay} className="safe" />
          <FontAwesomeIcon icon={faStop} className="caution" />
          {/* <FontAwesomeIcon icon={faTrash} className="danger" /> */}
        </span>
      </div>

    </Card>
  )
}