import laika.ast.Path.Root
import laika.ast._
import laika.config.ConfigBuilder
import laika.config.LaikaKeys
import laika.helium.Helium
import laika.helium.config._
import laika.sbt.LaikaConfig
import laika.theme._

object SiteConfigs {
  def apply(vars: Map[String, String]): Helium = Helium.defaults.site
    .metadata(
      title = Some("Edomata"),
      authors = Seq("Hossein Naderi"),
      language = Some("en")
    )
    .site
    .favIcons(
      Favicon.internal(Root / "icon.png", "32x32")
    )
    .site
    .landingPage(
      logo = Some(
        Image.internal(
          Root / "icon.png",
          width = Some(Length(50, LengthUnit.percent))
        )
      ),
      title = Some("Edomata"),
      subtitle = Some("Event-driven automatons for Scala and Scala.js"),
      latestReleases = Seq(
        ReleaseInfo(
          "Latest develop Release",
          vars.getOrElse("SNAPSHOT_VERSION", "N/A")
        ),
        ReleaseInfo("Latest Stable Release", vars.getOrElse("VERSION", "N/A"))
      ),
      license = Some("Apache 2.0"),
      documentationLinks = Seq(
        TextLink.internal(Root / "introduction.md", "Inroduction"),
        TextLink
          .internal(Root / "tutorials" / "0_getting_started.md", "Tutorials"),
        TextLink.internal(Root / "principles" / "index.md", "Principles")
      ),
      teasers = Seq(
        Teaser(
          "Purely functional",
          "Fully referentially transparent, no exceptions or runtime reflection and integration with cats-effect for polymorphic effect handling."
        ),
        Teaser(
          "Modular/Polymorphic",
          "You can decide on your ecosystem, usage, libraries, patterns ..."
        ),
        Teaser(
          "Lightweight",
          "Provides simple, composable tools that encourage good design for complex event-driven systems"
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
      homeLink = ImageLink
        .internal(Root / "introduction.md", Image.internal(Root / "icon.png")),
      navLinks = Seq(
        // IconLink.internal(Root / "api" / "index.html", HeliumIcon.api)
        IconLink
          .external("https://edomata.ir/", HeliumIcon.github)
      )
    )
    .site
    .baseURL("https://edomata.ir/")

}
