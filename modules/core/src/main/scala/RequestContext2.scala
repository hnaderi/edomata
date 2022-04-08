package edomata.core

import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

enum RequestContext2[+C, +S, +M, +R] {
  case Valid(
      command: CommandMessage[C, M],
      state: S,
      version: Long
  )
  case Conflict(err: NonEmptyChain[R])
  case Redundant
}

final case class CommandMessage[+C, +M](
    id: String,
    time: Instant,
    address: String,
    payload: C,
    metadata: M
)
object CommandMessage {
  extension [C, M](cmd: CommandMessage[C, M]) {
    def buildContext[S, R](
        state: S,
        version: Long
    ): RequestContext2.Valid[C, S, M, R] =
      RequestContext2.Valid(
        command = cmd,
        state = state,
        version = version
      )
  }
}
