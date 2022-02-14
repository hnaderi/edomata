import laika.helium.Helium
import laika.helium.config._
import laika.theme._
import laika.ast._
import laika.ast.Path.Root
import laika.config.{ConfigBuilder, LaikaKeys}
import laika.sbt.LaikaConfig

object SiteConfigs {
  val landing = Helium.defaults.site
    .landingPage(
      logo = Some(
        Image.internal(
          Root / "icon.png",
          width = Some(Length(50, LengthUnit.percent))
        )
      ),
      title = Some("Edomata"),
      subtitle = Some("Event-driven automatons for Scala"),
      latestReleases = Seq(
        ReleaseInfo("Latest Stable Release", "2.3.5"),
        ReleaseInfo("Latest Milestone Release", "2.4.0-M2")
      ),
      license = Some("Apache 2"),
      documentationLinks = Seq(
        TextLink.internal(Root / "introduction.md", "Inroduction"),
        TextLink.internal(Root / "tutorials" / "0_getting_started.md", "Tutorials"),
        TextLink.internal(Root / "principles" / "index.md", "Principles"),
        TextLink.internal(Root / "api"/ "index.html", "API docs")
      ),
      projectLinks = Seq(
        TextLink.external("http://somewhere.com/", "Demo")
      ),
      teasers = Seq(
        Teaser(
          "Purely functional",
          "Fully referentially transparent, no exceptions or runtime reflection and integration with cats-effect for polymorphic effect handling."
        ),
        Teaser(
          "Modular",
          "You can decide on your ecosystem, usage, libraries, patterns ..."
        ),
        Teaser(
          "Lightweight",
          "Provides simple tools that encourage good design for complex event-driven systems"
        ),
        Teaser(
          "DDD/Eventsourcing/CQRS",
          "enables modeling true eventsourced systems to ensure you can reason about your system"
        ),
        Teaser(
          "Principled",
          "Based on simple and battle tested old ideas on how to design and build distributed systems"
        ),
        Teaser(
          "Simple",
          "You can easily understand what's going on and don't need a phd in Akka clusterology!"
        )
      )
    )
    .site
    .topNavigationBar(
      homeLink = IconLink.internal(Root / "introduction.md", HeliumIcon.home),
      navLinks = Seq(
        IconLink.internal(Root / "api" / "index.html", HeliumIcon.api),
        TextLink.external("http://somewhere.com/", "Text Link")
      )
    )

}
