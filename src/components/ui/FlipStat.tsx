import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

/** Vertical number swap: outgoing rises out, incoming rises in (~180ms). */
export function FlipStat({
  value,
  className,
}: {
  value: number;
  className?: string;
}) {
  const prev = useRef(value);
  const [current, setCurrent] = useState(value);
  const [outgoing, setOutgoing] = useState<number | null>(null);

  useEffect(() => {
    if (value === prev.current) return;
    setOutgoing(prev.current);
    prev.current = value;
    setCurrent(value);
    const id = window.setTimeout(() => setOutgoing(null), 190);
    return () => window.clearTimeout(id);
  }, [value]);

  return (
    <strong className={cn("stat-value", className)} aria-label={String(current)}>
      <span className="stat-value-stack">
        {outgoing != null ? (
          <span className="stat-value-out" aria-hidden>
            {outgoing}
          </span>
        ) : null}
        <span key={current} className={cn("stat-value-in", outgoing != null && "stat-value-in--swap")}>
          {current}
        </span>
      </span>
    </strong>
  );
}
