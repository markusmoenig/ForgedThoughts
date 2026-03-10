import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  title: "ForgedThoughts",
  tagline: "Programmable SDF rendering with FT",

  future: {
    v4: true,
  },

  url: "https://forgedthoughts.com",
  baseUrl: "/",

  organizationName: "markusmoenig",
  projectName: "ForgedThoughts",

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "warn",

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      {
        docs: {
          routeBasePath: "docs",
          sidebarPath: "./sidebars.ts",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    navbar: {
      title: "ForgedThoughts",
      items: [
        {
          type: "docSidebar",
          sidebarId: "mainSidebar",
          position: "left",
          label: "Docs",
        },
        {
          href: "https://github.com/markusmoenig/ForgedThoughts",
          label: "GitHub",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Docs",
          items: [
            {
              label: "Overview",
              to: "/docs",
            },
            {
              label: "Materials",
              to: "/docs/materials",
            },
          ],
        },
        {
          title: "Project",
          items: [
            {
              label: "GitHub",
              href: "https://github.com/markusmoenig/ForgedThoughts",
            },
          ],
        },
      ],
      copyright: `Copyright ${new Date().getFullYear()} ForgedThoughts`,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
