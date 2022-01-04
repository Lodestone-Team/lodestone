import "./Instance.css";

import React, { useContext, useEffect, useState } from "react";
import { faCircle, faExclamationCircle, faPauseCircle, faPlay, faStop, faStopCircle } from '@fortawesome/free-solid-svg-icons'

import Card from "./Card";
import Icon from "../components/Icon";
import OverlayTrigger from "react-bootstrap/OverlayTrigger";
import {ServerContext} from "../contexts/ServerContext";
import Tooltip from "react-bootstrap/Tooltip";
import { faCircle as faRing } from '@fortawesome/free-regular-svg-icons'
import { toast } from 'react-toastify';

var utils = require("../utils")


async function getStatus(uuid, domain, port) {
  // 
  let response = await fetch(`https://${domain}:${port}/api/instance/${uuid}/status`);
  let status = await response.text();
  return status;
}

async function getPlayercount(uuid, domain, port) {
  //TODO: replace placeholder value with actual playercount

  //randomly return a number between 0 and 10
  return Math.floor(Math.random() * 10) + "/20";
}

export default function Instance({ name, version, flavour, port, uuid }) {
  const [playerCount, setPlayerCount] = useState("");
  const [status, setStatus] = useState("");
  const {pollrate, domain, webport} = useContext(ServerContext);

  // useEffect(() => {
  //   getStatus(uuid, domain, webport).then(setStatus);
  //   getPlayercount(uuid, domain, webport).then(setPlayerCount);
  // }, [uuid, domain, webport]);

  utils.useInterval(() => {
    getStatus(uuid, domain, webport).then(setStatus);
    getPlayercount(uuid, domain, webport).then(setPlayerCount);
  }, pollrate, true);

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
    fetch(`https://${domain}:8000/api/instance/${uuid}/start`, {
      method: 'POST',
    }).then(response => {
      if (response.ok) {
        toast.success("Starting Server");
        getStatus(uuid).then(setStatus);
      } else {
        response.text().then(toast.error);
      }
    }).catch(error => {
      console.error(error);
      toast.error("Failed to communicate with server.");
    });
  }

  let stopServer = () => {
    //POST /api/instance/{uuid}/start
    fetch(`https://${domain}:8000/api/instance/${uuid}/stop`, {
      method: 'POST',
    }).then(response => {
      if (response.ok) {
        toast.success("Stopping Server");
        getStatus(uuid).then(setStatus);
      } else {
        response.text().then(toast.error);
      }
    }).catch(error => {
      console.error(error);
      toast.error("Failed to communicate with server.");
    });
  }

  return (
    <Card className={"instance " + status} >
      <div className="title-bar">
        <h2 className="title">{utils.truncateString(name, 10)}</h2>
        <h3 className="subtitle">{domain}:{port}</h3>
      </div>
      <small>{utils.capitalize(flavour)} {version}</small>
      <span className="player-count">{playerCount ? playerCount : "..."}</span>

      <div className="status-bar">
        {renderStatusDot(status)}
        <span className="status">{status ? utils.capitalize(status) : "..."}</span>
        <span className="instance-actions">
          <OverlayTrigger
            placement="top"
            overlay={<Tooltip>Start Server</Tooltip>}
          >
            <Icon icon={faPlay} className="safe clickable" onClick={startServer} />
          </OverlayTrigger>

          <OverlayTrigger
            placement="top"
            overlay={<Tooltip>Stop Server</Tooltip>}
          >
            <Icon icon={faStop} className="caution clickable" onClick={stopServer} />
          </OverlayTrigger>
        </span>
      </div>

    </Card>
  )
}