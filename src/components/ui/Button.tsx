import { cn } from "@/lib/utils";
import type { ButtonHTMLAttributes } from "react";

const variants = {
  primary:
    "bg-[var(--accent)] text-[#1c1408] hover:brightness-110 shadow-[0_8px_20px_-12px_var(--accent)]",
  secondary: "bg-[var(--surface-2)] text-[var(--ink)] border border-[var(--line)] hover:bg-[var(--bg-2)]",
  ghost: "bg-transparent text-[var(--ink)] hover:bg-[var(--surface-2)]",
  danger: "bg-[var(--danger-soft)] text-[var(--danger)] hover:brightness-110",
  ok: "bg-[var(--ok)] text-[#07140f] hover:brightness-110",
};

const sizes = {
  sm: "h-9 px-3 text-sm",
  md: "h-11 px-4 text-sm",
  lg: "h-12 px-5",
};

export function Button({
  className,
  variant = "primary",
  size = "md",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
}) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-xl font-semibold transition disabled:opacity-50 disabled:pointer-events-none",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
