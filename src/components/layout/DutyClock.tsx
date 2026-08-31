import { cn } from "@/lib/utils";
import { useEffect, useRef, useState } from "react";

function pad2(n: number) {
  return n.toString().padStart(2, "0");
}

function isNightShift(date: Date) {
  const hour = date.getHours();
  return hour >= 22 || hour < 6;
}

function formatDateLabel(date: Date) {
  return date
    .toLocaleDateString("es-AR", { weekday: "short", day: "numeric", month: "short" })
    .replaceAll(".", "")
    .replace(",", "")
    .toLowerCase();
}

function FlipDigit({ value }: { value: string }) {
  const shown = useRef(value);
  const [incoming, setIncoming] = useState(value);
  const [outgoing, setOutgoing] = useState<string | null>(null);

  useEffect(() => {
    if (value === shown.current) return;
    const from = shown.current;
    shown.current = value;
    setOutgoing(from);
    setIncoming(value);
    const id = window.setTimeout(() => setOutgoing(null), 180);
    return () => window.clearTimeout(id);
  }, [value]);

  return (
    <span className={cn("flip-digit", outgoing !== null && "is-flipping")}>
      {outgoing !== null ? (
        <span className="flip-digit-out" aria-hidden>
          {outgoing}
        </span>
      ) : null}
      <span className="flip-digit-in">{incoming}</span>
    </span>
  );
}

function FlipPair({ value }: { value: string }) {
  return (
    <span className="inline-flex">
      <FlipDigit value={value[0] ?? "0"} />
      <FlipDigit value={value[1] ?? "0"} />
    </span>
  );
}

function StaticPair({ value }: { value: string }) {
  return (
    <span className="inline-flex">
      <span className="flip-digit">
        <span className="flip-digit-in">{value[0] ?? "0"}</span>
      </span>
      <span className="flip-digit">
        <span className="flip-digit-in">{value[1] ?? "0"}</span>
      </span>
    </span>
  );
}

export function DutyClock() {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 250);
    return () => window.clearInterval(id);
  }, []);

  const hours = pad2(now.getHours());
  const minutes = pad2(now.getMinutes());
  const seconds = pad2(now.getSeconds());
  const night = isNightShift(now);
  const colonDim = now.getMilliseconds() >= 500;
  const railPct = ((now.getSeconds() + now.getMilliseconds() / 1000) / 60) * 100;
  const label = `${hours}:${minutes}:${seconds}`;

  return (
    <time dateTime={now.toISOString()} aria-label={label} className="duty-clock block select-none">
      <p className="mb-1 hidden font-mono text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--muted)] lg:block">
        {formatDateLabel(now)}
      </p>
      <div className="hidden font-mono text-2xl font-semibold tabular-nums leading-none text-[var(--ink)] lg:flex lg:items-center">
        <FlipPair value={hours} />
        <span className={cn("duty-colon mx-0.5", night && "text-[var(--warn)]", colonDim && "is-dim")}>:</span>
        <FlipPair value={minutes} />
        <span className={cn("duty-colon mx-0.5", night && "text-[var(--warn)]", colonDim && "is-dim")}>:</span>
        <StaticPair value={seconds} />
      </div>
      <div className="flex flex-col items-center font-mono text-lg font-semibold tabular-nums leading-none text-[var(--ink)] lg:hidden">
        <FlipPair value={hours} />
        <FlipPair value={minutes} />
      </div>
      <div className="mt-1.5 h-0.5 overflow-hidden rounded-full bg-[var(--line)]">
        <div
          className={cn("h-full", night ? "bg-[var(--warn)]" : "bg-[var(--accent)]")}
          style={{ width: `${railPct}%` }}
        />
      </div>
      {night ? (
        <p className="mt-1.5 hidden font-mono text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--warn)] lg:block">
          Turno noche
        </p>
      ) : null}
    </time>
  );
}
