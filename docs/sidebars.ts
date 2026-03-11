import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  mainSidebar: [
    "index",
    {
      type: "category",
      label: "Getting Started",
      items: ["install", "cli"],
    },
    {
      type: "category",
      label: "Language",
      items: ["language", "objects", "booleans", "materials", "lights"],
    },
    {
      type: "category",
      label: "Renderer",
      items: ["renderer", "settings", "limitations"],
    },
  ],
};

export default sidebars;
