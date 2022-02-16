package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Ref
import edomata.backend.CommandMessage

import FSMDefinition.*

final class FakeCommandHandler[Domain](
    cmds: Ref[IO, List[DomainCommand[Domain]]]
) extends FakeCommandHandlerBuilder(cmds) {

  def ok: CommandHandler[IO, Domain] = cmd => add(cmd).as(Right(()))

  def failing: CommandHandler[IO, Domain] =
    cmd =>
      add(cmd) >> IO.raiseError(
        new Exception("This is a failing command handler!")
      )

  def rejecting(
      reason: RejectionFor[Domain],
      other: RejectionFor[Domain]*
  ): CommandHandler[IO, Domain] = cmd =>
    add(cmd).as(Left(NonEmptyChain.of(reason, other: _*)))

}

object FakeCommandHandler {
  def apply[Domain]: IO[FakeCommandHandler[Domain]] =
    FakeCommandHandlerBuilder(new FakeCommandHandler[Domain](_))
}
