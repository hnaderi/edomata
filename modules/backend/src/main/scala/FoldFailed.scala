package edfsm.backend.test

import cats.data.NonEmptyChain
import cats.implicits.*
import edfsm.backend.FSMDefinition.*

final case class FoldFailed[Domain](
    errors: NonEmptyChain[RejectionFor[Domain]]
) extends Exception(errors.map(_.toString).mkString_(", "))
