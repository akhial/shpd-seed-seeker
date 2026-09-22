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
it("offers three keyboard-accessible floor toggles without hint text", async () => {
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
  const buttons = [...container.querySelectorAll("button")];
  expect(buttons.map((button) => button.textContent)).toEqual(["Floor 7", "Floor 17", "Floor 22"]);
  expect(container.querySelector("legend")?.textContent).toBe("RoW farming floors");
  expect(container.querySelector("p, [role='tooltip']")).toBeNull();
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
it("places farming floors inside Requirements, before blanket requirements and scope", () => {
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
  expect(farming.closest("section")?.querySelector("h3")?.textContent).toBe("Requirements");
  expect(html.indexOf("RoW farming floors")).toBeLessThan(html.indexOf("Blanket Requirements"));
  expect(html.indexOf("RoW farming floors")).toBeLessThan(html.indexOf("Search scope"));
});
