import "./InstanceCreator.scss";

import React, { useContext, useEffect, useRef, useState } from "react";

import Button from "react-bootstrap/Button";
import CloseButton from "react-bootstrap/CloseButton";
import Form from "react-bootstrap/Form";
import Icon from "../components/Icon";
import Modal from "react-bootstrap/Modal";
import OverlayTrigger from "react-bootstrap/OverlayTrigger";
import PlusIcon from "../assets/plus.svg";
import { ClientContext } from "../contexts/ClientContext";
import Tooltip from "react-bootstrap/Tooltip";
import { faQuestionCircle } from '@fortawesome/free-solid-svg-icons'
import { toast } from 'react-toastify';
import Card from "./Card";

var utils = require("../utils")


export default function InstanceCreator(updateInstances) {
  const [show, setShow] = useState(false);
  const [flavours, setFlavours] = useState([]);
  const [name, setName] = useState("");
  const [flavour, setFlavour] = useState("");
  const [uuid, setUUID] = useState("");
  const [versions, setVersions] = useState([]);
  const [version, setVersion] = useState("");
  const [ready, setReady] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const { api_domain, api_path } = useContext(ClientContext);
  const toastId = useRef("");

  const checkError = () => {
    if (waiting) {
      toast.error("Waiting for the previous request to finish...");
      return true;
    }

    if (!ready) {
      toast.error("Please fill out all fields");
      return true;
    }

    return false;
  }

  useEffect(() => {
    if (name.length > 0 && flavour.length > 0 && version.length > 0) {
      setReady(true);
    }
    else {
      setReady(false);
    }
  }, [name, flavour, version]);

  // fetch flavours on showing of modal
  useEffect(() => {
    fetch(`${api_domain}${api_path}/jar/flavours`)
      .then((response) => response.json())
      .then((data) => {
        setFlavours(data)
      })
  }, [show, api_domain, api_path]);

  // fetch versions on selection of flavour
  useEffect(() => {
    if (flavour) {
      setVersions([]);
      setVersion("");
      fetch(`${api_domain}${api_path}/jar/${flavour}/versions`)
        .then((response) => response.json())
        .then((data) => {
          setVersions(data)
        })
    }
  }, [flavour, api_domain, api_path]);

  let createInstance = async (event) => {
    event.preventDefault();

    if (checkError()) return;

    toastId.current = toast.loading("Creating instance...");

    setWaiting(true);

    let jarUrlResponse = await fetch(`${api_domain}${api_path}/jar/${flavour}/${version}`);
    if (!jarUrlResponse.ok) {
      let error = await jarUrlResponse.json();
      error = error.error ? error.error : "Couldn't get jar URL";
      toast.update(toastId.current, { render: error, type: toast.TYPE.WARNING, isLoading: false, autoClose: 5000 });
      toastId.current = "";
      setWaiting(false);
      return;
    }
    let url = (await jarUrlResponse.json()).url;
    if(!url) {
      toast.update(toastId.current, { render: "Couldn't get jar URL", type: toast.TYPE.WARNING, isLoading: false, autoClose: 5000 });
      toastId.current = "";
      setWaiting(false);
      return;
    }
    let payload = JSON.stringify({ name, flavour, version, url });

    console.log(payload);

    let creationResponse = await fetch(`${api_domain}${api_path}/instance/${uuid}`, {
      method: "POST",
      body: payload,
    });
    if (!creationResponse.ok) {
      let error = await creationResponse.text();
      toast.update(toastId.current, { render: error, type: toast.TYPE.WARNING, isLoading: false, autoClose: 5000 });
      toastId.current = "";
      setWaiting(false);
      return;
    }

    toast.update(toastId.current, { render: "Successfully created instance!", type: toast.TYPE.SUCCESS, isLoading: false, autoClose: 5000, progress: 0});
    toastId.current = "";
    setWaiting(false);
    setShow(false);
    updateInstances();
  };

  utils.useInterval(() => {
    if (toastId.current) {
      fetch(`${api_domain}${api_path}/instance/${uuid}/download-progress`)
        .then((response) => {
          if (response.ok) {
            return response;
          }
        }).then((response) => {
          if (response) {
            return response.text();
          }
        }).then((progress) => {
          if (progress) {
            //progress is in "1000/1000" format
            let progressArray = progress.split("/");
            //careful if progress[1] is 0
            if(progressArray[1] === "0") {
              toast.update(toastId.current, {type:toast.TYPE.INFO, progress: 0 });
            }else{
              let progressPercentage = parseInt(progressArray[0]) / parseInt(progressArray[1]);
              toast.update(toastId.current, {type:toast.TYPE.INFO, progress: progressPercentage });
            }
          }
        });
    }
  }, 250);

  return (
    <Card className="instance instance-creator-card" tilt={true} >
      <div className="title-bar">
        <h2 className="title">Create new Instance</h2>
      </div>
      <span className="new-instance-button-wrapper">
        <img src={PlusIcon} alt="Plus Icon" className="new-instance-button clickable" onClick={() => {
          if (waiting) {
            toast.error("Waiting for the previous request to finish...");
            return;
          }
          setShow(true);
          setName("");
          setVersion("");
          setFlavour("");
          setVersions([]);
          setReady(false);
        }} />
      </span>

      
      <Modal show={show} onHide={() => { setShow(false); }}
        size="md"
        centered
        contentClassName="card main"
      >
        <div className="title-bar">
          <h2 className="title">Create new Instance</h2>
          <CloseButton onClick={() => setShow(false)} />
        </div>
        <Form onSubmit={createInstance} disabled>
          <Form.Group controlId="creationForm.Name" className="mb-3">
            <Form.Label>Instance Name</Form.Label>
            <Form.Control autoComplete="off" type="text" placeholder="My Instance" disabled={waiting}
              value={name} onChange={(event) => {
                setName(event.target.value)
                setUUID(`${event.target.value.replace(/[^0-9a-zA-Z]+/g, '')}-${Date.now().toString(16)}-${Math.floor(Math.random() * 1024)}`)
                // checkForm();
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
                  disabled={waiting}
                  onChange={(event) => {
                    setFlavour(event.target.value);
                    // checkForm();
                  }}
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
              <Form.Select disabled={waiting} value={version} onChange={(event) => {
                setVersion(event.target.value);
                // checkForm();
              }} >
                <option value="" disabled>Choose a version</option>
                {versions.map((myVersion) => (
                  <option key={myVersion} value={myVersion}>{myVersion}</option>
                ))}
              </Form.Select>
            </Form.Group>
          </div>
          <div className="d-grid create-button-wrapper">
            <Button variant="primary" type="submit" size="lg" disabled={!ready || waiting}>
              Create!
            </Button>
          </div>
        </Form>
      </Modal>
    </Card>
  );
}