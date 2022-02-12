package edfsm.backend.skunk

import cats.effect._
import cats.effect.implicits.*
import cats.effect.std.Console
import cats.implicits.*
import fs2.Stream
import fs2.io.net.Network
import natchez.Trace
import natchez.Trace.Implicits.noop
import skunk.*
import skunk.codec.text.text
import skunk.implicits.*
import skunk.net.SSLNegotiation
import skunk.net.protocol.Describe

import scala.concurrent.duration.*

import Resource.*

object Database {
  private def escaped(name: String) =
    val escName = s""""$name""""
    sql"#$escName"

  object Schema {
    def setSearchPath(name: String): Command[Void] =
      sql"SET search_path TO ${escaped(name)};".command

    def create(name: String): Command[Void] =
      sql"CREATE SCHEMA IF NOT EXISTS ${escaped(name)}".command

    def drop(name: String): Command[Void] =
      sql"DROP SCHEMA IF EXISTS ${escaped(name)}".command
  }

}
