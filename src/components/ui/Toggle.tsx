/**
 * Toggle switch component
 */

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}

export function Toggle({ checked, onChange, label, description, disabled }: ToggleProps) {
  return (
    <div className="flex items-start justify-between gap-3">
      <div>
        <p className="text-sm font-medium text-foreground">{label}</p>
        {description && (
          <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`
          relative inline-flex h-6 w-10 shrink-0 cursor-pointer rounded-full transition-colors
          disabled:cursor-not-allowed disabled:opacity-50
          ${checked ? 'bg-primary' : 'bg-muted'}
        `}
      >
        <span
          className={`
            pointer-events-none block h-5 w-5 rounded-full bg-card shadow transform transition-transform
            ${checked ? 'translate-x-4' : 'translate-x-0.5'}
            mt-0.5
          `}
        />
      </button>
    </div>
  );
}
