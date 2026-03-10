import React from "react";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";

export default function Home(): React.JSX.Element {
  return (
    <Layout
      title="ForgedThoughts"
      description="Programmable SDF rendering with FT"
    >
      <main
        style={{
          maxWidth: 980,
          margin: "0 auto",
          padding: "5rem 1.5rem",
        }}
      >
        <p
          style={{
            letterSpacing: "0.14em",
            textTransform: "uppercase",
            opacity: 0.7,
          }}
        >
          CPU SDF Rendering
        </p>
        <h1
          style={{
            fontSize: "clamp(3rem, 10vw, 6rem)",
            lineHeight: 0.95,
            marginBottom: "1rem",
          }}
        >
          ForgedThoughts
        </h1>
        <p style={{ fontSize: "1.2rem", maxWidth: 720, marginBottom: "2rem" }}>
          A programmable rendering project currently in development, built
          around signed distance fields, a small scene language called FT, and
          materials that are moving toward self-contained shading code.
        </p>
        <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap" }}>
          <Link
            className="button button--primary button--lg"
            to="/docs/install"
          >
            Get Started
          </Link>
          <Link
            className="button button--secondary button--lg"
            to="/docs/materials"
          >
            Material System
          </Link>
        </div>
      </main>
    </Layout>
  );
}
