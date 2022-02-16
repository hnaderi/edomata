package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Ref
import edomata.backend.CommandMessage

import FSMDefinition.*

final class Timeline[Domain] {
  enum Outcome {
    case Ok, Fail
    case Reject(
        reasons: NonEmptyChain[RejectionFor[Domain]]
    )
  }
  object Outcome {
    extension (outcome: Outcome) {
      def *(i: Int): Destiny = Destiny(outcome, Count.Number(i))
      def loop: Destiny = Destiny(outcome, Count.Infinite)
      def once: Destiny = outcome * 1
    }
  }

  final case class Destiny(
      outcome: Outcome,
      count: Count
  )

  enum Count {
    case Number(i: Int)
    case Infinite
  }

  val ok: Outcome = Outcome.Ok
  val fail: Outcome = Outcome.Fail
  def reject(r: RejectionFor[Domain], rs: RejectionFor[Domain]*): Outcome =
    Outcome.Reject(NonEmptyChain(r, rs: _*))
}

object Timeline {
  def apply[Domain]: Timeline[Domain] = new Timeline[Domain]
}
