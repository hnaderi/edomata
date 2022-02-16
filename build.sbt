import laika.io.config.SiteConfig
import sbt.ThisBuild
import Dependencies._

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.1.0"
ThisBuild / scalaVersion := scala3
ThisBuild / fork := true
ThisBuild / git.useGitDescribe := true
ThisBuild / versionScheme := Some("early-semver")

def module(name: String, deps: Seq[ModuleID] = Nil): Project = {
  val id = s"module-$name"
  Project(id, file(s"modules/$name"))
    .settings(
      Common.settings,
      libraryDependencies ++=
        Libraries.cats ++ Libraries.munit.map(_ % Test) ++ deps,
      moduleName := s"edomata-$name"
    )
    .enablePlugins(GitlabPlugin)
}

def testkit(name: String, deps: Seq[ModuleID] = Nil): Project = {
  val id = s"testkit-$name"
  Project(id, file(s"testkits/$name"))
    .settings(
      Common.settings,
      libraryDependencies ++=
        Libraries.cats ++ Libraries.munit ++ deps,
      moduleName := s"edomata-$name-testkit"
    )
    .enablePlugins(GitlabPlugin)
}

lazy val modules: List[ProjectReference] = List(
  core,
  backend,
  endpoint,
  eventsourcing,
  skunkBackend,
  backendTestkit,
  docs,
  mdocPlantuml
)

lazy val root = (project in file("."))
  .settings(
    publish / skip := true
  )
  .enablePlugins(GitVersioning)
  .enablePlugins(GitBranchPrompt)
  .aggregate(modules: _*)

lazy val mdocPlantuml = (project in file("mdoc-plantuml"))
  .settings(
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2022.1"
  )
  .enablePlugins(MdocPlugin)

import laika.parse.code.SyntaxHighlighting
import laika.markdown.github.GitHubFlavor

lazy val docs = (project in file("docs-build"))
  .settings(
    Common.settings,
    publish / skip := true,
    laikaIncludeAPI := true,
    laikaGenerateAPI / mappings := (ScalaUnidoc / packageDoc / mappings).value,
    Laika / sourceDirectories := Seq(mdocOut.value),
    mdocVariables := Map(
      "VERSION" -> version.value
    ),
    laikaExtensions ++= Seq(GitHubFlavor, SyntaxHighlighting),
    laikaTheme := SiteConfigs.landing.build
  )
  .enablePlugins(LaikaPlugin)
  .enablePlugins(ScalaUnidocPlugin)
  .enablePlugins(MdocPlugin)
  .dependsOn(
    core,
    backend,
    eventsourcing,
    mdocPlantuml
  )

import Libraries._

lazy val core = module("core").settings(
  libraryDependencies ++= cats
)

lazy val eventsourcing = module("eventsourcing", fs2 ++ odin)
  .dependsOn(core)

lazy val backend = module("backend")
  .dependsOn(core, eventsourcing)

lazy val endpoint = module("endpoint", http4s)
  .dependsOn(backend)

lazy val skunkBackend = module("skunk", skunk ++ odin)
  .dependsOn(backend)

lazy val backendTestkit = testkit("backend").dependsOn(backend)

addCommandAlias(
  "site",
  List("docs/clean", "docs/mdoc", "laikaSite").mkString(" ;")
)
addCommandAlias(
  "commit",
  List("clean", "scalafmtCheckAll", "scalafmtSbtCheck", "compile", "test")
    .mkString(" ;")
)
