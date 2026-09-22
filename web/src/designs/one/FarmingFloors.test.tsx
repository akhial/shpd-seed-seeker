// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it } from "vite-plus/test";
import { defaultQueryState, validateQuery } from "../../lib/query";
import { queryStore } from "../../lib/store";
import { QueryPanel } from "./QueryPanel";
import { FarmingFloors } from "./FarmingFloors";
import { renderToStaticMarkup } from "react-dom/server";

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
afterEach(() => queryStore.setState(defaultQueryState));
it("offers three keyboard-accessible floor toggles and help on hover, focus, or tap", async () => {
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  let query = defaultQueryState();
  const render = () =>
    root.render(
      <FarmingFloors
        query={query}
        onChange={(next) => {
          query = next;
          render();
        }}
      />,
    );
  await act(async () => {
    render();
  });
  const buttons = [...container.querySelectorAll<HTMLButtonElement>(".d1-farming-floor")];
  expect(buttons.map((button) => button.textContent)).toEqual(["Floor 7", "Floor 17", "Floor 22"]);
  expect(container.querySelector("legend > span")?.textContent).toBe(
    "Ring of Wealth farming floors",
  );
  const help = container.querySelector<HTMLButtonElement>(".d1-farming-help-button")!;
  const tooltip = container.querySelector<HTMLElement>('[role="tooltip"]')!;
  expect(help.getAttribute("aria-describedby")).toBe(tooltip.id);
  expect(tooltip.textContent).toBe("Dark floor with a garden.");
  expect(tooltip.hidden).toBe(true);
  await act(async () => {
    help.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
  });
  expect(tooltip.hidden).toBe(false);
  await act(async () => {
    help.dispatchEvent(new MouseEvent("mouseout", { bubbles: true }));
  });
  expect(tooltip.hidden).toBe(true);
  await act(async () => {
    help.click();
  });
  expect(tooltip.hidden).toBe(false);
  expect(query.floorRequirements ?? []).toEqual([]);
  await act(async () => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  });
  expect(tooltip.hidden).toBe(true);
  await act(async () => {
    help.focus();
  });
  expect(tooltip.hidden).toBe(false);
  await act(async () => {
    buttons[1].focus();
  });
  expect(tooltip.hidden).toBe(true);
  await act(async () => {
    buttons[1].click();
  });
  expect(buttons[1].getAttribute("aria-pressed")).toBe("true");
  expect(query.floorRequirements).toEqual([
    { depth: 17, feeling: "dark", any_rooms: ["garden", "secret_garden"] },
  ]);
  await act(async () => {
    buttons[1].click();
  });
  expect(query.floorRequirements).toEqual([]);
  await act(async () => {
    root.unmount();
  });
  container.remove();
});
it("places farming floors in a collapsed Rooms and feelings section below Blacksmith", () => {
  const query = defaultQueryState();
  const html = renderToStaticMarkup(
    <QueryPanel
      analysis={undefined}
      validation={validateQuery(query)}
      running={false}
      engineReady
      onToggleSearch={() => {}}
      isMac={false}
      shareNotice={undefined}
      onDismissShareNotice={() => {}}
    />,
  );
  const container = document.createElement("div");
  container.innerHTML = html;
  const farming = container.querySelector(".d1-farming-floors")!;
  const section = farming.closest("section")!;
  expect(section.getAttribute("aria-label")).toBe("Rooms and feelings");
  expect(farming.closest("details")?.querySelector("summary")?.textContent).toBe(
    "Rooms and feelings",
  );
  expect(farming.closest("details")?.open).toBe(false);
  expect(section.previousElementSibling?.querySelector("summary")?.textContent).toBe("Blacksmith");
  expect(container.querySelector('[aria-label="Requirements"] .d1-farming-floors')).toBeNull();
});
