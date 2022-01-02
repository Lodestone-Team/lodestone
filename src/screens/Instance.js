import "./Instance.css";
import "./Card.css";

import React, { useEffect, useState } from "react";
import { faPlay, faStop, faTrash } from '@fortawesome/free-solid-svg-icons'

import { FontAwesomeIcon } from '@fortawesome/react-fontawesome'

var utils = require("../utils")

async function getStatus(uuid) {
  //TODO: replace placeholder value with actual status
  return "Running";
}

async function getPlayercount(uuid) {
  //TODO: replace placeholder value with actual playercount
  return "3/20";
}

const domain = window.location.host.split(":")[0];

export default function Instance({ name, version, flavour, port, uuid }) {
  const [playerCount, setPlayerCount] = useState("");
  const [status, setStatus] = useState("");

  React.useEffect(() => {
    getStatus(uuid).then(setStatus);
    getPlayercount(uuid).then(setPlayerCount);
  }, [uuid]);

  return (
    <div className="instance card">
      <div className="title-bar">
        <h2 className="title">{name}</h2>
        <h3 className="subtitle">{domain}:{port}</h3>
      </div>
      <h4 className="small">{utils.capitalize(flavour)} {version}</h4>
      <span className="player-count">{playerCount ? playerCount : "..."}</span>

      <div className="status-bar">
        <span className="status">{status ? status : "..."}</span>
        <span className="instance-actions">
          <FontAwesomeIcon icon={faPlay} className="start-icon" />
          <FontAwesomeIcon icon={faStop} className="stop-icon" />
          <FontAwesomeIcon icon={faTrash} className="delete-icon" />
        </span>
      </div>

    </div>
  )
}