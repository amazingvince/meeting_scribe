/**
 * Meeting search input with debounce
 */

import { useState, useEffect, useCallback } from 'react';
import { Search, X } from 'lucide-react';
import { Input } from '../ui/Input';

interface MeetingSearchProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

export function MeetingSearch({
  value,
  onChange,
  placeholder = 'Search meetings...',
}: MeetingSearchProps) {
  const [localValue, setLocalValue] = useState(value);

  // Debounce the onChange callback
  useEffect(() => {
    const timer = setTimeout(() => {
      if (localValue !== value) {
        onChange(localValue);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [localValue, value, onChange]);

  // Sync external value changes
  useEffect(() => {
    setLocalValue(value);
  }, [value]);

  const handleClear = useCallback(() => {
    setLocalValue('');
    onChange('');
  }, [onChange]);

  return (
    <Input
      value={localValue}
      onChange={(e) => setLocalValue(e.target.value)}
      placeholder={placeholder}
      className="h-11 rounded-xl shadow-none"
      leftIcon={<Search className="w-4 h-4" />}
      rightIcon={
        localValue ? (
          <button
            onClick={handleClear}
            className="rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground"
            aria-label="Clear search"
          >
            <X className="w-4 h-4" />
          </button>
        ) : undefined
      }
    />
  );
}
