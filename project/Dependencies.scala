import sbt.CrossVersion
import sbt._
import sbt.compilerPlugin

object Dependencies {

  object Versions {
    val odin = "0.13.0"
    val cats = "2.7.0"
    val fs2 = "3.2.4"
    val catsEffect = "3.3.5"
    val http4s = "1.0.0-M30"
    val circe = "0.14.1"
    val skunk = "0.2.3"
    val scalaCheck = "1.15.4"
    val MUnit = "0.7.29"
    val CatsEffectMunit = "1.0.7"
    val scalacheckEffectVersion = "1.0.3"
  }

  object Libraries {
    val cats: Seq[ModuleID] = Seq(
      "org.typelevel" %% "cats-core"
    ).map(_ % Versions.cats)

    val catsEffect: Seq[ModuleID] = Seq(
      "org.typelevel" %% "cats-effect" % Versions.catsEffect
    )
    val catsEffectTestKit: Seq[ModuleID] = Seq(
      "org.typelevel" %% "cats-effect-testkit" % Versions.catsEffect % Test
    )

    val fs2: Seq[ModuleID] = Seq(
      "co.fs2" %% "fs2-core" % Versions.fs2
    )

    val circe: Seq[ModuleID] = Seq(
      "io.circe" %% "circe-core",
      "io.circe" %% "circe-refined",
      "io.circe" %% "circe-generic",
      "io.circe" %% "circe-parser"
    ).map(_ % Versions.circe)

    val skunk = Seq(
      "org.tpolecat" %% "skunk-core" % Versions.skunk,
      "org.tpolecat" %% "skunk-circe" % Versions.skunk
    )

    val http4s: Seq[ModuleID] = Seq(
      "org.http4s" %% "http4s-dsl",
      "org.http4s" %% "http4s-blaze-server",
      "org.http4s" %% "http4s-blaze-client",
      "org.http4s" %% "http4s-circe"
    ).map(_ % Versions.http4s)

    val scalaCheck: Seq[ModuleID] = Seq(
      "org.scalacheck" %% "scalacheck" % Versions.scalaCheck % Test
    )

    val odin: Seq[ModuleID] = Seq(
      "com.github.valskalla" %% "odin-core",
      "com.github.valskalla" %% "odin-json", //to enable JSON formatter if needed
      "com.github.valskalla" %% "odin-extras", //to enable additional features if needed (see docs)
      "com.github.valskalla" %% "odin-slf4j"
    ).map(_ % Versions.odin)

    val munit = Seq(
      "org.scalameta" %% "munit" % Versions.MUnit,
      "org.scalameta" %% "munit-scalacheck" % Versions.MUnit,
      "org.typelevel" %% "munit-cats-effect-3" % Versions.CatsEffectMunit,
      "org.typelevel" %% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion
    )
  }
}
