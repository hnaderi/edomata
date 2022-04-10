package edomata.core

import cats.data.NonEmptyChain

import java.time.Instant

final case class RequestContext[+C, +S, +M](
    command: CommandMessage[C, M],
    state: S
)

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
        state: S
    ): RequestContext[C, S, M] =
      RequestContext(
        command = cmd,
        state = state
      )
  }
}
