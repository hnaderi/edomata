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
      moduleName := s"edfsm-$name"
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
      moduleName := s"edfsm-$name-testkit"
    )
    .enablePlugins(GitlabPlugin)
}

lazy val modules: List[ProjectReference] = List(
  core,
  backend,
  endpoint,
  eventsourcing,
  skunkBackend,
  backendTestkit
)

lazy val root = (project in file("."))
  .settings(
    Common.settings,
    publish / skip := true
  )
  .aggregate(modules: _*)
  .enablePlugins(MicrositesPlugin)
  .enablePlugins(GitVersioning)
  .enablePlugins(GitBranchPrompt)

lazy val docs = project
  .in(file("docs-build"))
  .settings(
    Common.settings,
    mdocVariables := Map(
      "VERSION" -> version.value
    ),
    libraryDependencies := libraryDependencies.value.map(
      _ excludeAll (
        ExclusionRule(organization = "com.lihaoyi", name = "sourcecode_2.13")
      )
    )
  )
  .enablePlugins(MdocPlugin)

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
