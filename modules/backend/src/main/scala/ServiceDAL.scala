package edfsm.backend

import cats.Show
import cats.data.NonEmptyChain
import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edfsm.eventsourcing.*
import io.circe.Codec
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.syntax.*
import edfsm.protocols.command.CommandMessage

import java.time.OffsetDateTime

import FSMDefinition.*

trait ServiceDAL[F[_], Domain] {
  def appendJournal(
      streamId: String,
      time: OffsetDateTime,
      version: EventVersion,
      events: NonEmptyChain[InternalEventFor[Domain]]
  ): F[Unit]
  def readFromJournal(streamId: String): F[AggregateState[StateFor[Domain]]]
  def outbox(t: ExternalEventFor[Domain], time: OffsetDateTime): F[Unit]
  def appendCmdLog(cmd: DomainCommand[Domain]): F[Unit]
  def containsCmd(id: String): F[Boolean]

  def transaction[T](f: F[T]): F[T]
}
