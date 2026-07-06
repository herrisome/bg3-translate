import * as React from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export interface NumberInputProps
  extends Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    "type" | "value" | "defaultValue" | "onChange"
  > {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onValueChange: (value: number) => void;
}

const NumberInput = React.forwardRef<HTMLInputElement, NumberInputProps>(
  (
    {
      className,
      value,
      min = Number.NEGATIVE_INFINITY,
      max = Number.POSITIVE_INFINITY,
      step = 1,
      onValueChange,
      disabled,
      ...props
    },
    ref,
  ) => {
    const clamp = React.useCallback(
      (next: number) => Math.min(max, Math.max(min, next)),
      [max, min],
    );

    const commitValue = React.useCallback(
      (next: number) => {
        if (!Number.isFinite(next)) return;
        onValueChange(clamp(next));
      },
      [clamp, onValueChange],
    );

    const stepBy = (direction: 1 | -1) => {
      commitValue(value + step * direction);
    };

    return (
      <div className={cn("relative", className)}>
        <Input
          ref={ref}
          type="text"
          inputMode="numeric"
          value={String(value)}
          disabled={disabled}
          className="pr-12"
          onChange={(event) => {
            const raw = event.target.value.trim();
            if (raw === "") {
              onValueChange(clamp(min === Number.NEGATIVE_INFINITY ? 0 : min));
              return;
            }
            commitValue(Number(raw));
          }}
          {...props}
        />
        <div className="absolute right-1 top-1 flex h-8 w-8 flex-col overflow-hidden rounded-sm border border-input bg-background">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            disabled={disabled || value >= max}
            className="h-4 w-8 rounded-none p-0 text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            onClick={() => stepBy(1)}
            tabIndex={-1}
            aria-label="增加"
          >
            <ChevronUp className="h-3 w-3" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            disabled={disabled || value <= min}
            className="h-4 w-8 rounded-none p-0 text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            onClick={() => stepBy(-1)}
            tabIndex={-1}
            aria-label="减少"
          >
            <ChevronDown className="h-3 w-3" />
          </Button>
        </div>
      </div>
    );
  },
);
NumberInput.displayName = "NumberInput";

export { NumberInput };
