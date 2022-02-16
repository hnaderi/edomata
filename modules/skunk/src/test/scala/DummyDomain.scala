package edomata.backend.skunk

import cats.Monad
import cats.implicits.*
import edomata.backend.DomainLogic
import edomata.backend.DomainTransition
import edomata.backend.FSMDefinition.*
import edomata.core.Action.*

type DummyDomain = (
    HasState[Long],
    HasCommand[Int],
    HasRejection[Rejection],
    HasInternalEvent[Int],
    HasExternalEvent[Long]
)

enum Rejection {
  case Failure
}

object DummyDomain {
  def apply[F[_]: Monad]: DomainLogic[F, DummyDomain] = req =>
    val cmd = req.command.payload
    req.state match {
      case last if cmd > 0 => accept(cmd).publish(last + cmd)
      case _               => reject(Rejection.Failure)
    }

  def transition: DomainTransition[DummyDomain] = e => s => (e + s).validNec
}
