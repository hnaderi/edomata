import Dependencies._
import sbt.ThisBuild
import sbtcrossproject.CrossProject

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.3.3"

inThisBuild(
  List(
    tlBaseVersion := "0.12",
    scalaVersion := scala3,
    fork := true,
    Test / fork := false,
    organization := "dev.hnaderi",
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
        email = "mail@hnaderi.dev",
        url = url("https://hnaderi.dev")
      )
    )
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
  skunkJsoniterCodecs,
  skunkUpickleCodecs,
  doobieBackend,
  doobieCirceCodecs,
  doobieJsoniterCodecs,
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
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2024.5"
  )
  .enablePlugins(MdocPlugin)
  .enablePlugins(NoPublishPlugin)

lazy val docs = project
  .in(file("site"))
  .enablePlugins(EdomataSitePlugin)
  .disablePlugins(TypelevelSettingsPlugin)
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
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .settings(
      description := "Purely functional event-driven automata",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "cats-core" % Versions.cats,
        "org.typelevel" %%% "cats-laws" % Versions.cats % Test,
        "org.typelevel" %%% "discipline-munit" % Versions.CatsEffectMunit % Test
      )
    )
    .jsSettings(
      libraryDependencies ++= Seq(
        "io.github.cquiroz" %%% "scala-java-time" % "2.6.0"
      )
    )
}

lazy val backend = module("backend") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core)
    .settings(
      description := "Performant eventsourcing backend for edomata",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "cats-effect" % Versions.catsEffect,
        "org.typelevel" %%% "munit-cats-effect" % Versions.CatsEffectMunit % Test,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion % Test,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect % Test,
        "co.fs2" %%% "fs2-core" % Versions.fs2
      )
    )
}

lazy val postgres = module("postgres") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core, backend)
    .settings(
      description := "Postgres common components"
    )
}

lazy val skunkBackend = module("skunk") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(postgres)
    .dependsOn(driverTests % Test)
    .settings(
      description := "Skunk based backend for edomata",
      libraryDependencies ++= Seq(
        "org.tpolecat" %%% "skunk-core" % Versions.skunk,
        "org.typelevel" %%% "munit-cats-effect" % Versions.CatsEffectMunit % Test
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
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(skunkBackend)
    .settings(
      description := "Circe codecs for skunk backend",
      libraryDependencies ++= Seq(
        "org.tpolecat" %%% "skunk-circe" % Versions.skunk
      )
    )
}

lazy val skunkJsoniterCodecs = module("skunk-jsoniter") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(skunkBackend)
    .settings(
      description := "Jsoniter codecs for skunk backend",
      libraryDependencies ++= Seq(
        "com.github.plokhotnyuk.jsoniter-scala" %%% "jsoniter-scala-core" % Versions.jsoniter
      )
    )
}

lazy val skunkUpickleCodecs = module("skunk-upickle") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
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

lazy val doobieJsoniterCodecs = module("doobie-jsoniter") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .dependsOn(doobieBackend)
    .settings(
      description := "Jsoniter codecs for doobie backend",
      libraryDependencies ++= Seq(
        "com.github.plokhotnyuk.jsoniter-scala" %%% "jsoniter-scala-core" % Versions.jsoniter
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
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Full)
    .enablePlugins(NoPublishPlugin, BuildInfoPlugin)
    .dependsOn(postgres)
    .settings(
      buildInfoKeys := Seq[BuildInfoKey](crossProjectPlatform),
      buildInfoPackage := "tests",
      buildInfoOptions ++= Seq(
        BuildInfoOption.ConstantValue,
        BuildInfoOption.PackagePrivate
      ),
      description := "Integration tests for postgres backends",
      libraryDependencies ++= Seq(
        "org.typelevel" %%% "munit-cats-effect" % Versions.CatsEffectMunit,
        "org.typelevel" %%% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion,
        "org.typelevel" %%% "cats-effect-testkit" % Versions.catsEffect
      )
    )
    .nativeSettings(
      libraryDependencies += "com.armanbilge" %%% "epollcat" % "0.1.6",
      Test / envVars ++= Map("S2N_DONT_MLOCK" -> "1")
    )
}

lazy val munitTestkit = module("munit") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
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
