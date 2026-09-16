import loveNestLogo from "@/assets/love-nest-logo-borderless.png";
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
