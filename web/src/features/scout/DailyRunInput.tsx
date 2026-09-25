import { useEffect, useRef, useState } from "react";
import { CalendarIcon } from "../../shared/ui/icons";
import { formatSeedCode, parseSeedCodeSync } from "../../engine/wasm";

export const todayUTC = () => new Date().toISOString().slice(0, 10);

export function DailyRunInput({
  input,
  onInput,
  onScout,
  loading,
}: {
  input: string;
  onInput: (value: string) => void;
  onScout: (value: string) => void;
  loading: boolean;
}) {
  const picker = useRef<HTMLInputElement>(null);
  const [fallbackPicker, setFallbackPicker] = useState(false);
  useEffect(() => {
    if (fallbackPicker) picker.current?.focus();
  }, [fallbackPicker]);
  let ready = false;
  try {
    ready = parseSeedCodeSync(input).code === input;
  } catch {
    /* Partial input stays editable. */
  }
  const daily = /^\d/.test(input);
  return (
    <div className="d1-run-input">
      <form
        className="d1-scout-input-row"
        onSubmit={(event) => {
          event.preventDefault();
          if (ready && !loading) onScout(input);
        }}
      >
        <input
          className="d1-seed-field d1-mono"
          value={input}
          placeholder="Seed / YYYY-MM-DD"
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          aria-label="Seed code or daily date"
          title="Seed code or daily date (YYYY-MM-DD, UTC)"
          inputMode={daily ? "numeric" : "text"}
          disabled={loading}
          onChange={(event) => onInput(formatSeedCode(event.currentTarget.value))}
        />
        <div className="d1-run-shortcuts">
          <button
            type="button"
            className="d1-btn d1-date-button"
            disabled={loading}
            aria-label="Choose daily run date"
            title="Choose date"
            onClick={() => {
              try {
                if (!picker.current?.showPicker) throw new Error("Native picker unavailable");
                picker.current.showPicker();
              } catch {
                setFallbackPicker((shown) => !shown);
              }
            }}
          >
            <CalendarIcon size={18} />
          </button>
          <input
            ref={picker}
            type="date"
            className={fallbackPicker ? "d1-seed-field d1-date-fallback" : "d1-native-date"}
            value={ready && daily ? input : ""}
            min="1970-01-01"
            max="9999-12-31"
            aria-label="Daily run date (UTC)"
            aria-hidden={!fallbackPicker}
            tabIndex={fallbackPicker ? 0 : -1}
            disabled={loading}
            onKeyDown={(event) => {
              if (event.key === "Escape") setFallbackPicker(false);
            }}
            onChange={(event) => {
              if (event.currentTarget.value && event.currentTarget.validity.valid) {
                onInput(event.currentTarget.value);
                setFallbackPicker(false);
              }
            }}
          />
          <button
            type="button"
            className="d1-btn"
            disabled={loading}
            onClick={() => {
              const date = todayUTC();
              setFallbackPicker(false);
              onInput(date);
              onScout(date);
            }}
          >
            Today
          </button>
        </div>
        <button type="submit" className="d1-btn d1-btn-primary" disabled={!ready || loading}>
          {loading ? "Scouting…" : "Scout"}
        </button>
      </form>
    </div>
  );
}
