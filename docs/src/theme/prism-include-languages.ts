import siteConfig from "@generated/docusaurus.config";
import type * as PrismNamespace from "prismjs";
import type { Optional } from "utility-types";

function registerForgeLanguage(PrismObject: typeof PrismNamespace): void {
  const clike = PrismObject.languages.clike;

  PrismObject.languages.forge = PrismObject.languages.extend("clike", {
    comment: [
      {
        pattern: /(^|[^\\])\/\/.*/,
        lookbehind: true,
        greedy: true,
      },
      {
        pattern: /(^|[^\\])\/\*[\s\S]*?\*\//,
        lookbehind: true,
        greedy: true,
      },
    ],
    string: {
      pattern: /"([^"\\]|\\.)*"/,
      greedy: true,
    },
    "hex-color": {
      pattern: /#[0-9a-fA-F]{3,8}\b/,
      alias: "number",
    },
    keyword:
      /\b(?:let|var|fn|material|sdf|environment|import|export|return)\b/,
    builtin:
      /\b(?:Sphere|Box|Cylinder|Torus|ExtrudePolygon|Lambert|Metal|Dielectric|Camera|PointLight|SphereLight|EnvLight|RenderSettings|Bvh|Bricks|Naive|Top|Bottom|Left|Right|Front|Back|Center)\b/,
    boolean: /\b(?:true|false)\b/,
    function: /\b[a-zA-Z_]\w*(?=\s*\()/,
    number:
      /\b\d+(?:\.\d+)?(?:deg)?\b|\B\.\d+(?:deg)?\b/,
    operator: /[-+*/%=&|<>!]+|\./,
    punctuation: clike.punctuation,
  });
}

export default function prismIncludeLanguages(
  PrismObject: typeof PrismNamespace,
): void {
  const {
    themeConfig: { prism },
  } = siteConfig;
  const { additionalLanguages } = prism as { additionalLanguages: string[] };

  const prismBefore = globalThis.Prism;
  globalThis.Prism = PrismObject;

  additionalLanguages.forEach((lang) => {
    if (lang === "php") {
      require("prismjs/components/prism-markup-templating.js");
    }
    require(`prismjs/components/prism-${lang}`);
  });

  registerForgeLanguage(PrismObject);

  delete (globalThis as Optional<typeof globalThis, "Prism">).Prism;
  if (typeof prismBefore !== "undefined") {
    globalThis.Prism = PrismObject;
  }
}
