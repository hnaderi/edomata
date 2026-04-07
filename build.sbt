import Dependencies._
import sbt.ThisBuild
import sbtcrossproject.CrossProject

Global / onChangedBuildSource := ReloadOnSourceChanges

lazy val scala3 = "3.3.6"

inThisBuild(
  List(
    tlBaseVersion := "0.12",
    tlMimaPreviousVersions := Set.empty,
    scalaVersion := scala3,
    fork := true,
    Test / fork := false,
    organization := "dev.bsg",
    organizationName := "Beyond Scale Group",
    startYear := Some(2021),
    tlCiReleaseBranches := Seq(),
    licenses := Seq(License.Apache2),
    developers := List(
      Developer(
        id = "hnaderi",
        name = "Hossein Naderi",
        email = "mail@hnaderi.dev",
        url = url("https://hnaderi.dev")
      )
    ),
    credentials ++= {
      sys.env
        .get("GITHUB_TOKEN")
        .map { token =>
          Credentials(
            "GitHub Package Registry",
            "maven.pkg.github.com",
            "_",
            token
          )
        }
        .toSeq
    }
  )
)

// GitHub Packages publishTo override — must be at project scope to beat
// sbt-typelevel's TypelevelSonatypePlugin which sets publishTo per-project.
lazy val ghpPublishSettings: Seq[Setting[_]] =
  if (sys.env.contains("PUBLISH_TO_GITHUB"))
    Seq(
      publishTo := Some(
        "GitHub Packages" at "https://maven.pkg.github.com/beyond-scale-group/edomata"
      )
    )
  else Seq.empty

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
    .settings(ghpPublishSettings)

lazy val modules = List(
  core,
  backend,
  saas,
  saasSkunk,
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
  e2eTests,
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
    libraryDependencies += "net.sourceforge.plantuml" % "plantuml" % "1.2026.2"
  )
  .enablePlugins(MdocPlugin)
  .enablePlugins(NoPublishPlugin)

lazy val docs = project
  .in(file("site"))
  .enablePlugins(MdocPlugin)
  .disablePlugins(TypelevelSettingsPlugin)
  .settings(
    mdocIn := (ThisBuild / baseDirectory).value / "docs",
    mdocOut := (ThisBuild / baseDirectory).value / "website" / "docs",
    mdocVariables := Map(
      "VERSION" -> version.value
    )
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

lazy val saas = module("saas") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(core, backend, postgres)
    .settings(
      description := "Multi-tenant SaaS CRUD abstractions for edomata"
    )
}

lazy val saasSkunk = module("saas-skunk") {
  crossProject(JVMPlatform, JSPlatform, NativePlatform)
    .crossType(CrossType.Pure)
    .dependsOn(saas, skunkBackend)
    .settings(
      description := "SaaS-aware Skunk backend for edomata"
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
      libraryDependencies += "com.armanbilge" %%% "epollcat" % "0.1.7",
      Test / envVars ++= Map("S2N_DONT_MLOCK" -> "1")
    )
}

lazy val e2eTests = module("e2e") {
  crossProject(JVMPlatform)
    .crossType(CrossType.Pure)
    .enablePlugins(NoPublishPlugin)
    .dependsOn(
      driverTests,
      skunkBackend,
      skunkCirceCodecs,
      doobieBackend,
      doobieCirceCodecs
    )
    .settings(
      description := "E2E tests for postgres backends",
      libraryDependencies ++= Seq(
        "io.circe" %%% "circe-generic" % Versions.circe
      )
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
      saas,
      saasSkunk,
      skunkBackend,
      skunkCirceCodecs,
      skunkUpickleCodecs
    )
    .settings(
      libraryDependencies ++= Seq(
        "io.circe" %%% "circe-generic" % Versions.circe
      )
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
