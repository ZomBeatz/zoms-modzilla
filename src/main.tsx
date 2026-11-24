import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { api, logPageErrors } from "./api";
import "./tokens.css";
import "./styles.css";

logPageErrors();

// Zom's Modzilla was Zom's Nexus: what the page remembered under the old name carries over.
try {
  for (const k of Object.keys(localStorage)) {
    if (!k.startsWith("zoms-nexus:")) continue;
    const to = "zoms-modzilla:" + k.slice("zoms-nexus:".length);
    if (localStorage.getItem(to) == null) localStorage.setItem(to, localStorage.getItem(k)!);
    localStorage.removeItem(k);
  }
} catch {
  /* no storage */
}

async function start() {
  const first = await api.overview();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App first={first} />
    </StrictMode>,
  );
}

void start();
