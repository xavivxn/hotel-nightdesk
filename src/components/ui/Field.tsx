import { cn } from "@/lib/utils";
import type {
  FormEvent,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";

const field =
  "w-full rounded-lg border border-[var(--line)] bg-[var(--surface)] px-3.5 h-11 text-[var(--ink)] outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent-soft)] placeholder:text-[var(--muted)]";

/** Browser constraint tooltips follow OS locale; force Spanish copy for our UI. */
function spanishValidityMessage(el: {
  validity: ValidityState;
  validationMessage: string;
}): string {
  const v = el.validity;
  if (v.valueMissing) return "Completá este campo.";
  if (v.typeMismatch) return "El formato no es válido.";
  if (v.tooShort) return "El valor es demasiado corto.";
  if (v.tooLong) return "El valor es demasiado largo.";
  if (v.patternMismatch) return "El formato no es válido.";
  if (v.rangeUnderflow || v.rangeOverflow) return "El valor está fuera de rango.";
  if (v.stepMismatch) return "El valor no es válido.";
  return el.validationMessage || "Revisá este campo.";
}

function onInvalidSpanish(e: FormEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) {
  const el = e.currentTarget;
  el.setCustomValidity(spanishValidityMessage(el));
}

function clearCustomValidity(e: FormEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) {
  e.currentTarget.setCustomValidity("");
}

export function Label({ children }: { children: string }) {
  return (
    <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">
      {children}
    </label>
  );
}

export function Input({ className, onInvalid, onInput, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(field, className)}
      {...props}
      onInvalid={(e) => {
        onInvalidSpanish(e);
        onInvalid?.(e);
      }}
      onInput={(e) => {
        clearCustomValidity(e);
        onInput?.(e);
      }}
    />
  );
}

export function PasswordInput({
  show,
  onToggle,
  className,
  disabled,
  onInvalid,
  onInput,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { show: boolean; onToggle: () => void }) {
  return (
    <div className="relative">
      <input
        className={cn(field, "pr-11", className)}
        {...props}
        disabled={disabled}
        type={show ? "text" : "password"}
        onInvalid={(e) => {
          onInvalidSpanish(e);
          onInvalid?.(e);
        }}
        onInput={(e) => {
          clearCustomValidity(e);
          onInput?.(e);
        }}
      />
      <button
        type="button"
        disabled={disabled}
        aria-label={show ? "Ocultar contraseña" : "Mostrar contraseña"}
        title={show ? "Ocultar contraseña" : "Mostrar contraseña"}
        onClick={onToggle}
        className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1.5 text-[var(--muted)] hover:text-[var(--ink)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-soft)] disabled:pointer-events-none disabled:opacity-50"
      >
        <svg aria-hidden="true" viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth="1.8">
          {show ? (
            <>
              <path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" />
              <circle cx="12" cy="12" r="2.5" />
            </>
          ) : (
            <>
              <path d="m3 3 18 18" />
              <path d="M10.6 6.2A10.6 10.6 0 0 1 12 6c6 0 9.5 6 9.5 6a17.5 17.5 0 0 1-3.2 3.7M6.1 6.1C3.8 7.6 2.5 12 2.5 12s3.5 6 9.5 6c1.4 0 2.7-.3 3.8-.8" />
            </>
          )}
        </svg>
      </button>
    </div>
  );
}

export function Select({ className, onInvalid, onInput, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={cn(field, className)}
      {...props}
      onInvalid={(e) => {
        onInvalidSpanish(e);
        onInvalid?.(e);
      }}
      onInput={(e) => {
        clearCustomValidity(e);
        onInput?.(e);
      }}
    />
  );
}

export function Textarea({ className, onInvalid, onInput, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={cn(field, "h-24 py-3 resize-none", className)}
      {...props}
      onInvalid={(e) => {
        onInvalidSpanish(e);
        onInvalid?.(e);
      }}
      onInput={(e) => {
        clearCustomValidity(e);
        onInput?.(e);
      }}
    />
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <Label>{label}</Label>
      {children}
    </div>
  );
}
