package edomata.core

import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.Instant

final case class RequestContext[+C, +S](
    command: CommandMessage[C],
    state: S
)

final case class CommandMessage[+C](
    id: String,
    time: Instant,
    address: String,
    payload: C,
    metadata: MessageMetadata
)
object CommandMessage {
  extension [C, M](cmd: CommandMessage[C]) {
    def buildContext[S, R](
        state: S
    ): RequestContext[C, S] =
      RequestContext(
        command = cmd,
        state = state
      )

    def deriveMeta: MessageMetadata = cmd.metadata.copy(causation = cmd.id.some)
  }
}

final case class MessageMetadata(
    correlation: Option[String],
    causation: Option[String]
)
object MessageMetadata {
  def apply(id: String): MessageMetadata = MessageMetadata(id.some, id.some)
  def apply(correlation: String, causation: String): MessageMetadata =
    MessageMetadata(correlation = correlation.some, causation = causation.some)
}
