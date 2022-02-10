import sbt.Compile
import sbt.Keys.compile
import sbt.Keys.libraryDependencies
import sbt.Keys.organization
import sbt.Keys.organizationName
import sbt.Keys.scalacOptions

object Common {
  private val commonScalac3Options = Seq(
    "-deprecation",
    "-encoding",
    "UTF-8",
    "-feature",
    "-unchecked",
    "-Xfatal-warnings"
//    "-Ywarn-dead-code",
  )

  val settings = Seq(
    organization := "ir.hnaderi",
    organizationName := "hnaderi",
    scalacOptions ++= commonScalac3Options
  )
}
