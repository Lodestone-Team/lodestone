import React, { useContext, useEffect, useState } from "react";

import Button from "react-bootstrap/Button";
import Card from "./Card";
import CloseButton from "react-bootstrap/CloseButton";
import Modal from "react-bootstrap/Modal";
import PlusIcon from "../assets/plus.svg";

export default function InstanceCreator() {
  const [show, setShow] = useState(false);

  return (
    <>
      <img src={PlusIcon} alt="Plus Icon" className="new-instance-button clickable" onClick={() => setShow(true)} />
      <Modal show={show} onHide={() => setShow(false)}
        size="md"
        centered
        contentClassName="card main"
      >
        <div className="title-bar">
          <h2 className="title">Create new Instance</h2>
          <CloseButton onClick={() => setShow(false)}/>
        </div>
      </Modal>
    </>
  );
}