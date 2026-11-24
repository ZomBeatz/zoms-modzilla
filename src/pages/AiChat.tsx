// Ask an AI about a profile: the app gathers what it knows (the check, the game's logs, the last
// runs) with personal details taken out, shows it, and sends it only when you press Send.
// Optional: it's set up in Settings › AI help, with any service.

import { useEffect, useRef, useState } from "react";
import { Bot, Send } from "lucide-react";
import { api, on, type Profile } from "../api";
import { actionLabel, splitActions, type AiAction } from "../aiactions";
import { Dialog } from "../ui";

type Msg = { role: "user" | "assistant"; content: string };

const STARTERS = ["Why won't the game start?", "Why did the game crash last time?", "Read the logs and tell me what's wrong.", "Which of my mods are likely out of date?", "Is my load order OK?"];

export function AiChat({ profile, onClose, onSetup, onAction }: { profile: Profile; onClose: () => void; onSetup: () => void; onAction: (a: AiAction) => Promise<string> }) {
  const [ready, setReady] = useState<boolean | null>(null);
  const [context, setContext] = useState("");
  const [showCtx, setShowCtx] = useState(false);
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [live, setLive] = useState("");
  const [done, setDone] = useState<Record<string, string>>({});
  const asking = useRef("");
  const end = useRef<HTMLDivElement>(null);
  // The answer as it's written.
  useEffect(() => {
    const off = on<{ id: string; text: string }>("ai:text", (p) => {
      if (p.id === asking.current) setLive((t) => t + p.text);
    });
    return () => void off.then((f) => f());
  }, []);
  useEffect(() => {
    api.aiStatus().then((s) => setReady(!!s.settings && (s.hasKey || s.settings.provider !== "anthropic")), () => setReady(false));
    api.aiContext(profile.id, null).then(setContext, (e) => setError((e as Error).message));
  }, [profile.id]);
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end", behavior: "smooth" });
  }, [msgs, busy, live]);
  const send = async (text: string) => {
    const t = text.trim();
    if (!t || busy) return;
    const next: Msg[] = [...msgs, { role: "user", content: t }];
    setMsgs(next);
    setQ("");
    setBusy(true);
    setError("");
    setLive("");
    asking.current = `${Date.now()}`;
    try {
      const reply = await api.aiAsk(context, next, asking.current);
      setMsgs([...next, { role: "assistant", content: reply }]);
    } catch (e) {
      setError((e as Error).message);
      setMsgs(msgs);
      setQ(t);
    } finally {
      asking.current = "";
      setLive("");
      setBusy(false);
    }
  };
  return (
    <Dialog wide title={`Ask AI about “${profile.name}”`} onClose={onClose}>
      {ready === false ? (
        <div className="empty">
          <Bot size={28} />
          <p>AI help is optional and off. Set it up with any AI service (Claude, OpenAI, Gemini, or one on this computer like Ollama) in Settings › AI help.</p>
          <button className="btn primary" onClick={onSetup}>
            Set up AI help
          </button>
        </div>
      ) : (
        <div className="ai-chat">
          <details className="ai-ctx" open={showCtx} onToggle={(e) => setShowCtx((e.target as HTMLDetailsElement).open)}>
            <summary className="small">
              <b>What the AI will see</b> <span className="muted">· {context ? `${context.length < 1000 ? `${context.length} characters` : `${Math.round(context.length / 1000)}k characters`}: the game, these mods, the check, the logs, the last runs. Your home folder and names are taken out; keys never go.` : "gathering…"}</span>
            </summary>
            <textarea className="msettings-text" value={context} onChange={(e) => setContext(e.target.value)} spellCheck={false} aria-label="What the AI will see" />
            <p className="muted small">You can change or cut anything here before sending.</p>
          </details>
          <div className="ai-log" aria-live="polite">
            {msgs.length === 0 && (
              <div className="ai-starters">
                {STARTERS.map((s) => (
                  <button key={s} className="btn sm" disabled={!context || busy} onClick={() => void send(s)}>
                    {s}
                  </button>
                ))}
              </div>
            )}
            {msgs.map((m, i) => {
              const { text, actions } = m.role === "assistant" ? splitActions(m.content) : { text: m.content, actions: [] };
              return (
                <div key={i} className={`ai-msg ${m.role}`}>
                  {text}
                  {actions.length > 0 && (
                    <div className="ai-actions">
                      {actions.map((a, j) => {
                        const k = `${i}:${j}`;
                        return done[k] ? (
                          <span key={k} className="ok-text small">{done[k]}</span>
                        ) : (
                          <button
                            key={k}
                            className="btn sm"
                            onClick={async () => {
                              try {
                                const said = await onAction(a);
                                setDone((d) => ({ ...d, [k]: said }));
                              } catch (e) {
                                setError((e as Error).message);
                              }
                            }}
                          >
                            {actionLabel(a)}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
            {busy && <div className={`ai-msg assistant${live ? "" : " muted"}`}>{live ? splitActions(live).text : "Reading your setup…"}</div>}
            <div ref={end} />
          </div>
          {error && <p className="err-text small">{error}</p>}
          <form
            className="ai-ask"
            onSubmit={(e) => {
              e.preventDefault();
              void send(q);
            }}
          >
            <textarea
              value={q}
              rows={2}
              placeholder="Ask about this profile, a crash, a mod…"
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void send(q);
                }
              }}
              aria-label="Your question"
            />
            <button className="btn primary" disabled={!q.trim() || busy || !context}>
              <Send size={15} /> Send
            </button>
          </form>
          <p className="muted small">AI answers can be wrong. Its suggestions are buttons you choose to press, and each one undoes cleanly (Switch off has Undo; Find the problem mod puts everything back).</p>
        </div>
      )}
    </Dialog>
  );
}
