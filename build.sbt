import Dependencies._
import laika.io.config.SiteConfig
import laika.rewrite.link.ApiLinks
import laika.rewrite.link.LinkConfig
import sbt.ThisBuild
import sbtcrossproject.CrossProject

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.1.2"
val PrimaryJava = JavaSpec.temurin("8")
val LTSJava = JavaSpec.temurin("17")

inThisBuild(
  List(
    tlBaseVersion := "0.1",
    scalaVersion := scala3,
    fork := true,
    Test / fork := false,
    organization := "io.github.hnaderi",
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
        email = "hossein-naderi@hotmail.com",
        url = url("https://hnaderi.ir")
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
  sqlBackend,
  skunkBackend,
  skunkCirceCodecs,
  skunkUpickeCodec,
  doobieBackend,
  doobieCirceCodecs,
  doobieUpickeCodec,
  backendTests,
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
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2022.1"
  )
  .enablePlugins(MdocPlugin)
  .enablePlugins(NoPublishPlugin)

lazy val docs = project
  .in(file("site"))
  .enablePlugins(TypelevelSitePlugin)
  .settings(
    tlSiteHeliumConfig := SiteConfigs(version.value),
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
    sqlBackend.jvm,
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
      doobieBackend.jvm,
      examples.jvm
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
        "io.github.cquiroz" %%% "scala-java-time" % "2.3.0",
        ("org.scala-js" %%% "scalajs-java-securerandom" % "1.0.0")
          .cross(CrossVersion.for3Use2_13)
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
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit % Test,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion % Test,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect % Test,
        "co.fs2" %%% "fs2-core" % Versions.fs2
      )
    )
}

lazy val skunkBackend = module("skunk") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(sqlBackend)
    .dependsOn(backendTests % Test)
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
    .dependsOn(sqlBackend)
    .dependsOn(backendTests % Test)
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

lazy val doobieCirceCodecs = module("doobie-circe") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .enablePlugins(NoPublishPlugin)
    .dependsOn(doobieBackend)
    .settings(
      description := "Circe codecs for doobie backend",
      libraryDependencies ++= Seq(
        "io.circe" %%% "circe-parser" % Versions.circe
      )
    )
}

lazy val doobieUpickeCodec = module("doobie-upickle") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .enablePlugins(NoPublishPlugin)
    .dependsOn(doobieBackend)
    .settings(
      description := "uPickle codecs for doobie backend",
      libraryDependencies ++= Seq(
        "com.lihaoyi" %%% "upickle" % Versions.upickle
      )
    )
}

lazy val backendTests = module("backend-tests") {
  crossProject(JVMPlatform, JSPlatform)
    .crossType(CrossType.Full)
    .enablePlugins(NoPublishPlugin)
    .dependsOn(sqlBackend)
    .settings(
      description := "Integration tests for postgres backends",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "munit-cats-effect-3" % Versions.CatsEffectMunit,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect
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
