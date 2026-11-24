import { useEffect, useState } from "react";
import { Smartphone } from "lucide-react";
import { api } from "./api";
import { Dialog } from "./ui";

/** A QR code as squares. */
function Qr({ width, cells }: { width: number; cells: boolean[] }) {
  const q = 4; // quiet zone
  const n = width + q * 2;
  return (
    <svg className="qr" viewBox={`0 0 ${n} ${n}`} role="img" aria-label="QR code" shapeRendering="crispEdges">
      <rect width={n} height={n} fill="#fff" />
      {cells.map((on, i) => (on ? <rect key={i} x={(i % width) + q} y={Math.floor(i / width) + q} width={1} height={1} fill="#000" /> : null))}
    </svg>
  );
}

/** Send from your phone: a page on this computer, opened by scanning a QR code, for pasting the
 * Mod manager download link instead of typing it with a controller. Never the API key: the page
 * is plain http, fine for links that run out, not for a key that doesn't. Open only
 * while this dialog is. */
export function PhoneSend({ onClose }: { onClose: () => void }) {
  const [page, setPage] = useState<{ url: string; width: number; cells: boolean[] } | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    void api.remoteStart().then(setPage, (e) => setError((e as Error).message));
    return () => void api.remoteStop().catch(() => {});
  }, []);
  return (
    <Dialog title="Send from your phone" onClose={onClose} foot={<button className="btn" onClick={onClose}>Done</button>}>
      <p className="muted small">
        Scan this with your phone's camera, on the same Wi-Fi. On the page that opens, paste a <b>Mod manager download</b> link (long-press the button on nexusmods.com and copy the link) to download it here, or a mod's or collection's address. Free accounts still click each file on the website, as Nexus Mods asks.
      </p>
      {error && <p className="banner warn small">{error}</p>}
      {page && (
        <div className="phone-send">
          <Qr width={page.width} cells={page.cells} />
          <code className="small">{page.url.replace(/\?t=.*/, "")}</code>
        </div>
      )}
      <p className="muted small">The page only works while this is open (15 minutes at most), and only with the code in the QR code. Use it on your home network: it isn't encrypted.</p>
    </Dialog>
  );
}

export function PhoneSendButton({ label = "Send from your phone", className = "btn sm" }: { label?: string; className?: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className={className} onClick={() => setOpen(true)} title="Paste a download link on your phone instead of typing it here">
        <Smartphone size={14} /> {label}
      </button>
      {open && <PhoneSend onClose={() => setOpen(false)} />}
    </>
  );
}
