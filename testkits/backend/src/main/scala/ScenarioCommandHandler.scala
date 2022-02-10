package edfsm.backend

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Ref
import edfsm.protocols.command.CommandMessage

import FSMDefinition.*
import ScenarioCommandHandler.*

final class ScenarioCommandHandler[TL <: Timeline[Domain], Domain](
    cmds: Ref[IO, List[DomainCommand[Domain]]],
    val timeline: TL
) extends FakeCommandHandlerBuilder(cmds) {

  import timeline.*

  private def translate(outcome: Outcome): DomainAction[IO, Domain] =
    outcome match {
      case Outcome.Ok        => IO(Right(()))
      case Outcome.Fail      => IO.raiseError(Error.Fail)
      case Outcome.Reject(r) => IO(Left(r))
    }

  def sequence(d: Destiny, ds: Destiny*): IO[CommandHandler[IO, Domain]] =
    val destinies = d +: ds
    for {
      ref <- IO.ref(destinies)
      head <- IO.ref(d)
      next = ref.updateAndGet(_.tail).map(_.headOption).flatMap {
        case Some(d) => head.set(d)
        case None    => IO.unit
      }
    } yield cmd =>
      add(cmd) >>
        head.get.flatMap {
          case Destiny(a, Count.Number(i)) if i > 0 =>
            val transition =
              if i == 1 then next else IO.unit

            head.set(Destiny(a, Count.Number(i - 1))) >>
              transition >> translate(a)
          case Destiny(a, Count.Infinite) => translate(a)
          case Destiny(_, Count.Number(_)) =>
            IO.raiseError(Error.TimelineFinished)
        }
}

object ScenarioCommandHandler {
  def apply[Domain](
      timeline: Timeline[Domain]
  ): IO[ScenarioCommandHandler[timeline.type, Domain]] =
    FakeCommandHandlerBuilder(
      new ScenarioCommandHandler[timeline.type, Domain](_, timeline)
    )

  enum Error(msg: String) extends Exception(msg) {
    case Fail extends Error("This is a failing command handler!")
    case TimelineFinished
        extends Error("Defined timeline finished, cannot continue!")
  }

}
