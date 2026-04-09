import React from "react";
import clsx from "clsx";
import Link from "@docusaurus/Link";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import Layout from "@theme/Layout";
import CodeBlock from "@theme/CodeBlock";

const features = [
  {
    title: "Purely Functional",
    icon: "\u03bb",
    description:
      "Fully referentially transparent. No exceptions, no runtime reflection. Built on Cats Effect for polymorphic effect handling.",
  },
  {
    title: "Event Sourcing & CQRS",
    icon: "\u29bf",
    description:
      "First-class support for event-driven state machines, enabling true event-sourced systems with built-in CQRS patterns.",
  },
  {
    title: "Cross-Platform",
    icon: "\u2b22",
    description:
      "Run on JVM, Scala.js, or Scala Native. Your domain logic compiles everywhere, from servers to browsers to native binaries.",
  },
  {
    title: "Lightweight & Simple",
    icon: "\u25c7",
    description:
      "A few composable primitives, not a framework. Understand the entire API surface in an afternoon.",
  },
  {
    title: "Type-Safe Decisions",
    icon: "\u2713",
    description:
      "The Decision monad captures accept/reject/indecisive outcomes at the type level. Impossible states are unrepresentable.",
  },
  {
    title: "Production-Ready Backends",
    icon: "\u26c1",
    description:
      "PostgreSQL backends via Skunk (async, cross-platform) or Doobie (JDBC). With migration support and Flyway integration.",
  },
  {
    title: "Extensible",
    icon: "\u2699",
    description:
      "Swap serialization (Circe, Jsoniter, uPickle), backends, or build your own. Every component can be extended.",
  },
  {
    title: "Principled Design",
    icon: "\u25b3",
    description:
      "Based on battle-tested ideas from DDD, CQRS, and event sourcing pioneers. Simple concepts, powerful composition.",
  },
];

const exampleCode = `import edomata.core.*

// Define your domain events
enum AccountEvent:
  case Opened(name: String)
  case Deposited(amount: BigDecimal)
  case Withdrawn(amount: BigDecimal)

// Define your state
case class Account(name: String, balance: BigDecimal)

// Build a pure decision
def withdraw(amount: BigDecimal): Decision[String, AccountEvent, Unit] =
  for
    state <- Decision.read[Account, String]
    _ <- Decision.reject("Insufficient funds")
           .whenA(state.balance < amount)
    _ <- Decision.accept(AccountEvent.Withdrawn(amount))
  yield ()`;

function FeatureCard({ title, icon, description }) {
  return (
    <div className="feature-card">
      <h3>
        <span style={{ marginRight: "0.5rem", fontSize: "1.3rem" }}>{icon}</span>
        {title}
      </h3>
      <p>{description}</p>
    </div>
  );
}

function HeroSection() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <header className="hero">
      <div className="container">
        <h1 className="hero__title">{siteConfig.title}</h1>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div style={{ marginTop: "2rem", display: "flex", gap: "1rem", justifyContent: "center", flexWrap: "wrap" }}>
          <Link className="button button--primary button--lg" to="/docs/tutorials/getting-started">
            Get Started
          </Link>
          <Link className="button button--outline button--lg button--secondary" to="/docs/introduction">
            Learn More
          </Link>
        </div>
        <div className="hero-code">
          <CodeBlock language="scala">{exampleCode}</CodeBlock>
        </div>
      </div>
    </header>
  );
}

function FeaturesSection() {
  return (
    <section className="section">
      <div className="container">
        <h2 className="section-title">Why Edomata?</h2>
        <p className="section-subtitle">
          Simple primitives for complex event-driven systems
        </p>
        <div className="feature-grid">
          {features.map((feature, idx) => (
            <FeatureCard key={idx} {...feature} />
          ))}
        </div>
      </div>
    </section>
  );
}

function QuickStartSection() {
  return (
    <section className="section" style={{ borderTop: "1px solid var(--ifm-color-emphasis-200)" }}>
      <div className="container">
        <h2 className="section-title">Add to Your Project in Seconds</h2>
        <p className="section-subtitle">
          Pick a module, drop it into your build, and start building event-driven systems
        </p>
        <div style={{ maxWidth: 700, margin: "0 auto" }}>
          <h3>SBT (JVM)</h3>
          <CodeBlock language="scala">
            {`// Core library
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-core" % "0.12.5"

// With Skunk + Circe PostgreSQL backend (recommended)
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-skunk-circe" % "0.12.5"`}
          </CodeBlock>
          <h3>Scala.js / Scala Native</h3>
          <CodeBlock language="scala">
            {`libraryDependencies += "io.github.beyond-scale-group" %%% "edomata-core" % "0.12.5"`}
          </CodeBlock>
          <h3>Maven</h3>
          <CodeBlock language="xml">
            {`<dependency>
  <groupId>io.github.beyond-scale-group</groupId>
  <artifactId>edomata-core_3</artifactId>
  <version>0.12.5</version>
</dependency>`}
          </CodeBlock>
          <p style={{ textAlign: "center", marginTop: "1.5rem" }}>
            <Link className="button button--primary button--md" to="/docs/other/modules">
              See All Modules
            </Link>
          </p>
        </div>
      </div>
    </section>
  );
}

function EcosystemSection() {
  return (
    <section className="section" style={{ borderTop: "1px solid var(--ifm-color-emphasis-200)" }}>
      <div className="container">
        <h2 className="section-title">Built on Typelevel</h2>
        <p className="section-subtitle">
          Seamlessly integrates with the libraries you already use
        </p>
        <div style={{ display: "flex", gap: "2rem", justifyContent: "center", flexWrap: "wrap" }}>
          {[
            { name: "Cats", url: "https://typelevel.org/cats/" },
            { name: "Cats Effect", url: "https://typelevel.org/cats-effect/" },
            { name: "FS2", url: "https://fs2.io/" },
            { name: "Skunk", url: "https://tpolecat.github.io/skunk/" },
            { name: "Doobie", url: "https://tpolecat.github.io/doobie/" },
            { name: "Circe", url: "https://circe.github.io/circe/" },
          ].map((lib) => (
            <a
              key={lib.name}
              href={lib.url}
              style={{
                padding: "0.75rem 1.5rem",
                borderRadius: "8px",
                border: "1px solid var(--ifm-color-emphasis-200)",
                color: "var(--ifm-font-color-base)",
                textDecoration: "none",
                fontWeight: 600,
                transition: "border-color 0.2s",
              }}
              onMouseEnter={(e) => (e.target.style.borderColor = "var(--ifm-color-primary)")}
              onMouseLeave={(e) => (e.target.style.borderColor = "var(--ifm-color-emphasis-200)")}
            >
              {lib.name}
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout title={siteConfig.title} description={siteConfig.tagline}>
      <HeroSection />
      <main>
        <FeaturesSection />
        <QuickStartSection />
        <EcosystemSection />
      </main>
    </Layout>
  );
}
