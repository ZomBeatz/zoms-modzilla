// What the controller's buttons do, along the bottom in Deck mode.

export function PadHints() {
  const hint = (b: string, what: string, cls = "") => (
    <span className="pad-hint">
      <kbd className={`pad-btn ${cls}`}>{b}</kbd> {what}
    </span>
  );
  return (
    <div className="pad-hints" aria-hidden="true">
      {hint("A", "Select", "a")}
      {hint("B", "Back", "b")}
      {hint("X", "Menu", "x")}
      {hint("Y", "Play", "y")}
      {hint("LB RB", "Pages")}
      {hint("LT RT", "Scroll")}
      {hint("☰", "Find")}
    </div>
  );
}
