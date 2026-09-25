import { useEffect, useState } from "react";
import { formatSeedCode } from "../../lib/wasm";

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
  const [daily, setDaily] = useState(/^\d{4}-\d{2}-\d{2}$/.test(input));
  useEffect(() => {
    if (/^\d{4}-\d{2}-\d{2}$/.test(input)) setDaily(true);
    else if (/[A-Z]/i.test(input)) setDaily(false);
  }, [input]);
  const validDate =
    /^\d{4}-\d{2}-\d{2}$/.test(input) && input >= "1970-01-01" && input <= "9999-12-31";
  const ready = daily ? validDate : input.length === 11;
  return (
    <div className="d1-run-input">
      <div className="d1-run-modes" role="group" aria-label="Run type">
        <button
          type="button"
          className="d1-btn"
          aria-pressed={!daily}
          disabled={loading}
          onClick={() => {
            setDaily(false);
            if (daily) onInput("");
          }}
        >
          Seed code
        </button>
        <button
          type="button"
          className="d1-btn"
          aria-pressed={daily}
          disabled={loading}
          onClick={() => {
            setDaily(true);
            if (!daily) onInput(todayUTC());
          }}
        >
          Daily run
        </button>
      </div>
      <form
        className="d1-scout-input-row"
        onSubmit={(event) => {
          event.preventDefault();
          if (ready && !loading) onScout(input);
        }}
      >
        {daily ? (
          <>
            <input
              type="date"
              className="d1-seed-field"
              value={input}
              min="1970-01-01"
              max="9999-12-31"
              aria-label="Daily run date (UTC)"
              required
              disabled={loading}
              onChange={(event) => onInput(event.currentTarget.value)}
            />
            <button
              type="button"
              className="d1-btn"
              disabled={loading}
              onClick={() => {
                const date = todayUTC();
                onInput(date);
                onScout(date);
              }}
            >
              Today
            </button>
          </>
        ) : (
          <input
            className="d1-seed-field d1-mono"
            value={input}
            placeholder="AAA-AAA-AAA"
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            aria-label="Seed code"
            disabled={loading}
            onChange={(event) => onInput(formatSeedCode(event.currentTarget.value))}
          />
        )}
        <button type="submit" className="d1-btn d1-btn-primary" disabled={!ready || loading}>
          {loading ? "Scouting…" : "Scout"}
        </button>
      </form>
      {daily && (
        <p className="d1-daily-note">
          Choose any date · Daily runs change at midnight UTC.
          <br />
          Uses the supported game version, including past and future dates.
        </p>
      )}
    </div>
  );
}
