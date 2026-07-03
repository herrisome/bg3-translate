import * as React from "react";
import { Check, Minus } from "lucide-react";
import { cn } from "@/lib/utils";

type CheckState = "unchecked" | "checked" | "indeterminate";

interface CheckboxProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onChange"> {
  state?: CheckState;
  onToggle?: () => void;
}

const Checkbox = React.forwardRef<HTMLButtonElement, CheckboxProps>(
  ({ className, state = "unchecked", onToggle, disabled, ...props }, ref) => (
    <button
      ref={ref}
      type="button"
      role="checkbox"
      aria-checked={state === "checked"}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onToggle?.();
      }}
      className={cn(
        "flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors",
        state === "checked" && "border-primary bg-primary text-primary-foreground",
        state === "indeterminate" &&
          "border-primary bg-primary text-primary-foreground",
        state === "unchecked" && "border-input bg-background",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        disabled && "cursor-not-allowed opacity-50",
        className,
      )}
      {...props}
    >
      {state === "checked" && <Check className="h-3 w-3" strokeWidth={3} />}
      {state === "indeterminate" && (
        <Minus className="h-3 w-3" strokeWidth={3} />
      )}
    </button>
  ),
);
Checkbox.displayName = "Checkbox";

export { Checkbox };
export type { CheckState };
