<p align="center">
  <img src="https://edomata.ir/icon.png" height="100px" alt="Edomata icon" />
  <br/>
  <strong>Edomata</strong>
  <i>(Event-driven automata for Scala, Scala.js and scala native)</i>
</p>

<a href="https://typelevel.org/cats/"><img src="https://typelevel.org/cats/img/cats-badge.svg" height="40px" align="right" alt="Cats friendly" /></a>

[![edomata-core Scala version support](https://index.scala-lang.org/hnaderi/edomata/edomata-core/latest.svg?style=flat-square)](https://index.scala-lang.org/hnaderi/edomata/edomata-core)
[![javadoc](https://javadoc.io/badge2/dev.hnaderi/edomata-docs_3/scaladoc.svg?style=flat-square)](https://javadoc.io/doc/dev.hnaderi/edomata-docs_3) 
<img alt="GitHub Workflow Status" src="https://img.shields.io/github/actions/workflow/status/hnaderi/edomata/ci.yml?style=flat-square">
<img alt="GitHub" src="https://img.shields.io/github/license/hnaderi/edomata?style=flat-square">  
![Typelevel Affiliate Project](https://img.shields.io/badge/typelevel-affiliate%20project-FFB4B5.svg?style=flat-square)
[![Scala Steward badge](https://img.shields.io/badge/Scala_Steward-helping-blue.svg?style=flat-square&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA4AAAAQCAMAAAARSr4IAAAAVFBMVEUAAACHjojlOy5NWlrKzcYRKjGFjIbp293YycuLa3pYY2LSqql4f3pCUFTgSjNodYRmcXUsPD/NTTbjRS+2jomhgnzNc223cGvZS0HaSD0XLjbaSjElhIr+AAAAAXRSTlMAQObYZgAAAHlJREFUCNdNyosOwyAIhWHAQS1Vt7a77/3fcxxdmv0xwmckutAR1nkm4ggbyEcg/wWmlGLDAA3oL50xi6fk5ffZ3E2E3QfZDCcCN2YtbEWZt+Drc6u6rlqv7Uk0LdKqqr5rk2UCRXOk0vmQKGfc94nOJyQjouF9H/wCc9gECEYfONoAAAAASUVORK5CYII=)](https://scala-steward.org)

## Getting started
visit [Project site](https://edomata.ir/) to see tutorials and docs.
Also please drop a ⭐ if this project interests you. I need encouragement.

## Add to your build

### From Maven Central (upstream)
Use latest version from badge above
```scala
libraryDependencies += "dev.hnaderi" %% "edomata-core" % "<last version from badge>"
```
or other modules
```scala
libraryDependencies += "dev.hnaderi" %% "edomata-skunk-circe" % "<last version from badge>"
```
See [modules](https://edomata.ir/other/modules.html) for more info.

or for scala.js and or scala native
```scala
libraryDependencies += "dev.hnaderi" %%% "edomata-core" % "<last version from badge>"
```

### From GitHub Packages (BSG fork)

This fork publishes artifacts under `dev.bsg` to GitHub Packages. A new version is published automatically on every merge to `main`.

**1. Add the resolver** in your `build.sbt`:
```scala
resolvers += "GitHub Packages - edomata" at
  "https://maven.pkg.github.com/beyond-scale-group/edomata"
```

**2. Configure credentials** using a [GitHub Personal Access Token](https://github.com/settings/tokens) with `read:packages` scope. Choose one method:

*Option A* - Environment variable:
```scala
credentials += Credentials(
  "GitHub Package Registry",
  "maven.pkg.github.com",
  "_",
  sys.env("GITHUB_TOKEN")
)
```

*Option B* - SBT credentials file (`~/.sbt/.credentials`):
```
realm=GitHub Package Registry
host=maven.pkg.github.com
user=_
password=<your GitHub PAT>
```
```scala
credentials += Credentials(Path.userHome / ".sbt" / ".credentials")
```

**3. Add the dependency**:
```scala
// JVM
libraryDependencies += "dev.bsg" %% "edomata-skunk-circe" % "0.12.0"

// Scala.js / Scala Native
libraryDependencies += "dev.bsg" %%% "edomata-core" % "0.12.0"
```

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

**4. For GitHub Actions CI**, add `GITHUB_TOKEN` to your workflow:
```yaml
- name: Build
  run: sbt compile
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Projects

- [Edomata example from tutorials](https://github.com/hnaderi/edomata-example)
- Feel free to add your projects in a PR!

## Articles and blog posts

- [Event driven fractals at DZone](https://dzone.com/articles/event-driven-fractals)

## Adopters

Here's a (non-exhaustive) list of companies that use edomata in production. Don't see yours? You can add it in a PR!

- [eveince capital](https://eveince.com/) uses edomata in its trading platform and order management systems.
- [expert-flow.ai](https://expert-flow.ai/) uses edomata in its SaaS platform automating forensic expertise workflows for French legal professionals.
