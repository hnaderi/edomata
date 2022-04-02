package edomata.core

import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

final case class RequestContext2[+C, +S, +M](
    id: String,
    aggregateId: String,
    command: C,
    state: S,
    metadata: M
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
    def buildContext[S](state: S): RequestContext2[C, S, M] = RequestContext2(
      id = cmd.id,
      aggregateId = cmd.address,
      command = cmd.payload,
      state = state,
      metadata = cmd.metadata
    )
  }
}
