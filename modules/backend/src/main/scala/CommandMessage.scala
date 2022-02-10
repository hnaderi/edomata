package edfsm.protocols.command

import CommandMessage._
import java.time.Instant

type CommandId = String
type Address = String

final case class CommandMessage[+C](
    id: CommandId,
    time: Instant,
    address: Address,
    payload: C
)
