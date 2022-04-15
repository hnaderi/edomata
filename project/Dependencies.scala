import sbt._

object Dependencies {

  object Versions {
    val cats = "2.7.0"
    val fs2 = "3.2.7"
    val catsEffect = "3.3.5"
    val skunk = "0.3.1"
    val scalaCheck = "1.15.4"
    val MUnit = "0.7.29"
    val CatsEffectMunit = "1.0.7"
    val scalacheckEffectVersion = "1.0.3"
    val doobie = "1.0.0-RC2"
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

    val skunk = Seq(
      "org.tpolecat" %% "skunk-core" % Versions.skunk,
      "org.tpolecat" %% "skunk-circe" % Versions.skunk
    )

    val doobiePG = Seq(
      "org.tpolecat" %% "doobie-core",
      "org.tpolecat" %% "doobie-postgres"
    ).map(_ % Versions.doobie)

    val scalaCheck: Seq[ModuleID] = Seq(
      "org.scalacheck" %% "scalacheck" % Versions.scalaCheck % Test
    )

    val munit = Seq(
      "org.scalameta" %% "munit" % Versions.MUnit,
      "org.scalameta" %% "munit-scalacheck" % Versions.MUnit,
      "org.typelevel" %% "munit-cats-effect-3" % Versions.CatsEffectMunit,
      "org.typelevel" %% "scalacheck-effect-munit" % Versions.scalacheckEffectVersion
    )

    val catsLaws = Seq(
      "org.typelevel" %% "cats-laws" % Versions.cats % Test,
      "org.typelevel" %% "discipline-munit" % "1.0.9" % Test
    )
  }
}
