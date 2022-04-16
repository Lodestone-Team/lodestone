import React, { useEffect, useState } from "react";
import { useAuthState } from "react-firebase-hooks/auth";
import { Link } from "react-router-dom";
import Card from "./Card";
import { auth, sendPasswordReset } from "../firebase";
import "./Reset.css";
function Reset() {
  const [email, setEmail] = useState("");
  const [user, loading] = useAuthState(auth);
  useEffect(() => {
    if (loading) return;
    // if (user) navigate("/dashboard");
  }, [user, loading]);
  return (
    <Card className="reset__container">
      <input
        type="text"
        className="reset__textBox"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="E-mail Address"
      />
      <button
        className="reset__btn"
        onClick={() => sendPasswordReset(email)}
      >
        Send password reset email
      </button>
      <div>
        Don't have an account? <Link to="/register">Register</Link> now.
      </div>
    </Card>
  );
}
export default Reset;