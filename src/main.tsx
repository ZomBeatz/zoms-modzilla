import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { api } from "./api";
import "./tokens.css";
import "./styles.css";

async function start() {
  const first = await api.overview();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App first={first} />
    </StrictMode>,
  );
}

void start();
