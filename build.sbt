import Dependencies._
import laika.io.config.SiteConfig
import laika.rewrite.link.ApiLinks
import laika.rewrite.link.LinkConfig
import sbt.ThisBuild
import sbtcrossproject.CrossProject

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.1.3"
val PrimaryJava = JavaSpec.temurin("8")
val LTSJava = JavaSpec.temurin("17")

inThisBuild(
  List(
    tlBaseVersion := "0.7",
    scalaVersion := scala3,
    fork := true,
    Test / fork := false,
    organization := "dev.hnaderi",
    organizationName := "Hossein Naderi",
    startYear := Some(2021),
    tlSonatypeUseLegacyHost := false,
    tlCiReleaseBranches := Seq("main"),
    tlSitePublishBranch := Some("main"),
    githubWorkflowJavaVersions := Seq(PrimaryJava, LTSJava),
    githubWorkflowBuildPreamble ++= dockerComposeUp,
    licenses := Seq(License.Apache2),
    developers := List(
      Developer(
        id = "hnaderi",
        name = "Hossein Naderi",
        email = "mail@hnaderi.dev",
        url = url("https://hnaderi.dev")
      )
    )
  )
)

lazy val dockerComposeUp = Seq(
  WorkflowStep.Run(
    commands = List("docker-compose up -d"),
    name = Some("Start up Postgres")
  )
)

def module(mname: String): CrossProject => CrossProject =
  _.in(file(s"modules/$mname"))
    .settings(
      name := s"module-$mname",
      libraryDependencies ++= Seq(
        "org.scalameta" %%% "munit" % Versions.MUnit % Test,
        "org.scalameta" %%% "munit-scalacheck" % Versions.MUnit % Test
      ),
      moduleName := s"edomata-$mname"
    )

lazy val modules = List(
  core,
  backend,
  postgres,
  skunkBackend,
  skunkCirceCodecs,
  skunkUpickleCodecs,
  doobieBackend,
  doobieCirceCodecs,
  doobieUpickleCodecs,
  driverTests,
  munitTestkit,
  docs,
  unidocs,
  examples,
  mdocPlantuml
)

lazy val root = tlCrossRootProject
  .settings(
    name := "edomata"
  )
  .aggregate(modules: _*)

lazy val mdocPlantuml = project
  .in(file("mdoc-plantuml"))
  .settings(
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2022.12"
  )
  .enablePlugins(MdocPlugin)
  .enablePlugins(NoPublishPlugin)

lazy val docs = project
  .in(file("site"))
  .enablePlugins(TypelevelSitePlugin)
  .settings(
    tlSiteHeliumConfig := SiteConfigs(mdocVariables.value),
    tlSiteRelatedProjects := Seq(
      TypelevelProject.Cats,
      TypelevelProject.CatsEffect,
      TypelevelProject.Fs2,
      TypelevelProject.Discipline
    ),
    laikaConfig := LaikaConfig.defaults
      .withConfigValue(
        LinkConfig(apiLinks =
          Seq(
            ApiLinks(
              tlSiteApiUrl.value
                .map(_.toString())
                .getOrElse("/edomata/api/"),
              "edomata"
            )
          )
        )
      ),
    laikaIncludeAPI := true
  )
  .dependsOn(
    core.jvm,
    postgres.jvm,
    mdocPlantuml
  )

lazy val unidocs = project
  .in(file("unidocs"))
  .enablePlugins(TypelevelUnidocPlugin)
  .settings(
    name := "edomata-docs",
    description := "unified docs for edomata",
    ScalaUnidoc / unidoc / unidocProjectFilter := inAnyProject -- inProjects(
      mdocPlantuml,
      examples.jvm,
      examples.js,
      driverTests.jvm,
      driverTests.js
    )
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
        "io.github.cquiroz" %%% "scala-java-time" % "2.4.0",
        ("org.scala-js" %%% "scalajs-java-securerandom" % "1.0.0")
          .cross(CrossVersion.for3Use2_13)
      )
    )
}

lazy val backend = module("backend") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core)
    .settings(
      description := "Performant eventsourcing backend for edomata",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "cats-effect" % Versions.catsEffect,
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit % Test,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion % Test,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect % Test,
        "co.fs2" %%% "fs2-core" % Versions.fs2
      )
    )
}

lazy val postgres = module("postgres") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core, backend)
    .settings(
      description := "Postgres common components"
    )
}

lazy val skunkBackend = module("skunk") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(postgres)
    .dependsOn(driverTests % Test)
    .settings(
      description := "Skunk based backend for edomata",
      libraryDependencies ++= Seq(
        "org.tpolecat" %%% "skunk-core" % Versions.skunk,
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit % Test
      )
    )
    .jsSettings(
      Test / scalaJSLinkerConfig ~= (_.withModuleKind(
        ModuleKind.CommonJSModule
      ))
    )
}

lazy val doobieBackend = module("doobie") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(postgres)
    .dependsOn(driverTests % Test)
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

lazy val skunkUpickleCodecs = module("skunk-upickle") {
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

lazy val doobieCirceCodecs = module("doobie-circe") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(doobieBackend)
    .settings(
      description := "Circe codecs for doobie backend",
      libraryDependencies ++= Seq(
        "io.circe" %%% "circe-parser" % Versions.circe
      )
    )
}

lazy val doobieUpickleCodecs = module("doobie-upickle") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(doobieBackend)
    .settings(
      description := "uPickle codecs for doobie backend",
      libraryDependencies ++= Seq(
        "com.lihaoyi" %%% "upickle" % Versions.upickle
      )
    )
}

lazy val driverTests = module("backend-tests") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Full)
    .enablePlugins(NoPublishPlugin)
    .dependsOn(postgres)
    .settings(
      description := "Integration tests for postgres backends",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect
      )
    )
}

lazy val munitTestkit = module("munit") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .settings(
      description := "munit integration for edomata",
      libraryDependencies += "org.scalameta" %%% "munit" % Versions.MUnit
    )
    .dependsOn(core)
}

lazy val examples =
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(
      skunkBackend,
      skunkCirceCodecs,
      skunkUpickleCodecs
    )
    .settings(
      libraryDependencies += "io.circe" %%% "circe-generic" % Versions.circe
    )
    .enablePlugins(NoPublishPlugin)

def addAlias(name: String)(tasks: String*) =
  addCommandAlias(name, tasks.mkString(" ;"))

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
