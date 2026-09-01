import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

export function BootSplash({
  leaving = false,
  holdSeconds,
}: {
  leaving?: boolean;
  holdSeconds: number;
}) {
  return (
    <div
      className={cn("boot-splash", leaving && "is-leaving")}
      style={{ "--boot-hold": `${holdSeconds}s` } as CSSProperties}
      aria-live="polite"
    >
      <div className="boot-splash-inner">
        <p className="boot-wordmark">NIGHTDESK</p>
        <p className="boot-kicker">Abriendo recepción</p>
        <div className="boot-cells" aria-hidden="true">
          <span className="boot-cell">
            <span className="boot-cell-core" />
          </span>
          <span className="boot-cell">
            <span className="boot-cell-core" />
          </span>
          <span className="boot-cell">
            <span className="boot-cell-core" />
          </span>
          <span className="boot-cell">
            <span className="boot-cell-core" />
          </span>
        </div>
        <div className="boot-rail" aria-hidden="true">
          <span className="boot-rail-fill" />
        </div>
        <p className="boot-meta">SQLite · PIN · tablero</p>
      </div>
    </div>
  );
}
