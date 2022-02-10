package edfsm.backend

import cats.Functor
import cats.Monad
import cats.Show
import cats.data.EitherNec
import cats.effect.*
import cats.effect.implicits.*
import cats.effect.std.Random
import cats.effect.std.Semaphore
import cats.implicits.*
import edfsm.core.Action
import edfsm.core.Decision
import edfsm.core.DecisionT
import edfsm.eventsourcing.*
import edfsm.protocols.command.CommandMessage

import FSMDefinition.*
import TestSystem.*
import cats.MonadError
import edfsm.backend.test.FoldFailed

trait TestSystem[F[_], Domain] {
  def tell(cmd: DomainCommand[Domain]): F[TestResult[Domain]]
  def handler: CommandHandler[F, Domain]
  def inspect(id: String): F[TestUnit[Domain]]
}

object TestSystem {

  final case class TestUnit[Domain](
      state: StateFor[Domain],
      notifications: List[ExternalEventFor[Domain]],
      journal: List[InternalEventFor[Domain]]
  )

  final case class TestResult[Domain](
      decision: DomainDecision[Domain, Unit],
      notifications: Seq[ExternalEventFor[Domain]],
      newState: StateFor[Domain]
  )

  def handle[F[_], Domain](
      fold: DomainTransition[Domain],
      logic: DomainLogic[F, Domain]
  )(using
      F: MonadError[F, Throwable]
  ): RequestContext[Domain] => F[TestResult[Domain]] = req =>
    logic(req).run.flatMap { res =>
      val newS = res.decision match {
        case Decision.Accepted(evs, _) =>
          evs.foldLeftM(req.state)((s, e) => fold(e)(s).toEither)
        case Decision.InDecisive(_) =>
          req.state.asRight
        case Decision.Rejected(errs) =>
          req.state.asRight
      }

      F.fromEither(
        newS
          .map(TestResult(res.decision, res.notifications, _))
          .leftMap(FoldFailed(_))
      )
    }

  private def eventsFrom[Domain](
      dec: DomainDecision[Domain, Unit]
  ): List[InternalEventFor[Domain]] =
    dec match {
      case Decision.Accepted(evs, _) => evs.toList
      case _                         => Nil
    }

  def apply[F[_]: Concurrent, Domain](
      empty: StateFor[Domain],
      fold: DomainTransition[Domain],
      logic: DomainLogic[F, Domain]
  ): F[TestSystem[F, Domain]] =
    val program = handle(fold, logic)
    val default = TestUnit(empty, Nil, Nil)
    Ref
      .of(Map.empty[String, TestUnit[Domain]])
      .map(state =>
        new TestSystem[F, Domain] {
          def tell(
              cmd: DomainCommand[Domain]
          ): F[TestResult[Domain]] = for {
            s <- inspect(cmd.address)
            ctx = RequestContext(
              aggregateId = cmd.address,
              command = cmd,
              state = s.state
            )
            res <- program.apply(ctx)
            _ <- state.update(
              _.updated(
                ctx.aggregateId,
                s.copy(
                  state = res.newState,
                  journal = s.journal ++ eventsFrom(res.decision),
                  notifications = s.notifications ++ res.notifications
                )
              )
            )
          } yield res

          def handler: CommandHandler[F, Domain] = tell(_).map {
            _.decision match {
              case Decision.Rejected(errs) => errs.asLeft
              case _                       => ().asRight
            }
          }
          def inspect(id: String): F[TestUnit[Domain]] =
            state.get.map(_.getOrElse(id, default))
        }
      )
}
