package edfsm.backend

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Ref
import edfsm.backend.CommandMessage

import FSMDefinition.*
import FakeCommandHandlerBuilder.*

abstract class FakeCommandHandlerBuilder[Domain](
    cmds: CMDList[Domain]
) {
  protected def add(cmd: DomainCommand[Domain]) = cmds.update(_ :+ cmd)

  def commands: IO[List[CommandFor[Domain]]] =
    inbox.map(_.map(_.payload))
  def commandMessages: IO[List[DomainCommand[Domain]]] = inbox

  def inbox: IO[List[DomainCommand[Domain]]] = cmds.get
  def markAsRead: IO[Unit] = cmds.set(List.empty)
  def getAndCleanInbox: IO[List[DomainCommand[Domain]]] = inbox <* markAsRead
}

object FakeCommandHandlerBuilder {
  type CMDList[Domain] = Ref[IO, List[DomainCommand[Domain]]]
  def apply[CH <: FakeCommandHandlerBuilder[Domain], Domain](
      f: CMDList[Domain] => CH
  ): IO[CH] = IO.ref(List.empty[DomainCommand[Domain]]).map(f)
}
