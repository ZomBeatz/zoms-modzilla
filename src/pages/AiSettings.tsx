// Settings › AI help: optional, any service. Claude by its own API; everything else by the
// OpenAI-style API most services (and local ones like Ollama) speak.

import { useEffect, useState } from "react";
import { api } from "../api";

const SERVICES: { id: string; label: string; provider: "anthropic" | "openai"; base: string; model: string; needsKey: boolean; hint: string }[] = [
  { id: "anthropic", label: "Claude (Anthropic)", provider: "anthropic", base: "", model: "claude-opus-5-5", needsKey: true, hint: "A key from platform.claude.com. Opus 5.5 is the most capable; Sonnet 5.5 and Haiku 5.5 cost less." },
  { id: "openai", label: "OpenAI", provider: "openai", base: "https://api.openai.com/v1", model: "", needsKey: true, hint: "A key from platform.openai.com, and the model name you want to use." },
  { id: "gemini", label: "Google Gemini", provider: "openai", base: "https://generativelanguage.googleapis.com/v1beta/openai", model: "", needsKey: true, hint: "A key from Google AI Studio, and a Gemini model name." },
  { id: "openrouter", label: "OpenRouter (many AIs)", provider: "openai", base: "https://openrouter.ai/api/v1", model: "", needsKey: true, hint: "One key for many services; the model is named like vendor/model." },
  { id: "groq", label: "Groq", provider: "openai", base: "https://api.groq.com/openai/v1", model: "", needsKey: true, hint: "A key from console.groq.com." },
  { id: "mistral", label: "Mistral", provider: "openai", base: "https://api.mistral.ai/v1", model: "", needsKey: true, hint: "A key from console.mistral.ai." },
  { id: "ollama", label: "Ollama (on this computer)", provider: "openai", base: "http://localhost:11434/v1", model: "", needsKey: false, hint: "Free and private: nothing leaves this computer. Use a model you've pulled (ollama list)." },
  { id: "lmstudio", label: "LM Studio (on this computer)", provider: "openai", base: "http://localhost:1234/v1", model: "", needsKey: false, hint: "Free and private. Start LM Studio's server and load a model." },
  { id: "other", label: "Another OpenAI-style service", provider: "openai", base: "", model: "", needsKey: false, hint: "Its address (ending in /v1, usually), the model name, and a key if it needs one." },
];
const CLAUDE_MODELS = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-2"];

export function AiSettings({ toast }: { toast: (t: string) => void }) {
  const [status, setStatus] = useState<Awaited<ReturnType<typeof api.aiStatus>> | null>(null);
  const [svc, setSvc] = useState("anthropic");
  const [base, setBase] = useState("");
  const [model, setModel] = useState("claude-opus-5-5");
  const [key, setKey] = useState("");
  useEffect(() => {
    api.aiStatus().then((s) => {
      setStatus(s);
      if (s.settings) {
        const match = SERVICES.find((x) => x.provider === s.settings!.provider && (x.provider === "anthropic" || x.base === s.settings!.baseUrl)) ?? SERVICES.find((x) => x.id === "other")!;
        setSvc(match.id);
        setBase(s.settings.baseUrl ?? "");
        setModel(s.settings.model);
      }
    }, () => {});
  }, []);
  const s = SERVICES.find((x) => x.id === svc)!;
  const on = !!status?.settings;
  const save = async () => {
    try {
      const st = await api.aiSet({ provider: s.provider, model: model.trim(), baseUrl: s.provider === "anthropic" ? "" : base.trim() }, key ? key : null);
      setStatus(st);
      setKey("");
      toast("AI help is set up. Find Ask AI on a profile.");
    } catch (e) {
      toast((e as Error).message);
    }
  };
  return (
    <div className="panel">
      <h3>AI help {on ? <span className="pill accent">on</span> : <span className="pill">off</span>}</h3>
      <p className="lead">
        Optional. Ask an AI about a profile: why a game won't start, what a crash log means, which mod to blame. Zom's Modzilla gathers what it knows (the check, the game's logs, the last runs), takes out your home folder and names, shows it to you, and sends it only when you press Send. Bring any service: each has its own prices and privacy rules; one on this computer keeps everything here.
      </p>
      <label className="setting">
        <span className="what">
          <b>Service</b>
          <span>{s.hint}</span>
        </span>
        <select
          value={svc}
          onChange={(e) => {
            const n = SERVICES.find((x) => x.id === e.target.value)!;
            setSvc(n.id);
            setBase(n.base);
            setModel(n.model);
          }}
          aria-label="AI service"
        >
          {SERVICES.map((x) => (
            <option key={x.id} value={x.id}>
              {x.label}
            </option>
          ))}
        </select>
      </label>
      {s.provider !== "anthropic" && (
        <label className="setting">
          <span className="what">
            <b>Address</b>
            <span>Has to start with https://, or be on this computer.</span>
          </span>
          <input value={base} onChange={(e) => setBase(e.target.value)} placeholder="https://…/v1" aria-label="AI service address" />
        </label>
      )}
      <label className="setting">
        <span className="what">
          <b>Model</b>
        </span>
        <input list={s.provider === "anthropic" ? "claude-models" : undefined} value={model} onChange={(e) => setModel(e.target.value)} placeholder="model name" aria-label="AI model" />
        <datalist id="claude-models">
          {CLAUDE_MODELS.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
      </label>
      <label className="setting">
        <span className="what">
          <b>Key</b>
          <span>{status?.hasKey ? "A key is saved (kept in a file only you can read). Type a new one to replace it." : s.needsKey ? "Needed for this service." : "Only if the service asks for one."}</span>
        </span>
        <input type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder={status?.hasKey ? "••••••••" : "paste the key"} aria-label="AI key" />
      </label>
      <div className="row gap-sm">
        <button className="btn primary" disabled={!model.trim() || (s.provider !== "anthropic" && !base.trim()) || (s.needsKey && !key && !status?.hasKey)} onClick={() => void save()}>
          {on ? "Save" : "Turn on"}
        </button>
        {on && (
          <button
            className="btn"
            onClick={async () => {
              setStatus(await api.aiSet(null, null));
              toast("AI help is off, and its key is forgotten.");
            }}
          >
            Turn off
          </button>
        )}
      </div>
    </div>
  );
}
