import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { api, logPageErrors } from "./api";
import "./tokens.css";
import "./styles.css";

logPageErrors();

async function start() {
  const first = await api.overview();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App first={first} />
    </StrictMode>,
  );
}

void start();
