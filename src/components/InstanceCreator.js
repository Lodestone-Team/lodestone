import "./InstanceCreator.scss";

import React, { useContext, useEffect, useState } from "react";

import Button from "react-bootstrap/Button";
import CloseButton from "react-bootstrap/CloseButton";
import Form from "react-bootstrap/Form";
import Icon from "../components/Icon";
import Modal from "react-bootstrap/Modal";
import OverlayTrigger from "react-bootstrap/OverlayTrigger";
import PlusIcon from "../assets/plus.svg";
import { ServerContext } from "../contexts/ServerContext";
import Tooltip from "react-bootstrap/Tooltip";
import { faQuestionCircle } from '@fortawesome/free-solid-svg-icons'
import { toast } from 'react-toastify';

var utils = require("../utils")


export default function InstanceCreator() {
  const [show, setShow] = useState(false);
  const [flavours, setFlavours] = useState([]);
  const [name, setName] = useState("");
  const [flavour, setFlavour] = useState("");
  const [uuid, setUUID] = useState("");
  const [url, setURL] = useState("");
  const [versions, setVersions] = useState([]);
  const [version, setVersion] = useState("");
  const [ready, setReady] = useState(false);
  const { pollrate, domain, webport } = useContext(ServerContext);


  useEffect(() => {
    fetch(`https://${domain}:${webport}/api/jar/flavours`)
      .then((response) => response.json())
      .then((data) => {
        setFlavours(data)
      })
  }, [show, domain, webport]);


  useEffect(() => {
    if (flavour) {
      fetch(`https://${domain}:${webport}/api/jar/${flavour}/versions`)
        .then((response) => response.json())
        .then((data) => {
          setVersions(data)
        })
    }
  }, [flavour, domain, webport]);

  useEffect(() => {
    console.log(name)
    if (version.length <= 0) return setReady(false);
    if (name.length <= 0) return setReady(false);

    fetch(`https://${domain}:${webport}/api/jar/${flavour}/${version}`).then(response => {
      if (!response.ok) return;

      response.text().then(url => {
        setURL(url);
        setReady(true);
      });
    })

  }, [flavour, version, domain, webport, name]);

  let createInstance = (event) => {
    event.preventDefault();
    if (!ready) {
      toast.error("Please fill out all fields");
      return;
    }

    let payload = JSON.stringify({ name, flavour, version, url });
    console.log(payload);

    const creationToast = toast.loading("Creating instance...")

    fetch(`https://${domain}:${webport}/api/instance/${uuid}`, {
      method: "POST",
      body: payload,
    }).then(response => {
      if (!response.ok) {
        toast.update(creationToast, { render: "Failed to create instance", type: "error" });
        return;
      }
      toast.update(creationToast, { render: "Successfully created instance!", type: "success" });
      setShow(false);
    })

  };

  return (
    <>
      <img src={PlusIcon} alt="Plus Icon" className="new-instance-button clickable" onClick={() => {
        setShow(true);
        setName("");
        setVersion("");
        setFlavour("");
        setReady(false);
      }} />
      <Modal show={show} onHide={() => setShow(false)}
        size="md"
        centered
        contentClassName="card main"
      >
        <div className="title-bar">
          <h2 className="title">Create new Instance</h2>
          <CloseButton onClick={() => setShow(false)} />
        </div>
        <Form onSubmit={createInstance}>
          <Form.Group controlId="creationForm.Name" className="mb-3">
            <Form.Label>Instance Name</Form.Label>
            <Form.Control autoComplete="off" type="text" placeholder="My Instance"
              value={name} onChange={(event) => {
                setName(event.target.value)
                setUUID(`${event.target.value.replace(/[^0-9a-zA-Z]+/g, '')}-${Date.now().toString(16)}-${Math.floor(Math.random() * 1024)}`)
              }} />
            <Form.Text id="uuidBlock" muted>
              UUID: {name ? uuid : ""}
              <OverlayTrigger
                placement="top"
                overlay={<Tooltip>The unique ID of your instance, this value is auto generated and cannot be changed.</Tooltip>}
              >
                <Icon icon={faQuestionCircle} className="gray form-description-explainer" />
              </OverlayTrigger>
            </Form.Text>

          </Form.Group>
          <Form.Group className="mb-3">
            <Form.Label>Flavour</Form.Label>

            <div key="flavours" >
              {flavours.map((myFlavour) => (
                <Form.Check
                  inline
                  key={myFlavour}
                  type="radio"
                  label={utils.capitalize(myFlavour)}
                  name="flavour"
                  value={myFlavour}
                  onChange={(event) => setFlavour(event.target.value)}
                  checked={myFlavour === flavour}
                />))}
            </div>
          </Form.Group>
          <div className="mb-3 version-row">
            {/* <Form.Group className="snapshot-checkbox">
              <Form.Label>Filter</Form.Label>
              <Form.Check
                type="checkbox"
                label="Snapshots"
              />
            </Form.Group> */}
            <Form.Group className="flex-grow-1">
              <Form.Label>Minecraft Version</Form.Label>
              <Form.Select value={version} onChange={(event) => setVersion(event.target.value)} >
                <option value="" selected disabled>Choose a version</option>
                {versions.map((myVersion) => (
                  <option key={myVersion} value={myVersion}>{myVersion}</option>
                ))}
              </Form.Select>
            </Form.Group>
          </div>
          <div className="d-grid create-button-wrapper">
            <Button variant="primary" type="submit" size="lg" disabled={!ready}>
              Create!
            </Button>
          </div>
        </Form>
      </Modal>
    </>
  );
}