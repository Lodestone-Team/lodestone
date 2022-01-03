import "./Instance.css";
import "./Card.css";

import React, { useEffect, useState } from "react";
import { faCircle, faExclamationCircle, faPauseCircle, faPlay, faStop, faStopCircle } from '@fortawesome/free-solid-svg-icons'

import Card from "./Card";
import Icon from "./Icon";
import OverlayTrigger from "react-bootstrap/OverlayTrigger";
import Tooltip from "react-bootstrap/Tooltip";
import { faCircle as faRing } from '@fortawesome/free-regular-svg-icons'
import { toast } from 'react-toastify';

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
  return Math.floor(Math.random() * 10) + "/20";
}

const domain = window.location.host.split(":")[0];

export default function Instance({ name, version, flavour, port, uuid }) {
  const [playerCount, setPlayerCount] = useState("");
  const [status, setStatus] = useState("");

  useEffect(() => {
    getStatus(uuid).then(setStatus);
    getPlayercount(uuid).then(setPlayerCount);
  }, [uuid]);

  function renderStatusDot(status) {
    switch (status) {
      case "starting":
        return <Icon icon={faRing} className="status-dot green" />
      case "running":
        return <Icon icon={faCircle} className="status-dot green" />
      case "stopping":
        return <Icon icon={faPauseCircle} className="status-dot gray" />
      case "stopped":
        return <Icon icon={faStopCircle} className="status-dot gray" />
      default:
        return <Icon icon={faExclamationCircle} className="status-dot red" />
    }
  }

  let startServer = () => {
    //POST /api/instance/{uuid}/start
    fetch(`https://${domain}:${port}/api/instance/${uuid}/start`, {
      method: 'POST',
    }).then(response => {
      if (response.ok) {
        toast.success("Server started");
        getStatus(uuid).then(setStatus);
      } else {
        toast.error(response.body);
      }
    }).catch(error => {
      toast.error("Failed to communicate with server.");
    });
  }

  return (
    <Card className={"instance " + status} >
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
          <OverlayTrigger
            placement="top"
            overlay={<Tooltip>Start Server</Tooltip>}
          >
            <Icon icon={faPlay} className="safe" onClick={startServer} />
          </OverlayTrigger>

          <OverlayTrigger
            placement="top"
            overlay={<Tooltip>Stop Server</Tooltip>}
          >
            <Icon icon={faStop} className="caution" onClick={startServer} />
          </OverlayTrigger>
        </span>
      </div>

    </Card>
  )
}