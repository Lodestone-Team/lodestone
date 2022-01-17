import "./Instance.css";

import React, { useContext, useState } from "react";
import { faCircle, faExclamationCircle, faPauseCircle, faPlay, faStop, faStopCircle, faTrash } from '@fortawesome/free-solid-svg-icons'

import Card from "./Card";
import Icon from "../components/Icon";
import OverlayTrigger from "react-bootstrap/OverlayTrigger";
import { ServerContext } from "../contexts/ServerContext";
import Tooltip from "react-bootstrap/Tooltip";
import { faCircle as faRing } from '@fortawesome/free-regular-svg-icons'
import { toast } from 'react-toastify';

var utils = require("../utils")

export default function Instance({ name, version, flavour, port, uuid, updateInstances }) {
  const [playerCount, setPlayerCount] = useState("");
  const [status, setStatus] = useState("");
  const { pollrate, api_domain, api_path } = useContext(ServerContext);

  const getStatus = async (uuid) => {
    let response = await fetch(`${api_domain}${api_path}/instance/${uuid}/status`);
    let status = await response.text();
    return status;
  }

  const getPlayercount = async (uuid) => {
    let response = await fetch(`${api_domain}${api_path}/instance/${uuid}/playercount`);
    let playercount = await response.text();
    return `${playercount}/20`; //TODO: get max playercount from server
  }

  utils.useInterval(() => {
    getStatus(uuid).then(setStatus);
    getPlayercount(uuid).then(setPlayerCount);
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

  const startServer = async () => {
    //POST /api/v1/instance/{uuid}/start

    fetch(`${api_domain}${api_path}/instance/${uuid}/start`, {
      method: 'POST',
    }).then(response => {
      if (response.ok) {
        toast.info("Starting Server...");
        getStatus(uuid).then(setStatus);
      } else {
        response.text().then(toast.error);
      }
    }).catch(error => {
      console.error(error);
      toast.error("Failed to connect to the server.");
    });
  }

  const stopServer = async () => {
    //POST /api/v1/instance/{uuid}/stop
    fetch(`${api_domain}${api_path}/instance/${uuid}/stop`, {
      method: 'POST',
    }).then(response => {
      if (response.ok) {
        toast.info("Stopping Server...");
        getStatus(uuid).then(setStatus);
      } else {
        response.text().then(toast.error);
      }
    }).catch(error => {
      console.error(error);
      toast.error("Failed to communicate with server.");
    });
  }

  const deleteServer = async () => {
    //DELETE /api/v1/instance/{uuid}
    fetch(`${api_domain}${api_path}/instance/${uuid}`, {
      method: 'DELETE',
    }).then(response => {
      if (response.ok) {
        toast.success("Deleted Server");
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
        <h3 className="subtitle">{window.location.hostname}:{port}</h3>
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


          {/* TODO: move delete button into setting panel once that's done */}
          <OverlayTrigger
            placement="top"
            overlay={<Tooltip>Delete Server</Tooltip>}
          >
            <Icon icon={faTrash} className="danger clickable" onClick={deleteServer} />
          </OverlayTrigger>
        </span>
      </div>

    </Card>
  )
}