package edfsm.core

import cats.data.NonEmptyList

final case class EDFSM[F[_], S, E](
    read: F[Option[S]],
    append: NonEmptyList[E] => F[Unit]
)
