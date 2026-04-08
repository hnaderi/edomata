<p align="center">
  <img src="https://edomata.ir/icon.png" height="100px" alt="Edomata icon" />
  <br/>
  <strong>Edomata</strong>
  <i>(Event-driven automata for Scala, Scala.js and scala native)</i>
</p>

<a href="https://typelevel.org/cats/"><img src="https://typelevel.org/cats/img/cats-badge.svg" height="40px" align="right" alt="Cats friendly" /></a>

<img alt="GitHub Workflow Status" src="https://img.shields.io/github/actions/workflow/status/beyond-scale-group/edomata/ci.yml?style=flat-square">
<img alt="GitHub" src="https://img.shields.io/github/license/beyond-scale-group/edomata?style=flat-square">  
<img alt="Typelevel Affiliate Project" src="https://img.shields.io/badge/typelevel-affiliate%20project-FFB4B5.svg?style=flat-square">
<a href="https://scala-steward.org"><img alt="Scala Steward badge" src="https://img.shields.io/badge/Scala_Steward-helping-blue.svg?style=flat-square&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA4AAAAQCAMAAAARSr4IAAAAVFBMVEUAAACHjojlOy5NWlrKzcYRKjGFjIbp293YycuLa3pYY2LSqql4f3pCUFTgSjNodYRmcXUsPD/NTTbjRS+2jomhgnzNc223cGvZS0HaSD0XLjbaSjElhIr+AAAAAXRSTlMAQObYZgAAAHlJREFUCNdNyosOwyAIhWHAQS1Vt7a77/3fcxxdmv0xwmckutAR1nkm4ggbyEcg/wWmlGLDAA3oL50xi6fk5ffZ3E2E3QfZDCcCN2YtbEWZt+Drc6u6rlqv7Uk0LdKqqr5rk2UCRXOk0vmQKGfc94nOJyQjouF9H/wCc9gECEYfONoAAAAASUVORK5CYII=" /></a>

> This is the [Beyond Scale Group](https://github.com/beyond-scale-group) fork of [hnaderi/edomata](https://github.com/hnaderi/edomata). Artifacts are published under `dev.bsg` to GitHub Packages.

## Quick Start

Add to your `build.sbt`:

```scala
// Core library
libraryDependencies += "dev.bsg" %% "edomata-core" % "0.12.5"

// With Skunk + Circe PostgreSQL backend (recommended)
libraryDependencies += "dev.bsg" %% "edomata-skunk-circe" % "0.12.5"
```

For **Scala.js** or **Scala Native**, use `%%%`:
```scala
libraryDependencies += "dev.bsg" %%% "edomata-core" % "0.12.5"
```

### GitHub Packages resolver

This fork publishes under `dev.bsg` to GitHub Packages. Add the resolver:
```scala
resolvers += "GitHub Packages - edomata" at
  "https://maven.pkg.github.com/beyond-scale-group/edomata"
```

> **Authentication required**: GitHub Packages Maven registry requires a token even for public repositories.
> Add credentials to your build:
> ```scala
> credentials += Credentials(
>   "GitHub Package Registry",
>   "maven.pkg.github.com",
>   "_",
>   sys.env("GITHUB_TOKEN") // needs read:packages scope
> )
> ```

## Documentation

| Topic | Link |
|-------|------|
| Introduction | [docs/introduction.md](docs/introduction.md) |
| Getting started | [docs/tutorials/getting-started.md](docs/tutorials/getting-started.md) |
| Event sourcing | [docs/tutorials/eventsourcing.md](docs/tutorials/eventsourcing.md) |
| CQRS | [docs/tutorials/cqrs.md](docs/tutorials/cqrs.md) |
| Backends | [docs/tutorials/backends.md](docs/tutorials/backends.md) |
| Processes | [docs/tutorials/processes.md](docs/tutorials/processes.md) |
| SaaS / Multi-tenancy | [docs/tutorials/saas.md](docs/tutorials/saas.md) |
| Migrations | [docs/tutorials/migrations.md](docs/tutorials/migrations.md) |
| Skunk backend | [docs/backends/skunk.md](docs/backends/skunk.md) |
| Doobie backend | [docs/backends/doobie.md](docs/backends/doobie.md) |
| Design goals | [docs/about/design-goals.md](docs/about/design-goals.md) |
| Features | [docs/about/features.md](docs/about/features.md) |
| Definitions | [docs/principles/definitions.md](docs/principles/definitions.md) |
| Modules list | [docs/other/modules.md](docs/other/modules.md) |
| FAQ | [docs/other/faq.md](docs/other/faq.md) |

Visit the [documentation site](https://beyond-scale-group.github.io/edomata/) for the full guide.

**Available modules:**

| Module | Artifact | Platforms | Description |
|--------|----------|-----------|-------------|
| core | `"dev.bsg" %% "edomata-core"` | JVM, JS, Native | Core abstractions (Decision, Edomaton, Stomaton) |
| backend | `"dev.bsg" %% "edomata-backend"` | JVM, JS, Native | Event sourcing backend abstractions |
| postgres | `"dev.bsg" %% "edomata-postgres"` | JVM, JS, Native | PostgreSQL common components |
| skunk | `"dev.bsg" %% "edomata-skunk"` | JVM, JS, Native | Skunk-based PostgreSQL backend |
| skunk-circe | `"dev.bsg" %% "edomata-skunk-circe"` | JVM, JS, Native | Circe JSON codecs for Skunk |
| skunk-jsoniter | `"dev.bsg" %% "edomata-skunk-jsoniter"` | JVM, JS, Native | Jsoniter codecs for Skunk |
| skunk-upickle | `"dev.bsg" %% "edomata-skunk-upickle"` | JVM, JS, Native | uPickle codecs for Skunk |
| doobie | `"dev.bsg" %% "edomata-doobie"` | JVM only | Doobie-based PostgreSQL backend |
| doobie-circe | `"dev.bsg" %% "edomata-doobie-circe"` | JVM only | Circe JSON codecs for Doobie |
| doobie-jsoniter | `"dev.bsg" %% "edomata-doobie-jsoniter"` | JVM only | Jsoniter codecs for Doobie |
| doobie-upickle | `"dev.bsg" %% "edomata-doobie-upickle"` | JVM only | uPickle codecs for Doobie |
| saas | `"dev.bsg" %% "edomata-saas"` | JVM, JS, Native | Multi-tenant SaaS abstractions |
| saas-skunk | `"dev.bsg" %% "edomata-saas-skunk"` | JVM, JS, Native | Skunk-based SaaS backend |
| munit | `"dev.bsg" %% "edomata-munit"` | JVM, JS, Native | MUnit test framework integration |

> For Scala.js or Scala Native, use `%%%` instead of `%%`.


## Projects

- [Edomata example from tutorials](https://github.com/hnaderi/edomata-example) (upstream)
- [expert-flow.ai](https://expert-flow.ai/) - SaaS platform for forensic expertise workflows

## Articles and blog posts

- [Event driven fractals at DZone](https://dzone.com/articles/event-driven-fractals)

## Adopters

Here's a (non-exhaustive) list of companies that use edomata in production. Don't see yours? You can add it in a PR!

- [eveince capital](https://eveince.com/) uses edomata in its trading platform and order management systems.
- [expert-flow.ai](https://expert-flow.ai/) uses edomata in its SaaS platform automating forensic expertise workflows for French legal professionals.
