import laika.io.config.SiteConfig
import sbt.ThisBuild
import Dependencies._

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.1.1"
ThisBuild / scalaVersion := scala3
ThisBuild / fork := true
ThisBuild / git.useGitDescribe := true
ThisBuild / versionScheme := Some("early-semver")
ThisBuild / publishTo := sonatypePublishToBundle.value
ThisBuild / sonatypeCredentialHost := "s01.oss.sonatype.org"
ThisBuild / licenses := Seq(
  "APL2" -> url("http://www.apache.org/licenses/LICENSE-2.0.txt")
)
ThisBuild / developers := List(
  Developer(
    id = "hnaderi",
    name = "Hossein Naderi",
    email = "hossein-naderi@hotmail.com",
    url = url("https://hnaderi.ir")
  )
)

publishMavenStyle := true

import xerial.sbt.Sonatype._
sonatypeProjectHosting := Some(
  GitHubHosting("hnaderi", "edomata", "hossein-naderi@hotmail.com")
)

def module(name: String, deps: Seq[ModuleID] = Nil): Project = {
  val id = s"module-$name"
  Project(id, file(s"modules/$name"))
    .settings(
      Common.settings,
      libraryDependencies ++=
        Libraries.munit.map(_ % Test) ++ deps,
      moduleName := s"edomata-$name"
    )
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
}

lazy val modules: List[ProjectReference] = List(
  core,
  sqlBackend,
  skunkBackend,
  doobieBackend,
  docs,
  examples,
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
    laikaTheme := SiteConfigs(version.value).build
  )
  .enablePlugins(LaikaPlugin)
  .enablePlugins(ScalaUnidocPlugin)
  .enablePlugins(MdocPlugin)
  .dependsOn(
    core,
    mdocPlantuml
  )

import Libraries._

lazy val core = module("core").settings(
  libraryDependencies ++= cats ++ catsLaws
)

lazy val sqlBackend = module("sql-backend")
  .dependsOn(core)
  .settings(libraryDependencies ++= catsEffect ++ fs2)

lazy val skunkBackend = module("skunk").dependsOn(sqlBackend)

lazy val doobieBackend = module("doobie").dependsOn(sqlBackend)

lazy val examples = project.dependsOn(skunkBackend, doobieBackend)

def addAlias(name: String)(tasks: String*) =
  addCommandAlias(name, tasks.mkString(" ;"))

addAlias("site")(
  "docs/clean",
  "docs/mdoc",
  "laikaSite"
)
addAlias("commit")(
  "clean",
  "scalafmtCheckAll",
  "scalafmtSbtCheck",
  "compile",
  "test"
)
addAlias("precommit")(
  "scalafmtAll",
  "scalafmtSbt",
  "compile",
  "test"
)
