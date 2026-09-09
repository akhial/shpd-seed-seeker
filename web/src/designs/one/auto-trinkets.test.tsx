import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson } from "../../lib/query";
import { autoTrinketsStore, queryStore, setAutoTrinkets } from "../../lib/store";
import { QueryPanel } from "./QueryPanel";

const render = (running = false) =>
  renderToStaticMarkup(
    <QueryPanel
      validation={{ valid: true, errors: [] }}
      analysis={undefined}
      running={running}
      engineReady
      onToggleSearch={() => {}}
      isMac={false}
      shareNotice={undefined}
      onDismissShareNotice={() => {}}
    />,
  );

afterEach(() => {
  vi.unstubAllGlobals();
  queryStore.setState(() => defaultQueryState());
  autoTrinketsStore.setState(() => false);
});

describe("auto-apply performance setting", () => {
  it("remains available on a single-core device and persists the toggle", () => {
    vi.stubGlobal("navigator", { hardwareConcurrency: 1 });
    const setItem = vi.fn();
    vi.stubGlobal("localStorage", { getItem: () => null, setItem });
    queryStore.setState(() => defaultQueryState());
    setAutoTrinkets(true);
    const html = render();
    expect(html).toContain("Performance");
    expect(html).toContain("Auto-apply trinkets");
    expect(html).not.toContain("Number of search threads");
    expect(html).toContain('type="checkbox" checked=""');
    expect(setItem).toHaveBeenCalledWith("seedseeker.autoTrinkets.v1", "true");
  });

  it("disables auto-apply for any trinket requirement, including an OR alternative", () => {
    queryStore.setState(() =>
      fromQueryJson('{"requirements":[{"any_of":[{"item":"mimic_tooth"},{"item":"rat_skull"}]}]}'),
    );
    setAutoTrinkets(true);
    const html = render();
    expect(html).toContain("Auto-apply is inactive when requirements include a trinket.");
    expect(html).toMatch(/<input[^>]*disabled=""[^>]*checked=""[^>]*\/>/);
    expect(autoTrinketsStore.state).toBe(true);
  });
});
