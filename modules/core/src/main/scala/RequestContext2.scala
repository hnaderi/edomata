package edomata.core

import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

final case class RequestContext2[M[C] <: CommandMetadata[C], C, S](
    aggregateId: String,
    command: M[C],
    state: S
)

trait CommandMetadata[+C] {
  val id: String
  val time: java.time.Instant
  val address: String
  val payload: C
}

final case class CommandMessage[+C](
    id: String,
    time: Instant,
    address: String,
    payload: C
) extends CommandMetadata[C]
