import { forwardRef, type CSSProperties } from "react";
import loveNestLogo from "@/assets/love-nest-logo-borderless.png";
import { LOGO_ASPECT } from "@/lib/login-transition";
import { cn } from "@/lib/utils";

export function LoveNestLogo({ className }: { className?: string }) {
  return (
    <img
      src={loveNestLogo}
      alt="Love Nestt Motel"
      className={cn("brand-logo", className)}
    />
  );
}

/**
 * Layer ids, back to front. Each file in `src/assets/logo-layers/<id>.png` must share
 * the full logo canvas (1005 × 868, transparent) so the layers stack without offsets.
 * Until the layered assets exist, the full logo is used as a single `emblem` layer.
 */
export const LOGO_LAYERS = ["leaves-left", "leaves-right", "emblem", "wordmark", "flourish"] as const;
export type LogoLayer = (typeof LOGO_LAYERS)[number];

const layerFiles = import.meta.glob<string>("../../assets/logo-layers/*.png", {
  eager: true,
  import: "default",
});

function layerSrc(id: LogoLayer): string | undefined {
  const hit = Object.entries(layerFiles).find(([path]) => path.endsWith(`/${id}.png`));
  return hit?.[1];
}

const layers: { id: LogoLayer; src: string }[] = layerSrc("emblem")
  ? LOGO_LAYERS.flatMap((id) => {
      const src = layerSrc(id);
      return src ? [{ id, src }] : [];
    })
  : [{ id: "emblem", src: loveNestLogo }];

/** Stacked logo used by the login stage so each layer can move on its own. */
export const LoveNestLogoLayered = forwardRef<HTMLDivElement, { className?: string }>(
  function LoveNestLogoLayered({ className }, ref) {
    return (
      <div
        ref={ref}
        role="img"
        aria-label="Love Nestt Motel"
        className={cn("brand-logo-layered", className)}
        style={{ "--logo-aspect": LOGO_ASPECT } as CSSProperties}
      >
        {layers.map((layer) => (
          <span key={layer.id} className="login-layer" data-layer={layer.id}>
            <img src={layer.src} alt="" draggable={false} />
          </span>
        ))}
      </div>
    );
  },
);
