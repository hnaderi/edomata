import Dependencies._
import laika.io.config.SiteConfig
import sbt.ThisBuild
import sbtcrossproject.CrossProject

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.1.1"
inThisBuild(
  List(
    tlBaseVersion := "0.0",
    scalaVersion := scala3,
    fork := true,
    Test / fork := false,
    organization := "io.github.hnaderi",
    organizationName := "Hossein Naderi",
    startYear := Some(2021),
    tlSonatypeUseLegacyHost := false,
    tlCiReleaseBranches := Seq("main"),
    tlSitePublishBranch := Some("main"),
    licenses := Seq(License.Apache2),
    developers := List(
      Developer(
        id = "hnaderi",
        name = "Hossein Naderi",
        email = "hossein-naderi@hotmail.com",
        url = url("https://hnaderi.ir")
      )
    )
  )
)

def module(mname: String): CrossProject => CrossProject =
  _.in(file(s"modules/$mname"))
    .settings(
      name := s"module-$mname",
      scalacOptions ++= Seq("-Xfatal-warnings"),
      libraryDependencies ++= Seq(
        "org.scalameta" %%% "munit" % Versions.MUnit % Test,
        "org.scalameta" %%% "munit-scalacheck" % Versions.MUnit % Test,
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit % Test,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion % Test
      ),
      moduleName := s"edomata-$mname"
    )

lazy val modules = List(
  core,
  sqlBackend,
  skunkBackend,
  skunkCirceCodecs,
  skunkUpickeCodec,
  doobieBackend,
  docs,
  examples,
  mdocPlantuml
)

lazy val root = tlCrossRootProject
  .enablePlugins(GitBranchPrompt)
  .aggregate(modules: _*)

lazy val mdocPlantuml = (project in file("mdoc-plantuml"))
  .settings(
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2022.1"
  )
  .enablePlugins(MdocPlugin)
  .enablePlugins(NoPublishPlugin)

import laika.parse.code.SyntaxHighlighting
import laika.markdown.github.GitHubFlavor

lazy val docs = (project in file("docs-build"))
  .settings(
    publish / skip := true,
    laikaIncludeAPI := true,
    // laikaGenerateAPI / mappings := (ScalaUnidoc / packageDoc / mappings).value,
    Laika / sourceDirectories := Seq(mdocOut.value),
    laikaExtensions ++= Seq(GitHubFlavor, SyntaxHighlighting),
    laikaTheme := SiteConfigs(version.value).build
  )
  .enablePlugins(TypelevelSitePlugin)
  .enablePlugins(TypelevelUnidocPlugin)
  .dependsOn(
    core.jvm,
    mdocPlantuml
  )

lazy val core = module("core") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .settings(
      description := "Purely functional event-driven automata",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "cats-core" % Versions.cats,
        "org.typelevel" %%% "cats-laws" % Versions.cats % Test,
        "org.typelevel" %%% "discipline-munit" % "1.0.9" % Test
      )
    )
    .jsSettings(
      libraryDependencies ++= Seq(
        "io.github.cquiroz" %%% "scala-java-time" % "2.3.0"
      )
    )
}

lazy val sqlBackend = module("sql-backend") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core)
    .settings(
      description := "Performant eventsourcing backend for edomata",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "cats-effect" % Versions.catsEffect,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect % Test,
        "co.fs2" %%% "fs2-core" % Versions.fs2
      )
    )
}

lazy val skunkBackend = module("skunk") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(sqlBackend)
    .settings(
      description := "Skunk based backend for edomata",
      libraryDependencies ++= Seq(
        "org.tpolecat" %%% "skunk-core" % Versions.skunk
      )
    )
}

lazy val doobieBackend = module("doobie") {
  crossProject(JVMPlatform)
    .dependsOn(sqlBackend)
    .enablePlugins(NoPublishPlugin)
    .settings(
      description := "Doobie based backend for edomata",
      libraryDependencies ++= Seq(
        "org.tpolecat" %% "doobie-core" % Versions.doobie,
        "org.tpolecat" %% "doobie-postgres" % Versions.doobie
      )
    )
}

lazy val skunkCirceCodecs = module("skunk-circe") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(skunkBackend)
    .settings(
      description := "Circe codecs for skunk backend",
      libraryDependencies ++= Seq(
        "org.tpolecat" %%% "skunk-circe" % Versions.skunk
      )
    )
}

lazy val skunkUpickeCodec = module("skunk-upickle") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(skunkBackend)
    .settings(
      description := "uPickle codecs for skunk backend",
      libraryDependencies ++= Seq(
        "com.lihaoyi" %%% "upickle" % Versions.upickle
      )
    )
}

lazy val examples =
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(
      skunkBackend,
      skunkCirceCodecs,
      skunkUpickeCodec
    )
    .enablePlugins(NoPublishPlugin)

def addAlias(name: String)(tasks: String*) =
  addCommandAlias(name, tasks.mkString(" ;"))

addAlias("site")(
  "docs/tlSite"
)
addAlias("commit")(
  "clean",
  "scalafmtCheckAll",
  "scalafmtSbtCheck",
  "headerCheckAll",
  "githubWorkflowCheck",
  "compile",
  "test"
)
addAlias("precommit")(
  "scalafmtAll",
  "scalafmtSbt",
  "headerCreateAll",
  "githubWorkflowGenerate",
  "compile",
  "test"
)
